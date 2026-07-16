use crate::authorization::{
    ToolAuthorizationDecision, ToolAuthorizationMode, ToolAuthorizationPreview,
    ToolAuthorizationRequest, ToolAuthorizationRisk, ToolAuthorizationScope,
};
use crate::runtime::capability::OperationCapabilitySnapshot;
use crate::runtime::facade::CodingSessionError;
use crate::runtime::snapshot::SnapshotCoordinator;
use crate::services::event::EventService;
use pi_agent_core::api::agent::{BeforeToolCallContext, BeforeToolCallResult};
use pi_agent_core::api::tool::AgentTool;
use pi_agent_core::api::transcript::create_session_id;
use pi_agent_core::api::transcript::create_timestamp;
use regex::Regex;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::oneshot;

#[derive(Debug, Clone, Default)]
pub(crate) struct ToolAuthorizationInventory {
    plugin_tools: BTreeMap<String, Option<PluginToolAuthorizationRisk>>,
    explicit_tools: BTreeMap<String, Option<PluginToolAuthorizationRisk>>,
}

impl ToolAuthorizationInventory {
    pub(crate) fn new(plugin_tools: &[AgentTool], explicit_tools: &[AgentTool]) -> Self {
        Self {
            plugin_tools: plugin_tools
                .iter()
                .map(|tool| (tool.name.clone(), plugin_tool_risk(tool)))
                .collect(),
            explicit_tools: explicit_tools
                .iter()
                .map(|tool| (tool.name.clone(), plugin_tool_risk(tool)))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PluginToolAuthorizationRisk {
    WorkspaceLocalReadOnly,
    SideEffect,
}

fn plugin_tool_risk(tool: &AgentTool) -> Option<PluginToolAuthorizationRisk> {
    match tool
        .parameters
        .get("x-pi-authorization-risk")
        .and_then(Value::as_str)
    {
        Some("workspace_local_read_only") => {
            Some(PluginToolAuthorizationRisk::WorkspaceLocalReadOnly)
        }
        Some("side_effect") => Some(PluginToolAuthorizationRisk::SideEffect),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AuthorizationHookContext {
    pub(crate) service: AuthorizationService,
    pub(crate) turn_id: String,
    pub(crate) capability_snapshot: OperationCapabilitySnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct OperationGrant {
    operation_id: String,
    tool_name: String,
    scope: ToolAuthorizationScope,
}

struct PendingAuthorization {
    request: ToolAuthorizationRequest,
    sender: oneshot::Sender<PendingResolution>,
}

#[derive(Debug)]
enum PendingResolution {
    Allow,
    Deny(String),
}

#[derive(Default)]
struct AuthorizationState {
    pending: BTreeMap<String, PendingAuthorization>,
    grants: HashSet<OperationGrant>,
    revision: u64,
}

#[derive(Clone)]
pub(crate) struct AuthorizationService {
    mode: ToolAuthorizationMode,
    coordinator: Arc<SnapshotCoordinator>,
    event_service: EventService,
    state: Arc<Mutex<AuthorizationState>>,
}

impl std::fmt::Debug for AuthorizationService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthorizationService")
            .field("mode", &self.mode)
            .field("pending", &self.state.lock().unwrap().pending.len())
            .finish()
    }
}

impl AuthorizationService {
    pub(crate) fn new(
        mode: ToolAuthorizationMode,
        coordinator: Arc<SnapshotCoordinator>,
        event_service: EventService,
    ) -> Self {
        Self {
            mode,
            coordinator,
            event_service,
            state: Arc::new(Mutex::new(AuthorizationState::default())),
        }
    }

    pub(crate) fn pending(&self) -> Vec<ToolAuthorizationRequest> {
        pending_requests(&self.state.lock().unwrap())
    }

    #[cfg(test)]
    pub(crate) fn set_event_service(&mut self, event_service: EventService) {
        self.event_service = event_service;
    }

    pub(crate) async fn authorize(
        &self,
        context: BeforeToolCallContext,
        turn_id: String,
        snapshot: OperationCapabilitySnapshot,
        inventory: ToolAuthorizationInventory,
    ) -> Result<Option<BeforeToolCallResult>, String> {
        let operation_id = context
            .execution_context
            .scope_id()
            .ok_or_else(|| "tool authorization requires an operation identity".to_owned())?
            .to_owned();
        if operation_id != snapshot.operation_id {
            return Err("tool authorization operation identity mismatch".into());
        }

        let evaluation = evaluate(&context, &snapshot, &inventory)?;
        let Evaluation::Ask {
            risk,
            scope,
            preview,
        } = evaluation
        else {
            return Ok(None);
        };
        let grant = OperationGrant {
            operation_id: operation_id.clone(),
            tool_name: context.tool_name.clone(),
            scope: scope.clone(),
        };
        if self.state.lock().unwrap().grants.contains(&grant) {
            return Ok(None);
        }

        let authorization_id = format!("auth_{}", create_session_id());
        let request = ToolAuthorizationRequest {
            authorization_id: authorization_id.clone(),
            operation_id,
            turn_id,
            tool_call_id: context.tool_call_id.clone(),
            tool_name: context.tool_name.clone(),
            risk,
            scope,
            preview,
            capability_generation: snapshot.generation.get(),
            requested_at: create_timestamp(),
        };
        match self.mode {
            ToolAuthorizationMode::AllowAll => return Ok(None),
            ToolAuthorizationMode::Deny => {
                let reason = "tool invocation requires authorization";
                self.event_service
                    .emit_tool_authorization_required(request.clone());
                self.event_service
                    .emit_tool_authorization_denied(request, reason);
                return Ok(Some(blocked(reason)));
            }
            ToolAuthorizationMode::Interactive => {}
        }
        let (sender, receiver) = oneshot::channel();
        let (revision, pending) = {
            let mut state = self.state.lock().unwrap();
            state.pending.insert(
                authorization_id.clone(),
                PendingAuthorization {
                    request: request.clone(),
                    sender,
                },
            );
            state.revision = state.revision.wrapping_add(1);
            (state.revision, pending_requests(&state))
        };
        self.sync_pending_snapshot(revision, pending);
        self.event_service.emit_tool_authorization_required(request);

        tokio::select! {
            resolution = receiver => match resolution {
                Ok(PendingResolution::Allow) => Ok(None),
                Ok(PendingResolution::Deny(reason)) => Ok(Some(blocked(reason))),
                Err(_) => Ok(Some(blocked("tool authorization was interrupted"))),
            },
            _ = context.execution_context.cancel_token().cancelled() => {
                if let Some(request) = self.remove_pending(&authorization_id) {
                    self.event_service.emit_tool_authorization_cancelled(
                        request,
                        "tool authorization was cancelled",
                    );
                }
                Ok(Some(blocked("tool authorization was cancelled")))
            }
        }
    }

    pub(crate) fn decide(
        &self,
        authorization_id: &str,
        decision: ToolAuthorizationDecision,
    ) -> Result<(), CodingSessionError> {
        let current_generation = self.coordinator.current_capability_generation().get();
        let (entry, resolution, revision, pending) = {
            let mut state = self.state.lock().unwrap();
            let Some(entry) = state.pending.remove(authorization_id) else {
                return Err(CodingSessionError::Input {
                    message: format!(
                        "unknown or already resolved authorization: {authorization_id}"
                    ),
                });
            };
            state.revision = state.revision.wrapping_add(1);
            if entry.request.capability_generation != current_generation {
                let revision = state.revision;
                let pending = pending_requests(&state);
                drop(state);
                self.sync_pending_snapshot(revision, pending);
                self.event_service.emit_tool_authorization_cancelled(
                    entry.request.clone(),
                    "tool authorization capability generation is stale",
                );
                let _ = entry.sender.send(PendingResolution::Deny(
                    "tool authorization capability generation is stale".into(),
                ));
                return Err(CodingSessionError::Input {
                    message: "tool authorization capability generation is stale".into(),
                });
            }

            let resolution = match &decision {
                ToolAuthorizationDecision::AllowOnce => PendingResolution::Allow,
                ToolAuthorizationDecision::AllowForOperation => {
                    state.grants.insert(OperationGrant {
                        operation_id: entry.request.operation_id.clone(),
                        tool_name: entry.request.tool_name.clone(),
                        scope: entry.request.scope.clone(),
                    });
                    PendingResolution::Allow
                }
                ToolAuthorizationDecision::Deny { reason } => PendingResolution::Deny(
                    reason
                        .clone()
                        .unwrap_or_else(|| "tool invocation denied by user".into()),
                ),
            };
            let revision = state.revision;
            let pending = pending_requests(&state);
            (entry, resolution, revision, pending)
        };
        self.sync_pending_snapshot(revision, pending);
        match &resolution {
            PendingResolution::Allow => {
                self.event_service
                    .emit_tool_authorization_approved(entry.request.clone(), decision);
            }
            PendingResolution::Deny(reason) => {
                self.event_service
                    .emit_tool_authorization_denied(entry.request.clone(), reason.clone());
            }
        }
        if entry.sender.send(resolution).is_err() {
            self.event_service.emit_tool_authorization_cancelled(
                entry.request,
                "authorization waiter is no longer active",
            );
            return Err(CodingSessionError::Input {
                message: format!("authorization waiter is no longer active: {authorization_id}"),
            });
        }
        Ok(())
    }

    pub(crate) fn cancel_operation(&self, operation_id: &str, reason: &str) {
        let (entries, revision, pending) = {
            let mut state = self.state.lock().unwrap();
            let ids = state
                .pending
                .iter()
                .filter(|(_, entry)| entry.request.operation_id == operation_id)
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>();
            let entries = ids
                .into_iter()
                .filter_map(|id| state.pending.remove(&id))
                .collect::<Vec<_>>();
            if !entries.is_empty() {
                state.revision = state.revision.wrapping_add(1);
            }
            state
                .grants
                .retain(|grant| grant.operation_id != operation_id);
            let pending = pending_requests(&state);
            (entries, state.revision, pending)
        };
        self.sync_pending_snapshot(revision, pending);
        for entry in entries {
            self.event_service
                .emit_tool_authorization_cancelled(entry.request.clone(), reason);
            let _ = entry
                .sender
                .send(PendingResolution::Deny(reason.to_owned()));
        }
    }

    pub(crate) fn cancel_all(&self, reason: &str) {
        let (entries, revision) = {
            let mut state = self.state.lock().unwrap();
            let entries = std::mem::take(&mut state.pending)
                .into_values()
                .collect::<Vec<_>>();
            if !entries.is_empty() {
                state.revision = state.revision.wrapping_add(1);
            }
            state.grants.clear();
            (entries, state.revision)
        };
        self.sync_pending_snapshot(revision, Vec::new());
        for entry in entries {
            self.event_service
                .emit_tool_authorization_cancelled(entry.request.clone(), reason);
            let _ = entry
                .sender
                .send(PendingResolution::Deny(reason.to_owned()));
        }
    }

    fn remove_pending(&self, authorization_id: &str) -> Option<ToolAuthorizationRequest> {
        let (request, revision, pending) = {
            let mut state = self.state.lock().unwrap();
            let request = state
                .pending
                .remove(authorization_id)
                .map(|entry| entry.request);
            if request.is_some() {
                state.revision = state.revision.wrapping_add(1);
            }
            let pending = pending_requests(&state);
            (request, state.revision, pending)
        };
        self.sync_pending_snapshot(revision, pending);
        request
    }

    fn sync_pending_snapshot(&self, mut revision: u64, mut pending: Vec<ToolAuthorizationRequest>) {
        loop {
            self.coordinator.set_pending_authorizations(pending);
            let state = self.state.lock().unwrap();
            if state.revision == revision {
                return;
            }
            revision = state.revision;
            pending = pending_requests(&state);
        }
    }
}

fn pending_requests(state: &AuthorizationState) -> Vec<ToolAuthorizationRequest> {
    let mut requests = state
        .pending
        .values()
        .map(|entry| entry.request.clone())
        .collect::<Vec<_>>();
    requests.sort_by(|left, right| {
        left.requested_at
            .cmp(&right.requested_at)
            .then_with(|| left.authorization_id.cmp(&right.authorization_id))
    });
    requests
}

enum Evaluation {
    Allow,
    Ask {
        risk: ToolAuthorizationRisk,
        scope: ToolAuthorizationScope,
        preview: ToolAuthorizationPreview,
    },
}

fn evaluate(
    context: &BeforeToolCallContext,
    snapshot: &OperationCapabilitySnapshot,
    inventory: &ToolAuthorizationInventory,
) -> Result<Evaluation, String> {
    match context.tool_name.as_str() {
        "read" | "grep" | "find" | "ls" => {
            let Some(filesystem) = snapshot.filesystem.as_ref() else {
                return Err("filesystem capability is not granted".into());
            };
            let path = context
                .arguments
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or(".");
            let resolved = filesystem
                .resolve_path(path)
                .map_err(|error| error.to_string())?;
            if path_is_within(&resolved, &filesystem.cwd) {
                Ok(Evaluation::Allow)
            } else {
                Ok(path_request(
                    ToolAuthorizationRisk::ExternalRead,
                    resolved,
                    "Read outside the workspace",
                ))
            }
        }
        "write" | "edit" => {
            let Some(filesystem) = snapshot.filesystem.as_ref() else {
                return Err("filesystem capability is not granted".into());
            };
            let path = context
                .arguments
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| "filesystem mutation is missing `path`".to_owned())?;
            let resolved = filesystem
                .resolve_path(path)
                .map_err(|error| error.to_string())?;
            Ok(path_request_with_content(
                ToolAuthorizationRisk::FilesystemMutation,
                resolved,
                "Modify a file",
                mutation_content_preview(context),
            ))
        }
        "bash" => {
            let Some(shell) = snapshot.shell.as_ref() else {
                return Err("shell capability is not granted".into());
            };
            let command = context
                .arguments
                .get("command")
                .and_then(Value::as_str)
                .ok_or_else(|| "shell invocation is missing `command`".to_owned())?;
            let redacted = redact_command(command);
            Ok(Evaluation::Ask {
                risk: ToolAuthorizationRisk::ShellExecution,
                scope: ToolAuthorizationScope::Shell {
                    cwd: shell.cwd.to_string_lossy().into_owned(),
                    command_fingerprint: fingerprint(command.as_bytes()),
                },
                preview: ToolAuthorizationPreview {
                    summary: "Execute a shell command".into(),
                    path: None,
                    command: Some(redacted),
                    cwd: Some(shell.cwd.to_string_lossy().into_owned()),
                    content_preview: None,
                },
            })
        }
        "delegate_agent" | "delegate_team" => Ok(Evaluation::Allow),
        name if inventory.plugin_tools.contains_key(name) => {
            match inventory.plugin_tools.get(name).copied().flatten() {
                Some(PluginToolAuthorizationRisk::WorkspaceLocalReadOnly) => Ok(Evaluation::Allow),
                Some(PluginToolAuthorizationRisk::SideEffect) | None => Ok(argument_request(
                    context,
                    ToolAuthorizationRisk::PluginSideEffect,
                    "Run a plugin tool",
                )),
            }
        }
        name if inventory.explicit_tools.contains_key(name) => {
            match inventory.explicit_tools.get(name).copied().flatten() {
                Some(PluginToolAuthorizationRisk::WorkspaceLocalReadOnly) => Ok(Evaluation::Allow),
                Some(PluginToolAuthorizationRisk::SideEffect) | None => Ok(argument_request(
                    context,
                    ToolAuthorizationRisk::Unknown,
                    "Run a custom tool",
                )),
            }
        }
        _ => Ok(argument_request(
            context,
            ToolAuthorizationRisk::Unknown,
            "Run a tool without risk metadata",
        )),
    }
}

fn path_request(risk: ToolAuthorizationRisk, path: PathBuf, summary: &str) -> Evaluation {
    path_request_with_content(risk, path, summary, None)
}

fn path_request_with_content(
    risk: ToolAuthorizationRisk,
    path: PathBuf,
    summary: &str,
    content_preview: Option<String>,
) -> Evaluation {
    let path = path.to_string_lossy().into_owned();
    Evaluation::Ask {
        risk,
        scope: ToolAuthorizationScope::Path { path: path.clone() },
        preview: ToolAuthorizationPreview {
            summary: summary.into(),
            path: Some(path),
            command: None,
            cwd: None,
            content_preview,
        },
    }
}

fn argument_request(
    context: &BeforeToolCallContext,
    risk: ToolAuthorizationRisk,
    summary: &str,
) -> Evaluation {
    Evaluation::Ask {
        risk,
        scope: ToolAuthorizationScope::ToolArguments {
            fingerprint: argument_fingerprint(&context.arguments),
        },
        preview: ToolAuthorizationPreview {
            summary: format!("{summary}: {}", context.tool_name),
            path: None,
            command: None,
            cwd: None,
            content_preview: None,
        },
    }
}

fn blocked(reason: impl Into<String>) -> BeforeToolCallResult {
    BeforeToolCallResult {
        block: true,
        reason: Some(reason.into()),
    }
}

fn path_is_within(path: &Path, cwd: &Path) -> bool {
    path.starts_with(cwd)
}

fn argument_fingerprint(arguments: &Value) -> String {
    fingerprint(canonical_json(arguments).as_bytes())
}

fn fingerprint(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn canonical_json(value: &Value) -> String {
    match value {
        Value::Object(values) => {
            let mut fields = values.iter().collect::<Vec<_>>();
            fields.sort_by_key(|(name, _)| *name);
            let fields = fields
                .into_iter()
                .map(|(key, value)| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(key).unwrap(),
                        canonical_json(value)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{fields}}}")
        }
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(canonical_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        _ => serde_json::to_string(value).unwrap(),
    }
}

fn redact_command(command: &str) -> String {
    redact_sensitive_text(command)
}

fn mutation_content_preview(context: &BeforeToolCallContext) -> Option<String> {
    let raw = if context.tool_name == "write" {
        context.arguments.get("content")?.as_str()?.to_owned()
    } else {
        context
            .arguments
            .get("edits")?
            .as_array()?
            .iter()
            .take(4)
            .flat_map(|edit| {
                let old = edit.get("oldText").and_then(Value::as_str).unwrap_or("");
                let new = edit.get("newText").and_then(Value::as_str).unwrap_or("");
                old.lines()
                    .take(3)
                    .map(|line| format!("- {line}"))
                    .chain(new.lines().take(3).map(|line| format!("+ {line}")))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let bounded = raw.lines().take(12).collect::<Vec<_>>().join("\n");
    let bounded = bounded.chars().take(1_200).collect::<String>();
    (!bounded.is_empty()).then(|| redact_sensitive_text(&bounded))
}

fn redact_sensitive_text(text: &str) -> String {
    static ASSIGNMENT: OnceLock<Regex> = OnceLock::new();
    static JSON_FIELD: OnceLock<Regex> = OnceLock::new();
    static BEARER: OnceLock<Regex> = OnceLock::new();
    let assignment = ASSIGNMENT.get_or_init(|| {
        Regex::new(r"(?i)\b(api[_-]?key|token|password|passwd|secret)\s*=\s*([^\s;&|]+)")
            .expect("redaction regex is valid")
    });
    let json_field = JSON_FIELD.get_or_init(|| {
        Regex::new(
            r#"(?i)([\"']?(?:api[_-]?key|token|password|passwd|secret)[\"']?\s*:\s*)[\"'][^\"']+[\"']"#,
        )
        .expect("redaction regex is valid")
    });
    let bearer = BEARER
        .get_or_init(|| Regex::new(r"(?i)\bbearer\s+[^\s;&|]+").expect("redaction regex is valid"));
    let redacted = assignment.replace_all(text, "$1=<redacted>");
    let redacted = json_field.replace_all(&redacted, "$1\"<redacted>\"");
    bearer
        .replace_all(&redacted, "Bearer <redacted>")
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::capability::{FilesystemCapability, ShellCapability};
    use pi_agent_core::api::tool::ToolExecutionContext;
    use pi_ai::api::conversation::AssistantMessage;
    use serde_json::json;
    use tempfile::TempDir;
    use tokio_util::sync::CancellationToken;

    fn service(mode: ToolAuthorizationMode) -> (AuthorizationService, Arc<SnapshotCoordinator>) {
        let coordinator = SnapshotCoordinator::new();
        let events = EventService::with_snapshot_coordinator(coordinator.clone());
        (
            AuthorizationService::new(mode, coordinator.clone(), events),
            coordinator,
        )
    }

    fn snapshot(operation_id: &str, cwd: &Path) -> OperationCapabilitySnapshot {
        let mut snapshot = OperationCapabilitySnapshot::permissive(operation_id);
        snapshot.filesystem = Some(FilesystemCapability::new(cwd.to_path_buf()));
        snapshot.shell = Some(ShellCapability::new(cwd.to_path_buf()));
        snapshot
    }

    fn context(
        operation_id: &str,
        tool_call_id: &str,
        tool_name: &str,
        arguments: Value,
        cancel_token: CancellationToken,
    ) -> BeforeToolCallContext {
        BeforeToolCallContext {
            execution_context: ToolExecutionContext::new(
                Some(operation_id),
                1,
                tool_call_id,
                tool_name,
                cancel_token,
            ),
            assistant_message: AssistantMessage::empty("test", "test"),
            tool_call_id: tool_call_id.into(),
            tool_name: tool_name.into(),
            arguments,
            messages: Vec::new(),
        }
    }

    fn inventory() -> ToolAuthorizationInventory {
        ToolAuthorizationInventory::new(&[], &[])
    }

    async fn wait_for_request(service: &AuthorizationService) -> ToolAuthorizationRequest {
        for _ in 0..100 {
            if let Some(request) = service.pending().into_iter().next() {
                return request;
            }
            tokio::task::yield_now().await;
        }
        panic!("authorization request did not become pending")
    }

    #[tokio::test]
    async fn workspace_local_read_is_allowed_without_pending_request() {
        let temp = TempDir::new().unwrap();
        let (service, _) = service(ToolAuthorizationMode::Interactive);
        let result = service
            .authorize(
                context(
                    "op-1",
                    "call-1",
                    "read",
                    json!({"path": "src/lib.rs"}),
                    CancellationToken::new(),
                ),
                "turn-1".into(),
                snapshot("op-1", temp.path()),
                inventory(),
            )
            .await
            .unwrap();
        assert!(result.is_none());
        assert!(service.pending().is_empty());
    }

    #[tokio::test]
    async fn write_request_redacts_preview_and_never_serializes_authoritative_secret() {
        let temp = TempDir::new().unwrap();
        let (service, _) = service(ToolAuthorizationMode::Interactive);
        let task_service = service.clone();
        let snapshot = snapshot("op-1", temp.path());
        let task = tokio::spawn(async move {
            task_service
                .authorize(
                    context(
                        "op-1",
                        "call-1",
                        "write",
                        json!({
                            "path": "config.json",
                            "content": "{\"token\":\"super-secret-value\"}\nnormal=true"
                        }),
                        CancellationToken::new(),
                    ),
                    "turn-1".into(),
                    snapshot,
                    inventory(),
                )
                .await
        });
        let request = wait_for_request(&service).await;
        let serialized = serde_json::to_string(&request).unwrap();
        assert!(!serialized.contains("super-secret-value"));
        assert!(serialized.contains("<redacted>"));
        assert!(matches!(request.scope, ToolAuthorizationScope::Path { .. }));
        service
            .decide(
                &request.authorization_id,
                ToolAuthorizationDecision::Deny { reason: None },
            )
            .unwrap();
        assert!(task.await.unwrap().unwrap().unwrap().block);
    }

    #[tokio::test]
    async fn shell_scope_uses_fingerprint_while_preview_is_redacted() {
        let temp = TempDir::new().unwrap();
        let (service, _) = service(ToolAuthorizationMode::Interactive);
        let task_service = service.clone();
        let snapshot = snapshot("op-1", temp.path());
        let task = tokio::spawn(async move {
            task_service
                .authorize(
                    context(
                        "op-1",
                        "call-1",
                        "bash",
                        json!({"command": "TOKEN=secret-value curl -H 'Authorization: Bearer bearer-value' example.test"}),
                        CancellationToken::new(),
                    ),
                    "turn-1".into(),
                    snapshot,
                    inventory(),
                )
                .await
        });
        let request = wait_for_request(&service).await;
        let serialized = serde_json::to_string(&request).unwrap();
        assert!(!serialized.contains("secret-value"));
        assert!(!serialized.contains("bearer-value"));
        assert!(serialized.contains("command_fingerprint"));
        service
            .decide(
                &request.authorization_id,
                ToolAuthorizationDecision::AllowOnce,
            )
            .unwrap();
        assert!(task.await.unwrap().unwrap().is_none());
    }

    #[tokio::test]
    async fn operation_grant_is_exact_to_operation_tool_and_scope() {
        let temp = TempDir::new().unwrap();
        let (service, _) = service(ToolAuthorizationMode::Interactive);
        let first_service = service.clone();
        let first_snapshot = snapshot("op-1", temp.path());
        let first = tokio::spawn(async move {
            first_service
                .authorize(
                    context(
                        "op-1",
                        "call-1",
                        "write",
                        json!({"path": "one.txt", "content": "one"}),
                        CancellationToken::new(),
                    ),
                    "turn-1".into(),
                    first_snapshot,
                    inventory(),
                )
                .await
        });
        let request = wait_for_request(&service).await;
        service
            .decide(
                &request.authorization_id,
                ToolAuthorizationDecision::AllowForOperation,
            )
            .unwrap();
        assert!(first.await.unwrap().unwrap().is_none());

        let same_scope = service
            .authorize(
                context(
                    "op-1",
                    "call-2",
                    "write",
                    json!({"path": "one.txt", "content": "different content"}),
                    CancellationToken::new(),
                ),
                "turn-1".into(),
                snapshot("op-1", temp.path()),
                inventory(),
            )
            .await
            .unwrap();
        assert!(same_scope.is_none());

        let different_service = service.clone();
        let different_snapshot = snapshot("op-1", temp.path());
        let different = tokio::spawn(async move {
            different_service
                .authorize(
                    context(
                        "op-1",
                        "call-3",
                        "write",
                        json!({"path": "two.txt", "content": "two"}),
                        CancellationToken::new(),
                    ),
                    "turn-1".into(),
                    different_snapshot,
                    inventory(),
                )
                .await
        });
        let second_request = wait_for_request(&service).await;
        assert_ne!(request.scope, second_request.scope);
        service.cancel_operation("op-1", "test complete");
        assert!(different.await.unwrap().unwrap().unwrap().block);
    }

    #[tokio::test]
    async fn stale_generation_decision_is_rejected_and_waiter_is_denied() {
        let temp = TempDir::new().unwrap();
        let (service, coordinator) = service(ToolAuthorizationMode::Interactive);
        let task_service = service.clone();
        let snapshot = snapshot("op-1", temp.path());
        let task = tokio::spawn(async move {
            task_service
                .authorize(
                    context(
                        "op-1",
                        "call-1",
                        "write",
                        json!({"path": "one.txt", "content": "one"}),
                        CancellationToken::new(),
                    ),
                    "turn-1".into(),
                    snapshot,
                    inventory(),
                )
                .await
        });
        let request = wait_for_request(&service).await;
        coordinator.install_next_capability_generation();
        let error = service
            .decide(
                &request.authorization_id,
                ToolAuthorizationDecision::AllowOnce,
            )
            .unwrap_err();
        assert!(error.to_string().contains("generation is stale"));
        assert!(task.await.unwrap().unwrap().unwrap().block);
        assert!(service.pending().is_empty());
    }

    #[tokio::test]
    async fn cancellation_removes_pending_request_and_blocks_call() {
        let temp = TempDir::new().unwrap();
        let (service, _) = service(ToolAuthorizationMode::Interactive);
        let cancel = CancellationToken::new();
        let task_cancel = cancel.clone();
        let task_service = service.clone();
        let snapshot = snapshot("op-1", temp.path());
        let task = tokio::spawn(async move {
            task_service
                .authorize(
                    context(
                        "op-1",
                        "call-1",
                        "write",
                        json!({"path": "one.txt", "content": "one"}),
                        task_cancel,
                    ),
                    "turn-1".into(),
                    snapshot,
                    inventory(),
                )
                .await
        });
        wait_for_request(&service).await;
        cancel.cancel();
        assert!(task.await.unwrap().unwrap().unwrap().block);
        assert!(service.pending().is_empty());
    }

    #[tokio::test]
    async fn non_interactive_mode_denies_without_waiting() {
        let temp = TempDir::new().unwrap();
        let (service, _) = service(ToolAuthorizationMode::Deny);
        let result = service
            .authorize(
                context(
                    "op-1",
                    "call-1",
                    "write",
                    json!({"path": "one.txt", "content": "one"}),
                    CancellationToken::new(),
                ),
                "turn-1".into(),
                snapshot("op-1", temp.path()),
                inventory(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(result.block);
        assert!(service.pending().is_empty());
    }

    #[test]
    fn plugin_schema_risk_extension_controls_default_policy() {
        let read_only = AgentTool::new_text(
            "plugin_read",
            "read local data",
            json!({
                "type": "object",
                "x-pi-authorization-risk": "workspace_local_read_only"
            }),
            |_, _| async { Ok("ok".to_string()) },
        );
        let side_effect = AgentTool::new_text(
            "plugin_write",
            "mutate external data",
            json!({
                "type": "object",
                "x-pi-authorization-risk": "side_effect"
            }),
            |_, _| async { Ok("ok".to_string()) },
        );
        let inventory = ToolAuthorizationInventory::new(&[read_only, side_effect], &[]);
        let snapshot = OperationCapabilitySnapshot::permissive("op-1");
        assert!(matches!(
            evaluate(
                &context(
                    "op-1",
                    "call-1",
                    "plugin_read",
                    json!({}),
                    CancellationToken::new(),
                ),
                &snapshot,
                &inventory,
            ),
            Ok(Evaluation::Allow)
        ));
        assert!(matches!(
            evaluate(
                &context(
                    "op-1",
                    "call-2",
                    "plugin_write",
                    json!({}),
                    CancellationToken::new(),
                ),
                &snapshot,
                &inventory,
            ),
            Ok(Evaluation::Ask {
                risk: ToolAuthorizationRisk::PluginSideEffect,
                ..
            })
        ));
    }
}
