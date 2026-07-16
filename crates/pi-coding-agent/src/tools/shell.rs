use crate::runtime::facade::ShellCapability;
use crate::tools::output::{DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES};
use futures::future::{BoxFuture, FutureExt};
use pi_agent_core::api::tool::{AgentTool, AgentToolOutput, ToolFn, ToolUpdateCallback};
use pi_ai::api::conversation::ContentBlock;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::AsyncReadExt;

const DESCRIPTION: &str = "Execute a bash command in the working directory. Returns merged stdout and stderr. Output is truncated to the last 2000 lines or 50KB (whichever is hit first). Commands time out after 120 seconds by default; timeout is capped at 600 seconds.";
const BUFFER_KEEP: usize = 65536;
const DRAIN_GRACE_MS: u64 = 500;
const DEFAULT_TIMEOUT_SECS: f64 = 120.0;
const MAX_TIMEOUT_SECS: f64 = 600.0;

#[derive(Clone)]
pub struct BashSpawnContext {
    pub command: String,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
}

pub type BashSpawnHook = Arc<dyn Fn(BashSpawnContext) -> BashSpawnContext + Send + Sync>;

#[derive(Clone, Default)]
pub struct BashOptions {
    pub shell_path: Option<String>,
    pub command_prefix: Option<String>,
    pub spawn_hook: Option<BashSpawnHook>,
}

pub trait BashOperations: Send + Sync {
    fn execute<'a>(
        &'a self,
        cwd: &'a Path,
        args: serde_json::Value,
        options: &'a BashOptions,
        on_update: Option<ToolUpdateCallback>,
    ) -> BoxFuture<'a, Result<Vec<ContentBlock>, String>>;
}

#[derive(Debug, Default)]
pub struct RealBashOperations;

impl BashOperations for RealBashOperations {
    fn execute<'a>(
        &'a self,
        cwd: &'a Path,
        args: serde_json::Value,
        options: &'a BashOptions,
        on_update: Option<ToolUpdateCallback>,
    ) -> BoxFuture<'a, Result<Vec<ContentBlock>, String>> {
        async move { bash_execute_real(cwd, args, options, on_update).await }.boxed()
    }
}

fn schema() -> serde_json::Value {
    serde_json::json!({
        "type":"object",
        "properties":{
            "command":{"type":"string","description":"Bash command to execute"},
            "timeout":{"type":"number","description":"Timeout in seconds (optional, default 120, max 600)"}
        },
        "required":["command"]
    })
}

async fn resolve_shell_path(custom_shell_path: Option<&str>) -> Result<String, String> {
    if let Some(shell_path) = custom_shell_path {
        if tokio::fs::try_exists(shell_path).await.unwrap_or(false) {
            return Ok(shell_path.to_string());
        }
        return Err(format!("Custom shell path not found: {shell_path}"));
    }

    // On Windows, look for Git Bash in known locations and bash.exe on PATH.
    // Mirrors TS `getShellConfig` in `pi/packages/coding-agent/src/utils/shell.ts`.
    #[cfg(windows)]
    {
        // Git Bash in standard install locations
        let candidates: &[&str] = &[
            "C:\\Program Files\\Git\\bin\\bash.exe",
            "C:\\Program Files (x86)\\Git\\bin\\bash.exe",
        ];
        for path in candidates {
            if tokio::fs::try_exists(path).await.unwrap_or(false) {
                return Ok(path.to_string());
            }
        }
        // Fallback: search bash.exe on PATH (Cygwin, MSYS2, WSL, etc.)
        if let Ok(output) = tokio::process::Command::new("where")
            .arg("bash.exe")
            .output()
            .await
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = stdout.lines().next() {
                let path = line.trim();
                if !path.is_empty() && tokio::fs::try_exists(path).await.unwrap_or(false) {
                    return Ok(path.to_string());
                }
            }
        }
        return Err(
            "No bash shell found. Options:\n  \
                1. Install Git for Windows (https://git-scm.com)\n  \
                2. Add your bash to PATH (Cygwin, MSYS2, etc.)\n  \
                Searched Git Bash in: C:\\Program Files\\Git\\bin\\bash.exe, C:\\Program Files (x86)\\Git\\bin\\bash.exe"
                .into(),
        );
    }

    #[cfg(not(windows))]
    if tokio::fs::try_exists("/bin/bash").await.unwrap_or(false) {
        Ok("/bin/bash".into())
    } else {
        Ok("bash".into())
    }
}

fn safe_process_env() -> HashMap<String, String> {
    std::env::vars()
        .filter(|(key, _)| is_safe_env_key(key))
        .collect()
}

fn is_safe_env_key(key: &str) -> bool {
    matches!(
        key,
        "PATH"
            | "HOME"
            | "USER"
            | "USERNAME"
            | "SHELL"
            | "TMPDIR"
            | "TEMP"
            | "TMP"
            | "LANG"
            | "LC_ALL"
            | "LC_CTYPE"
            | "TERM"
    ) || key.starts_with("LC_")
}

fn resolve_spawn_context(
    command: String,
    cwd: PathBuf,
    spawn_hook: Option<&BashSpawnHook>,
) -> BashSpawnContext {
    let context = BashSpawnContext {
        command,
        cwd,
        env: safe_process_env(),
    };
    match spawn_hook {
        Some(hook) => hook(context),
        None => context,
    }
}

fn timeout_secs_from_args(args: &serde_json::Value) -> Result<f64, String> {
    let raw = args
        .get("timeout")
        .and_then(|v| v.as_f64())
        .unwrap_or(DEFAULT_TIMEOUT_SECS);
    if !raw.is_finite() || raw <= 0.0 {
        return Err(format!(
            "bash: timeout must be a finite positive number of seconds (max {MAX_TIMEOUT_SECS})"
        ));
    }
    Ok(raw.min(MAX_TIMEOUT_SECS))
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

    fn buf_to_string(&self) -> String {
        String::from_utf8_lossy(&self.buf).into_owned()
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
    on_update: Option<&ToolUpdateCallback>,
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
                    Ok(n) => push_output(tail, &stdout_buf[..n], on_update),
                    Err(_) => stdout_open = false,
                }
            }
            read = stderr.read(&mut stderr_buf), if stderr_open => {
                match read {
                    Ok(0) => stderr_open = false,
                    Ok(n) => push_output(tail, &stderr_buf[..n], on_update),
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
    on_update: Option<&ToolUpdateCallback>,
) {
    let grace = std::time::Duration::from_millis(DRAIN_GRACE_MS);
    let _ = tokio::time::timeout(grace, drain_pipes(stdout, stderr, tail, on_update)).await;
}

fn push_output(tail: &mut OutputTail, data: &[u8], on_update: Option<&ToolUpdateCallback>) {
    tail.push(data);
    if let Some(on_update) = on_update {
        let text = apply_truncation(&tail.buf_to_string(), tail.total_lines, tail.overflowed);
        on_update(AgentToolOutput::new(vec![ContentBlock::Text {
            text,
            text_signature: None,
        }]));
    }
}

#[cfg(test)]
pub async fn bash_execute(
    cwd: &Path,
    args: serde_json::Value,
) -> Result<Vec<ContentBlock>, String> {
    bash_execute_with_options(cwd, args, &BashOptions::default()).await
}

#[cfg(test)]
pub async fn bash_execute_with_options(
    cwd: &Path,
    args: serde_json::Value,
    options: &BashOptions,
) -> Result<Vec<ContentBlock>, String> {
    bash_execute_with_options_and_update(cwd, args, options, None).await
}

#[cfg(test)]
pub async fn bash_execute_with_options_and_update(
    cwd: &Path,
    args: serde_json::Value,
    options: &BashOptions,
    on_update: Option<ToolUpdateCallback>,
) -> Result<Vec<ContentBlock>, String> {
    bash_execute_with_operations(cwd, args, options, on_update, Arc::new(RealBashOperations)).await
}

pub async fn bash_execute_with_operations(
    cwd: &Path,
    args: serde_json::Value,
    options: &BashOptions,
    on_update: Option<ToolUpdateCallback>,
    ops: Arc<dyn BashOperations>,
) -> Result<Vec<ContentBlock>, String> {
    ops.execute(cwd, args, options, on_update).await
}

async fn bash_execute_real(
    cwd: &Path,
    args: serde_json::Value,
    options: &BashOptions,
    on_update: Option<ToolUpdateCallback>,
) -> Result<Vec<ContentBlock>, String> {
    let command = args
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or("bash: missing or non-string 'command' argument")?
        .to_string();
    let timeout_secs = timeout_secs_from_args(&args)?;
    let workdir = cwd.to_path_buf();
    let resolved_command = match options.command_prefix.as_deref() {
        Some(prefix) if !prefix.is_empty() => format!("{prefix}\n{command}"),
        _ => command,
    };
    let spawn_context =
        resolve_spawn_context(resolved_command, workdir, options.spawn_hook.as_ref());
    if !tokio::fs::try_exists(&spawn_context.cwd)
        .await
        .unwrap_or(false)
    {
        return Err(format!(
            "Working directory does not exist: {}\nCannot execute bash commands.",
            spawn_context.cwd.display()
        ));
    }
    let shell_path = resolve_shell_path(options.shell_path.as_deref()).await?;

    let mut cmd = tokio::process::Command::new(shell_path);
    cmd.arg("-c")
        .arg(&spawn_context.command)
        .current_dir(&spawn_context.cwd)
        .env_clear()
        .envs(&spawn_context.env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    {
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let cmd_ref: &mut std::process::Command = cmd.as_std_mut();
        cmd_ref.creation_flags(CREATE_NO_WINDOW);
    }
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

    let mut tail = OutputTail::new();
    let mut timed_out = false;
    let mut exit_code: Option<i32> = None;
    let mut stdout_open = true;
    let mut stderr_open = true;
    let mut stdout_buf = vec![0u8; 8192];
    let mut stderr_buf = vec![0u8; 8192];
    let timeout = tokio::time::sleep(std::time::Duration::from_secs_f64(timeout_secs));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            status = child.wait() => {
                match status {
                    Ok(status) => exit_code = status.code(),
                    Err(e) => {
                        terminate_child_process_tree(&mut child).await;
                        drain_with_grace(stdout, stderr, &mut tail, on_update.as_ref()).await;
                        return Err(format!("bash: wait failed: {e}"));
                    }
                }
                break;
            }
            _ = &mut timeout => {
                terminate_child_process_tree(&mut child).await;
                timed_out = true;
                break;
            }
            read = stdout.read(&mut stdout_buf), if stdout_open => {
                match read {
                    Ok(0) => stdout_open = false,
                    Ok(n) => push_output(&mut tail, &stdout_buf[..n], on_update.as_ref()),
                    Err(_) => stdout_open = false,
                }
            }
            read = stderr.read(&mut stderr_buf), if stderr_open => {
                match read {
                    Ok(0) => stderr_open = false,
                    Ok(n) => push_output(&mut tail, &stderr_buf[..n], on_update.as_ref()),
                    Err(_) => stderr_open = false,
                }
            }
        }
    }

    drain_with_grace(stdout, stderr, &mut tail, on_update.as_ref()).await;
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

async fn terminate_child_process_tree(child: &mut tokio::process::Child) {
    #[cfg(unix)]
    if let Some(pid) = child.id() {
        if kill_process_group(pid).await {
            let _ = child.wait().await;
            return;
        }
    }

    let _ = child.kill().await;
}

#[cfg(unix)]
async fn kill_process_group(pid: u32) -> bool {
    let group = format!("-{pid}");
    if tokio::process::Command::new("kill")
        .arg("-KILL")
        .arg(&group)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .is_ok_and(|status| status.success())
    {
        return true;
    }

    tokio::process::Command::new("kill")
        .arg("-KILL")
        .arg(pid.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .is_ok_and(|status| status.success())
}

pub fn bash_tool(shell: ShellCapability) -> AgentTool {
    bash_tool_with_operations(shell, Arc::new(RealBashOperations))
}

pub fn bash_tool_with_operations(
    shell: ShellCapability,
    ops: Arc<dyn BashOperations>,
) -> AgentTool {
    let execute: ToolFn = Arc::new(move |context, args, on_update| {
        let shell = shell.clone();
        let ops = ops.clone();
        let cancel_token = context.cancel_token().clone();
        Box::pin(async move {
            let options = BashOptions::default();
            tokio::select! {
                _ = cancel_token.cancelled() => Err("tool execution cancelled".to_owned()),
                result = bash_execute_with_operations(
                    &shell.cwd,
                    args,
                    &options,
                    on_update,
                    ops,
                ) => result.map(AgentToolOutput::new),
            }
        })
    });
    AgentTool {
        name: "bash".into(),
        description: DESCRIPTION.into(),
        parameters: schema(),
        execution_mode: Some(pi_agent_core::api::tool::ToolExecutionMode::Sequential),
        execute,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pi_agent_core::api::tool::ToolExecutionContext;
    use tokio_util::sync::CancellationToken;

    struct PendingBashOperations;

    impl BashOperations for PendingBashOperations {
        fn execute<'a>(
            &'a self,
            _cwd: &'a Path,
            _args: serde_json::Value,
            _options: &'a BashOptions,
            _on_update: Option<ToolUpdateCallback>,
        ) -> BoxFuture<'a, Result<Vec<ContentBlock>, String>> {
            futures::future::pending().boxed()
        }
    }

    #[tokio::test]
    async fn bash_tool_honors_execution_context_cancellation() {
        let cancel_token = CancellationToken::new();
        let context = ToolExecutionContext::new(
            Some("op_cancelled"),
            0,
            "call_bash",
            "bash",
            cancel_token.clone(),
        );
        let tool = bash_tool_with_operations(
            ShellCapability::new(PathBuf::from(".")),
            Arc::new(PendingBashOperations),
        );
        cancel_token.cancel();

        let error = (tool.execute)(context, serde_json::json!({"command": "sleep 10"}), None)
            .await
            .unwrap_err();

        assert_eq!(error, "tool execution cancelled");
    }
}
