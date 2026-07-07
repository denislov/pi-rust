use super::wire;
use crate::models::calculate_cost;
use crate::providers::process_framework::{SseEventHandler, SseEventResult, process_sse};
use crate::stream::EventStream;
use crate::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Cost, Model, StopReason, Usage,
};
use crate::util::json_repair::parse_streaming_json;
use bytes::Bytes;
use futures::Stream;
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;

pub fn process<E>(
    body: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: Model,
    cancel: Option<CancellationToken>,
) -> EventStream
where
    E: std::fmt::Display + Send + 'static,
{
    process_sse(
        body,
        model,
        cancel,
        CompletionsHandler::default(),
        "openai-completions",
    )
}

#[derive(Default)]
struct CompletionsHandler {
    first_event: bool,
    text_content_index: Option<u32>,
    thinking_content_index: Option<u32>,
    tool_index_map: HashMap<u32, u32>,
    tool_args_acc: HashMap<u32, String>,
    finish_reason: Option<String>,
}

impl SseEventHandler for CompletionsHandler {
    fn handle_event(
        &mut self,
        data: &str,
        partial: &mut AssistantMessage,
        model: &Model,
    ) -> Result<SseEventResult, String> {
        let chunk: wire::ChatCompletionChunk =
            serde_json::from_str(data).map_err(|e| format!("SSE parse error: {}", e))?;

        let mut events = Vec::new();

        if !self.first_event {
            partial.response_id = Some(chunk.id.clone());
            partial.response_model = Some(chunk.model.clone());
            partial.timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            events.push(AssistantMessageEvent::Start {
                content_index: None,
                partial: partial.clone(),
            });
            self.first_event = true;
        }

        for choice in chunk.choices {
            if let Some(fr) = choice.finish_reason.as_deref()
                && !fr.is_empty() && fr != "null" {
                    self.finish_reason = choice.finish_reason.clone();
                }

            if let Some(text_delta) = &choice.delta.content
                && !text_delta.is_empty() {
                    if self.text_content_index.is_none() {
                        self.text_content_index = Some(partial.content.len() as u32);
                        partial.content.push(ContentBlock::Text {
                            text: String::new(),
                            text_signature: None,
                        });
                        events.push(AssistantMessageEvent::TextStart {
                            content_index: self.text_content_index.unwrap(),
                            partial: partial.clone(),
                        });
                    }
                    let ci = self.text_content_index.unwrap();
                    if let Some(ContentBlock::Text { text, .. }) =
                        partial.content.get_mut(ci as usize)
                    {
                        text.push_str(text_delta);
                    }
                    events.push(AssistantMessageEvent::TextDelta {
                        content_index: ci,
                        delta: text_delta.clone(),
                        partial: partial.clone(),
                    });
                }

            if let Some(reasoning_delta) = first_reasoning_delta(&choice.delta)
                && !reasoning_delta.is_empty() {
                    if self.thinking_content_index.is_none() {
                        self.thinking_content_index = Some(partial.content.len() as u32);
                        partial.content.push(ContentBlock::Thinking {
                            thinking: String::new(),
                            thinking_signature: None,
                            redacted: None,
                        });
                        events.push(AssistantMessageEvent::ThinkingStart {
                            content_index: self.thinking_content_index.unwrap(),
                            partial: partial.clone(),
                        });
                    }
                    let ci = self.thinking_content_index.unwrap();
                    if let Some(ContentBlock::Thinking { thinking, .. }) =
                        partial.content.get_mut(ci as usize)
                    {
                        thinking.push_str(reasoning_delta);
                    }
                    events.push(AssistantMessageEvent::ThinkingDelta {
                        content_index: ci,
                        delta: reasoning_delta.to_string(),
                        partial: partial.clone(),
                    });
                }

            if let Some(tc_deltas) = &choice.delta.tool_calls {
                for tc in tc_deltas {
                    let openai_idx = tc.index.unwrap_or(0);
                    let content_pos = match self.tool_index_map.get(&openai_idx) {
                        Some(&pos) => pos,
                        None => {
                            let pos = partial.content.len() as u32;
                            self.tool_index_map.insert(openai_idx, pos);
                            partial.content.push(ContentBlock::ToolCall {
                                id: tc.id.clone().unwrap_or_default(),
                                name: String::new(),
                                arguments: serde_json::json!({}),
                                thought_signature: None,
                            });
                            events.push(AssistantMessageEvent::ToolcallStart {
                                content_index: pos,
                                partial: partial.clone(),
                            });
                            pos
                        }
                    };

                    if let Some(ref id) = tc.id
                        && let Some(ContentBlock::ToolCall { id: block_id, .. }) =
                            partial.content.get_mut(content_pos as usize)
                        {
                            *block_id = id.clone();
                        }
                    if let Some(ref func) = tc.function {
                        if let Some(ref name) = func.name
                            && let Some(ContentBlock::ToolCall {
                                name: block_name,
                                id: block_id,
                                ..
                            }) = partial.content.get_mut(content_pos as usize)
                            {
                                *block_name = name.clone();
                                if let Some(ref id) = tc.id {
                                    *block_id = id.clone();
                                }
                            }
                        if let Some(ref args) = func.arguments {
                            let acc = self.tool_args_acc.entry(openai_idx).or_default();
                            acc.push_str(args);
                            let parsed = parse_streaming_json(acc);
                            if let Some(ContentBlock::ToolCall { arguments, .. }) =
                                partial.content.get_mut(content_pos as usize)
                            {
                                *arguments = parsed;
                            }
                            events.push(AssistantMessageEvent::ToolcallDelta {
                                content_index: content_pos,
                                delta: args.clone(),
                                partial: partial.clone(),
                            });
                        }
                    }
                }
            }
        }

        if let Some(usage) = &chunk.usage {
            partial.usage = map_usage(usage, model);
        }

        Ok(SseEventResult::Continue(events))
    }

    fn finalize(
        &self,
        partial: &mut AssistantMessage,
        _model: &Model,
    ) -> Vec<AssistantMessageEvent> {
        let mut events = Vec::new();

        if let Some(ci) = self.text_content_index {
            events.push(AssistantMessageEvent::TextEnd {
                content_index: ci,
                partial: partial.clone(),
            });
        }

        if let Some(ci) = self.thinking_content_index {
            events.push(AssistantMessageEvent::ThinkingEnd {
                content_index: ci,
                partial: partial.clone(),
            });
        }

        for (oi, pos) in &self.tool_index_map {
            let acc = self.tool_args_acc.get(oi).map(|s| s.as_str()).unwrap_or("");
            let parsed = parse_streaming_json(acc);
            if let Some(ContentBlock::ToolCall { arguments, .. }) =
                partial.content.get_mut(*pos as usize)
            {
                *arguments = parsed;
            }
            events.push(AssistantMessageEvent::ToolcallEnd {
                content_index: *pos,
                partial: partial.clone(),
            });
        }

        partial.stop_reason = map_finish_reason(self.finish_reason.as_deref());

        if partial.usage.total_tokens == 0 {
            partial.usage = Usage::default();
        }

        events
    }
}

fn first_reasoning_delta(delta: &wire::Delta) -> Option<&str> {
    delta
        .reasoning_content
        .as_deref()
        .filter(|value| !value.is_empty())
        .or_else(|| delta.reasoning.as_deref().filter(|value| !value.is_empty()))
        .or_else(|| {
            delta
                .reasoning_text
                .as_deref()
                .filter(|value| !value.is_empty())
        })
}

fn map_finish_reason(reason: Option<&str>) -> StopReason {
    match reason {
        Some("stop") | None => StopReason::Stop,
        Some("length") => StopReason::Length,
        Some("tool_calls") => StopReason::ToolUse,
        Some("content_filter") => StopReason::Error,
        _ => StopReason::Stop,
    }
}

fn map_usage(usage: &wire::ChatUsage, model: &Model) -> Usage {
    let cache_tokens = usage
        .prompt_tokens_details
        .as_ref()
        .map(|d| d.cached_tokens)
        .unwrap_or(0);

    // OpenAI's `prompt_tokens` is the *total* prompt size and already includes
    // the cached subset reported in `cached_tokens`. To match Anthropic-side
    // semantics where `input` excludes cache hits (and to avoid double-billing
    // the cached portion at both input and cache_read rates), subtract the
    // cached tokens from `input`.
    let non_cached_input = usage.prompt_tokens.saturating_sub(cache_tokens);

    let mut result = Usage {
        input: non_cached_input,
        output: usage.completion_tokens,
        cache_read: cache_tokens,
        cache_write: 0,
        total_tokens: if usage.total_tokens == 0 {
            usage.prompt_tokens + usage.completion_tokens
        } else {
            usage.total_tokens
        },
        cost: Cost::default(),
    };
    calculate_cost(model, &mut result);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ModelCost, ModelInput};

    fn test_model() -> Model {
        Model {
            id: "gpt-4o".into(),
            name: "GPT-4o".into(),
            api: "openai".into(),
            provider: "openai".into(),
            base_url: "https://api.openai.com".into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost {
                input: 5.0,
                output: 15.0,
                cache_read: 2.5,
                cache_write: 0.0,
            },
            context_window: 128_000,
            max_tokens: 16_384,
            headers: None,
            compat: None,
        }
    }

    #[test]
    fn map_usage_excludes_cached_tokens_from_input() {
        // OpenAI's `prompt_tokens` includes the cached subset. `input` must
        // hold only the non-cached portion so it isn't double-billed at both
        // the input and cache_read rates.
        let usage = wire::ChatUsage {
            prompt_tokens: 1000,
            completion_tokens: 200,
            total_tokens: 1200,
            prompt_tokens_details: Some(wire::PromptTokensDetails { cached_tokens: 800 }),
            completion_tokens_details: None,
        };
        let mapped = map_usage(&usage, &test_model());
        assert_eq!(mapped.input, 200, "input should exclude cached tokens");
        assert_eq!(mapped.cache_read, 800);
        assert_eq!(mapped.output, 200);
        assert_eq!(mapped.total_tokens, 1200);
    }

    #[test]
    fn map_usage_without_cache_details_keeps_full_input() {
        let usage = wire::ChatUsage {
            prompt_tokens: 1000,
            completion_tokens: 200,
            total_tokens: 1200,
            prompt_tokens_details: None,
            completion_tokens_details: None,
        };
        let mapped = map_usage(&usage, &test_model());
        assert_eq!(mapped.input, 1000);
        assert_eq!(mapped.cache_read, 0);
    }
}
