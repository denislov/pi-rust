mod support;

use std::collections::HashMap;

use pi_agent_core::AgentResources;
use pi_ai::providers::faux::FauxProvider;
use pi_ai::types::{Model, ModelCost, ModelInput};
use pi_coding_agent::api::{
    CapabilityStatus, CliArgs, CliDiagnostic, CliDiagnosticSeverity, CliError, CliOutput,
    CliRunOptions, CodingAgentCapabilities, CodingAgentEvent, CodingAgentEventReceiver,
    CodingAgentSession, CodingAgentSessionExport, CodingAgentSessionExportItem,
    CodingAgentSessionOptions, CodingAgentSessionSummary, CodingAgentSessionView, CodingDiagnostic,
    CodingDiagnosticSeverity, CodingSessionError, ColorValue, CompactionProtocolResult,
    CompactionReason, ContextFile, DetectionConfidence, DetectionSource, ModelRotation,
    ModelRotationEntry, PendingDelegationConfirmation, PrintModeOptions, ProfileId,
    PromptInvocation, PromptRunOptions, PromptTurnMode, PromptTurnOptions, PromptTurnOutcome,
    ProtocolDelegationFoldedBlock, ProtocolEvent, ProtocolSelfHealingEditCheckOutput,
    ProtocolSelfHealingEditReplacement, REQUIRED_TOKEN_KEYS, ResolveError, ResolvedColor,
    ResolvedTheme, ResourceLoadOptions, RpcCapabilities, RpcCapabilityStatus, RpcCommand,
    RpcDelegationCapabilityStatus, RpcDelegationRenderingMetadata, RpcResponse,
    RpcSelfHealingEditModelRepair, RpcSelfHealingEditReplacement, RpcSessionState,
    SelfHealingEditCheckOutput, SelfHealingEditDiagnostic, SelfHealingEditModelRepairOptions,
    SelfHealingEditOutcome, SelfHealingEditRepairAttempt, SelfHealingEditReplacement,
    SelfHealingEditRequest, SessionMode, StreamingBehavior, TerminalTheme, ThemeBg, ThemeColor,
    ThemeExportColors, ThemeJson, ToolExecutionResult, ToolFilter, build_agent_resources,
    builtin_dark, builtin_tools, detect_terminal_background, discover_context_files, filter_tools,
    get_resolved_theme_colors, get_theme_export_colors, get_theme_for_rgb_color, help_text,
    is_light_theme, parse_args, parse_model_rotation, parse_osc11_background_color,
    render_diagnostics, resolve, resolve_resource_paths,
};
use support::ProviderGuard;

fn model(api: &str) -> Model {
    Model {
        id: "test-model".into(),
        name: "Test Model".into(),
        api: api.into(),
        provider: "test".into(),
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

#[test]
fn public_api_symbols_are_importable() {
    let args = CliArgs::default();
    assert_eq!(args.max_turns, None);

    let parsed = parse_args(vec!["-p".to_string(), "hello".to_string()]).unwrap();
    assert!(parsed.print);
    assert_eq!(parsed.prompt.as_deref(), Some("hello"));

    let print_options = PrintModeOptions::new("hello", model("public-api-test"));
    assert_eq!(print_options.prompt, "hello");
    assert!(!print_options.register_builtins);

    let output = CliOutput {
        exit_code: 0,
        stdout: "ok\n".into(),
        stderr: String::new(),
    };
    assert_eq!(output.exit_code, 0);

    let runtime_options = CliRunOptions::default();
    assert!(runtime_options.register_builtins);
    let mode = SessionMode::Enabled;
    assert!(matches!(mode, SessionMode::Enabled));

    let invocation = PromptInvocation::Text("hello".into());
    assert!(matches!(invocation, PromptInvocation::Text(_)));

    let tools = builtin_tools(std::env::current_dir().unwrap());
    let read_only = filter_tools(
        tools,
        &ToolFilter {
            allow: vec!["read".into()],
            ..ToolFilter::default()
        },
    );
    assert_eq!(read_only.len(), 1);
    assert_eq!(read_only[0].name, "read");

    let _prompt_run_type_name = std::any::type_name::<PromptRunOptions>();

    let diagnostic_text = render_diagnostics(&[CliDiagnostic {
        severity: CliDiagnosticSeverity::Warning,
        message: "heads up".into(),
        source: None,
        code: None,
    }]);
    assert_eq!(diagnostic_text, "warning: heads up\n");

    let err = CliError::MissingPrompt;
    assert_eq!(err.to_string(), "missing prompt");

    assert!(help_text().contains("Usage:"));
}

#[test]
fn model_rotation_surface_is_importable_from_api_facade() {
    let rotation = parse_model_rotation("gpt-4*:high,claude-*:medium").unwrap();
    let _rotation_type_name = std::any::type_name::<ModelRotation>();
    let _entry_type_name = std::any::type_name::<ModelRotationEntry>();

    assert_eq!(rotation.entries.len(), 2);
    assert_eq!(rotation.entries[0].pattern, "gpt-4*");
    assert!(rotation.matches("gpt-4.1"));
    assert!(rotation.matches("claude-sonnet"));
    assert!(!rotation.matches("mistral-large"));
}

#[test]
fn resource_surface_is_importable_from_api_facade() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    let project = workspace.join("project");
    let agent_dir = temp.path().join("agent-home");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(&agent_dir).unwrap();
    std::fs::write(workspace.join("AGENTS.md"), "project instructions").unwrap();

    let options = ResourceLoadOptions {
        no_skills: true,
        no_prompt_templates: true,
        no_themes: true,
        skill_paths: vec!["skills".into()],
        prompt_paths: vec!["prompts".into()],
        theme_paths: vec!["themes".into()],
        theme: Some("dark".into()),
    };
    assert!(options.no_skills);

    let resolved = resolve_resource_paths(&["relative".into()], &project);
    assert_eq!(resolved, vec![project.join("relative")]);

    let context_files = discover_context_files(&project, &agent_dir, false);
    let _context_file_type_name = std::any::type_name::<ContextFile>();
    assert_eq!(context_files.len(), 1);
    assert_eq!(context_files[0].content, "project instructions");

    let resources = build_agent_resources(Vec::new(), Vec::new());
    let default_resources = AgentResources::default();
    assert_eq!(resources.skills.len(), default_resources.skills.len());
    assert_eq!(
        resources.prompt_templates.len(),
        default_resources.prompt_templates.len()
    );
}

#[test]
fn theme_model_types_are_importable_from_api_facade() {
    let _theme_type_name = std::any::type_name::<ThemeJson>();
    let _resolved_type_name = std::any::type_name::<ResolvedTheme>();
    let _error_type_name = std::any::type_name::<ResolveError>();
    assert!(REQUIRED_TOKEN_KEYS.contains(&"text"));
    assert_eq!(ThemeColor::Text, ThemeColor::from_key("text").unwrap());
    assert_eq!(ThemeBg::UserMessageBg.key(), "userMessageBg");

    let parsed = ColorValue::parse(&serde_json::json!("#010203")).unwrap();
    assert_eq!(parsed, ColorValue::Hex(1, 2, 3));
    let vars = HashMap::from([("accent".to_string(), parsed.clone())]);
    assert_eq!(
        resolve(&ColorValue::Var("accent".into()), &vars),
        Ok(ResolvedColor::Hex(1, 2, 3))
    );

    let theme = builtin_dark();
    assert!(theme.missing_tokens().is_empty());
    let resolved = theme.resolve_colors().unwrap();
    let _ = resolved.fg(ThemeColor::Text);
    let _ = resolved.bg(ThemeBg::UserMessageBg);
}

#[test]
fn theme_detection_and_export_helpers_are_importable_from_api_facade() {
    assert_eq!(
        parse_osc11_background_color("\x1b]11;#fefefe\x07"),
        Some((254, 254, 254))
    );
    assert_eq!(
        get_theme_for_rgb_color((250, 250, 250)),
        TerminalTheme::Light
    );
    assert_eq!(get_theme_for_rgb_color((1, 2, 3)), TerminalTheme::Dark);

    let detection = detect_terminal_background([("COLORFGBG", "0;15")]);
    assert_eq!(detection.theme, TerminalTheme::Light);
    assert_eq!(detection.source, DetectionSource::ColorFgbg);
    assert_eq!(detection.confidence, DetectionConfidence::High);

    assert!(is_light_theme(Some("light")));
    assert!(!is_light_theme(Some("dark")));

    let theme = builtin_dark();
    let export_colors = get_theme_export_colors(&theme);
    let _export_type_name = std::any::type_name::<ThemeExportColors>();
    assert!(
        export_colors
            .page_bg
            .as_deref()
            .unwrap_or("#000000")
            .starts_with('#')
    );

    let resolved_colors = get_resolved_theme_colors(&theme, "#ffffff");
    assert!(resolved_colors.iter().any(|(key, _)| key == "text"));
}

#[test]
fn protocol_wire_types_are_importable_from_api_facade() {
    fn type_name<T>() -> &'static str {
        std::any::type_name::<T>()
    }

    assert!(matches!(
        ProtocolEvent::AgentStart,
        ProtocolEvent::AgentStart
    ));
    assert!(matches!(CompactionReason::Manual, CompactionReason::Manual));
    assert!(matches!(StreamingBehavior::Steer, StreamingBehavior::Steer));
    assert!(type_name::<ToolExecutionResult>().contains("ToolExecutionResult"));
    assert!(type_name::<CompactionProtocolResult>().contains("CompactionProtocolResult"));
    assert!(
        type_name::<ProtocolSelfHealingEditReplacement>()
            .contains("ProtocolSelfHealingEditReplacement")
    );
    assert!(
        type_name::<ProtocolSelfHealingEditCheckOutput>()
            .contains("ProtocolSelfHealingEditCheckOutput")
    );
    assert!(type_name::<ProtocolDelegationFoldedBlock>().contains("ProtocolDelegationFoldedBlock"));
    assert!(type_name::<RpcSelfHealingEditReplacement>().contains("RpcSelfHealingEditReplacement"));
    assert!(type_name::<RpcSelfHealingEditModelRepair>().contains("RpcSelfHealingEditModelRepair"));
    assert!(type_name::<RpcCommand>().contains("RpcCommand"));
    assert!(type_name::<RpcSessionState>().contains("RpcSessionState"));
    assert!(type_name::<RpcCapabilities>().contains("RpcCapabilities"));
    assert!(type_name::<RpcCapabilityStatus>().contains("RpcCapabilityStatus"));
    assert!(type_name::<RpcDelegationCapabilityStatus>().contains("RpcDelegationCapabilityStatus"));
    assert!(
        type_name::<RpcDelegationRenderingMetadata>().contains("RpcDelegationRenderingMetadata")
    );
    assert!(type_name::<RpcResponse>().contains("RpcResponse"));
}

#[tokio::test]
async fn coding_session_public_api_symbols_are_importable() {
    let temp = tempfile::tempdir().unwrap();
    let options = CodingAgentSessionOptions::new()
        .with_session_id("sess_public_api")
        .with_cwd(temp.path())
        .with_session_log_root(temp.path())
        .with_session_path("sess_public_api");
    assert_eq!(options.session_id(), Some("sess_public_api"));
    assert_eq!(options.cwd(), Some(temp.path()));
    assert_eq!(options.session_log_root(), Some(temp.path()));
    assert_eq!(
        options.session_path(),
        Some(std::path::Path::new("sess_public_api"))
    );

    let mut session = CodingAgentSession::create(options).await.unwrap();
    let view = session.view();
    assert_eq!(
        view,
        CodingAgentSessionView {
            session_id: "sess_public_api".into(),
            default_agent_profile_id: ProfileId::from("default"),
        }
    );

    let capabilities = session.capabilities();
    assert_eq!(
        capabilities,
        CodingAgentCapabilities {
            prompt: CapabilityStatus::Available,
            abort: CapabilityStatus::Disabled {
                reason: "no prompt is running".into(),
            },
            steer: CapabilityStatus::Disabled {
                reason: "no prompt is running".into(),
            },
            follow_up: CapabilityStatus::Disabled {
                reason: "no prompt is running".into(),
            },
            compact: CapabilityStatus::Available,
            fork: CapabilityStatus::Available,
            clone_session: CapabilityStatus::Available,
            branch_summary: CapabilityStatus::Available,
            switch_session: CapabilityStatus::Unsupported {
                reason: "session switching is not exposed on CodingAgentSession yet".into(),
            },
            export: CapabilityStatus::Available,
            plugin_reload: CapabilityStatus::Available,
            self_healing_edit: CapabilityStatus::Available,
            agent_profiles: CapabilityStatus::Available,
            team_profiles: CapabilityStatus::Available,
            delegation: CapabilityStatus::Available,
            tools: CapabilityStatus::Available,
            shell: CapabilityStatus::Available,
            plugins: CapabilityStatus::Available,
        }
    );

    let agent_profiles = session.agent_profiles();
    assert!(
        agent_profiles.iter().any(|profile| {
            profile.id.as_str() == "default" && profile.display_name == "Default"
        })
    );
    assert!(session.team_profiles().is_empty());
    assert!(session.profile_diagnostics().is_empty());
    assert!(session.pending_delegation_confirmations().is_empty());

    let mut receiver = session.subscribe();
    assert!(receiver.try_recv().unwrap().is_none());
    let _receiver_type_name = std::any::type_name::<CodingAgentEventReceiver>();
    let _export_type_name = std::any::type_name::<CodingAgentSessionExport>();
    let _export_item_type_name = std::any::type_name::<CodingAgentSessionExportItem>();
    let _pending_delegation_type_name = std::any::type_name::<PendingDelegationConfirmation>();

    let error = CodingSessionError::UnsupportedCapability {
        capability: "prompt".into(),
    };
    assert_eq!(error.code(), "unsupported_capability");
    assert_eq!(error.to_string(), "unsupported capability: prompt");

    let event = CodingAgentEvent::PromptFailed {
        operation_id: "op_public_api".into(),
        error,
    };
    assert!(matches!(
        event,
        CodingAgentEvent::PromptFailed {
            operation_id,
            error: CodingSessionError::UnsupportedCapability { .. },
        } if operation_id == "op_public_api"
    ));

    let prompt_options = PromptTurnOptions::new(PromptInvocation::Text("hello".into()))
        .with_mode(PromptTurnMode::Print);
    assert!(matches!(
        prompt_options.invocation(),
        PromptInvocation::Text(text) if text == "hello"
    ));
    assert_eq!(prompt_options.mode(), PromptTurnMode::Print);

    let prompt_error = session.prompt(prompt_options).await.unwrap_err();
    assert_eq!(prompt_error.code(), "config");
    assert!(prompt_error.to_string().contains("runtime snapshot"));

    let branch_summary_error = session
        .summarize_branch(
            PromptTurnOptions::new(PromptInvocation::Text(String::new()))
                .with_mode(PromptTurnMode::Print),
            "leaf_abandoned",
            "leaf_target",
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(branch_summary_error.code(), "config");
    assert!(
        branch_summary_error
            .to_string()
            .contains("branch summary options do not include a runtime snapshot")
    );

    let diagnostic = CodingDiagnostic::warning("heads up").with_code("phase_2");
    assert_eq!(diagnostic.severity, CodingDiagnosticSeverity::Warning);
    assert_eq!(diagnostic.code.as_deref(), Some("phase_2"));

    let outcome = PromptTurnOutcome::Aborted {
        operation_id: "op_public_api".into(),
        turn_id: None,
        reason: "test".into(),
        session_id: None,
    };
    assert!(matches!(
        outcome,
        PromptTurnOutcome::Aborted { reason, .. } if reason == "test"
    ));

    let _self_healing_check_output_type_name = std::any::type_name::<SelfHealingEditCheckOutput>();
    let _self_healing_diagnostic_type_name = std::any::type_name::<SelfHealingEditDiagnostic>();
    let _self_healing_outcome_type_name = std::any::type_name::<SelfHealingEditOutcome>();
    let _self_healing_repair_attempt_type_name =
        std::any::type_name::<SelfHealingEditRepairAttempt>();
}

#[tokio::test]
async fn coding_session_self_healing_edit_persists_typed_events() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    let sessions = temp.path().join("sessions");
    std::fs::create_dir_all(workspace.join("src")).unwrap();
    std::fs::write(workspace.join("src/app.txt"), "one\ntwo\n").unwrap();
    let mut session = CodingAgentSession::create(
        CodingAgentSessionOptions::new()
            .with_session_id("sess_self_healing_entrypoint")
            .with_cwd(&workspace)
            .with_session_log_root(&sessions),
    )
    .await
    .unwrap();

    let outcome = session
        .self_healing_edit(
            "src/app.txt",
            vec![SelfHealingEditReplacement::new("two", "deux")],
        )
        .await
        .unwrap();

    assert_eq!(outcome.path, "src/app.txt");
    assert_eq!(outcome.attempts, 1);
    assert_eq!(outcome.first_changed_line, Some(2));
    assert!(outcome.message.contains("Successfully replaced 1 block"));
    assert_eq!(
        std::fs::read_to_string(workspace.join("src/app.txt")).unwrap(),
        "one\ndeux\n"
    );
    let event_log = std::fs::read_to_string(
        sessions
            .join("sess_self_healing_entrypoint")
            .join("events.jsonl"),
    )
    .unwrap();
    assert!(
        event_log.contains(r#""kind":"operation.started""#),
        "{event_log}"
    );
    assert!(
        event_log.contains(r#""operation":{"kind":"self_healing_edit"}"#),
        "{event_log}"
    );
    assert!(
        event_log.contains(r#""kind":"self_healing_edit.started""#),
        "{event_log}"
    );
    assert!(
        event_log.contains(r#""kind":"self_healing_edit.completed""#),
        "{event_log}"
    );
    assert!(event_log.contains(r#""path":"src/app.txt""#), "{event_log}");
    assert!(event_log.contains(r#""attempts":1"#), "{event_log}");
    assert!(
        event_log.contains(r#""kind":"operation.committed""#),
        "{event_log}"
    );
    assert_eq!(
        session.capabilities().self_healing_edit,
        CapabilityStatus::Available
    );
}

#[tokio::test]
async fn coding_session_self_healing_edit_with_check_command_records_output() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    let sessions = temp.path().join("sessions");
    std::fs::create_dir_all(workspace.join("src")).unwrap();
    std::fs::write(workspace.join("src/app.txt"), "one\ntwo\n").unwrap();
    let mut session = CodingAgentSession::create(
        CodingAgentSessionOptions::new()
            .with_session_id("sess_self_healing_check_command")
            .with_cwd(&workspace)
            .with_session_log_root(&sessions),
    )
    .await
    .unwrap();

    let outcome = session
        .self_healing_edit_with_options(
            SelfHealingEditRequest::new(
                "src/app.txt",
                vec![SelfHealingEditReplacement::new("two", "deux")],
            )
            .with_check_command("printf check-ok"),
        )
        .await
        .unwrap();

    assert_eq!(
        std::fs::read_to_string(workspace.join("src/app.txt")).unwrap(),
        "one\ndeux\n"
    );
    let check_output = outcome
        .check_output
        .as_ref()
        .expect("check output should be recorded");
    assert_eq!(check_output.command, "printf check-ok");
    assert_eq!(check_output.exit_code, 0);
    assert_eq!(check_output.stdout, "check-ok");
    assert!(check_output.stderr.is_empty());

    let event_log = std::fs::read_to_string(
        sessions
            .join("sess_self_healing_check_command")
            .join("events.jsonl"),
    )
    .unwrap();
    assert!(
        event_log.contains(r#""kind":"self_healing_edit.completed""#),
        "{event_log}"
    );
}

#[tokio::test]
async fn coding_session_self_healing_edit_uses_planned_repair_attempts() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    let sessions = temp.path().join("sessions");
    std::fs::create_dir_all(workspace.join("src")).unwrap();
    std::fs::write(workspace.join("src/app.txt"), "one\ntwo\n").unwrap();
    let mut session = CodingAgentSession::create(
        CodingAgentSessionOptions::new()
            .with_session_id("sess_self_healing_planned_repair")
            .with_cwd(&workspace)
            .with_session_log_root(&sessions),
    )
    .await
    .unwrap();

    let outcome = session
        .self_healing_edit_with_options(
            SelfHealingEditRequest::new(
                "src/app.txt",
                vec![SelfHealingEditReplacement::new("two", "deux")],
            )
            .with_check_command("grep -q dos src/app.txt")
            .with_repair_attempts(vec![vec![SelfHealingEditReplacement::new("deux", "dos")]]),
        )
        .await
        .unwrap();

    assert_eq!(
        std::fs::read_to_string(workspace.join("src/app.txt")).unwrap(),
        "one\ndos\n"
    );
    assert_eq!(outcome.attempts, 2);
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("grep -q dos src/app.txt")),
        "{:#?}",
        outcome.diagnostics
    );
    let check_output = outcome
        .check_output
        .as_ref()
        .expect("final check output should be recorded");
    assert_eq!(check_output.command, "grep -q dos src/app.txt");
    assert_eq!(check_output.exit_code, 0);
}

#[tokio::test]
async fn coding_session_self_healing_edit_uses_model_repair_strategy() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    let sessions = temp.path().join("sessions");
    std::fs::create_dir_all(workspace.join("src")).unwrap();
    std::fs::write(workspace.join("src/app.txt"), "one\ntwo\n").unwrap();
    let api = "public-api-self-healing-model-repair";
    let _provider_guard = ProviderGuard::register(
        api,
        std::sync::Arc::new(FauxProvider::simple_text(
            r#"{"edits":[{"oldText":"deux","newText":"dos"}]}"#,
        )),
    );
    let repair_options = PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
        prompt: String::new(),
        model: model(api),
        api_key: None,
        system_prompt: Some("Return only self-healing edit repair JSON.".into()),
        max_turns: Some(1),
        tools: Vec::new(),
        register_builtins: false,
        session: Some(pi_coding_agent::api::SessionRunOptions::disabled(
            workspace.clone(),
        )),
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("repair self-healing edit".into()),
    });
    let mut session = CodingAgentSession::create(
        CodingAgentSessionOptions::new()
            .with_session_id("sess_self_healing_model_repair")
            .with_cwd(&workspace)
            .with_session_log_root(&sessions),
    )
    .await
    .unwrap();
    let mut events = session.subscribe();

    let outcome = session
        .self_healing_edit_with_options(
            SelfHealingEditRequest::new(
                "src/app.txt",
                vec![SelfHealingEditReplacement::new("two", "deux")],
            )
            .with_check_command("grep -q dos src/app.txt")
            .with_model_repair(SelfHealingEditModelRepairOptions::new(repair_options)),
        )
        .await
        .unwrap();

    assert_eq!(
        std::fs::read_to_string(workspace.join("src/app.txt")).unwrap(),
        "one\ndos\n"
    );
    assert_eq!(outcome.attempts, 2);
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("grep -q dos src/app.txt")),
        "{:#?}",
        outcome.diagnostics
    );
    let check_output = outcome
        .check_output
        .as_ref()
        .expect("final check output should be recorded");
    assert_eq!(check_output.command, "grep -q dos src/app.txt");
    assert_eq!(check_output.exit_code, 0);
    assert_eq!(outcome.repair_attempts.len(), 1);
    assert_eq!(outcome.repair_attempts[0].attempt, 1);
    assert_eq!(outcome.repair_attempts[0].replacements[0].old_text, "deux");
    assert_eq!(outcome.repair_attempts[0].replacements[0].new_text, "dos");

    let event_log = std::fs::read_to_string(
        sessions
            .join("sess_self_healing_model_repair")
            .join("events.jsonl"),
    )
    .unwrap();
    assert!(
        event_log.contains(r#""kind":"self_healing_edit.repair_attempted""#),
        "{event_log}"
    );
    assert!(event_log.contains(r#""attempt":1"#), "{event_log}");
    assert!(event_log.contains(r#""old_text":"deux""#), "{event_log}");
    assert!(event_log.contains(r#""new_text":"dos""#), "{event_log}");
    assert!(event_log.contains(r#""exit_code":0"#), "{event_log}");

    let emitted_events = std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
    assert!(
        emitted_events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::SelfHealingEditStarted {
                path,
                replacements: 1,
                ..
            } if path == "src/app.txt"
        )),
        "{emitted_events:#?}"
    );
    let repair_event_count = emitted_events
        .iter()
        .filter(|event| {
            matches!(
                event,
                CodingAgentEvent::SelfHealingEditRepairAttempted { .. }
            )
        })
        .count();
    assert_eq!(repair_event_count, 1, "{emitted_events:#?}");
    assert!(
        emitted_events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::SelfHealingEditRepairAttempted {
                path,
                attempt: 1,
                replacements,
                check_output: Some(check_output),
                ..
            } if path == "src/app.txt"
                && replacements[0].old_text == "deux"
                && replacements[0].new_text == "dos"
                && check_output.exit_code == 0
        )),
        "{emitted_events:#?}"
    );
    assert!(
        emitted_events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::SelfHealingEditCompleted {
                path,
                attempts: 2,
                check_output: Some(check_output),
                ..
            } if path == "src/app.txt" && check_output.exit_code == 0
        )),
        "{emitted_events:#?}"
    );
}

#[tokio::test]
async fn coding_session_self_healing_edit_model_repair_invalid_json_preserves_check_output() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    let sessions = temp.path().join("sessions");
    std::fs::create_dir_all(workspace.join("src")).unwrap();
    std::fs::write(workspace.join("src/app.txt"), "one\ntwo\n").unwrap();
    let api = "public-api-self-healing-model-repair-invalid-json";
    let _provider_guard = ProviderGuard::register(
        api,
        std::sync::Arc::new(FauxProvider::simple_text("not repair json")),
    );
    let repair_options = PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
        prompt: String::new(),
        model: model(api),
        api_key: None,
        system_prompt: Some("Return only self-healing edit repair JSON.".into()),
        max_turns: Some(1),
        tools: Vec::new(),
        register_builtins: false,
        session: Some(pi_coding_agent::api::SessionRunOptions::disabled(
            workspace.clone(),
        )),
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("repair self-healing edit".into()),
    });
    let mut session = CodingAgentSession::create(
        CodingAgentSessionOptions::new()
            .with_session_id("sess_self_healing_model_repair_invalid_json")
            .with_cwd(&workspace)
            .with_session_log_root(&sessions),
    )
    .await
    .unwrap();

    let error = session
        .self_healing_edit_with_options(
            SelfHealingEditRequest::new(
                "src/app.txt",
                vec![SelfHealingEditReplacement::new("two", "deux")],
            )
            .with_check_command("grep -q dos src/app.txt")
            .with_model_repair(SelfHealingEditModelRepairOptions::new(repair_options)),
        )
        .await
        .unwrap_err();

    let CodingSessionError::SelfHealingEditFailed {
        message,
        diagnostics,
        check_output,
        repair_attempts,
    } = error
    else {
        panic!("expected self-healing edit failure, got {error:?}");
    };
    assert!(
        message.contains("self-healing edit repair failed"),
        "{message}"
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("not valid JSON edits")),
        "{diagnostics:#?}"
    );
    assert!(repair_attempts.is_empty());
    let check_output = check_output.expect("check output should be preserved");
    assert_eq!(check_output.command, "grep -q dos src/app.txt");
    assert_ne!(check_output.exit_code, 0);
    assert_eq!(
        std::fs::read_to_string(workspace.join("src/app.txt")).unwrap(),
        "one\ndeux\n"
    );
    let event_log = std::fs::read_to_string(
        sessions
            .join("sess_self_healing_model_repair_invalid_json")
            .join("events.jsonl"),
    )
    .unwrap();
    assert!(
        event_log.contains(r#""kind":"operation.failed""#),
        "{event_log}"
    );
    assert!(
        !event_log.contains(r#""kind":"self_healing_edit.completed""#),
        "{event_log}"
    );
}

#[tokio::test]
async fn coding_session_self_healing_edit_failed_check_exposes_output() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    let sessions = temp.path().join("sessions");
    std::fs::create_dir_all(workspace.join("src")).unwrap();
    std::fs::write(workspace.join("src/app.txt"), "one\ntwo\n").unwrap();
    let mut session = CodingAgentSession::create(
        CodingAgentSessionOptions::new()
            .with_session_id("sess_self_healing_failed_check")
            .with_cwd(&workspace)
            .with_session_log_root(&sessions),
    )
    .await
    .unwrap();

    let error = session
        .self_healing_edit_with_options(
            SelfHealingEditRequest::new(
                "src/app.txt",
                vec![SelfHealingEditReplacement::new("two", "deux")],
            )
            .with_check_command("printf check-failed >&2; exit 7"),
        )
        .await
        .unwrap_err();

    let CodingSessionError::SelfHealingEditFailed {
        message,
        diagnostics,
        check_output,
        repair_attempts,
    } = error
    else {
        panic!("expected self-healing edit failure, got {error:?}");
    };
    assert!(message.contains("self-healing edit check failed"));
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("check-failed"));
    assert!(repair_attempts.is_empty());
    let check_output = check_output.expect("check output should be preserved");
    assert_eq!(check_output.command, "printf check-failed >&2; exit 7");
    assert_eq!(check_output.stdout, "");
    assert_eq!(check_output.stderr, "check-failed");
    assert_eq!(check_output.exit_code, 7);
    assert_eq!(
        std::fs::read_to_string(workspace.join("src/app.txt")).unwrap(),
        "one\ndeux\n"
    );
}

#[tokio::test]
async fn coding_session_self_healing_edit_requires_persistent_session() {
    let temp = tempfile::tempdir().unwrap();
    let mut session =
        CodingAgentSession::non_persistent(CodingAgentSessionOptions::new().with_cwd(temp.path()))
            .await
            .unwrap();

    let error = session
        .self_healing_edit(
            "src/app.txt",
            vec![SelfHealingEditReplacement::new("two", "deux")],
        )
        .await
        .unwrap_err();

    assert_eq!(error.code(), "unsupported_capability");
    assert!(
        error
            .to_string()
            .contains("self-healing edit requires a persistent Rust-native session"),
        "{error}"
    );
}

#[tokio::test]
async fn coding_session_self_healing_edit_failure_records_failed_operation() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    let sessions = temp.path().join("sessions");
    std::fs::create_dir_all(workspace.join("src")).unwrap();
    std::fs::write(workspace.join("src/app.txt"), "one\ntwo\n").unwrap();
    let mut session = CodingAgentSession::create(
        CodingAgentSessionOptions::new()
            .with_session_id("sess_self_healing_failure")
            .with_cwd(&workspace)
            .with_session_log_root(&sessions),
    )
    .await
    .unwrap();

    let error = session
        .self_healing_edit(
            "src/app.txt",
            vec![SelfHealingEditReplacement::new("", "deux")],
        )
        .await
        .unwrap_err();

    assert_eq!(
        std::fs::read_to_string(workspace.join("src/app.txt")).unwrap(),
        "one\ntwo\n"
    );
    assert!(
        error.to_string().contains("oldText must not be empty"),
        "{error}"
    );
    let event_log = std::fs::read_to_string(
        sessions
            .join("sess_self_healing_failure")
            .join("events.jsonl"),
    )
    .unwrap();
    assert!(
        event_log.contains(r#""operation":{"kind":"self_healing_edit"}"#),
        "{event_log}"
    );
    assert!(
        event_log.contains(r#""kind":"self_healing_edit.started""#),
        "{event_log}"
    );
    assert!(
        event_log.contains(r#""kind":"operation.failed""#),
        "{event_log}"
    );
    assert!(
        !event_log.contains(r#""kind":"self_healing_edit.completed""#),
        "{event_log}"
    );
}

#[tokio::test]
async fn coding_session_open_or_create_and_list_are_public() {
    let temp = tempfile::tempdir().unwrap();
    let options = CodingAgentSessionOptions::new()
        .with_session_id("sess_public_list")
        .with_session_log_root(temp.path());

    let created = CodingAgentSession::open_or_create(options.clone())
        .await
        .unwrap();
    let reopened = CodingAgentSession::open_or_create(options.clone())
        .await
        .unwrap();
    let summaries = CodingAgentSession::list(
        CodingAgentSessionOptions::new().with_session_log_root(temp.path()),
    )
    .unwrap();
    let _summary_type_name = std::any::type_name::<CodingAgentSessionSummary>();

    assert_eq!(created.view().session_id, "sess_public_list");
    assert_eq!(reopened.view().session_id, "sess_public_list");
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].session_id, "sess_public_list");
    assert_eq!(
        summaries[0].session_dir,
        temp.path().join("sess_public_list")
    );
    assert!(!summaries[0].created_at.is_empty());
    assert!(!summaries[0].updated_at.is_empty());
}
