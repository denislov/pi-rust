# Design: pi-coding-agent M7 — config + auth base (Rust-native, TOML)

- Date: 2026-06-19
- Status: Draft (pending review)
- Scope: Milestone M7. Add a Rust-native settings + auth subsystem to `pi-coding-agent`: layered TOML settings (global + project), a TOML auth store, an API-key resolution chain, and `pi-ai` env-key expansion. Wire both into model/key resolution.
- Depends on: `pi-coding-agent` `args.rs` + `runtime.rs` (current model/key resolution); `pi-ai` `util/env_keys.rs` (env-var → provider map). No dependency on OAuth (deferred to M8).
- Roadmap: [docs/roadmap/M7-config-auth.md](../../roadmap/M7-config-auth.md)

## 1. Context

`pi-coding-agent` has **no** config/settings/auth code today. `runtime.rs` resolves the API key only from the `--api-key` CLI flag (no env-var or file fallback), and the model from `--model` → override → a hardcoded default. There is no global/project settings file, no stored credentials, and no env-var key resolution wired into the CLI.

The TS reference (`pi/packages/coding-agent/src/core/settings-manager.ts`, `auth-storage.ts`) keeps settings at `~/.pi/agent/settings.json` (+ project `./.pi/settings.json`) and credentials at `~/.pi/agent/auth.json`, with deep merge and a multi-source key-resolution chain. We **do not** copy its format or location.

Confirmed product decisions for pi-rust:
- **Config is Rust-native and fully independent of pi** (no shared files, no reading pi's settings/auth). Sessions are format-compatible with pi but live in pi-rust's own location; interop is opt-in via `--session-dir`.
- **Config format is TOML** (settings + auth), matching Rust conventions (Cargo), comment-friendly.
- **Scope is core-first (YAGNI):** only settings fields whose features already exist in pi-rust are implemented now; the long tail is deferred to the milestones that introduce those features.

## 2. Goals and success criteria

Add a Rust-native, TOML-based settings + auth base and wire it into model/key resolution.

Done when:

1. `cargo fmt --check`, `cargo test -p pi-coding-agent`, `cargo test -p pi-ai`, `cargo test --workspace`, and `cargo check --workspace` pass from `pi-rust/`.
2. A `config` module loads layered settings: built-in defaults, overlaid by global `~/.pi-rust/settings.toml`, overlaid by project `./.pi-rust/settings.toml` (precedence: **project > global > defaults**); nested objects merge recursively, scalars/arrays override wholesale.
3. `PI_RUST_DIR` env var overrides the global config directory; absence of any settings file yields defaults with no error.
4. An `AuthStore` loads `~/.pi-rust/auth.toml` (per-provider `{ type = "api_key", key = "..." }` entries); `oauth`-typed entries are parsed and skipped (reserved for M8) without error.
5. API-key resolution follows the chain **`--api-key` (CLI) > `auth.toml` (api_key) > environment variable > none**, and `runtime.rs` uses it instead of reading `--api-key` directly.
6. Auth `key` values support `$VAR` / `${VAR}` substitution and `$$` → literal `$`; an unset `$VAR` resolves to "no key from this source" with a diagnostic (not a hard error). `!command` execution is **not** implemented.
7. When `--model`/`--provider` are absent, `select_model()` falls back to `default_provider`/`default_model` from settings.
8. `pi-ai` `env_keys.rs` covers the additional providers `minimax`, `minimax-cn`, `xiaomi*`, `github-copilot`; `amazon-bedrock` and `google-vertex` detection return a `<authenticated>` sentinel only (no real auth in M7).
9. Config/auth load failures are **non-fatal**: collected as `Vec<ConfigDiagnostic>` and drained to stderr at startup. `auth.toml` with permissions looser than `0600` produces a warning diagnostic (Unix).
10. The whole suite is offline and deterministic — no network, no real credentials; env-dependent tests inject their own env and temp dirs.

## 3. Non-goals (this milestone)

- OAuth login/refresh and the `oauth` auth entry resolution (M8).
- `!command` value execution (M8+).
- Bedrock SigV4 signing and Vertex ADC real authentication (M8) — only sentinel detection here.
- Writing settings back / persisting modified fields (lands with the command that needs it, e.g. `/model` in M11).
- Settings fields for not-yet-built features: theme/markdown, terminal/images, editor/UI, keybindings, packages/extensions/skills/prompts/themes lists, scoped models, warnings, changelog.
- Interactive settings/auth UI (M11), `/login` `/logout` commands (M8/M11).
- Migration from pi's config (config is independent; nothing to migrate).

## 4. Design

### 4.1 Module structure

New module `crates/pi-coding-agent/src/config/`:

| File | Responsibility |
|---|---|
| `mod.rs` | `load_config(cli, cwd) -> (Config, Vec<ConfigDiagnostic>)` aggregate entry; `Config { settings, auth }`; `ConfigDiagnostic` type; re-exports |
| `paths.rs` | resolve global dir (`PI_RUST_DIR` or `~/.pi-rust/`) and project dir (`<cwd>/.pi-rust/`); file paths for settings/auth |
| `settings.rs` | `Settings` (resolved) + `PartialSettings` (per-layer, all-Option); load, deep-merge, resolve-with-defaults |
| `auth.rs` | `AuthStore`, `AuthEntry`, `resolve_api_key`, `resolve_config_value` ($ENV substitution), `0600` check |

`runtime.rs` and `args.rs` are edited to consume the module. `pi-ai/src/util/env_keys.rs` is extended.

### 4.2 Paths (`paths.rs`)

```rust
pub struct ConfigPaths { pub global_dir: PathBuf, pub project_dir: PathBuf }

pub fn resolve(cwd: &Path) -> ConfigPaths {
    let global_dir = match std::env::var_os("PI_RUST_DIR") {
        Some(p) => PathBuf::from(p),
        None => dirs::home_dir().unwrap_or_default().join(".pi-rust"),
    };
    ConfigPaths { global_dir, project_dir: cwd.join(".pi-rust") }
}
```

- Global settings: `global_dir/settings.toml`; global auth: `global_dir/auth.toml`.
- Project settings: `project_dir/settings.toml` (project has no auth file in M7 — credentials are global-only).
- Uses the `dirs` crate for the home dir (cross-platform). Directories are **not** created in M7 (read-only load; creation happens when something first writes).

### 4.3 Settings (`settings.rs`)

Two structs: a per-layer `PartialSettings` (all fields `Option`, `#[serde(default)]`, used for deserialization + merge) and a resolved `Settings` (concrete, defaults applied). M7 fields only:

```rust
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PartialSettings {
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    pub default_thinking_level: Option<ThinkingLevel>,
    pub transport: Option<Transport>,            // auto | sse | websocket
    pub steering_mode: Option<QueueMode>,        // all | one-at-a-time
    pub follow_up_mode: Option<QueueMode>,
    pub session_dir: Option<PathBuf>,
    pub compaction: Option<PartialCompaction>,
    pub retry: Option<PartialRetry>,
}
// PartialCompaction { enabled, reserve_tokens, keep_recent_tokens } (all Option)
// PartialRetry { enabled, max_retries, base_delay_ms } (all Option)
```

- **Load:** read a layer file → `toml::from_str::<PartialSettings>()`. Missing file → `PartialSettings::default()` (all `None`). Parse error → default + push diagnostic.
- **Merge:** `merge(base, over)` — for each scalar/array field, `over.or(base)`; for nested (`compaction`, `retry`), recurse field-wise. Applied as `merge(global, project)`.
- **Resolve:** `PartialSettings -> Settings` fills built-in defaults (compaction enabled/16384/20000; retry enabled/3/2000; transport auto; queue modes one-at-a-time; thinking off). `default_provider`/`default_model`/`session_dir` stay `Option` on the resolved struct (no global default).
- `deny_unknown_fields` surfaces typos as parse diagnostics rather than silent drops.

### 4.4 Auth (`auth.rs`)

```rust
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthEntry {
    ApiKey { key: String },
    Oauth(toml::Value),   // reserved for M8; parsed but not resolved in M7
}

pub struct AuthStore { entries: BTreeMap<String, AuthEntry> }  // key = provider id
```

`auth.toml` shape:

```toml
[anthropic]
type = "api_key"
key  = "$ANTHROPIC_API_KEY"

[openai]
type = "api_key"
key  = "sk-literal-or-$OPENAI_API_KEY"
```

**Resolution chain** — `resolve_api_key(provider, cli_key, &auth, env) -> Option<ResolvedKey>` where `ResolvedKey { value: String, source: KeySource }`:

1. `cli_key` (from `--api-key`) → `KeySource::Cli`.
2. `auth.entries[provider]` if `ApiKey` → `resolve_config_value(key)` → `KeySource::AuthFile`. (`Oauth` → skip in M7.)
3. `pi_ai::env_api_key(provider)` → `KeySource::Env`.
4. else `None`.

**Value substitution** — `resolve_config_value(raw) -> Result<Option<String>, ConfigDiagnostic>`:
- `$$` → literal `$`; `$!` → literal `!` (forward-compat with deferred `!command`).
- `$VAR` / `${VAR}` → `env::var(VAR)`; unset → `Ok(None)` + diagnostic "env VAR referenced by auth.toml is unset".
- otherwise literal string.

**Permissions (Unix):** after locating `auth.toml`, if mode & `0o077 != 0`, push a warning diagnostic (does not block load). Guarded by `#[cfg(unix)]`.

### 4.5 `pi-ai` env_keys expansion

Extend `crates/pi-ai/src/util/env_keys.rs`:
- Add `minimax` (`MINIMAX_API_KEY`), `minimax-cn` (`MINIMAX_CN_API_KEY`), `xiaomi`/`xiaomi-token-plan-*` (respective vars), `github-copilot` (`COPILOT_GITHUB_TOKEN`).
- `amazon-bedrock`: if any of `AWS_PROFILE` / `AWS_ACCESS_KEY_ID` / `AWS_BEARER_TOKEN_BEDROCK` present → return sentinel `"<authenticated>"`.
- `google-vertex`: if `GOOGLE_APPLICATION_CREDENTIALS` set or default ADC file exists → sentinel `"<authenticated>"`.
- The sentinel signals "credentials present, provider will self-auth" — M7 does not act on it beyond resolution; real signing/ADC is M8.

### 4.6 CLI / runtime integration

- `runtime.rs`: replace direct `--api-key` read with `auth::resolve_api_key(provider, cli_key, &config.auth, std::env::vars)`. The resolved value flows into `StreamOptions.api_key` as today. A sentinel value is passed through unchanged (provider-side handling is M8).
- `select_model()`: when `--model` absent, use `settings.default_model`; when `--provider` absent, use `settings.default_provider`; final fallback stays the existing hardcoded default.
- `args.rs`: add `--provider <id>` (minimal: feeds provider into key/model resolution). Other M10 flags (`--no-context-files`, etc.) are out of scope here.
- `load_config` is called once at startup; diagnostics drain to stderr. A `quiet_startup` gate is deferred along with the startup-settings group (its field does not exist in M7).

### 4.7 Error handling

- `ConfigDiagnostic { severity: Warn | Error, message: String, source: PathBuf | None }`. Collected, never panicked.
- Parse failure of a settings/auth file → `Warn` diagnostic + that layer treated as empty.
- Unset `$VAR` in auth value → `Warn`; loose `auth.toml` perms → `Warn`.
- No new public error enum is required for M7; diagnostics are data, not control flow.

### 4.8 Testing strategy (offline)

`config/settings.rs` unit tests:
- `merge_project_overrides_global_overrides_default`: build two `PartialSettings`, assert resolved precedence; assert nested `compaction`/`retry` merge field-wise (project sets only `max_retries`, global sets `base_delay_ms`, both survive).
- `missing_files_yield_defaults`: temp dir with no files → resolved defaults, no diagnostics.
- `unknown_field_produces_diagnostic`: settings.toml with a typo'd key → diagnostic, other fields still load.

`config/auth.rs` unit tests:
- `resolve_chain_precedence`: with CLI key + auth entry + env all set, assert `Cli` wins; remove CLI → `AuthFile`; remove entry → `Env`; remove all → `None`.
- `dollar_env_substitution`: `key = "$FOO"` with `FOO=bar` → `bar`; unset → `None` + diagnostic; `$$` → literal `$`.
- `oauth_entry_skipped`: `type = "oauth"` entry → resolution returns `None`, no error.
- `loose_permissions_warn` (`#[cfg(unix)]`): chmod temp `auth.toml` to `0644` → warning diagnostic.

`pi-ai` `env_keys` unit tests:
- `new_providers_resolved`: inject `MINIMAX_API_KEY` etc., assert returned; `bedrock_vertex_sentinel`: inject `AWS_PROFILE` / ADC path → `<authenticated>`.

`runtime` integration test:
- `runtime_uses_env_key_when_no_cli_flag`: temp `PI_RUST_DIR`, set provider env key, no `--api-key`, assert built `StreamOptions.api_key` is the env value (faux provider; no network).

Env-mutating tests serialize via a shared guard (env is process-global) or set vars within a single test to avoid cross-test races (mirrors the existing global-registry isolation note in cross-cutting).

### 4.9 File structure

| File | Operation |
|---|---|
| `pi-coding-agent/src/config/mod.rs` | new: `Config`, `ConfigDiagnostic`, `load_config` |
| `pi-coding-agent/src/config/paths.rs` | new: dir/path resolution + `PI_RUST_DIR` |
| `pi-coding-agent/src/config/settings.rs` | new: `Settings`/`PartialSettings`, merge, resolve, tests |
| `pi-coding-agent/src/config/auth.rs` | new: `AuthStore`/`AuthEntry`, resolution chain, `$ENV` substitution, tests |
| `pi-coding-agent/src/lib.rs` | edit: `mod config;` + re-exports |
| `pi-coding-agent/src/runtime.rs` | edit: use `auth::resolve_api_key`; settings fallback in `select_model` |
| `pi-coding-agent/src/args.rs` | edit: add `--provider` |
| `pi-coding-agent/Cargo.toml` | edit: add `toml`, `dirs` deps |
| `pi-ai/src/util/env_keys.rs` | edit: add providers + bedrock/vertex sentinels + tests |
| `pi-coding-agent/tests/` | new/edit: runtime key-resolution integration test |

### 4.10 Verification

Run from `pi-rust/`:

```bash
cargo fmt --check
cargo test -p pi-ai
cargo test -p pi-coding-agent
cargo test --workspace
cargo check --workspace
```

All must pass.

## 5. Key decisions and constraints

- **Fully independent of pi** — own dir `~/.pi-rust/` (`PI_RUST_DIR` override); no reading pi's files. Sessions stay format-compatible; interop is opt-in via `--session-dir`.
- **TOML for settings + auth** — idiomatic Rust, comment-friendly, single format.
- **Partial-then-resolve merge** — per-layer all-`Option` structs merge cleanly (project > global > defaults), then resolve to a concrete `Settings` with defaults. Nested objects merge field-wise.
- **YAGNI field scope** — only fields whose features exist today; `deny_unknown_fields` turns future/typo keys into diagnostics rather than silent drops (the deferred groups will add their fields when they land).
- **Resolution chain `CLI > auth.toml > env > none`** — mirrors pi minus the deferred OAuth/`!command` sources.
- **`$ENV` substitution only** — `$VAR`/`${VAR}`/`$$`; `!command` deferred (security surface). `$!` reserved.
- **Sentinels for self-authing providers** — Bedrock/Vertex resolution returns `<authenticated>`; real auth is M8.
- **Diagnostics, not panics** — all load/parse/permission issues are collected and surfaced; bad config never crashes startup.
- **Offline tests** — env-injection + temp dirs; no network, no real keys; env-mutating tests avoid cross-test races.
