mod commands;
mod event_queue;
pub(crate) mod events;
mod prompt;
mod state;
mod stats;
mod wire;

use crate::api::runtime::CliRunOptions;
use crate::app::cli::error::CliError;
use crate::protocol::jsonl::JsonlLineReader;
use crate::protocol::types::{RpcCommand, RpcResponse};
use event_queue::RpcQueuedProductEvent;
use serde_json::Value;
use state::{CodingOperationTaskResult, RpcState};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::oneshot;
pub use wire::write_rpc_response;
use wire::{command_id, command_type, is_supported_m5_command};

pub async fn run_rpc_mode_for_io<R, W>(
    reader: R,
    writer: &mut W,
    options: CliRunOptions,
) -> Result<(), CliError>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut state = RpcState::new(options)?;
    let mut lines = JsonlLineReader::new(reader);
    let result = run_rpc_loop(&mut state, &mut lines, writer).await;
    let _ = state.detach_client().await;
    result
}

async fn run_rpc_loop<R, W>(
    state: &mut RpcState,
    lines: &mut JsonlLineReader<R>,
    writer: &mut W,
) -> Result<(), CliError>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut input_closed = false;

    loop {
        if input_closed && !state.has_active_operations() {
            break;
        }

        let background_completion_rx = &mut state.background_completion_rx;
        let event = match (
            input_closed,
            state.foreground.as_mut(),
            state.session_events.as_mut(),
            state.session_events_closed,
        ) {
            (false, Some(foreground), Some(events), false) => {
                tokio::select! {
                    line = lines.read_next_line() => RpcLoopEvent::Input(line),
                    event = events.recv() => RpcLoopEvent::CodingEvent(event),
                    done = &mut foreground.done => RpcLoopEvent::CodingPromptDone(done),
                    completion = background_completion_rx.recv() => RpcLoopEvent::BackgroundOperationDone(completion),
                }
            }
            (false, Some(foreground), _, _) => {
                tokio::select! {
                    line = lines.read_next_line() => RpcLoopEvent::Input(line),
                    done = &mut foreground.done => RpcLoopEvent::CodingPromptDone(done),
                    completion = background_completion_rx.recv() => RpcLoopEvent::BackgroundOperationDone(completion),
                }
            }
            (true, Some(foreground), Some(events), false) => {
                tokio::select! {
                    event = events.recv() => RpcLoopEvent::CodingEvent(event),
                    done = &mut foreground.done => RpcLoopEvent::CodingPromptDone(done),
                    completion = background_completion_rx.recv() => RpcLoopEvent::BackgroundOperationDone(completion),
                }
            }
            (true, Some(foreground), _, _) => {
                tokio::select! {
                    done = &mut foreground.done => RpcLoopEvent::CodingPromptDone(done),
                    completion = background_completion_rx.recv() => RpcLoopEvent::BackgroundOperationDone(completion),
                }
            }
            (false, None, Some(events), false) => {
                tokio::select! {
                    line = lines.read_next_line() => RpcLoopEvent::Input(line),
                    event = events.recv() => RpcLoopEvent::CodingEvent(event),
                    completion = background_completion_rx.recv() => RpcLoopEvent::BackgroundOperationDone(completion),
                }
            }
            (false, None, _, _) => {
                tokio::select! {
                    line = lines.read_next_line() => RpcLoopEvent::Input(line),
                    completion = background_completion_rx.recv() => RpcLoopEvent::BackgroundOperationDone(completion),
                }
            }
            (true, None, Some(events), false) => {
                tokio::select! {
                    event = events.recv() => RpcLoopEvent::CodingEvent(event),
                    completion = background_completion_rx.recv() => RpcLoopEvent::BackgroundOperationDone(completion),
                }
            }
            (true, None, _, _) => {
                RpcLoopEvent::BackgroundOperationDone(background_completion_rx.recv().await)
            }
        };

        match event {
            RpcLoopEvent::Input(line) => {
                let Some(line) = line.map_err(|e| CliError::AgentFailure(e.to_string()))? else {
                    input_closed = true;
                    continue;
                };
                handle_input_line(state, &line, writer).await?;
            }
            RpcLoopEvent::CodingEvent(Some(RpcQueuedProductEvent::Overflow { skipped })) => {
                write_rpc_response(
                    writer,
                    RpcResponse::error_with_data(
                        None,
                        "event_stream",
                        format!(
                            "event stream lagged by {skipped} events; client must request a fresh UI snapshot"
                        ),
                        serde_json::json!({
                            "code": "event_stream_lag",
                            "skipped": skipped,
                            "recovery": "fresh_snapshot"
                        }),
                    ),
                )
                .await?;
                state.session_events_closed = true;
            }
            RpcLoopEvent::CodingEvent(Some(RpcQueuedProductEvent::Event(event))) => {
                state.write_product_event(event, writer).await?;
            }
            RpcLoopEvent::CodingEvent(None) => {
                state.session_events_closed = true;
            }
            RpcLoopEvent::CodingPromptDone(result) => {
                state.finish_coding_running_prompt(result, writer).await?;
            }
            RpcLoopEvent::BackgroundOperationDone(Some(completion)) => {
                state
                    .finish_background_operation(completion, writer)
                    .await?;
            }
            RpcLoopEvent::BackgroundOperationDone(None) => {
                return Err(CliError::AgentFailure(
                    "RPC background completion channel closed while operations were active".into(),
                ));
            }
        }
    }

    Ok(())
}

enum RpcLoopEvent {
    Input(Result<Option<String>, std::io::Error>),
    CodingEvent(Option<RpcQueuedProductEvent>),
    CodingPromptDone(Result<CodingOperationTaskResult, oneshot::error::RecvError>),
    BackgroundOperationDone(Option<state::RpcBackgroundCompletion>),
}

async fn handle_input_line<W>(
    state: &mut RpcState,
    line: &str,
    writer: &mut W,
) -> Result<(), CliError>
where
    W: AsyncWrite + Unpin,
{
    let value: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(error) => {
            write_rpc_response(
                writer,
                RpcResponse::error(None, "parse", format!("Failed to parse command: {error}")),
            )
            .await?;
            return Ok(());
        }
    };

    let command_name = command_type(&value);
    if !is_supported_m5_command(&command_name) {
        write_rpc_response(
            writer,
            RpcResponse::error(
                command_id(&value),
                command_name.clone(),
                format!("unsupported command in Rust M5: {command_name}"),
            ),
        )
        .await?;
        return Ok(());
    }

    let command: RpcCommand = match serde_json::from_value(value) {
        Ok(command) => command,
        Err(error) => {
            write_rpc_response(
                writer,
                RpcResponse::error(None, command_name, format!("Invalid command: {error}")),
            )
            .await?;
            return Ok(());
        }
    };

    state.handle_command(command, writer).await
}

pub async fn run_rpc_mode_stdio(options: CliRunOptions) -> Result<(), CliError> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    run_rpc_mode_for_io(stdin, &mut stdout, options).await
}
