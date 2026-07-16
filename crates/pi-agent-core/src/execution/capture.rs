use crate::execution::truncate::{DEFAULT_MAX_BYTES, TruncationLimit, truncate_tail};
use crate::execution::{ExecOptions, ExecutionEnv};
use crate::execution::{ExecutionError, ExecutionErrorCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellCaptureOptions {
    pub max_lines: usize,
    pub max_bytes: usize,
}

impl Default for ShellCaptureOptions {
    fn default() -> Self {
        Self {
            max_lines: crate::execution::truncate::DEFAULT_MAX_LINES,
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellCaptureResult {
    pub output: String,
    pub exit_code: Option<i32>,
    pub cancelled: bool,
    pub truncated: bool,
    pub full_output_path: Option<String>,
}

pub fn sanitize_binary_output(text: &str) -> String {
    text.chars()
        .filter(|ch| {
            let code = *ch as u32;
            code == 0x09
                || code == 0x0a
                || code == 0x0d
                || (code > 0x1f && !(0xfff9..=0xfffb).contains(&code))
        })
        .collect()
}

pub async fn execute_shell_with_capture<E: ExecutionEnv>(
    env: &E,
    command: &str,
    options: ShellCaptureOptions,
) -> Result<ShellCaptureResult, ExecutionError> {
    let output = match env.exec(command, Some(ExecOptions::default())).await {
        Ok(output) => output,
        Err(error) if error.code() == ExecutionErrorCode::Aborted => {
            return Ok(ShellCaptureResult {
                output: String::new(),
                exit_code: None,
                cancelled: true,
                truncated: false,
                full_output_path: None,
            });
        }
        Err(error) => return Err(error),
    };

    let full_output = sanitize_binary_output(&(output.stdout + &output.stderr)).replace('\r', "");
    let truncation = truncate_tail(
        &full_output,
        TruncationLimit {
            max_lines: options.max_lines,
            max_bytes: options.max_bytes,
        },
    );

    let full_output_path = if truncation.truncated {
        let path = env
            .create_temp_file("bash-", ".log")
            .await
            .map_err(file_error_to_execution_error)?;
        env.write_file(path.to_string_lossy().as_ref(), full_output.as_bytes())
            .await
            .map_err(file_error_to_execution_error)?;
        Some(path.to_string_lossy().to_string())
    } else {
        None
    };

    Ok(ShellCaptureResult {
        output: if truncation.truncated {
            truncation.content
        } else {
            full_output
        },
        exit_code: Some(output.exit_code),
        cancelled: false,
        truncated: truncation.truncated,
        full_output_path,
    })
}

fn file_error_to_execution_error(error: crate::execution::FileError) -> ExecutionError {
    ExecutionError::CallbackError {
        message: error.to_string(),
    }
}

pub fn bash_execution_to_text(
    command: &str,
    output: &str,
    exit_code: Option<i32>,
    cancelled: bool,
    truncated: bool,
    full_output_path: Option<&str>,
) -> String {
    let mut text = format!("Ran `{}`\n", command);
    if output.is_empty() {
        text.push_str("(no output)");
    } else {
        text.push_str("```\n");
        text.push_str(output);
        text.push_str("\n```");
    }
    if cancelled {
        text.push_str("\n\n(command cancelled)");
    } else if let Some(code) = exit_code
        && code != 0
    {
        text.push_str(&format!("\n\nCommand exited with code {}", code));
    }
    if truncated && let Some(path) = full_output_path {
        text.push_str(&format!("\n\n[Output truncated. Full output: {}]", path));
    }
    text
}
