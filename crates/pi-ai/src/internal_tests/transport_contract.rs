use pi_ai::api::conversation::{AssistantMessage, StopReason};
use pi_ai::api::hooks::{ProviderResponseInfo, ProviderStreamHooks};
use pi_ai::api::model::{Model, ModelCost, ModelInput};
use pi_ai::api::stream::{AssistantMessageEvent, EventStream, StreamOptions, complete};
use pi_ai::transport::headers::merge_headers;
use pi_ai::transport::http::send_json_stream;
use pi_ai::transport::retry::RetryConfig;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::mpsc;

fn test_model() -> Model {
    Model {
        id: "test-model".into(),
        name: "Test Model".into(),
        api: "openai-responses".into(),
        provider: "openai".into(),
        base_url: "http://127.0.0.1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost::default(),
        context_window: 128_000,
        max_tokens: 4096,
        headers: None,
        compat: None,
    }
}

fn done_stream(api: &'static str) -> EventStream {
    Box::pin(async_stream::stream! {
        yield AssistantMessageEvent::Done {
            reason: StopReason::Stop,
            message: AssistantMessage::empty(api, "test-model"),
        };
    })
}

fn start_http_server(responses: Vec<String>) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        for response in responses {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_http_request(&mut stream);
            tx.send(request).unwrap();
            stream.write_all(response.as_bytes()).unwrap();
            stream.flush().unwrap();
        }
    });

    (format!("http://{}", addr), rx)
}

fn read_http_request(stream: &mut std::net::TcpStream) -> String {
    let mut data = Vec::new();
    let mut buf = [0_u8; 1024];
    loop {
        let n = stream.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
        if data.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }

    let header_end = data
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|pos| pos + 4)
        .unwrap_or(data.len());
    let headers = String::from_utf8_lossy(&data[..header_end]).to_string();
    let content_length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);

    let already_read = data.len().saturating_sub(header_end);
    let remaining = content_length.saturating_sub(already_read);
    if remaining > 0 {
        let mut body = vec![0_u8; remaining];
        stream.read_exact(&mut body).unwrap();
        data.extend_from_slice(&body);
    }

    String::from_utf8_lossy(&data).to_string()
}

fn response(status: u16, headers: &[(&str, &str)], body: &str) -> String {
    let reason = match status {
        200 => "OK",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        _ => "Status",
    };
    let mut out = format!(
        "HTTP/1.1 {} {}\r\ncontent-length: {}\r\n",
        status,
        reason,
        body.len()
    );
    for (name, value) in headers {
        out.push_str(name);
        out.push_str(": ");
        out.push_str(value);
        out.push_str("\r\n");
    }
    out.push_str("\r\n");
    out.push_str(body);
    out
}

#[test]
fn headers_merge_priority() {
    let model = serde_json::json!({"x-custom": "model-value"});
    let opts = serde_json::json!({"x-custom": "option-value"});
    let headers = merge_headers(Some(&model), Some(&opts), []);
    assert_eq!(
        headers.get("x-custom").map(String::as_str),
        Some("option-value")
    );
}

#[test]
fn headers_generated_preserved() {
    let opts = serde_json::json!({"x-extra": "extra"});
    let headers = merge_headers(
        None,
        Some(&opts),
        [("authorization".into(), "Bearer sk-test".into())],
    );
    assert_eq!(
        headers.get("authorization").map(String::as_str),
        Some("Bearer sk-test")
    );
    assert_eq!(headers.get("x-extra").map(String::as_str), Some("extra"));
}

#[test]
fn retry_config_defaults() {
    let cfg = RetryConfig::from_options(None);
    assert_eq!(cfg.max_retries, 0);
    assert_eq!(cfg.max_retry_delay_ms, 10_000);
    assert_eq!(cfg.timeout_ms, None);
}

#[test]
fn retry_config_from_options() {
    let opts = StreamOptions {
        max_retries: Some(1),
        max_retry_delay_ms: Some(5_000),
        timeout_ms: Some(30_000),
        ..Default::default()
    };
    let cfg = RetryConfig::from_options(Some(&opts));
    assert_eq!(cfg.max_retries, 1);
    assert_eq!(cfg.max_retry_delay_ms, 5_000);
    assert_eq!(cfg.timeout_ms, Some(30_000));
}

#[tokio::test]
async fn response_hook_receives_actual_headers() {
    let (url, _requests) = start_http_server(vec![response(
        200,
        &[
            ("content-type", "text/event-stream"),
            ("x-request-id", "req-123"),
        ],
        "",
    )]);
    let client = reqwest::Client::new();
    let model = test_model();
    let (tx, rx) = mpsc::channel::<ProviderResponseInfo>();
    let hooks = ProviderStreamHooks {
        on_payload: None,
        on_response: Some(Arc::new(move |info| {
            let tx = tx.clone();
            Box::pin(async move {
                tx.send(info).unwrap();
                Ok(())
            })
        })),
    };
    let opts = StreamOptions {
        hooks: Some(hooks),
        ..Default::default()
    };

    let stream = send_json_stream(
        &client,
        &model,
        Some(&opts),
        "openai-responses",
        client.post(&url),
        serde_json::json!({"message": "hello"}),
        |_body, _model, _cancel| done_stream("openai-responses"),
    );

    complete(stream).await.unwrap();
    let info = rx.try_recv().unwrap();
    assert_eq!(info.status, Some(200));
    assert_eq!(
        info.headers
            .as_ref()
            .and_then(|h| h.get("x-request-id"))
            .and_then(|v| v.as_str()),
        Some("req-123")
    );
}

#[tokio::test]
async fn retry_after_above_cap_errors_without_retrying() {
    let (url, requests) = start_http_server(vec![
        response(429, &[("retry-after", "5")], "slow down"),
        response(200, &[("content-type", "text/event-stream")], ""),
    ]);
    let client = reqwest::Client::new();
    let model = test_model();
    let opts = StreamOptions {
        max_retries: Some(1),
        max_retry_delay_ms: Some(1_000),
        ..Default::default()
    };

    let stream = send_json_stream(
        &client,
        &model,
        Some(&opts),
        "openai-responses",
        client.post(&url),
        serde_json::json!({"message": "hello"}),
        |_body, _model, _cancel| done_stream("openai-responses"),
    );

    let err = complete(stream).await.unwrap_err();
    assert!(err.contains("Retry-After 5000ms exceeds max_retry_delay_ms 1000"));
    requests.try_recv().unwrap();
    assert!(requests.try_recv().is_err());
}

#[tokio::test]
async fn explicit_retry_retries_retryable_status_once() {
    let (url, requests) = start_http_server(vec![
        response(500, &[], "temporary failure"),
        response(200, &[("content-type", "text/event-stream")], ""),
    ]);
    let client = reqwest::Client::new();
    let model = test_model();
    let opts = StreamOptions {
        max_retries: Some(1),
        ..Default::default()
    };

    let stream = send_json_stream(
        &client,
        &model,
        Some(&opts),
        "openai-responses",
        client.post(&url),
        serde_json::json!({"message": "hello"}),
        |_body, _model, _cancel| done_stream("openai-responses"),
    );

    complete(stream).await.unwrap();
    requests.try_recv().unwrap();
    requests.try_recv().unwrap();
}

#[tokio::test]
async fn payload_hook_mutates_actual_request_body() {
    let (url, requests) = start_http_server(vec![response(
        200,
        &[("content-type", "text/event-stream")],
        "",
    )]);
    let client = reqwest::Client::new();
    let model = test_model();
    let hooks = ProviderStreamHooks {
        on_payload: Some(Arc::new(|_model, mut payload| {
            Box::pin(async move {
                payload["message"] = serde_json::json!("hooked");
                Ok(payload)
            })
        })),
        on_response: None,
    };
    let opts = StreamOptions {
        hooks: Some(hooks),
        ..Default::default()
    };

    let stream = send_json_stream(
        &client,
        &model,
        Some(&opts),
        "openai-responses",
        client.post(&url),
        serde_json::json!({"message": "original"}),
        |_body, _model, _cancel| done_stream("openai-responses"),
    );

    complete(stream).await.unwrap();
    let request = requests.try_recv().unwrap();
    assert!(request.contains(r#""message":"hooked""#), "{request}");
    assert!(!request.contains(r#""message":"original""#), "{request}");
}
// Internal transport-contract tests.
