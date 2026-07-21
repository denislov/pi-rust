mod activation;
mod api;
#[allow(
    dead_code,
    reason = "ADR-002 lease runtime is retained while contribution dispatch remains explicitly skipped"
)]
mod grant;
#[allow(
    dead_code,
    reason = "lease-only Host API boundary is retained while contribution dispatch remains explicitly skipped"
)]
mod host;
mod lock;
mod manifest;
mod package;
mod platform;
#[allow(
    dead_code,
    reason = "minimum Wasmtime invocation boundary is retained while contribution dispatch remains explicitly skipped"
)]
mod runtime;
mod store;

pub use api::{
    CodingAgentExtensionActivation, CodingAgentExtensionActivationRequest,
    CodingAgentExtensionGrantRequest, CodingAgentExtensionPermission,
    CodingAgentExtensionSourceChannel, CodingAgentExtensionTrustLevel,
    CodingAgentInstalledExtensionPackage,
};
pub(crate) use platform::ExtensionPlatformOwner;
