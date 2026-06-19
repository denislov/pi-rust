# M7 — config + auth base Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Rust-native, TOML-based settings + auth subsystem to `pi-coding-agent` and wire it into model selection and API-key resolution.

**Architecture:** A new `config` module (`paths`, `settings`, `auth`, `mod`) loads layered TOML settings (project over global over defaults) and a per-provider auth store. A resolution chain (`CLI > auth.toml > env > none`) replaces the direct `--api-key` read in the headless run path; `select_model` gains a `default_model` fallback. `pi-ai`'s env-key map is expanded. All loads are non-fatal — failures become `ConfigDiagnostic`s drained to stderr.

**Tech Stack:** Rust 2024, `serde`, `toml`, `dirs`, `thiserror` (existing), `tempfile` (dev, existing).

Spec: [docs/superpowers/specs/2026-06-19-pi-coding-agent-m7-config-auth-design.md](../specs/2026-06-19-pi-coding-agent-m7-config-auth-design.md)

## Global Constraints

- Verification (run from `pi-rust/`): `cargo fmt --check`, `cargo test -p pi-ai`, `cargo test -p pi-coding-agent`, `cargo test --workspace`, `cargo check --workspace` — all pass.
- Offline + deterministic: no network, no real credentials. Env-mutating tests set/remove their own vars within the test; temp dirs via `tempfile`.
- Config is Rust-native and independent of pi: global dir `~/.pi-rust/` (override `PI_RUST_DIR`), project dir `<cwd>/.pi-rust/`. Format is **TOML** (settings + auth).
- Settings precedence: **project > global > built-in defaults**; nested objects merge field-wise, scalars override wholesale. `#[serde(default, deny_unknown_fields)]` on every settings struct.
- Key-resolution chain: `--api-key` (CLI) > `auth.toml` (`api_key`) > env var (`pi_ai::env_api_key`) > none. `oauth` entries parse but are skipped (reserved for M8). `$VAR`/`${VAR}`/`$$`/`$!` substitution; no `!command` execution.
- Diagnostics, not panics: all load/parse/permission issues collected as `ConfigDiagnostic` and drained to stderr.
- Consumed in M7: `default_model` (via `select_model`) and the API key (via `resolve_api_key`), in the **headless print/json path**. Interactive-path key/model wiring is deferred (reuses the same helpers later). Other settings are parsed/merged but not yet consumed.

---

### Task 1: Dependencies + `config` scaffold (paths + diagnostics)

**Files:**
- Modify: `crates/pi-coding-agent/Cargo.toml` (add `toml`, `dirs`)
- Create: `crates/pi-coding-agent/src/config/mod.rs` (`ConfigDiagnostic`, `DiagnosticSeverity`)
- Create: `crates/pi-coding-agent/src/config/paths.rs` (`ConfigPaths`, `resolve`)
- Modify: `crates/pi-coding-agent/src/lib.rs:8` (add `pub mod config;`)

**Interfaces:**
- Produces: `config::ConfigPaths { global_dir: PathBuf, project_dir: PathBuf }` with methods `global_settings()`, `project_settings()`, `global_auth()` → `PathBuf`; `config::paths::resolve(cwd: &Path) -> ConfigPaths`; `config::ConfigDiagnostic` with `warn(msg, Option<PathBuf>)` / `error(msg, Option<PathBuf>)` constructors and fields `severity: DiagnosticSeverity`, `message: String`, `source: Option<PathBuf>`.

- [ ] **Step 1: Add dependencies**

In `crates/pi-coding-agent/Cargo.toml`, under `[dependencies]`, add:

```toml
dirs = "5"
toml = "0.8"
```

- [ ] **Step 2: Write `config/mod.rs` with diagnostics + module wiring**

Create `crates/pi-coding-agent/src/config/mod.rs`:

```rust
pub mod paths;

use std::path::PathBuf;

pub use paths::{ConfigPaths, resolve as resolve_paths};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDiagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub source: Option<PathBuf>,
}

impl ConfigDiagnostic {
    pub fn warn(message: impl Into<String>, source: Option<PathBuf>) -> Self {
        Self { severity: DiagnosticSeverity::Warn, message: message.into(), source }
    }

    pub fn error(message: impl Into<String>, source: Option<PathBuf>) -> Self {
        Self { severity: DiagnosticSeverity::Error, message: message.into(), source }
    }
}
```

- [ ] **Step 3: Write the failing test for `paths::resolve`**

Create `crates/pi-coding-agent/src/config/paths.rs` with only the test module first:

```rust
use std::path::{Path, PathBuf};

pub struct ConfigPaths {
    pub global_dir: PathBuf,
    pub project_dir: PathBuf,
}

impl ConfigPaths {
    pub fn global_settings(&self) -> PathBuf { self.global_dir.join("settings.toml") }
    pub fn project_settings(&self) -> PathBuf { self.project_dir.join("settings.toml") }
    pub fn global_auth(&self) -> PathBuf { self.global_dir.join("auth.toml") }
}

pub fn resolve(_cwd: &Path) -> ConfigPaths {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_dir_is_cwd_dot_pi_rust() {
        let p = resolve(Path::new("/tmp/work"));
        assert_eq!(p.project_dir, PathBuf::from("/tmp/work/.pi-rust"));
        assert_eq!(p.project_settings(), PathBuf::from("/tmp/work/.pi-rust/settings.toml"));
    }

    #[test]
    fn pi_rust_dir_env_overrides_global() {
        // SAFETY: single-threaded test; var removed at end.
        unsafe { std::env::set_var("PI_RUST_DIR", "/custom/cfg"); }
        let p = resolve(Path::new("/tmp/work"));
        assert_eq!(p.global_dir, PathBuf::from("/custom/cfg"));
        assert_eq!(p.global_auth(), PathBuf::from("/custom/cfg/auth.toml"));
        unsafe { std::env::remove_var("PI_RUST_DIR"); }
    }
}
```

Add `pub mod config;` to `crates/pi-coding-agent/src/lib.rs` (after the existing `pub mod` lines, keep alphabetical: between `pub mod args;` and `pub mod error;`).

- [ ] **Step 4: Run the test to verify it fails**

Run: `cargo test -p pi-coding-agent config::paths::tests -- --test-threads=1`
Expected: FAIL (panics with `not implemented` from `unimplemented!()`).

- [ ] **Step 5: Implement `resolve`**

Replace the `resolve` body in `paths.rs`:

```rust
pub fn resolve(cwd: &Path) -> ConfigPaths {
    let global_dir = match std::env::var_os("PI_RUST_DIR") {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".pi-rust"),
    };
    ConfigPaths { global_dir, project_dir: cwd.join(".pi-rust") }
}
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p pi-coding-agent config::paths::tests -- --test-threads=1`
Expected: PASS (2 tests).

- [ ] **Step 7: Commit**

```bash
git add crates/pi-coding-agent/Cargo.toml crates/pi-coding-agent/src/config/ crates/pi-coding-agent/src/lib.rs
git commit -m "feat(m7): add config module scaffold (paths + diagnostics)"
```

---

### Task 2: Settings schema + merge + resolve

**Files:**
- Create: `crates/pi-coding-agent/src/config/settings.rs`
- Modify: `crates/pi-coding-agent/src/config/mod.rs` (add `pub mod settings;`)

**Interfaces:**
- Consumes: nothing from prior tasks.
- Produces: `settings::PartialSettings` (`Deserialize`, all-`Option`, with `merge(self, over) -> PartialSettings` and `resolve(self) -> Settings`); `settings::Settings` with fields `default_provider: Option<String>`, `default_model: Option<String>`, `default_thinking_level: Option<String>`, `transport: String`, `steering_mode: String`, `follow_up_mode: String`, `session_dir: Option<String>`, `compaction: CompactionSettings`, `retry: RetrySettings`; `CompactionSettings { enabled: bool, reserve_tokens: u64, keep_recent_tokens: u64 }`; `RetrySettings { enabled: bool, max_retries: u32, base_delay_ms: u64 }`.

- [ ] **Step 1: Write the failing tests**

Create `crates/pi-coding-agent/src/config/settings.rs`:

```rust
use serde::Deserialize;

#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PartialCompaction {
    pub enabled: Option<bool>,
    pub reserve_tokens: Option<u64>,
    pub keep_recent_tokens: Option<u64>,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PartialRetry {
    pub enabled: Option<bool>,
    pub max_retries: Option<u32>,
    pub base_delay_ms: Option<u64>,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PartialSettings {
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    pub default_thinking_level: Option<String>,
    pub transport: Option<String>,
    pub steering_mode: Option<String>,
    pub follow_up_mode: Option<String>,
    pub session_dir: Option<String>,
    pub compaction: Option<PartialCompaction>,
    pub retry: Option<PartialRetry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompactionSettings {
    pub enabled: bool,
    pub reserve_tokens: u64,
    pub keep_recent_tokens: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetrySettings {
    pub enabled: bool,
    pub max_retries: u32,
    pub base_delay_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    pub default_thinking_level: Option<String>,
    pub transport: String,
    pub steering_mode: String,
    pub follow_up_mode: String,
    pub session_dir: Option<String>,
    pub compaction: CompactionSettings,
    pub retry: RetrySettings,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_applied_on_empty() {
        let s = PartialSettings::default().resolve();
        assert_eq!(s.transport, "auto");
        assert_eq!(s.steering_mode, "one-at-a-time");
        assert!(s.compaction.enabled);
        assert_eq!(s.compaction.reserve_tokens, 16384);
        assert_eq!(s.compaction.keep_recent_tokens, 20000);
        assert_eq!(s.retry.max_retries, 3);
        assert_eq!(s.retry.base_delay_ms, 2000);
        assert!(s.default_model.is_none());
    }

    #[test]
    fn project_overrides_global_scalars() {
        let global = PartialSettings { default_model: Some("a".into()), transport: Some("sse".into()), ..Default::default() };
        let project = PartialSettings { default_model: Some("b".into()), ..Default::default() };
        let s = global.merge(project).resolve();
        assert_eq!(s.default_model.as_deref(), Some("b")); // project wins
        assert_eq!(s.transport, "sse");                    // global survives where project is silent
    }

    #[test]
    fn nested_objects_merge_field_wise() {
        let global = PartialSettings {
            compaction: Some(PartialCompaction { reserve_tokens: Some(100), keep_recent_tokens: Some(200), ..Default::default() }),
            ..Default::default()
        };
        let project = PartialSettings {
            compaction: Some(PartialCompaction { reserve_tokens: Some(999), ..Default::default() }),
            ..Default::default()
        };
        let s = global.merge(project).resolve();
        assert_eq!(s.compaction.reserve_tokens, 999);     // project overrides
        assert_eq!(s.compaction.keep_recent_tokens, 200); // global field survives
        assert!(s.compaction.enabled);                    // default fills the gap
    }
}
```

Add `pub mod settings;` to `config/mod.rs`.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p pi-coding-agent config::settings::tests -- --test-threads=1`
Expected: FAIL (`no method named merge`/`resolve`).

- [ ] **Step 3: Implement `merge` and `resolve`**

Append to `settings.rs` (before the `#[cfg(test)]` module):

```rust
fn merge_compaction(base: Option<PartialCompaction>, over: Option<PartialCompaction>) -> Option<PartialCompaction> {
    match (base, over) {
        (None, x) | (x, None) => x,
        (Some(b), Some(o)) => Some(PartialCompaction {
            enabled: o.enabled.or(b.enabled),
            reserve_tokens: o.reserve_tokens.or(b.reserve_tokens),
            keep_recent_tokens: o.keep_recent_tokens.or(b.keep_recent_tokens),
        }),
    }
}

fn merge_retry(base: Option<PartialRetry>, over: Option<PartialRetry>) -> Option<PartialRetry> {
    match (base, over) {
        (None, x) | (x, None) => x,
        (Some(b), Some(o)) => Some(PartialRetry {
            enabled: o.enabled.or(b.enabled),
            max_retries: o.max_retries.or(b.max_retries),
            base_delay_ms: o.base_delay_ms.or(b.base_delay_ms),
        }),
    }
}

impl PartialSettings {
    pub fn merge(self, over: PartialSettings) -> PartialSettings {
        PartialSettings {
            default_provider: over.default_provider.or(self.default_provider),
            default_model: over.default_model.or(self.default_model),
            default_thinking_level: over.default_thinking_level.or(self.default_thinking_level),
            transport: over.transport.or(self.transport),
            steering_mode: over.steering_mode.or(self.steering_mode),
            follow_up_mode: over.follow_up_mode.or(self.follow_up_mode),
            session_dir: over.session_dir.or(self.session_dir),
            compaction: merge_compaction(self.compaction, over.compaction),
            retry: merge_retry(self.retry, over.retry),
        }
    }

    pub fn resolve(self) -> Settings {
        let c = self.compaction.unwrap_or_default();
        let r = self.retry.unwrap_or_default();
        Settings {
            default_provider: self.default_provider,
            default_model: self.default_model,
            default_thinking_level: self.default_thinking_level,
            transport: self.transport.unwrap_or_else(|| "auto".to_string()),
            steering_mode: self.steering_mode.unwrap_or_else(|| "one-at-a-time".to_string()),
            follow_up_mode: self.follow_up_mode.unwrap_or_else(|| "one-at-a-time".to_string()),
            session_dir: self.session_dir,
            compaction: CompactionSettings {
                enabled: c.enabled.unwrap_or(true),
                reserve_tokens: c.reserve_tokens.unwrap_or(16384),
                keep_recent_tokens: c.keep_recent_tokens.unwrap_or(20000),
            },
            retry: RetrySettings {
                enabled: r.enabled.unwrap_or(true),
                max_retries: r.max_retries.unwrap_or(3),
                base_delay_ms: r.base_delay_ms.unwrap_or(2000),
            },
        }
    }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p pi-coding-agent config::settings::tests -- --test-threads=1`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/pi-coding-agent/src/config/
git commit -m "feat(m7): add settings schema, deep merge, and resolve"
```

---

### Task 3: Settings loading from TOML files

**Files:**
- Modify: `crates/pi-coding-agent/src/config/settings.rs` (add loaders + tests)

**Interfaces:**
- Consumes: `ConfigDiagnostic` (Task 1), `PartialSettings`/`Settings` (Task 2), `ConfigPaths` (Task 1).
- Produces: `settings::load_partial(path: &Path, diags: &mut Vec<ConfigDiagnostic>) -> PartialSettings`; `settings::load_settings(paths: &ConfigPaths, diags: &mut Vec<ConfigDiagnostic>) -> Settings`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `settings.rs`:

```rust
#[test]
fn missing_file_yields_default_no_diags() {
    let mut diags = Vec::new();
    let p = std::path::Path::new("/nonexistent/dir/settings.toml");
    let parsed = load_partial(p, &mut diags);
    assert_eq!(parsed, PartialSettings::default());
    assert!(diags.is_empty());
}

#[test]
fn parses_toml_and_unknown_field_warns() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.toml");
    std::fs::write(&path, "default_model = \"x\"\nbogus_field = 1\n").unwrap();
    let mut diags = Vec::new();
    let parsed = load_partial(&path, &mut diags);
    // deny_unknown_fields makes the whole parse fail -> default + warn diagnostic
    assert_eq!(parsed, PartialSettings::default());
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, crate::config::DiagnosticSeverity::Warn);
}

#[test]
fn load_settings_project_overrides_global() {
    let global = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    std::fs::write(global.path().join("settings.toml"), "default_model = \"g\"\ntransport = \"sse\"\n").unwrap();
    std::fs::write(project.path().join("settings.toml"), "default_model = \"p\"\n").unwrap();
    let paths = crate::config::ConfigPaths {
        global_dir: global.path().to_path_buf(),
        project_dir: project.path().to_path_buf(),
    };
    let mut diags = Vec::new();
    let s = load_settings(&paths, &mut diags);
    assert_eq!(s.default_model.as_deref(), Some("p"));
    assert_eq!(s.transport, "sse");
    assert!(diags.is_empty());
}
```

Add `use std::path::Path;` at the top of `settings.rs` if not present, and `use crate::config::{ConfigDiagnostic, ConfigPaths};`.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p pi-coding-agent config::settings::tests -- --test-threads=1`
Expected: FAIL (`load_partial`/`load_settings` not found).

- [ ] **Step 3: Implement the loaders**

Append to `settings.rs` (before tests):

```rust
pub fn load_partial(path: &Path, diags: &mut Vec<ConfigDiagnostic>) -> PartialSettings {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return PartialSettings::default(),
        Err(err) => {
            diags.push(ConfigDiagnostic::warn(
                format!("failed to read settings: {err}"),
                Some(path.to_path_buf()),
            ));
            return PartialSettings::default();
        }
    };
    match toml::from_str::<PartialSettings>(&text) {
        Ok(parsed) => parsed,
        Err(err) => {
            diags.push(ConfigDiagnostic::warn(
                format!("failed to parse settings: {err}"),
                Some(path.to_path_buf()),
            ));
            PartialSettings::default()
        }
    }
}

pub fn load_settings(paths: &ConfigPaths, diags: &mut Vec<ConfigDiagnostic>) -> Settings {
    let global = load_partial(&paths.global_settings(), diags);
    let project = load_partial(&paths.project_settings(), diags);
    global.merge(project).resolve()
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p pi-coding-agent config::settings::tests -- --test-threads=1`
Expected: PASS (6 tests total in module).

- [ ] **Step 5: Commit**

```bash
git add crates/pi-coding-agent/src/config/settings.rs
git commit -m "feat(m7): load + layer settings from TOML files"
```

---

### Task 4: `pi-ai` env-key expansion + sentinels + root re-export

**Files:**
- Modify: `crates/pi-ai/src/util/env_keys.rs`
- Modify: `crates/pi-ai/src/lib.rs` (re-export `env_api_key`)

**Interfaces:**
- Produces: `pi_ai::env_api_key(provider: &str) -> Option<String>` (re-exported at crate root), now covering the additional providers and Bedrock/Vertex sentinels.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `crates/pi-ai/src/util/env_keys.rs`:

```rust
#[test]
fn returns_minimax_key() {
    unsafe { std::env::set_var("MINIMAX_API_KEY", "mm-test"); }
    assert_eq!(env_api_key("minimax"), Some("mm-test".into()));
    unsafe { std::env::remove_var("MINIMAX_API_KEY"); }
}

#[test]
fn returns_copilot_token() {
    unsafe { std::env::set_var("COPILOT_GITHUB_TOKEN", "ghp-test"); }
    assert_eq!(env_api_key("github-copilot"), Some("ghp-test".into()));
    unsafe { std::env::remove_var("COPILOT_GITHUB_TOKEN"); }
}

#[test]
fn bedrock_returns_sentinel_when_aws_profile_set() {
    unsafe { std::env::set_var("AWS_PROFILE", "default"); }
    assert_eq!(env_api_key("amazon-bedrock"), Some("<authenticated>".into()));
    unsafe { std::env::remove_var("AWS_PROFILE"); }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p pi-ai env_keys -- --test-threads=1`
Expected: FAIL (`minimax`/`github-copilot`/`amazon-bedrock` return `None`).

- [ ] **Step 3: Extend the provider map + sentinel logic**

In `env_keys.rs`, add the new arms to `provider_env_vars` (before the `_ => &[]` arm):

```rust
        "minimax" => &["MINIMAX_API_KEY"],
        "minimax-cn" => &["MINIMAX_CN_API_KEY"],
        "xiaomi" => &["XIAOMI_API_KEY"],
        "xiaomi-token-plan-cn" => &["XIAOMI_TOKEN_PLAN_CN_API_KEY"],
        "xiaomi-token-plan-ams" => &["XIAOMI_TOKEN_PLAN_AMS_API_KEY"],
        "xiaomi-token-plan-sgp" => &["XIAOMI_TOKEN_PLAN_SGP_API_KEY"],
        "github-copilot" => &["COPILOT_GITHUB_TOKEN"],
```

Then update `env_api_key` to handle self-authing providers via a sentinel. Replace the function body:

```rust
pub fn env_api_key(provider: &str) -> Option<String> {
    for var in provider_env_vars(provider) {
        if let Ok(val) = std::env::var(var) {
            if !val.is_empty() {
                return Some(val);
            }
        }
    }
    if self_auth_present(provider) {
        return Some("<authenticated>".to_string());
    }
    None
}

/// Providers that authenticate via an external credential chain rather than a
/// single API-key env var. Returns true when credentials appear to be present;
/// real signing/ADC is implemented in M8.
fn self_auth_present(provider: &str) -> bool {
    match provider {
        "amazon-bedrock" => ["AWS_PROFILE", "AWS_ACCESS_KEY_ID", "AWS_BEARER_TOKEN_BEDROCK"]
            .iter()
            .any(|v| std::env::var_os(v).is_some_and(|s| !s.is_empty())),
        "google-vertex" => {
            std::env::var_os("GOOGLE_APPLICATION_CREDENTIALS").is_some_and(|s| !s.is_empty())
        }
        _ => false,
    }
}
```

- [ ] **Step 4: Re-export `env_api_key` at the crate root**

In `crates/pi-ai/src/lib.rs`, after the existing `pub use` lines (around line 11), add:

```rust
pub use util::env_keys::env_api_key;
```

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test -p pi-ai env_keys -- --test-threads=1`
Expected: PASS (existing + 3 new tests).

- [ ] **Step 6: Commit**

```bash
git add crates/pi-ai/src/util/env_keys.rs crates/pi-ai/src/lib.rs
git commit -m "feat(m7): expand pi-ai env-key map + bedrock/vertex sentinels"
```

---

### Task 5: Auth value substitution (`$ENV`)

**Files:**
- Create: `crates/pi-coding-agent/src/config/auth.rs`
- Modify: `crates/pi-coding-agent/src/config/mod.rs` (add `pub mod auth;`)

**Interfaces:**
- Consumes: `ConfigDiagnostic` (Task 1).
- Produces: `auth::resolve_config_value(raw: &str, diags: &mut Vec<ConfigDiagnostic>) -> Option<String>`.

- [ ] **Step 1: Write the failing tests**

Create `crates/pi-coding-agent/src/config/auth.rs`:

```rust
use crate::config::ConfigDiagnostic;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_passthrough() {
        let mut d = Vec::new();
        assert_eq!(resolve_config_value("sk-literal", &mut d), Some("sk-literal".into()));
        assert!(d.is_empty());
    }

    #[test]
    fn dollar_var_and_braced_var() {
        unsafe { std::env::set_var("PI_TEST_KEY", "secret"); }
        let mut d = Vec::new();
        assert_eq!(resolve_config_value("$PI_TEST_KEY", &mut d), Some("secret".into()));
        assert_eq!(resolve_config_value("pre-${PI_TEST_KEY}-post", &mut d), Some("pre-secret-post".into()));
        unsafe { std::env::remove_var("PI_TEST_KEY"); }
    }

    #[test]
    fn escapes() {
        let mut d = Vec::new();
        assert_eq!(resolve_config_value("$$literal", &mut d), Some("$literal".into()));
        assert_eq!(resolve_config_value("a$!b", &mut d), Some("a!b".into()));
    }

    #[test]
    fn unset_var_returns_none_with_diag() {
        unsafe { std::env::remove_var("PI_TEST_MISSING"); }
        let mut d = Vec::new();
        assert_eq!(resolve_config_value("$PI_TEST_MISSING", &mut d), None);
        assert_eq!(d.len(), 1);
    }
}
```

Add `pub mod auth;` to `config/mod.rs`.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p pi-coding-agent config::auth::tests -- --test-threads=1`
Expected: FAIL (`resolve_config_value` not found).

- [ ] **Step 3: Implement `resolve_config_value`**

Add to `auth.rs` (above the test module):

```rust
/// Expand `$VAR` / `${VAR}` from the environment, with `$$` → `$` and `$!` → `!`.
/// Returns `None` (plus a diagnostic) if a referenced variable is unset.
pub fn resolve_config_value(raw: &str, diags: &mut Vec<ConfigDiagnostic>) -> Option<String> {
    let mut out = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '$' {
            out.push(c);
            continue;
        }
        match chars.peek().copied() {
            Some('$') => { chars.next(); out.push('$'); }
            Some('!') => { chars.next(); out.push('!'); }
            Some('{') => {
                chars.next(); // consume '{'
                let mut var = String::new();
                let mut closed = false;
                for ch in chars.by_ref() {
                    if ch == '}' { closed = true; break; }
                    var.push(ch);
                }
                if !closed {
                    out.push('$');
                    out.push('{');
                    out.push_str(&var);
                    continue;
                }
                match std::env::var(&var) {
                    Ok(value) => out.push_str(&value),
                    Err(_) => {
                        diags.push(ConfigDiagnostic::warn(
                            format!("env var {var} referenced by auth.toml is unset"),
                            None,
                        ));
                        return None;
                    }
                }
            }
            Some(first) if first.is_ascii_alphabetic() || first == '_' => {
                let mut var = String::new();
                while let Some(&next) = chars.peek() {
                    if next.is_ascii_alphanumeric() || next == '_' {
                        var.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                match std::env::var(&var) {
                    Ok(value) => out.push_str(&value),
                    Err(_) => {
                        diags.push(ConfigDiagnostic::warn(
                            format!("env var {var} referenced by auth.toml is unset"),
                            None,
                        ));
                        return None;
                    }
                }
            }
            _ => out.push('$'),
        }
    }
    Some(out)
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p pi-coding-agent config::auth::tests -- --test-threads=1`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/pi-coding-agent/src/config/
git commit -m "feat(m7): add auth value substitution ($ENV)"
```

---

### Task 6: Auth store load + entry access + permissions

**Files:**
- Modify: `crates/pi-coding-agent/src/config/auth.rs`

**Interfaces:**
- Consumes: `ConfigDiagnostic` (Task 1).
- Produces: `auth::AuthEntry` (`#[serde(tag = "type", rename_all = "snake_case")]` enum: `ApiKey { key: String }`, `Oauth(toml::Value)`); `auth::AuthStore` with `load(path: &Path, diags: &mut Vec<ConfigDiagnostic>) -> AuthStore`, `api_key_entry(&self, provider: &str) -> Option<&str>`, and `Default`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `auth.rs`:

```rust
#[test]
fn loads_api_key_entries_and_skips_oauth() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("auth.toml");
    std::fs::write(
        &path,
        "[anthropic]\ntype = \"api_key\"\nkey = \"sk-x\"\n\n[openai]\ntype = \"oauth\"\naccess_token = \"t\"\n",
    ).unwrap();
    let mut d = Vec::new();
    let store = AuthStore::load(&path, &mut d);
    assert_eq!(store.api_key_entry("anthropic"), Some("sk-x"));
    assert_eq!(store.api_key_entry("openai"), None); // oauth skipped in M7
    assert_eq!(store.api_key_entry("missing"), None);
}

#[test]
fn missing_auth_file_is_empty_no_diag() {
    let mut d = Vec::new();
    let store = AuthStore::load(std::path::Path::new("/no/such/auth.toml"), &mut d);
    assert_eq!(store.api_key_entry("anthropic"), None);
    assert!(d.is_empty());
}

#[cfg(unix)]
#[test]
fn loose_permissions_warn() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("auth.toml");
    std::fs::write(&path, "[anthropic]\ntype = \"api_key\"\nkey = \"sk-x\"\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    let mut d = Vec::new();
    let _ = AuthStore::load(&path, &mut d);
    assert!(d.iter().any(|x| x.message.contains("permissions")));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p pi-coding-agent config::auth::tests -- --test-threads=1`
Expected: FAIL (`AuthStore`/`AuthEntry` not found).

- [ ] **Step 3: Implement `AuthEntry`, `AuthStore`, permissions check**

Add to `auth.rs` (above the test module). Add `use std::collections::BTreeMap; use std::path::Path; use serde::Deserialize;` to the imports at the top:

```rust
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthEntry {
    ApiKey { key: String },
    Oauth(toml::Value),
}

#[derive(Debug, Default, Clone)]
pub struct AuthStore {
    entries: BTreeMap<String, AuthEntry>,
}

impl AuthStore {
    pub fn load(path: &Path, diags: &mut Vec<ConfigDiagnostic>) -> AuthStore {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return AuthStore::default(),
            Err(err) => {
                diags.push(ConfigDiagnostic::warn(
                    format!("failed to read auth: {err}"),
                    Some(path.to_path_buf()),
                ));
                return AuthStore::default();
            }
        };
        #[cfg(unix)]
        check_permissions(path, diags);
        match toml::from_str::<BTreeMap<String, AuthEntry>>(&text) {
            Ok(entries) => AuthStore { entries },
            Err(err) => {
                diags.push(ConfigDiagnostic::warn(
                    format!("failed to parse auth: {err}"),
                    Some(path.to_path_buf()),
                ));
                AuthStore::default()
            }
        }
    }

    /// Raw `api_key` value for a provider (before `$ENV` substitution). `oauth`
    /// entries return `None` in M7.
    pub fn api_key_entry(&self, provider: &str) -> Option<&str> {
        match self.entries.get(provider) {
            Some(AuthEntry::ApiKey { key }) => Some(key.as_str()),
            _ => None,
        }
    }
}

#[cfg(unix)]
fn check_permissions(path: &Path, diags: &mut Vec<ConfigDiagnostic>) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(path) {
        let mode = meta.permissions().mode();
        if mode & 0o077 != 0 {
            diags.push(ConfigDiagnostic::warn(
                format!("auth.toml has loose permissions {:o}; expected 0600", mode & 0o777),
                Some(path.to_path_buf()),
            ));
        }
    }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p pi-coding-agent config::auth::tests -- --test-threads=1`
Expected: PASS (substitution tests + 3 new).

- [ ] **Step 5: Commit**

```bash
git add crates/pi-coding-agent/src/config/auth.rs
git commit -m "feat(m7): add TOML auth store with api_key entries + perms check"
```

---

### Task 7: API-key resolution chain

**Files:**
- Modify: `crates/pi-coding-agent/src/config/auth.rs`

**Interfaces:**
- Consumes: `AuthStore` (Task 6), `resolve_config_value` (Task 5), `pi_ai::env_api_key` (Task 4).
- Produces: `auth::KeySource` (`Cli`, `AuthFile`, `Env`); `auth::ResolvedKey { value: String, source: KeySource }`; `auth::resolve_api_key(provider: &str, cli_key: Option<&str>, store: &AuthStore, diags: &mut Vec<ConfigDiagnostic>) -> Option<ResolvedKey>`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `auth.rs`:

```rust
fn store_with(provider: &str, key: &str) -> AuthStore {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("auth.toml");
    std::fs::write(&path, format!("[{provider}]\ntype = \"api_key\"\nkey = \"{key}\"\n")).unwrap();
    let mut d = Vec::new();
    AuthStore::load(&path, &mut d)
}

#[test]
fn cli_key_wins() {
    let store = store_with("anthropic", "from-file");
    unsafe { std::env::set_var("ANTHROPIC_API_KEY", "from-env"); }
    let mut d = Vec::new();
    let r = resolve_api_key("anthropic", Some("from-cli"), &store, &mut d).unwrap();
    assert_eq!(r.value, "from-cli");
    assert_eq!(r.source, KeySource::Cli);
    unsafe { std::env::remove_var("ANTHROPIC_API_KEY"); }
}

#[test]
fn auth_file_beats_env() {
    let store = store_with("anthropic", "from-file");
    unsafe { std::env::set_var("ANTHROPIC_API_KEY", "from-env"); }
    let mut d = Vec::new();
    let r = resolve_api_key("anthropic", None, &store, &mut d).unwrap();
    assert_eq!(r.value, "from-file");
    assert_eq!(r.source, KeySource::AuthFile);
    unsafe { std::env::remove_var("ANTHROPIC_API_KEY"); }
}

#[test]
fn falls_back_to_env_then_none() {
    let store = AuthStore::default();
    unsafe { std::env::set_var("ANTHROPIC_API_KEY", "from-env"); }
    let mut d = Vec::new();
    let r = resolve_api_key("anthropic", None, &store, &mut d).unwrap();
    assert_eq!(r.source, KeySource::Env);
    unsafe { std::env::remove_var("ANTHROPIC_API_KEY"); }
    assert!(resolve_api_key("anthropic", None, &store, &mut d).is_none());
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p pi-coding-agent config::auth::tests -- --test-threads=1`
Expected: FAIL (`resolve_api_key`/`KeySource` not found).

- [ ] **Step 3: Implement the chain**

Add to `auth.rs` (above the test module):

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeySource {
    Cli,
    AuthFile,
    Env,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedKey {
    pub value: String,
    pub source: KeySource,
}

pub fn resolve_api_key(
    provider: &str,
    cli_key: Option<&str>,
    store: &AuthStore,
    diags: &mut Vec<ConfigDiagnostic>,
) -> Option<ResolvedKey> {
    if let Some(key) = cli_key {
        if !key.is_empty() {
            return Some(ResolvedKey { value: key.to_string(), source: KeySource::Cli });
        }
    }
    if let Some(raw) = store.api_key_entry(provider) {
        if let Some(value) = resolve_config_value(raw, diags) {
            if !value.is_empty() {
                return Some(ResolvedKey { value, source: KeySource::AuthFile });
            }
        }
    }
    if let Some(value) = pi_ai::env_api_key(provider) {
        return Some(ResolvedKey { value, source: KeySource::Env });
    }
    None
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p pi-coding-agent config::auth::tests -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/pi-coding-agent/src/config/auth.rs
git commit -m "feat(m7): add API-key resolution chain (cli>auth>env)"
```

---

### Task 8: `load_config` aggregate + diagnostics drain

**Files:**
- Modify: `crates/pi-coding-agent/src/config/mod.rs`

**Interfaces:**
- Consumes: `resolve_paths` (Task 1), `settings::load_settings` (Task 3), `auth::AuthStore::load` (Task 6).
- Produces: `config::Config { settings: Settings, auth: AuthStore }`; `config::load_config(cwd: &Path) -> (Config, Vec<ConfigDiagnostic>)`; `config::drain_diagnostics(diags: &[ConfigDiagnostic]) -> String` (renders to a stderr-bound string; empty if none).

- [ ] **Step 1: Write the failing tests**

Add a `tests` module to `config/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_config_reads_settings_and_auth_from_pi_rust_dir() {
        let dir = tempfile::tempdir().unwrap();
        let global = dir.path().join("global");
        std::fs::create_dir_all(&global).unwrap();
        std::fs::write(global.join("settings.toml"), "default_model = \"m\"\n").unwrap();
        std::fs::write(global.join("auth.toml"), "[anthropic]\ntype=\"api_key\"\nkey=\"sk-x\"\n").unwrap();
        // SAFETY: single-threaded test.
        unsafe { std::env::set_var("PI_RUST_DIR", global.to_str().unwrap()); }
        let work = dir.path().join("work");
        std::fs::create_dir_all(&work).unwrap();
        let (config, diags) = load_config(&work);
        assert_eq!(config.settings.default_model.as_deref(), Some("m"));
        assert_eq!(config.auth.api_key_entry("anthropic"), Some("sk-x"));
        assert!(diags.is_empty());
        unsafe { std::env::remove_var("PI_RUST_DIR"); }
    }

    #[test]
    fn drain_renders_warnings() {
        let diags = vec![ConfigDiagnostic::warn("boom", None)];
        let text = drain_diagnostics(&diags);
        assert!(text.contains("boom"));
        assert!(drain_diagnostics(&[]).is_empty());
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p pi-coding-agent config::tests -- --test-threads=1`
Expected: FAIL (`Config`/`load_config`/`drain_diagnostics` not found).

- [ ] **Step 3: Implement the aggregate**

Add to `config/mod.rs` (after the diagnostic types; add `use std::path::Path;` and `use settings::Settings; use auth::AuthStore;`):

```rust
pub struct Config {
    pub settings: settings::Settings,
    pub auth: auth::AuthStore,
}

pub fn load_config(cwd: &Path) -> (Config, Vec<ConfigDiagnostic>) {
    let mut diags = Vec::new();
    let paths = paths::resolve(cwd);
    let settings = settings::load_settings(&paths, &mut diags);
    let auth = auth::AuthStore::load(&paths.global_auth(), &mut diags);
    (Config { settings, auth }, diags)
}

/// Render diagnostics for stderr. Empty string when there are none.
pub fn drain_diagnostics(diags: &[ConfigDiagnostic]) -> String {
    let mut out = String::new();
    for d in diags {
        let label = match d.severity {
            DiagnosticSeverity::Warn => "warning",
            DiagnosticSeverity::Error => "error",
        };
        match &d.source {
            Some(p) => out.push_str(&format!("config {label}: {} ({})\n", d.message, p.display())),
            None => out.push_str(&format!("config {label}: {}\n", d.message)),
        }
    }
    out
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p pi-coding-agent config::tests -- --test-threads=1`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/pi-coding-agent/src/config/mod.rs
git commit -m "feat(m7): add load_config aggregate + diagnostics drain"
```

---

### Task 9: Wire settings + auth into model/key resolution (headless path)

**Files:**
- Modify: `crates/pi-coding-agent/src/runtime.rs:59` (`select_model` signature + `default_model` fallback)
- Modify: `crates/pi-coding-agent/src/lib.rs` (load_config, drain, `select_model` call, key resolution)
- Modify: `crates/pi-coding-agent/src/interactive/app.rs:882` (compile fix: pass `None`)
- Create: `crates/pi-coding-agent/tests/config_wiring.rs` (integration test)

**Interfaces:**
- Consumes: `config::load_config` (Task 8), `config::drain_diagnostics` (Task 8), `auth::resolve_api_key` (Task 7), `Model.provider: String` (pi-ai).
- Produces: `select_model(args: &CliArgs, default_model: Option<&str>, model_override: Option<Model>) -> Result<Model, CliError>` (new signature).

> Scope note: this task wires the **headless print/json path** (`run_cli_with_options` in `lib.rs`). Interactive-mode key resolution reuses `auth::resolve_api_key` in a later increment; here `interactive/app.rs` only gets the one-argument compile fix for the new `select_model` signature.

- [ ] **Step 1: Write the failing integration test**

Create `crates/pi-coding-agent/tests/config_wiring.rs`:

```rust
use pi_coding_agent::config;

#[test]
fn select_model_uses_default_model_when_no_flag() {
    use pi_coding_agent::{CliArgs, select_model};
    let args = CliArgs::default(); // args.model is None
    // default_model resolves via lookup_model; use a known built-in id.
    let model = select_model(&args, Some("claude-sonnet-4-5"), None).expect("model");
    assert_eq!(model.id, "claude-sonnet-4-5");
}

#[test]
fn load_config_from_temp_pi_rust_dir() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("settings.toml"), "default_model = \"claude-sonnet-4-5\"\n").unwrap();
    // SAFETY: single-threaded integration test.
    unsafe { std::env::set_var("PI_RUST_DIR", dir.path().to_str().unwrap()); }
    let (cfg, diags) = config::load_config(std::path::Path::new("."));
    assert_eq!(cfg.settings.default_model.as_deref(), Some("claude-sonnet-4-5"));
    assert!(diags.is_empty());
    unsafe { std::env::remove_var("PI_RUST_DIR"); }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p pi-coding-agent --test config_wiring -- --test-threads=1`
Expected: FAIL (compile error: `select_model` takes 2 args, not 3).

- [ ] **Step 3: Change `select_model` signature + add `default_model` fallback**

In `runtime.rs`, replace `select_model`:

```rust
pub fn select_model(
    args: &CliArgs,
    default_model: Option<&str>,
    model_override: Option<Model>,
) -> Result<Model, CliError> {
    if let Some(model_id) = &args.model {
        return pi_ai::lookup_model(model_id)
            .ok_or_else(|| CliError::UnknownModel(model_id.clone()));
    }

    if let Some(model_id) = default_model {
        return pi_ai::lookup_model(model_id)
            .ok_or_else(|| CliError::UnknownModel(model_id.to_string()));
    }

    if let Some(model) = model_override {
        return Ok(model);
    }

    pi_ai::lookup_model(DEFAULT_MODEL_ID)
        .ok_or_else(|| CliError::UnknownModel(DEFAULT_MODEL_ID.to_string()))
}
```

- [ ] **Step 4: Wire `load_config` + resolution into `lib.rs` headless path**

In `crates/pi-coding-agent/src/lib.rs`, inside `run_cli_with_options`, after `let cwd = ...` (line ~74) and before `parse_args`, load config and drain diagnostics:

```rust
    let (config, config_diags) = config::load_config(&cwd);
    let diag_text = config::drain_diagnostics(&config_diags);
    if !diag_text.is_empty() {
        eprint!("{diag_text}");
    }
```

Update the `select_model` call (line ~103) to pass the settings default:

```rust
    let model = match select_model(&parsed, config.settings.default_model.as_deref(), options.model_override) {
        Ok(model) => model,
        Err(error) => return CliOutput::failure(error),
    };
```

Replace the api_key field in the `SessionPromptOptions` construction (line ~174) so the key resolves through the chain. Because the struct literal moves `model` (via its `model,` field), compute everything into locals **before** the struct literal. Right after the `select_model` block, add:

```rust
    let provider = model.provider.clone();
    let resolved_api_key = {
        let mut key_diags = Vec::new();
        let resolved = config::auth::resolve_api_key(
            &provider,
            parsed.api_key.as_deref(),
            &config.auth,
            &mut key_diags,
        );
        let key_text = config::drain_diagnostics(&key_diags);
        if !key_text.is_empty() {
            eprint!("{key_text}");
        }
        resolved.map(|r| r.value)
    };
```

Then in the `SessionPromptOptions { ... }` literal, change:

```rust
        api_key: parsed.api_key,
```

to:

```rust
        api_key: resolved_api_key,
```

This borrows nothing from `model` at the struct literal, so the `model,` field move is unaffected.

- [ ] **Step 5: Compile-fix the interactive caller**

In `crates/pi-coding-agent/src/interactive/app.rs:882`, change:

```rust
    let model = select_model(parsed, options.model_override)?;
```

to:

```rust
    let model = select_model(parsed, None, options.model_override)?;
```

- [ ] **Step 6: Run the integration test + workspace**

Run: `cargo test -p pi-coding-agent --test config_wiring -- --test-threads=1`
Expected: PASS (2 tests).

Run: `cargo test --workspace` and `cargo check --workspace`
Expected: PASS (no regressions).

- [ ] **Step 7: Commit**

```bash
git add crates/pi-coding-agent/src/runtime.rs crates/pi-coding-agent/src/lib.rs crates/pi-coding-agent/src/interactive/app.rs crates/pi-coding-agent/tests/config_wiring.rs
git commit -m "feat(m7): wire settings default_model + auth key resolution into headless path"
```

---

### Task 10: Final verification + fmt

**Files:** none (verification only)

- [ ] **Step 1: Format**

Run: `cargo fmt`
Then: `cargo fmt --check`
Expected: clean.

- [ ] **Step 2: Full suite**

Run from `pi-rust/`:

```bash
cargo test -p pi-ai
cargo test -p pi-coding-agent
cargo test --workspace
cargo check --workspace
```

Expected: all PASS.

- [ ] **Step 3: Commit any fmt changes**

```bash
git add -A
git commit -m "style(m7): cargo fmt" || echo "nothing to format"
```

---

## Notes for the implementer

- Env-mutating tests run with `--test-threads=1` (the process env is global). The commands above pin that; if you run the whole suite, prefer `cargo test -p pi-coding-agent -- --test-threads=1` for the config modules.
- `toml::Value` requires the `toml` dependency (added in Task 1).
- Do not implement OAuth, `!command`, Bedrock signing, or `--provider` — they are explicitly deferred (see spec §3).
- Interactive-path key/model wiring is deferred; only the compile-fix in Task 9 Step 5 touches `app.rs`.
