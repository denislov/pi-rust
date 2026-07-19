use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CodingAgentExtensionPermission {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
