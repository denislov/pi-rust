//! JSONL framing behavior shared by machine-readable protocols.

use pi_coding_agent::api::protocol::{read_jsonl_lines, serialize_json_line};
use serde_json::json;
use tokio::io::AsyncWriteExt;

#[test]
fn serialize_json_line_appends_exactly_one_lf() {
    let line = serialize_json_line(&json!({"type": "agent_start"})).unwrap();
    assert_eq!(line, "{\"type\":\"agent_start\"}\n");
}

#[tokio::test]
async fn jsonl_reader_splits_only_on_lf_and_strips_cr() {
    let input = b"{\"type\":\"a\"}\r\n{\"message\":\"line\\u2028inside\"}\n{\"type\":\"c\"}";
    let lines = read_jsonl_lines(&input[..]).await.unwrap();
    assert_eq!(
        lines,
        vec![
            "{\"type\":\"a\"}".to_string(),
            "{\"message\":\"line\\u2028inside\"}".to_string(),
            "{\"type\":\"c\"}".to_string(),
        ]
    );
}

#[tokio::test]
async fn jsonl_reader_handles_chunk_boundaries() {
    let (mut writer, reader) = tokio::io::duplex(8);
    let task = tokio::spawn(async move { read_jsonl_lines(reader).await.unwrap() });
    writer.write_all(b"{\"type\"").await.unwrap();
    writer
        .write_all(b":\"a\"}\n{\"type\":\"b\"}")
        .await
        .unwrap();
    drop(writer);
    let lines = task.await.unwrap();
    assert_eq!(
        lines,
        vec![
            "{\"type\":\"a\"}".to_string(),
            "{\"type\":\"b\"}".to_string()
        ]
    );
}
