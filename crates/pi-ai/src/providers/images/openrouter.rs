use serde::Serialize;

use crate::images::{
    AssistantImages, ImageContent, ImageInput, ImageOutput, ImagesContext, ImagesModel,
    ImagesModelOutput, ImagesUsage, TextContent,
};

#[derive(Debug, Clone, Serialize)]
pub struct OpenRouterImageRequest {
    pub model: String,
    pub messages: Vec<serde_json::Value>,
    pub stream: bool,
    pub modalities: Vec<String>,
}

pub fn build_request(model: &ImagesModel, context: &ImagesContext) -> OpenRouterImageRequest {
    let content = context
        .input
        .iter()
        .map(|item| match item {
            ImageInput::Text(TextContent { text }) => {
                serde_json::json!({"type": "text", "text": text})
            }
            ImageInput::Image(ImageContent { data, mime_type }) => serde_json::json!({
                "type": "image_url",
                "image_url": {"url": format!("data:{};base64,{}", mime_type, data)}
            }),
        })
        .collect::<Vec<_>>();
    OpenRouterImageRequest {
        model: model.id.clone(),
        messages: vec![serde_json::json!({"role": "user", "content": content})],
        stream: false,
        modalities: if model.output.contains(&ImagesModelOutput::Text) {
            vec!["image".into(), "text".into()]
        } else {
            vec!["image".into()]
        },
    }
}

pub fn response_to_images(
    model: &ImagesModel,
    response: serde_json::Value,
) -> Result<AssistantImages, String> {
    let mut output = AssistantImages {
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        response_id: response
            .get("id")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        output: Vec::new(),
        stop_reason: "stop".into(),
        usage: response.get("usage").map(parse_usage),
        error_message: None,
        timestamp: 0,
    };

    if let Some(choice) = response
        .get("choices")
        .and_then(|v| v.as_array())
        .and_then(|choices| choices.first())
        && let Some(message) = choice.get("message")
    {
        if let Some(content) = message.get("content").and_then(|v| v.as_str())
            && !content.is_empty()
        {
            output.output.push(ImageOutput::Text(TextContent {
                text: content.into(),
            }));
        }
        if let Some(images) = message.get("images").and_then(|v| v.as_array()) {
            for image in images {
                let url = image.get("image_url").and_then(|v| v.as_str()).or_else(|| {
                    image
                        .get("image_url")
                        .and_then(|v| v.get("url"))
                        .and_then(|v| v.as_str())
                });
                if let Some((mime_type, data)) = parse_data_url(url.unwrap_or_default()) {
                    output
                        .output
                        .push(ImageOutput::Image(ImageContent { mime_type, data }));
                }
            }
        }
    }
    Ok(output)
}

fn parse_usage(value: &serde_json::Value) -> ImagesUsage {
    let prompt = value
        .get("prompt_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let completion = value
        .get("completion_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let details = value.get("prompt_tokens_details");
    let cached = details
        .and_then(|v| v.get("cached_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let cache_write = details
        .and_then(|v| v.get("cache_write_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let cache_read = if cache_write > 0 {
        cached.saturating_sub(cache_write)
    } else {
        cached
    };
    let input = prompt.saturating_sub(cache_read + cache_write);
    ImagesUsage {
        input,
        output: completion,
        cache_read,
        cache_write,
        total_tokens: input + completion + cache_read + cache_write,
    }
}

fn parse_data_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("data:")?;
    let (mime_type, data) = rest.split_once(";base64,")?;
    Some((mime_type.into(), data.into()))
}
