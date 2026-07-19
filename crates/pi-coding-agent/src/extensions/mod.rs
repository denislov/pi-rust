mod activation;
mod api;
#[allow(dead_code)]
mod grant;
mod lock;
mod manifest;
mod package;
mod platform;
mod store;

pub use api::{
    CodingAgentExtensionActivation, CodingAgentExtensionActivationRequest,
    CodingAgentExtensionGrantRequest, CodingAgentExtensionPermission,
    CodingAgentExtensionSourceChannel, CodingAgentExtensionTrustLevel,
    CodingAgentInstalledExtensionPackage,
};
pub(crate) use platform::ExtensionPlatformOwner;
