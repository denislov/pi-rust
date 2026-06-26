# Settings Persistence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make TUI settings menu changes persist to disk in pi-rust, matching TypeScript pi's behavior.

**Architecture:** Each setting change from the TUI settings menu updates an in-memory `Settings` struct and writes only the changed fields back to the global `settings.toml` file. A `settings_delta: PartialSettings` accumulator tracks which fields have been modified; on save, the delta is merged into the existing file content using `toml::Value` table merging, so unmodified fields and manual edits are preserved.

**Tech Stack:** Rust, `serde` (Serialize), `toml 0.8`

## Global Constraints

- All `Partial*` setting structs must serialize with `#[serde(skip_serializing_if = "Option::is_none")]` so unmodified optional fields are omitted from TOML output
- Save target is always **global** settings (`~/.pi-rust/settings.toml`), matching TypeScript pi's behavior where the main settings menu writes to global scope
- Must preserve existing settings file contents (comments may be lost since `toml::Value` doesn't preserve them — acceptable for now, matches TS JSON approach which also drops comments)
- Test must use temporary directories, not real user config
- Follow existing patterns: `AuthStore::save()` is the reference for file writing

---
### Task 1: Add `Serialize` to all PartialSettings types and verify round-trip

**Files:**
- Modify: `crates/pi-coding-agent/src/config/settings.rs`

**Interfaces:**
- Consumes: existing `PartialCompaction`, `PartialRetry`, `PartialWarnings`, `PartialTerminal`, `PartialSettings` (currently `Deserialize` only)
- Produces: all 5 structs gain `Serialize + serde::ser::Serialize`; Option fields annotated with `#[serde(skip_serializing_if = "Option::is_none")]` and `#[serde(default)]`

- [ ] **Step 1: Write the failing round-trip test**

Add at the bottom of the `mod tests` block in `settings.rs`:

```rust
#[test]
fn partial_settings_serialize_round_trip() {
    // A PartialSettings with some fields set should round-trip through
    // serialize → deserialize without losing data.
    let original = PartialSettings {
        theme: Some("light".into()),
        transport: Some("sse".into()),
        compaction: Some(PartialCompaction {
            enabled: Some(false),
            reserve_tokens: Some(8192),
            ..Default::default()
        }),
        terminal: Some(PartialTerminal {
            show_images: Some(false),
            image_width_cells: Some(80),
            ..Default::default()
        }),
        ..Default::default()
    };
    let toml_str = toml::to_string_pretty(&original)
        .expect("serialize should succeed");
    let parsed: PartialSettings = toml::from_str(&toml_str)
        .expect("deserialize should succeed");
    assert_eq!(original, parsed);
}
```

- [ ] **Step 2: Run test and verify it fails**

Run: `cargo test -p pi-coding-agent settings::tests::partial_settings_serialize_round_trip -- --nocapture`
Expected: Compile error — `Serialize` not implemented for `PartialSettings`

- [ ] **Step 3: Add `Serialize` + serde attributes**

Change the import line and all 5 `#[derive(...)]` annotations:

Line 2: `use serde::{Deserialize, Serialize};`

```rust
#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PartialCompaction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reserve_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_recent_tokens: Option<u64>,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PartialRetry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_delay_ms: Option<u64>,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PartialWarnings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anthropic_extra_usage: Option<bool>,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PartialTerminal {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_images: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_progress: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clear_on_shrink: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_resize_images: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_images: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_width_cells: Option<u32>,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PartialSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_thinking_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steering_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub follow_up_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub themes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_context_files: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hide_thinking_block: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collapse_changelog: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quiet_startup: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_skill_commands: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub double_escape_action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tree_filter_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_command_prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npm_command: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_proxy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_idle_timeout_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub websocket_connect_timeout_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled_models: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<PartialWarnings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal: Option<PartialTerminal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compaction: Option<PartialCompaction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<PartialRetry>,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p pi-coding-agent settings::tests::partial_settings_serialize_round_trip -- --nocached`
Expected: PASS

- [ ] **Step 5: Verify existing tests still pass**

Run: `cargo test -p pi-coding-agent config::settings::tests -- --nocapture`
Expected: ALL PASS

- [ ] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/config/settings.rs
git commit -m "feat(settings): add Serialize to PartialSettings types"
```

---
### Task 2: Add `merge_and_save_settings` function

**Files:**
- Modify: `crates/pi-coding-agent/src/config/settings.rs`

**Interfaces:**
- Consumes: `&ConfigPaths` and `&PartialSettings` (delta — only non-None fields are changed)
- Produces: `pub fn merge_and_save_settings(paths: &ConfigPaths, scope: SettingsScope, delta: &PartialSettings, diags: &mut Vec<ConfigDiagnostic>)` — writes merged settings to the appropriate file; returns nothing on success, pushes diagnostics on failure
- Also consumes a new `SettingsScope` enum in `crate::config`

- [ ] **Step 1: Add `SettingsScope` enum to `config/mod.rs`**

Add to `crate::config` (e.g., near `DiagnosticSeverity` in `config/mod.rs`):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsScope {
    Global,
    Project,
}
```

Re-export it from `pub use` block:

```rust
pub use settings::{..., SettingsScope};
```

Wait — don't re-export from settings. Instead add it to `mod.rs`'s `pub use` block directly:

```rust
// in mod.rs
pub use paths::{ConfigPaths, resolve as resolve_paths};
pub use settings::Settings;
pub use self::SettingsScope;  // defined in mod.rs
```

Actually, simpler: define it in `settings.rs` and export via `pub use settings::SettingsScope` in `mod.rs`.

- [ ] **Step 2: Write the failing test for `merge_and_save_settings`**

Add to `mod tests` in `settings.rs`:

```rust
#[test]
fn merge_and_save_settings_writes_delta_and_preserves_existing() {
    use crate::config::ConfigPaths;
    let dir = tempfile::tempdir().unwrap();
    let global = dir.path().join("global");
    let project = dir.path().join("project");
    std::fs::create_dir_all(&global).unwrap();
    std::fs::create_dir_all(&project).unwrap();

    // Write an existing settings file with a field we won't touch
    std::fs::write(
        global.join("settings.toml"),
        "default_model = \"claude-3\"\ntransport = \"sse\"\n",
    )
    .unwrap();

    let paths = ConfigPaths {
        global_dir: global.clone(),
        project_dir: project.clone(),
    };
    let delta = PartialSettings {
        theme: Some("light".into()),
        ..Default::default()
    };

    let mut diags = Vec::new();
    crate::config::settings::merge_and_save_settings(
        &paths,
        crate::config::SettingsScope::Global,
        &delta,
        &mut diags,
    );

    assert!(diags.is_empty(), "diags: {diags:?}");

    // Read back and verify merge
    let saved = std::fs::read_to_string(global.join("settings.toml")).unwrap();
    let parsed: PartialSettings = toml::from_str(&saved).unwrap();
    assert_eq!(parsed.default_model.as_deref(), Some("claude-3"), "existing field preserved");
    assert_eq!(parsed.transport.as_deref(), Some("sse"), "existing field preserved");
    assert_eq!(parsed.theme.as_deref(), Some("light"), "delta field written");
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p pi-coding-agent settings::tests::merge_and_save_settings_writes_delta_and_preserves_existing -- --nocapture`
Expected: Compile error — `merge_and_save_settings` not found

- [ ] **Step 4: Write `merge_and_save_settings` and helper `merge_toml_tables`**

Add after the `load_settings` function (before `#[cfg(test)]`):

```rust
/// Recursively merge `over` table into `base` table. Over writes base.
fn merge_toml_tables(base: &mut toml::value::Table, over: &toml::value::Table) {
    for (key, value) in over {
        match (base.get_mut(key), value) {
            (Some(toml::Value::Table(base_table)), toml::Value::Table(over_table)) => {
                merge_toml_tables(base_table, over_table);
            }
            _ => {
                base.insert(key.clone(), value.clone());
            }
        }
    }
}

/// Merge a PartialSettings delta into the settings file for the given scope
/// and write it back to disk. Non-None fields in `delta` overwrite matching
/// keys in the file; fields that are `None` in `delta` are left untouched.
/// If the file doesn't exist yet, it is created with just the delta content.
pub fn merge_and_save_settings(
    paths: &ConfigPaths,
    scope: SettingsScope,
    delta: &PartialSettings,
    diags: &mut Vec<ConfigDiagnostic>,
) {
    let path = match scope {
        SettingsScope::Global => paths.global_settings(),
        SettingsScope::Project => paths.project_settings(),
    };

    // Serialize delta to TOML string, then parse as Value::Table.
    // Because PartialSettings uses skip_serializing_if = Option::is_none,
    // only the fields that are Some(...) appear in the output.
    let delta_str = match toml::to_string(delta) {
        Ok(s) => s,
        Err(err) => {
            diags.push(ConfigDiagnostic::warn(
                format!("failed to serialize settings delta: {err}"),
                Some(path),
            ));
            return;
        }
    };
    let delta_value: toml::Value = match toml::from_str(&delta_str) {
        Ok(v) => v,
        Err(err) => {
            diags.push(ConfigDiagnostic::warn(
                format!("failed to parse serialized delta: {err}"),
                Some(path),
            ));
            return;
        }
    };
    let Some(delta_table) = delta_value.as_table() else {
        diags.push(ConfigDiagnostic::warn(
            "settings delta produced a non-table value".into(),
            Some(path),
        ));
        return;
    };

    // Read existing file content, or start with an empty table
    let mut current_table = match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str::<toml::Value>(&text)
            .ok()
            .and_then(|v| match v {
                toml::Value::Table(t) => Some(t),
                _ => None,
            })
            .unwrap_or_default(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            toml::value::Table::new()
        }
        Err(err) => {
            diags.push(ConfigDiagnostic::warn(
                format!("failed to read settings file: {err}"),
                Some(path),
            ));
            return;
        }
    };

    // Merge delta into current
    merge_toml_tables(&mut current_table, delta_table);

    // Serialize merged table and write
    let merged_value = toml::Value::Table(current_table);
    let merged_str = match toml::to_string_pretty(&merged_value) {
        Ok(s) => s,
        Err(err) => {
            diags.push(ConfigDiagnostic::warn(
                format!("failed to serialize merged settings: {err}"),
                Some(path),
            ));
            return;
        }
    };

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            if let Err(err) = std::fs::create_dir_all(parent) {
                diags.push(ConfigDiagnostic::warn(
                    format!("failed to create settings dir: {err}"),
                    Some(path),
                ));
                return;
            }
        }
    }

    if let Err(err) = std::fs::write(&path, merged_str) {
        diags.push(ConfigDiagnostic::warn(
            format!("failed to write settings file: {err}"),
            Some(path),
        ));
    }
}
```

Also add the `SettingsScope` re-export at the top of `settings.rs`:

Put the enum definition at the top of `settings.rs`, after imports but before structs:

```rust
/// Which settings file to target when saving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsScope {
    Global,
    Project,
}
```

And update the `pub use` in `config/mod.rs` to export both `Settings` and `SettingsScope`:

```rust
pub use settings::{PartialSettings, Settings, SettingsScope};
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p pi-coding-agent settings::tests::merge_and_save_settings_writes_delta_and_preserves_existing -- --nocapture`
Expected: PASS

- [ ] **Step 6: Add edge-case tests for missing file and error handling**

Add more tests:

```rust
#[test]
fn merge_and_save_settings_creates_file_when_missing() {
    let dir = tempfile::tempdir().unwrap();
    let global = dir.path().join("global");
    std::fs::create_dir_all(&global).unwrap();
    let paths = ConfigPaths {
        global_dir: global.clone(),
        project_dir: dir.path().join("project"),
    };
    let delta = PartialSettings {
        theme: Some("dark".into()),
        ..Default::default()
    };
    let mut diags = Vec::new();
    merge_and_save_settings(&paths, SettingsScope::Global, &delta, &mut diags);
    assert!(diags.is_empty());
    let saved = std::fs::read_to_string(global.join("settings.toml")).unwrap();
    assert!(saved.contains("dark"));
}

#[test]
fn merge_and_save_settings_handles_nested_delta_merge() {
    let dir = tempfile::tempdir().unwrap();
    let global = dir.path().join("global");
    std::fs::create_dir_all(&global).unwrap();
    std::fs::write(
        global.join("settings.toml"),
        "[compaction]\nenabled = true\nreserve_tokens = 16384\n",
    )
    .unwrap();
    let paths = ConfigPaths {
        global_dir: global.clone(),
        project_dir: dir.path().join("project"),
    };
    // Only change compaction.enabled
    let delta = PartialSettings {
        compaction: Some(PartialCompaction {
            enabled: Some(false),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut diags = Vec::new();
    merge_and_save_settings(&paths, SettingsScope::Global, &delta, &mut diags);
    assert!(diags.is_empty());
    let saved = std::fs::read_to_string(global.join("settings.toml")).unwrap();
    let parsed: PartialSettings = toml::from_str(&saved).unwrap();
    let c = parsed.compaction.unwrap();
    assert!(!c.enabled.unwrap(), "delta overrides");
    assert_eq!(c.reserve_tokens, Some(16384), "existing field preserved");
}
```

- [ ] **Step 7: Run all config tests**

Run: `cargo test -p pi-coding-agent config:: -- --nocapture`
Expected: ALL PASS

- [ ] **Step 8: Commit**

```bash
git add crates/pi-coding-agent/src/config/settings.rs crates/pi-coding-agent/src/config/mod.rs
git commit -m "feat(settings): add merge_and_save_settings with TOML table merge"
```

---
### Task 3: Wire persistence into InteractiveRoot

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/root.rs`

**Interfaces:**
- Consumes: `config::ConfigPaths` (via `config::resolve_paths(&self.cwd)`)
- Consumes: `config::merge_and_save_settings`, `config::SettingsScope`
- Produces: settings changes from the TUI settings menu are persisted to `~/.pi-rust/settings.toml`

- [ ] **Step 1: Write the failing integration test**

Add a test in `crates/pi-coding-agent/src/interactive/app.rs` (in the existing `mod tests` block):

```rust
#[test]
fn settings_menu_cycles_theme_and_persists_to_file() {
    use crate::config::SettingsScope;
    let dir = tempfile::tempdir().unwrap();
    let global = dir.path().join("global");
    std::fs::create_dir_all(&global).unwrap();
    // Create a minimal existing settings file
    std::fs::write(
        global.join("settings.toml"),
        "default_model = \"claude-3\"\n",
    )
    .unwrap();

    // Safety: set PI_RUST_DIR to our temp dir
    unsafe { std::env::set_var("PI_RUST_DIR", global.to_str().unwrap()); }

    let mut root = InteractiveRoot::new(
        dir.path().join("work"),
        "faux-model".to_string(),
        "no-session".to_string(),
    );

    root.handle_slash_command(ParsedSlashCommand {
        name: "settings".to_string(),
        args: String::new(),
        original: "/settings".to_string(),
    });
    // Cycle theme (Enter on first item)
    root.handle_input(&key_event("\r"));

    // Verify update is reported
    let updated = root.take_settings_update()
        .expect("should emit settings update");
    assert_eq!(updated.theme.as_deref(), Some("light"));

    // Verify file was written
    let saved = std::fs::read_to_string(global.join("settings.toml")).unwrap();
    let parsed: crate::config::settings::PartialSettings = toml::from_str(&saved).unwrap();
    assert_eq!(parsed.theme.as_deref(), Some("light"), "theme persisted");
    assert_eq!(parsed.default_model.as_deref(), Some("claude-3"), "existing field preserved");

    unsafe { std::env::remove_var("PI_RUST_DIR"); }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pi-coding-agent interactive::app::tests::settings_menu_cycles_theme_and_persists_to_file -- --nocapture`
Expected: PASS (test exists but no file is written — actually it will pass because the test doesn't check for file writing yet)

Wait — the test above DOES check file writing, so it will fail since no file is written. Let me adjust this.

Actually the test as written should fail because no file gets written. Let me verify the current state first:

Run: `cargo test -p pi-coding-agent interactive::app::tests::settings_menu_cycles_theme_and_persists_to_file -- --nocapture`
Expected: Currently fails because test function doesn't exist yet, or if added, fails because `global.join("settings.toml")` doesn't contain the theme line.

- [ ] **Step 3: Add `settings_delta` field to `InteractiveRoot`**

Add the new field to the struct (near `settings` and `settings_update`):

```rust
pub(super) settings_delta: crate::config::PartialSettings,
```

- [ ] **Step 4: Initialize `settings_delta` in the constructor**

In `new_with_theme_models_and_settings`, after `settings_update: None,` add:

```rust
settings_delta: crate::config::PartialSettings::default(),
```

- [ ] **Step 5: Set delta field in `apply_settings_value` and call save**

In each branch of `apply_settings_value`, set the corresponding field on `self.settings_delta`. Then at the end of the method, before or after `self.settings_update = ...`, call save.

The pattern for each field:

```rust
"theme" => {
    self.settings.theme = Some(value.to_string());
    self.settings_delta.theme = Some(value.to_string());
    self.apply_builtin_theme(value);
}
```

For compaction (nested):
```rust
"auto_compaction" => {
    self.settings.compaction.enabled = value == "on";
    self.settings_delta.compaction
        .get_or_insert_with(|| crate::config::settings::PartialCompaction::default())
        .enabled = Some(value == "on");
}
```

For terminal (nested):
```rust
"show_images" => {
    self.settings.terminal.show_images = value == "on";
    self.settings_delta.terminal
        .get_or_insert_with(|| crate::config::settings::PartialTerminal::default())
        .show_images = Some(value == "on");
}
```

For warnings (nested):
```rust
"warnings_anthropic_extra_usage" => {
    self.settings.warnings.anthropic_extra_usage = value == "on";
    self.settings_delta.warnings
        .get_or_insert_with(|| crate::config::settings::PartialWarnings::default())
        .anthropic_extra_usage = Some(value == "on");
}
```

At the end of `apply_settings_value`, after `self.settings_update = Some(self.settings.clone());`, add:

```rust
// Persist to disk
use crate::config::{merge_and_save_settings, SettingsScope};
let paths = crate::config::resolve_paths(&self.cwd);
let mut diags = Vec::new();
merge_and_save_settings(&paths, SettingsScope::Global, &self.settings_delta, &mut diags);
// Diagnostics are silently dropped for now (matching TS behavior where
// write errors don't interrupt the UI)
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test -p pi-coding-agent interactive::app::tests::settings_menu_cycles_theme_and_persists_to_file -- --nocapture`
Expected: PASS

- [ ] **Step 7: Run all existing settings menu tests to verify nothing broke**

Run: `cargo test -p pi-coding-agent interactive::app::tests::settings_menu -- --nocapture`
Expected: ALL PASS

- [ ] **Step 8: Add a second persistence test for a different setting**

Add after the previous test:

```rust
#[test]
fn settings_menu_toggles_auto_compaction_and_persists() {
    let dir = tempfile::tempdir().unwrap();
    let global = dir.path().join("global");
    std::fs::create_dir_all(&global).unwrap();
    std::fs::write(
        global.join("settings.toml"),
        "[compaction]\nenabled = true\nreserve_tokens = 16384\n",
    )
    .unwrap();

    unsafe { std::env::set_var("PI_RUST_DIR", global.to_str().unwrap()); }

    let mut root = InteractiveRoot::new(
        dir.path().join("work"),
        "faux-model".to_string(),
        "no-session".to_string(),
    );

    root.handle_slash_command(ParsedSlashCommand {
        name: "settings".to_string(),
        args: String::new(),
        original: "/settings".to_string(),
    });
    // Navigate from theme (0) down to auto_compaction (1)
    root.handle_input(&key_event("\x1b[B"));
    root.handle_input(&key_event("\r"));

    let updated = root.take_settings_update().expect("should update");
    assert!(!updated.compaction.enabled);

    let saved = std::fs::read_to_string(global.join("settings.toml")).unwrap();
    let parsed: crate::config::settings::PartialSettings = toml::from_str(&saved).unwrap();
    let c = parsed.compaction.unwrap();
    assert_eq!(c.enabled, Some(false), "compaction.enabled toggled");
    assert_eq!(c.reserve_tokens, Some(16384), "other compaction fields preserved");

    unsafe { std::env::remove_var("PI_RUST_DIR"); }
}
```

- [ ] **Step 9: Run all tests**

Run: `cargo test -p pi-coding-agent -- --nocapture`
Expected: ALL PASS

- [ ] **Step 10: Run full workspace tests**

Run: `cargo test --workspace --nocapture`
Expected: ALL PASS

- [ ] **Step 11: Check formatting**

Run: `cargo fmt --check`
If fails, run `cargo fmt` to fix.

- [ ] **Step 12: Commit**

```bash
git add crates/pi-coding-agent/src/interactive/root.rs crates/pi-coding-agent/src/interactive/app.rs
git commit -m "feat(settings): wire settings persistence into TUI settings menu"
```

---
