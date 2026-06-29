use pi_ai::types::{Model, ModelCost, ModelInput};
use pi_coding_agent::api::{
    CliArgs, CliDiagnostic, CliDiagnosticSeverity, CliError, CliOutput, CliRunOptions,
    CodingAgentCapabilities, CodingAgentEvent, CodingAgentEventReceiver, CodingAgentSession,
    CodingAgentSessionOptions, CodingAgentSessionSummary, CodingAgentSessionView, CodingDiagnostic,
    CodingDiagnosticSeverity, CodingSessionError, PrintModeOptions, PromptInvocation,
    PromptTurnMode, PromptTurnOptions, PromptTurnOutcome, SessionMode, SessionPromptOptions,
    ToolFilter, builtin_tools, filter_tools, help_text, parse_args, render_diagnostics,
};

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

    let _session_prompt_type_name = std::any::type_name::<SessionPromptOptions>();

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

#[tokio::test]
async fn coding_session_public_api_symbols_are_importable() {
    let temp = tempfile::tempdir().unwrap();
    let options = CodingAgentSessionOptions::new()
        .with_session_id("sess_public_api")
        .with_session_log_root(temp.path())
        .with_session_path("sess_public_api");
    assert_eq!(options.session_id(), Some("sess_public_api"));
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
        }
    );

    let capabilities = session.capabilities();
    assert_eq!(
        capabilities,
        CodingAgentCapabilities {
            prompt: false,
            session_log: false,
            plugins: false,
        }
    );

    let mut receiver = session.subscribe();
    assert!(receiver.try_recv().unwrap().is_none());
    let _receiver_type_name = std::any::type_name::<CodingAgentEventReceiver>();

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
