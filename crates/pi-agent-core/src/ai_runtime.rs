use crate::types::ProviderStreamer;
use pi_ai::stream::EventStream;
use pi_ai::types::{Context, Model, StreamOptions};

pub(crate) fn stream_model_with_provider_streamer(
    model: &Model,
    context: Context,
    options: Option<StreamOptions>,
    provider_streamer: Option<ProviderStreamer>,
) -> EventStream {
    match provider_streamer {
        Some(provider_streamer) => provider_streamer(model, context, options),
        None => stream_model_with_global_runtime(model, context, options),
    }
}

#[allow(deprecated)]
pub(crate) fn stream_model_with_global_runtime(
    model: &Model,
    context: Context,
    options: Option<StreamOptions>,
) -> EventStream {
    pi_ai::stream_model(model, context, options)
}
