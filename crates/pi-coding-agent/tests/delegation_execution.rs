use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_stream::stream;
use pi_agent_core::AgentResources;
use pi_ai::registry::{self, ApiProvider};
use pi_ai::stream::EventStream;
use pi_ai::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Message, Model, ModelCost,
    ModelInput, StopReason, StreamOptions,
};
use pi_coding_agent::api::{
    CodingAgentEvent, CodingAgentSession, CodingAgentSessionOptions, PromptInvocation,
    PromptRunOptions, PromptTurnOptions, SessionRunOptions,
};
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
    let _env_guard = EnvGuard::set_pi_rust_dir(global);

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
    let mut events = session.subscribe();

    let outcome = session
        .prompt(prompt_options(&cwd, api, "plan feature"))
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
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationRequested { target_id, task, .. }
                if target_id.as_str() == "coder" && task == "implement parser"
        )),
        "expected delegation request event, got {events:#?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationApproved { target_id, task, .. }
                if target_id.as_str() == "coder" && task == "implement parser"
        )),
        "expected delegation approved event, got {events:#?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationStarted { target_id, child_operation_id, .. }
                if target_id.as_str() == "coder" && !child_operation_id.is_empty()
        )),
        "expected delegation started event, got {events:#?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationCompleted { target_id, final_text, .. }
                if target_id.as_str() == "coder" && final_text == "child result"
        )),
        "expected delegation completed event, got {events:#?}"
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
    let _env_guard = EnvGuard::set_pi_rust_dir(global);

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
    let mut events = session.subscribe();

    let outcome = session
        .prompt(prompt_options(&cwd, api, "plan feature"))
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
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationCompleted { target_id, final_text, .. }
                if target_id.as_str() == "coder" && final_text == "child ready"
        )),
        "expected parent delegation to complete, got {events:#?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationRequested {
                requesting_profile_id,
                target_id,
                task,
                ..
            } if requesting_profile_id.as_str() == "coder"
                && target_id.as_str() == "reviewer"
                && task == "review parser"
        )),
        "expected child nested delegation request, got {events:#?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationRejected {
                requesting_profile_id,
                target_id,
                reason,
                ..
            } if requesting_profile_id.as_str() == "coder"
                && target_id.as_str() == "reviewer"
                && reason.contains("max_depth")
        )),
        "expected child nested delegation rejection, got {events:#?}"
    );
    assert!(
        !events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationApproved {
                requesting_profile_id,
                target_id,
                ..
            } if requesting_profile_id.as_str() == "coder"
                && target_id.as_str() == "reviewer"
        )),
        "nested delegation must not be approved when depth is exhausted: {events:#?}"
    );
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
    let _env_guard = EnvGuard::set_pi_rust_dir(global);

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
    let mut events = session.subscribe();

    let outcome = session
        .prompt(prompt_options(&cwd, api, "plan team work"))
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
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationStarted { target_id, target_kind, child_operation_id, .. }
                if target_id.as_str() == "implementation"
                    && *target_kind == pi_coding_agent::api::ProfileKind::Team
                    && !child_operation_id.is_empty()
        )),
        "expected team delegation started event, got {events:#?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationCompleted { target_id, final_text, .. }
                if target_id.as_str() == "implementation"
                    && final_text.contains("Team implementation completed.")
                    && final_text.contains("member result")
        )),
        "expected team delegation completed event, got {events:#?}"
    );
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
    let _env_guard = EnvGuard::set_pi_rust_dir(global);

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
    let mut events = session.subscribe();

    let outcome = session
        .prompt(prompt_options(&cwd, api, "plan feature"))
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
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationRequested { target_id, task, .. }
                if target_id.as_str() == "coder" && task == "implement parser"
        )),
        "expected delegation request event, got {events:#?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationConfirmationRequired {
                target_id,
                task,
                reason,
                ..
            } if target_id.as_str() == "coder"
                && task == "implement parser"
                && reason == "delegation policy requires confirmation"
        )),
        "expected delegation confirmation-required event, got {events:#?}"
    );
    assert!(
        !events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationApproved { .. }
                | CodingAgentEvent::DelegationStarted { .. }
                | CodingAgentEvent::DelegationCompleted { .. }
        )),
        "confirmation-required delegation must not approve or run child work: {events:#?}"
    );
}

#[tokio::test]
async fn approves_pending_delegation_confirmation_through_session_owner() {
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
    let _env_guard = EnvGuard::set_pi_rust_dir(global);

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
    let mut events = session.subscribe();

    let outcome = session
        .prompt(prompt_options(&cwd, api, "plan feature"))
        .await
        .unwrap();
    assert_eq!(outcome.final_text(), Some("parent ready"));

    let pending = session.pending_delegation_confirmations();
    assert_eq!(pending.len(), 1);
    session
        .approve_delegation_confirmation(&pending[0].operation_id, &pending[0].tool_call_id)
        .await
        .unwrap();
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
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationApproved { target_id, task, .. }
                if target_id.as_str() == "coder" && task == "implement parser"
        )),
        "expected approved event after confirmation, got {events:#?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationCompleted { target_id, final_text, .. }
                if target_id.as_str() == "coder" && final_text == "child result"
        )),
        "expected completed event after confirmation, got {events:#?}"
    );
}

#[tokio::test]
async fn rejects_pending_delegation_confirmation_through_session_owner() {
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
    let _env_guard = EnvGuard::set_pi_rust_dir(global);

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
    let mut events = session.subscribe();

    let outcome = session
        .prompt(prompt_options(&cwd, api, "plan feature"))
        .await
        .unwrap();
    assert_eq!(outcome.final_text(), Some("parent ready"));

    let pending = session.pending_delegation_confirmations();
    assert_eq!(pending.len(), 1);
    session
        .reject_delegation_confirmation(
            &pending[0].operation_id,
            &pending[0].tool_call_id,
            "not now",
        )
        .unwrap();
    assert!(session.pending_delegation_confirmations().is_empty());

    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        2,
        "rejected confirmation should not run child work"
    );

    let events = drain_events(&mut events);
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationRejected {
                target_id,
                task,
                reason,
                ..
            } if target_id.as_str() == "coder" && task == "implement parser" && reason == "not now"
        )),
        "expected rejection event after confirmation rejection, got {events:#?}"
    );
    assert!(
        !events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationApproved { .. }
                | CodingAgentEvent::DelegationStarted { .. }
                | CodingAgentEvent::DelegationCompleted { .. }
        )),
        "rejected confirmation must not approve or run child work: {events:#?}"
    );
}

#[tokio::test]
async fn persistent_session_reopens_pending_delegation_confirmation() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    let sessions = temp.path().join("sessions");
    fs::create_dir_all(&global).unwrap();
    write_confirmation_agent_profiles(&cwd);
    let _env_guard = EnvGuard::set_pi_rust_dir(global);

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
        .prompt(prompt_options(&cwd, api, "plan feature"))
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
    let _env_guard = EnvGuard::set_pi_rust_dir(global);

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
        .prompt(prompt_options(&cwd, api, "plan feature"))
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
    let mut events = reopened.subscribe();
    assert_eq!(reopened.pending_delegation_confirmations().len(), 1);

    reopened
        .approve_delegation_confirmation(&operation_id, &tool_call_id)
        .await
        .unwrap();
    assert!(reopened.pending_delegation_confirmations().is_empty());
    drop(reopened);

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
    drop(calls);

    let events = drain_events(&mut events);
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationApproved { target_id, task, .. }
                if target_id.as_str() == "coder" && task == "implement parser"
        )),
        "expected restored approval event, got {events:#?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationCompleted { target_id, final_text, .. }
                if target_id.as_str() == "coder" && final_text == "child result"
        )),
        "expected restored approval completion event, got {events:#?}"
    );

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
    let _env_guard = EnvGuard::set_pi_rust_dir(global);

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
        .prompt(prompt_options(&cwd, api, "plan feature"))
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
    let mut events = reopened.subscribe();
    assert_eq!(reopened.pending_delegation_confirmations().len(), 1);

    reopened
        .reject_delegation_confirmation(&operation_id, &tool_call_id, "not now")
        .unwrap();
    assert!(reopened.pending_delegation_confirmations().is_empty());
    drop(reopened);

    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        2,
        "restored rejection should not run child work"
    );
    drop(calls);

    let events = drain_events(&mut events);
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationRejected {
                target_id,
                task,
                reason,
                ..
            } if target_id.as_str() == "coder" && task == "implement parser" && reason == "not now"
        )),
        "expected restored rejection event, got {events:#?}"
    );
    assert!(
        !events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationApproved { .. }
                | CodingAgentEvent::DelegationStarted { .. }
                | CodingAgentEvent::DelegationCompleted { .. }
        )),
        "restored rejection must not approve or run child work: {events:#?}"
    );

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
    let _env_guard = EnvGuard::set_pi_rust_dir(global);

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
    let mut events = session.subscribe();

    let outcome = session
        .prompt(prompt_options(&cwd, api, "plan feature"))
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
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationApproved { target_id, task, .. }
                if target_id.as_str() == "missing-coder" && task == "implement parser"
        )),
        "expected delegation approved event, got {events:#?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationStarted { target_id, child_operation_id, .. }
                if target_id.as_str() == "missing-coder" && !child_operation_id.is_empty()
        )),
        "expected delegation started event, got {events:#?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::AgentInvocationFailed { profile_id, error, .. }
                if profile_id.as_str() == "missing-coder"
                    && error.to_string().contains("Unknown agent profile: missing-coder")
        )),
        "expected child agent invocation failure event, got {events:#?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationFailed {
                target_id,
                task,
                child_operation_id,
                error,
                ..
            } if target_id.as_str() == "missing-coder"
                && task == "implement parser"
                && !child_operation_id.is_empty()
                && error.to_string().contains("Unknown agent profile: missing-coder")
        )),
        "expected delegation failed event, got {events:#?}"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, CodingAgentEvent::DelegationCompleted { .. })),
        "failed delegation must not emit completion: {events:#?}"
    );
}

fn prompt_options(cwd: &Path, api: &str, prompt: &str) -> PromptTurnOptions {
    PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
        prompt: prompt.into(),
        model: fallback_model(api),
        api_key: None,
        system_prompt: Some("Runtime fallback instructions.".into()),
        max_turns: Some(4),
        tools: Vec::new(),
        register_builtins: false,
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

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn drain_events(
    receiver: &mut pi_coding_agent::api::CodingAgentEventReceiver,
) -> Vec<CodingAgentEvent> {
    let mut events = Vec::new();
    while let Ok(Some(event)) = receiver.try_recv() {
        events.push(event);
    }
    events
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
    api: String,
}

impl ProviderGuard {
    fn register(
        api: &str,
        calls: Arc<Mutex<Vec<RecordedCall>>>,
        responses: Vec<ScriptedResponse>,
    ) -> Self {
        registry::register(
            api,
            Arc::new(ScriptedProvider {
                calls,
                responses: Arc::new(Mutex::new(responses)),
            }),
        );
        Self { api: api.into() }
    }
}

impl Drop for ProviderGuard {
    fn drop(&mut self) {
        registry::unregister(&self.api);
    }
}

struct EnvGuard {
    previous: Option<OsString>,
}

impl EnvGuard {
    fn set_pi_rust_dir(path: PathBuf) -> Self {
        let previous = std::env::var_os("PI_RUST_DIR");
        unsafe {
            std::env::set_var("PI_RUST_DIR", path);
        }
        Self { previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.previous {
                Some(previous) => std::env::set_var("PI_RUST_DIR", previous),
                None => std::env::remove_var("PI_RUST_DIR"),
            }
        }
    }
}
