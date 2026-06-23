use crate::agent::AgentState;
use crate::convert::{assemble_context, convert_to_context, default_convert_to_llm};
use crate::types::{AgentMessage, ThinkingLevel};
use pi_ai::types::{Context, Message, Model, StreamOptions, ThinkingConfig};
use std::sync::{Arc, RwLock};
use tokio_util::sync::CancellationToken;

pub(crate) struct PreparedProviderRequest {
    pub model: Model,
    pub context: Context,
    pub stream_options: StreamOptions,
}

pub(crate) fn prepare_provider_request(
    state: &Arc<RwLock<AgentState>>,
    cancel: CancellationToken,
    transformed_messages: Option<Vec<AgentMessage>>,
    llm_messages_override: Option<Vec<Message>>,
) -> Result<PreparedProviderRequest, String> {
    let mut s = state.write().unwrap();
    let messages_for_ctx = transformed_messages.as_ref().unwrap_or(&s.messages);
    let context = if let Some(llm_messages) = llm_messages_override {
        assemble_context(
            &s.config.system_prompt,
            messages_for_ctx,
            llm_messages,
            &s.tools,
            &s.config.resources,
        )
    } else if transformed_messages.is_some() {
        let llm_messages = default_convert_to_llm(messages_for_ctx, &s.config.resources);
        assemble_context(
            &s.config.system_prompt,
            messages_for_ctx,
            llm_messages,
            &s.tools,
            &s.config.resources,
        )
    } else {
        convert_to_context(
            &s.config.system_prompt,
            &s.messages,
            &s.tools,
            &s.config.resources,
        )
    };
    let mut stream_options = stream_options_for_turn(
        &s.config.model,
        s.config.stream_options.clone().unwrap_or_default(),
        s.config.thinking_level,
    );
    stream_options.cancel = Some(cancel.clone());

    let model = s.config.model.clone();
    let provider_request_override = s.provider_request_override.take();
    drop(s);

    let mut request = PreparedProviderRequest {
        model,
        context,
        stream_options,
    };
    if let Some(override_request) = provider_request_override {
        request.context = override_request.context;
        if let Some(override_options) = override_request.stream_options {
            request.stream_options = override_options;
        }
        request.stream_options.cancel = Some(cancel);
    }

    Ok(request)
}

pub(crate) fn stream_options_for_turn(
    model: &Model,
    mut options: StreamOptions,
    thinking_level: ThinkingLevel,
) -> StreamOptions {
    if !model.reasoning {
        options.thinking = None;
        return options;
    }

    match thinking_level {
        ThinkingLevel::Off => {
            options.thinking = None;
        }
        _ => {
            let budget_tokens = match thinking_level {
                ThinkingLevel::Minimal => Some(1024u32),
                ThinkingLevel::Low => Some(2048u32),
                ThinkingLevel::Medium => Some(4096u32),
                ThinkingLevel::High => Some(8192u32),
                ThinkingLevel::XHigh => Some(16384u32),
                ThinkingLevel::Off => None,
            };
            options.thinking = Some(ThinkingConfig {
                enabled: true,
                budget_tokens,
                effort: Some(thinking_level.to_string()),
            });
        }
    }

    options
}

#[cfg(test)]
mod tests {
    use crate::types::ThinkingLevel;
    use pi_ai::types::{Model, ModelCost, ModelInput, StreamOptions};

    fn model(reasoning: bool) -> Model {
        Model {
            id: "faux-model".into(),
            name: "Faux Model".into(),
            api: "faux".into(),
            provider: "faux".into(),
            base_url: String::new(),
            reasoning,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
            },
            context_window: 0,
            max_tokens: 0,
            headers: None,
            compat: None,
        }
    }

    #[test]
    fn thinking_options_are_applied_for_reasoning_models() {
        let options = super::stream_options_for_turn(
            &model(true),
            StreamOptions::default(),
            ThinkingLevel::High,
        );

        let thinking = options.thinking.expect("thinking should be enabled");
        assert!(thinking.enabled);
        assert_eq!(thinking.budget_tokens, Some(8192));
        assert_eq!(thinking.effort.as_deref(), Some("high"));
    }

    #[test]
    fn thinking_options_are_omitted_for_non_reasoning_models() {
        let options = super::stream_options_for_turn(
            &model(false),
            StreamOptions::default(),
            ThinkingLevel::High,
        );

        assert!(options.thinking.is_none());
    }
}
