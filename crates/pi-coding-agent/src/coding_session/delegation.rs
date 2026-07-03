use pi_agent_core::AgentTool;

use super::profiles::{DelegationPolicy, ProfileId};

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
