# Phase 5: Boundary Enforcement and Stage 9 Closure - Pattern Map

**Mapped:** 2026-07-13
**Files analyzed:** 13 planned file groups
**Analogs found:** 13 / 13
**Dispatch note:** Produced with the generic-agent workaround for `gsd-pattern-mapper`; typed agent dispatch was unavailable in this session.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` | boundary test / source scanner | recursive file-I/O -> sanitize -> structural transform -> assertions | same file: `rust_files_under`, receiver-aware ledgers, suppression and alternate-facade guards | exact |
| `crates/pi-coding-agent/tests/api_boundary_guards.rs` | API boundary test / compile harness owner | request-response compile contract | same file: internal-contract source guards; `public_api.rs` positive contract | exact |
| `crates/pi-coding-agent/tests/public_api.rs` | public contract test | compile/import + exhaustive transform | same file: independent stable facade signature closure | exact |
| `crates/pi-coding-agent/tests/fixtures/**` (exact layout discretionary) | external consumer fixture crates | Cargo compile-pass / compile-fail | `api_boundary_guards.rs` and the independent `public_api.rs` inventory | role-match |
| `crates/pi-coding-agent/src/lib.rs` (only if fixture exposes a real leak) | stable facade / module visibility | compile-time export transform | existing curated `api` facade and hidden migration modules in the same file | exact |
| `.planning/phases/05-boundary-enforcement-and-stage-9-closure/05-STAGE-9-CLOSURE.md` (name may be selected by planner) | authoritative closure report | batch command evidence -> concise audit record | `04-VERIFICATION.md` | exact |
| `.planning/PROJECT.md` | milestone status document | batch status update | current completion checklists and validated-phase annotations | exact |
| `.planning/REQUIREMENTS.md` | requirements ledger | requirement-to-evidence mapping | completed Phase 1-4 requirement rows in the same file | exact |
| `.planning/ROADMAP.md` | roadmap status document | plan/status aggregation | completed Phase 4 section and plan checklist in the same file | exact |
| `.planning/STATE.md` | workflow state / handoff | current state + decision history | existing completed-phase decision and Stage 10 handoff entries | exact |
| `docs/superpowers/ARCHITECTURE.md` | current architecture authority | verified architecture narrative | existing product-operation runtime and adapter boundary sections | exact |
| `docs/superpowers/specs/2026-07-10-canonical-operation-runtime-convergence-design.md` | current design authority | design status -> closure link | existing Stage 9 completion criteria and Stage 10 boundary | exact |
| `docs/superpowers/plans/2026-07-10-canonical-operation-runtime-convergence-plan.md` | historical plan | superseded marker -> authoritative report link | existing top-level status/context block; preserve body as history | exact |

The fixture directory is intentionally classified as a group because `05-CONTEXT.md` locks the access-path matrix but delegates the exact Cargo/rustc harness layout. The closure report filename is likewise planner discretion; there must be exactly one authoritative report.

## Pattern Assignments

### `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` (boundary test, recursive source analysis)

**Analog:** Existing helpers and executable ledgers in the same file.

**Recursive deterministic discovery pattern** (`product_runtime_boundary_guards.rs:939-958`):

```rust
fn rust_files_under(root: &Path) -> Vec<PathBuf> {
    let Ok(metadata) = fs::metadata(root) else {
        return Vec::new();
    };
    if metadata.is_file() {
        return (root.extension().and_then(|extension| extension.to_str()) == Some("rs"))
            .then(|| root.to_path_buf())
            .into_iter()
            .collect();
    }
    let mut files = fs::read_dir(root)
        .expect("read Rust source directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("read Rust source entries")
        .into_iter()
        .flat_map(|entry| rust_files_under(&entry.path()))
        .collect::<Vec<_>>();
    files.sort();
    files
}
```

Copy the sorted recursion and path normalization, but harden the inventory wrapper around it: registered paths must exist, produce non-empty readable sets, normalize to one repository-relative path, and have exactly one adapter owner. The current helper's `metadata` miss returns an empty set; Phase 5's inventory must turn that condition into a path-specific failure.

**Owner-method structural extraction pattern** (`product_runtime_boundary_guards.rs:967-1021`):

```rust
let source = fs::read_to_string(path).expect("read CodingAgentSession source");
let sanitized = sanitize_rust_source(&source);
let relative = relative_path(repo_root, path);
let lines = sanitized.lines().collect::<Vec<_>>();

// ... enter only `impl CodingAgentSession`, track brace depth ...
if depth == 1 {
    if trimmed.starts_with("#[") {
        attributes.push(trimmed.to_owned());
    } else if let Some((visibility, name)) = parse_visible_method(trimmed) {
        let end_index = visible_method_end(&lines, index);
        methods.push(SessionMethod {
            name,
            visibility,
            test_only: attributes.iter().any(|attribute| attribute == "#[cfg(test)]"),
            attributes: attributes.clone(),
            body: lines[index..=end_index].join("\n"),
            file: relative.clone(),
            line: index + 1,
            end_line: end_index + 1,
        });
    }
}
```

Extend this receiver/structure-aware approach for adapter calls. Do not regress to method-name substring authority. Run `sanitize_rust_source` before recognition, retain `line_is_cfg_test_gated`, and report repository-relative file/line diagnostics.

**Fail-closed diagnostic pattern** (`provider_registry_boundary_guards.rs:23-36`, a close source-guard analog):

```rust
let scan = SourceScan::new();
let mut violations = Vec::new();
for root in scan.roots() {
    collect_direct_registry_mutations(scan.repo_root(), &root, &mut violations);
}
assert!(
    violations.is_empty(),
    "direct pi_ai::registry mutation must stay behind ProviderGuard:\n{}",
    violations.join("\n")
);
```

Apply one collected-violations assertion per architectural rule: inventory ownership, prohibited canonical receiver calls, exception count drift, production deprecation suppression, and alternate facade/synonym creation. The centralized exception table must contain scoped receiver/path/reason/count data; do not add inline suppression directives.

**Fixture pattern:** Add unit-style scanner fixtures in this integration target for multiline, chained, parenthesized, commented, raw-string, doc-comment, char/string, and `#[cfg(test)]` cases. Each negative fixture must contain one real canonical violation and assert its category/path; positive fixtures must demonstrate legitimate same-name receivers.

### `crates/pi-coding-agent/tests/api_boundary_guards.rs` and external fixtures (API contract tests, compiler request-response)

**Analogs:** Existing negative internal-contract guards in `api_boundary_guards.rs`, independent positive closure in `public_api.rs`, and Rust visibility in `coding_session/public_operation.rs`.

**Canonical public/private contrast** (`coding_session/public_operation.rs:41-104`):

```rust
#[derive(Debug)]
pub enum CodingAgentOperation {
    Prompt(PromptTurnOptions),
    Compact(PromptTurnOptions),
    // ... explicit stable variants ...
    ExportCurrent,
    ExportCurrentHtml(PathBuf),
}

#[derive(Debug)]
pub enum CodingAgentOperationOutcome {
    Prompt(PromptTurnOutcome),
    Compact(PromptTurnOutcome),
    // ... explicit typed outcomes ...
    Export(CodingAgentSessionExport),
    ExportHtml(PathBuf),
}

impl CodingAgentOperation {
    pub(crate) fn into_internal(self, plugin_load: PluginLoadOptions) -> Operation {
        // internal conversion remains crate-private
    }
}
```

External pass fixtures should import stable contracts only through `pi_coding_agent::api`, explicitly construct/reference all 15 operation variants and their outcomes/supporting types, and avoid deriving the expected inventory from production exports. External fail fixtures should attempt the locked matrix for internal `Operation`/metadata, runtime services, plugin load options/registries, and `Flow` contracts through `api`, crate root, and public-looking/doc-hidden module paths.

Use compiler success/failure as the primary authority. The harness should assert a broad failure category (privacy, unresolved import, inaccessible item, or type mismatch as appropriate), not exact rustc wording. Run fixtures offline and isolate their target directory so nested Cargo invocations do not contend with the parent test build. If the compiler demonstrates a real leak, narrow `src/lib.rs`; do not add production test hooks merely to simplify the harness.

### `crates/pi-coding-agent/tests/public_api.rs` (positive contract test, explicit inventory)

**Analog:** Existing `stable_api_signature_closure_is_importable` and facade behavior tests in the same file.

Preserve its established downstream-consumer import style and explicit operation list. The inventory is evidence only when it is manually independent from `CodingAgentOperation` metadata and `api` re-export generation. Keep behavior assertions alongside compile/import closure; do not replace them with compile-only checks.

### `crates/pi-coding-agent/src/lib.rs` (stable facade, compile-time transform)

**Analog:** Existing `pub mod api` curated barrel and migration-hidden root modules.

Only edit this file if an external negative fixture proves that an internal category remains reachable. The pattern is curated re-export through `api`, private modules by default, and narrowly documented `#[doc(hidden)]` compatibility only where still intentional. Never export internal `Operation`, dispatch metadata, services, plugin load options/registries, or product Flow nodes to satisfy a fixture.

### Stage 9 closure report (evidence report, batch transform)

**Analog:** `.planning/phases/04-test-convergence-and-compatibility-deletion/04-VERIFICATION.md:1-104`.

**Frontmatter and outcome pattern:**

```yaml
---
phase: 04-test-convergence-and-compatibility-deletion
verified: 2026-07-12T18:14:28Z
status: passed
score: 5/5 must-haves verified
behavior_unverified: 0
overrides_applied: 0
---
```

**Structured command evidence pattern** (`04-VERIFICATION.md`, Behavioral Spot-Checks):

```markdown
| Behavior | Command | Result | Status |
|---|---|---|---|
| Complete deletion/retained/private-exception ledger | `cargo test ... --exact` | 1 passed, 0 failed | PASS |
| Stable API boundary | `cargo test ... api_boundary_guards` | 5 passed, 0 failed | PASS |
| Formatting | `cargo fmt --check` | exit 0 | PASS |
| Full workspace behavior | `cargo test --workspace` | all tests passed | PASS |
| Full workspace compile | `cargo check --workspace` | exit 0 | PASS |
| Diff hygiene | `git diff --check` | exit 0 | PASS |
```

Use the same concise evidence style, but make this report the single Stage 9 authority. Include exact command, UTC timestamp, status, meaningful count/conclusion, source-audit scope, HEAD/commit identity, and dirty-worktree state. Map `GUARD-01..04` and `CLOSE-01..04`, record the 16 deleted-method conclusion and retained owner/API conclusion, and bound the Stage 10 handoff to current untyped `ProductEvent` families and compatibility subscriptions with source locations. Do not paste raw Cargo logs.

### Planning and current-authority documents (status/config documentation, batch update)

**Analogs:** Existing completed Phase 1-4 checklist/status rows in `.planning/PROJECT.md`, `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, and `.planning/STATE.md`; current Stage 9 narrative in the architecture/design docs.

Apply one consistent closure fact across all files only after verification passes:

- mark Phase 5 and `GUARD-01..04`/`CLOSE-01..04` complete;
- link the authoritative closure report rather than duplicate evidence;
- state that `CodingAgentSession::run(CodingAgentOperation)` plus `pi_coding_agent::api` is the closed Stage 9 operation boundary;
- identify typed `ProductEvent` payload convergence and compatibility subscription deletion as Stage 10, without implementing it;
- preserve dependency direction, durable transaction/replay facts, and adapter behavior guarantees in current architecture text.

### Historical plan supersession marker (documentation metadata)

**Analog:** The historical plan's existing introductory status/context area, not its task body.

Add a prominent top-of-file superseded marker containing the closure-report link and a short authority statement. Preserve the original plan body as historical design/execution input; do not rewrite old unchecked/checked tasks to simulate final truth.

## Shared Patterns

### Deterministic Source Guards

**Source:** `product_runtime_boundary_guards.rs:939-1021`, `:1062-1110`, `:1175`, `:1217`.

Apply sorted recursive discovery, repository-relative forward-slash paths, Rust-source sanitization, cfg(test) exclusion, receiver/owner structure, and collected path-specific diagnostics. Missing metadata, unreadable roots, empty inventories, duplicate ownership, and unknown receivers are violations rather than silent skips.

### Compiler-First Visibility

**Source:** `coding_session/public_operation.rs:41-107`, `tests/public_api.rs`, `tests/api_boundary_guards.rs`.

Positive access is explicit through `pi_coding_agent::api`; negative access is proven from a separate dependent crate. Source scans supplement the compiler for repository ownership/synonym/suppression rules only.

### Error Handling and Validation

Test setup failures use state-specific `expect(...)`; architectural violations accumulate and fail once with path/line/category context. Compile-fail diagnostics are classified semantically, avoiding brittle complete rustc-message snapshots.

### Scope and Exception Control

The existing 16 deleted-method ledger remains independent and zero-count. The private `load_plugins(PluginLoadOptions)` exception remains owner-test-only with exactly four justified calls. Any count increase is a review failure until the centralized table is deliberately amended with a reason.

### Verification Order

Run focused scanner/API targets first, then `cargo fmt --check`, `cargo test -p pi-coding-agent`, `cargo check -p pi-coding-agent`, source audits, `cargo test --workspace`, `cargo check --workspace`, and `git diff --check`. Record evidence after commands finish, including commit/worktree identity.

## No Analog Found

None. Every planned role has a repository-local analog. The only discretionary choice is the exact external fixture directory/harness organization; its contract and test ownership are fully covered by the existing API boundary patterns.

## Metadata

**Analog search scope:** `crates/pi-coding-agent/tests`, `crates/pi-coding-agent/src/lib.rs`, `crates/pi-coding-agent/src/coding_session`, `.planning/phases/04-*`, `.planning/*.md`, and current Stage 9 architecture/design/plan documents.
**Strong analogs retained:** 5 (`product_runtime_boundary_guards.rs`, `api_boundary_guards.rs`, `public_api.rs`, `public_operation.rs`, `04-VERIFICATION.md`).
**Pattern extraction date:** 2026-07-13

