use std::future::Future;
use std::pin::Pin;

use crate::convert::convert_to_context;
use crate::flow::{Action, FlowNode};
use crate::loop_runtime::context::stream_options_for_turn;
use crate::types::ProviderRequestSnapshot;

use super::context::AgentTurnContext;

pub struct PrepareContextNode;

impl FlowNode<AgentTurnContext> for PrepareContextNode {
    fn name(&self) -> &str {
        "prepare_context"
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut AgentTurnContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move {
            prepare_context(ctx)?;
            Action::new("default").map_err(|err| err.to_string())
        })
    }
}

pub fn prepare_context(ctx: &mut AgentTurnContext) -> Result<(), String> {
    let context = convert_to_context(
        &ctx.config.system_prompt,
        &ctx.messages,
        &ctx.tools,
        &ctx.resources,
    );
    let mut stream_options = stream_options_for_turn(
        &ctx.config.model,
        ctx.config.stream_options.clone().unwrap_or_default(),
        ctx.config.thinking_level,
    );
    stream_options.cancel = Some(ctx.cancel_token.clone());

    ctx.provider_request = Some(ProviderRequestSnapshot {
        model: ctx.config.model.clone(),
        context,
        stream_options,
    });
    Ok(())
}
