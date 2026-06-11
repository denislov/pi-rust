use serde::Serialize;
use tokio::io::{AsyncRead, AsyncReadExt};

pub fn serialize_json_line<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let mut line = serde_json::to_string(value)?;
    line.push('\n');
    Ok(line)
}

pub async fn read_jsonl_lines<R>(mut reader: R) -> std::io::Result<Vec<String>>
where
    R: AsyncRead + Unpin,
{
    let mut lines = Vec::new();
    let mut reader = JsonlLineReader::new(&mut reader);
    while let Some(line) = reader.read_next_line().await? {
        lines.push(line);
    }
    Ok(lines)
}

pub struct JsonlLineReader<R> {
    reader: R,
    pending: Vec<u8>,
    reached_eof: bool,
}

impl<R> JsonlLineReader<R>
where
    R: AsyncRead + Unpin,
{
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            pending: Vec::new(),
            reached_eof: false,
        }
    }

    pub async fn read_next_line(&mut self) -> std::io::Result<Option<String>> {
        loop {
            if let Some(line_end) = self.pending.iter().position(|byte| *byte == b'\n') {
                let mut bytes: Vec<u8> = self.pending.drain(..=line_end).collect();
                bytes.pop();
                return Ok(Some(line_from_bytes(bytes)));
            }

            if self.reached_eof {
                if self.pending.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(line_from_bytes(std::mem::take(&mut self.pending))));
            }

            let mut chunk = [0; 8192];
            let read = self.reader.read(&mut chunk).await?;
            if read == 0 {
                self.reached_eof = true;
            } else {
                self.pending.extend_from_slice(&chunk[..read]);
            }
        }
    }
}

fn line_from_bytes(mut bytes: Vec<u8>) -> String {
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    String::from_utf8_lossy(&bytes).to_string()
}
