pub mod compat;
pub mod images;
pub mod models;
pub mod providers;
pub mod registry;
pub mod stream;
pub mod transport;
pub mod types;
pub mod util;

pub use models::{all_models, calculate_cost, get_model, get_models, get_providers, lookup_model};
pub use registry::{
    AiClient, ApiProvider, EnvProviderAuthResolver, ProviderAuthResolver, ProviderRegistry,
    register, stream_model,
};
pub use stream::{EventStream, complete};
pub use types::{
    AssistantMessage, AssistantMessageDiagnostic, AssistantMessageEvent, ContentBlock, Context,
    Cost, DiagnosticErrorInfo, Message, Model, ModelCost, ModelInput, ProviderResponseInfo,
    ProviderStreamHooks, StopReason, StreamOptions, ThinkingConfig, Tool, Usage,
};
pub use util::env_keys::env_api_key;

/// Stable facade for embedding `pi-ai`.
///
/// The root modules remain public during migration. New downstream code should
/// prefer this module for APIs that are intended to stay stable. Global
/// `register` and `stream_model` are re-exported here as compatibility helpers
/// until the scoped provider runtime is introduced.
pub mod api {
    pub use crate::models::{
        all_models, calculate_cost, get_model, get_models, get_providers, lookup_model,
    };
    pub use crate::registry::{
        AiClient, ApiProvider, EnvProviderAuthResolver, ProviderAuthResolver, ProviderRegistry,
        register, stream_model,
    };
    pub use crate::stream::{EventStream, complete};
    pub use crate::types::{
        AssistantMessage, AssistantMessageDiagnostic, AssistantMessageEvent, ContentBlock, Context,
        Cost, DiagnosticErrorInfo, Message, Model, ModelCost, ModelInput, ProviderResponseInfo,
        ProviderStreamHooks, StopReason, StreamOptions, ThinkingConfig, Tool, Usage,
    };
    pub use crate::util::env_keys::env_api_key;
}
