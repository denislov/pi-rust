use pi_agent_core::AgentTool;
use serde::{Deserialize, Serialize};

use super::CodingSessionError;
use super::event::CodingAgentEvent;
use super::event_service::EventService;
use super::profiles::{DelegationConfirmationMode, DelegationPolicy, ProfileId, ProfileKind};
use super::prompt::DelegationRequest;

pub(crate) fn delegation_tools(
    profile_id: Option<&ProfileId>,
    policy: Option<&DelegationPolicy>,
) -> Vec<AgentTool> {
    let Some(policy) = policy else {
        return Vec::new();
    };
    let profile_id = profile_id
        .cloned()
        .unwrap_or_else(|| ProfileId::from("default"));
    let mut tools = Vec::new();
    if policy.allow_delegate_agent {
        tools.push(delegate_agent_tool(profile_id.clone(), policy.clone()));
    }
    if policy.allow_delegate_team {
        tools.push(delegate_team_tool(profile_id, policy.clone()));
    }
    tools
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DelegationAuthorizationDecision {
    Approved {
        request: DelegationRequest,
        child_delegation_depth: usize,
    },
    RequiresConfirmation {
        request: DelegationRequest,
        reason: String,
        child_delegation_depth: usize,
    },
    Rejected {
        request: DelegationRequest,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DelegationLineageEntry {
    pub(crate) kind: ProfileKind,
    pub(crate) id: ProfileId,
}

impl DelegationLineageEntry {
    pub(crate) fn new(kind: ProfileKind, id: impl Into<ProfileId>) -> Self {
        Self {
            kind,
            id: id.into(),
        }
    }

    pub(crate) fn agent(id: impl Into<ProfileId>) -> Self {
        Self::new(ProfileKind::Agent, id)
    }
}

pub(crate) fn delegation_lineage_for_request(
    parent_lineage: &[DelegationLineageEntry],
    request: &DelegationRequest,
) -> Vec<DelegationLineageEntry> {
    let mut lineage = parent_lineage.to_vec();
    push_unique_lineage_entry(
        &mut lineage,
        DelegationLineageEntry::agent(request.requesting_profile_id.clone()),
    );
    push_unique_lineage_entry(
        &mut lineage,
        DelegationLineageEntry::new(request.target_kind, request.target_id.clone()),
    );
    lineage
}

fn push_unique_lineage_entry(
    lineage: &mut Vec<DelegationLineageEntry>,
    entry: DelegationLineageEntry,
) {
    if !lineage.iter().any(|existing| existing == &entry) {
        lineage.push(entry);
    }
}

#[cfg(test)]
pub(crate) fn authorize_delegation_requests(
    requests: &[DelegationRequest],
    policy: &DelegationPolicy,
    current_depth: usize,
) -> Vec<DelegationAuthorizationDecision> {
    authorize_delegation_requests_with_lineage(requests, policy, current_depth, &[])
}

pub(crate) fn authorize_delegation_requests_with_lineage(
    requests: &[DelegationRequest],
    policy: &DelegationPolicy,
    current_depth: usize,
    lineage: &[DelegationLineageEntry],
) -> Vec<DelegationAuthorizationDecision> {
    let mut accepted_children = 0usize;
    requests
        .iter()
        .map(|request| {
            let mut effective_lineage = lineage.to_vec();
            push_unique_lineage_entry(
                &mut effective_lineage,
                DelegationLineageEntry::agent(request.requesting_profile_id.clone()),
            );
            if let Some(reason) = rejection_reason(
                request,
                policy,
                current_depth,
                accepted_children,
                &effective_lineage,
            ) {
                return DelegationAuthorizationDecision::Rejected {
                    request: request.clone(),
                    reason,
                };
            }
            accepted_children += 1;
            let child_delegation_depth = current_depth.saturating_add(1);
            if let Some(reason) = confirmation_reason(request, policy) {
                DelegationAuthorizationDecision::RequiresConfirmation {
                    request: request.clone(),
                    reason,
                    child_delegation_depth,
                }
            } else {
                DelegationAuthorizationDecision::Approved {
                    request: request.clone(),
                    child_delegation_depth,
                }
            }
        })
        .collect()
}

pub(crate) fn emit_delegation_approved(event_service: &EventService, request: &DelegationRequest) {
    event_service.emit(CodingAgentEvent::DelegationApproved {
        operation_id: request.operation_id.clone(),
        turn_id: request.turn_id.clone(),
        tool_call_id: request.tool_call_id.clone(),
        requesting_profile_id: request.requesting_profile_id.clone(),
        target_kind: request.target_kind,
        target_id: request.target_id.clone(),
        task: request.task.clone(),
    });
}

pub(crate) fn emit_delegation_rejected(
    event_service: &EventService,
    request: &DelegationRequest,
    reason: &str,
) {
    event_service.emit(CodingAgentEvent::DelegationRejected {
        operation_id: request.operation_id.clone(),
        turn_id: request.turn_id.clone(),
        tool_call_id: request.tool_call_id.clone(),
        requesting_profile_id: request.requesting_profile_id.clone(),
        target_kind: request.target_kind,
        target_id: request.target_id.clone(),
        task: request.task.clone(),
        reason: reason.to_owned(),
    });
}

pub(crate) fn emit_delegation_confirmation_required(
    event_service: &EventService,
    request: &DelegationRequest,
    reason: &str,
) {
    event_service.emit(CodingAgentEvent::DelegationConfirmationRequired {
        operation_id: request.operation_id.clone(),
        turn_id: request.turn_id.clone(),
        tool_call_id: request.tool_call_id.clone(),
        requesting_profile_id: request.requesting_profile_id.clone(),
        target_kind: request.target_kind,
        target_id: request.target_id.clone(),
        task: request.task.clone(),
        reason: reason.to_owned(),
    });
}

pub(crate) fn emit_delegation_started(
    event_service: &EventService,
    request: &DelegationRequest,
    child_operation_id: String,
) {
    event_service.emit(CodingAgentEvent::DelegationStarted {
        operation_id: request.operation_id.clone(),
        turn_id: request.turn_id.clone(),
        tool_call_id: request.tool_call_id.clone(),
        requesting_profile_id: request.requesting_profile_id.clone(),
        target_kind: request.target_kind,
        target_id: request.target_id.clone(),
        task: request.task.clone(),
        child_operation_id,
    });
}

pub(crate) fn emit_delegation_completed(
    event_service: &EventService,
    request: &DelegationRequest,
    child_operation_id: String,
    final_text: String,
) {
    event_service.emit(CodingAgentEvent::DelegationCompleted {
        operation_id: request.operation_id.clone(),
        turn_id: request.turn_id.clone(),
        tool_call_id: request.tool_call_id.clone(),
        requesting_profile_id: request.requesting_profile_id.clone(),
        target_kind: request.target_kind,
        target_id: request.target_id.clone(),
        task: request.task.clone(),
        child_operation_id,
        final_text,
    });
}

pub(crate) fn emit_delegation_failed(
    event_service: &EventService,
    request: &DelegationRequest,
    child_operation_id: String,
    error: CodingSessionError,
) {
    event_service.emit(CodingAgentEvent::DelegationFailed {
        operation_id: request.operation_id.clone(),
        turn_id: request.turn_id.clone(),
        tool_call_id: request.tool_call_id.clone(),
        requesting_profile_id: request.requesting_profile_id.clone(),
        target_kind: request.target_kind,
        target_id: request.target_id.clone(),
        task: request.task.clone(),
        child_operation_id,
        error,
    });
}

fn rejection_reason(
    request: &DelegationRequest,
    policy: &DelegationPolicy,
    current_depth: usize,
    accepted_children: usize,
    lineage: &[DelegationLineageEntry],
) -> Option<String> {
    if current_depth >= policy.max_depth {
        return Some("delegation policy max_depth is exhausted".into());
    }
    if accepted_children >= policy.max_parallel_children {
        return Some("delegation policy max_parallel_children is exhausted".into());
    }
    if lineage
        .iter()
        .any(|entry| entry.kind == request.target_kind && entry.id == request.target_id)
    {
        return Some(format!(
            "delegation cycle detected for {} target {}",
            profile_kind_label(request.target_kind),
            request.target_id
        ));
    }
    match request.target_kind {
        ProfileKind::Agent if !policy.allow_delegate_agent => {
            Some("delegation policy does not allow agent delegation".into())
        }
        ProfileKind::Agent if !target_is_allowed(&policy.allowed_agents, &request.target_id) => {
            Some("target agent is not allowed by delegation policy".into())
        }
        ProfileKind::Team if !policy.allow_delegate_team => {
            Some("delegation policy does not allow team delegation".into())
        }
        ProfileKind::Team if !target_is_allowed(&policy.allowed_teams, &request.target_id) => {
            Some("target team is not allowed by delegation policy".into())
        }
        _ => None,
    }
}

fn profile_kind_label(kind: ProfileKind) -> &'static str {
    match kind {
        ProfileKind::Agent => "agent",
        ProfileKind::Team => "team",
    }
}

fn confirmation_reason(request: &DelegationRequest, policy: &DelegationPolicy) -> Option<String> {
    match policy.require_confirmation {
        DelegationConfirmationMode::Never => None,
        DelegationConfirmationMode::Always => {
            Some("delegation policy requires confirmation".into())
        }
        DelegationConfirmationMode::Writes if request.target_kind == ProfileKind::Team => {
            Some("team delegation requires confirmation under writes policy".into())
        }
        DelegationConfirmationMode::Writes => None,
    }
}

fn delegate_agent_tool(profile_id: ProfileId, policy: DelegationPolicy) -> AgentTool {
    AgentTool::new_text(
        "delegate_agent",
        "Request bounded help from another configured agent profile. The session owner validates policy before any child work is allowed.",
        delegation_parameters("agent_id"),
        move |args| {
            let profile_id = profile_id.clone();
            let policy = policy.clone();
            async move { handle_delegation_request("agent", "agent_id", &profile_id, &policy, args) }
        },
    )
}

fn delegate_team_tool(profile_id: ProfileId, policy: DelegationPolicy) -> AgentTool {
    AgentTool::new_text(
        "delegate_team",
        "Request bounded help from a configured team profile. The session owner validates policy before any child work is allowed.",
        delegation_parameters("team_id"),
        move |args| {
            let profile_id = profile_id.clone();
            let policy = policy.clone();
            async move { handle_delegation_request("team", "team_id", &profile_id, &policy, args) }
        },
    )
}

fn delegation_parameters(target_field: &str) -> serde_json::Value {
    let mut properties = serde_json::Map::new();
    properties.insert(
        target_field.to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Configured agent or team profile id"
        }),
    );
    properties.insert(
        "task".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Focused task for the delegated child operation"
        }),
    );
    serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": [target_field, "task"],
        "additionalProperties": false
    })
}

fn handle_delegation_request(
    target_kind: &str,
    target_field: &str,
    profile_id: &ProfileId,
    policy: &DelegationPolicy,
    args: serde_json::Value,
) -> Result<String, String> {
    let target = required_profile_id(&args, target_field)?;
    let task = required_non_empty_string(&args, "task")?;
    if policy.max_depth == 0 {
        return Ok(delegation_response(
            "rejected",
            target_kind,
            &target,
            &task,
            Some("delegation policy max_depth is 0"),
            profile_id,
        ));
    }
    let allowed = match target_kind {
        "agent" => target_is_allowed(&policy.allowed_agents, &target),
        "team" => target_is_allowed(&policy.allowed_teams, &target),
        _ => false,
    };
    if !allowed {
        return Ok(delegation_response(
            "rejected",
            target_kind,
            &target,
            &task,
            Some("target is not allowed by delegation policy"),
            profile_id,
        ));
    }
    Ok(delegation_response(
        "requested",
        target_kind,
        &target,
        &task,
        Some("delegation request captured for session-owned authorization"),
        profile_id,
    ))
}

fn required_profile_id(args: &serde_json::Value, key: &str) -> Result<ProfileId, String> {
    let value = required_non_empty_string(args, key)?;
    ProfileId::new(value).map_err(|error| format!("invalid {key}: {error}"))
}

fn required_non_empty_string(args: &serde_json::Value, key: &str) -> Result<String, String> {
    let value = args
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .unwrap_or_default();
    if value.is_empty() {
        return Err(format!("delegation request requires non-empty {key}"));
    }
    Ok(value.to_string())
}

fn target_is_allowed(allowed: &[ProfileId], target: &ProfileId) -> bool {
    allowed.is_empty() || allowed.iter().any(|allowed| allowed == target)
}

fn delegation_response(
    status: &str,
    target_kind: &str,
    target: &ProfileId,
    task: &str,
    message: Option<&str>,
    profile_id: &ProfileId,
) -> String {
    let mut response = serde_json::json!({
        "status": status,
        "target_kind": target_kind,
        "target_id": target.as_str(),
        "task": task,
        "requesting_profile_id": profile_id.as_str(),
    });
    if let Some(message) = message {
        response["message"] = serde_json::Value::String(message.to_string());
    }
    response.to_string()
}

#[cfg(test)]
mod tests {
    use pi_agent_core::AgentToolOutput;
    use pi_ai::types::ContentBlock;

    use super::super::profiles::{DelegationConfirmationMode, ProfileKind};
    use super::super::prompt::DelegationRequest;
    use super::*;

    #[tokio::test]
    async fn delegate_agent_tool_accepts_allowed_request() {
        let policy = DelegationPolicy {
            allow_delegate_agent: true,
            max_depth: 1,
            allowed_agents: vec![ProfileId::from("coder")],
            ..DelegationPolicy::default()
        };
        let tools = delegation_tools(Some(&ProfileId::from("planner")), Some(&policy));

        assert_eq!(
            tools
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["delegate_agent"]
        );
        assert!(tools[0].parameters["properties"].get("agent_id").is_some());

        let response = run_tool(
            &tools[0],
            serde_json::json!({"agent_id": "coder", "task": "implement it"}),
        )
        .await;
        assert_eq!(response["status"], "requested");
        assert_eq!(response["target_kind"], "agent");
        assert_eq!(response["target_id"], "coder");
        assert_eq!(response["requesting_profile_id"], "planner");
    }

    #[tokio::test]
    async fn delegate_agent_tool_rejects_disallowed_target() {
        let policy = DelegationPolicy {
            allow_delegate_agent: true,
            max_depth: 1,
            allowed_agents: vec![ProfileId::from("coder")],
            ..DelegationPolicy::default()
        };
        let tools = delegation_tools(Some(&ProfileId::from("planner")), Some(&policy));

        let response = run_tool(
            &tools[0],
            serde_json::json!({"agent_id": "reviewer", "task": "review it"}),
        )
        .await;
        assert_eq!(response["status"], "rejected");
        assert!(
            response["message"]
                .as_str()
                .unwrap()
                .contains("not allowed")
        );
    }

    #[tokio::test]
    async fn delegate_team_tool_rejects_zero_depth_policy() {
        let policy = DelegationPolicy {
            allow_delegate_team: true,
            max_depth: 0,
            allowed_teams: vec![ProfileId::from("implementation")],
            ..DelegationPolicy::default()
        };
        let tools = delegation_tools(Some(&ProfileId::from("planner")), Some(&policy));

        assert_eq!(
            tools
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["delegate_team"]
        );
        assert!(tools[0].parameters["properties"].get("team_id").is_some());
        let response = run_tool(
            &tools[0],
            serde_json::json!({"team_id": "implementation", "task": "build it"}),
        )
        .await;
        assert_eq!(response["status"], "rejected");
        assert!(response["message"].as_str().unwrap().contains("max_depth"));
    }

    #[test]
    fn delegation_authorization_auto_approves_agent_when_confirmation_is_never() {
        let policy = DelegationPolicy {
            allow_delegate_agent: true,
            max_depth: 1,
            max_parallel_children: 2,
            require_confirmation: DelegationConfirmationMode::Never,
            allowed_agents: vec![ProfileId::from("coder")],
            ..DelegationPolicy::default()
        };
        let requests = vec![request("tool_1", ProfileKind::Agent, "coder")];

        let decisions = authorize_delegation_requests(&requests, &policy, 0);

        assert_eq!(decisions.len(), 1);
        assert!(matches!(
            &decisions[0],
            DelegationAuthorizationDecision::Approved {
                request,
                child_delegation_depth,
            }
                if request.tool_call_id == "tool_1"
                    && request.target_id.as_str() == "coder"
                    && *child_delegation_depth == 1
        ));
    }

    #[test]
    fn delegation_authorization_holds_team_when_writes_confirmation_required() {
        let policy = DelegationPolicy {
            allow_delegate_team: true,
            max_depth: 1,
            max_parallel_children: 1,
            require_confirmation: DelegationConfirmationMode::Writes,
            allowed_teams: vec![ProfileId::from("implementation")],
            ..DelegationPolicy::default()
        };
        let requests = vec![request("tool_1", ProfileKind::Team, "implementation")];

        let decisions = authorize_delegation_requests(&requests, &policy, 0);

        assert_eq!(decisions.len(), 1);
        assert!(matches!(
            &decisions[0],
            DelegationAuthorizationDecision::RequiresConfirmation {
                request,
                reason,
                child_delegation_depth,
            }
                if request.target_id.as_str() == "implementation"
                    && reason.contains("team")
                    && *child_delegation_depth == 1
        ));
    }

    #[test]
    fn delegation_authorization_rejects_requests_past_child_limit() {
        let policy = DelegationPolicy {
            allow_delegate_agent: true,
            max_depth: 1,
            max_parallel_children: 1,
            require_confirmation: DelegationConfirmationMode::Never,
            ..DelegationPolicy::default()
        };
        let requests = vec![
            request("tool_1", ProfileKind::Agent, "coder"),
            request("tool_2", ProfileKind::Agent, "reviewer"),
        ];

        let decisions = authorize_delegation_requests(&requests, &policy, 0);

        assert_eq!(decisions.len(), 2);
        assert!(matches!(
            &decisions[0],
            DelegationAuthorizationDecision::Approved { request, .. }
                if request.tool_call_id == "tool_1"
        ));
        assert!(matches!(
            &decisions[1],
            DelegationAuthorizationDecision::Rejected { request, reason }
                if request.tool_call_id == "tool_2" && reason.contains("max_parallel_children")
        ));
    }

    #[test]
    fn delegation_authorization_rejects_when_depth_budget_is_exhausted() {
        let policy = DelegationPolicy {
            allow_delegate_agent: true,
            max_depth: 1,
            max_parallel_children: 1,
            require_confirmation: DelegationConfirmationMode::Never,
            ..DelegationPolicy::default()
        };
        let requests = vec![request("tool_1", ProfileKind::Agent, "coder")];

        let decisions = authorize_delegation_requests(&requests, &policy, 1);

        assert_eq!(decisions.len(), 1);
        assert!(matches!(
            &decisions[0],
            DelegationAuthorizationDecision::Rejected { request, reason }
                if request.tool_call_id == "tool_1" && reason.contains("max_depth")
        ));
    }

    fn request(tool_call_id: &str, target_kind: ProfileKind, target_id: &str) -> DelegationRequest {
        DelegationRequest {
            operation_id: "op_1".into(),
            turn_id: "turn_1".into(),
            tool_call_id: tool_call_id.into(),
            requesting_profile_id: ProfileId::from("planner"),
            target_kind,
            target_id: ProfileId::from(target_id),
            task: "help with task".into(),
        }
    }

    async fn run_tool(tool: &AgentTool, args: serde_json::Value) -> serde_json::Value {
        let output = (tool.execute)(
            args,
            Some(std::sync::Arc::new(|_update: AgentToolOutput| {})),
        )
        .await
        .expect("tool should return structured text");
        let text = output
            .content
            .iter()
            .find_map(|block| match block {
                ContentBlock::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .expect("text output");
        serde_json::from_str(text).unwrap()
    }
}
