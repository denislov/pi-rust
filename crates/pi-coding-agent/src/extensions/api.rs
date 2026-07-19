use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CodingAgentExtensionPermission {
    #[serde(rename = "model.invoke")]
    ModelInvoke,
    #[serde(rename = "process.exec")]
    ProcessExec,
    #[serde(rename = "ui.interact")]
    UiInteract,
    #[serde(rename = "workspace.read")]
    WorkspaceRead,
    #[serde(rename = "workspace.write")]
    WorkspaceWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingAgentExtensionSourceChannel {
    Bundled,
    Local,
    Registry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingAgentExtensionTrustLevel {
    Untrusted,
    Verified,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodingAgentExtensionGrantRequest {
    pub package_digest: String,
    pub source_channel: CodingAgentExtensionSourceChannel,
    pub source_digest: String,
    pub trust: CodingAgentExtensionTrustLevel,
    #[serde(default)]
    pub session_ids: BTreeSet<String>,
    #[serde(default)]
    pub permissions: BTreeSet<CodingAgentExtensionPermission>,
}

impl std::fmt::Debug for CodingAgentExtensionGrantRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CodingAgentExtensionGrantRequest")
            .field("package_digest", &self.package_digest)
            .field("source_channel", &self.source_channel)
            .field("source_digest", &"<redacted>")
            .field("trust", &self.trust)
            .field("session_scope_count", &self.session_ids.len())
            .field("permission_count", &self.permissions.len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodingAgentExtensionActivationRequest {
    pub workspace_id: String,
    pub root_package_digests: Vec<String>,
    pub grants: Vec<CodingAgentExtensionGrantRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodingAgentInstalledExtensionPackage {
    pub id: String,
    pub version: String,
    pub package_digest: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodingAgentExtensionActivation {
    pub workspace_id: String,
    pub root_package_digests: Vec<String>,
    pub packages: Vec<CodingAgentInstalledExtensionPackage>,
}
