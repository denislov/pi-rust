#![allow(deprecated)]

mod support;

use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use async_stream::stream;
use pi_agent_core::{AgentResources, AgentTool};
use pi_ai::registry::ApiProvider;
use pi_ai::stream::EventStream;
use pi_ai::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Message, Model, ModelCost,
    ModelInput, StopReason, StreamOptions,
};
use pi_coding_agent::api::{
    CodingAgentOperation, CodingAgentOperationOutcome, CodingAgentProductEvent,
    CodingAgentProductEventReceiver, CodingAgentSession, CodingAgentSessionExportItem,
    CodingAgentSessionOptions, PromptInvocation, PromptRunOptions, PromptTurnOptions,
    SessionRunOptions,
};
use support::{EnvGuard, ProviderGuard as RegistryProviderGuard};
use tempfile::tempdir;

#[tokio::test]
async fn prompt_executes_approved_agent_delegation_after_parent_success() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    fs::create_dir_all(&global).unwrap();
    write_file(
        cwd.join(".pi-rust/agents/delegating-planner.toml"),
        r#"
schema_version = 1
id = "delegating-planner"
display_name = "Delegating Planner"

[delegation]
allow_delegate_agent = true
max_depth = 1
max_parallel_children = 1
require_confirmation = "never"
allowed_agents = ["coder"]
"#,
    );
    write_file(
        cwd.join(".pi-rust/agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
system_prompt = "Coder child instructions."
"#,
    );
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let api = "delegation-execution-agent-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard = ProviderGuard::register(
        api,
        calls.clone(),
        vec![
            ScriptedResponse::tool_call(
                "tool_delegate_agent",
                "delegate_agent",
                serde_json::json!({"agent_id": "coder", "task": "implement parser"}),
            ),
            ScriptedResponse::text("parent ready"),
            ScriptedResponse::text("child result"),
        ],
    );

    let mut session = CodingAgentSession::non_persistent(
        CodingAgentSessionOptions::new()
            .with_cwd(&cwd)
            .with_default_agent_profile_id("delegating-planner"),
    )
    .await
    .unwrap();
    let mut events = session.subscribe_product_events_public();

    let outcome = session
        .run(CodingAgentOperation::Prompt(prompt_options(
            &cwd,
            api,
            "plan feature",
        )))
        .await
        .unwrap();

    assert_eq!(outcome.final_text(), Some("parent ready"));
    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        3,
        "expected parent tool, parent final, and child calls"
    );
    assert_eq!(user_texts(&calls[0].context), vec!["plan feature"]);
    assert_eq!(user_texts(&calls[2].context), vec!["implement parser"]);
    assert_eq!(
        calls[2].context.system_prompt.as_deref(),
        Some("Coder child instructions.")
    );

    let events = drain_events(&mut events);
    assert_event_count(&events, "Delegation(Requested)", 1);
    assert_event_count(&events, "Delegation(Approved)", 1);
    assert_event_count(&events, "Delegation(Started)", 1);
    assert_event_count(&events, "Delegation(Completed)", 1);
}

#[tokio::test]
async fn built_in_default_profile_auto_approves_read_only_helper_delegation() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    fs::create_dir_all(&global).unwrap();
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let api = "delegation-built-in-default-helper-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard = ProviderGuard::register(
        api,
        calls.clone(),
        vec![
            ScriptedResponse::tool_call(
                "tool_delegate_explore",
                "delegate_agent",
                serde_json::json!({"agent_id": "explore", "task": "inspect replay"}),
            ),
            ScriptedResponse::text("parent ready"),
            ScriptedResponse::text("explore result"),
        ],
    );

    let mut session =
        CodingAgentSession::non_persistent(CodingAgentSessionOptions::new().with_cwd(&cwd))
            .await
            .unwrap();
    let mut events = session.subscribe_product_events_public();

    let outcome = session
        .run(CodingAgentOperation::Prompt(prompt_options_with_tools(
            &cwd,
            api,
            "plan feature",
            vec![parent_only_tool()],
        )))
        .await
        .unwrap();

    assert_eq!(outcome.final_text(), Some("parent ready"));
    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        3,
        "expected parent tool, parent final, and built-in helper calls"
    );
    assert_eq!(user_texts(&calls[0].context), vec!["plan feature"]);
    assert_eq!(user_texts(&calls[2].context), vec!["inspect replay"]);
    assert_eq!(
        calls[2].context.system_prompt.as_deref(),
        Some(
            "You are a read-only exploration helper. Gather context and summarize findings without making changes."
        )
    );

    let parent_tools = tool_names(&calls[0].context);
    assert!(
        parent_tools.iter().any(|tool| tool == "delegate_agent"),
        "default parent profile should expose built-in agent delegation: {parent_tools:#?}"
    );
    assert!(
        parent_tools.iter().any(|tool| tool == "parent_only"),
        "parent should keep explicitly configured runtime tools: {parent_tools:#?}"
    );
    let helper_tools = tool_names(&calls[2].context);
    assert!(
        !helper_tools.iter().any(|tool| tool == "parent_only"),
        "built-in helper must not inherit parent runtime tools: {helper_tools:#?}"
    );
    assert!(
        !helper_tools.iter().any(|tool| tool == "delegate_agent"),
        "built-in helper must not inherit delegation authority: {helper_tools:#?}"
    );
    drop(calls);

    let events = drain_events(&mut events);
    assert_event_count(&events, "Delegation(Approved)", 1);
    assert_event_count(&events, "Delegation(Started)", 1);
    assert_event_count(&events, "Delegation(Completed)", 1);
    assert_event_count(&events, "Delegation(ConfirmationRequired)", 0);
}

#[tokio::test]
async fn delegated_helper_receives_minimal_context_without_parent_transcript() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    let sessions = temp.path().join("sessions");
    fs::create_dir_all(&global).unwrap();
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let api = "delegation-helper-minimal-context-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard = ProviderGuard::register(
        api,
        calls.clone(),
        vec![
            ScriptedResponse::text("parent history answer"),
            ScriptedResponse::tool_call(
                "tool_delegate_explore",
                "delegate_agent",
                serde_json::json!({"agent_id": "explore", "task": "inspect replay"}),
            ),
            ScriptedResponse::text("parent ready"),
            ScriptedResponse::text("explore result"),
        ],
    );

    let mut session = CodingAgentSession::create(
        CodingAgentSessionOptions::new()
            .with_cwd(&cwd)
            .with_session_log_root(&sessions)
            .with_session_id("sess_default_helper_minimal_context"),
    )
    .await
    .unwrap();

    let first = session
        .run(CodingAgentOperation::Prompt(prompt_options(
            &cwd,
            api,
            "parent secret context",
        )))
        .await
        .unwrap();
    assert_eq!(first.final_text(), Some("parent history answer"));

    let second = session
        .run(CodingAgentOperation::Prompt(prompt_options(
            &cwd,
            api,
            "plan with prior history",
        )))
        .await
        .unwrap();
    assert_eq!(second.final_text(), Some("parent ready"));

    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        4,
        "expected first prompt, parent delegation tool/final, and helper calls"
    );
    let second_parent_texts = context_texts(&calls[1].context);
    assert!(
        second_parent_texts
            .iter()
            .any(|text| text.contains("parent secret context")),
        "persistent parent prompt should hydrate prior user text: {second_parent_texts:#?}"
    );
    assert!(
        second_parent_texts
            .iter()
            .any(|text| text.contains("parent history answer")),
        "persistent parent prompt should hydrate prior assistant text: {second_parent_texts:#?}"
    );

    assert_eq!(user_texts(&calls[3].context), vec!["inspect replay"]);
    assert_eq!(
        calls[3].context.system_prompt.as_deref(),
        Some(
            "You are a read-only exploration helper. Gather context and summarize findings without making changes."
        )
    );
    let helper_context_texts = context_texts(&calls[3].context);
    for forbidden in [
        "parent secret context",
        "parent history answer",
        "plan with prior history",
        "parent ready",
    ] {
        assert!(
            !helper_context_texts
                .iter()
                .any(|text| text.contains(forbidden)),
            "delegated helper context must omit parent transcript text {forbidden:?}: {helper_context_texts:#?}"
        );
    }
}

#[tokio::test]
async fn persistent_default_helper_delegation_exports_folded_block() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    let sessions = temp.path().join("sessions");
    fs::create_dir_all(&global).unwrap();
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let api = "delegation-built-in-default-helper-export-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard = ProviderGuard::register(
        api,
        calls.clone(),
        vec![
            ScriptedResponse::tool_call(
                "tool_delegate_explore",
                "delegate_agent",
                serde_json::json!({"agent_id": "explore", "task": "inspect replay"}),
            ),
            ScriptedResponse::text("parent ready"),
            ScriptedResponse::text("explore result"),
        ],
    );

    let mut session = CodingAgentSession::create(
        CodingAgentSessionOptions::new()
            .with_cwd(&cwd)
            .with_session_log_root(&sessions)
            .with_session_id("sess_default_helper_export"),
    )
    .await
    .unwrap();

    let outcome = session
        .run(CodingAgentOperation::Prompt(prompt_options(
            &cwd,
            api,
            "plan feature",
        )))
        .await
        .unwrap();
    assert_eq!(outcome.final_text(), Some("parent ready"));

    let export = match session
        .run(CodingAgentOperation::ExportCurrent)
        .await
        .unwrap()
    {
        CodingAgentOperationOutcome::Export(value) => value,
        other => panic!("expected export outcome, got {other:?}"),
    };
    assert!(
        export.transcript.iter().any(|item| matches!(
            item,
            CodingAgentSessionExportItem::Delegation {
                target_id,
                task,
                status,
                summary: Some(summary),
                ..
            } if target_id.as_str() == "explore"
                && task == "inspect replay"
                && status == "completed"
                && summary == "explore result"
        )),
        "expected exported folded delegation block, got {:#?}",
        export.transcript
    );
    assert!(
        !export.transcript.iter().any(|item| matches!(
            item,
            CodingAgentSessionExportItem::Tool { name, .. } if name == "delegate_agent"
        )),
        "delegation request tool call should fold into the delegation block: {:#?}",
        export.transcript
    );
    assert!(
        !export.transcript.iter().any(|item| matches!(
            item,
            CodingAgentSessionExportItem::Assistant { text, .. } if text == "explore result"
        )),
        "delegated child output must stay folded, not appear as a parent assistant message: {:#?}",
        export.transcript
    );
}

#[tokio::test]
async fn delegated_child_does_not_inherit_parent_runtime_tools_without_profile_allowlist() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    fs::create_dir_all(&global).unwrap();
    write_file(
        cwd.join(".pi-rust/agents/delegating-planner.toml"),
        r#"
schema_version = 1
id = "delegating-planner"
display_name = "Delegating Planner"

[delegation]
allow_delegate_agent = true
max_depth = 1
max_parallel_children = 1
require_confirmation = "never"
allowed_agents = ["coder"]
"#,
    );
    write_file(
        cwd.join(".pi-rust/agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
system_prompt = "Coder child instructions."
"#,
    );
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let api = "delegation-child-capability-release-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard = ProviderGuard::register(
        api,
        calls.clone(),
        vec![
            ScriptedResponse::tool_call(
                "tool_delegate_coder",
                "delegate_agent",
                serde_json::json!({"agent_id": "coder", "task": "implement parser"}),
            ),
            ScriptedResponse::text("parent ready"),
            ScriptedResponse::text("child result"),
        ],
    );

    let mut session = CodingAgentSession::non_persistent(
        CodingAgentSessionOptions::new()
            .with_cwd(&cwd)
            .with_default_agent_profile_id("delegating-planner"),
    )
    .await
    .unwrap();

    let outcome = session
        .run(CodingAgentOperation::Prompt(prompt_options_with_tools(
            &cwd,
            api,
            "plan feature",
            vec![parent_only_tool()],
        )))
        .await
        .unwrap();

    assert_eq!(outcome.final_text(), Some("parent ready"));
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 3);
    let parent_tools = tool_names(&calls[0].context);
    assert!(
        parent_tools.iter().any(|tool| tool == "parent_only"),
        "parent should keep its explicitly configured runtime tool: {parent_tools:#?}"
    );
    assert!(
        parent_tools.iter().any(|tool| tool == "delegate_agent"),
        "parent should expose policy delegation tool: {parent_tools:#?}"
    );
    let child_tools = tool_names(&calls[2].context);
    assert!(
        !child_tools.iter().any(|tool| tool == "parent_only"),
        "delegated child must not inherit parent runtime tools without an explicit profile allowlist: {child_tools:#?}"
    );
}

#[tokio::test]
async fn delegated_child_rejects_nested_delegation_when_depth_is_exhausted() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    fs::create_dir_all(&global).unwrap();
    write_file(
        cwd.join(".pi-rust/agents/delegating-planner.toml"),
        r#"
schema_version = 1
id = "delegating-planner"
display_name = "Delegating Planner"

[delegation]
allow_delegate_agent = true
max_depth = 1
max_parallel_children = 1
require_confirmation = "never"
allowed_agents = ["coder"]
"#,
    );
    write_file(
        cwd.join(".pi-rust/agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
system_prompt = "Coder child instructions."

[delegation]
allow_delegate_agent = true
max_depth = 1
max_parallel_children = 1
require_confirmation = "never"
allowed_agents = ["reviewer"]
"#,
    );
    write_file(
        cwd.join(".pi-rust/agents/reviewer.toml"),
        r#"
schema_version = 1
id = "reviewer"
display_name = "Reviewer"
system_prompt = "Reviewer child instructions."
"#,
    );
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let api = "delegation-recursive-depth-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard = ProviderGuard::register(
        api,
        calls.clone(),
        vec![
            ScriptedResponse::tool_call(
                "tool_delegate_coder",
                "delegate_agent",
                serde_json::json!({"agent_id": "coder", "task": "implement parser"}),
            ),
            ScriptedResponse::text("parent ready"),
            ScriptedResponse::tool_call(
                "tool_delegate_reviewer",
                "delegate_agent",
                serde_json::json!({"agent_id": "reviewer", "task": "review parser"}),
            ),
            ScriptedResponse::text("child ready"),
        ],
    );

    let mut session = CodingAgentSession::non_persistent(
        CodingAgentSessionOptions::new()
            .with_cwd(&cwd)
            .with_default_agent_profile_id("delegating-planner"),
    )
    .await
    .unwrap();
    let mut events = session.subscribe_product_events_public();

    let outcome = session
        .run(CodingAgentOperation::Prompt(prompt_options(
            &cwd,
            api,
            "plan feature",
        )))
        .await
        .unwrap();

    assert_eq!(outcome.final_text(), Some("parent ready"));
    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        4,
        "expected parent tool/final and child tool/final calls"
    );
    assert_eq!(user_texts(&calls[0].context), vec!["plan feature"]);
    assert_eq!(user_texts(&calls[2].context), vec!["implement parser"]);

    let events = drain_events(&mut events);
    assert_event_count(&events, "Delegation(Requested)", 2);
    assert_event_count(&events, "Delegation(Approved)", 1);
    assert_event_count(&events, "Delegation(Completed)", 1);
    assert_event_count(&events, "Delegation(Rejected)", 1);
}

#[tokio::test]
async fn nested_confirmation_required_delegation_is_queued_at_session_owner() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    fs::create_dir_all(&global).unwrap();
    write_file(
        cwd.join(".pi-rust/agents/delegating-planner.toml"),
        r#"
schema_version = 1
id = "delegating-planner"
display_name = "Delegating Planner"

[delegation]
allow_delegate_agent = true
max_depth = 2
max_parallel_children = 1
require_confirmation = "never"
allowed_agents = ["coder"]
"#,
    );
    write_file(
        cwd.join(".pi-rust/agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
system_prompt = "Coder child instructions."

[delegation]
allow_delegate_agent = true
max_depth = 2
max_parallel_children = 1
require_confirmation = "always"
allowed_agents = ["reviewer"]
"#,
    );
    write_file(
        cwd.join(".pi-rust/agents/reviewer.toml"),
        r#"
schema_version = 1
id = "reviewer"
display_name = "Reviewer"
system_prompt = "Reviewer child instructions."
"#,
    );
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let api = "delegation-nested-confirmation-queue-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard = ProviderGuard::register(
        api,
        calls.clone(),
        vec![
            ScriptedResponse::tool_call(
                "tool_delegate_coder",
                "delegate_agent",
                serde_json::json!({"agent_id": "coder", "task": "implement parser"}),
            ),
            ScriptedResponse::text("parent ready"),
            ScriptedResponse::tool_call(
                "tool_delegate_reviewer",
                "delegate_agent",
                serde_json::json!({"agent_id": "reviewer", "task": "review parser"}),
            ),
            ScriptedResponse::text("coder ready"),
        ],
    );

    let mut session = CodingAgentSession::non_persistent(
        CodingAgentSessionOptions::new()
            .with_cwd(&cwd)
            .with_default_agent_profile_id("delegating-planner"),
    )
    .await
    .unwrap();
    let mut events = session.subscribe_product_events_public();

    let outcome = session
        .run(CodingAgentOperation::Prompt(prompt_options(
            &cwd,
            api,
            "plan feature",
        )))
        .await
        .unwrap();

    assert_eq!(outcome.final_text(), Some("parent ready"));
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 4);
    assert_eq!(user_texts(&calls[0].context), vec!["plan feature"]);
    assert_eq!(user_texts(&calls[2].context), vec!["implement parser"]);
    drop(calls);

    let pending = session.pending_delegation_confirmations();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].requesting_profile_id.as_str(), "coder");
    assert_eq!(pending[0].target_id.as_str(), "reviewer");
    assert_eq!(pending[0].task, "review parser");

    let events = drain_events(&mut events);
    assert_event_count(&events, "Delegation(ConfirmationRequired)", 1);
    assert_event_count(&events, "Delegation(Rejected)", 0);
    assert_event_count(&events, "Delegation(Approved)", 1);
    assert_event_count(&events, "Delegation(Started)", 1);
}

#[tokio::test]
async fn persistent_session_reopens_nested_delegation_confirmation_and_approves() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    let sessions = temp.path().join("sessions");
    fs::create_dir_all(&global).unwrap();
    write_file(
        cwd.join(".pi-rust/agents/delegating-planner.toml"),
        r#"
schema_version = 1
id = "delegating-planner"
display_name = "Delegating Planner"

[delegation]
allow_delegate_agent = true
max_depth = 2
max_parallel_children = 1
require_confirmation = "never"
allowed_agents = ["coder"]
"#,
    );
    write_file(
        cwd.join(".pi-rust/agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
system_prompt = "Coder child instructions."

[delegation]
allow_delegate_agent = true
max_depth = 2
max_parallel_children = 1
require_confirmation = "always"
allowed_agents = ["reviewer"]
"#,
    );
    write_file(
        cwd.join(".pi-rust/agents/reviewer.toml"),
        r#"
schema_version = 1
id = "reviewer"
display_name = "Reviewer"
system_prompt = "Reviewer child instructions."
"#,
    );
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let api = "delegation-persistent-nested-confirmation-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard = ProviderGuard::register(
        api,
        calls.clone(),
        vec![
            ScriptedResponse::tool_call(
                "tool_delegate_coder",
                "delegate_agent",
                serde_json::json!({"agent_id": "coder", "task": "implement parser"}),
            ),
            ScriptedResponse::text("parent ready"),
            ScriptedResponse::tool_call(
                "tool_delegate_reviewer",
                "delegate_agent",
                serde_json::json!({"agent_id": "reviewer", "task": "review parser"}),
            ),
            ScriptedResponse::text("coder ready"),
            ScriptedResponse::text("reviewer result"),
        ],
    );

    let mut session = CodingAgentSession::create(persistent_confirmation_session_options(
        &cwd,
        &sessions,
        "sess_nested_delegation_pending_approve",
    ))
    .await
    .unwrap();
    let outcome = session
        .run(CodingAgentOperation::Prompt(prompt_options(
            &cwd,
            api,
            "plan feature",
        )))
        .await
        .unwrap();
    assert_eq!(outcome.final_text(), Some("parent ready"));
    let original_pending = session.pending_delegation_confirmations();
    assert_eq!(original_pending.len(), 1);
    assert_eq!(original_pending[0].requesting_profile_id.as_str(), "coder");
    assert_eq!(original_pending[0].target_id.as_str(), "reviewer");
    assert_eq!(original_pending[0].task, "review parser");
    let operation_id = original_pending[0].operation_id.clone();
    let tool_call_id = original_pending[0].tool_call_id.clone();
    drop(session);

    let mut reopened = CodingAgentSession::open(persistent_confirmation_session_options(
        &cwd,
        &sessions,
        "sess_nested_delegation_pending_approve",
    ))
    .await
    .unwrap();
    let pending = reopened.pending_delegation_confirmations();
    assert_eq!(pending, original_pending);
    let mut events = reopened.subscribe_product_events_public();

    let outcome = reopened
        .run(CodingAgentOperation::ApproveDelegation {
            operation_id: operation_id.clone(),
            tool_call_id: tool_call_id.clone(),
        })
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        CodingAgentOperationOutcome::DelegationApproved
    ));
    assert!(reopened.pending_delegation_confirmations().is_empty());
    drop(reopened);

    {
        let calls = calls.lock().unwrap();
        assert_eq!(
            calls.len(),
            5,
            "nested approval should run reviewer exactly once after reopen"
        );
        assert_eq!(user_texts(&calls[4].context), vec!["review parser"]);
        assert_eq!(
            calls[4].context.system_prompt.as_deref(),
            Some("Reviewer child instructions.")
        );
    }

    let events = drain_events(&mut events);
    assert_event_count(&events, "Delegation(Approved)", 1);
    assert_event_count(&events, "Delegation(Started)", 1);
    assert_event_count(&events, "Delegation(Completed)", 1);

    let reopened_again = CodingAgentSession::open(persistent_confirmation_session_options(
        &cwd,
        &sessions,
        "sess_nested_delegation_pending_approve",
    ))
    .await
    .unwrap();
    assert!(reopened_again.pending_delegation_confirmations().is_empty());
}

#[tokio::test]
async fn recursive_agent_delegation_executes_until_depth_budget_is_exhausted() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    fs::create_dir_all(&global).unwrap();
    write_file(
        cwd.join(".pi-rust/agents/delegating-planner.toml"),
        r#"
schema_version = 1
id = "delegating-planner"
display_name = "Delegating Planner"

[delegation]
allow_delegate_agent = true
max_depth = 2
max_parallel_children = 1
require_confirmation = "never"
allowed_agents = ["coder"]
"#,
    );
    write_file(
        cwd.join(".pi-rust/agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
system_prompt = "Coder child instructions."

[delegation]
allow_delegate_agent = true
max_depth = 2
max_parallel_children = 1
require_confirmation = "never"
allowed_agents = ["reviewer"]
"#,
    );
    write_file(
        cwd.join(".pi-rust/agents/reviewer.toml"),
        r#"
schema_version = 1
id = "reviewer"
display_name = "Reviewer"
system_prompt = "Reviewer child instructions."

[delegation]
allow_delegate_agent = true
max_depth = 2
max_parallel_children = 1
require_confirmation = "never"
allowed_agents = ["qa"]
"#,
    );
    write_file(
        cwd.join(".pi-rust/agents/qa.toml"),
        r#"
schema_version = 1
id = "qa"
display_name = "QA"
system_prompt = "QA child instructions."
"#,
    );
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let api = "delegation-recursive-budget-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard = ProviderGuard::register(
        api,
        calls.clone(),
        vec![
            ScriptedResponse::tool_call(
                "tool_delegate_coder",
                "delegate_agent",
                serde_json::json!({"agent_id": "coder", "task": "implement parser"}),
            ),
            ScriptedResponse::text("parent ready"),
            ScriptedResponse::tool_call(
                "tool_delegate_reviewer",
                "delegate_agent",
                serde_json::json!({"agent_id": "reviewer", "task": "review parser"}),
            ),
            ScriptedResponse::text("coder ready"),
            ScriptedResponse::tool_call(
                "tool_delegate_qa",
                "delegate_agent",
                serde_json::json!({"agent_id": "qa", "task": "verify parser"}),
            ),
            ScriptedResponse::text("reviewer ready"),
            ScriptedResponse::text("qa should not run"),
        ],
    );

    let mut session = CodingAgentSession::non_persistent(
        CodingAgentSessionOptions::new()
            .with_cwd(&cwd)
            .with_default_agent_profile_id("delegating-planner"),
    )
    .await
    .unwrap();
    let mut events = session.subscribe_product_events_public();

    let outcome = session
        .run(CodingAgentOperation::Prompt(prompt_options(
            &cwd,
            api,
            "plan feature",
        )))
        .await
        .unwrap();

    assert_eq!(outcome.final_text(), Some("parent ready"));
    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        6,
        "expected parent, coder, and reviewer tool/final calls without running qa"
    );
    assert_eq!(user_texts(&calls[0].context), vec!["plan feature"]);
    assert_eq!(user_texts(&calls[2].context), vec!["implement parser"]);
    assert_eq!(user_texts(&calls[4].context), vec!["review parser"]);

    let events = drain_events(&mut events);
    assert_event_count(&events, "Delegation(Requested)", 3);
    assert_event_count(&events, "Delegation(Approved)", 2);
    assert_event_count(&events, "Delegation(Started)", 2);
    assert_event_count(&events, "Delegation(Completed)", 2);
    assert_event_count(&events, "Delegation(Rejected)", 1);
}

#[tokio::test]
async fn recursive_agent_delegation_rejects_cycle_to_ancestor_profile() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    fs::create_dir_all(&global).unwrap();
    write_file(
        cwd.join(".pi-rust/agents/delegating-planner.toml"),
        r#"
schema_version = 1
id = "delegating-planner"
display_name = "Delegating Planner"

[delegation]
allow_delegate_agent = true
max_depth = 3
max_parallel_children = 1
require_confirmation = "never"
allowed_agents = ["coder"]
"#,
    );
    write_file(
        cwd.join(".pi-rust/agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
system_prompt = "Coder child instructions."

[delegation]
allow_delegate_agent = true
max_depth = 3
max_parallel_children = 1
require_confirmation = "never"
allowed_agents = ["delegating-planner"]
"#,
    );
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let api = "delegation-cycle-rejection-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard = ProviderGuard::register(
        api,
        calls.clone(),
        vec![
            ScriptedResponse::tool_call(
                "tool_delegate_coder",
                "delegate_agent",
                serde_json::json!({"agent_id": "coder", "task": "implement parser"}),
            ),
            ScriptedResponse::text("parent ready"),
            ScriptedResponse::tool_call(
                "tool_delegate_planner",
                "delegate_agent",
                serde_json::json!({"agent_id": "delegating-planner", "task": "replan parser"}),
            ),
            ScriptedResponse::text("coder ready"),
            ScriptedResponse::text("planner should not run"),
        ],
    );

    let mut session = CodingAgentSession::non_persistent(
        CodingAgentSessionOptions::new()
            .with_cwd(&cwd)
            .with_default_agent_profile_id("delegating-planner"),
    )
    .await
    .unwrap();
    let mut events = session.subscribe_product_events_public();

    let outcome = session
        .run(CodingAgentOperation::Prompt(prompt_options(
            &cwd,
            api,
            "plan feature",
        )))
        .await
        .unwrap();

    assert_eq!(outcome.final_text(), Some("parent ready"));
    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        4,
        "cycle rejection should run only parent and coder tool/final calls"
    );
    assert_eq!(user_texts(&calls[0].context), vec!["plan feature"]);
    assert_eq!(user_texts(&calls[2].context), vec!["implement parser"]);

    let events = drain_events(&mut events);
    assert_event_count(&events, "Delegation(Requested)", 2);
    assert_event_count(&events, "Delegation(Approved)", 1);
    assert_event_count(&events, "Delegation(Started)", 1);
    assert_event_count(&events, "Delegation(Completed)", 1);
    assert_event_count(&events, "Delegation(Rejected)", 1);
}

#[tokio::test]
async fn prompt_executes_approved_team_delegation_after_parent_success() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    fs::create_dir_all(&global).unwrap();
    write_file(
        cwd.join(".pi-rust/agents/delegating-planner.toml"),
        r#"
schema_version = 1
id = "delegating-planner"
display_name = "Delegating Planner"

[delegation]
allow_delegate_team = true
max_depth = 1
max_parallel_children = 1
require_confirmation = "never"
allowed_teams = ["implementation"]
"#,
    );
    write_file(
        cwd.join(".pi-rust/agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
system_prompt = "Team member instructions."
"#,
    );
    write_file(
        cwd.join(".pi-rust/teams/implementation.toml"),
        r#"
schema_version = 1
id = "implementation"
display_name = "Implementation Team"
supervisor = "deterministic"
strategy = "plan_execute_review"
members = ["coder"]
"#,
    );
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let api = "delegation-execution-team-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard = ProviderGuard::register(
        api,
        calls.clone(),
        vec![
            ScriptedResponse::tool_call(
                "tool_delegate_team",
                "delegate_team",
                serde_json::json!({"team_id": "implementation", "task": "build feature"}),
            ),
            ScriptedResponse::text("parent ready"),
            ScriptedResponse::text("member result"),
        ],
    );

    let mut session = CodingAgentSession::non_persistent(
        CodingAgentSessionOptions::new()
            .with_cwd(&cwd)
            .with_default_agent_profile_id("delegating-planner"),
    )
    .await
    .unwrap();
    let mut events = session.subscribe_product_events_public();

    let outcome = session
        .run(CodingAgentOperation::Prompt(prompt_options(
            &cwd,
            api,
            "plan team work",
        )))
        .await
        .unwrap();

    assert_eq!(outcome.final_text(), Some("parent ready"));
    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        3,
        "expected parent tool, parent final, and team member calls"
    );
    assert_eq!(user_texts(&calls[0].context), vec!["plan team work"]);
    assert_eq!(user_texts(&calls[2].context), vec!["build feature"]);
    assert_eq!(
        calls[2].context.system_prompt.as_deref(),
        Some("Team member instructions.")
    );

    let events = drain_events(&mut events);
    assert_event_count(&events, "Delegation(Started)", 1);
    assert_event_count(&events, "Delegation(Completed)", 1);
}

#[tokio::test]
async fn prompt_emits_confirmation_required_without_running_child_delegation() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    fs::create_dir_all(&global).unwrap();
    write_file(
        cwd.join(".pi-rust/agents/delegating-planner.toml"),
        r#"
schema_version = 1
id = "delegating-planner"
display_name = "Delegating Planner"

[delegation]
allow_delegate_agent = true
max_depth = 1
max_parallel_children = 1
require_confirmation = "always"
allowed_agents = ["coder"]
"#,
    );
    write_file(
        cwd.join(".pi-rust/agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
system_prompt = "Coder child instructions."
"#,
    );
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let api = "delegation-confirmation-required-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard = ProviderGuard::register(
        api,
        calls.clone(),
        vec![
            ScriptedResponse::tool_call(
                "tool_delegate_agent",
                "delegate_agent",
                serde_json::json!({"agent_id": "coder", "task": "implement parser"}),
            ),
            ScriptedResponse::text("parent ready"),
        ],
    );

    let mut session = CodingAgentSession::non_persistent(
        CodingAgentSessionOptions::new()
            .with_cwd(&cwd)
            .with_default_agent_profile_id("delegating-planner"),
    )
    .await
    .unwrap();
    let mut events = session.subscribe_product_events_public();

    let outcome = session
        .run(CodingAgentOperation::Prompt(prompt_options(
            &cwd,
            api,
            "plan feature",
        )))
        .await
        .unwrap();

    assert_eq!(outcome.final_text(), Some("parent ready"));
    let pending = session.pending_delegation_confirmations();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].target_id.as_str(), "coder");
    assert_eq!(pending[0].task, "implement parser");
    assert_eq!(pending[0].reason, "delegation policy requires confirmation");
    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        2,
        "confirmation-required delegation should not run child work"
    );
    assert_eq!(user_texts(&calls[0].context), vec!["plan feature"]);
    assert_eq!(user_texts(&calls[1].context), vec!["plan feature"]);

    let events = drain_events(&mut events);
    assert_event_count(&events, "Delegation(Requested)", 1);
    assert_event_count(&events, "Delegation(ConfirmationRequired)", 1);
    assert_event_count(&events, "Delegation(Approved)", 0);
    assert_event_count(&events, "Delegation(Started)", 0);
    assert_event_count(&events, "Delegation(Completed)", 0);
}

#[tokio::test]
async fn approves_pending_delegation_confirmation_through_canonical_operation() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    fs::create_dir_all(&global).unwrap();
    write_file(
        cwd.join(".pi-rust/agents/delegating-planner.toml"),
        r#"
schema_version = 1
id = "delegating-planner"
display_name = "Delegating Planner"

[delegation]
allow_delegate_agent = true
max_depth = 1
max_parallel_children = 1
require_confirmation = "always"
allowed_agents = ["coder"]
"#,
    );
    write_file(
        cwd.join(".pi-rust/agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
system_prompt = "Coder child instructions."
"#,
    );
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let api = "delegation-confirmation-approve-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard = ProviderGuard::register(
        api,
        calls.clone(),
        vec![
            ScriptedResponse::tool_call(
                "tool_delegate_agent",
                "delegate_agent",
                serde_json::json!({"agent_id": "coder", "task": "implement parser"}),
            ),
            ScriptedResponse::text("parent ready"),
            ScriptedResponse::text("child result"),
        ],
    );

    let mut session = CodingAgentSession::non_persistent(
        CodingAgentSessionOptions::new()
            .with_cwd(&cwd)
            .with_default_agent_profile_id("delegating-planner"),
    )
    .await
    .unwrap();
    let mut events = session.subscribe_product_events_public();

    let outcome = session
        .run(CodingAgentOperation::Prompt(prompt_options(
            &cwd,
            api,
            "plan feature",
        )))
        .await
        .unwrap();
    assert_eq!(outcome.final_text(), Some("parent ready"));

    let pending = session.pending_delegation_confirmations();
    assert_eq!(pending.len(), 1);
    let outcome = session
        .run(CodingAgentOperation::ApproveDelegation {
            operation_id: pending[0].operation_id.clone(),
            tool_call_id: pending[0].tool_call_id.clone(),
        })
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        CodingAgentOperationOutcome::DelegationApproved
    ));
    assert!(session.pending_delegation_confirmations().is_empty());

    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        3,
        "approval should run exactly one child provider call"
    );
    assert_eq!(user_texts(&calls[2].context), vec!["implement parser"]);
    assert_eq!(
        calls[2].context.system_prompt.as_deref(),
        Some("Coder child instructions.")
    );

    let events = drain_events(&mut events);
    assert_event_count(&events, "Delegation(Approved)", 1);
    assert_event_count(&events, "Delegation(Started)", 1);
    assert_event_count(&events, "Delegation(Completed)", 1);
}

#[tokio::test]
async fn rejects_pending_delegation_confirmation_through_canonical_operation() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    fs::create_dir_all(&global).unwrap();
    write_file(
        cwd.join(".pi-rust/agents/delegating-planner.toml"),
        r#"
schema_version = 1
id = "delegating-planner"
display_name = "Delegating Planner"

[delegation]
allow_delegate_agent = true
max_depth = 1
max_parallel_children = 1
require_confirmation = "always"
allowed_agents = ["coder"]
"#,
    );
    write_file(
        cwd.join(".pi-rust/agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
system_prompt = "Coder child instructions."
"#,
    );
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let api = "delegation-confirmation-reject-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard = ProviderGuard::register(
        api,
        calls.clone(),
        vec![
            ScriptedResponse::tool_call(
                "tool_delegate_agent",
                "delegate_agent",
                serde_json::json!({"agent_id": "coder", "task": "implement parser"}),
            ),
            ScriptedResponse::text("parent ready"),
        ],
    );

    let mut session = CodingAgentSession::non_persistent(
        CodingAgentSessionOptions::new()
            .with_cwd(&cwd)
            .with_default_agent_profile_id("delegating-planner"),
    )
    .await
    .unwrap();
    let mut events = session.subscribe_product_events_public();

    let outcome = session
        .run(CodingAgentOperation::Prompt(prompt_options(
            &cwd,
            api,
            "plan feature",
        )))
        .await
        .unwrap();
    assert_eq!(outcome.final_text(), Some("parent ready"));

    let pending = session.pending_delegation_confirmations();
    assert_eq!(pending.len(), 1);
    let outcome = session
        .run(CodingAgentOperation::RejectDelegation {
            operation_id: pending[0].operation_id.clone(),
            tool_call_id: pending[0].tool_call_id.clone(),
            reason: "not now".into(),
        })
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        CodingAgentOperationOutcome::DelegationRejected
    ));
    assert!(session.pending_delegation_confirmations().is_empty());

    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        2,
        "rejected confirmation should not run child work"
    );

    let events = drain_events(&mut events);
    assert_event_count(&events, "Delegation(Rejected)", 1);
    assert_event_count(&events, "Delegation(Approved)", 0);
    assert_event_count(&events, "Delegation(Started)", 0);
    assert_event_count(&events, "Delegation(Completed)", 0);
}

#[tokio::test]
async fn persistent_session_reopens_pending_delegation_confirmation() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    let sessions = temp.path().join("sessions");
    fs::create_dir_all(&global).unwrap();
    write_confirmation_agent_profiles(&cwd);
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let api = "delegation-persistent-pending-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard = ProviderGuard::register(
        api,
        calls.clone(),
        vec![
            ScriptedResponse::tool_call(
                "tool_delegate_agent",
                "delegate_agent",
                serde_json::json!({"agent_id": "coder", "task": "implement parser"}),
            ),
            ScriptedResponse::text("parent ready"),
        ],
    );

    let mut session = CodingAgentSession::create(persistent_confirmation_session_options(
        &cwd,
        &sessions,
        "sess_delegation_pending_restore",
    ))
    .await
    .unwrap();

    let outcome = session
        .run(CodingAgentOperation::Prompt(prompt_options(
            &cwd,
            api,
            "plan feature",
        )))
        .await
        .unwrap();
    assert_eq!(outcome.final_text(), Some("parent ready"));
    let original_pending = session.pending_delegation_confirmations();
    assert_eq!(original_pending.len(), 1);
    let original = original_pending[0].clone();
    drop(session);

    let reopened = CodingAgentSession::open(persistent_confirmation_session_options(
        &cwd,
        &sessions,
        "sess_delegation_pending_restore",
    ))
    .await
    .unwrap();

    let pending = reopened.pending_delegation_confirmations();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0], original);
    assert_eq!(pending[0].target_id.as_str(), "coder");
    assert_eq!(pending[0].task, "implement parser");
    assert_eq!(pending[0].reason, "delegation policy requires confirmation");
    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        2,
        "reopening pending confirmation should not run child work"
    );
}

#[tokio::test]
async fn reopened_persistent_session_approves_restored_delegation_confirmation() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    let sessions = temp.path().join("sessions");
    fs::create_dir_all(&global).unwrap();
    write_confirmation_agent_profiles(&cwd);
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let api = "delegation-persistent-approve-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard = ProviderGuard::register(
        api,
        calls.clone(),
        vec![
            ScriptedResponse::tool_call(
                "tool_delegate_agent",
                "delegate_agent",
                serde_json::json!({"agent_id": "coder", "task": "implement parser"}),
            ),
            ScriptedResponse::text("parent ready"),
            ScriptedResponse::text("child result"),
        ],
    );

    let mut session = CodingAgentSession::create(persistent_confirmation_session_options(
        &cwd,
        &sessions,
        "sess_delegation_pending_approve",
    ))
    .await
    .unwrap();
    let outcome = session
        .run(CodingAgentOperation::Prompt(prompt_options(
            &cwd,
            api,
            "plan feature",
        )))
        .await
        .unwrap();
    assert_eq!(outcome.final_text(), Some("parent ready"));
    let pending = session.pending_delegation_confirmations();
    assert_eq!(pending.len(), 1);
    let operation_id = pending[0].operation_id.clone();
    let tool_call_id = pending[0].tool_call_id.clone();
    drop(session);

    let mut reopened = CodingAgentSession::open(persistent_confirmation_session_options(
        &cwd,
        &sessions,
        "sess_delegation_pending_approve",
    ))
    .await
    .unwrap();
    let mut events = reopened.subscribe_product_events_public();
    assert_eq!(reopened.pending_delegation_confirmations().len(), 1);

    let outcome = reopened
        .run(CodingAgentOperation::ApproveDelegation {
            operation_id: operation_id.clone(),
            tool_call_id: tool_call_id.clone(),
        })
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        CodingAgentOperationOutcome::DelegationApproved
    ));
    assert!(reopened.pending_delegation_confirmations().is_empty());
    drop(reopened);

    {
        let calls = calls.lock().unwrap();
        assert_eq!(
            calls.len(),
            3,
            "restored approval should run exactly one child provider call"
        );
        assert_eq!(user_texts(&calls[2].context), vec!["implement parser"]);
        assert_eq!(
            calls[2].context.system_prompt.as_deref(),
            Some("Coder child instructions.")
        );
    }

    let events = drain_events(&mut events);
    assert_event_count(&events, "Delegation(Approved)", 1);
    assert_event_count(&events, "Delegation(Started)", 1);
    assert_event_count(&events, "Delegation(Completed)", 1);

    let reopened_again = CodingAgentSession::open(persistent_confirmation_session_options(
        &cwd,
        &sessions,
        "sess_delegation_pending_approve",
    ))
    .await
    .unwrap();
    assert!(reopened_again.pending_delegation_confirmations().is_empty());
}

#[tokio::test]
async fn reopened_persistent_session_rejects_restored_delegation_confirmation() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    let sessions = temp.path().join("sessions");
    fs::create_dir_all(&global).unwrap();
    write_confirmation_agent_profiles(&cwd);
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let api = "delegation-persistent-reject-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard = ProviderGuard::register(
        api,
        calls.clone(),
        vec![
            ScriptedResponse::tool_call(
                "tool_delegate_agent",
                "delegate_agent",
                serde_json::json!({"agent_id": "coder", "task": "implement parser"}),
            ),
            ScriptedResponse::text("parent ready"),
        ],
    );

    let mut session = CodingAgentSession::create(persistent_confirmation_session_options(
        &cwd,
        &sessions,
        "sess_delegation_pending_reject",
    ))
    .await
    .unwrap();
    let outcome = session
        .run(CodingAgentOperation::Prompt(prompt_options(
            &cwd,
            api,
            "plan feature",
        )))
        .await
        .unwrap();
    assert_eq!(outcome.final_text(), Some("parent ready"));
    let pending = session.pending_delegation_confirmations();
    assert_eq!(pending.len(), 1);
    let operation_id = pending[0].operation_id.clone();
    let tool_call_id = pending[0].tool_call_id.clone();
    drop(session);

    let mut reopened = CodingAgentSession::open(persistent_confirmation_session_options(
        &cwd,
        &sessions,
        "sess_delegation_pending_reject",
    ))
    .await
    .unwrap();
    let mut events = reopened.subscribe_product_events_public();
    assert_eq!(reopened.pending_delegation_confirmations().len(), 1);

    let outcome = reopened
        .run(CodingAgentOperation::RejectDelegation {
            operation_id: operation_id.clone(),
            tool_call_id: tool_call_id.clone(),
            reason: "not now".into(),
        })
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        CodingAgentOperationOutcome::DelegationRejected
    ));
    assert!(reopened.pending_delegation_confirmations().is_empty());
    drop(reopened);

    {
        let calls = calls.lock().unwrap();
        assert_eq!(
            calls.len(),
            2,
            "restored rejection should not run child work"
        );
    }

    let events = drain_events(&mut events);
    assert_event_count(&events, "Delegation(Rejected)", 1);
    assert_event_count(&events, "Delegation(Approved)", 0);
    assert_event_count(&events, "Delegation(Started)", 0);
    assert_event_count(&events, "Delegation(Completed)", 0);

    let reopened_again = CodingAgentSession::open(persistent_confirmation_session_options(
        &cwd,
        &sessions,
        "sess_delegation_pending_reject",
    ))
    .await
    .unwrap();
    assert!(reopened_again.pending_delegation_confirmations().is_empty());
}

#[tokio::test]
async fn prompt_emits_failed_lifecycle_for_approved_agent_delegation_failure() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    fs::create_dir_all(&global).unwrap();
    write_file(
        cwd.join(".pi-rust/agents/delegating-planner.toml"),
        r#"
schema_version = 1
id = "delegating-planner"
display_name = "Delegating Planner"

[delegation]
allow_delegate_agent = true
max_depth = 1
max_parallel_children = 1
require_confirmation = "never"
allowed_agents = ["missing-coder"]
"#,
    );
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let api = "delegation-child-failure-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard = ProviderGuard::register(
        api,
        calls.clone(),
        vec![
            ScriptedResponse::tool_call(
                "tool_delegate_agent",
                "delegate_agent",
                serde_json::json!({"agent_id": "missing-coder", "task": "implement parser"}),
            ),
            ScriptedResponse::text("parent ready"),
        ],
    );

    let mut session = CodingAgentSession::non_persistent(
        CodingAgentSessionOptions::new()
            .with_cwd(&cwd)
            .with_default_agent_profile_id("delegating-planner"),
    )
    .await
    .unwrap();
    let mut events = session.subscribe_product_events_public();

    let outcome = session
        .run(CodingAgentOperation::Prompt(prompt_options(
            &cwd,
            api,
            "plan feature",
        )))
        .await
        .unwrap();

    assert_eq!(outcome.final_text(), Some("parent ready"));
    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        2,
        "failed child profile resolution should not call the provider"
    );
    assert_eq!(user_texts(&calls[0].context), vec!["plan feature"]);
    assert_eq!(user_texts(&calls[1].context), vec!["plan feature"]);

    let events = drain_events(&mut events);
    assert_event_count(&events, "Delegation(Approved)", 1);
    assert_event_count(&events, "Delegation(Started)", 1);
    assert_event_count(&events, "Agent(InvocationFailed)", 1);
    assert_event_count(&events, "Delegation(Failed)", 1);
    assert_event_count(&events, "Delegation(Completed)", 0);
}

fn prompt_options(cwd: &Path, api: &str, prompt: &str) -> PromptTurnOptions {
    prompt_options_with_tools(cwd, api, prompt, Vec::new())
}

fn prompt_options_with_tools(
    cwd: &Path,
    api: &str,
    prompt: &str,
    tools: Vec<AgentTool>,
) -> PromptTurnOptions {
    PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
        prompt: prompt.into(),
        model: fallback_model(api),
        api_key: None,
        auth_diagnostics: Vec::new(),
        system_prompt: Some("Runtime fallback instructions.".into()),
        max_turns: Some(4),
        tools,
        register_builtins: false,
        ai_client: None,
        session: Some(SessionRunOptions::disabled(cwd.to_path_buf())),
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text(prompt.into()),
    })
}

fn persistent_confirmation_session_options(
    cwd: &Path,
    sessions: &Path,
    session_id: &str,
) -> CodingAgentSessionOptions {
    CodingAgentSessionOptions::new()
        .with_cwd(cwd)
        .with_session_id(session_id)
        .with_session_log_root(sessions)
        .with_default_agent_profile_id("delegating-planner")
}

fn write_confirmation_agent_profiles(cwd: &Path) {
    write_file(
        cwd.join(".pi-rust/agents/delegating-planner.toml"),
        r#"
schema_version = 1
id = "delegating-planner"
display_name = "Delegating Planner"

[delegation]
allow_delegate_agent = true
max_depth = 1
max_parallel_children = 1
require_confirmation = "always"
allowed_agents = ["coder"]
"#,
    );
    write_file(
        cwd.join(".pi-rust/agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
system_prompt = "Coder child instructions."
"#,
    );
}

trait PromptOutcomeExt {
    fn final_text(&self) -> Option<&str>;
}

impl PromptOutcomeExt for pi_coding_agent::api::PromptTurnOutcome {
    fn final_text(&self) -> Option<&str> {
        match self {
            pi_coding_agent::api::PromptTurnOutcome::Success { final_text, .. } => {
                Some(final_text.as_str())
            }
            _ => None,
        }
    }
}

impl PromptOutcomeExt for CodingAgentOperationOutcome {
    fn final_text(&self) -> Option<&str> {
        match self {
            CodingAgentOperationOutcome::Prompt(outcome) => outcome.final_text(),
            other => panic!("expected prompt outcome, got {other:?}"),
        }
    }
}

fn fallback_model(api: &str) -> Model {
    Model {
        id: "fallback-model".into(),
        name: "Fallback Model".into(),
        api: api.into(),
        provider: "test".into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost::default(),
        context_window: 0,
        max_tokens: 0,
        headers: None,
        compat: None,
    }
}

fn parent_only_tool() -> AgentTool {
    AgentTool::new_text(
        "parent_only",
        "parent-only capability",
        serde_json::json!({"type": "object"}),
        |_args| async { Ok("parent-only".to_owned()) },
    )
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn drain_events(receiver: &mut CodingAgentProductEventReceiver) -> Vec<CodingAgentProductEvent> {
    let mut events = Vec::new();
    while let Ok(Some(event)) = receiver.try_recv() {
        events.push(event);
    }
    events
}

fn assert_event_count(events: &[CodingAgentProductEvent], kind: &str, expected: usize) {
    assert_eq!(
        events.iter().filter(|event| event.kind == kind).count(),
        expected,
        "unexpected {kind} event count: {events:#?}"
    );
}

fn tool_names(context: &Context) -> Vec<String> {
    context
        .tools
        .as_ref()
        .map(|tools| tools.iter().map(|tool| tool.name.clone()).collect())
        .unwrap_or_default()
}

fn user_texts(context: &Context) -> Vec<String> {
    context
        .messages
        .iter()
        .filter_map(|message| match message {
            Message::User { content } => Some(
                content
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text, .. } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            _ => None,
        })
        .collect()
}

fn context_texts(context: &Context) -> Vec<String> {
    let mut texts = Vec::new();
    for message in &context.messages {
        let content = match message {
            Message::User { content }
            | Message::Assistant { content }
            | Message::ToolResult { content, .. } => content,
        };
        for block in content {
            if let ContentBlock::Text { text, .. } = block {
                texts.push(text.clone());
            }
        }
    }
    texts
}

#[derive(Debug, Clone)]
struct RecordedCall {
    context: Context,
}

#[derive(Debug, Clone)]
enum ScriptedResponse {
    Text(String),
    ToolCall {
        id: String,
        name: String,
        arguments: serde_json::Value,
    },
}

impl ScriptedResponse {
    fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    fn tool_call(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        Self::ToolCall {
            id: id.into(),
            name: name.into(),
            arguments,
        }
    }
}

struct ScriptedProvider {
    calls: Arc<Mutex<Vec<RecordedCall>>>,
    responses: Arc<Mutex<Vec<ScriptedResponse>>>,
}

impl ApiProvider for ScriptedProvider {
    fn stream(&self, model: &Model, ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
        self.calls
            .lock()
            .unwrap()
            .push(RecordedCall { context: ctx });
        let response = self.responses.lock().unwrap().remove(0);
        let model_id = model.id.clone();
        Box::pin(stream! {
            let mut message = AssistantMessage::empty("delegation-execution-test", &model_id);
            message.provider = Some("delegation-execution-test".into());
            match response {
                ScriptedResponse::Text(text) => {
                    message.content.push(ContentBlock::Text {
                        text,
                        text_signature: None,
                    });
                    message.stop_reason = StopReason::Stop;
                    yield AssistantMessageEvent::Done {
                        reason: StopReason::Stop,
                        message,
                    };
                }
                ScriptedResponse::ToolCall { id, name, arguments } => {
                    message.content.push(ContentBlock::ToolCall {
                        id,
                        name,
                        arguments,
                        thought_signature: None,
                    });
                    message.stop_reason = StopReason::ToolUse;
                    yield AssistantMessageEvent::Done {
                        reason: StopReason::ToolUse,
                        message,
                    };
                }
            }
        })
    }
}

struct ProviderGuard {
    _guard: RegistryProviderGuard<'static>,
}

impl ProviderGuard {
    fn register(
        api: &str,
        calls: Arc<Mutex<Vec<RecordedCall>>>,
        responses: Vec<ScriptedResponse>,
    ) -> Self {
        let guard = RegistryProviderGuard::register(
            api,
            Arc::new(ScriptedProvider {
                calls,
                responses: Arc::new(Mutex::new(responses)),
            }),
        );
        Self { _guard: guard }
    }
}
