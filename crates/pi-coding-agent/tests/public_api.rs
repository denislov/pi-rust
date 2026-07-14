mod support;

use std::collections::HashMap;
use std::fs;

use pi_agent_core::AgentResources;
use pi_ai::providers::faux::FauxProvider;
use pi_ai::types::{Model, ModelCost, ModelInput};
use pi_coding_agent::api::{
    AgentInvocationOptions, AgentInvocationOutcome, AgentProfile, AgentTeamMemberOutcome,
    AgentTeamOptions, AgentTeamOutcome, BranchSummaryReusePolicy, CapabilityStatus, CliArgs,
    CliDiagnostic, CliDiagnosticSeverity, CliError, CliOutput, CliRunOptions,
    CodingAgentAgentProductEvent, CodingAgentCapabilities, CodingAgentCapabilityProductEvent,
    CodingAgentClientConnection, CodingAgentClientId, CodingAgentControlId, CodingAgentControlKind,
    CodingAgentControlRejectionReason, CodingAgentDelegationEventContext,
    CodingAgentDelegationProductEvent, CodingAgentDetachOutcome, CodingAgentDiagnosticProductEvent,
    CodingAgentDraft, CodingAgentDraftId, CodingAgentDraftKind, CodingAgentLifecycleRejection,
    CodingAgentMessageProductEvent, CodingAgentOperation, CodingAgentOperationOutcome,
    CodingAgentOutcomeAcknowledgementId, CodingAgentPluginDiagnostic, CodingAgentPluginLoadOutcome,
    CodingAgentProductEvent, CodingAgentProductEventCapabilityRevocation,
    CodingAgentProductEventCheckOutput, CodingAgentProductEventDiagnostic,
    CodingAgentProductEventDurability, CodingAgentProductEventError, CodingAgentProductEventFamily,
    CodingAgentProductEventKind, CodingAgentProductEventProfileKind,
    CodingAgentProductEventReceiver, CodingAgentProductEventReplacement,
    CodingAgentProductEventTerminalOperation, CodingAgentProductEventTerminalOperationKind,
    CodingAgentProductEventTerminalStatus, CodingAgentProductEventUsage,
    CodingAgentProfileProductEvent, CodingAgentRuntimeProductEvent, CodingAgentSession,
    CodingAgentSessionExport, CodingAgentSessionExportItem, CodingAgentSessionOptions,
    CodingAgentSessionProductEvent, CodingAgentSessionSummary, CodingAgentSessionView,
    CodingAgentShutdownOutcome, CodingAgentSnapshot, CodingAgentSnapshotCursor,
    CodingAgentSubmittedEventDurability, CodingAgentSubmittedTerminalAnchor,
    CodingAgentTeamProductEvent, CodingAgentTerminalUncertainty, CodingAgentToolProductEvent,
    CodingAgentWorkflowProductEvent, CodingDiagnostic, CodingDiagnosticSeverity,
    CodingSessionError, ColorValue, CompactionProtocolResult, CompactionReason, ContextFile,
    DetectionConfidence, DetectionSource, ModelRotation, ModelRotationEntry,
    PendingDelegationConfirmation, PrintModeOptions, ProfileDiagnostic, ProfileId,
    PromptInvocation, PromptRunOptions, PromptTurnMode, PromptTurnOptions, PromptTurnOutcome,
    ProtocolDelegationFoldedBlock, ProtocolEvent, ProtocolSelfHealingEditCheckOutput,
    ProtocolSelfHealingEditReplacement, REQUIRED_TOKEN_KEYS, ResolveError, ResolvedColor,
    ResolvedTheme, ResourceLoadOptions, RpcCapabilities, RpcCapabilityStatus, RpcCommand,
    RpcDelegationCapabilityStatus, RpcDelegationRenderingMetadata, RpcResponse,
    RpcSelfHealingEditModelRepair, RpcSelfHealingEditReplacement, RpcSessionState,
    SelfHealingEditCheckOutput, SelfHealingEditDiagnostic, SelfHealingEditModelRepairOptions,
    SelfHealingEditOutcome, SelfHealingEditRepairAttempt, SelfHealingEditReplacement,
    SelfHealingEditRequest, SessionMode, StreamingBehavior, TeamProfile, TerminalTheme, ThemeBg,
    ThemeColor, ThemeExportColors, ThemeJson, ToolExecutionResult, ToolFilter,
    build_agent_resources, builtin_dark, builtin_tools, detect_terminal_background,
    discover_context_files, filter_tools, get_resolved_theme_colors, get_theme_export_colors,
    get_theme_for_rgb_color, help_text, is_light_theme, parse_args, parse_model_rotation,
    parse_osc11_background_color, render_diagnostics, resolve, resolve_resource_paths,
};
use support::{EnvGuard, ProviderGuard};

#[test]
fn lifecycle_values_are_exhaustive_and_importable() {
    let detach = [
        CodingAgentDetachOutcome::Detached,
        CodingAgentDetachOutcome::AlreadyDetached,
        CodingAgentDetachOutcome::StaleGeneration,
    ];
    let shutdown = [
        CodingAgentShutdownOutcome::ShutDown,
        CodingAgentShutdownOutcome::AlreadyShutDown,
    ];
    let rejections = [
        CodingAgentLifecycleRejection::Detached,
        CodingAgentLifecycleRejection::StaleGeneration,
        CodingAgentLifecycleRejection::RuntimeShutDown,
    ];
    assert_eq!(detach.len(), 3);
    assert_eq!(shutdown.len(), 2);
    assert_eq!(
        rejections.map(CodingAgentLifecycleRejection::code),
        ["detached", "stale_generation", "runtime_shut_down"]
    );

    let acknowledgement: CodingAgentOutcomeAcknowledgementId =
        serde_json::from_str(r#""outcome_1""#).unwrap();
    assert_eq!(acknowledgement.as_str(), "outcome_1");
    let anchors = [
        CodingAgentSubmittedTerminalAnchor::ProductEvent {
            sequence: 41,
            durability: CodingAgentSubmittedEventDurability::Durable,
        },
        CodingAgentSubmittedTerminalAnchor::OutcomeOnly { acknowledgement },
        CodingAgentSubmittedTerminalAnchor::TerminalUncertain {
            operation_id: "op_uncertain".into(),
            recovery: CodingAgentTerminalUncertainty::RecoveryRequired,
        },
    ];
    assert_eq!(anchors.len(), 3);
}

#[test]
fn lifecycle_serialization_is_stable_and_authority_free() {
    assert_eq!(
        serde_json::to_value(CodingAgentDetachOutcome::AlreadyDetached).unwrap(),
        serde_json::json!("already_detached")
    );
    assert_eq!(
        serde_json::to_value(CodingAgentShutdownOutcome::AlreadyShutDown).unwrap(),
        serde_json::json!("already_shut_down")
    );
    assert_eq!(
        serde_json::to_value(CodingAgentLifecycleRejection::RuntimeShutDown).unwrap(),
        serde_json::json!("runtime_shut_down")
    );

    let acknowledgement: CodingAgentOutcomeAcknowledgementId =
        serde_json::from_str(r#""ack_1""#).unwrap();
    let values = [
        serde_json::to_value(CodingAgentSubmittedTerminalAnchor::ProductEvent {
            sequence: 7,
            durability: CodingAgentSubmittedEventDurability::Uncertain,
        })
        .unwrap(),
        serde_json::to_value(CodingAgentSubmittedTerminalAnchor::OutcomeOnly { acknowledgement })
            .unwrap(),
        serde_json::to_value(CodingAgentSubmittedTerminalAnchor::TerminalUncertain {
            operation_id: "op_7".into(),
            recovery: CodingAgentTerminalUncertainty::RecoveryRequired,
        })
        .unwrap(),
    ];
    assert_eq!(
        values,
        [
            serde_json::json!({"kind":"product_event","sequence":7,"durability":"uncertain"}),
            serde_json::json!({"kind":"outcome_only","acknowledgement":"ack_1"}),
            serde_json::json!({"kind":"terminal_uncertain","operation_id":"op_7","recovery":"recovery_required"}),
        ]
    );
    let serialized = serde_json::to_string(&values).unwrap();
    for forbidden in [
        "coordinator",
        "generation",
        "signature",
        "session_id",
        "pending_session_write",
        "receiver",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "leaked {forbidden}: {serialized}"
        );
    }
}

#[test]
fn stable_api_signature_closure_is_importable() {
    fn name<T>() -> &'static str {
        std::any::type_name::<T>()
    }

    let prompt = || PromptTurnOptions::new(PromptInvocation::Text("test".into()));
    let operations = [
        CodingAgentOperation::Prompt(prompt()),
        CodingAgentOperation::Compact(prompt()),
        CodingAgentOperation::BranchSummary {
            options: prompt(),
            source_leaf_id: "source".into(),
            target_leaf_id: "target".into(),
            custom_instructions: None,
            reuse: BranchSummaryReusePolicy::ReuseExisting,
        },
        CodingAgentOperation::SelfHealingEdit(SelfHealingEditRequest::new(
            "src/lib.rs",
            vec![SelfHealingEditReplacement::new("old", "new")],
        )),
        CodingAgentOperation::InvokeAgent(AgentInvocationOptions::new(
            "reviewer",
            "review",
            prompt(),
        )),
        CodingAgentOperation::InvokeTeam(AgentTeamOptions::new("review", "review", prompt())),
        CodingAgentOperation::PluginLoad,
        CodingAgentOperation::PluginCommand {
            command_id: "plugin.command".into(),
            args: serde_json::Value::Null,
        },
        CodingAgentOperation::SetDefaultAgentProfile {
            profile_id: ProfileId::from("reviewer"),
        },
        CodingAgentOperation::ApproveDelegation {
            operation_id: "operation".into(),
            tool_call_id: "tool".into(),
        },
        CodingAgentOperation::RejectDelegation {
            operation_id: "operation".into(),
            tool_call_id: "tool".into(),
            reason: "rejected".into(),
        },
        CodingAgentOperation::ForkSession {
            target_leaf_id: None,
        },
        CodingAgentOperation::SwitchActiveLeaf {
            target_leaf_id: "leaf".into(),
        },
        CodingAgentOperation::ExportCurrent,
        CodingAgentOperation::ExportCurrentHtml("session.html".into()),
    ];
    assert_eq!(operations.len(), 15);

    for type_name in [
        name::<CodingAgentOperationOutcome>(),
        name::<PromptTurnOutcome>(),
        name::<SelfHealingEditOutcome>(),
        name::<AgentInvocationOutcome>(),
        name::<AgentTeamOutcome>(),
        name::<AgentTeamMemberOutcome>(),
        name::<CodingAgentPluginLoadOutcome>(),
        name::<CodingAgentPluginDiagnostic>(),
        name::<CodingAgentSessionExport>(),
        name::<CodingSessionError>(),
        name::<CodingAgentSessionOptions>(),
        name::<CodingAgentSessionSummary>(),
        name::<CodingAgentSessionView>(),
        name::<CodingAgentSnapshot>(),
        name::<CodingAgentSnapshotCursor>(),
        name::<CodingAgentCapabilities>(),
        name::<CapabilityStatus>(),
        name::<CodingAgentProductEvent>(),
        name::<CodingAgentProductEventReceiver>(),
        name::<CodingAgentClientId>(),
        name::<CodingAgentClientConnection>(),
        name::<AgentProfile>(),
        name::<TeamProfile>(),
        name::<ProfileDiagnostic>(),
        name::<PendingDelegationConfirmation>(),
    ] {
        assert!(type_name.starts_with("pi_coding_agent::"), "{type_name}");
    }

    let _create = CodingAgentSession::create;
    let _open = CodingAgentSession::open;
    let _open_or_create = CodingAgentSession::open_or_create;
    let _non_persistent = CodingAgentSession::non_persistent;
    let _list = CodingAgentSession::list;
    let _run = CodingAgentSession::run;
    let _snapshot = CodingAgentSession::snapshot;
    let _view = CodingAgentSession::view;
    let _capabilities = CodingAgentSession::capabilities;
    let _subscribe = CodingAgentSession::subscribe_product_events_public;
    let _connect = CodingAgentSession::connect;
    let _profiles = CodingAgentSession::agent_profiles;
    let _teams = CodingAgentSession::team_profiles;
    let _diagnostics = CodingAgentSession::profile_diagnostics;
    let _pending = CodingAgentSession::pending_delegation_confirmations;
    let _canonical_dispatch = CodingAgentSession::run;
}

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
fn public_api_tests_use_stable_facade_imports() {
    let source = include_str!("public_api.rs");
    let forbidden_import = "use pi_coding_agent::".to_owned() + "{";
    assert!(
        !source
            .lines()
            .any(|line| line.trim_start().starts_with(&forbidden_import)),
        "public API tests should import stable symbols through pi_coding_agent::api"
    );
}

#[test]
fn canonical_operation_runtime_variants_are_public() {
    let branch_summary = |reuse| CodingAgentOperation::BranchSummary {
        options: PromptTurnOptions::new(PromptInvocation::Text("summarize".into())),
        source_leaf_id: "leaf_source".into(),
        target_leaf_id: "leaf_target".into(),
        custom_instructions: None,
        reuse,
    };
    let _ = branch_summary(BranchSummaryReusePolicy::AlwaysCreate);
    let _ = branch_summary(BranchSummaryReusePolicy::ReuseExisting);
    let _ = CodingAgentOperation::PluginLoad;
    let _ = CodingAgentOperation::PluginCommand {
        command_id: "plugin.command".into(),
        args: serde_json::json!({"value": 1}),
    };
    let _ = CodingAgentOperation::SetDefaultAgentProfile {
        profile_id: ProfileId::from("reviewer"),
    };
    let _ = CodingAgentOperation::ApproveDelegation {
        operation_id: "op_parent".into(),
        tool_call_id: "tool_delegate".into(),
    };
    let _ = CodingAgentOperation::RejectDelegation {
        operation_id: "op_parent".into(),
        tool_call_id: "tool_delegate".into(),
        reason: "not now".into(),
    };
    let _ = CodingAgentOperation::ForkSession {
        target_leaf_id: Some("leaf_target".into()),
    };
    let _ = CodingAgentOperation::SwitchActiveLeaf {
        target_leaf_id: "leaf_target".into(),
    };
    let plugin_load = CodingAgentPluginLoadOutcome {
        loaded_plugin_ids: vec!["sample".into()],
        diagnostics: vec![CodingAgentPluginDiagnostic {
            plugin_id: Some("sample".into()),
            message: "loaded".into(),
        }],
        capability_changed: true,
    };
    let _ = CodingAgentOperationOutcome::PluginLoad(plugin_load);
    let _ = CodingAgentOperationOutcome::PluginCommand("ok".into());
    let _ = CodingAgentOperationOutcome::DefaultAgentProfileChanged;
    let _ = CodingAgentOperationOutcome::DelegationApproved;
    let _ = CodingAgentOperationOutcome::DelegationRejected;
    let _ = CodingAgentOperationOutcome::SessionForked;
    let _ = CodingAgentOperationOutcome::ActiveLeafSwitched;
}

#[tokio::test]
async fn coding_session_run_public_operation_facade_is_importable() {
    let temp = tempfile::tempdir().unwrap();
    let mut session = CodingAgentSession::create(
        CodingAgentSessionOptions::new()
            .with_session_id("sess_public_run")
            .with_session_log_root(temp.path()),
    )
    .await
    .unwrap();
    let outcome = session
        .run(CodingAgentOperation::ExportCurrent)
        .await
        .unwrap();

    assert!(matches!(outcome, CodingAgentOperationOutcome::Export(_)));
}

#[tokio::test]
async fn coding_session_run_dispatches_public_runtime_operations() {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    let global = temp.path().join("global");
    std::fs::create_dir_all(project.join(".pi-rust/plugins")).unwrap();
    std::fs::create_dir_all(global.join("plugins")).unwrap();
    let _env = EnvGuard::with_pi_rust_dir(&global);
    let mut session =
        CodingAgentSession::non_persistent(CodingAgentSessionOptions::new().with_cwd(&project))
            .await
            .unwrap();

    let plugin_load = session.run(CodingAgentOperation::PluginLoad).await.unwrap();
    let CodingAgentOperationOutcome::PluginLoad(plugin_load) = plugin_load else {
        panic!("plugin load should return the public plugin-load projection")
    };
    assert!(plugin_load.loaded_plugin_ids.is_empty());
    assert!(plugin_load.diagnostics.is_empty());
    assert!(!plugin_load.capability_changed);

    let profile_change = session
        .run(CodingAgentOperation::SetDefaultAgentProfile {
            profile_id: ProfileId::from("reviewer"),
        })
        .await
        .unwrap();
    assert!(matches!(
        profile_change,
        CodingAgentOperationOutcome::DefaultAgentProfileChanged
    ));
    assert_eq!(
        session.snapshot().session.default_agent_profile_id,
        ProfileId::from("reviewer")
    );

    let plugin_command_error = session
        .run(CodingAgentOperation::PluginCommand {
            command_id: "missing.command".into(),
            args: serde_json::json!({}),
        })
        .await
        .unwrap_err();
    assert_eq!(plugin_command_error.code(), "unsupported_capability");

    let fork_error = session
        .run(CodingAgentOperation::ForkSession {
            target_leaf_id: None,
        })
        .await
        .unwrap_err();
    assert_eq!(fork_error.code(), "unsupported_capability");
    assert!(
        fork_error
            .to_string()
            .contains("fork requires a persistent Rust-native session")
    );

    let switch_error = session
        .run(CodingAgentOperation::SwitchActiveLeaf {
            target_leaf_id: "leaf_target".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(switch_error.code(), "unsupported_capability");
    assert!(
        switch_error
            .to_string()
            .contains("active leaf navigation requires a persistent Rust-native session")
    );
}

#[tokio::test]
async fn coding_session_snapshot_public_facade_is_importable() {
    let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
        .await
        .unwrap();

    let snapshot: CodingAgentSnapshot = session.snapshot();
    let session_id = snapshot.session.session_id.clone();
    assert!(session_id.starts_with("runtime_sess_"));
    assert_eq!(snapshot.cursor.last_event_sequence, 0);
    let _cursor_type_name = std::any::type_name::<CodingAgentSnapshotCursor>();

    let client_id = CodingAgentClientId::new("public-client");
    let connected: CodingAgentClientConnection = session.connect(client_id.clone()).unwrap();
    assert_eq!(connected.client_id, client_id);
    assert_eq!(connected.snapshot.session.session_id, session_id);
}

#[tokio::test]
async fn client_connection_state_takeover_ack_and_drafts_are_generation_scoped() {
    let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
        .await
        .unwrap();
    let first = session
        .connect(CodingAgentClientId::new("stateful-client"))
        .unwrap();
    first
        .set_prompt_draft(
            pi_coding_agent::api::CodingAgentDraftId("draft-1".into()),
            "hello",
        )
        .unwrap();
    let snapshot = first.state().unwrap();
    assert_eq!(snapshot.drafts.len(), 1);
    assert_eq!(snapshot.drafts[0].id.0, "draft-1");
    assert_eq!(first.acknowledge(0).unwrap(), 0);

    let second = session
        .connect(CodingAgentClientId::new("stateful-client"))
        .unwrap();
    assert!(second.generation.0 > first.generation.0);
    assert_eq!(second.state().unwrap().drafts[0].text, "hello");
    assert_eq!(first.state().unwrap_err().code(), "stale_generation");
    assert_eq!(first.acknowledge(1).unwrap_err().code(), "stale_generation");
}

#[tokio::test]
async fn detach_outcomes_and_lifecycle_rejection_paths_are_typed_and_preserve_state() {
    let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
        .await
        .unwrap();
    let id = CodingAgentClientId::new("detach-public-client");
    let first = session.connect(id.clone()).unwrap();
    first
        .set_prompt_draft(CodingAgentDraftId("preserved-prompt".into()), "hello")
        .unwrap();
    assert_eq!(first.acknowledge(7).unwrap(), 7);

    assert_eq!(first.detach().unwrap(), CodingAgentDetachOutcome::Detached);
    assert_eq!(
        first.detach().unwrap(),
        CodingAgentDetachOutcome::AlreadyDetached
    );
    for error in [
        first.state().unwrap_err(),
        first.acknowledge(8).unwrap_err(),
        first
            .acknowledge_outcome(serde_json::from_str(r#""outcome-detached""#).unwrap())
            .unwrap_err(),
        first
            .set_prompt_draft(CodingAgentDraftId("rejected".into()), "rejected")
            .unwrap_err(),
        first.reconnect(7).unwrap_err(),
    ] {
        assert_eq!(
            error,
            CodingSessionError::Lifecycle {
                reason: CodingAgentLifecycleRejection::Detached
            }
        );
    }
    assert_eq!(
        first
            .enqueue_control_draft(CodingAgentDraft {
                id: CodingAgentDraftId("rejected-control".into()),
                kind: CodingAgentDraftKind::Steer,
                text: "rejected".into(),
            })
            .unwrap_err(),
        pi_coding_agent::api::CodingAgentMutationRejection::Detached
    );
    assert_eq!(
        first
            .prompt_control("active-prompt")
            .abort(CodingAgentControlId("abort-detached".into()), "stop")
            .unwrap_err()
            .reason,
        CodingAgentControlRejectionReason::Detached
    );

    let prompt = CodingAgentOperation::Prompt(PromptTurnOptions::new(PromptInvocation::Text(
        "hello".into(),
    )));
    assert_eq!(
        first
            .prepare_submission(
                &mut session,
                CodingAgentDraftId("preserved-prompt".into()),
                &prompt,
            )
            .unwrap_err(),
        CodingSessionError::Lifecycle {
            reason: CodingAgentLifecycleRejection::Detached
        }
    );

    let second = session.connect(id).unwrap();
    assert_eq!(
        first.detach().unwrap(),
        CodingAgentDetachOutcome::StaleGeneration
    );
    assert_eq!(second.state().unwrap().drafts[0].text, "hello");
    assert_eq!(second.acknowledge(3).unwrap(), 7);
    assert_eq!(
        first.state().unwrap_err(),
        CodingSessionError::Lifecycle {
            reason: CodingAgentLifecycleRejection::StaleGeneration
        }
    );
}

#[tokio::test]
async fn detach_wakes_a_blocked_reconnect_receiver_without_leaking_an_event() {
    let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
        .await
        .unwrap();
    let connection = session
        .connect(CodingAgentClientId::new("detach-receiver-client"))
        .unwrap();
    let pi_coding_agent::api::CodingAgentReconnect::Replayed { mut receiver, .. } =
        connection.reconnect(0).unwrap()
    else {
        panic!("empty retained stream should establish a live receiver")
    };
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let blocked = tokio::spawn(async move {
        ready_tx.send(()).unwrap();
        receiver.recv().await
    });
    ready_rx.await.unwrap();

    assert_eq!(
        connection.detach().unwrap(),
        CodingAgentDetachOutcome::Detached
    );
    let error = tokio::time::timeout(std::time::Duration::from_secs(2), blocked)
        .await
        .expect("detach must wake a blocked receiver")
        .unwrap()
        .unwrap_err();
    assert_eq!(
        error,
        CodingSessionError::Lifecycle {
            reason: CodingAgentLifecycleRejection::Detached
        }
    );
}

#[tokio::test]
async fn scoped_control_authorization_and_rejected_drafts_are_typed_and_preserved() {
    let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
        .await
        .unwrap();
    let first = session
        .connect(CodingAgentClientId::new("control-client"))
        .unwrap();
    first
        .enqueue_control_draft(CodingAgentDraft {
            id: CodingAgentDraftId("draft-control".into()),
            kind: CodingAgentDraftKind::Steer,
            text: "steer me".into(),
        })
        .unwrap();
    let control = first.prompt_control("prompt-not-running");
    assert_eq!(
        control
            .abort(CodingAgentControlId("abort-1".into()), "cancel")
            .unwrap_err()
            .reason,
        CodingAgentControlRejectionReason::TargetNotRunning
    );
    assert_eq!(
        control
            .steer(CodingAgentControlId("".into()), "")
            .unwrap_err()
            .reason,
        CodingAgentControlRejectionReason::InvalidInput
    );
    assert_eq!(
        control
            .steer_draft(CodingAgentDraftId("draft-control".into()))
            .unwrap_err()
            .reason,
        CodingAgentControlRejectionReason::TargetNotRunning
    );
    assert_eq!(first.state().unwrap().drafts[0].id.0, "draft-control");

    let takeover = session
        .connect(CodingAgentClientId::new("control-client"))
        .unwrap();
    let stale = first.prompt_control("prompt-not-running");
    assert_eq!(
        stale
            .abort(CodingAgentControlId("abort-stale".into()), "cancel")
            .unwrap_err()
            .reason,
        CodingAgentControlRejectionReason::StaleGeneration
    );
    assert_eq!(takeover.state().unwrap().drafts[0].text, "steer me");
    let _ = CodingAgentControlKind::Abort;
}

#[tokio::test]
async fn client_connection_replays_unacknowledged_delivery_and_ack_is_explicit() {
    let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
        .await
        .unwrap();
    let connection = session
        .connect(CodingAgentClientId::new("replay-client"))
        .unwrap();
    let recovery = connection.reconnect(0).unwrap();
    match recovery {
        pi_coding_agent::api::CodingAgentReconnect::Replayed { events, cursor, .. } => {
            assert!(events.is_empty());
            assert_eq!(cursor.last_event_sequence, 0);
        }
        other => panic!("unexpected recovery: {other:?}"),
    }
    assert_eq!(connection.acknowledge(0).unwrap(), 0);
}

#[tokio::test]
async fn submission_lease_drop_preserves_draft_and_releases_exclusivity() {
    let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
        .await
        .unwrap();
    let connection = session
        .connect(CodingAgentClientId::new("lease-client"))
        .unwrap();
    connection
        .set_prompt_draft(
            pi_coding_agent::api::CodingAgentDraftId("draft-lease".into()),
            "tracked prompt",
        )
        .unwrap();
    let operation = CodingAgentOperation::Prompt(PromptTurnOptions::new(PromptInvocation::Text(
        "tracked prompt".into(),
    )));
    let lease = connection
        .prepare_submission(
            &mut session,
            pi_coding_agent::api::CodingAgentDraftId("draft-lease".into()),
            &operation,
        )
        .unwrap();
    assert_eq!(
        connection
            .prepare_submission(
                &mut session,
                pi_coding_agent::api::CodingAgentDraftId("draft-lease".into()),
                &operation,
            )
            .unwrap_err()
            .code(),
        "submission_preparation_busy"
    );
    drop(lease);
    assert_eq!(connection.state().unwrap().drafts[0].text, "tracked prompt");
    let replacement = connection
        .prepare_submission(
            &mut session,
            pi_coding_agent::api::CodingAgentDraftId("draft-lease".into()),
            &operation,
        )
        .unwrap();
    drop(replacement);
}

#[tokio::test]
async fn submission_lease_canonical_run_clears_draft_and_records_terminal() {
    let api = "public-api-submission-lease";
    let _provider_guard = ProviderGuard::register(
        api,
        std::sync::Arc::new(FauxProvider::simple_text("tracked response")),
    );
    let prompt = PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
        prompt: "tracked prompt".into(),
        model: model(api),
        api_key: None,
        auth_diagnostics: Vec::new(),
        system_prompt: Some("test".into()),
        max_turns: Some(1),
        tools: Vec::new(),
        register_builtins: false,
        session: None,
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("tracked prompt".into()),
    });
    let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
        .await
        .unwrap();
    let connection = session
        .connect(CodingAgentClientId::new("lease-run-client"))
        .unwrap();
    let draft_id = pi_coding_agent::api::CodingAgentDraftId("draft-run".into());
    connection
        .set_prompt_draft(draft_id.clone(), "tracked prompt")
        .unwrap();
    let operation = CodingAgentOperation::Prompt(prompt);
    let lease = connection
        .prepare_submission(&mut session, draft_id, &operation)
        .unwrap();

    let outcome = session.run(operation).await.unwrap();
    assert!(matches!(outcome, CodingAgentOperationOutcome::Prompt(_)));
    drop(lease);
    let state = connection.state().unwrap();
    assert!(state.drafts.is_empty());
    let submitted = state.submitted_operation.expect("tracked terminal state");
    assert_eq!(submitted.kind, "prompt");
    assert!(matches!(
        submitted.status,
        pi_coding_agent::api::CodingAgentSubmittedOperationStatus::Terminal { .. }
    ));

    let pi_coding_agent::api::CodingAgentReconnect::Replayed { events, cursor, .. } =
        connection.reconnect(0).unwrap()
    else {
        panic!("terminal event must remain retained")
    };
    let terminal_sequence = events
        .iter()
        .filter(|event| event.terminal_status().is_some())
        .map(CodingAgentProductEvent::sequence)
        .max()
        .expect("tracked prompt terminal event");
    assert!(connection.state().unwrap().submitted_operation.is_some());
    assert_eq!(
        connection.acknowledge(terminal_sequence).unwrap(),
        terminal_sequence
    );
    assert!(connection.state().unwrap().submitted_operation.is_none());
    assert!(cursor.last_event_sequence >= terminal_sequence);
}

#[test]
fn client_reconnect_source_uses_only_the_atomic_replay_live_boundary() {
    let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/coding_session");
    let connection = fs::read_to_string(source_root.join("public_projection.rs")).unwrap();
    let coordinator = fs::read_to_string(source_root.join("snapshot_coordinator.rs")).unwrap();
    let reconnect = connection
        .split("pub fn reconnect(")
        .nth(1)
        .and_then(|source| source.split("pub fn set_prompt_draft(").next())
        .unwrap();

    assert!(reconnect.contains("recovery_boundary_after_for_client"));
    assert!(reconnect.contains("receiver: CodingAgentReconnectReceiver"));
    assert!(!reconnect.contains("retained_events_after"));
    assert!(!coordinator.contains("fn retained_events_after"));
}

#[test]
fn client_errors_have_stable_non_overlapping_codes() {
    assert_eq!(
        CodingSessionError::StaleClientConnection {
            client_id: "c".into()
        }
        .code(),
        "stale_client_connection"
    );
    assert_eq!(
        CodingSessionError::SubmissionPreparationBusy.code(),
        "submission_preparation_busy"
    );
    assert_eq!(
        CodingSessionError::ClientCapacityExceeded { limit: 64 }.code(),
        "client_capacity_exceeded"
    );
}

#[test]
fn legacy_run_connection_surface_has_no_dispatcher() {
    let source = fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/coding_session/public_projection.rs"),
    )
    .unwrap();
    let connection = source
        .split("impl CodingAgentClientConnection")
        .nth(1)
        .unwrap();
    assert!(!connection.contains("pub async fn run("));
    assert!(!connection.contains("pub async fn submit("));
    assert!(connection.contains("prepare_submission("));
}

#[test]
fn snapshot_topology_has_one_registry_and_zero_authority_client_facade() {
    let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/coding_session");
    let coordinator = fs::read_to_string(source_root.join("snapshot_coordinator.rs")).unwrap();
    let client_service = fs::read_to_string(source_root.join("client_service.rs")).unwrap();
    let event_service = fs::read_to_string(source_root.join("event_service.rs")).unwrap();

    assert_eq!(coordinator.matches("clients: HashMap<").count(), 1);
    assert!(coordinator.contains("pub(crate) struct SnapshotState"));
    assert!(coordinator.contains("retained_product_events: VecDeque<ProductEvent>"));
    assert!(coordinator.contains("pub(crate) projection: Option<SnapshotProjection>"));
    assert!(client_service.contains("coordinator: Arc<SnapshotCoordinator>"));
    assert!(!client_service.contains("HashMap<"));
    assert!(!client_service.contains("Mutex<"));
    assert!(!event_service.contains("EventPublicationState"));
    assert!(!event_service.contains("publication_state"));
}

#[test]
fn snapshot_writers_1_startup_drain_releases_marker_lock_before_projection() {
    let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/coding_session");
    let owner = fs::read_to_string(source_root.join("mod.rs")).unwrap();
    let take = owner.find("std::mem::take(&mut *markers)").unwrap();
    let project = owner.find("mark_recovery_projected()").unwrap();
    let emit = owner.find("emit_operation_recovered").unwrap();
    assert!(take < project && project < emit);
}

#[test]
fn snapshot_writers_2_operation_guard_releases_owner_lock_before_projection_clear() {
    let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/coding_session");
    let operation_control = fs::read_to_string(source_root.join("operation_control.rs")).unwrap();
    let drop_lock = operation_control.rfind("drop(active);").unwrap();
    let clear_projection = operation_control
        .find("set_active_operation(None)")
        .unwrap();
    assert!(drop_lock < clear_projection);
}

#[tokio::test]
async fn snapshot_writers_3_capability_install_is_atomic_and_bounded() {
    let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
        .await
        .unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        session.run(CodingAgentOperation::SetDefaultAgentProfile {
            profile_id: ProfileId::from("snapshot-writer"),
        }),
    )
    .await
    .expect("capability writer must not deadlock")
    .unwrap();

    let snapshot = session.snapshot();
    assert_eq!(
        snapshot.session.default_agent_profile_id,
        ProfileId::from("snapshot-writer")
    );
    assert_eq!(snapshot.cursor.capability_generation, 2);
    assert_eq!(snapshot.active_operation, None);
    assert!(snapshot.cursor.last_event_sequence >= 2);
}

#[test]
fn snapshot_writers_4_navigation_refreshes_projection_before_publication() {
    let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/coding_session");
    let owner = fs::read_to_string(source_root.join("mod.rs")).unwrap();
    let refresh = owner.find("self.refresh_snapshot_projection();").unwrap();
    let publish = owner
        .find("emit_session_opened(forked_session_id)")
        .unwrap();
    assert!(refresh < publish);
}

#[test]
fn snapshot_writers_5_event_commit_releases_coordinator_before_broadcast() {
    let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/coding_session");
    let event_service = fs::read_to_string(source_root.join("event_service.rs")).unwrap();
    let drop_before_send = event_service.find("drop(state);").unwrap();
    let broadcast_send = event_service.find("self.product_sender.send").unwrap();
    assert!(drop_before_send < broadcast_send);
}

#[test]
fn snapshot_writers_6_client_mutations_return_owned_snapshots_after_release() {
    let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/coding_session");
    let coordinator = fs::read_to_string(source_root.join("snapshot_coordinator.rs")).unwrap();
    let client_service = fs::read_to_string(source_root.join("client_service.rs")).unwrap();
    assert!(coordinator.contains("pub(crate) fn client_snapshot("));
    assert!(coordinator.contains("let projection = state"));
    assert!(client_service.contains("self.coordinator.client_snapshot(handle)"));
}

#[test]
fn coding_session_product_event_subscription_public_facade_is_importable() {
    let _event_type_name = std::any::type_name::<CodingAgentProductEvent>();
    let _receiver_type_name = std::any::type_name::<CodingAgentProductEventReceiver>();

    let contract_types = [
        std::any::type_name::<CodingAgentProductEventKind>(),
        std::any::type_name::<CodingAgentProductEventFamily>(),
        std::any::type_name::<CodingAgentProductEventDurability>(),
        std::any::type_name::<CodingAgentProductEventTerminalStatus>(),
        std::any::type_name::<CodingAgentProductEventTerminalOperation>(),
        std::any::type_name::<CodingAgentProductEventTerminalOperationKind>(),
        std::any::type_name::<CodingAgentProductEventError>(),
        std::any::type_name::<CodingAgentProductEventUsage>(),
        std::any::type_name::<CodingAgentProductEventReplacement>(),
        std::any::type_name::<CodingAgentProductEventDiagnostic>(),
        std::any::type_name::<CodingAgentProductEventCheckOutput>(),
        std::any::type_name::<CodingAgentProductEventProfileKind>(),
        std::any::type_name::<CodingAgentProductEventCapabilityRevocation>(),
        std::any::type_name::<CodingAgentDelegationEventContext>(),
    ];
    assert!(
        contract_types
            .iter()
            .all(|name| name.starts_with("pi_coding_agent::"))
    );

    let diagnostic =
        CodingAgentProductEventKind::Diagnostic(CodingAgentDiagnosticProductEvent::Diagnostic {
            operation_id: None,
            message: "ready".into(),
        });
    assert_eq!(
        typed_event_family(&diagnostic),
        CodingAgentProductEventFamily::Diagnostic
    );
    assert_eq!(diagnostic.as_str(), "diagnostic");
    assert_eq!(
        serde_json::to_value(&diagnostic).unwrap()["family"],
        "diagnostic"
    );
}

fn typed_event_family(event: &CodingAgentProductEventKind) -> CodingAgentProductEventFamily {
    match event {
        CodingAgentProductEventKind::Session(value) => {
            let _: &CodingAgentSessionProductEvent = value;
            CodingAgentProductEventFamily::Session
        }
        CodingAgentProductEventKind::Profile(value) => {
            let _: &CodingAgentProfileProductEvent = value;
            CodingAgentProductEventFamily::Profile
        }
        CodingAgentProductEventKind::Agent(value) => {
            let _: &CodingAgentAgentProductEvent = value;
            CodingAgentProductEventFamily::Agent
        }
        CodingAgentProductEventKind::Team(value) => {
            let _: &CodingAgentTeamProductEvent = value;
            CodingAgentProductEventFamily::Team
        }
        CodingAgentProductEventKind::Message(value) => {
            let _: &CodingAgentMessageProductEvent = value;
            CodingAgentProductEventFamily::Message
        }
        CodingAgentProductEventKind::Tool(value) => {
            let _: &CodingAgentToolProductEvent = value;
            CodingAgentProductEventFamily::Tool
        }
        CodingAgentProductEventKind::Runtime(value) => {
            let _: &CodingAgentRuntimeProductEvent = value;
            CodingAgentProductEventFamily::Runtime
        }
        CodingAgentProductEventKind::Delegation(value) => {
            let _: &CodingAgentDelegationProductEvent = value;
            CodingAgentProductEventFamily::Delegation
        }
        CodingAgentProductEventKind::Workflow(value) => {
            let _: &CodingAgentWorkflowProductEvent = value;
            CodingAgentProductEventFamily::Workflow
        }
        CodingAgentProductEventKind::Diagnostic(value) => {
            let _: &CodingAgentDiagnosticProductEvent = value;
            CodingAgentProductEventFamily::Diagnostic
        }
        CodingAgentProductEventKind::Capability(value) => {
            let _: &CodingAgentCapabilityProductEvent = value;
            CodingAgentProductEventFamily::Capability
        }
    }
}

#[allow(dead_code)]
fn typed_product_event_family(event: &CodingAgentProductEvent) -> CodingAgentProductEventFamily {
    event.family()
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

    let mut receiver = session.subscribe_product_events_public();
    assert!(receiver.try_recv().unwrap().is_none());
    let _receiver_type_name = std::any::type_name::<CodingAgentProductEventReceiver>();
    let _export_type_name = std::any::type_name::<CodingAgentSessionExport>();
    let _export_item_type_name = std::any::type_name::<CodingAgentSessionExportItem>();
    let _pending_delegation_type_name = std::any::type_name::<PendingDelegationConfirmation>();

    let error = CodingSessionError::UnsupportedCapability {
        capability: "prompt".into(),
    };
    assert_eq!(error.code(), "unsupported_capability");
    assert_eq!(error.to_string(), "unsupported capability: prompt");

    let prompt_options = PromptTurnOptions::new(PromptInvocation::Text("hello".into()))
        .with_mode(PromptTurnMode::Print);
    assert!(matches!(
        prompt_options.invocation(),
        PromptInvocation::Text(text) if text == "hello"
    ));
    assert_eq!(prompt_options.mode(), PromptTurnMode::Print);

    let prompt_error = session
        .run(CodingAgentOperation::Prompt(prompt_options))
        .await
        .unwrap_err();
    assert_eq!(prompt_error.code(), "config");
    assert!(prompt_error.to_string().contains("runtime snapshot"));

    let branch_summary_error = session
        .run(CodingAgentOperation::BranchSummary {
            options: PromptTurnOptions::new(PromptInvocation::Text(String::new()))
                .with_mode(PromptTurnMode::Print),
            source_leaf_id: "leaf_abandoned".into(),
            target_leaf_id: "leaf_target".into(),
            custom_instructions: None,
            reuse: BranchSummaryReusePolicy::AlwaysCreate,
        })
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
        .run(CodingAgentOperation::SelfHealingEdit(
            SelfHealingEditRequest::new(
                "src/app.txt",
                vec![SelfHealingEditReplacement::new("two", "deux")],
            ),
        ))
        .await
        .unwrap();
    let CodingAgentOperationOutcome::SelfHealingEdit(outcome) = outcome else {
        panic!("self-healing operation returned another outcome")
    };

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
        .run(CodingAgentOperation::SelfHealingEdit(
            SelfHealingEditRequest::new(
                "src/app.txt",
                vec![SelfHealingEditReplacement::new("two", "deux")],
            )
            .with_check_command("printf check-ok"),
        ))
        .await
        .unwrap();
    let CodingAgentOperationOutcome::SelfHealingEdit(outcome) = outcome else {
        panic!("self-healing operation returned another outcome")
    };

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
        .run(CodingAgentOperation::SelfHealingEdit(
            SelfHealingEditRequest::new(
                "src/app.txt",
                vec![SelfHealingEditReplacement::new("two", "deux")],
            )
            .with_check_command("grep -q dos src/app.txt")
            .with_repair_attempts(vec![vec![SelfHealingEditReplacement::new("deux", "dos")]]),
        ))
        .await
        .unwrap();
    let CodingAgentOperationOutcome::SelfHealingEdit(outcome) = outcome else {
        panic!("self-healing operation returned another outcome")
    };

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
        auth_diagnostics: Vec::new(),
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
    let mut events = session.subscribe_product_events_public();

    let outcome = session
        .run(CodingAgentOperation::SelfHealingEdit(
            SelfHealingEditRequest::new(
                "src/app.txt",
                vec![SelfHealingEditReplacement::new("two", "deux")],
            )
            .with_check_command("grep -q dos src/app.txt")
            .with_model_repair(SelfHealingEditModelRepairOptions::new(repair_options)),
        ))
        .await
        .unwrap();
    let CodingAgentOperationOutcome::SelfHealingEdit(outcome) = outcome else {
        panic!("self-healing operation returned another outcome")
    };

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
            event.event(),
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::SelfHealingEditStarted {
                    path,
                    replacements: 1,
                    ..
                }
            ) if path == "src/app.txt"
        )),
        "{emitted_events:#?}"
    );
    let repair_event_count = emitted_events
        .iter()
        .filter(|event| {
            matches!(
                event.event(),
                CodingAgentProductEventKind::Workflow(
                    CodingAgentWorkflowProductEvent::SelfHealingEditRepairAttempted { .. }
                )
            )
        })
        .count();
    assert_eq!(repair_event_count, 1, "{emitted_events:#?}");
    assert!(
        emitted_events.iter().any(|event| matches!(
            event.event(),
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::SelfHealingEditRepairAttempted {
                    path,
                    attempt: 1,
                    replacements,
                    check_output: Some(check_output),
                    ..
                }
            ) if path == "src/app.txt"
                && replacements[0].old_text == "deux"
                && replacements[0].new_text == "dos"
                && check_output.exit_code == 0
        )),
        "{emitted_events:#?}"
    );
    assert!(
        emitted_events.iter().any(|event| matches!(
            event.event(),
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::SelfHealingEditCompleted {
                    path,
                    attempts: 2,
                    check_output: Some(check_output),
                    ..
                }
            ) if path == "src/app.txt" && check_output.exit_code == 0
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
        auth_diagnostics: Vec::new(),
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
        .run(CodingAgentOperation::SelfHealingEdit(
            SelfHealingEditRequest::new(
                "src/app.txt",
                vec![SelfHealingEditReplacement::new("two", "deux")],
            )
            .with_check_command("grep -q dos src/app.txt")
            .with_model_repair(SelfHealingEditModelRepairOptions::new(repair_options)),
        ))
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
        .run(CodingAgentOperation::SelfHealingEdit(
            SelfHealingEditRequest::new(
                "src/app.txt",
                vec![SelfHealingEditReplacement::new("two", "deux")],
            )
            .with_check_command("printf check-failed >&2; exit 7"),
        ))
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
        .run(CodingAgentOperation::SelfHealingEdit(
            SelfHealingEditRequest::new(
                "src/app.txt",
                vec![SelfHealingEditReplacement::new("two", "deux")],
            ),
        ))
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
        .run(CodingAgentOperation::SelfHealingEdit(
            SelfHealingEditRequest::new(
                "src/app.txt",
                vec![SelfHealingEditReplacement::new("", "deux")],
            ),
        ))
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
