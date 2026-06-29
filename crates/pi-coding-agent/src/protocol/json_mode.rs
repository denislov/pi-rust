use crate::coding_session::{AgentEventMappingContext, CodingAgentEvent, map_agent_event};
use crate::protocol::events::CodingProtocolEventAdapter;
use crate::protocol::jsonl::serialize_json_line;
use crate::protocol::session_runner::{SessionPromptOptions, run_session_prompt};
use crate::protocol::types::ProtocolEvent;
use crate::{CliError, CliOutput};
use pi_agent_core::session::{SessionHeader, create_session_id, create_timestamp};

pub async fn run_json_mode(options: SessionPromptOptions) -> CliOutput {
    let header = SessionHeader {
        entry_type: "session".into(),
        version: 3,
        id: create_session_id(),
        timestamp: create_timestamp(),
        cwd: std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .display()
            .to_string(),
        parent_session: None,
    };

    let mut stdout = match serialize_json_line(&header) {
        Ok(line) => line,
        Err(error) => {
            return CliOutput {
                exit_code: 1,
                stdout: String::new(),
                stderr: format!("agent failure: {error}\n"),
            };
        }
    };

    match serialize_json_line(&ProtocolEvent::AgentStart) {
        Ok(line) => stdout.push_str(&line),
        Err(error) => {
            return CliOutput {
                exit_code: 1,
                stdout,
                stderr: format!("agent failure: {error}\n"),
            };
        }
    }

    let operation_id = "json_prompt".to_string();
    let turn_id = "json_turn".to_string();
    let mapping_context = AgentEventMappingContext::new(operation_id.clone(), turn_id.clone());
    let mut adapter = CodingProtocolEventAdapter::new_with_provider(
        options.model.api.clone(),
        options.model.provider.clone(),
        options.model.id.clone(),
    );
    let run = run_session_prompt(
        options,
        Some(&mut |event| {
            for coding_event in map_agent_event(&mapping_context, event) {
                push_coding_protocol_events(&mut stdout, &mut adapter, &coding_event)?;
            }
            Ok(())
        }),
    )
    .await;

    match run {
        Ok(_) => {
            let completed = CodingAgentEvent::PromptCompleted {
                operation_id,
                turn_id,
            };
            if let Err(error) = push_coding_protocol_events(&mut stdout, &mut adapter, &completed) {
                return CliOutput {
                    exit_code: 1,
                    stdout,
                    stderr: format!("{error}\n"),
                };
            }
            CliOutput {
                exit_code: 0,
                stdout,
                stderr: String::new(),
            }
        }
        Err(error) => CliOutput {
            exit_code: 1,
            stdout,
            stderr: format!("{error}\n"),
        },
    }
}

fn push_coding_protocol_events(
    stdout: &mut String,
    adapter: &mut CodingProtocolEventAdapter,
    event: &CodingAgentEvent,
) -> Result<(), CliError> {
    for protocol_event in adapter.push(event) {
        stdout.push_str(
            &serialize_json_line(&protocol_event)
                .map_err(|error| CliError::AgentFailure(error.to_string()))?,
        );
    }
    Ok(())
}
