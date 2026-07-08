mod commands;
pub(crate) mod events;
mod prompt;
mod state;
mod stats;
mod wire;

use crate::protocol::jsonl::JsonlLineReader;
use crate::protocol::types::{RpcCommand, RpcResponse};
use crate::{CliError, CliRunOptions};
use serde_json::Value;
use state::{CodingOperationTaskResult, RpcState, RunningPrompt};
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
    let mut input_closed = false;

    loop {
        if input_closed && !state.is_streaming() {
            break;
        }

        let event = match (input_closed, state.running.as_mut()) {
            (false, Some(RunningPrompt::Coding(running))) if !running.events_closed => {
                tokio::select! {
                    line = lines.read_next_line() => RpcLoopEvent::Input(line),
                    event = running.events.recv() => RpcLoopEvent::CodingEvent(event),
                    done = &mut running.done => RpcLoopEvent::CodingPromptDone(done),
                }
            }
            (false, Some(RunningPrompt::Coding(running))) => {
                tokio::select! {
                    line = lines.read_next_line() => RpcLoopEvent::Input(line),
                    done = &mut running.done => RpcLoopEvent::CodingPromptDone(done),
                }
            }
            (true, Some(RunningPrompt::Coding(running))) if !running.events_closed => {
                tokio::select! {
                    event = running.events.recv() => RpcLoopEvent::CodingEvent(event),
                    done = &mut running.done => RpcLoopEvent::CodingPromptDone(done),
                }
            }
            (true, Some(RunningPrompt::Coding(running))) => {
                RpcLoopEvent::CodingPromptDone((&mut running.done).await)
            }
            (false, None) => RpcLoopEvent::Input(lines.read_next_line().await),
            (true, None) => break,
        };

        match event {
            RpcLoopEvent::Input(line) => {
                let Some(line) = line.map_err(|e| CliError::AgentFailure(e.to_string()))? else {
                    input_closed = true;
                    continue;
                };
                handle_input_line(&mut state, &line, writer).await?;
            }
            RpcLoopEvent::CodingEvent(Some(event)) => {
                state.write_product_event(event, writer).await?;
            }
            RpcLoopEvent::CodingEvent(None) => {
                if let Some(RunningPrompt::Coding(running)) = state.running.as_mut() {
                    running.events_closed = true;
                }
            }
            RpcLoopEvent::CodingPromptDone(result) => {
                state.finish_coding_running_prompt(result, writer).await?;
            }
        }
    }

    Ok(())
}

enum RpcLoopEvent {
    Input(Result<Option<String>, std::io::Error>),
    CodingEvent(Option<crate::coding_session::ProductEvent>),
    CodingPromptDone(Result<CodingOperationTaskResult, oneshot::error::RecvError>),
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
