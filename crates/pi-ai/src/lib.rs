#[doc(hidden)]
pub mod compat;
#[doc(hidden)]
pub mod images;
#[doc(hidden)]
pub mod models;
#[doc(hidden)]
pub mod providers;
#[doc(hidden)]
pub mod registry;
#[doc(hidden)]
pub mod stream;
#[doc(hidden)]
pub mod transport;
#[doc(hidden)]
pub mod types;
#[doc(hidden)]
pub mod util;

pub use models::{all_models, calculate_cost, get_model, get_models, get_providers, lookup_model};
pub use providers::{builtin_provider_apis, register_builtins_into};
#[deprecated(
    note = "use AiClient or ProviderRegistry for scoped provider runtime registration"
)]
pub use registry::register;
#[deprecated(note = "use AiClient or ProviderRegistry for scoped provider runtime streaming")]
pub use registry::stream_model;
pub use registry::{
    AiClient, ApiProvider, EnvProviderAuthResolver, ProviderAuthResolver, ProviderRegistry,
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
/// prefer this module for APIs that are intended to stay stable. Root-level
/// `register` and `stream_model` remain compatibility helpers outside this
/// stable facade; new runtime code should use `AiClient` or `ProviderRegistry`.
pub mod api {
    pub use crate::compat::{
        AnthropicMessagesCompat, CacheControlFormat, ModelCompat, OpenAICompletionsCompat,
        OpenAIResponsesCompat, OpenRouterRouting, ThinkingFormat, ThinkingLevelMap,
        ThinkingLevelValue, VercelGatewayRouting,
    };
    pub use crate::images::{
        AssistantImages, ImageContent, ImageInput, ImageOutput, ImagesContext, ImagesModel,
        ImagesModelCost, ImagesModelOutput, ImagesUsage, TextContent,
    };
    pub use crate::models::{
        all_models, calculate_cost, get_model, get_models, get_providers, lookup_model,
    };
    pub use crate::providers::{builtin_provider_apis, register_builtins_into};
    pub use crate::registry::{
        AiClient, ApiProvider, EnvProviderAuthResolver, ProviderAuthResolver, ProviderRegistry,
    };
    pub use crate::stream::{EventStream, complete};
    pub use crate::transport::error::{ProviderError, ProviderErrorKind};
    pub use crate::types::hooks::{
        ProviderPayloadHook, ProviderPayloadHookFuture, ProviderResponseHook,
        ProviderResponseHookFuture,
    };
    pub use crate::types::{
        AssistantMessage, AssistantMessageDiagnostic, AssistantMessageEvent, ContentBlock, Context,
        Cost, DiagnosticErrorInfo, Message, Model, ModelCost, ModelInput, ProviderResponseInfo,
        ProviderStreamHooks, StopReason, StreamOptions, ThinkingConfig, Tool, Usage,
    };
    pub use crate::util::env_keys::env_api_key;
    pub use crate::util::http::{RetryConfig, is_retryable_status, parse_retry_after_ms};
}
