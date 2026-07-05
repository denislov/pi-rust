use pi_ai::stream::EventStream;
use pi_ai::types::{Context, Model, StreamOptions};

pub(crate) fn stream_model_with_global_runtime(
    model: &Model,
    context: Context,
    options: Option<StreamOptions>,
) -> EventStream {
    pi_ai::stream_model(model, context, options)
}
