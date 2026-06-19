use pi_ai::providers::bedrock;
use pi_ai::registry::{self, ApiProvider};
use pi_ai::types::{
    AssistantMessageEvent, ContentBlock, Context, Message, Model, ModelCost, ModelInput,
    StopReason, StreamOptions, Tool,
};

fn test_model() -> Model {
    Model {
        id: "anthropic.claude-3-5-sonnet-20240620-v1:0".into(),
        name: "Claude 3.5 Sonnet".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 200000,
        max_tokens: 8192,
        headers: None,
        compat: None,
    }
}

fn event_frame(payload: serde_json::Value) -> bytes::Bytes {
    let payload = serde_json::to_vec(&payload).unwrap();
    let total_len = (16 + payload.len()) as u32;
    let mut frame = Vec::with_capacity(total_len as usize);
    frame.extend_from_slice(&total_len.to_be_bytes());
    frame.extend_from_slice(&0u32.to_be_bytes());
    frame.extend_from_slice(&0u32.to_be_bytes());
    frame.extend_from_slice(&payload);
    frame.extend_from_slice(&0u32.to_be_bytes());
    bytes::Bytes::from(frame)
}

#[test]
fn bedrock_request_maps_messages_tools_cache_and_inference() {
    let model = test_model();
    let ctx = Context {
        system_prompt: Some("Be concise.".into()),
        messages: vec![
            Message::User {
                content: vec![
                    ContentBlock::Text {
                        text: "look".into(),
                        text_signature: None,
                    },
                    ContentBlock::Image {
                        data: "abc".into(),
                        mime_type: "image/png".into(),
                    },
                ],
            },
            Message::Assistant {
                content: vec![ContentBlock::ToolCall {
                    id: "call:one".into(),
                    name: "read".into(),
                    arguments: serde_json::json!({"path": "Cargo.toml"}),
                    thought_signature: None,
                }],
            },
            Message::ToolResult {
                tool_call_id: "call:one".into(),
                tool_name: Some("read".into()),
                is_error: Some(false),
                content: vec![ContentBlock::Text {
                    text: "contents".into(),
                    text_signature: None,
                }],
            },
        ],
        tools: Some(vec![Tool {
            name: "read".into(),
            description: Some("Read a file".into()),
            parameters: serde_json::json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        }]),
    };
    let opts = StreamOptions {
        max_tokens: Some(2048),
        temperature: Some(0.25),
        cache_retention: Some(serde_json::json!("long")),
        tool_choice: Some(serde_json::json!({"type": "tool", "name": "read"})),
        ..Default::default()
    };

    let req = bedrock::convert::build_request(&model, &ctx, &Some(opts));
    let json = serde_json::to_value(&req).unwrap();

    assert_eq!(json["modelId"], "anthropic.claude-3-5-sonnet-20240620-v1:0");
    assert_eq!(json["system"][0]["text"], "Be concise.");
    assert_eq!(json["system"][1]["cachePoint"]["ttl"], "ONE_HOUR");
    assert_eq!(json["messages"][0]["role"], "user");
    assert_eq!(json["messages"][0]["content"][1]["image"]["format"], "png");
    assert_eq!(
        json["messages"][1]["content"][0]["toolUse"]["toolUseId"],
        "call_one"
    );
    assert_eq!(
        json["messages"][2]["content"][0]["toolResult"]["content"][0]["text"],
        "contents"
    );
    assert_eq!(
        json["messages"][2]["content"][1]["cachePoint"]["ttl"],
        "ONE_HOUR"
    );
    assert_eq!(json["inferenceConfig"]["maxTokens"], 2048);
    assert_eq!(json["inferenceConfig"]["temperature"], 0.25);
    assert_eq!(json["toolConfig"]["tools"][0]["toolSpec"]["name"], "read");
    assert_eq!(json["toolConfig"]["toolChoice"]["tool"]["name"], "read");
}

#[tokio::test]
async fn bedrock_event_stream_maps_text_thinking_tool_usage_and_done() {
    let frames = vec![
        event_frame(serde_json::json!({"messageStart": {"role": "assistant"}})),
        event_frame(serde_json::json!({
            "contentBlockDelta": {"contentBlockIndex": 0, "delta": {"text": "Hello "}}
        })),
        event_frame(serde_json::json!({
            "contentBlockDelta": {"contentBlockIndex": 0, "delta": {"text": "world"}}
        })),
        event_frame(serde_json::json!({"contentBlockStop": {"contentBlockIndex": 0}})),
        event_frame(serde_json::json!({
            "contentBlockDelta": {
                "contentBlockIndex": 1,
                "delta": {"reasoningContent": {"text": "think", "signature": "sig"}}
            }
        })),
        event_frame(serde_json::json!({"contentBlockStop": {"contentBlockIndex": 1}})),
        event_frame(serde_json::json!({
            "contentBlockStart": {
                "contentBlockIndex": 2,
                "start": {"toolUse": {"toolUseId": "tool-1", "name": "read"}}
            }
        })),
        event_frame(serde_json::json!({
            "contentBlockDelta": {
                "contentBlockIndex": 2,
                "delta": {"toolUse": {"input": "{\"path\":\"Cargo.toml\"}"}}
            }
        })),
        event_frame(serde_json::json!({"contentBlockStop": {"contentBlockIndex": 2}})),
        event_frame(serde_json::json!({"messageStop": {"stopReason": "tool_use"}})),
        event_frame(serde_json::json!({
            "metadata": {
                "usage": {
                    "inputTokens": 10,
                    "outputTokens": 4,
                    "totalTokens": 14,
                    "cacheReadInputTokens": 3,
                    "cacheWriteInputTokens": 1
                }
            }
        })),
    ];
    let body = futures::stream::iter(frames.into_iter().map(Ok::<_, String>));

    let stream = bedrock::process::process(body, test_model(), None);
    use futures::StreamExt;
    let events: Vec<_> = stream.collect().await;

    assert!(matches!(events[0], AssistantMessageEvent::Start { .. }));
    assert!(events.iter().any(
        |event| matches!(event, AssistantMessageEvent::TextDelta { delta, .. } if delta == "Hello ")
    ));
    assert!(events
        .iter()
        .any(|event| matches!(event, AssistantMessageEvent::ThinkingDelta { delta, .. } if delta == "think")));
    assert!(events
        .iter()
        .any(|event| matches!(event, AssistantMessageEvent::ToolcallDelta { delta, .. } if delta == "{\"path\":\"Cargo.toml\"}")));

    let AssistantMessageEvent::Done { reason, message } = events.last().unwrap() else {
        panic!("expected final done event");
    };
    assert_eq!(*reason, StopReason::ToolUse);
    assert_eq!(message.stop_reason, StopReason::ToolUse);
    assert_eq!(message.usage.input, 10);
    assert_eq!(message.usage.output, 4);
    assert_eq!(message.usage.cache_read, 3);
    assert_eq!(message.usage.cache_write, 1);
    assert_eq!(message.usage.total_tokens, 14);
    assert!(message.usage.cost.input > 0.0);
    assert_eq!(
        message.content[0],
        ContentBlock::Text {
            text: "Hello world".into(),
            text_signature: None,
        }
    );
    assert_eq!(
        message.content[1],
        ContentBlock::Thinking {
            thinking: "think".into(),
            thinking_signature: Some("sig".into()),
            redacted: None,
        }
    );
    assert_eq!(
        message.content[2],
        ContentBlock::ToolCall {
            id: "tool-1".into(),
            name: "read".into(),
            arguments: serde_json::json!({"path": "Cargo.toml"}),
            thought_signature: None,
        }
    );
}

#[test]
fn sigv4_signing_matches_fixed_vector() {
    let signed = bedrock::sigv4::sign(
        bedrock::sigv4::SignRequest {
            method: "POST",
            uri: "/model/anthropic.claude-3-5-sonnet-20240620-v1:0/converse-stream",
            query: "",
            host: "bedrock-runtime.us-east-1.amazonaws.com",
            region: "us-east-1",
            service: "bedrock",
            access_key: "AKIDEXAMPLE",
            secret_key: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
            session_token: None,
            amz_date: "20260102T030405Z",
            date: "20260102",
            body: br#"{"modelId":"anthropic.claude-3-5-sonnet-20240620-v1:0"}"#,
        },
        &[],
    );

    assert_eq!(
        signed.payload_hash,
        "49908fb78d40c41665ef8970abf39d352e4251a3257e7370206295faffd582d5"
    );
    assert_eq!(
        signed.signature,
        "fb6532cd9d390370a046145801fb06b493a79eccc7966a691bd961e27a717919"
    );
    assert!(
        signed
            .authorization
            .contains("Credential=AKIDEXAMPLE/20260102/us-east-1/bedrock/aws4_request")
    );
    assert!(
        signed
            .authorization
            .contains("SignedHeaders=host;x-amz-content-sha256;x-amz-date")
    );
}

#[test]
fn bedrock_auth_headers_prefers_bearer_token() {
    let headers = bedrock::auth_headers(
        "https://bedrock-runtime.us-east-1.amazonaws.com/model/x/converse-stream",
        "us-east-1",
        br#"{}"#,
        &Some(StreamOptions {
            bedrock_bearer_token: Some("bedrock-token".into()),
            ..Default::default()
        }),
    )
    .unwrap();

    assert_eq!(
        headers.get("authorization").map(String::as_str),
        Some("Bearer bedrock-token")
    );
}

#[tokio::test]
async fn bedrock_provider_missing_credentials_returns_error_event() {
    let provider = bedrock::BedrockProvider::new(None);
    let model = test_model();
    let ctx = Context {
        system_prompt: None,
        messages: vec![],
        tools: None,
    };

    unsafe {
        std::env::remove_var("AWS_ACCESS_KEY_ID");
        std::env::remove_var("AWS_SECRET_ACCESS_KEY");
        std::env::remove_var("AWS_BEARER_TOKEN_BEDROCK");
    }

    let event_stream = provider.stream(&model, ctx, None);
    use futures::StreamExt;
    let events: Vec<_> = event_stream.collect().await;
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], AssistantMessageEvent::Error { .. }));
}

#[test]
fn builtins_register_bedrock_api() {
    registry::unregister("bedrock-converse-stream");
    pi_ai::providers::register_builtins();
    assert!(registry::lookup("bedrock-converse-stream").is_some());
}
