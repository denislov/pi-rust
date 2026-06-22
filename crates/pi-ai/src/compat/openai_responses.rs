use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OpenAIResponsesCompat {
    #[serde(rename = "sendSessionIdHeader", default)]
    pub send_session_id_header: Option<bool>,
    #[serde(rename = "supportsLongCacheRetention", default)]
    pub supports_long_cache_retention: Option<bool>,
}
