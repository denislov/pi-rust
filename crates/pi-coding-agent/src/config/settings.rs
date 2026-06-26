use crate::config::{ConfigDiagnostic, ConfigPaths};
use serde::Deserialize;
use std::path::Path;

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
pub struct PartialTerminal {
    pub show_images: Option<bool>,
    pub show_progress: Option<bool>,
    pub clear_on_shrink: Option<bool>,
    pub auto_resize_images: Option<bool>,
    pub block_images: Option<bool>,
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
    pub skills: Option<Vec<String>>,
    pub prompts: Option<Vec<String>>,
    pub themes: Option<Vec<String>>,
    pub theme: Option<String>,
    pub no_context_files: Option<bool>,
    pub hide_thinking_block: Option<bool>,
    pub collapse_changelog: Option<bool>,
    pub quiet_startup: Option<bool>,
    pub enable_skill_commands: Option<bool>,
    pub double_escape_action: Option<String>,
    pub tree_filter_mode: Option<String>,
    pub terminal: Option<PartialTerminal>,
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
pub struct TerminalSettings {
    pub show_images: bool,
    pub show_progress: bool,
    pub clear_on_shrink: bool,
    pub auto_resize_images: bool,
    pub block_images: bool,
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
    pub skills: Vec<String>,
    pub prompts: Vec<String>,
    pub themes: Vec<String>,
    pub theme: Option<String>,
    pub no_context_files: bool,
    pub hide_thinking_block: bool,
    pub collapse_changelog: bool,
    pub quiet_startup: bool,
    pub enable_skill_commands: bool,
    pub double_escape_action: String,
    pub tree_filter_mode: String,
    pub terminal: TerminalSettings,
    pub compaction: CompactionSettings,
    pub retry: RetrySettings,
}

fn merge_compaction(
    base: Option<PartialCompaction>,
    over: Option<PartialCompaction>,
) -> Option<PartialCompaction> {
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

fn merge_terminal(
    base: Option<PartialTerminal>,
    over: Option<PartialTerminal>,
) -> Option<PartialTerminal> {
    match (base, over) {
        (None, x) | (x, None) => x,
        (Some(b), Some(o)) => Some(PartialTerminal {
            show_images: o.show_images.or(b.show_images),
            show_progress: o.show_progress.or(b.show_progress),
            clear_on_shrink: o.clear_on_shrink.or(b.clear_on_shrink),
            auto_resize_images: o.auto_resize_images.or(b.auto_resize_images),
            block_images: o.block_images.or(b.block_images),
        }),
    }
}

fn merge_vec(base: Option<Vec<String>>, over: Option<Vec<String>>) -> Option<Vec<String>> {
    match (base, over) {
        (None, x) | (x, None) => x,
        (Some(mut base), Some(over)) => {
            base.extend(over);
            Some(base)
        }
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
            skills: merge_vec(self.skills, over.skills),
            prompts: merge_vec(self.prompts, over.prompts),
            themes: merge_vec(self.themes, over.themes),
            theme: over.theme.or(self.theme),
            no_context_files: over.no_context_files.or(self.no_context_files),
            hide_thinking_block: over.hide_thinking_block.or(self.hide_thinking_block),
            collapse_changelog: over.collapse_changelog.or(self.collapse_changelog),
            quiet_startup: over.quiet_startup.or(self.quiet_startup),
            enable_skill_commands: over.enable_skill_commands.or(self.enable_skill_commands),
            double_escape_action: over.double_escape_action.or(self.double_escape_action),
            tree_filter_mode: over.tree_filter_mode.or(self.tree_filter_mode),
            terminal: merge_terminal(self.terminal, over.terminal),
            compaction: merge_compaction(self.compaction, over.compaction),
            retry: merge_retry(self.retry, over.retry),
        }
    }

    pub fn resolve(self) -> Settings {
        let c = self.compaction.unwrap_or_default();
        let r = self.retry.unwrap_or_default();
        let t = self.terminal.unwrap_or_default();
        Settings {
            default_provider: self.default_provider,
            default_model: self.default_model,
            default_thinking_level: self.default_thinking_level,
            transport: self.transport.unwrap_or_else(|| "auto".to_string()),
            steering_mode: self
                .steering_mode
                .unwrap_or_else(|| "one-at-a-time".to_string()),
            follow_up_mode: self
                .follow_up_mode
                .unwrap_or_else(|| "one-at-a-time".to_string()),
            session_dir: self.session_dir,
            skills: self.skills.unwrap_or_default(),
            prompts: self.prompts.unwrap_or_default(),
            themes: self.themes.unwrap_or_default(),
            theme: self.theme,
            no_context_files: self.no_context_files.unwrap_or(false),
            hide_thinking_block: self.hide_thinking_block.unwrap_or(false),
            collapse_changelog: self.collapse_changelog.unwrap_or(false),
            quiet_startup: self.quiet_startup.unwrap_or(false),
            enable_skill_commands: self.enable_skill_commands.unwrap_or(true),
            double_escape_action: self.double_escape_action.unwrap_or_else(|| "tree".to_string()),
            tree_filter_mode: self.tree_filter_mode.unwrap_or_else(|| "default".to_string()),
            terminal: TerminalSettings {
                show_images: t.show_images.unwrap_or(true),
                show_progress: t.show_progress.unwrap_or(true),
                clear_on_shrink: t.clear_on_shrink.unwrap_or(false),
                auto_resize_images: t.auto_resize_images.unwrap_or(true),
                block_images: t.block_images.unwrap_or(false),
            },
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

pub fn load_partial(path: &Path, diags: &mut Vec<ConfigDiagnostic>) -> PartialSettings {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return PartialSettings::default();
        }
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
        assert!(!s.hide_thinking_block);
        assert!(!s.collapse_changelog);
        assert!(!s.quiet_startup);
        assert!(s.enable_skill_commands);
        assert_eq!(s.double_escape_action, "tree");
        assert_eq!(s.tree_filter_mode, "default");
        assert!(!s.terminal.clear_on_shrink);
        assert!(s.terminal.auto_resize_images);
        assert!(!s.terminal.block_images);
    }

    #[test]
    fn project_overrides_global_scalars() {
        let global = PartialSettings {
            default_model: Some("a".into()),
            transport: Some("sse".into()),
            ..Default::default()
        };
        let project = PartialSettings {
            default_model: Some("b".into()),
            ..Default::default()
        };
        let s = global.merge(project).resolve();
        assert_eq!(s.default_model.as_deref(), Some("b")); // project wins
        assert_eq!(s.transport, "sse"); // global survives where project is silent
    }

    #[test]
    fn nested_objects_merge_field_wise() {
        let global = PartialSettings {
            compaction: Some(PartialCompaction {
                reserve_tokens: Some(100),
                keep_recent_tokens: Some(200),
                ..Default::default()
            }),
            ..Default::default()
        };
        let project = PartialSettings {
            compaction: Some(PartialCompaction {
                reserve_tokens: Some(999),
                ..Default::default()
            }),
            ..Default::default()
        };
        let s = global.merge(project).resolve();
        assert_eq!(s.compaction.reserve_tokens, 999); // project overrides
        assert_eq!(s.compaction.keep_recent_tokens, 200); // global field survives
        assert!(s.compaction.enabled); // default fills the gap
    }

    #[test]
    fn terminal_theme_and_context_settings_resolve_defaults_and_merge() {
        let global = PartialSettings {
            theme: Some("dark".into()),
            no_context_files: Some(true),
            terminal: Some(PartialTerminal {
                show_images: Some(false),
                show_progress: Some(false),
                ..Default::default()
            }),
            ..Default::default()
        };
        let project = PartialSettings {
            theme: Some("light".into()),
            terminal: Some(PartialTerminal {
                show_progress: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        };

        let s = global.merge(project).resolve();

        assert_eq!(s.theme.as_deref(), Some("light"));
        assert!(s.no_context_files);
        assert!(!s.terminal.show_images);
        assert!(s.terminal.show_progress);
    }

    #[test]
    fn terminal_new_fields_merge_field_wise() {
        let global = PartialSettings {
            terminal: Some(PartialTerminal {
                clear_on_shrink: Some(true),
                auto_resize_images: Some(false),
                ..Default::default()
            }),
            ..Default::default()
        };
        let project = PartialSettings {
            terminal: Some(PartialTerminal {
                auto_resize_images: Some(true),
                block_images: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        };
        let s = global.merge(project).resolve();
        assert!(s.terminal.clear_on_shrink);      // global survives
        assert!(s.terminal.auto_resize_images);    // project overrides
        assert!(s.terminal.block_images);          // project adds
    }

    #[test]
    fn scalar_new_fields_merge_and_resolve() {
        let global = PartialSettings {
            hide_thinking_block: Some(true),
            quiet_startup: Some(true),
            double_escape_action: Some("fork".into()),
            ..Default::default()
        };
        let project = PartialSettings {
            hide_thinking_block: Some(false),
            collapse_changelog: Some(true),
            tree_filter_mode: Some("user-only".into()),
            ..Default::default()
        };
        let s = global.merge(project).resolve();
        assert!(!s.hide_thinking_block);           // project overrides
        assert!(s.collapse_changelog);              // project adds
        assert!(s.quiet_startup);                   // global survives
        assert!(s.enable_skill_commands);            // default
        assert_eq!(s.double_escape_action, "fork"); // global survives
        assert_eq!(s.tree_filter_mode, "user-only");// project overrides
    }

    #[test]
    fn terminal_defaults_are_enabled_and_context_files_default_on() {
        let s = PartialSettings::default().resolve();
        assert!(s.terminal.show_images);
        assert!(s.terminal.show_progress);
        assert!(!s.terminal.clear_on_shrink);
        assert!(s.terminal.auto_resize_images);
        assert!(!s.terminal.block_images);
        assert!(!s.no_context_files);
        assert!(!s.hide_thinking_block);
        assert!(s.enable_skill_commands);
        assert_eq!(s.double_escape_action, "tree");
        assert_eq!(s.tree_filter_mode, "default");
        assert!(s.theme.is_none());
    }

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
        std::fs::write(
            global.path().join("settings.toml"),
            "default_model = \"g\"\ntransport = \"sse\"\n",
        )
        .unwrap();
        std::fs::write(
            project.path().join("settings.toml"),
            "default_model = \"p\"\n",
        )
        .unwrap();
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

    #[test]
    fn resource_paths_merge_global_and_project_settings() {
        let global = PartialSettings {
            skills: Some(vec!["global-skill".into()]),
            prompts: Some(vec!["global-prompt".into()]),
            themes: Some(vec!["global-theme".into()]),
            ..Default::default()
        };
        let project = PartialSettings {
            skills: Some(vec!["project-skill".into()]),
            prompts: Some(vec!["project-prompt".into()]),
            themes: Some(vec!["project-theme".into()]),
            ..Default::default()
        };

        let s = global.merge(project).resolve();

        assert_eq!(s.skills, vec!["global-skill", "project-skill"]);
        assert_eq!(s.prompts, vec!["global-prompt", "project-prompt"]);
        assert_eq!(s.themes, vec!["global-theme", "project-theme"]);
    }
}
