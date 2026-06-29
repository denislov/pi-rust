use pi_ai::providers::faux::FauxProvider;
use pi_ai::registry;
use pi_ai::stream::EventStream;
use pi_ai::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Model, ModelCost, ModelInput,
    StopReason, StreamOptions,
};
use pi_coding_agent::{CliRunOptions, protocol::rpc::run_rpc_mode_for_io};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::io::AsyncBufReadExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Notify;

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

struct PausingProvider {
    release: Arc<Notify>,
    opened: Arc<AtomicBool>,
}

impl pi_ai::registry::ApiProvider for PausingProvider {
    fn stream(&self, model: &Model, _ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
        let release = Arc::clone(&self.release);
        let opened = Arc::clone(&self.opened);
        let model_id = model.id.clone();
        Box::pin(async_stream::stream! {
            let mut partial = AssistantMessage::empty("pausing", &model_id);
            partial.provider = Some("pausing".into());
            yield AssistantMessageEvent::Start { content_index: None, partial: partial.clone() };

            partial.content.push(ContentBlock::Text {
                text: String::new(),
                text_signature: None,
            });
            yield AssistantMessageEvent::TextStart { content_index: 0, partial: partial.clone() };

            if let Some(ContentBlock::Text { text, .. }) = partial.content.last_mut() {
                text.push_str("partial");
            }
            yield AssistantMessageEvent::TextDelta {
                content_index: 0,
                delta: "partial".to_string(),
                partial: partial.clone(),
            };

            if !opened.load(Ordering::SeqCst) {
                release.notified().await;
                opened.store(true, Ordering::SeqCst);
            }

            yield AssistantMessageEvent::TextEnd { content_index: 0, partial: partial.clone() };
            partial.stop_reason = StopReason::Stop;
            yield AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message: partial,
            };
        })
    }
}

struct AbortAwareProvider {
    cancelled: Arc<AtomicBool>,
    release: Arc<Notify>,
}

impl pi_ai::registry::ApiProvider for AbortAwareProvider {
    fn stream(&self, model: &Model, _ctx: Context, opts: Option<StreamOptions>) -> EventStream {
        let cancelled = Arc::clone(&self.cancelled);
        let release = Arc::clone(&self.release);
        let cancel = opts.and_then(|opts| opts.cancel);
        let model_id = model.id.clone();
        Box::pin(async_stream::stream! {
            let mut partial = AssistantMessage::empty("abort-aware", &model_id);
            partial.provider = Some("abort-aware".into());
            yield AssistantMessageEvent::Start { content_index: None, partial: partial.clone() };

            if let Some(cancel) = cancel {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        cancelled.store(true, Ordering::SeqCst);
                        partial.stop_reason = StopReason::Aborted;
                    }
                    _ = release.notified() => {
                        partial.stop_reason = StopReason::Stop;
                    }
                }
            } else {
                release.notified().await;
                partial.stop_reason = StopReason::Stop;
            }

            let reason = partial.stop_reason.clone();
            yield AssistantMessageEvent::Done {
                reason,
                message: partial,
            };
        })
    }
}

#[tokio::test]
async fn rpc_processes_command_before_stdin_eof() {
    let api = "pi-coding-rpc-streaming";
    registry::register(api, Arc::new(FauxProvider::simple_text("unused")));

    let (mut input_writer, input_reader) = tokio::io::duplex(128);
    let (output_writer, mut output_reader) = tokio::io::duplex(4096);
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

    let mut buf = vec![0; 4096];
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
async fn rpc_state_reports_capabilities_when_idle() {
    let api = "pi-coding-rpc-capabilities-idle";
    registry::register(api, Arc::new(FauxProvider::simple_text("unused")));

    let input = b"{\"id\":\"s1\",\"type\":\"get_state\"}\n";
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
    let capabilities = &lines[0]["data"]["capabilities"];
    assert_eq!(capabilities["prompt"]["status"], "available");
    assert_eq!(capabilities["abort"]["status"], "disabled");
    assert_eq!(capabilities["followUp"]["status"], "available");
    assert_eq!(capabilities["compact"]["status"], "unsupported");
    assert_eq!(capabilities["tools"]["status"], "available");
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_state_reports_prompt_busy_while_running() {
    let api = "pi-coding-rpc-capabilities-busy";
    let release = Arc::new(Notify::new());
    registry::register(
        api,
        Arc::new(PausingProvider {
            release: Arc::clone(&release),
            opened: Arc::new(AtomicBool::new(false)),
        }),
    );

    let (mut input_writer, input_reader) = tokio::io::duplex(512);
    let (output_writer, output_reader) = tokio::io::duplex(4096);
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
        .write_all(b"{\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\"}\n")
        .await
        .unwrap();

    let mut lines = tokio::io::BufReader::new(output_reader).lines();
    let prompt_response = tokio::time::timeout(Duration::from_millis(250), lines.next_line())
        .await
        .expect("prompt response before provider completes")
        .unwrap()
        .unwrap();
    let prompt_response: serde_json::Value = serde_json::from_str(&prompt_response).unwrap();
    assert_eq!(prompt_response["success"], true);

    input_writer
        .write_all(b"{\"id\":\"s1\",\"type\":\"get_state\"}\n")
        .await
        .unwrap();

    let state = tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before get_state response");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "response" && value["command"] == "get_state" {
                break value;
            }
        }
    })
    .await
    .expect("state response while prompt is running");

    release.notify_one();
    drop(input_writer);
    task.await.unwrap();

    assert_eq!(state["data"]["isStreaming"], true);
    let capabilities = &state["data"]["capabilities"];
    assert_eq!(capabilities["prompt"]["status"], "busy");
    assert_eq!(capabilities["prompt"]["operation"], "prompt");
    assert_eq!(capabilities["abort"]["status"], "available");
    assert_eq!(capabilities["steer"]["status"], "available");
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
async fn rpc_streams_agent_events_before_prompt_finishes() {
    let api = "pi-coding-rpc-live-events";
    let release = Arc::new(Notify::new());
    registry::register(
        api,
        Arc::new(PausingProvider {
            release: Arc::clone(&release),
            opened: Arc::new(AtomicBool::new(false)),
        }),
    );

    let (mut input_writer, input_reader) = tokio::io::duplex(256);
    let (output_writer, output_reader) = tokio::io::duplex(4096);
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
        .write_all(b"{\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\"}\n")
        .await
        .unwrap();

    let mut lines = tokio::io::BufReader::new(output_reader).lines();
    let response_line = tokio::time::timeout(Duration::from_millis(250), lines.next_line())
        .await
        .expect("prompt response before provider completes")
        .unwrap()
        .unwrap();
    let response: serde_json::Value = serde_json::from_str(&response_line).unwrap();
    assert_eq!(response["id"], "p1");
    assert_eq!(response["command"], "prompt");
    assert_eq!(response["success"], true);

    let event_line = tokio::time::timeout(Duration::from_millis(250), lines.next_line()).await;
    release.notify_one();
    drop(input_writer);
    task.await.unwrap();

    let event_line = event_line
        .expect("agent event before prompt finishes")
        .unwrap()
        .unwrap();
    let event: serde_json::Value = serde_json::from_str(&event_line).unwrap();
    assert_eq!(event["type"], "agent_start");
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_abort_cancels_running_prompt() {
    let api = "pi-coding-rpc-abort";
    let cancelled = Arc::new(AtomicBool::new(false));
    let release = Arc::new(Notify::new());
    registry::register(
        api,
        Arc::new(AbortAwareProvider {
            cancelled: Arc::clone(&cancelled),
            release: Arc::clone(&release),
        }),
    );

    let (mut input_writer, input_reader) = tokio::io::duplex(256);
    let (output_writer, output_reader) = tokio::io::duplex(4096);
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
        .write_all(b"{\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\"}\n")
        .await
        .unwrap();

    let mut lines = tokio::io::BufReader::new(output_reader).lines();
    let prompt_response = tokio::time::timeout(Duration::from_millis(250), lines.next_line())
        .await
        .expect("prompt response before provider completes")
        .unwrap()
        .unwrap();
    let prompt_response: serde_json::Value = serde_json::from_str(&prompt_response).unwrap();
    assert_eq!(prompt_response["success"], true);

    input_writer
        .write_all(b"{\"id\":\"a1\",\"type\":\"abort\"}\n")
        .await
        .unwrap();

    let abort_response = tokio::time::timeout(Duration::from_millis(250), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before abort response");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "response" && value["command"] == "abort" {
                break value;
            }
        }
    })
    .await;

    release.notify_one();
    drop(input_writer);
    task.await.unwrap();

    let abort_response = abort_response.expect("abort response while prompt is running");
    assert_eq!(abort_response["id"], "a1");
    assert_eq!(abort_response["success"], true);
    assert!(cancelled.load(Ordering::SeqCst));
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_steer_while_running_updates_queue() {
    let api = "pi-coding-rpc-steer-live";
    let release = Arc::new(Notify::new());
    registry::register(
        api,
        Arc::new(PausingProvider {
            release: Arc::clone(&release),
            opened: Arc::new(AtomicBool::new(false)),
        }),
    );

    let (mut input_writer, input_reader) = tokio::io::duplex(512);
    let (output_writer, output_reader) = tokio::io::duplex(4096);
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
        .write_all(b"{\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\"}\n")
        .await
        .unwrap();
    let mut lines = tokio::io::BufReader::new(output_reader).lines();
    let _ = lines.next_line().await.unwrap().unwrap();
    let _ = lines.next_line().await.unwrap().unwrap();

    input_writer
        .write_all(b"{\"id\":\"s1\",\"type\":\"steer\",\"message\":\"look here\"}\n")
        .await
        .unwrap();

    let queue_update = tokio::time::timeout(Duration::from_millis(250), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before queue update");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "queue_update" {
                break value;
            }
        }
    })
    .await;

    let queue_update = queue_update.expect("queue update while prompt is running");
    assert_eq!(queue_update["steering"], serde_json::json!(["look here"]));
    release.notify_one();
    drop(input_writer);
    tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before agent_end");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "agent_end" {
                break;
            }
        }
    })
    .await
    .expect("agent_end after releasing paused provider");
    task.await.unwrap();
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_follow_up_prompt_while_running_updates_queue() {
    let api = "pi-coding-rpc-follow-up-live";
    let release = Arc::new(Notify::new());
    registry::register(
        api,
        Arc::new(PausingProvider {
            release: Arc::clone(&release),
            opened: Arc::new(AtomicBool::new(false)),
        }),
    );

    let (mut input_writer, input_reader) = tokio::io::duplex(512);
    let (output_writer, output_reader) = tokio::io::duplex(4096);
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
        .write_all(b"{\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\"}\n")
        .await
        .unwrap();
    let mut lines = tokio::io::BufReader::new(output_reader).lines();
    let _ = lines.next_line().await.unwrap().unwrap();
    let _ = lines.next_line().await.unwrap().unwrap();

    input_writer
        .write_all(
            b"{\"id\":\"f1\",\"type\":\"prompt\",\"message\":\"next\",\"streamingBehavior\":\"followUp\"}\n",
        )
        .await
        .unwrap();

    let queue_update = tokio::time::timeout(Duration::from_millis(250), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before queue update");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "queue_update" {
                break value;
            }
        }
    })
    .await;

    let queue_update = queue_update.expect("follow-up queue update while prompt is running");
    assert_eq!(queue_update["followUp"], serde_json::json!(["next"]));
    release.notify_one();
    drop(input_writer);
    tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before agent_end");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "agent_end" {
                break;
            }
        }
    })
    .await
    .expect("agent_end after releasing paused provider");
    task.await.unwrap();
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_plain_prompt_while_running_returns_error() {
    let api = "pi-coding-rpc-running-prompt-error";
    let release = Arc::new(Notify::new());
    registry::register(
        api,
        Arc::new(PausingProvider {
            release: Arc::clone(&release),
            opened: Arc::new(AtomicBool::new(false)),
        }),
    );

    let (mut input_writer, input_reader) = tokio::io::duplex(512);
    let (output_writer, output_reader) = tokio::io::duplex(4096);
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
        .write_all(b"{\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\"}\n")
        .await
        .unwrap();
    let mut lines = tokio::io::BufReader::new(output_reader).lines();
    let _ = lines.next_line().await.unwrap().unwrap();
    let _ = lines.next_line().await.unwrap().unwrap();

    input_writer
        .write_all(b"{\"id\":\"p2\",\"type\":\"prompt\",\"message\":\"second\"}\n")
        .await
        .unwrap();

    let response = tokio::time::timeout(Duration::from_millis(250), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before second prompt response");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "response" && value["id"] == "p2" {
                break value;
            }
        }
    })
    .await;

    release.notify_one();
    drop(input_writer);
    task.await.unwrap();

    let response = response.expect("plain prompt rejection while prompt is running");
    assert_eq!(response["success"], false);
    assert_eq!(
        response["error"],
        "agent is streaming; set streamingBehavior to steer or followUp"
    );
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
