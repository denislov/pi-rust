use pi_ai::providers::faux::FauxProvider;
use pi_ai::registry;
use pi_ai::types::{Model, ModelCost, ModelInput};
use pi_coding_agent::{CliRunOptions, protocol::rpc::run_rpc_mode_for_io};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn faux_model(api: &str) -> Model {
    Model {
        id: "faux-model".into(),
        name: "Faux Model".into(),
        api: api.into(),
        provider: "faux".into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost::default(),
        context_window: 8_000,
        max_tokens: 1_024,
        headers: None,
        compat: None,
    }
}

fn parse_lines(bytes: &[u8]) -> Vec<serde_json::Value> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[tokio::test]
async fn rpc_processes_command_before_stdin_eof() {
    let api = "pi-coding-rpc-streaming";
    registry::register(api, Arc::new(FauxProvider::simple_text("unused")));

    let (mut input_writer, input_reader) = tokio::io::duplex(128);
    let (output_writer, mut output_reader) = tokio::io::duplex(1024);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    });

    input_writer
        .write_all(b"{\"id\":\"s1\",\"type\":\"get_state\"}\n")
        .await
        .unwrap();

    let mut buf = vec![0; 1024];
    let bytes_read = tokio::time::timeout(Duration::from_millis(250), output_reader.read(&mut buf))
        .await
        .expect("rpc response before stdin EOF")
        .unwrap();

    let lines = parse_lines(&buf[..bytes_read]);
    assert_eq!(lines[0]["id"], "s1");
    assert_eq!(lines[0]["command"], "get_state");
    assert_eq!(lines[0]["success"], true);

    drop(input_writer);
    task.await.unwrap();
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_parse_error_keeps_process_alive_for_next_command() {
    let api = "pi-coding-rpc-parse";
    registry::register(api, Arc::new(FauxProvider::simple_text("unused")));

    let input = b"{bad json}\n{\"id\":\"s1\",\"type\":\"get_state\"}\n";
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["type"], "response");
    assert_eq!(lines[0]["command"], "parse");
    assert_eq!(lines[0]["success"], false);
    assert_eq!(lines[1]["id"], "s1");
    assert_eq!(lines[1]["command"], "get_state");
    assert_eq!(lines[1]["success"], true);
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_uses_settings_default_model_when_no_override_is_provided() {
    let _guard = ENV_LOCK.lock().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("settings.toml"),
        "default_model = \"claude-haiku-4-5\"\n",
    )
    .unwrap();
    unsafe {
        std::env::set_var("PI_RUST_DIR", dir.path().to_str().unwrap());
    }

    let input = b"{\"id\":\"s1\",\"type\":\"get_state\"}\n";
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["data"]["model"]["id"], "claude-haiku-4-5");

    unsafe {
        std::env::remove_var("PI_RUST_DIR");
    }
}

#[tokio::test]
async fn rpc_unsupported_command_returns_error_response() {
    let api = "pi-coding-rpc-unsupported";
    registry::register(api, Arc::new(FauxProvider::simple_text("unused")));

    let input = b"{\"id\":\"m1\",\"type\":\"set_model\",\"provider\":\"faux\",\"modelId\":\"x\"}\n";
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["id"], "m1");
    assert_eq!(lines[0]["command"], "set_model");
    assert_eq!(lines[0]["success"], false);
    assert_eq!(
        lines[0]["error"],
        "unsupported command in Rust M5: set_model"
    );
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_prompt_returns_response_then_agent_events() {
    let api = "pi-coding-rpc-prompt";
    registry::register(api, Arc::new(FauxProvider::simple_text("Hello")));

    let input = b"{\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\"}\n";
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["id"], "p1");
    assert_eq!(lines[0]["command"], "prompt");
    assert_eq!(lines[0]["success"], true);
    assert!(lines.iter().any(|line| line["type"] == "agent_start"));
    assert!(lines.iter().any(|line| line["type"] == "agent_end"));
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_state_commands_update_get_state() {
    let api = "pi-coding-rpc-state";
    registry::register(api, Arc::new(FauxProvider::simple_text("unused")));

    let input = b"{\"id\":\"t1\",\"type\":\"set_thinking_level\",\"level\":\"high\"}\n\
                  {\"id\":\"q1\",\"type\":\"set_steering_mode\",\"mode\":\"one-at-a-time\"}\n\
                  {\"id\":\"s1\",\"type\":\"get_state\"}\n";
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    let state = lines
        .iter()
        .find(|line| line["command"] == "get_state")
        .unwrap();
    assert_eq!(state["data"]["thinkingLevel"], "high");
    assert_eq!(state["data"]["steeringMode"], "one-at-a-time");
    registry::unregister(api);
}
