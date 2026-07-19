mod activation;
mod api;
#[allow(dead_code)]
mod grant;
#[allow(dead_code)]
mod host;
mod lock;
mod manifest;
mod package;
mod platform;
#[allow(dead_code)]
mod runtime;
mod store;

pub use api::{
    CodingAgentExtensionActivation, CodingAgentExtensionActivationRequest,
    CodingAgentExtensionGrantRequest, CodingAgentExtensionPermission,
    CodingAgentExtensionSourceChannel, CodingAgentExtensionTrustLevel,
    CodingAgentInstalledExtensionPackage,
};
pub(crate) use platform::ExtensionPlatformOwner;
