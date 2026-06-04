use crate::tools::truncate::{TruncationOptions, truncate_tail};
use pi_agent_core::{AgentTool, ToolFn};
use pi_ai::types::ContentBlock;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::AsyncReadExt;

const DESCRIPTION: &str = "Execute a bash command in the working directory. Returns merged stdout and stderr. Output is truncated to the last 2000 lines or 50KB (whichever is hit first). Optionally provide a timeout in seconds.";

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

pub async fn bash_execute(
    cwd: &Path,
    args: serde_json::Value,
) -> Result<Vec<ContentBlock>, String> {
    let command = args
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or("bash: missing or non-string 'command' argument")?
        .to_string();
    let timeout = args
        .get("timeout")
        .and_then(|v| v.as_u64())
        .filter(|secs| *secs > 0);
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

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "bash: failed to capture stdout".to_string())?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| "bash: failed to capture stderr".to_string())?;
    let mut buf: Vec<u8> = Vec::new();

    let collect = async {
        let mut merged = Vec::new();
        let mut stdout_open = true;
        let mut stderr_open = true;
        let mut stdout_buf = vec![0u8; 8192];
        let mut stderr_buf = vec![0u8; 8192];
        while stdout_open || stderr_open {
            tokio::select! {
                read = stdout.read(&mut stdout_buf), if stdout_open => {
                    match read {
                        Ok(0) => stdout_open = false,
                        Ok(n) => merged.extend_from_slice(&stdout_buf[..n]),
                        Err(_) => stdout_open = false,
                    }
                }
                read = stderr.read(&mut stderr_buf), if stderr_open => {
                    match read {
                        Ok(0) => stderr_open = false,
                        Ok(n) => merged.extend_from_slice(&stderr_buf[..n]),
                        Err(_) => stderr_open = false,
                    }
                }
            }
        }
        merged
    };

    let status_res = if let Some(secs) = timeout {
        match tokio::time::timeout(std::time::Duration::from_secs(secs), async {
            buf = collect.await;
            child.wait().await
        })
        .await
        {
            Ok(s) => s.map_err(|e| format!("bash: wait failed: {e}")).map(Some),
            Err(_) => {
                let _ = child.kill().await;
                Ok(None)
            }
        }
    } else {
        buf = collect.await;
        child
            .wait()
            .await
            .map_err(|e| format!("bash: wait failed: {e}"))
            .map(Some)
    };

    let merged = String::from_utf8_lossy(&buf).into_owned();
    let tr = truncate_tail(&merged, &TruncationOptions::default());
    let mut text = tr.content.clone();
    if tr.truncated {
        text = format!(
            "{text}\n\n[Output truncated: showing last {} of {} lines (50KB/2000-line limit).]",
            tr.output_lines, tr.total_lines
        );
    }

    match status_res? {
        None => Err(format!(
            "{}{}Command timed out after {} seconds",
            text,
            if text.is_empty() { "" } else { "\n\n" },
            timeout.unwrap()
        )),
        Some(status) => {
            let code = status.code();
            match code {
                Some(0) => {
                    let success_text = if text.is_empty() && !tr.truncated {
                        "(no output)".to_string()
                    } else {
                        text.clone()
                    };
                    Ok(vec![ContentBlock::Text {
                        text: success_text,
                        text_signature: None,
                    }])
                }
                Some(c) => {
                    let success_text = if text.is_empty() && !tr.truncated {
                        "(no output)".to_string()
                    } else {
                        text.clone()
                    };
                    Err(format!(
                        "{}{}Command exited with code {c}",
                        success_text,
                        if success_text.is_empty() { "" } else { "\n\n" }
                    ))
                }
                None => Err(format!(
                    "{}{}Command terminated by signal",
                    text,
                    if text.is_empty() { "" } else { "\n\n" }
                )),
            }
        }
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
