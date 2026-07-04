# pi-coding-agent Env Guard Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove unguarded process-wide environment mutation from `pi-coding-agent` tests so env-dependent tests serialize and restore caller state.

**Architecture:** Keep production env reads unchanged. Add a restoring `EnvGuard` to crate-local `#[cfg(test)]` support for unit tests, and add a matching integration-test `tests/support` helper because each integration test file compiles as a separate crate. Convert tests from hand-written `std::env::set_var/remove_var` cleanup to scoped guard calls, leaving product code and normal env reads alone.

**Tech Stack:** Rust 2024, standard-library `Mutex`, `OsString`, integration-test support modules, no new dependency.

---

### Task 1: Establish The RED Audit

**Files:**
- Inspect: `crates/pi-coding-agent/src/**/*.rs`
- Inspect: `crates/pi-coding-agent/tests/**/*.rs`

- [ ] **Step 1: Run the mutation audit before code changes**

Run:

```bash
rg -n "std::env::(set_var|remove_var)" crates/pi-coding-agent/tests crates/pi-coding-agent/src -g '*.rs'
```

Expected: FAIL-style evidence for this hardening task: direct `std::env::set_var/remove_var` calls appear in integration tests and crate-local `#[cfg(test)]` modules outside shared support.

- [ ] **Step 2: Classify allowed final hits**

The final audit should allow direct mutation only inside:

```text
crates/pi-coding-agent/src/lib.rs              # #[cfg(test)] test_support::EnvGuard implementation
crates/pi-coding-agent/tests/support/mod.rs    # integration-test EnvGuard implementation
```

Product env reads such as `std::env::var` and `std::env::var_os` remain allowed outside this audit.

### Task 2: Add Restoring EnvGuard Helpers

**Files:**
- Modify: `crates/pi-coding-agent/src/lib.rs`
- Add: `crates/pi-coding-agent/tests/support/mod.rs`

- [ ] **Step 1: Upgrade crate-local test support**

Replace the existing `#[cfg(test)] pub(crate) mod test_support` body in `src/lib.rs` with a support module that keeps `env_lock()` for existing non-mutating callers and adds `EnvGuard`:

```rust
#[cfg(test)]
pub(crate) mod test_support {
    use std::ffi::{OsStr, OsString};
    use std::sync::{Mutex, MutexGuard};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    pub(crate) fn env_lock() -> MutexGuard<'static, ()> {
        ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(crate) struct EnvGuard<'a> {
        _lock: MutexGuard<'a, ()>,
        saved: Vec<(&'static str, Option<OsString>)>,
    }

    impl EnvGuard<'static> {
        pub(crate) fn new(names: &[&'static str]) -> Self {
            let lock = env_lock();
            let saved = names
                .iter()
                .map(|name| (*name, std::env::var_os(name)))
                .collect();
            Self { _lock: lock, saved }
        }
    }

    impl EnvGuard<'_> {
        pub(crate) fn set<V: AsRef<OsStr>>(&self, name: &str, value: V) {
            unsafe {
                std::env::set_var(name, value);
            }
        }

        pub(crate) fn remove(&self, name: &str) {
            unsafe {
                std::env::remove_var(name);
            }
        }

        pub(crate) fn set_pi_rust_dir<V: AsRef<OsStr>>(&self, value: V) {
            self.set("PI_RUST_DIR", value);
        }
    }

    impl Drop for EnvGuard<'_> {
        fn drop(&mut self) {
            for (name, value) in self.saved.iter().rev() {
                unsafe {
                    match value {
                        Some(value) => std::env::set_var(name, value),
                        None => std::env::remove_var(name),
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 2: Add integration-test support**

Create `crates/pi-coding-agent/tests/support/mod.rs` with the same public helper for integration tests:

```rust
use std::ffi::{OsStr, OsString};
use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

pub struct EnvGuard<'a> {
    _lock: MutexGuard<'a, ()>,
    saved: Vec<(&'static str, Option<OsString>)>,
}

impl EnvGuard<'static> {
    pub fn new(names: &[&'static str]) -> Self {
        let lock = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let saved = names
            .iter()
            .map(|name| (*name, std::env::var_os(name)))
            .collect();
        Self { _lock: lock, saved }
    }
}

#[allow(dead_code)]
impl EnvGuard<'_> {
    pub fn set<V: AsRef<OsStr>>(&self, name: &str, value: V) {
        unsafe {
            std::env::set_var(name, value);
        }
    }

    pub fn remove(&self, name: &str) {
        unsafe {
            std::env::remove_var(name);
        }
    }

    pub fn set_pi_rust_dir<V: AsRef<OsStr>>(&self, value: V) {
        self.set("PI_RUST_DIR", value);
    }
}

impl Drop for EnvGuard<'_> {
    fn drop(&mut self) {
        for (name, value) in self.saved.iter().rev() {
            unsafe {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }
}
```

- [ ] **Step 3: Run helper format/check smoke**

Run:

```bash
/home/whai/.cargo/bin/cargo fmt --check
/home/whai/.cargo/bin/cargo test -p pi-coding-agent config::auth::tests::literal_passthrough --lib --quiet
```

Expected: format passes and existing unit test still compiles with upgraded support.

### Task 3: Convert Integration Tests

**Files:**
- Modify: `crates/pi-coding-agent/tests/agent_invocation.rs`
- Modify: `crates/pi-coding-agent/tests/agent_team_flow.rs`
- Modify: `crates/pi-coding-agent/tests/agent_profile_runtime.rs`
- Modify: `crates/pi-coding-agent/tests/delegation_execution.rs`
- Modify: `crates/pi-coding-agent/tests/config_wiring.rs`
- Modify: `crates/pi-coding-agent/tests/interactive_abort.rs`
- Modify: `crates/pi-coding-agent/tests/interactive_mode.rs`
- Modify: `crates/pi-coding-agent/tests/rpc_mode.rs`

- [ ] **Step 1: Replace per-file PI_RUST_DIR guards**

For files with local `struct EnvGuard`, add:

```rust
mod support;

use support::EnvGuard;
```

Remove `use std::ffi::OsString;`, remove local `EnvGuard` definitions, and keep call sites as:

```rust
let _env_guard = EnvGuard::set_pi_rust_dir(global);
```

If the shared helper only exposes `EnvGuard::new`, use this equivalent instead:

```rust
let _env_guard = EnvGuard::new(&["PI_RUST_DIR"]);
_env_guard.set_pi_rust_dir(global);
```

- [ ] **Step 2: Convert direct integration env mutation blocks**

Replace hand-written save/set/remove blocks with scoped guards. Examples:

```rust
let env = EnvGuard::new(&["PI_RUST_DIR"]);
env.set_pi_rust_dir(dir.path());
```

```rust
let env = EnvGuard::new(&["PI_RUST_DIR", "ANTHROPIC_API_KEY"]);
env.set_pi_rust_dir(dir.path());
env.set("ANTHROPIC_API_KEY", "from-env");
```

```rust
let env = EnvGuard::new(&[
    "PI_RUST_DIR",
    "ANTHROPIC_API_KEY",
    "CLAUDE_API_KEY",
    "ANTHROPIC_KEY",
]);
env.set_pi_rust_dir(dir.path());
env.remove("ANTHROPIC_API_KEY");
env.remove("CLAUDE_API_KEY");
env.remove("ANTHROPIC_KEY");
```

- [ ] **Step 3: Run integration RED-to-GREEN audit**

Run:

```bash
rg -n "std::env::(set_var|remove_var)" crates/pi-coding-agent/tests -g '*.rs'
```

Expected after conversion: only `crates/pi-coding-agent/tests/support/mod.rs` contains direct set/remove calls.

- [ ] **Step 4: Run focused integration tests**

Run:

```bash
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test config_wiring --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test agent_invocation --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test agent_team_flow --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test agent_profile_runtime --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test delegation_execution --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test interactive_abort --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test interactive_mode --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --test rpc_mode --quiet
```

Expected: all pass without warnings.

### Task 4: Convert Crate-Local Unit Tests

**Files:**
- Modify: `crates/pi-coding-agent/src/config/auth.rs`
- Modify: `crates/pi-coding-agent/src/config/mod.rs`
- Modify: `crates/pi-coding-agent/src/config/paths.rs`
- Modify: `crates/pi-coding-agent/src/session.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `crates/pi-coding-agent/src/interactive/app.rs`

- [ ] **Step 1: Replace env_lock mutation tests with EnvGuard**

For tests that mutate env, replace:

```rust
let _guard = crate::test_support::env_lock();
```

with:

```rust
let env = crate::test_support::EnvGuard::new(&["PI_RUST_DIR"]);
```

or with the full touched env list:

```rust
let env = crate::test_support::EnvGuard::new(&[
    "PI_RUST_DIR",
    "PI_AGENT_DIR",
    "PI_SESSION_DIR",
]);
```

Then replace raw mutation blocks with:

```rust
env.set_pi_rust_dir(dir.path());
env.remove("PI_SESSION_DIR");
```

- [ ] **Step 2: Preserve non-mutating env_lock callers**

Leave `crate::test_support::env_lock()` in tests that need serialization but do not mutate env directly, to avoid widening the diff.

- [ ] **Step 3: Run source mutation audit**

Run:

```bash
rg -n "std::env::(set_var|remove_var)" crates/pi-coding-agent/src -g '*.rs'
```

Expected after conversion: only `crates/pi-coding-agent/src/lib.rs` contains direct set/remove calls in the test support implementation.

- [ ] **Step 4: Run focused unit tests**

Run:

```bash
/home/whai/.cargo/bin/cargo test -p pi-coding-agent config::auth::tests --lib --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent config::tests --lib --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent config::paths::tests --lib --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent session::tests --lib --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent coding_session::tests --lib --quiet
/home/whai/.cargo/bin/cargo test -p pi-coding-agent interactive::app::tests --lib --quiet
```

Expected: all pass without warnings.

### Task 5: Docs And Final Verification

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-04-pi-coding-agent-env-guard-hardening-plan.md`

- [ ] **Step 1: Update TODO source documents**

Add the new plan under `## Source Documents`:

```markdown
- [pi-coding-agent env guard hardening plan](superpowers/plans/2026-07-04-pi-coding-agent-env-guard-hardening-plan.md)
```

- [ ] **Step 2: Update cross-cutting test-boundary note**

Update the deterministic/offline cross-cutting item to say the `pi-coding-agent` env-mutation follow-up is hardened, while timing-sensitive assertions remain a separate follow-up if still present.

- [ ] **Step 3: Mark this plan complete**

Mark each checkbox complete as implementation and verification finish.

- [ ] **Step 4: Run final verification**

Run:

```bash
/home/whai/.cargo/bin/cargo fmt --check
rg -n "std::env::(set_var|remove_var)" crates/pi-coding-agent/tests crates/pi-coding-agent/src -g '*.rs'
/home/whai/.cargo/bin/cargo test -p pi-coding-agent --quiet
/home/whai/.cargo/bin/cargo test --workspace --quiet
/home/whai/.cargo/bin/cargo check --workspace --quiet
git diff --check
```

Expected: formatting/check/tests pass. The env mutation audit reports only the two support helper files.
