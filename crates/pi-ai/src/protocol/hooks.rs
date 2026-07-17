use crate::model::Model;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub type ProviderPayloadHookFuture =
    Pin<Box<dyn Future<Output = Result<serde_json::Value, String>> + Send>>;
pub type ProviderResponseHookFuture = Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
pub type ProviderPayloadHook =
    Arc<dyn Fn(Model, serde_json::Value) -> ProviderPayloadHookFuture + Send + Sync>;
pub type ProviderResponseHook =
    Arc<dyn Fn(ProviderResponseInfo) -> ProviderResponseHookFuture + Send + Sync>;

#[derive(Clone, Serialize, Deserialize)]
pub struct ProviderResponseInfo {
    pub status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<serde_json::Value>,
}

impl std::fmt::Debug for ProviderResponseInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderResponseInfo")
            .field("status", &self.status)
            .field("headers", &self.headers.as_ref().map(redacted_headers))
            .finish()
    }
}

fn redacted_headers(headers: &serde_json::Value) -> serde_json::Value {
    let Some(object) = headers.as_object() else {
        return serde_json::Value::String("[REDACTED]".into());
    };
    let mut redacted = serde_json::Map::new();
    for (name, value) in object {
        let sensitive = matches!(
            name.to_ascii_lowercase().as_str(),
            "authorization"
                | "api-key"
                | "x-api-key"
                | "cookie"
                | "set-cookie"
                | "x-amz-security-token"
                | "x-amz-signature"
        );
        redacted.insert(
            name.clone(),
            if sensitive {
                serde_json::Value::String("[REDACTED]".into())
            } else {
                value.clone()
            },
        );
    }
    serde_json::Value::Object(redacted)
}

#[derive(Clone, Default)]
pub struct ProviderStreamHooks {
    pub on_payload: Option<ProviderPayloadHook>,
    pub on_response: Option<ProviderResponseHook>,
}

impl std::fmt::Debug for ProviderStreamHooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderStreamHooks")
            .field("on_payload", &self.on_payload.is_some())
            .field("on_response", &self.on_response.is_some())
            .finish()
    }
}

impl ProviderStreamHooks {
    pub async fn apply_payload(
        &self,
        model: &Model,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        match self.on_payload.as_ref() {
            Some(hook) => hook(model.clone(), payload).await,
            None => Ok(payload),
        }
    }

    pub async fn emit_response(&self, response: ProviderResponseInfo) -> Result<(), String> {
        match self.on_response.as_ref() {
            Some(hook) => hook(response).await,
            None => Ok(()),
        }
    }
}
