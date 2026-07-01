use std::collections::BTreeSet;
use std::io::IsTerminal;
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
#[cfg(test)]
use std::sync::{Arc, Mutex};

use pi_agent_core::{AgentResources, session::create_session_id};
use pi_ai::types::Model;
#[cfg(test)]
use pi_tui::{Component, InputEvent, Terminal, visible_width};
use pi_tui::{KeybindingsManager, ProcessTerminal, TuiTheme, dark_theme, light_theme};

#[cfg(test)]
use crate::interactive::clipboard::ClipboardSink;
use crate::interactive::input::InputPump;
use crate::interactive::key_hints::{app_key_hint, key_hint};
use crate::interactive::r#loop::{
    LoopResult, run_interactive_loop, run_interactive_loop_with_input,
};
#[cfg(test)]
use crate::interactive::render::{
    TranscriptRenderCache, TranscriptRenderOptions, TranscriptStyles, format_tokens,
    render_transcript_lines, running_status_text,
};
#[cfg(test)]
use crate::interactive::root::{
    FooterStats, InteractiveAction, InteractiveRoot, InteractiveStatus,
};
#[cfg(test)]
use crate::interactive::session_actions::SessionChoiceKind;
use crate::interactive::session_actions::{SessionChoice, collect_session_choices};
#[cfg(test)]
use crate::interactive::slash::{
    ParsedSlashCommand, builtin_slash_commands, help_text, parse_slash_command,
};
#[cfg(test)]
use crate::interactive::{Transcript, TranscriptItem, UiEvent};
use crate::request::resolve_cli_context;
use crate::runtime::{SessionMode, SessionRunOptions};
use crate::session::ResolvedSessionTarget;
use crate::{CliArgs, CliError, CliOutput, CliRunOptions, config, resources};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InteractiveModeOptions {
    pub terminal_required: bool,
}

impl Default for InteractiveModeOptions {
    fn default() -> Self {
        Self {
            terminal_required: true,
        }
    }
}

pub async fn run_interactive_mode(parsed: CliArgs, options: CliRunOptions) -> CliOutput {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return CliOutput {
            exit_code: 1,
            stdout: String::new(),
            stderr: "interactive mode requires a TTY\n".to_string(),
        };
    }

    let terminal = ProcessTerminal::new();
    match run_interactive_loop_with_input(parsed, options, terminal, InputPump::from_stdin).await {
        Ok(result) => CliOutput {
            exit_code: result.exit_code,
            stdout: String::new(),
            stderr: String::new(),
        },
        Err(error) => CliOutput {
            exit_code: 1,
            stdout: String::new(),
            stderr: format!("{error}\n"),
        },
    }
}

static INTERACTIVE_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Clone)]
pub(super) struct PromptContext {
    pub(super) model: Model,
    pub(super) api_key: Option<String>,
    pub(super) cli_api_key: Option<String>,
    pub(super) auth: crate::config::AuthStore,
    pub(super) system_prompt: Option<String>,
    pub(super) max_turns: Option<u32>,
    pub(super) tools: Vec<pi_agent_core::AgentTool>,
    pub(super) register_builtins: bool,
    pub(super) session: Option<SessionRunOptions>,
    pub(super) session_target: Option<ResolvedSessionTarget>,
    pub(super) session_name: Option<String>,
    pub(super) thinking_level: Option<pi_agent_core::ThinkingLevel>,
    pub(super) tool_execution: Option<pi_agent_core::ToolExecutionMode>,
    pub(super) resources: AgentResources,
    pub(super) context_files: Vec<crate::resources::ContextFile>,
    pub(super) settings: crate::config::Settings,
    pub(super) theme: TuiTheme,
    pub(super) resolved_theme: crate::theme::ResolvedTheme,
    /// Custom themes directory (global `<agent_dir>/themes`), used to start
    /// the hot-reload watcher. Mirrors TS `getCustomThemesDir`.
    pub(super) themes_dir: PathBuf,
    pub(super) model_choices: Vec<Model>,
    pub(super) model_rotation: Vec<Model>,
    pub(super) session_choices: Vec<SessionChoice>,
}

pub(super) fn build_prompt_context(
    parsed: &CliArgs,
    options: CliRunOptions,
) -> Result<PromptContext, CliError> {
    let cwd = options.session.cwd.clone();
    let config_paths = config::resolve_paths(&cwd);
    let resolved = resolve_cli_context(
        parsed.clone(),
        options,
        cwd,
        config_paths.global_dir.clone(),
    )?;
    let diagnostic_text = crate::request::render_diagnostics(&resolved.diagnostics);
    if !diagnostic_text.is_empty() {
        eprint!("{diagnostic_text}");
    }
    let model_rotation = rotation_model_choices(
        parsed.models.as_deref(),
        parsed
            .provider
            .as_deref()
            .or(resolved.config.settings.default_provider.as_deref()),
        Some(&resolved.config.settings.enabled_models),
    )?;
    let model_choices = configured_model_choices(
        &resolved.model,
        parsed.api_key.as_deref(),
        &resolved.config.auth,
    );
    let theme = resolve_tui_theme(
        resolved.config.settings.theme.as_deref(),
        resolved.loaded_resources.selected_theme.as_ref(),
    );
    let resolved_theme = resolve_resolved_theme(
        resolved.config.settings.theme.as_deref(),
        resolved.loaded_resources.selected_theme.as_ref(),
    );

    let session_target = match (&resolved.session, resolved.session_target.clone()) {
        (Some(session), None) if matches!(session.mode, SessionMode::Enabled) => {
            Some(ResolvedSessionTarget::OpenOrCreateId(create_session_id()))
        }
        (_, target) => target,
    };
    let session_choices = collect_session_choices(&resolved.session);

    Ok(PromptContext {
        model: resolved.model,
        api_key: resolved.api_key,
        cli_api_key: parsed.api_key.clone(),
        auth: resolved.config.auth,
        system_prompt: resolved.system_prompt,
        max_turns: parsed.max_turns,
        tools: resolved.tools,
        register_builtins: resolved.register_builtins,
        session: resolved.session,
        session_target,
        session_name: resolved.session_name,
        thinking_level: parsed.thinking.or_else(|| {
            resolved
                .config
                .settings
                .default_thinking_level
                .as_deref()
                .and_then(|s| s.parse().ok())
        }),
        tool_execution: parsed.tool_execution,
        resources: resolved.agent_resources,
        context_files: crate::resources::discover_context_files(
            &resolved.cwd,
            &resolved.config_paths.global_dir,
            crate::runtime::effective_no_context_files(&resolved.parsed, &resolved.config.settings),
        ),
        settings: resolved.config.settings,
        theme,
        resolved_theme,
        themes_dir: config_paths.global_dir.join("themes"),
        model_choices,
        model_rotation,
        session_choices,
    })
}

fn resolve_tui_theme(
    theme_name: Option<&str>,
    selected: Option<&resources::ThemeResource>,
) -> TuiTheme {
    if let Some(theme) = selected {
        return resources::tui_theme_from_resource(theme);
    }
    match theme_name {
        Some("light") => light_theme(),
        _ => dark_theme(),
    }
}

/// Resolve the active theme into the full 51-token [`ResolvedTheme`] used for
/// thinking-level editor borders and other token-driven rendering. Invalid
/// user themes fall back to the built-in dark theme, mirroring TS `setTheme`.
fn resolve_resolved_theme(
    theme_name: Option<&str>,
    selected: Option<&resources::ThemeResource>,
) -> crate::theme::ResolvedTheme {
    if let Some(theme) = selected {
        if let Ok(resolved) = theme.theme.resolve_colors() {
            return resolved;
        }
    }
    let json = match theme_name {
        Some("light") => crate::theme::builtin_light(),
        _ => crate::theme::builtin_dark(),
    };
    json.resolve_colors().unwrap_or_else(|_| {
        crate::theme::builtin_dark()
            .resolve_colors()
            .expect("built-in dark theme resolves")
    })
}

pub(super) fn resolve_prompt_api_key(
    provider: &str,
    cli_api_key: Option<&str>,
    auth: &crate::config::AuthStore,
) -> (Option<String>, Vec<crate::request::CliDiagnostic>) {
    let mut key_diags = Vec::new();
    let resolved = config::auth::resolve_api_key(provider, cli_api_key, auth, &mut key_diags);
    let diagnostics = key_diags
        .iter()
        .map(crate::request::CliDiagnostic::from_config)
        .collect();
    (resolved.map(|r| r.value), diagnostics)
}

fn configured_model_choices(
    current_model: &Model,
    cli_api_key: Option<&str>,
    auth: &crate::config::AuthStore,
) -> Vec<Model> {
    let mut configured_providers = BTreeSet::new();
    for provider in pi_ai::get_providers() {
        if provider_has_configured_key(&provider, &current_model.provider, cli_api_key, auth) {
            configured_providers.insert(provider);
        }
    }

    let mut models = pi_ai::all_models()
        .iter()
        .filter(|model| configured_providers.contains(&model.provider))
        .cloned()
        .collect::<Vec<_>>();
    models.sort_by(|left, right| {
        left.provider
            .cmp(&right.provider)
            .then_with(|| left.id.cmp(&right.id))
    });
    if let Some(current_index) = models
        .iter()
        .position(|model| model.provider == current_model.provider && model.id == current_model.id)
    {
        let current = models.remove(current_index);
        models.insert(0, current);
    }
    models
}

fn rotation_model_choices(
    models_arg: Option<&str>,
    provider: Option<&str>,
    enabled_models: Option<&[String]>,
) -> Result<Vec<Model>, CliError> {
    // CLI `--models` takes precedence over settings `enabled_models`
    let models_arg: Option<String> = match models_arg {
        Some(arg) => Some(arg.to_string()),
        None => enabled_models
            .filter(|list| !list.is_empty())
            .map(|list| list.join(",")),
    };
    let Some(ref models_arg) = models_arg else {
        return Ok(Vec::new());
    };
    let rotation = crate::models::parse_model_rotation(&models_arg)?;
    let mut candidates = pi_ai::all_models().to_vec();
    candidates.sort_by(|left, right| left.id.cmp(&right.id));
    if let Some(provider) = provider {
        candidates.retain(|model| model.provider == provider);
    }
    Ok(candidates
        .into_iter()
        .filter(|model| rotation.matches(&model.id) || rotation.matches(&model.name))
        .collect())
}

fn provider_has_configured_key(
    provider: &str,
    current_provider: &str,
    cli_api_key: Option<&str>,
    auth: &crate::config::AuthStore,
) -> bool {
    if provider == current_provider && cli_api_key.is_some_and(|key| !key.is_empty()) {
        return true;
    }
    let mut diags = Vec::new();
    config::auth::resolve_api_key(provider, None, auth, &mut diags).is_some()
}

pub(super) fn session_label(session: &Option<SessionRunOptions>) -> String {
    match session {
        Some(session) if matches!(session.mode, SessionMode::Enabled) => "session".to_string(),
        _ => "no-session".to_string(),
    }
}

pub(super) fn welcome_line(keybindings: &KeybindingsManager) -> String {
    format!(
        "pi-rust {}\n{} · {} · /help\n{} · {}",
        env!("CARGO_PKG_VERSION"),
        key_hint(keybindings, "tui.input.submit", "submit"),
        key_hint(keybindings, "tui.input.newLine", "newline"),
        app_key_hint(keybindings, "app.interrupt", "interrupt/exit"),
        app_key_hint(keybindings, "app.tools.expand", "expand tools"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pi_tui::StdinBuffer;

    /// Build render options with no resolved theme (fallback palette) and
    /// color disabled, for layout-focused assertions.
    fn opts(width: usize, max_tool_result_lines: usize) -> TranscriptRenderOptions<'static> {
        TranscriptRenderOptions {
            width,
            max_tool_result_lines,
            color: false,
            markdown_theme: pi_tui::MarkdownTheme::default(),
            hide_thinking_block: false,
            hidden_thinking_label: "Thinking...",
            styles: TranscriptStyles::from_theme(None),
        }
    }

    /// Same as [`opts`] but with color enabled, for style assertions.
    fn opts_color(width: usize, max_tool_result_lines: usize) -> TranscriptRenderOptions<'static> {
        let mut o = opts(width, max_tool_result_lines);
        o.color = true;
        o
    }

    fn key_event(data: &str) -> InputEvent {
        let mut buffer = StdinBuffer::new();
        let mut events = buffer.process(data);
        events.extend(buffer.flush());
        assert_eq!(events.len(), 1, "expected exactly one input event");
        events.remove(0)
    }

    fn ctrl_p_event(shift: bool) -> InputEvent {
        let mut modifiers = pi_tui::KeyModifiers::CTRL;
        if shift {
            modifiers.insert(pi_tui::KeyModifiers::SHIFT);
        }
        InputEvent::Key(pi_tui::KeyEvent {
            key: pi_tui::Key::Char(if shift { "P".into() } else { "p".into() }),
            modifiers,
            kind: pi_tui::KeyEventKind::Press,
        })
    }

    #[test]
    fn transcript_render_cache_reuses_history_and_misses_changed_block_only() {
        let mut transcript = Transcript::new();
        transcript.push(TranscriptItem::system("welcome"));
        transcript.push(TranscriptItem::user("hello"));
        transcript.apply_event(UiEvent::AssistantDelta {
            text: "first reply".to_string(),
        });

        let mut cache = TranscriptRenderCache::new();
        let opts = opts(80, 3);
        let first = cache.render_lines(&transcript, &opts);
        assert!(first.join("\n").contains("first reply"));
        assert_eq!(cache.stats().block_misses, 3);

        cache.reset_stats();
        let second = cache.render_lines(&transcript, &opts);
        assert_eq!(second, first);
        assert_eq!(cache.stats().block_hits, 3);
        assert_eq!(cache.stats().block_misses, 0);

        transcript.apply_event(UiEvent::AssistantDelta {
            text: " plus delta".to_string(),
        });
        cache.reset_stats();
        let updated = cache.render_lines(&transcript, &opts);
        let stats = cache.stats();
        assert!(updated.join("\n").contains("first reply plus delta"));
        assert_eq!(stats.block_hits, 2, "{stats:?}");
        assert_eq!(stats.block_misses, 1, "{stats:?}");
    }

    #[test]
    fn scrolled_streaming_uses_per_block_row_delta() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "model".to_string(),
            "session".to_string(),
        );
        for index in 0..40 {
            root.transcript
                .push(TranscriptItem::user(format!("message {index}")));
        }
        root.transcript.scroll_page_up(12);
        let _ = root.render(100);

        root.reset_render_cache_stats();
        root.apply_events(vec![UiEvent::AssistantDelta {
            text: "streaming reply".to_string(),
        }]);

        let stats = root.render_cache_stats();
        assert_eq!(stats.row_metadata_hits, 1, "{stats:?}");
        assert_eq!(stats.row_delta_hits, 1, "{stats:?}");
        assert_eq!(stats.row_delta_fallbacks, 0, "{stats:?}");
        assert_eq!(stats.line_count_hits, 0, "{stats:?}");
        assert_eq!(stats.line_count_misses, 0, "{stats:?}");
        assert_eq!(stats.block_misses, 1, "{stats:?}");
        assert_eq!(stats.block_hits, 0, "{stats:?}");
        assert!(root.transcript.scroll_offset() > 12);
    }

    #[test]
    fn scrolled_streaming_preserves_exact_wrapped_row_delta() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "model".to_string(),
            "session".to_string(),
        );
        root.set_viewport_size(24, 24);
        for index in 0..8 {
            root.transcript
                .push(TranscriptItem::user(format!("message {index}")));
        }
        root.transcript.scroll_page_up(4);
        let _ = root.render(24);

        let before = root.transcript.clone();
        let previous_offset = root.transcript.scroll_offset();
        root.apply_events(vec![UiEvent::AssistantDelta {
            text: "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda".to_string(),
        }]);

        let mut check_cache = TranscriptRenderCache::new();
        let before_rows = check_cache.render_lines(&before, &opts(24, 3)).len();
        let after_rows = check_cache
            .render_lines(&root.transcript, &opts(24, 3))
            .len();
        let expected_delta = after_rows.saturating_sub(before_rows);
        assert!(
            expected_delta > 1,
            "expected wrapped output: {expected_delta}"
        );
        assert_eq!(
            root.transcript.scroll_offset(),
            previous_offset + expected_delta
        );
    }

    #[test]
    #[ignore = "local performance baseline; prints render timings and cache stats"]
    fn stress_interactive_transcript_render_baseline() {
        fn elapsed_ms(start: std::time::Instant) -> f64 {
            start.elapsed().as_secs_f64() * 1000.0
        }

        for item_count in [100usize, 1_000, 5_000] {
            let mut transcript = Transcript::new();
            transcript.push(TranscriptItem::system("welcome"));
            for index in 0..item_count {
                if index % 2 == 0 {
                    transcript.push(TranscriptItem::user(format!(
                        "user message {index}\n\n- one\n- two\n- three"
                    )));
                } else {
                    transcript.push(TranscriptItem::assistant(
                        format!("assistant_{index}"),
                        format!(
                            "assistant message {index}\n\n```rust\nfn main() {{ println!(\"{index}\"); }}\n```"
                        ),
                        true,
                    ));
                }
            }

            let opts = opts(120, 3);
            let mut cache = TranscriptRenderCache::new();
            let first_start = std::time::Instant::now();
            let first = cache.render_lines(&transcript, &opts);
            let first_ms = elapsed_ms(first_start);
            cache.reset_stats();
            let second_start = std::time::Instant::now();
            let second = cache.render_lines(&transcript, &opts);
            let second_ms = elapsed_ms(second_start);
            let second_stats = cache.stats();
            assert_eq!(first, second);

            cache.reset_stats();
            let count_start = std::time::Instant::now();
            let rows = cache.line_count(&transcript, &opts);
            let count_ms = elapsed_ms(count_start);
            let count_stats = cache.stats();

            println!(
                "items={item_count} rows={} first_ms={first_ms:.3} second_ms={second_ms:.3} count_ms={count_ms:.3} second_stats={second_stats:?} count_stats={count_stats:?}",
                rows
            );
        }

        let mut streaming = Transcript::new();
        streaming.push(TranscriptItem::system("welcome"));
        streaming.apply_event(UiEvent::AssistantDelta {
            text: "start".to_string(),
        });
        let opts = opts(120, 3);
        let mut cache = TranscriptRenderCache::new();
        let start = std::time::Instant::now();
        for delta in 0..100 {
            streaming.apply_event(UiEvent::AssistantDelta {
                text: format!(" delta-{delta}"),
            });
            let _ = cache.render_lines(&streaming, &opts);
        }
        println!("streaming_deltas=100 total_ms={:.3}", elapsed_ms(start));
    }

    #[test]
    fn build_prompt_context_uses_config_defaults_and_auth() {
        let _guard = crate::test_support::env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.toml"),
            "default_model = \"claude-haiku-4-5\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("auth.toml"),
            "[anthropic]\ntype = \"api_key\"\nkey = \"from-auth\"\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                dir.path().join("auth.toml"),
                std::fs::Permissions::from_mode(0o600),
            )
            .unwrap();
        }
        unsafe {
            std::env::set_var("PI_RUST_DIR", dir.path().to_str().unwrap());
        }

        let ctx = build_prompt_context(&CliArgs::default(), CliRunOptions::default()).unwrap();

        assert_eq!(ctx.model.id, "claude-haiku-4-5");
        assert_eq!(ctx.api_key.as_deref(), Some("from-auth"));

        unsafe {
            std::env::remove_var("PI_RUST_DIR");
        }
    }

    #[test]
    fn build_prompt_context_applies_selected_theme_to_editor_borders() {
        let _guard = crate::test_support::env_lock();
        let dir = tempfile::tempdir().unwrap();
        let themes_dir = dir.path().join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();
        std::fs::write(dir.path().join("settings.toml"), "theme = \"violet\"\n").unwrap();
        std::fs::write(
            themes_dir.join("violet.json"),
            r##"{
                "name": "violet",
                "vars": {
                    "ib": "#211144",
                    "mb": "#1234aa"
                },
                "colors": {
                    "borderMuted": "ib",
                    "border": "mb"
                }
            }"##,
        )
        .unwrap();
        unsafe {
            std::env::set_var("PI_RUST_DIR", dir.path().to_str().unwrap());
        }

        let ctx = build_prompt_context(&CliArgs::default(), CliRunOptions::default()).unwrap();

        assert_eq!(
            ctx.theme.editor.active_border.fg,
            pi_tui::Color::Rgb(0x21, 0x11, 0x44)
        );
        assert_eq!(
            ctx.theme.editor.menu_border.fg,
            pi_tui::Color::Rgb(0x12, 0x34, 0xaa)
        );

        unsafe {
            std::env::remove_var("PI_RUST_DIR");
        }
    }

    #[test]
    fn render_transcript_lines_compacts_tool_rows_and_truncates_noisy_output() {
        // New visual spec (plan stage 2): tool blocks get a status background
        // shell, the header carries tool name + target + status, results are
        // indented two columns, and a collapsed view shows an "expand tools"
        // hint instead of a bare "truncated" note. Block spacing is handled
        // by the caller, so a single tool block renders with no leading gap.
        let mut transcript = Transcript::new();
        transcript.apply_event(UiEvent::ToolStarted {
            call_id: "tool_1".to_string(),
            name: "read".to_string(),
            args: serde_json::json!({"path": "src/lib.rs"}),
        });

        let pending = render_transcript_lines(&transcript, &opts(80, 3));
        assert_eq!(pending.len(), 1, "{pending:?}");
        assert!(
            pending[0]
                .trim_start()
                .starts_with("read src/lib.rs running")
        );

        transcript.apply_event(UiEvent::ToolFinished {
            call_id: "tool_1".to_string(),
            result: "line 1\nline 2\nline 3\nline 4\nline 5".to_string(),
            is_error: false,
        });

        let collapsed = render_transcript_lines(&transcript, &opts(80, 3));
        // Header + 3 result lines + 1 "more lines" hint.
        assert_eq!(collapsed.len(), 5, "{collapsed:?}");
        assert!(
            collapsed[0]
                .trim_start()
                .starts_with("read src/lib.rs done"),
            "{}",
            collapsed[0]
        );
        assert!(collapsed[1].trim_start().starts_with("line 1"));
        assert!(collapsed[2].trim_start().starts_with("line 2"));
        assert!(collapsed[3].trim_start().starts_with("line 3"));
        assert!(
            collapsed[4]
                .trim_start()
                .starts_with("... 2 more lines (expand tools)"),
            "{}",
            collapsed[4]
        );

        let expanded = render_transcript_lines(&transcript, &opts(80, 20));
        assert_eq!(expanded.len(), 6, "{expanded:?}");
        assert!(expanded[5].trim_start().starts_with("line 5"));
    }

    #[test]
    fn render_transcript_lines_uses_tool_targets_and_does_not_truncate_write_or_edit() {
        let mut transcript = Transcript::new();
        transcript.push(TranscriptItem::Tool {
            call_id: "call_write".to_string(),
            name: "write".to_string(),
            args: serde_json::json!({"path": "src/main.rs"}),
            result: Some("w1\nw2\nw3\nw4\nw5".to_string()),
            is_error: false,
        });
        transcript.push(TranscriptItem::Tool {
            call_id: "call_edit".to_string(),
            name: "edit".to_string(),
            args: serde_json::json!({"file_path": "src/lib.rs"}),
            result: Some("e1\ne2\ne3\ne4\ne5".to_string()),
            is_error: false,
        });
        transcript.push(TranscriptItem::Tool {
            call_id: "call_bash".to_string(),
            name: "bash".to_string(),
            args: serde_json::json!({"command": "cargo test -p pi-coding-agent"}),
            result: Some("ok".to_string()),
            is_error: false,
        });

        let lines = render_transcript_lines(&transcript, &opts(120, 3));
        // write/edit keep all result lines (no truncation); bash yields 1 line.
        // Each block is separated from the next by one blank line.
        let headers: Vec<&str> = lines
            .iter()
            .map(|line| line.trim())
            .filter(|line| {
                line.starts_with("write ") || line.starts_with("edit ") || line.starts_with("$ ")
            })
            .collect();
        assert_eq!(
            headers,
            [
                "write src/main.rs done",
                "edit src/lib.rs done",
                "$ cargo test -p pi-coding-agent done",
            ],
            "{lines:?}"
        );
        // write results w1..w5 all present (no "more lines" hint for write/edit).
        let body: String = lines
            .iter()
            .map(|l| l.trim())
            .collect::<Vec<_>>()
            .join("\n");
        for w in ["w1", "w2", "w3", "w4", "w5"] {
            assert!(body.contains(w), "write result {w} missing: {body}");
        }
        for e in ["e1", "e2", "e3", "e4", "e5"] {
            assert!(body.contains(e), "edit result {e} missing: {body}");
        }
        // Two blank-line separators between the three blocks.
        assert_eq!(
            lines.iter().filter(|l| l.trim().is_empty()).count(),
            2,
            "expected one blank line between each block: {lines:?}"
        );
    }

    #[test]
    fn render_transcript_lines_separates_finished_tool_and_assistant_with_blank_line() {
        // Plan stage 1 spacing policy: the old full-width `─` rule between a
        // finished tool and the following assistant message is removed; a
        // single blank line now separates the blocks.
        let mut transcript = Transcript::new();
        transcript.push(TranscriptItem::Tool {
            call_id: "call_read".to_string(),
            name: "read".to_string(),
            args: serde_json::json!({"path": "src/lib.rs"}),
            result: Some("contents".to_string()),
            is_error: false,
        });
        transcript.apply_event(UiEvent::AssistantDelta {
            text: "next answer".to_string(),
        });
        transcript.apply_event(UiEvent::AssistantDone);

        let lines = render_transcript_lines(&transcript, &opts(40, 3));
        assert!(
            lines[0].trim_start().starts_with("read src/lib.rs done"),
            "{}",
            lines[0]
        );
        assert!(
            lines[1].trim_start().starts_with("contents"),
            "{}",
            lines[1]
        );
        // Exactly one blank line separates the tool block from the assistant.
        assert_eq!(lines[2], "", "expected blank separator: {lines:?}");
        assert!(
            lines[3].trim_start().starts_with("next answer"),
            "{}",
            lines[3]
        );
        // No full-width rule anywhere in the frame.
        assert!(
            !lines.iter().any(|l| l.contains('─')),
            "rule should be gone: {lines:?}"
        );
    }

    #[test]
    fn render_transcript_lines_colors_error_item_red_bold() {
        use pi_tui::{ERROR, paint_with};
        let red_bold = |s: &str| paint_with(s, &ERROR, true);
        let mut transcript = Transcript::new();
        transcript.push(TranscriptItem::Error {
            text: "boom".to_string(),
        });

        // With color: `Error:` label + body, both red bold (TS assistant-message
        // error fallback style).
        let colored = render_transcript_lines(&transcript, &opts_color(80, 3));
        assert_eq!(
            colored,
            vec![format!("{} {}", red_bold("Error:"), red_bold("boom"))]
        );

        // Without color: plain `Error: boom`.
        let plain = render_transcript_lines(&transcript, &opts(80, 3));
        assert_eq!(plain, vec!["Error: boom".to_string()]);
    }

    #[test]
    fn ctrl_o_toggles_tool_output_expansion_in_root() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.set_viewport_size(40, 24);
        root.transcript.push(TranscriptItem::Tool {
            call_id: "tool_1".to_string(),
            name: "read".to_string(),
            args: serde_json::Value::Null,
            result: Some("l1\nl2\nl3\nl4\nl5\nl6".to_string()),
            is_error: false,
        });

        let collapsed = root.render(40).join("\n");
        assert!(
            collapsed.contains("more lines"),
            "collapsed tool output should show a truncation hint: {collapsed}"
        );

        // Ctrl+O is the single byte 0x0f, which parse_control_char maps to
        // Key::Char("o") + CTRL. Feed it through StdinBuffer like the real loop.
        let mut buffer = StdinBuffer::new();
        let events = buffer.process("\x0f");
        assert_eq!(events.len(), 1, "ctrl+o should produce one input event");
        root.handle_input(&events[0]);
        assert!(
            root.tool_output_expanded,
            "ctrl+o should flip the expand flag"
        );

        let expanded = root.render(40).join("\n");
        assert!(
            !expanded.contains("more lines"),
            "expanded tool output should not show a truncation hint: {expanded}"
        );
        assert!(
            expanded.contains("l6"),
            "expanded tool output should show the last line: {expanded}"
        );
    }

    #[test]
    fn footer_shows_spinner_when_running() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.set_status(InteractiveStatus::Running);
        let footer = root.footer(80).join("\n");
        assert!(
            footer.contains("running"),
            "footer should contain 'running' when status is Running: {footer}"
        );
        let has_spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
            .iter()
            .any(|frame| footer.contains(frame));
        assert!(
            has_spinner,
            "footer should contain a braille spinner char when Running: {footer}"
        );
    }

    #[test]
    fn running_status_text_uses_loader_sequence() {
        assert_eq!(running_status_text(0), "⠋ running");
        assert_eq!(running_status_text(1), "⠙ running");
    }

    #[test]
    fn footer_no_spinner_when_idle() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.set_status(InteractiveStatus::Idle);
        let footer = root.footer(80).join("\n");
        assert!(
            footer.contains("status: idle"),
            "footer should contain 'status: idle' when Idle: {footer}"
        );
        let has_spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
            .iter()
            .any(|frame| footer.contains(frame));
        assert!(
            !has_spinner,
            "footer should NOT contain a braille spinner char when Idle: {footer}"
        );
    }

    #[test]
    fn spinner_frame_advances_through_sequence() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.set_status(InteractiveStatus::Running);

        root.spinner_frame = 3;
        let footer_at_3 = root.footer(80).join("\n");
        assert!(
            footer_at_3.contains("⠸"),
            "footer at frame 3 should contain '⠸': {footer_at_3}"
        );

        root.spinner_frame = 4;
        let footer_at_4 = root.footer(80).join("\n");
        assert!(
            footer_at_4.contains("⠼"),
            "footer at frame 4 should contain '⠼': {footer_at_4}"
        );
    }

    fn footer_model(id: &str, provider: &str, reasoning: bool, context_window: u32) -> Model {
        Model {
            id: id.into(),
            name: id.into(),
            api: "faux".into(),
            provider: provider.into(),
            base_url: String::new(),
            reasoning,
            thinking_level_map: None,
            input: vec![pi_ai::types::ModelInput::Text],
            cost: pi_ai::types::ModelCost::default(),
            context_window,
            max_tokens: 0,
            headers: None,
            compat: None,
        }
    }

    #[test]
    fn format_tokens_uses_decimal_tiers() {
        assert_eq!(format_tokens(999), "999");
        assert_eq!(format_tokens(1_500), "1.5k");
        assert_eq!(format_tokens(15_000), "15k");
        assert_eq!(format_tokens(1_500_000), "1.5M");
        assert_eq!(format_tokens(15_000_000), "15M");
    }

    #[test]
    fn footer_stats_line_shows_cache_and_cost() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "session".to_string(),
        );
        root.stats = FooterStats {
            input: 10,
            output: 20,
            cache_read: 30,
            cache_write: 40,
            cost: 0.125,
            context_tokens: None,
        };
        let footer = root.footer(80).join("\n");
        assert!(footer.contains("↑10"), "{footer}");
        assert!(footer.contains("↓20"), "{footer}");
        assert!(footer.contains("R30"), "{footer}");
        assert!(footer.contains("W40"), "{footer}");
        assert!(footer.contains("$0.125"), "{footer}");
    }

    #[test]
    fn footer_shows_context_percentage_and_auto_indicator() {
        let mut root =
            InteractiveRoot::new(PathBuf::from("."), "m".to_string(), "session".to_string());
        root.model = Some(footer_model("m", "p", false, 200_000));
        root.stats.context_tokens = Some(100_000);
        let footer = root.footer(80).join("\n");
        assert!(footer.contains("50.0%/200k (auto)"), "{footer}");
    }

    #[test]
    fn footer_shows_unknown_context_after_compaction() {
        let mut root =
            InteractiveRoot::new(PathBuf::from("."), "m".to_string(), "session".to_string());
        root.model = Some(footer_model("m", "p", false, 200_000));
        root.stats.context_tokens = None;
        let footer = root.footer(80).join("\n");
        assert!(footer.contains("?/200k (auto)"), "{footer}");
    }

    #[test]
    fn footer_auto_indicator_off_when_compaction_disabled() {
        let mut root =
            InteractiveRoot::new(PathBuf::from("."), "m".to_string(), "session".to_string());
        root.model = Some(footer_model("m", "p", false, 200_000));
        root.settings.compaction.enabled = false;
        let footer = root.footer(80).join("\n");
        assert!(!footer.contains("(auto)"), "{footer}");
    }

    #[test]
    fn footer_shows_thinking_level_for_reasoning_model() {
        let mut root =
            InteractiveRoot::new(PathBuf::from("."), "m".to_string(), "session".to_string());
        root.model = Some(footer_model("m", "p", true, 200_000));
        root.thinking_level = pi_agent_core::ThinkingLevel::High;
        let footer = root.footer(80).join("\n");
        assert!(footer.contains("m • high"), "{footer}");

        root.thinking_level = pi_agent_core::ThinkingLevel::Off;
        let footer = root.footer(80).join("\n");
        assert!(footer.contains("m • thinking off"), "{footer}");
    }

    #[test]
    fn footer_shows_provider_prefix_with_multiple_providers() {
        let mut root =
            InteractiveRoot::new(PathBuf::from("."), "m".to_string(), "session".to_string());
        root.model = Some(footer_model("m", "anthropic", false, 200_000));
        root.available_models = vec![
            footer_model("m", "anthropic", false, 200_000),
            footer_model("g", "openai", false, 200_000),
        ];
        let footer = root.footer(80).join("\n");
        assert!(footer.contains("(anthropic) m"), "{footer}");
    }

    #[test]
    fn footer_omits_provider_prefix_with_single_provider() {
        let mut root =
            InteractiveRoot::new(PathBuf::from("."), "m".to_string(), "session".to_string());
        root.model = Some(footer_model("m", "anthropic", false, 200_000));
        root.available_models = vec![footer_model("m", "anthropic", false, 200_000)];
        let footer = root.footer(80).join("\n");
        assert!(!footer.contains("(anthropic)"), "{footer}");
    }

    #[test]
    fn footer_lines_never_exceed_width() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "session".to_string(),
        );
        root.model = Some(footer_model("faux-model", "p", true, 200_000));
        root.stats = FooterStats {
            input: 123_456,
            output: 654_321,
            cache_read: 99_999,
            cache_write: 88_888,
            cost: 1.234,
            context_tokens: Some(180_000),
        };
        for width in [10, 20, 40, 80] {
            for line in root.footer(width) {
                assert!(
                    visible_width(&line) <= width,
                    "footer line exceeds width {width}: {:?}",
                    line
                );
            }
        }
    }

    #[test]
    fn footer_pwd_line_includes_git_branch() {
        let dir = tempfile::tempdir().unwrap();
        let git = dir.path().join(".git").join("refs").join("heads");
        std::fs::create_dir_all(&git).unwrap();
        std::fs::write(
            dir.path().join(".git").join("HEAD"),
            "ref: refs/heads/main\n",
        )
        .unwrap();
        let root = InteractiveRoot::new(
            dir.path().to_path_buf(),
            "m".to_string(),
            "session".to_string(),
        );
        let footer = root.footer(80).join("\n");
        assert!(footer.contains("(main)"), "{footer}");
    }

    #[test]
    fn set_status_idle_resets_spinner_frame() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.spinner_frame = 5;
        root.set_status(InteractiveStatus::Idle);
        assert_eq!(
            root.spinner_frame, 0,
            "set_status(Idle) should reset spinner_frame to 0"
        );
    }

    #[test]
    fn render_state_changes_with_spinner_frame() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.set_status(InteractiveStatus::Running);
        root.spinner_frame = 0;
        let state_at_0 = root.render_state();
        root.spinner_frame = 1;
        let state_at_1 = root.render_state();
        assert_ne!(
            state_at_0, state_at_1,
            "render_state should differ when spinner_frame changes"
        );
    }

    #[test]
    fn slash_registry_contains_typescript_builtin_commands() {
        let names: Vec<String> = builtin_slash_commands()
            .iter()
            .map(|command| command.name.clone())
            .collect();
        assert_eq!(
            names,
            [
                "help",
                "settings",
                "model",
                "scoped-models",
                "export",
                "share",
                "copy",
                "name",
                "session",
                "changelog",
                "hotkeys",
                "fork",
                "clone",
                "tree",
                "login",
                "logout",
                "new",
                "compact",
                "resume",
                "reload",
                "quit",
            ]
            .map(String::from)
        );
    }

    #[test]
    fn slash_suggestions_render_when_editor_starts_with_slash() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.editor.set_text("/");

        let rendered = root.render(80).join("\n");

        assert!(rendered.contains("/help"), "{rendered}");
        assert!(rendered.contains("Show help"), "{rendered}");
        assert!(rendered.contains("/settings"), "{rendered}");
        assert!(rendered.contains("Open settings menu"), "{rendered}");
        assert!(rendered.contains("/model"), "{rendered}");
        assert!(rendered.contains("(1/21)"), "{rendered}");
    }

    #[test]
    fn editor_border_uses_active_theme_style_in_normal_input_state() {
        let theme = pi_tui::TuiTheme::custom(
            "custom",
            pi_tui::ThemePalette {
                accent: pi_tui::Color::Cyan,
                muted: pi_tui::Color::Ansi256(244),
                text: pi_tui::Color::White,
                background: pi_tui::Color::Default,
                error: pi_tui::Color::Red,
                success: pi_tui::Color::Green,
                warning: pi_tui::Color::Yellow,
                path: pi_tui::Color::Cyan,
                input_border: pi_tui::Color::Rgb(10, 20, 30),
                menu_border: pi_tui::Color::Rgb(40, 50, 60),
            },
        );
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        )
        .with_theme(theme);

        assert_eq!(
            root.editor_border_style().fg,
            pi_tui::Color::Rgb(10, 20, 30)
        );

        let rendered = root.render(40);
        let editor_row = rendered
            .iter()
            .position(|line| line.contains("> "))
            .expect("editor row should render");
        assert!(rendered[editor_row - 1].contains("─"), "{rendered:?}");
        assert!(rendered[editor_row + 1].contains("─"), "{rendered:?}");
    }

    #[test]
    fn editor_border_reflects_thinking_level_from_resolved_theme() {
        let resolved = crate::theme::builtin_dark().resolve_colors().unwrap();
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        )
        .with_theme(dark_theme())
        .with_resolved_theme(resolved);

        // thinkingHigh in dark.json -> "#b294bb"
        root.thinking_level = pi_agent_core::ThinkingLevel::High;
        assert_eq!(
            root.editor_border_style().fg,
            pi_tui::Color::Rgb(0xb2, 0x94, 0xbb)
        );

        // thinkingOff in dark.json -> "darkGray" var -> "#505050"
        root.thinking_level = pi_agent_core::ThinkingLevel::Off;
        assert_eq!(
            root.editor_border_style().fg,
            pi_tui::Color::Rgb(0x50, 0x50, 0x50)
        );
    }

    #[test]
    fn settings_menu_uses_menu_theme_border_style() {
        let theme = pi_tui::TuiTheme::custom(
            "custom",
            pi_tui::ThemePalette {
                accent: pi_tui::Color::Cyan,
                muted: pi_tui::Color::Ansi256(244),
                text: pi_tui::Color::White,
                background: pi_tui::Color::Default,
                error: pi_tui::Color::Red,
                success: pi_tui::Color::Green,
                warning: pi_tui::Color::Yellow,
                path: pi_tui::Color::Cyan,
                input_border: pi_tui::Color::Rgb(10, 20, 30),
                menu_border: pi_tui::Color::Rgb(40, 50, 60),
            },
        );
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        )
        .with_theme(theme);

        root.handle_slash_command(ParsedSlashCommand {
            name: "settings".to_string(),
            args: String::new(),
            original: "/settings".to_string(),
        });

        assert!(root.selecting_settings);
        assert_eq!(
            root.editor_border_style().fg,
            pi_tui::Color::Rgb(40, 50, 60)
        );
        let rendered = root.render(60).join("\n");
        assert!(rendered.contains("Settings"), "{rendered}");
        assert!(!rendered.contains("not implemented"), "{rendered}");
    }

    #[test]
    fn settings_menu_renders_theme_and_auto_compaction_items() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );

        root.handle_slash_command(ParsedSlashCommand {
            name: "settings".to_string(),
            args: String::new(),
            original: "/settings".to_string(),
        });

        let rendered = root.render(80).join("\n");
        assert!(rendered.contains("Settings"), "{rendered}");
        assert!(rendered.contains("Theme"), "{rendered}");
        assert!(rendered.contains("Auto compact"), "{rendered}");
        assert!(rendered.contains("Enter/Space to change"), "{rendered}");
    }

    #[test]
    fn settings_menu_cycles_theme_and_reports_settings_update() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );

        root.handle_slash_command(ParsedSlashCommand {
            name: "settings".to_string(),
            args: String::new(),
            original: "/settings".to_string(),
        });
        root.handle_input(&key_event("\r"));

        assert_eq!(root.theme.name, "light");
        assert_eq!(root.settings.theme.as_deref(), Some("light"));
        let updated = root
            .take_settings_update()
            .expect("theme cycle should emit settings update");
        assert_eq!(updated.theme.as_deref(), Some("light"));
        let rendered = root.render(80).join("\n");
        assert!(rendered.contains("Theme"), "{rendered}");
        assert!(rendered.contains("light"), "{rendered}");
    }

    #[test]
    fn settings_menu_transport_cycles_and_reports_update() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        assert_eq!(root.settings.transport, "auto");

        root.handle_slash_command(ParsedSlashCommand {
            name: "settings".to_string(),
            args: String::new(),
            original: "/settings".to_string(),
        });
        // Navigate from theme (item 0) down to transport (item 2)
        root.handle_input(&key_event("\x1b[B"));
        root.handle_input(&key_event("\x1b[B"));
        root.handle_input(&key_event("\r"));

        assert_eq!(root.settings.transport, "sse");
        let updated = root
            .take_settings_update()
            .expect("transport toggle should emit settings update");
        assert_eq!(updated.transport, "sse");
    }

    #[test]
    fn settings_menu_steering_mode_cycles_and_reports_update() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        assert_eq!(root.settings.steering_mode, "one-at-a-time");

        root.handle_slash_command(ParsedSlashCommand {
            name: "settings".to_string(),
            args: String::new(),
            original: "/settings".to_string(),
        });
        // Navigate from theme (0) down to steering_mode (3)
        root.handle_input(&key_event("\x1b[B"));
        root.handle_input(&key_event("\x1b[B"));
        root.handle_input(&key_event("\x1b[B"));
        root.handle_input(&key_event("\r"));

        assert_eq!(root.settings.steering_mode, "all");
        let updated = root
            .take_settings_update()
            .expect("steering mode toggle should emit settings update");
        assert_eq!(updated.steering_mode, "all");
    }

    #[test]
    fn settings_menu_follow_up_mode_cycles_and_reports_update() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        assert_eq!(root.settings.follow_up_mode, "one-at-a-time");

        root.handle_slash_command(ParsedSlashCommand {
            name: "settings".to_string(),
            args: String::new(),
            original: "/settings".to_string(),
        });
        // Navigate from theme (0) down to follow_up_mode (4)
        for _ in 0..4 {
            root.handle_input(&key_event("\x1b[B"));
        }
        root.handle_input(&key_event("\r"));

        assert_eq!(root.settings.follow_up_mode, "all");
        let updated = root
            .take_settings_update()
            .expect("follow-up mode toggle should emit settings update");
        assert_eq!(updated.follow_up_mode, "all");
    }

    #[test]
    fn settings_menu_show_images_toggles_and_reports_update() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        assert!(root.settings.terminal.show_images);

        root.handle_slash_command(ParsedSlashCommand {
            name: "settings".to_string(),
            args: String::new(),
            original: "/settings".to_string(),
        });
        // Navigate from theme (0) down to show_images (5)
        for _ in 0..5 {
            root.handle_input(&key_event("\x1b[B"));
        }
        root.handle_input(&key_event("\r"));

        assert!(!root.settings.terminal.show_images);
        let updated = root
            .take_settings_update()
            .expect("show images toggle should emit settings update");
        assert!(!updated.terminal.show_images);
    }

    #[test]
    fn settings_menu_image_width_cycles_and_reports_update() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        assert_eq!(root.settings.terminal.image_width_cells, 60);

        root.handle_slash_command(ParsedSlashCommand {
            name: "settings".to_string(),
            args: String::new(),
            original: "/settings".to_string(),
        });
        // Navigate from theme (0) down to image_width_cells (6)
        for _ in 0..6 {
            root.handle_input(&key_event("\x1b[B"));
        }
        root.handle_input(&key_event("\r"));

        assert_eq!(root.settings.terminal.image_width_cells, 80);
        let updated = root
            .take_settings_update()
            .expect("image width toggle should emit settings update");
        assert_eq!(updated.terminal.image_width_cells, 80);
        assert_eq!(
            root.settings_delta()
                .terminal
                .as_ref()
                .and_then(|terminal| terminal.image_width_cells),
            Some(80)
        );
    }

    #[test]
    fn settings_menu_show_progress_toggles_and_reports_update() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        assert!(!root.settings.terminal.show_progress);

        root.handle_slash_command(ParsedSlashCommand {
            name: "settings".to_string(),
            args: String::new(),
            original: "/settings".to_string(),
        });
        // Navigate from theme (0) down to show_progress (7)
        for _ in 0..7 {
            root.handle_input(&key_event("\x1b[B"));
        }
        root.handle_input(&key_event("\r"));

        assert!(root.settings.terminal.show_progress);
        let updated = root
            .take_settings_update()
            .expect("show progress toggle should emit settings update");
        assert!(updated.terminal.show_progress);
    }

    #[test]
    fn settings_menu_auto_resize_images_toggles_and_reports_update() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        assert!(root.settings.terminal.auto_resize_images);

        root.handle_slash_command(ParsedSlashCommand {
            name: "settings".to_string(),
            args: String::new(),
            original: "/settings".to_string(),
        });
        for _ in 0..8 {
            root.handle_input(&key_event("\x1b[B"));
        }
        root.handle_input(&key_event("\r"));

        assert!(!root.settings.terminal.auto_resize_images);
        let updated = root
            .take_settings_update()
            .expect("auto resize images toggle should emit settings update");
        assert!(!updated.terminal.auto_resize_images);
    }

    #[test]
    fn settings_menu_block_images_toggles_and_reports_update() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        assert!(!root.settings.terminal.block_images);

        root.handle_slash_command(ParsedSlashCommand {
            name: "settings".to_string(),
            args: String::new(),
            original: "/settings".to_string(),
        });
        for _ in 0..9 {
            root.handle_input(&key_event("\x1b[B"));
        }
        root.handle_input(&key_event("\r"));

        assert!(root.settings.terminal.block_images);
        let updated = root
            .take_settings_update()
            .expect("block images toggle should emit settings update");
        assert!(updated.terminal.block_images);
    }

    #[test]
    fn settings_menu_skill_commands_toggles_and_reports_update() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        assert!(root.settings.enable_skill_commands);

        root.handle_slash_command(ParsedSlashCommand {
            name: "settings".to_string(),
            args: String::new(),
            original: "/settings".to_string(),
        });
        for _ in 0..10 {
            root.handle_input(&key_event("\x1b[B"));
        }
        root.handle_input(&key_event("\r"));

        assert!(!root.settings.enable_skill_commands);
        let updated = root
            .take_settings_update()
            .expect("skill commands toggle should emit settings update");
        assert!(!updated.enable_skill_commands);
    }

    #[test]
    fn settings_menu_hide_thinking_toggles_and_reports_update() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        assert!(!root.settings.hide_thinking_block);

        root.handle_slash_command(ParsedSlashCommand {
            name: "settings".to_string(),
            args: String::new(),
            original: "/settings".to_string(),
        });
        for _ in 0..11 {
            root.handle_input(&key_event("\x1b[B"));
        }
        root.handle_input(&key_event("\r"));

        assert!(root.settings.hide_thinking_block);
        let updated = root
            .take_settings_update()
            .expect("hide thinking toggle should emit settings update");
        assert!(updated.hide_thinking_block);
    }

    #[test]
    fn settings_menu_collapse_changelog_toggles_and_reports_update() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        assert!(!root.settings.collapse_changelog);

        root.handle_slash_command(ParsedSlashCommand {
            name: "settings".to_string(),
            args: String::new(),
            original: "/settings".to_string(),
        });
        for _ in 0..12 {
            root.handle_input(&key_event("\x1b[B"));
        }
        root.handle_input(&key_event("\r"));

        assert!(root.settings.collapse_changelog);
        let updated = root
            .take_settings_update()
            .expect("collapse changelog toggle should emit settings update");
        assert!(updated.collapse_changelog);
    }

    #[test]
    fn settings_menu_quiet_startup_toggles_and_reports_update() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        assert!(!root.settings.quiet_startup);

        root.handle_slash_command(ParsedSlashCommand {
            name: "settings".to_string(),
            args: String::new(),
            original: "/settings".to_string(),
        });
        for _ in 0..13 {
            root.handle_input(&key_event("\x1b[B"));
        }
        root.handle_input(&key_event("\r"));

        assert!(root.settings.quiet_startup);
        let updated = root
            .take_settings_update()
            .expect("quiet startup toggle should emit settings update");
        assert!(updated.quiet_startup);
    }

    #[test]
    fn settings_menu_clear_on_shrink_toggles_and_reports_update() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        assert!(!root.settings.terminal.clear_on_shrink);

        root.handle_slash_command(ParsedSlashCommand {
            name: "settings".to_string(),
            args: String::new(),
            original: "/settings".to_string(),
        });
        for _ in 0..14 {
            root.handle_input(&key_event("\x1b[B"));
        }
        root.handle_input(&key_event("\r"));

        assert!(root.settings.terminal.clear_on_shrink);
        let updated = root
            .take_settings_update()
            .expect("clear on shrink toggle should emit settings update");
        assert!(updated.terminal.clear_on_shrink);
    }

    #[test]
    fn settings_menu_double_escape_action_cycles_and_reports_update() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        assert_eq!(root.settings.double_escape_action, "tree");

        root.handle_slash_command(ParsedSlashCommand {
            name: "settings".to_string(),
            args: String::new(),
            original: "/settings".to_string(),
        });
        for _ in 0..15 {
            root.handle_input(&key_event("\x1b[B"));
        }
        root.handle_input(&key_event("\r"));

        assert_eq!(root.settings.double_escape_action, "fork");
        let updated = root
            .take_settings_update()
            .expect("double escape toggle should emit settings update");
        assert_eq!(updated.double_escape_action, "fork");
    }

    #[test]
    fn settings_menu_warnings_toggles_and_reports_update() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        assert!(root.settings.warnings.anthropic_extra_usage);

        root.handle_slash_command(ParsedSlashCommand {
            name: "settings".to_string(),
            args: String::new(),
            original: "/settings".to_string(),
        });
        for _ in 0..16 {
            root.handle_input(&key_event("\x1b[B"));
        }
        root.handle_input(&key_event("\r"));

        assert!(!root.settings.warnings.anthropic_extra_usage);
        let updated = root
            .take_settings_update()
            .expect("warnings toggle should emit settings update");
        assert!(!updated.warnings.anthropic_extra_usage);
    }

    #[test]
    fn settings_menu_thinking_level_cycles_and_reports_update() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        assert!(root.settings.default_thinking_level.is_none());
        assert_eq!(root.thinking_level, pi_agent_core::ThinkingLevel::Off);

        root.handle_slash_command(ParsedSlashCommand {
            name: "settings".to_string(),
            args: String::new(),
            original: "/settings".to_string(),
        });
        for _ in 0..17 {
            root.handle_input(&key_event("\x1b[B"));
        }
        root.handle_input(&key_event("\r"));

        assert_eq!(
            root.settings.default_thinking_level.as_deref(),
            Some("minimal")
        );
        assert_eq!(root.thinking_level, pi_agent_core::ThinkingLevel::Minimal);
        assert_eq!(
            root.take_selected_thinking_level(),
            Some(pi_agent_core::ThinkingLevel::Minimal)
        );
        let updated = root
            .take_settings_update()
            .expect("thinking level toggle should emit settings update");
        assert_eq!(updated.default_thinking_level.as_deref(), Some("minimal"));
    }

    #[test]
    fn settings_menu_http_idle_timeout_cycles_and_reports_update() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        assert_eq!(root.settings.http_idle_timeout_ms, 300_000);

        root.handle_slash_command(ParsedSlashCommand {
            name: "settings".to_string(),
            args: String::new(),
            original: "/settings".to_string(),
        });
        for _ in 0..18 {
            root.handle_input(&key_event("\x1b[B"));
        }
        root.handle_input(&key_event("\r"));

        assert_eq!(root.settings.http_idle_timeout_ms, 0);
        let updated = root
            .take_settings_update()
            .expect("HTTP idle timeout toggle should emit settings update");
        assert_eq!(updated.http_idle_timeout_ms, 0);
        assert_eq!(root.settings_delta().http_idle_timeout_ms, Some(0));
    }

    #[test]
    fn settings_menu_toggles_auto_compaction_and_reports_settings_update() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        assert!(root.settings.compaction.enabled);

        root.handle_slash_command(ParsedSlashCommand {
            name: "settings".to_string(),
            args: String::new(),
            original: "/settings".to_string(),
        });
        root.handle_input(&key_event("\x1b[B"));
        root.handle_input(&key_event("\r"));

        assert!(!root.settings.compaction.enabled);
        let updated = root
            .take_settings_update()
            .expect("auto compact toggle should emit settings update");
        assert!(!updated.compaction.enabled);
        let rendered = root.render(80).join("\n");
        assert!(rendered.contains("Auto compact"), "{rendered}");
        assert!(rendered.contains("off"), "{rendered}");
    }

    #[test]
    fn settings_menu_tracks_changes_in_settings_delta() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        // Initially empty delta (no changes tracked yet)
        {
            let delta = root.settings_delta();
            assert!(delta.theme.is_none());
            assert!(delta.compaction.is_none());
            assert!(delta.transport.is_none());
        }

        root.handle_slash_command(ParsedSlashCommand {
            name: "settings".to_string(),
            args: String::new(),
            original: "/settings".to_string(),
        });
        // Cycle theme (Enter on first item)
        root.handle_input(&key_event("\r"));

        // Verify delta tracks the theme change
        {
            let delta = root.settings_delta();
            assert_eq!(delta.theme.as_deref(), Some("light"), "delta tracks theme");
        }

        // Cycle auto_compaction
        root.handle_input(&key_event("\x1b[B")); // down to auto_compaction
        root.handle_input(&key_event("\r"));

        {
            let delta = root.settings_delta();
            assert_eq!(
                delta.theme.as_deref(),
                Some("light"),
                "delta still has theme"
            );
            let compaction = delta.compaction.as_ref().expect("compaction delta");
            assert_eq!(compaction.enabled, Some(false), "delta tracks compaction");
        }
    }

    #[test]
    fn login_command_saves_provider_api_key_and_updates_auth_state() {
        let _guard = crate::test_support::env_lock();
        let dir = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("PI_RUST_DIR", dir.path());
        }
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "claude-haiku-4-5".to_string(),
            "no-session".to_string(),
        );

        root.handle_slash_command(ParsedSlashCommand {
            name: "login".to_string(),
            args: "anthropic sk-test-login".to_string(),
            original: "/login anthropic sk-test-login".to_string(),
        });

        assert_eq!(root.auth.api_key_entry("anthropic"), Some("sk-test-login"));
        let text = std::fs::read_to_string(dir.path().join("auth.toml")).unwrap();
        assert!(text.contains("[anthropic]"), "{text}");
        assert!(text.contains("sk-test-login"), "{text}");
        assert!(last_system_text(&root).contains("Saved API key for anthropic"));
        assert_ne!(root.action, InteractiveAction::Submit);
        assert!(root.pending_submit.is_none());

        unsafe {
            std::env::remove_var("PI_RUST_DIR");
        }
    }

    #[test]
    fn logout_command_removes_provider_auth_entry_and_updates_auth_state() {
        let _guard = crate::test_support::env_lock();
        let dir = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("PI_RUST_DIR", dir.path());
        }
        let auth_path = dir.path().join("auth.toml");
        std::fs::write(
            &auth_path,
            "[anthropic]\ntype = \"api_key\"\nkey = \"sk-test-login\"\n",
        )
        .unwrap();
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "claude-haiku-4-5".to_string(),
            "no-session".to_string(),
        );
        root.auth.set_api_key("anthropic", "sk-test-login");

        root.handle_slash_command(ParsedSlashCommand {
            name: "logout".to_string(),
            args: "anthropic".to_string(),
            original: "/logout anthropic".to_string(),
        });

        assert_eq!(root.auth.api_key_entry("anthropic"), None);
        let text = std::fs::read_to_string(&auth_path).unwrap();
        assert!(!text.contains("[anthropic]"), "{text}");
        assert!(last_system_text(&root).contains("Removed stored auth for anthropic"));
        assert_ne!(root.action, InteractiveAction::Submit);
        assert!(root.pending_submit.is_none());

        unsafe {
            std::env::remove_var("PI_RUST_DIR");
        }
    }

    #[test]
    fn slash_suggestions_filter_and_hide_after_arguments() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.editor.set_text("/mo");
        let filtered = root.render(80).join("\n");
        assert!(filtered.contains("model"), "{filtered}");
        assert!(!filtered.contains("settings"), "{filtered}");

        root.editor.set_text("/model ");
        let with_argument_space = root.render(80).join("\n");
        assert!(
            !with_argument_space.contains("Select model"),
            "{with_argument_space}"
        );
        assert!(
            !with_argument_space.contains("(1/"),
            "{with_argument_space}"
        );
    }

    #[test]
    fn slash_suggestions_can_be_selected_and_accepted() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.editor.set_text("/");
        root.handle_input(&key_event("\x1b[B"));
        root.handle_input(&key_event("\x1b[B"));
        let moved = root.render(80).join("\n");
        assert!(moved.contains("(3/21)"), "{moved}");

        root.handle_input(&key_event("\t"));

        assert_eq!(root.editor.text(), "/model ");
        assert_eq!(root.take_action(), InteractiveAction::None);
        let rendered = root.render(80).join("\n");
        assert!(!rendered.contains("(2/21)"), "{rendered}");
    }

    #[test]
    fn slash_suggestions_can_be_cancelled_for_current_query() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.editor.set_text("/");

        root.handle_input(&key_event("\x1b"));
        let cancelled = root.render(80).join("\n");
        assert!(!cancelled.contains("Open settings menu"), "{cancelled}");

        root.handle_input(&key_event("m"));
        let changed = root.render(80).join("\n");
        assert!(changed.contains("model"), "{changed}");
    }

    #[test]
    fn ctrl_p_cycles_models_from_rotation() {
        let rotation = vec![
            pi_ai::lookup_model("claude-haiku-4-5").unwrap(),
            pi_ai::lookup_model("gpt-5").unwrap(),
            pi_ai::lookup_model("gpt-5-mini").unwrap(),
        ];
        let mut root = InteractiveRoot::new_with_theme_and_models(
            PathBuf::from("."),
            "claude-haiku-4-5".to_string(),
            "no-session".to_string(),
            dark_theme(),
            rotation.clone(),
        );
        root.model_rotation = rotation;

        root.handle_input(&ctrl_p_event(false));
        assert_eq!(root.model_id, "gpt-5");
        assert_eq!(root.take_selected_model().unwrap().id, "gpt-5");

        root.handle_input(&ctrl_p_event(false));
        assert_eq!(root.model_id, "gpt-5-mini");

        root.handle_input(&ctrl_p_event(true));
        assert_eq!(root.model_id, "gpt-5");
    }

    #[test]
    fn resume_command_opens_session_selector_and_selects_session() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("/tmp/project"),
            "faux-model".to_string(),
            "session".to_string(),
        );
        root.session_choices = vec![SessionChoice {
            id: "session-alpha".to_string(),
            cwd: "/tmp/project".to_string(),
            path: PathBuf::from("/tmp/sessions/session-alpha"),
            created_at: "2026-06-20T00:00:00Z".to_string(),
            name: Some("Project Alpha".to_string()),
            entry_count: 3,
            active_leaf_id: Some("leaf-alpha".to_string()),
            kind: SessionChoiceKind::RustNative,
        }];

        root.handle_slash_command(ParsedSlashCommand {
            name: "resume".to_string(),
            args: String::new(),
            original: "/resume".to_string(),
        });

        assert!(root.selecting_session);
        let rendered = root.render(80).join("\n");
        assert!(rendered.contains("Select session"), "{rendered}");
        assert!(rendered.contains("Project Alpha"), "{rendered}");
        assert!(rendered.contains("session-alpha"), "{rendered}");

        root.handle_input(&key_event("\r"));

        let selected = root
            .take_selected_session()
            .expect("session selection should be returned to loop");
        assert_eq!(selected.id, "session-alpha");
        assert_eq!(selected.path, PathBuf::from("/tmp/sessions/session-alpha"));
        assert!(!root.selecting_session);
    }

    #[test]
    fn session_selector_filters_by_name_and_cwd() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("/tmp/project"),
            "faux-model".to_string(),
            "session".to_string(),
        );
        root.session_choices = vec![
            SessionChoice {
                id: "session-alpha".to_string(),
                cwd: "/tmp/project".to_string(),
                path: PathBuf::from("/tmp/sessions/session-alpha"),
                created_at: "2026-06-20T00:00:00Z".to_string(),
                name: Some("Project Alpha".to_string()),
                entry_count: 3,
                active_leaf_id: Some("leaf-alpha".to_string()),
                kind: SessionChoiceKind::RustNative,
            },
            SessionChoice {
                id: "session-beta".to_string(),
                cwd: "/tmp/other".to_string(),
                path: PathBuf::from("/tmp/sessions/session-beta"),
                created_at: "2026-06-21T00:00:00Z".to_string(),
                name: Some("Beta Tools".to_string()),
                entry_count: 8,
                active_leaf_id: Some("leaf-beta".to_string()),
                kind: SessionChoiceKind::RustNative,
            },
        ];
        root.handle_slash_command(ParsedSlashCommand {
            name: "resume".to_string(),
            args: String::new(),
            original: "/resume".to_string(),
        });

        root.handle_input(&key_event("B"));

        let rendered = root.render(80).join("\n");
        assert!(rendered.contains("Beta Tools"), "{rendered}");
        assert!(rendered.contains("/tmp/other"), "{rendered}");
        assert!(!rendered.contains("Project Alpha"), "{rendered}");
    }

    #[test]
    fn model_command_accepts_thinking_suffix() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "claude-haiku-4-5".to_string(),
            "no-session".to_string(),
        );

        root.handle_slash_command(ParsedSlashCommand {
            name: "model".to_string(),
            args: "gpt-5:high".to_string(),
            original: "/model gpt-5:high".to_string(),
        });

        assert_eq!(root.model_id, "gpt-5");
        assert_eq!(root.take_selected_model().unwrap().id, "gpt-5");
        assert_eq!(
            root.take_selected_thinking_level(),
            Some(pi_agent_core::ThinkingLevel::High)
        );
    }

    #[test]
    fn render_state_changes_when_slash_suggestion_selection_changes() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.editor.set_text("/");

        let before = root.render_state();
        root.handle_input(&key_event("\x1b[B"));

        assert_ne!(root.render_state(), before);
    }

    #[test]
    fn exact_slash_command_enter_submits_instead_of_accepting_suggestion() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.editor.set_text("/quit");

        root.handle_input(&key_event("\r"));

        assert_eq!(root.take_action(), InteractiveAction::Exit);
    }

    #[test]
    fn submitted_prompt_is_added_to_editor_history() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.editor.set_text("hello history");

        root.handle_input(&key_event("\r"));
        assert_eq!(root.take_action(), InteractiveAction::Submit);
        assert_eq!(root.take_pending_submit().as_deref(), Some("hello history"));

        root.handle_input(&key_event("\x1b[A"));

        assert_eq!(root.editor.text(), "hello history");
    }

    #[test]
    fn parse_slash_command_returns_command_name_and_arguments() {
        assert_eq!(
            parse_slash_command("/model gpt-5"),
            Some(ParsedSlashCommand {
                name: "model".to_string(),
                args: "gpt-5".to_string(),
                original: "/model gpt-5".to_string(),
            })
        );
        assert_eq!(
            parse_slash_command("/NAME Project Phoenix"),
            Some(ParsedSlashCommand {
                name: "name".to_string(),
                args: "Project Phoenix".to_string(),
                original: "/NAME Project Phoenix".to_string(),
            })
        );
    }

    #[test]
    fn parse_slash_command_preserves_non_slash_prompt_path() {
        assert_eq!(parse_slash_command("hello"), None);
        assert_eq!(parse_slash_command("  /quit"), None);
    }

    #[test]
    fn help_text_lists_all_builtin_commands() {
        let help = help_text();
        for command in builtin_slash_commands() {
            assert!(
                help.contains(&format!("/{}", command.name)),
                "help text should list /{}: {help}",
                command.name
            );
        }
    }

    #[test]
    fn handle_slash_command_quit_sets_exit_when_idle() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.handle_slash_command(ParsedSlashCommand {
            name: "quit".to_string(),
            args: String::new(),
            original: "/quit".to_string(),
        });
        assert_eq!(root.action, InteractiveAction::Exit);
    }

    #[test]
    fn handle_slash_command_quit_sets_abort_when_running() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.set_status(InteractiveStatus::Running);
        root.handle_slash_command(ParsedSlashCommand {
            name: "quit".to_string(),
            args: String::new(),
            original: "/quit".to_string(),
        });
        assert_eq!(root.action, InteractiveAction::AbortRunning);
    }

    #[test]
    fn handle_slash_command_help_pushes_system_item() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.handle_slash_command(ParsedSlashCommand {
            name: "help".to_string(),
            args: String::new(),
            original: "/help".to_string(),
        });
        let text = last_system_text(&root);
        assert!(text.contains("/model"), "{text}");
        assert!(text.contains("/reload"), "{text}");
        assert_ne!(root.action, InteractiveAction::Submit);
        assert!(root.pending_submit.is_none());
    }

    #[test]
    fn handle_known_pending_command_reports_not_implemented_without_submit() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.handle_slash_command(ParsedSlashCommand {
            name: "scoped-models".to_string(),
            args: String::new(),
            original: "/scoped-models".to_string(),
        });
        let text = last_system_text(&root);
        assert!(text.contains("/scoped-models"), "{text}");
        assert!(text.contains("not implemented"), "{text}");
        assert_ne!(root.action, InteractiveAction::Submit);
        assert!(root.pending_submit.is_none());
    }

    #[test]
    fn copy_command_copies_last_assistant_message_to_clipboard() {
        let clipboard = Arc::new(TestClipboard::default());
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        )
        .with_clipboard(clipboard.clone());
        root.apply_events(vec![
            UiEvent::AssistantDelta {
                text: "first answer".to_string(),
            },
            UiEvent::AssistantDone,
            UiEvent::AssistantDelta {
                text: "second answer".to_string(),
            },
            UiEvent::AssistantDone,
        ]);

        root.handle_slash_command(ParsedSlashCommand {
            name: "copy".to_string(),
            args: String::new(),
            original: "/copy".to_string(),
        });

        assert_eq!(clipboard.last_text(), Some("second answer".to_string()));
        let text = last_system_text(&root);
        assert!(
            text.contains("Copied last agent message to clipboard"),
            "{text}"
        );
        assert!(!text.contains("not implemented"), "{text}");
        assert_ne!(root.action, InteractiveAction::Submit);
    }

    #[test]
    fn copy_command_reports_error_when_no_assistant_message_exists() {
        let clipboard = Arc::new(TestClipboard::default());
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        )
        .with_clipboard(clipboard.clone());

        root.handle_slash_command(ParsedSlashCommand {
            name: "copy".to_string(),
            args: String::new(),
            original: "/copy".to_string(),
        });

        assert_eq!(clipboard.last_text(), None);
        let text = last_system_text(&root);
        assert!(text.contains("No agent messages to copy yet."), "{text}");
        assert!(!text.contains("not implemented"), "{text}");
    }

    #[test]
    fn export_command_rejects_jsonl_path() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("session-export.jsonl");
        let mut root = InteractiveRoot::new(
            dir.path().to_path_buf(),
            "faux-model".to_string(),
            "Project Phoenix".to_string(),
        );
        root.push_user("hello".to_string());
        root.apply_events(vec![
            UiEvent::AssistantDelta {
                text: "world".to_string(),
            },
            UiEvent::AssistantDone,
        ]);

        root.handle_slash_command(ParsedSlashCommand {
            name: "export".to_string(),
            args: output.display().to_string(),
            original: format!("/export {}", output.display()),
        });

        assert!(!output.exists());
        let status = last_system_text(&root);
        assert!(
            status
                .contains("Failed to export session: JSONL session export is no longer supported"),
            "{status}"
        );
        assert!(!status.contains("not implemented"), "{status}");
    }

    #[test]
    fn export_command_writes_html_when_path_ends_with_html() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("session-export.html");
        let mut root = InteractiveRoot::new(
            dir.path().to_path_buf(),
            "faux-model".to_string(),
            "Project Phoenix".to_string(),
        );
        root.push_user("hello <user>".to_string());
        root.apply_events(vec![
            UiEvent::AssistantDelta {
                text: "world <assistant>".to_string(),
            },
            UiEvent::AssistantDone,
        ]);

        root.handle_slash_command(ParsedSlashCommand {
            name: "export".to_string(),
            args: output.display().to_string(),
            original: format!("/export {}", output.display()),
        });

        let text = std::fs::read_to_string(&output).unwrap();
        assert!(text.contains("<!doctype html>"), "{text}");
        assert!(text.contains("hello &lt;user&gt;"), "{text}");
        assert!(text.contains("world &lt;assistant&gt;"), "{text}");
        let status = last_system_text(&root);
        assert!(status.contains("Session exported to:"), "{status}");
    }

    #[test]
    fn new_command_clears_ui_state_and_requests_new_session() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "Project Phoenix".to_string(),
        );
        root.stats = FooterStats {
            input: 123,
            output: 456,
            ..Default::default()
        };
        root.push_user("old prompt".to_string());
        root.apply_events(vec![
            UiEvent::AssistantDelta {
                text: "old response".to_string(),
            },
            UiEvent::AssistantDone,
        ]);

        root.handle_slash_command(ParsedSlashCommand {
            name: "new".to_string(),
            args: String::new(),
            original: "/new".to_string(),
        });

        assert_eq!(root.action, InteractiveAction::NewSession);
        assert_eq!(root.stats, FooterStats::default());
        let rendered = root.render(80).join("\n");
        assert!(rendered.contains("New session started"), "{rendered}");
        assert!(!rendered.contains("old prompt"), "{rendered}");
        assert!(!rendered.contains("old response"), "{rendered}");
    }

    #[test]
    fn reload_command_requests_resource_reload() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );

        root.handle_slash_command(ParsedSlashCommand {
            name: "reload".to_string(),
            args: String::new(),
            original: "/reload".to_string(),
        });

        assert_eq!(root.action, InteractiveAction::ReloadResources);
        let text = last_system_text(&root);
        assert!(
            text.contains("Reloading keybindings and resources"),
            "{text}"
        );
        assert!(!text.contains("not implemented"), "{text}");
    }

    #[test]
    fn import_command_rejects_jsonl_session_paths() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("legacy-session.jsonl");
        std::fs::write(&source, "{}").unwrap();
        let mut root = InteractiveRoot::new(
            dir.path().to_path_buf(),
            "faux-model".to_string(),
            "session".to_string(),
        );

        root.handle_slash_command(ParsedSlashCommand {
            name: "import".to_string(),
            args: format!("\"{}\"", source.display()),
            original: format!("/import \"{}\"", source.display()),
        });

        assert!(root.take_selected_session().is_none());
        assert!(root.active_session_path.is_none());
        assert!(root.active_leaf_id.is_none());
        let text = last_system_text(&root);
        assert!(
            text.contains("JSONL session import is no longer supported."),
            "{text}"
        );
        assert!(!text.contains("not implemented"), "{text}");
    }

    #[test]
    fn import_command_reports_unsupported_without_path() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "session".to_string(),
        );

        root.handle_slash_command(ParsedSlashCommand {
            name: "import".to_string(),
            args: String::new(),
            original: "/import".to_string(),
        });

        assert!(root.take_selected_session().is_none());
        let text = last_system_text(&root);
        assert!(
            text.contains("JSONL session import is no longer supported."),
            "{text}"
        );
        assert!(!text.contains("not implemented"), "{text}");
    }

    #[test]
    fn clone_command_ignores_legacy_jsonl_active_path() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("legacy-session.jsonl");
        std::fs::write(&source, "{}").unwrap();
        let mut root = InteractiveRoot::new(
            dir.path().to_path_buf(),
            "faux-model".to_string(),
            "session".to_string(),
        );
        root.active_session_path = Some(source);
        root.active_leaf_id = Some("legacy-leaf".to_string());

        root.handle_slash_command(ParsedSlashCommand {
            name: "clone".to_string(),
            args: String::new(),
            original: "/clone".to_string(),
        });

        assert!(root.take_selected_session().is_none());
        let text = last_system_text(&root);
        assert!(text.contains("Nothing to clone yet"), "{text}");
        assert!(!text.contains("not implemented"), "{text}");
    }

    #[test]
    fn clone_command_reports_status_when_no_active_session_exists() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "session".to_string(),
        );

        root.handle_slash_command(ParsedSlashCommand {
            name: "clone".to_string(),
            args: String::new(),
            original: "/clone".to_string(),
        });

        assert!(root.take_selected_session().is_none());
        let text = last_system_text(&root);
        assert!(text.contains("Nothing to clone yet"), "{text}");
        assert!(!text.contains("not implemented"), "{text}");
    }

    #[test]
    fn fork_command_ignores_legacy_jsonl_active_path() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("legacy-session.jsonl");
        std::fs::write(&source, "{}").unwrap();
        let mut root = InteractiveRoot::new(
            dir.path().to_path_buf(),
            "faux-model".to_string(),
            "session".to_string(),
        );
        root.active_session_path = Some(source);
        root.active_leaf_id = Some("legacy-leaf".to_string());

        root.handle_slash_command(ParsedSlashCommand {
            name: "fork".to_string(),
            args: String::new(),
            original: "/fork".to_string(),
        });

        assert!(root.take_selected_session().is_none());
        let text = last_system_text(&root);
        assert!(text.contains("Nothing to fork yet"), "{text}");
        assert!(!text.contains("not implemented"), "{text}");
    }

    #[test]
    fn double_escape_tree_action_no_session() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "session".to_string(),
        );

        root.handle_input(&key_event("\x1b"));
        root.handle_input(&key_event("\x1b"));

        // No active session, so tree should show "No entries in session"
        let text = last_system_text(&root);
        assert!(
            text.contains("No entries in session"),
            "expected 'No entries in session' but got: {text}"
        );
    }

    #[test]
    fn tree_selector_down_key_changes_render_state() {
        let temp = tempfile::tempdir().unwrap();
        let cwd = temp.path().join("project");
        std::fs::create_dir_all(&cwd).unwrap();
        let session_path = write_rust_native_session(temp.path(), &cwd, "hello tree");
        let mut root =
            InteractiveRoot::new(cwd.clone(), "faux-model".to_string(), "session".to_string());
        root.set_active_session_choice(SessionChoice {
            id: "sess_tree_fixture".to_string(),
            cwd: cwd.display().to_string(),
            path: session_path,
            created_at: "2026-06-30T00:00:00Z".to_string(),
            name: None,
            entry_count: 2,
            active_leaf_id: Some("leaf_1".to_string()),
            kind: SessionChoiceKind::RustNative,
        });

        root.handle_slash_command(ParsedSlashCommand {
            name: "tree".to_string(),
            args: String::new(),
            original: "/tree".to_string(),
        });

        assert!(root.selecting_tree);
        let before = root.render_state();
        root.handle_input(&key_event("\x1b[B"));
        let after = root.render_state();

        assert_ne!(
            before, after,
            "tree selection movement must be visible to the render scheduler"
        );
    }

    #[test]
    fn double_escape_none_action_does_nothing() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "session".to_string(),
        );
        root.settings.double_escape_action = "none".to_string();
        let initial_len = root.transcript.items().len();

        root.handle_input(&key_event("\x1b"));
        root.handle_input(&key_event("\x1b"));

        assert_eq!(root.transcript.items().len(), initial_len);
        assert!(root.take_selected_session().is_none());
    }

    #[test]
    fn double_escape_fork_action_ignores_legacy_jsonl_active_path() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("legacy-session.jsonl");
        std::fs::write(&source, "{}").unwrap();
        let mut root = InteractiveRoot::new(
            dir.path().to_path_buf(),
            "faux-model".to_string(),
            "session".to_string(),
        );
        root.settings.double_escape_action = "fork".to_string();
        root.active_session_path = Some(source);
        root.active_leaf_id = Some("legacy-leaf".to_string());

        root.handle_input(&key_event("\x1b"));
        root.handle_input(&key_event("\x1b"));

        assert!(root.take_selected_session().is_none());
        let text = last_system_text(&root);
        assert!(text.contains("Nothing to fork yet"), "{text}");
    }

    #[test]
    fn handle_unknown_slash_command_reports_error_without_submit() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.handle_slash_command(ParsedSlashCommand {
            name: "does-not-exist".to_string(),
            args: String::new(),
            original: "/does-not-exist".to_string(),
        });
        let text = last_system_text(&root);
        assert!(text.contains("unknown command: /does-not-exist"), "{text}");
        assert_ne!(root.action, InteractiveAction::Submit);
        assert!(root.pending_submit.is_none());
    }

    // ── expand_skill_command ──────────────────────────────────────────

    #[test]
    fn expand_skill_command_expands_to_xml_block() {
        let skills = vec![pi_agent_core::Skill {
            name: "rust".into(),
            description: "Rust".into(),
            location: "/skills/rust/SKILL.md".into(),
            content: "Rust programming guide".into(),
            disable_model_invocation: false,
        }];

        let result = crate::interactive::commands::expand_skill_command(
            "/skill:rust write a function",
            &skills,
        );
        assert!(result.contains("<skill name=\"rust\""), "{result}");
        assert!(result.contains("Rust programming guide"), "{result}");
        assert!(result.contains("write a function"), "{result}");
    }

    #[test]
    fn expand_skill_command_unknown_passes_through() {
        let result =
            crate::interactive::commands::expand_skill_command("/skill:unknown do something", &[]);
        assert_eq!(result, "/skill:unknown do something");
    }

    #[test]
    fn expand_skill_command_non_skill_passes_through() {
        let result = crate::interactive::commands::expand_skill_command("/help", &[]);
        assert_eq!(result, "/help");
    }

    // ── expand_prompt_template ────────────────────────────────────────

    #[test]
    fn expand_prompt_template_substitutes_args() {
        let templates = vec![pi_agent_core::PromptTemplate {
            name: "review".into(),
            description: "Review".into(),
            content: "Review $1 and $ARGUMENTS".into(),
            location: "/prompts/review.md".into(),
        }];

        let result =
            crate::interactive::commands::expand_prompt_template("/review code tests", &templates);
        assert_eq!(result, "Review code and code tests");
    }

    #[test]
    fn expand_prompt_template_unknown_passes_through() {
        let result = crate::interactive::commands::expand_prompt_template("/unknown blah", &[]);
        assert_eq!(result, "/unknown blah");
    }

    #[test]
    fn expand_prompt_template_non_slash_passes_through() {
        let result = crate::interactive::commands::expand_prompt_template("just text", &[]);
        assert_eq!(result, "just text");
    }

    // ── expansion in handle_slash_command catch-all ───────────────────

    #[test]
    fn handle_unknown_slash_command_with_template_expands_and_submits() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.prompt_templates = vec![pi_agent_core::PromptTemplate {
            name: "review".into(),
            description: "Review".into(),
            content: "Review $ARGUMENTS".into(),
            location: "/prompts/review.md".into(),
        }];

        root.handle_slash_command(ParsedSlashCommand {
            name: "review".to_string(),
            args: "code tests".to_string(),
            original: "/review code tests".to_string(),
        });

        assert_eq!(
            root.action,
            InteractiveAction::Submit,
            "template expansion should trigger submit"
        );
        assert_eq!(
            root.pending_submit.as_deref(),
            Some("Review code tests"),
            "pending_submit should contain expanded template"
        );
    }

    #[test]
    fn handle_unknown_slash_command_with_skill_expands_and_submits() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.skills = vec![pi_agent_core::Skill {
            name: "rust".into(),
            description: "Rust".into(),
            location: "/skills/rust/SKILL.md".into(),
            content: "Rust programming guide".into(),
            disable_model_invocation: false,
        }];

        root.handle_slash_command(ParsedSlashCommand {
            name: "skill:rust".to_string(),
            args: "write a fn".to_string(),
            original: "/skill:rust write a fn".to_string(),
        });

        assert_eq!(
            root.action,
            InteractiveAction::Submit,
            "skill expansion should trigger submit"
        );
        let expanded = root.pending_submit.expect("has pending submit");
        assert!(expanded.contains("<skill name=\"rust\""), "{expanded}");
        assert!(expanded.contains("write a fn"), "{expanded}");
    }

    #[test]
    fn handle_builtin_slash_command_still_works_with_templates_loaded() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        // Even with templates loaded, builtin commands take priority
        root.prompt_templates = vec![pi_agent_core::PromptTemplate {
            name: "help".into(),
            description: "custom help".into(),
            content: "custom content".into(),
            location: "/prompts/help.md".into(),
        }];

        root.handle_slash_command(ParsedSlashCommand {
            name: "help".to_string(),
            args: String::new(),
            original: "/help".to_string(),
        });

        // Should still show builtin help, not submit custom content
        assert_ne!(root.action, InteractiveAction::Submit);
        assert!(root.pending_submit.is_none());
        let text = last_system_text(&root);
        assert!(text.contains("/reload"), "{text}");
    }

    // ── all_slash_commands ────────────────────────────────────────────

    #[test]
    fn all_slash_commands_includes_templates_and_skills() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );

        // Default (no templates/skills): only builtin commands
        let commands = root.all_slash_commands();
        assert!(commands.iter().any(|c| c.name == "help"));
        assert!(commands.iter().any(|c| c.name == "quit"));

        // With templates loaded
        root.prompt_templates = vec![pi_agent_core::PromptTemplate {
            name: "review".into(),
            description: "Review code".into(),
            content: "content".into(),
            location: "/p/review.md".into(),
        }];
        root.skills = vec![pi_agent_core::Skill {
            name: "rust".into(),
            description: "Rust".into(),
            location: "/s/rust".into(),
            content: "content".into(),
            disable_model_invocation: false,
        }];
        let commands = root.all_slash_commands();
        assert!(commands.iter().any(|c| c.name == "review"));
        assert!(commands.iter().any(|c| c.name == "skill:rust"));
    }

    #[test]
    fn all_slash_commands_excludes_skills_when_disabled() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.settings.enable_skill_commands = false;
        root.prompt_templates = vec![pi_agent_core::PromptTemplate {
            name: "review".into(),
            description: "Review code".into(),
            content: "content".into(),
            location: "/p/review.md".into(),
        }];
        root.skills = vec![pi_agent_core::Skill {
            name: "rust".into(),
            description: "Rust".into(),
            location: "/s/rust".into(),
            content: "content".into(),
            disable_model_invocation: false,
        }];

        let commands = root.all_slash_commands();
        assert!(commands.iter().any(|c| c.name == "review"));
        assert!(commands.iter().any(|c| c.name == "help"));
        assert!(
            !commands.iter().any(|c| c.name == "skill:rust"),
            "skill commands should be excluded when enable_skill_commands is off"
        );
    }

    #[test]
    fn expand_skill_command_does_not_expand_when_disabled() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.settings.enable_skill_commands = false;
        root.skills = vec![pi_agent_core::Skill {
            name: "rust".into(),
            description: "Rust".into(),
            location: "/s/rust".into(),
            content: "Rust guide".into(),
            disable_model_invocation: false,
        }];

        let result = root.expand_prompt_text("/skill:rust write code");
        assert_eq!(
            result, "/skill:rust write code",
            "skill command should pass through when disabled"
        );
    }

    #[test]
    fn expand_skill_command_expands_when_enabled() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.settings.enable_skill_commands = true;
        root.skills = vec![pi_agent_core::Skill {
            name: "rust".into(),
            description: "Rust".into(),
            location: "/s/rust".into(),
            content: "Rust guide".into(),
            disable_model_invocation: false,
        }];

        let result = root.expand_prompt_text("/skill:rust write code");
        assert!(
            result.contains("Rust guide"),
            "skill command should expand when enabled: {result}"
        );
    }

    #[test]
    fn name_command_without_args_shows_current_session_label() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "session-123".to_string(),
        );
        root.handle_slash_command(ParsedSlashCommand {
            name: "name".to_string(),
            args: String::new(),
            original: "/name".to_string(),
        });
        let text = last_system_text(&root);
        assert!(text.contains("session-123"), "{text}");
    }

    #[test]
    fn name_command_with_args_updates_session_label() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.handle_slash_command(ParsedSlashCommand {
            name: "name".to_string(),
            args: "Project Phoenix".to_string(),
            original: "/name Project Phoenix".to_string(),
        });
        assert_eq!(root.session_label, "Project Phoenix");
        let text = last_system_text(&root);
        assert!(text.contains("Session name set: Project Phoenix"), "{text}");
    }

    #[test]
    fn session_command_reports_active_rust_native_session_details() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("/tmp/project"),
            "faux-model".to_string(),
            "Project Phoenix".to_string(),
        );
        root.set_active_session_choice(SessionChoice {
            id: "sess_rust_native".to_string(),
            cwd: "/tmp/project".to_string(),
            path: PathBuf::from("/tmp/project/sessions/sess_rust_native"),
            created_at: "2026-06-30T00:00:00Z".to_string(),
            name: None,
            entry_count: 2,
            active_leaf_id: Some("leaf_1".to_string()),
            kind: SessionChoiceKind::RustNative,
        });

        root.handle_slash_command(ParsedSlashCommand {
            name: "session".to_string(),
            args: String::new(),
            original: "/session".to_string(),
        });

        let text = last_system_text(&root);
        assert!(text.contains("Storage: rust-native"), "{text}");
        assert!(text.contains("Session ID: sess_rust_native"), "{text}");
        assert!(text.contains("Entries: 2"), "{text}");
        assert!(text.contains("Active leaf: leaf_1"), "{text}");
    }

    #[test]
    fn clone_command_reports_failure_for_missing_rust_native_session() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("/tmp/project"),
            "faux-model".to_string(),
            "Project Phoenix".to_string(),
        );
        root.set_active_session_choice(SessionChoice {
            id: "sess_rust_native".to_string(),
            cwd: "/tmp/project".to_string(),
            path: PathBuf::from("/tmp/project/sessions/sess_rust_native"),
            created_at: "2026-06-30T00:00:00Z".to_string(),
            name: None,
            entry_count: 2,
            active_leaf_id: Some("leaf_1".to_string()),
            kind: SessionChoiceKind::RustNative,
        });

        root.handle_slash_command(ParsedSlashCommand {
            name: "clone".to_string(),
            args: String::new(),
            original: "/clone".to_string(),
        });

        assert!(root.take_selected_session().is_none());
        let text = last_system_text(&root);
        assert!(text.contains("Failed to clone session:"), "{text}");
        assert!(!text.contains("not implemented"), "{text}");
    }

    #[test]
    fn fork_command_reports_failure_for_missing_rust_native_session() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("/tmp/project"),
            "faux-model".to_string(),
            "Project Phoenix".to_string(),
        );
        root.set_active_session_choice(SessionChoice {
            id: "sess_rust_native".to_string(),
            cwd: "/tmp/project".to_string(),
            path: PathBuf::from("/tmp/project/sessions/sess_rust_native"),
            created_at: "2026-06-30T00:00:00Z".to_string(),
            name: None,
            entry_count: 2,
            active_leaf_id: Some("leaf_1".to_string()),
            kind: SessionChoiceKind::RustNative,
        });

        root.handle_slash_command(ParsedSlashCommand {
            name: "fork".to_string(),
            args: String::new(),
            original: "/fork".to_string(),
        });

        assert!(root.take_selected_session().is_none());
        let text = last_system_text(&root);
        assert!(text.contains("Failed to fork session:"), "{text}");
        assert!(!text.contains("not implemented"), "{text}");
    }

    #[test]
    fn session_command_reports_current_footer_state() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("/tmp/project"),
            "faux-model".to_string(),
            "Project Phoenix".to_string(),
        );
        root.stats = FooterStats {
            input: 1234,
            output: 5678,
            ..Default::default()
        };
        root.handle_slash_command(ParsedSlashCommand {
            name: "session".to_string(),
            args: String::new(),
            original: "/session".to_string(),
        });
        let text = last_system_text(&root);
        assert!(text.contains("Session Info"), "{text}");
        assert!(text.contains("Project Phoenix"), "{text}");
        assert!(text.contains("faux-model"), "{text}");
        assert!(text.contains("1.2k"), "{text}");
        assert!(text.contains("5.7k"), "{text}");
    }

    #[test]
    fn hotkeys_command_mentions_core_interactive_bindings() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.handle_slash_command(ParsedSlashCommand {
            name: "hotkeys".to_string(),
            args: String::new(),
            original: "/hotkeys".to_string(),
        });
        let text = last_system_text(&root);
        assert!(text.contains("Navigation"), "{text}");
        assert!(text.contains("Ctrl+C"), "{text}");
        assert!(text.contains("Ctrl+O"), "{text}");
    }

    #[tokio::test]
    async fn real_stdin_reader_is_created_after_terminal_start() {
        #[derive(Default)]
        struct OrderingTerminal {
            events: Arc<Mutex<Vec<&'static str>>>,
        }

        impl Terminal for OrderingTerminal {
            fn size(&self) -> pi_tui::TerminalSize {
                pi_tui::TerminalSize {
                    columns: 80,
                    rows: 24,
                }
            }

            fn write(&mut self, _data: &str) -> std::io::Result<()> {
                Ok(())
            }

            fn move_by(&mut self, _rows: i16) -> std::io::Result<()> {
                Ok(())
            }

            fn move_to_column(&mut self, _column: usize) -> std::io::Result<()> {
                Ok(())
            }

            fn hide_cursor(&mut self) -> std::io::Result<()> {
                Ok(())
            }

            fn show_cursor(&mut self) -> std::io::Result<()> {
                Ok(())
            }

            fn clear_line(&mut self) -> std::io::Result<()> {
                Ok(())
            }

            fn clear_from_cursor(&mut self) -> std::io::Result<()> {
                Ok(())
            }

            fn clear_screen(&mut self) -> std::io::Result<()> {
                Ok(())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }

            fn start(&mut self) -> std::io::Result<()> {
                self.events.lock().unwrap().push("start");
                Ok(())
            }
        }

        let events = Arc::new(Mutex::new(Vec::new()));
        let terminal = OrderingTerminal {
            events: Arc::clone(&events),
        };
        let parsed = CliArgs::default();
        let options = CliRunOptions {
            register_builtins: false,
            ..CliRunOptions::default()
        };

        let result = run_interactive_loop_with_input(parsed, options, terminal, || {
            events.lock().unwrap().push("input");
            InputPump::from_chunks(Vec::new())
        })
        .await;

        if let Err(error) = result {
            panic!("interactive loop should complete: {error}");
        }
        assert_eq!(&*events.lock().unwrap(), &["start", "input"]);
    }

    fn last_system_text(root: &InteractiveRoot) -> String {
        match root.transcript.items().last() {
            Some(TranscriptItem::System { text }) => text.clone(),
            other => panic!("expected last transcript item to be System, got {other:?}"),
        }
    }

    fn write_rust_native_session(root: &Path, cwd: &Path, text: &str) -> PathBuf {
        let session_id = "sess_tree_fixture";
        let session_dir = root.join(session_id);
        std::fs::create_dir_all(session_dir.join("blobs")).unwrap();
        std::fs::create_dir_all(session_dir.join("index")).unwrap();
        let manifest = serde_json::json!({
            "schema": "pi-rust.session",
            "version": 1,
            "session_id": session_id,
            "created_at": "2026-06-30T00:00:00Z",
            "updated_at": "2026-06-30T00:00:02Z",
            "active_leaf_id": "leaf_1",
            "event_log": "events.jsonl"
        });
        std::fs::write(
            session_dir.join("session.json"),
            serde_json::to_string(&manifest).unwrap(),
        )
        .unwrap();

        let event = |event_id: &str,
                     operation_id: Option<&str>,
                     turn_id: Option<&str>,
                     created_at: &str,
                     kind: &str,
                     data: serde_json::Value| {
            let mut value = serde_json::json!({
                "schema": "pi-rust.session.event",
                "version": 2,
                "session_id": session_id,
                "event_id": event_id,
                "created_at": created_at,
                "kind": kind,
                "data": data
            });
            if let Some(operation_id) = operation_id {
                value["operation_id"] = serde_json::json!(operation_id);
            }
            if let Some(turn_id) = turn_id {
                value["turn_id"] = serde_json::json!(turn_id);
            }
            serde_json::to_string(&value).unwrap()
        };
        let events = vec![
            event(
                "evt_0",
                None,
                None,
                "2026-06-30T00:00:00Z",
                "session.created",
                serde_json::json!({ "cwd": cwd.display().to_string() }),
            ),
            event(
                "evt_1",
                Some("op_1"),
                Some("turn_1"),
                "2026-06-30T00:00:01Z",
                "operation.started",
                serde_json::json!({ "operation": { "kind": "prompt" } }),
            ),
            event(
                "evt_2",
                Some("op_1"),
                Some("turn_1"),
                "2026-06-30T00:00:01Z",
                "turn.input.recorded",
                serde_json::json!({
                    "content": [{ "type": "text", "data": { "text": text } }]
                }),
            ),
            event(
                "evt_3",
                Some("op_1"),
                Some("turn_1"),
                "2026-06-30T00:00:01Z",
                "operation.committed",
                serde_json::json!({ "new_leaf_id": "leaf_1" }),
            ),
            event(
                "evt_4",
                Some("op_2"),
                Some("turn_2"),
                "2026-06-30T00:00:02Z",
                "operation.started",
                serde_json::json!({ "operation": { "kind": "prompt" } }),
            ),
            event(
                "evt_5",
                Some("op_2"),
                Some("turn_2"),
                "2026-06-30T00:00:02Z",
                "turn.input.recorded",
                serde_json::json!({
                    "content": [{
                        "type": "text",
                        "data": { "text": format!("follow-up {text}") }
                    }]
                }),
            ),
            event(
                "evt_6",
                Some("op_2"),
                Some("turn_2"),
                "2026-06-30T00:00:02Z",
                "operation.committed",
                serde_json::json!({ "new_leaf_id": "leaf_2" }),
            ),
        ];
        std::fs::write(session_dir.join("events.jsonl"), events.join("\n") + "\n").unwrap();
        session_dir
    }

    #[derive(Default)]
    struct TestClipboard {
        text: Mutex<Option<String>>,
    }

    impl ClipboardSink for TestClipboard {
        fn copy_text(&self, text: &str) -> Result<(), String> {
            *self.text.lock().unwrap() = Some(text.to_string());
            Ok(())
        }
    }

    impl TestClipboard {
        fn last_text(&self) -> Option<String> {
            self.text.lock().unwrap().clone()
        }
    }
}

#[cfg(any(test, feature = "test-harness", debug_assertions))]
pub mod test_harness {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use pi_ai::providers::faux::FauxProvider;
    use pi_ai::registry;
    use pi_ai::types::{Model, ModelCost, ModelInput};
    use pi_tui::{TerminalOp, VirtualTerminal};

    use super::*;

    #[derive(Debug)]
    pub struct ScriptedInteractiveOutput {
        pub rendered: String,
        pub exit_code: i32,
        pub terminal_restored: bool,
        pub cursor_row: usize,
        pub cursor_col: usize,
        pub ops: Vec<TerminalOp>,
        pub rendered_lines: Vec<String>,
        pub session_file: PathBuf,
    }

    impl ScriptedInteractiveOutput {
        pub fn contains(&self, needle: &str) -> bool {
            self.rendered.contains(needle)
        }
    }

    pub async fn run_scripted_interactive(
        provider: FauxProvider,
        input: &str,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        run_scripted(provider, input, None).await
    }

    pub async fn run_scripted_interactive_with_size(
        provider: FauxProvider,
        input: &str,
        columns: usize,
        rows: usize,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        run_scripted_with_provider_and_size(Arc::new(provider), vec![input], None, columns, rows)
            .await
    }

    pub async fn run_scripted_interactive_with_session_dir(
        provider: FauxProvider,
        session_dir: &Path,
        input: &str,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        run_scripted(provider, input, Some(session_dir)).await
    }

    pub async fn run_scripted_interactive_with_args_and_session_dir(
        provider: FauxProvider,
        parsed: CliArgs,
        session_dir: &Path,
        input: &str,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        run_scripted_with_provider_args_and_size(
            Arc::new(provider),
            parsed,
            vec![input],
            Some(session_dir),
            80,
            24,
        )
        .await
    }

    pub async fn run_scripted_interactive_with_session_dir_and_waits(
        provider: FauxProvider,
        session_dir: &Path,
        input_steps: Vec<(&str, &str)>,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        run_scripted_interactive_with_session_dir_size_and_waits(
            provider,
            session_dir,
            input_steps,
            80,
            24,
        )
        .await
    }

    pub async fn run_scripted_interactive_with_session_dir_size_and_waits(
        provider: FauxProvider,
        session_dir: &Path,
        input_steps: Vec<(&str, &str)>,
        columns: usize,
        rows: usize,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        run_scripted_with_provider_and_waits(
            Arc::new(provider),
            session_dir,
            input_steps,
            columns,
            rows,
        )
        .await
    }

    pub async fn run_scripted_interactive_with_provider_chunks(
        provider: Arc<dyn pi_ai::registry::ApiProvider>,
        input_chunks: Vec<&str>,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        run_scripted_with_provider(provider, input_chunks, None).await
    }

    pub async fn run_scripted_idle_interactive(
        input: &str,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        run_scripted_idle_interactive_with_size(input, 80, 24).await
    }

    pub async fn run_scripted_idle_interactive_with_size(
        input: &str,
        columns: usize,
        rows: usize,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        let mut input = InputPump::from_chunks(vec![input.to_string()]);
        let parsed = CliArgs::default();
        let options = CliRunOptions {
            register_builtins: false,
            ..CliRunOptions::default()
        };
        let result = run_interactive_loop(
            parsed,
            options,
            VirtualTerminal::new(columns, rows),
            &mut input,
        )
        .await?;
        Ok(scripted_output(result, None))
    }

    /// Drive the interactive loop with a sequence of `(chunk, post_delay)`
    /// steps. After each chunk is sent, the harness sleeps `post_delay`
    /// before sending the next chunk (or, on the final step, before closing
    /// stdin and letting the loop terminate). This allows tests to exercise
    /// the [`StdinBuffer`] idle-flush timer for stuck escape sequences.
    pub async fn run_scripted_idle_interactive_with_delays(
        steps: Vec<(&str, Duration)>,
        columns: usize,
        rows: usize,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let mut input = InputPump::from_receiver(rx);
        let parsed = CliArgs::default();
        let options = CliRunOptions {
            register_builtins: false,
            ..CliRunOptions::default()
        };

        let owned_steps = steps
            .into_iter()
            .map(|(chunk, delay)| (chunk.to_string(), delay))
            .collect::<Vec<_>>();
        let driver = async move {
            for (chunk, delay) in owned_steps {
                if tx.send(chunk).is_err() {
                    return;
                }
                if delay > Duration::ZERO {
                    tokio::time::sleep(delay).await;
                }
            }
            drop(tx);
        };

        let run = run_interactive_loop(
            parsed,
            options,
            VirtualTerminal::new(columns, rows),
            &mut input,
        );
        let (result, ()) = tokio::join!(run, driver);
        Ok(scripted_output(result?, None))
    }

    async fn run_scripted_with_provider(
        provider: Arc<dyn pi_ai::registry::ApiProvider>,
        input_chunks: Vec<&str>,
        session_dir: Option<&Path>,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        run_scripted_with_provider_and_size(provider, input_chunks, session_dir, 80, 24).await
    }

    async fn run_scripted_with_provider_and_size(
        provider: Arc<dyn pi_ai::registry::ApiProvider>,
        input_chunks: Vec<&str>,
        session_dir: Option<&Path>,
        columns: usize,
        rows: usize,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        run_scripted_with_provider_args_and_size(
            provider,
            CliArgs::default(),
            input_chunks,
            session_dir,
            columns,
            rows,
        )
        .await
    }

    async fn run_scripted_with_provider_args_and_size(
        provider: Arc<dyn pi_ai::registry::ApiProvider>,
        parsed: CliArgs,
        input_chunks: Vec<&str>,
        session_dir: Option<&Path>,
        columns: usize,
        rows: usize,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        let api = format!(
            "interactive-harness-{}",
            INTERACTIVE_ID.fetch_add(1, Ordering::SeqCst)
        );
        registry::register(&api, provider);

        let chunks = input_chunks
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let mut input = InputPump::from_chunks(chunks);
        let session = session_dir
            .map(|dir| SessionRunOptions {
                mode: SessionMode::Enabled,
                cwd: dir.to_path_buf(),
                session_dir: Some(dir.to_path_buf()),
            })
            .unwrap_or_else(|| SessionRunOptions::disabled(PathBuf::from(".")));
        let options = CliRunOptions {
            model_override: Some(faux_model(&api)),
            tools: Vec::new(),
            register_builtins: false,
            session,
        };

        let result = run_interactive_loop(
            parsed,
            options,
            VirtualTerminal::new(columns, rows),
            &mut input,
        )
        .await;
        registry::unregister(&api);

        Ok(scripted_output(result?, session_dir))
    }

    async fn run_scripted_with_provider_and_waits(
        provider: Arc<dyn pi_ai::registry::ApiProvider>,
        session_dir: &Path,
        input_steps: Vec<(&str, &str)>,
        columns: usize,
        rows: usize,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        let api = format!(
            "interactive-harness-{}",
            INTERACTIVE_ID.fetch_add(1, Ordering::SeqCst)
        );
        registry::register(&api, provider);

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let mut input = InputPump::from_receiver(rx);
        let parsed = CliArgs::default();
        let session = SessionRunOptions {
            mode: SessionMode::Enabled,
            cwd: session_dir.to_path_buf(),
            session_dir: Some(session_dir.to_path_buf()),
        };
        let options = CliRunOptions {
            model_override: Some(faux_model(&api)),
            tools: Vec::new(),
            register_builtins: false,
            session,
        };

        let session_dir_for_input = session_dir.to_path_buf();
        let input_steps = input_steps
            .into_iter()
            .map(|(chunk, wait_for)| (chunk.to_string(), wait_for.to_string()))
            .collect::<Vec<_>>();
        let input_driver = async move {
            for (chunk, wait_for) in input_steps {
                if tx.send(chunk).is_err() {
                    return Ok::<(), CliError>(());
                }
                wait_for_session_text(&session_dir_for_input, &wait_for).await?;
            }
            Ok(())
        };

        let run = run_interactive_loop(
            parsed,
            options,
            VirtualTerminal::new(columns, rows),
            &mut input,
        );
        let (result, input_result) = tokio::join!(run, input_driver);
        registry::unregister(&api);
        input_result?;

        Ok(scripted_output(result?, Some(session_dir)))
    }

    async fn run_scripted(
        provider: FauxProvider,
        input: &str,
        session_dir: Option<&Path>,
    ) -> Result<ScriptedInteractiveOutput, CliError> {
        run_scripted_with_provider(Arc::new(provider), vec![input], session_dir).await
    }

    fn scripted_output(
        result: LoopResult<VirtualTerminal>,
        session_dir: Option<&Path>,
    ) -> ScriptedInteractiveOutput {
        let terminal_restored = result.tui.terminal().ops().contains(&TerminalOp::Stop);
        let rendered = result.tui.terminal().written_output();
        let cursor_row = result.tui.terminal().cursor_row();
        let cursor_col = result.tui.terminal().cursor_col();
        let ops = result.tui.terminal().ops().to_vec();
        let rendered_lines = result.tui.rendered_lines().to_vec();
        ScriptedInteractiveOutput {
            rendered,
            exit_code: result.exit_code,
            terminal_restored,
            cursor_row,
            cursor_col,
            ops,
            rendered_lines,
            session_file: session_dir
                .and_then(|dir| first_rust_native_event_log(dir).ok())
                .unwrap_or_default(),
        }
    }

    fn faux_model(api: &str) -> Model {
        Model {
            id: "faux-model".into(),
            name: "Faux Model".into(),
            api: api.into(),
            provider: "faux".into(),
            base_url: String::new(),
            reasoning: false,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
            },
            context_window: 0,
            max_tokens: 0,
            headers: None,
            compat: None,
        }
    }

    fn first_rust_native_event_log(root: &Path) -> Result<PathBuf, std::io::Error> {
        let mut files = Vec::new();
        collect_rust_native_event_logs(root, &mut files)?;
        files.sort();
        files.into_iter().next().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "no rust-native event log")
        })
    }

    async fn wait_for_session_text(root: &Path, needle: &str) -> Result<(), CliError> {
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(2);
        loop {
            if session_files_contain(root, needle) {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(CliError::AgentFailure(format!(
                    "timed out waiting for session text: {needle}"
                )));
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }

    fn session_files_contain(root: &Path, needle: &str) -> bool {
        let mut files = Vec::new();
        if collect_rust_native_event_logs(root, &mut files).is_err() {
            return false;
        }
        files.iter().any(|path| {
            std::fs::read_to_string(path)
                .map(|text| text.contains(needle))
                .unwrap_or(false)
        })
    }

    fn collect_rust_native_event_logs(
        root: &Path,
        out: &mut Vec<PathBuf>,
    ) -> Result<(), std::io::Error> {
        if !root.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(root)? {
            let path = entry?.path();
            if !path.is_dir() {
                continue;
            }
            let event_log = path.join("events.jsonl");
            if path.join("session.json").is_file() && event_log.is_file() {
                out.push(event_log);
            } else {
                collect_rust_native_event_logs(&path, out)?;
            }
        }
        Ok(())
    }
}
