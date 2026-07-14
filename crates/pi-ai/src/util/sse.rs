use async_stream::stream;
use bytes::Bytes;
use futures::{Stream, StreamExt};

#[derive(Debug, Clone, PartialEq)]
pub struct ServerSentEvent {
    pub event: Option<String>,
    pub data: String,
}

/// Process raw SSE chunk and return any complete events found.
/// Maintains `buf` as leftover incomplete line for the next call.
pub fn process_chunk(chunk: &[u8], buf: &mut Vec<u8>) -> Vec<ServerSentEvent> {
    buf.extend_from_slice(chunk);
    let mut events = Vec::new();
    let mut event_type: Option<String> = None;
    let mut data_parts: Vec<String> = Vec::new();

    loop {
        let pos = buf.iter().position(|&b| b == b'\n');
        let line_end = match pos {
            Some(p) => p,
            None => break,
        };
        let mut line = buf.drain(..=line_end).collect::<Vec<u8>>();
        if line.ends_with(b"\r\n") {
            line.truncate(line.len() - 2);
        } else if line.ends_with(b"\n") {
            line.pop();
        }

        let line_str = String::from_utf8_lossy(&line);
        let trimmed = line_str.trim_end_matches('\r');

        if trimmed.is_empty() {
            if !data_parts.is_empty() {
                events.push(ServerSentEvent {
                    event: event_type.take(),
                    data: data_parts.join(""),
                });
                data_parts.clear();
            } else {
                event_type = None;
            }
        } else if let Some(rest) = trimmed.strip_prefix(':') {
            let _ = rest;
        } else if let Some(rest) = trimmed.strip_prefix("event:") {
            event_type = Some(rest.strip_prefix(' ').unwrap_or(rest).to_string());
        } else if let Some(rest) = trimmed.strip_prefix("data:") {
            data_parts.push(rest.strip_prefix(' ').unwrap_or(rest).to_string());
        }
    }
    events
}

/// Convert a Stream<Bytes> (from reqwest) into a Stream<ServerSentEvent>.
pub fn iterate_sse<E>(
    body: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
) -> impl Stream<Item = Result<ServerSentEvent, String>> + Send
where
    E: std::fmt::Display + Send + 'static,
{
    let mut buf: Vec<u8> = Vec::new();
    stream! {
        futures::pin_mut!(body);
        while let Some(chunk_result) = body.next().await {
            let chunk = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    yield Err(format!("SSE read error: {}", e));
                    return;
                }
            };
            for event in process_chunk(&chunk, &mut buf) {
                yield Ok(event);
            }
        }
        if !buf.is_empty() {
            for event in process_chunk(&[], &mut buf) {
                yield Ok(event);
            }
            if !buf.is_empty() {
                let data = String::from_utf8_lossy(&buf).into_owned();
                buf.clear();
                yield Ok(ServerSentEvent { event: None, data });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;

    #[test]
    fn basic_sse_event() {
        let input = b"data: {\"hello\":\"world\"}\n\n";
        let mut buf = Vec::new();
        let events = process_chunk(input, &mut buf);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, None);
        assert_eq!(events[0].data, "{\"hello\":\"world\"}");
    }

    #[test]
    fn event_with_type() {
        let input = b"event: message_start\ndata: {\"type\":\"x\"}\n\n";
        let mut buf = Vec::new();
        let events = process_chunk(input, &mut buf);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event.as_deref(), Some("message_start"));
        assert_eq!(events[0].data, "{\"type\":\"x\"}");
    }

    #[test]
    fn multi_line_data() {
        let input = b"data: line1\ndata: line2\n\n";
        let mut buf = Vec::new();
        let events = process_chunk(input, &mut buf);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "line1line2");
    }

    #[test]
    fn comment_lines_ignored() {
        let input = b": this is a comment\ndata: real\n\n";
        let mut buf = Vec::new();
        let events = process_chunk(input, &mut buf);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "real");
    }

    #[test]
    fn crlf_line_endings() {
        let input = b"data: foo\r\n\r\n";
        let mut buf = Vec::new();
        let events = process_chunk(input, &mut buf);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "foo");
    }

    #[test]
    fn event_split_across_chunks() {
        let mut buf = Vec::new();
        let events1 = process_chunk(b"data: hel", &mut buf);
        assert!(events1.is_empty());

        let events2 = process_chunk(b"lo world\n\n", &mut buf);
        assert_eq!(events2.len(), 1);
        assert_eq!(events2[0].data, "hello world");
    }

    #[tokio::test]
    async fn iterate_sse_from_stream() {
        let body = stream::iter(vec![
            Ok::<_, String>(Bytes::from("data: chunk1\n\n")),
            Ok(Bytes::from("data: chunk2\n\n")),
        ]);
        let results: Vec<_> = iterate_sse(body).collect().await;
        let events: Vec<_> = results.into_iter().map(|r| r.unwrap()).collect();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].data, "chunk1");
        assert_eq!(events[1].data, "chunk2");
    }
}
