use crate::protocol::events::ProtocolEventAdapter;
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

    let mut adapter = ProtocolEventAdapter::new_with_provider(
        options.model.api.clone(),
        options.model.provider.clone(),
        options.model.id.clone(),
    );
    let run = run_session_prompt(
        options,
        Some(&mut |event| {
            for protocol_event in adapter.push(event) {
                stdout.push_str(
                    &serialize_json_line(&protocol_event)
                        .map_err(|error| CliError::AgentFailure(error.to_string()))?,
                );
            }
            Ok(())
        }),
    )
    .await;

    match run {
        Ok(_) => CliOutput {
            exit_code: 0,
            stdout,
            stderr: String::new(),
        },
        Err(error) => CliOutput {
            exit_code: 1,
            stdout,
            stderr: format!("{error}\n"),
        },
    }
}
