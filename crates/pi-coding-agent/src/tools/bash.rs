use pi_agent_core::{AgentTool, ToolFn};
use pi_ai::types::ContentBlock;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::AsyncReadExt;

use crate::tools::truncate::{DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES};

const DESCRIPTION: &str = "Execute a bash command in the working directory. Returns merged stdout and stderr. Output is truncated to the last 2000 lines or 50KB (whichever is hit first). Optionally provide a timeout in seconds.";
const BUFFER_KEEP: usize = 65536;
const DRAIN_GRACE_MS: u64 = 500;

fn schema() -> serde_json::Value {
    serde_json::json!({
        "type":"object",
        "properties":{
            "command":{"type":"string","description":"Bash command to execute"},
            "timeout":{"type":"number","description":"Timeout in seconds (optional)"}
        },
        "required":["command"]
    })
}

fn drain_byte(buf: &mut Vec<u8>, keep: usize) {
    let drop = buf.len().saturating_sub(keep);
    if drop == 0 {
        return;
    }
    let mut skip = drop;
    while skip < buf.len() && (buf[skip] & 0xC0) == 0x80 {
        skip += 1;
    }
    buf.drain(..skip);
}

struct OutputTail {
    buf: Vec<u8>,
    total_lines: usize,
    total_bytes: usize,
    overflowed: bool,
}

impl OutputTail {
    fn new() -> Self {
        Self {
            buf: Vec::new(),
            total_lines: 0,
            total_bytes: 0,
            overflowed: false,
        }
    }

    fn push(&mut self, data: &[u8]) {
        self.total_bytes += data.len();
        for &b in data {
            if b == b'\n' {
                self.total_lines += 1;
            }
        }
        self.buf.extend_from_slice(data);
        if self.buf.len() > BUFFER_KEEP * 2 {
            drain_byte(&mut self.buf, BUFFER_KEEP);
            self.overflowed = true;
        }
    }
}

fn apply_truncation(text: &str, known_lines: usize, overflowed: bool) -> String {
    let lines: Vec<&str> = if text.is_empty() {
        Vec::new()
    } else {
        let mut ls: Vec<&str> = text.split('\n').collect();
        if text.ends_with('\n') {
            ls.pop();
        }
        ls
    };
    let buf_len = lines.len();
    let buf_bytes = text.len();

    if buf_len <= DEFAULT_MAX_LINES && buf_bytes <= DEFAULT_MAX_BYTES && !overflowed {
        return text.to_string();
    }

    let mut out: Vec<String> = Vec::new();
    let mut bytes = 0usize;
    let mut byte_hit = false;
    for line in lines.iter().rev() {
        if out.len() >= DEFAULT_MAX_LINES {
            break;
        }
        let lb = line.len() + if !out.is_empty() { 1 } else { 0 };
        if bytes + lb > DEFAULT_MAX_BYTES {
            byte_hit = true;
            if out.is_empty() {
                let b = line.as_bytes();
                let keep = DEFAULT_MAX_BYTES.min(b.len());
                let mut start = b.len() - keep;
                while start < b.len() && (b[start] & 0xC0) == 0x80 {
                    start += 1;
                }
                out.insert(0, String::from_utf8_lossy(&b[start..]).into_owned());
            }
            break;
        }
        out.insert(0, (*line).to_string());
        bytes += lb;
    }
    let output = out.join("\n");
    let output_lines = out.len();
    let end = if overflowed || byte_hit || output_lines < buf_len {
        format!(
            "\n\n[Output truncated: showing last {output_lines} of {known_lines} lines (50KB/2000-line limit).]"
        )
    } else {
        String::new()
    };
    format!("{output}{end}")
}

async fn drain_pipes(
    mut stdout: tokio::process::ChildStdout,
    mut stderr: tokio::process::ChildStderr,
    tail: &mut OutputTail,
) {
    let mut stdout_open = true;
    let mut stderr_open = true;
    let mut stdout_buf = vec![0u8; 8192];
    let mut stderr_buf = vec![0u8; 8192];
    while stdout_open || stderr_open {
        tokio::select! {
            read = stdout.read(&mut stdout_buf), if stdout_open => {
                match read {
                    Ok(0) => stdout_open = false,
                    Ok(n) => tail.push(&stdout_buf[..n]),
                    Err(_) => stdout_open = false,
                }
            }
            read = stderr.read(&mut stderr_buf), if stderr_open => {
                match read {
                    Ok(0) => stderr_open = false,
                    Ok(n) => tail.push(&stderr_buf[..n]),
                    Err(_) => stderr_open = false,
                }
            }
        }
    }
}

async fn drain_with_grace(
    stdout: tokio::process::ChildStdout,
    stderr: tokio::process::ChildStderr,
    tail: &mut OutputTail,
) {
    let grace = std::time::Duration::from_millis(DRAIN_GRACE_MS);
    match tokio::time::timeout(grace, drain_pipes(stdout, stderr, tail)).await {
        Ok(()) => {}
        Err(_) => {}
    }
}

pub async fn bash_execute(
    cwd: &Path,
    args: serde_json::Value,
) -> Result<Vec<ContentBlock>, String> {
    let command = args
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or("bash: missing or non-string 'command' argument")?
        .to_string();
    let timeout_secs = args.get("timeout").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let workdir = cwd.to_path_buf();
    if !tokio::fs::try_exists(&workdir).await.unwrap_or(false) {
        return Err(format!(
            "Working directory does not exist: {}\nCannot execute bash commands.",
            workdir.display()
        ));
    }

    let mut cmd = tokio::process::Command::new("bash");
    cmd.arg("-c")
        .arg(&command)
        .current_dir(&workdir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("bash: failed to spawn: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "bash: failed to capture stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "bash: failed to capture stderr".to_string())?;

    let mut tail = OutputTail::new();

    let mut timed_out = false;
    let mut exit_code: Option<i32> = None;

    if timeout_secs > 0.0 && timeout_secs.is_finite() {
        let dur = std::time::Duration::from_secs_f64(timeout_secs);
        match tokio::time::timeout(dur, child.wait()).await {
            Ok(Ok(status)) => {
                exit_code = status.code();
            }
            Ok(Err(e)) => {
                let _ = child.kill().await;
                drain_with_grace(stdout, stderr, &mut tail).await;
                return Err(format!("bash: wait failed: {e}"));
            }
            Err(_) => {
                let _ = child.kill().await;
                timed_out = true;
            }
        }
    } else {
        match child.wait().await {
            Ok(status) => {
                exit_code = status.code();
            }
            Err(e) => {
                let _ = child.kill().await;
                drain_with_grace(stdout, stderr, &mut tail).await;
                return Err(format!("bash: wait failed: {e}"));
            }
        }
    }

    drain_with_grace(stdout, stderr, &mut tail).await;
    let text = apply_truncation(&tail.buf_to_string(), tail.total_lines, tail.overflowed);

    if timed_out {
        return Err(format!(
            "{}{}Command timed out after {timeout_secs} seconds",
            text,
            if text.is_empty() { "" } else { "\n\n" }
        ));
    }

    let success_text = if text.is_empty() && !tail.overflowed {
        "(no output)".to_string()
    } else {
        text.clone()
    };

    match exit_code {
        Some(0) => Ok(vec![ContentBlock::Text {
            text: success_text,
            text_signature: None,
        }]),
        Some(c) => Err(format!(
            "{}{}Command exited with code {c}",
            success_text,
            if success_text.is_empty() { "" } else { "\n\n" }
        )),
        None => Err(format!(
            "{}{}Command terminated by signal",
            text,
            if text.is_empty() { "" } else { "\n\n" }
        )),
    }
}

impl OutputTail {
    fn buf_to_string(&self) -> String {
        String::from_utf8_lossy(&self.buf).into_owned()
    }
}

pub fn bash_tool(cwd: PathBuf) -> AgentTool {
    let execute: ToolFn = Arc::new(move |args| {
        let cwd = cwd.clone();
        Box::pin(async move { bash_execute(&cwd, args).await })
    });
    AgentTool {
        name: "bash".into(),
        description: DESCRIPTION.into(),
        parameters: schema(),
        execute,
    }
}
