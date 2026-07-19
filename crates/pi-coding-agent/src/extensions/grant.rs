use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use super::api::{
    CodingAgentExtensionPermission, CodingAgentExtensionSourceChannel,
    CodingAgentExtensionTrustLevel,
};

const GRANT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum ExtensionPermission {
    #[serde(rename = "model.invoke")]
    ModelInvoke,
    #[serde(rename = "process.exec")]
    ProcessExec,
    #[serde(rename = "session.read")]
    SessionRead,
    #[serde(rename = "session.write")]
    SessionWrite,
    #[serde(rename = "ui.interact")]
    UiInteract,
    #[serde(rename = "workspace.read")]
    WorkspaceRead,
    #[serde(rename = "workspace.write")]
    WorkspaceWrite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ExtensionGrantIdentity {
    pub(crate) id: String,
    pub(crate) version: String,
    pub(crate) package_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExtensionSourceChannel {
    Bundled,
    Local,
    Registry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExtensionTrustLevel {
    Untrusted,
    Verified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ExtensionGrantScope {
    pub(crate) workspace_id: String,
    #[serde(default)]
    pub(crate) session_ids: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GrantRecord {
    schema_version: u32,
    pub(crate) extension: ExtensionGrantIdentity,
    pub(crate) source_channel: ExtensionSourceChannel,
    pub(crate) source_digest: String,
    pub(crate) trust: ExtensionTrustLevel,
    pub(crate) scope: ExtensionGrantScope,
    pub(crate) permissions: BTreeSet<ExtensionPermission>,
    pub(crate) contract_world: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExtensionOperationScope {
    pub(crate) workspace_id: String,
    pub(crate) session_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ExtensionGrantGeneration(u64);

#[derive(Debug)]
pub(crate) struct ExtensionInstanceGrant {
    record: Arc<GrantRecord>,
    generation: ExtensionGrantGeneration,
    revoked: CancellationToken,
}

#[derive(Debug, Clone)]
pub(crate) struct OperationCapabilityLease {
    identity: ExtensionGrantIdentity,
    generation: ExtensionGrantGeneration,
    operation_id: String,
    scope: ExtensionOperationScope,
    permissions: BTreeSet<ExtensionPermission>,
    deadline: Instant,
    instance_revoked: CancellationToken,
    operation_cancelled: CancellationToken,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RevokedExtensionGrant {
    pub(crate) extension_id: String,
    pub(crate) generation: ExtensionGrantGeneration,
}

#[derive(Debug, Default)]
pub(crate) struct ExtensionGrantRegistry {
    state: Mutex<ExtensionGrantRegistryState>,
}

#[derive(Debug, Default)]
struct ExtensionGrantRegistryState {
    next_generation: u64,
    active: BTreeMap<(String, String), Arc<ExtensionInstanceGrant>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum ExtensionGrantError {
    #[error("invalid extension grant: {0}")]
    InvalidRecord(String),
    #[error("unsupported extension permission: {0}")]
    UnsupportedPermission(String),
    #[error("extension grant is not active")]
    NotActive,
    #[error("extension operation lease is revoked")]
    Revoked,
    #[error("extension operation lease is cancelled")]
    Cancelled,
    #[error("extension operation lease deadline exceeded")]
    DeadlineExceeded,
    #[error("extension operation lease identity mismatch")]
    IdentityMismatch,
    #[error("extension operation lease scope denied")]
    ScopeDenied,
    #[error("extension operation lease permission denied")]
    PermissionDenied,
    #[error("extension operation result belongs to a stale generation")]
    StaleGeneration,
}

impl ExtensionPermission {
    pub(crate) fn parse(value: &str) -> Result<Self, ExtensionGrantError> {
        match value {
            "model.invoke" => Ok(Self::ModelInvoke),
            "process.exec" => Ok(Self::ProcessExec),
            "session.read" => Ok(Self::SessionRead),
            "session.write" => Ok(Self::SessionWrite),
            "ui.interact" => Ok(Self::UiInteract),
            "workspace.read" => Ok(Self::WorkspaceRead),
            "workspace.write" => Ok(Self::WorkspaceWrite),
            other => Err(ExtensionGrantError::UnsupportedPermission(other.into())),
        }
    }

    fn requires_session(self) -> bool {
        matches!(self, Self::SessionRead | Self::SessionWrite)
    }
}

impl From<CodingAgentExtensionPermission> for ExtensionPermission {
    fn from(value: CodingAgentExtensionPermission) -> Self {
        match value {
            CodingAgentExtensionPermission::ModelInvoke => Self::ModelInvoke,
            CodingAgentExtensionPermission::ProcessExec => Self::ProcessExec,
            CodingAgentExtensionPermission::SessionRead => Self::SessionRead,
            CodingAgentExtensionPermission::SessionWrite => Self::SessionWrite,
            CodingAgentExtensionPermission::UiInteract => Self::UiInteract,
            CodingAgentExtensionPermission::WorkspaceRead => Self::WorkspaceRead,
            CodingAgentExtensionPermission::WorkspaceWrite => Self::WorkspaceWrite,
        }
    }
}

impl From<CodingAgentExtensionSourceChannel> for ExtensionSourceChannel {
    fn from(value: CodingAgentExtensionSourceChannel) -> Self {
        match value {
            CodingAgentExtensionSourceChannel::Bundled => Self::Bundled,
            CodingAgentExtensionSourceChannel::Local => Self::Local,
            CodingAgentExtensionSourceChannel::Registry => Self::Registry,
        }
    }
}

impl From<CodingAgentExtensionTrustLevel> for ExtensionTrustLevel {
    fn from(value: CodingAgentExtensionTrustLevel) -> Self {
        match value {
            CodingAgentExtensionTrustLevel::Untrusted => Self::Untrusted,
            CodingAgentExtensionTrustLevel::Verified => Self::Verified,
        }
    }
}

impl GrantRecord {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        extension: ExtensionGrantIdentity,
        source_channel: ExtensionSourceChannel,
        source_digest: String,
        trust: ExtensionTrustLevel,
        scope: ExtensionGrantScope,
        requested_permissions: impl IntoIterator<Item = impl AsRef<str>>,
        granted_permissions: impl IntoIterator<Item = ExtensionPermission>,
        contract_world: String,
    ) -> Result<Self, ExtensionGrantError> {
        let requested = requested_permissions
            .into_iter()
            .map(|permission| ExtensionPermission::parse(permission.as_ref()))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let permissions = granted_permissions.into_iter().collect::<BTreeSet<_>>();
        if !permissions.is_subset(&requested) {
            return Err(invalid("granted permissions must be requested"));
        }
        let record = Self {
            schema_version: GRANT_SCHEMA_VERSION,
            extension,
            source_channel,
            source_digest,
            trust,
            scope,
            permissions,
            contract_world,
        };
        record.validate()?;
        Ok(record)
    }

    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, ExtensionGrantError> {
        let record: Self = serde_json::from_slice(bytes)
            .map_err(|error| invalid(format!("invalid JSON: {error}")))?;
        record.validate()?;
        Ok(record)
    }

    pub(crate) fn to_json(&self) -> Result<Vec<u8>, ExtensionGrantError> {
        serde_json::to_vec_pretty(self)
            .map_err(|error| invalid(format!("cannot serialize record: {error}")))
    }

    fn validate(&self) -> Result<(), ExtensionGrantError> {
        require(
            self.schema_version == GRANT_SCHEMA_VERSION,
            "schemaVersion must be 1",
        )?;
        require(valid_id(&self.extension.id), "extension id is invalid")?;
        semver::Version::parse(&self.extension.version)
            .map_err(|error| invalid(format!("extension version is invalid: {error}")))?;
        require(
            valid_digest(&self.extension.package_digest),
            "package digest is invalid",
        )?;
        require(
            valid_digest(&self.source_digest),
            "source digest is invalid",
        )?;
        require(
            !self.scope.workspace_id.is_empty(),
            "workspace id is required",
        )?;
        require(
            self.scope.workspace_id.len() <= 256,
            "workspace id is too long",
        )?;
        require(
            self.scope.session_ids.len() <= 256,
            "too many session scopes",
        )?;
        require(
            self.scope
                .session_ids
                .iter()
                .all(|id| !id.is_empty() && id.len() <= 256),
            "session ids must contain 1 to 256 bytes",
        )?;
        require(self.permissions.len() <= 32, "too many permissions")?;
        require(
            self.contract_world == "pi:extension/extension@0.1.0",
            "contract world is unsupported",
        )
    }
}

impl ExtensionGrantRegistry {
    pub(crate) fn install(
        &self,
        record: GrantRecord,
    ) -> Result<Arc<ExtensionInstanceGrant>, ExtensionGrantError> {
        record.validate()?;
        let key = (
            record.scope.workspace_id.clone(),
            record.extension.id.clone(),
        );
        let mut state = self.state.lock().expect("extension grant lock poisoned");
        if let Some(previous) = state.active.remove(&key) {
            previous.revoked.cancel();
        }
        state.next_generation = state
            .next_generation
            .checked_add(1)
            .ok_or_else(|| invalid("extension grant generation exhausted"))?;
        let grant = Arc::new(ExtensionInstanceGrant {
            record: Arc::new(record),
            generation: ExtensionGrantGeneration(state.next_generation),
            revoked: CancellationToken::new(),
        });
        state.active.insert(key, grant.clone());
        Ok(grant)
    }

    pub(super) fn replace_workspace(
        &self,
        workspace_id: &str,
        records: Vec<GrantRecord>,
    ) -> Result<Vec<Arc<ExtensionInstanceGrant>>, ExtensionGrantError> {
        for record in &records {
            record.validate()?;
            if record.scope.workspace_id != workspace_id {
                return Err(ExtensionGrantError::ScopeDenied);
            }
        }
        let mut state = self.state.lock().expect("extension grant lock poisoned");
        let existing = state
            .active
            .iter()
            .filter(|((workspace, _), _)| workspace == workspace_id)
            .map(|((_, id), grant)| (id.as_str(), grant))
            .collect::<BTreeMap<_, _>>();
        let requested = records
            .iter()
            .map(|record| (record.extension.id.as_str(), record))
            .collect::<BTreeMap<_, _>>();
        if requested.len() != records.len() || records.len() > 256 {
            return Err(invalid(
                "workspace grants must contain at most 256 unique extension ids",
            ));
        }
        if existing.len() == requested.len()
            && requested.iter().all(|(id, record)| {
                existing
                    .get(id)
                    .is_some_and(|grant| grant.record.as_ref() == *record)
            })
        {
            return Ok(existing.into_values().cloned().collect());
        }
        let generation_end = state
            .next_generation
            .checked_add(records.len() as u64)
            .ok_or_else(|| invalid("extension grant generation exhausted"))?;
        let old_keys = state
            .active
            .keys()
            .filter(|(workspace, _)| workspace == workspace_id)
            .cloned()
            .collect::<Vec<_>>();
        for key in old_keys {
            if let Some(old) = state.active.remove(&key) {
                old.revoked.cancel();
            }
        }

        let mut installed = Vec::with_capacity(records.len());
        for record in records {
            state.next_generation += 1;
            let key = (workspace_id.to_owned(), record.extension.id.clone());
            let grant = Arc::new(ExtensionInstanceGrant {
                record: Arc::new(record),
                generation: ExtensionGrantGeneration(state.next_generation),
                revoked: CancellationToken::new(),
            });
            state.active.insert(key, grant.clone());
            installed.push(grant);
        }
        debug_assert_eq!(state.next_generation, generation_end);
        Ok(installed)
    }

    pub(crate) fn revoke(
        &self,
        workspace_id: &str,
        extension_id: &str,
    ) -> Option<RevokedExtensionGrant> {
        let grant = self
            .state
            .lock()
            .expect("extension grant lock poisoned")
            .active
            .remove(&(workspace_id.into(), extension_id.into()))?;
        grant.revoked.cancel();
        Some(RevokedExtensionGrant {
            extension_id: extension_id.into(),
            generation: grant.generation,
        })
    }

    pub(crate) fn admit(
        &self,
        workspace_id: &str,
        extension_id: &str,
        operation_id: String,
        scope: ExtensionOperationScope,
        deadline: Instant,
        operation_cancelled: CancellationToken,
    ) -> Result<OperationCapabilityLease, ExtensionGrantError> {
        let grant = self
            .state
            .lock()
            .expect("extension grant lock poisoned")
            .active
            .get(&(workspace_id.into(), extension_id.into()))
            .cloned()
            .ok_or(ExtensionGrantError::NotActive)?;
        grant.mint_lease(operation_id, scope, deadline, operation_cancelled)
    }

    pub(crate) fn validate_late_result(
        &self,
        lease: &OperationCapabilityLease,
    ) -> Result<(), ExtensionGrantError> {
        lease.validate_liveness()?;
        let state = self.state.lock().expect("extension grant lock poisoned");
        let active = state
            .active
            .get(&(lease.scope.workspace_id.clone(), lease.identity.id.clone()))
            .ok_or(ExtensionGrantError::StaleGeneration)?;
        if active.generation != lease.generation
            || active.record.extension.package_digest != lease.identity.package_digest
        {
            return Err(ExtensionGrantError::StaleGeneration);
        }
        Ok(())
    }
}

impl ExtensionInstanceGrant {
    fn mint_lease(
        &self,
        operation_id: String,
        scope: ExtensionOperationScope,
        deadline: Instant,
        operation_cancelled: CancellationToken,
    ) -> Result<OperationCapabilityLease, ExtensionGrantError> {
        if self.revoked.is_cancelled() {
            return Err(ExtensionGrantError::Revoked);
        }
        require(!operation_id.is_empty(), "operation id is required")?;
        if deadline <= Instant::now() {
            return Err(ExtensionGrantError::DeadlineExceeded);
        }
        if scope.workspace_id != self.record.scope.workspace_id
            || scope
                .session_id
                .as_ref()
                .is_some_and(|session_id| !self.record.scope.session_ids.contains(session_id))
        {
            return Err(ExtensionGrantError::ScopeDenied);
        }
        Ok(OperationCapabilityLease {
            identity: self.record.extension.clone(),
            generation: self.generation,
            operation_id,
            scope,
            permissions: self.record.permissions.clone(),
            deadline,
            instance_revoked: self.revoked.clone(),
            operation_cancelled,
        })
    }
}

impl OperationCapabilityLease {
    pub(crate) fn authorize(
        &self,
        operation_id: &str,
        scope: &ExtensionOperationScope,
        permission: ExtensionPermission,
    ) -> Result<(), ExtensionGrantError> {
        self.validate_liveness()?;
        if operation_id != self.operation_id {
            return Err(ExtensionGrantError::IdentityMismatch);
        }
        if scope != &self.scope || (permission.requires_session() && scope.session_id.is_none()) {
            return Err(ExtensionGrantError::ScopeDenied);
        }
        if !self.permissions.contains(&permission) {
            return Err(ExtensionGrantError::PermissionDenied);
        }
        Ok(())
    }

    fn validate_liveness(&self) -> Result<(), ExtensionGrantError> {
        if self.instance_revoked.is_cancelled() {
            return Err(ExtensionGrantError::Revoked);
        }
        if self.operation_cancelled.is_cancelled() {
            return Err(ExtensionGrantError::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(ExtensionGrantError::DeadlineExceeded);
        }
        Ok(())
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_lowercase()
        && value.split(['.', '-']).all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn require(condition: bool, message: &str) -> Result<(), ExtensionGrantError> {
    condition.then_some(()).ok_or_else(|| invalid(message))
}

fn invalid(message: impl Into<String>) -> ExtensionGrantError {
    ExtensionGrantError::InvalidRecord(message.into())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn record() -> GrantRecord {
        GrantRecord::new(
            ExtensionGrantIdentity {
                id: "example.review".into(),
                version: "1.2.3".into(),
                package_digest: "a".repeat(64),
            },
            ExtensionSourceChannel::Registry,
            "b".repeat(64),
            ExtensionTrustLevel::Verified,
            ExtensionGrantScope {
                workspace_id: "workspace-1".into(),
                session_ids: BTreeSet::from(["session-1".into()]),
            },
            ["workspace.read", "session.read"],
            [
                ExtensionPermission::WorkspaceRead,
                ExtensionPermission::SessionRead,
            ],
            "pi:extension/extension@0.1.0".into(),
        )
        .unwrap()
    }

    fn scope() -> ExtensionOperationScope {
        ExtensionOperationScope {
            workspace_id: "workspace-1".into(),
            session_id: Some("session-1".into()),
        }
    }

    fn lease(registry: &ExtensionGrantRegistry) -> OperationCapabilityLease {
        registry.install(record()).unwrap();
        registry
            .admit(
                "workspace-1",
                "example.review",
                "op-1".into(),
                scope(),
                Instant::now() + Duration::from_secs(30),
                CancellationToken::new(),
            )
            .unwrap()
    }

    #[test]
    fn grant_record_round_trips_without_authorization_material() {
        let record = record();
        let json = record.to_json().unwrap();
        let decoded = GrantRecord::parse(&json).unwrap();

        assert_eq!(decoded, record);
        let text = String::from_utf8(json).unwrap();
        for forbidden in ["token", "secret", "credential", "authorizationSubject"] {
            assert!(!text.contains(forbidden));
        }
    }

    #[test]
    fn rejects_unknown_and_unrequested_permissions() {
        assert!(matches!(
            GrantRecord::new(
                record().extension,
                ExtensionSourceChannel::Local,
                "b".repeat(64),
                ExtensionTrustLevel::Untrusted,
                record().scope,
                ["network.raw"],
                [],
                "pi:extension/extension@0.1.0".into(),
            ),
            Err(ExtensionGrantError::UnsupportedPermission(_))
        ));
        assert!(
            GrantRecord::new(
                record().extension,
                ExtensionSourceChannel::Local,
                "b".repeat(64),
                ExtensionTrustLevel::Untrusted,
                record().scope,
                ["workspace.read"],
                [ExtensionPermission::WorkspaceWrite],
                "pi:extension/extension@0.1.0".into(),
            )
            .is_err()
        );
    }

    #[test]
    fn lease_checks_operation_scope_permission_and_cancellation() {
        let registry = ExtensionGrantRegistry::default();
        let cancelled = CancellationToken::new();
        registry.install(record()).unwrap();
        let lease = registry
            .admit(
                "workspace-1",
                "example.review",
                "op-1".into(),
                scope(),
                Instant::now() + Duration::from_secs(30),
                cancelled.clone(),
            )
            .unwrap();

        lease
            .authorize("op-1", &scope(), ExtensionPermission::SessionRead)
            .unwrap();
        assert_eq!(
            lease.authorize("op-other", &scope(), ExtensionPermission::SessionRead),
            Err(ExtensionGrantError::IdentityMismatch)
        );
        assert_eq!(
            lease.authorize("op-1", &scope(), ExtensionPermission::WorkspaceWrite),
            Err(ExtensionGrantError::PermissionDenied)
        );
        cancelled.cancel();
        assert_eq!(
            lease.authorize("op-1", &scope(), ExtensionPermission::SessionRead),
            Err(ExtensionGrantError::Cancelled)
        );
    }

    #[test]
    fn revoke_blocks_admission_and_rejects_late_results() {
        let registry = ExtensionGrantRegistry::default();
        let lease = lease(&registry);
        let revoked = registry.revoke("workspace-1", "example.review").unwrap();

        assert_eq!(revoked.extension_id, "example.review");
        assert_eq!(
            registry.validate_late_result(&lease),
            Err(ExtensionGrantError::Revoked)
        );
        assert!(matches!(
            registry.admit(
                "workspace-1",
                "example.review",
                "op-2".into(),
                scope(),
                Instant::now() + Duration::from_secs(30),
                CancellationToken::new(),
            ),
            Err(ExtensionGrantError::NotActive)
        ));
    }

    #[test]
    fn replacement_generation_revokes_old_lease_and_accepts_new_result() {
        let registry = ExtensionGrantRegistry::default();
        let old = lease(&registry);
        registry.install(record()).unwrap();
        let new = registry
            .admit(
                "workspace-1",
                "example.review",
                "op-2".into(),
                scope(),
                Instant::now() + Duration::from_secs(30),
                CancellationToken::new(),
            )
            .unwrap();

        assert_eq!(
            registry.validate_late_result(&old),
            Err(ExtensionGrantError::Revoked)
        );
        registry.validate_late_result(&new).unwrap();
    }

    #[test]
    fn identical_workspace_replacement_preserves_generation_and_lease() {
        let registry = ExtensionGrantRegistry::default();
        let old = lease(&registry);

        registry
            .replace_workspace("workspace-1", vec![record()])
            .unwrap();

        registry.validate_late_result(&old).unwrap();
        old.authorize("op-1", &scope(), ExtensionPermission::SessionRead)
            .unwrap();
    }
}
