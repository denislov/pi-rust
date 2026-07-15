use crate::coding_session::FilesystemCapability;
use crate::tools::path::resolve_to_cwd;
use crate::tools::truncate::{
    DEFAULT_MAX_BYTES, TruncatedBy, TruncationOptions, format_size, truncate_head,
};
use futures::future::{BoxFuture, FutureExt};
use pi_agent_core::api::{AgentTool, AgentToolOutput, ToolFn};
use pi_ai::api::ContentBlock;
use std::path::Path;
use std::sync::Arc;

const DESCRIPTION: &str = "Read the contents of a text file. Output is truncated to 2000 lines or 50KB (whichever is hit first). Use offset/limit for large files; continue with offset until complete. Image files are not read in this mode.";
const MAX_READ_FILE_BYTES: u64 = 5 * 1024 * 1024;

fn image_mime(path: &Path) -> Option<&'static str> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())?
        .to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

fn schema() -> serde_json::Value {
    serde_json::json!({
        "type":"object",
        "properties":{
            "path":{"type":"string","description":"Path to the file to read (relative or absolute)"},
            "offset":{"type":"number","description":"Line number to start reading from (1-indexed)"},
            "limit":{"type":"number","description":"Maximum number of lines to read"}
        },
        "required":["path"]
    })
}

fn text_block(t: String) -> Vec<ContentBlock> {
    vec![ContentBlock::Text {
        text: t,
        text_signature: None,
    }]
}

pub trait ReadOperations: Send + Sync {
    fn read_file<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<Vec<u8>, String>>;
}

#[derive(Debug, Default)]
pub struct RealReadOperations;

impl ReadOperations for RealReadOperations {
    fn read_file<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<Vec<u8>, String>> {
        async move {
            let metadata = tokio::fs::metadata(path)
                .await
                .map_err(|e| format!("read: cannot stat {}: {e}", path.display()))?;
            if metadata.len() > MAX_READ_FILE_BYTES {
                return Err(format!(
                    "read: refusing to read {} because it is {} and exceeds the {} safety limit; use a shell pager or a narrower tool instead",
                    path.display(),
                    format_size(metadata.len() as usize),
                    format_size(MAX_READ_FILE_BYTES as usize),
                ));
            }
            tokio::fs::read(path)
                .await
                .map_err(|e| format!("read: cannot read {}: {e}", path.display()))
        }
        .boxed()
    }
}

#[cfg(test)]
pub async fn read_execute(
    cwd: &Path,
    args: serde_json::Value,
) -> Result<Vec<ContentBlock>, String> {
    read_execute_with_operations(cwd, args, Arc::new(RealReadOperations)).await
}

pub async fn read_execute_with_operations(
    cwd: &Path,
    args: serde_json::Value,
    ops: Arc<dyn ReadOperations>,
) -> Result<Vec<ContentBlock>, String> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("read: missing or non-string 'path' argument")?
        .to_string();
    let offset = args
        .get("offset")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize);
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize);
    let abs = resolve_to_cwd(&path, cwd);

    let raw = ops.read_file(&abs).await?;
    if let Some(mime) = image_mime(&abs) {
        return Ok(text_block(format!(
            "Read image file [{mime}]\n[Image content is not supported in headless mode yet; omitted.]"
        )));
    }

    let content = String::from_utf8_lossy(&raw).into_owned();
    let all: Vec<&str> = content.split('\n').collect();
    let total = all.len();

    let start = offset.unwrap_or(1).saturating_sub(1);
    let start_display = start + 1;
    if start >= all.len() {
        return Err(format!(
            "Offset {} is beyond end of file ({} lines total)",
            offset.unwrap_or(1),
            total
        ));
    }

    let (selected, user_limited): (String, Option<usize>) = match limit {
        Some(l) => {
            let end = (start + l).min(all.len());
            (all[start..end].join("\n"), Some(end - start))
        }
        None => (all[start..].join("\n"), None),
    };

    let tr = truncate_head(&selected, &TruncationOptions::default());
    let out = if tr.first_line_exceeds_limit {
        let first_line_bytes = all[start].len();
        format!(
            "[Line {start_display} is {}, exceeds {} limit. Use bash: sed -n '{start_display}p' {path} | head -c {DEFAULT_MAX_BYTES}]",
            format_size(first_line_bytes),
            format_size(DEFAULT_MAX_BYTES)
        )
    } else if tr.truncated {
        let end_display = start_display + tr.output_lines - 1;
        let next = end_display + 1;
        if tr.truncated_by == TruncatedBy::Lines {
            format!(
                "{}\n\n[Showing lines {start_display}-{end_display} of {total}. Use offset={next} to continue.]",
                tr.content
            )
        } else {
            format!(
                "{}\n\n[Showing lines {start_display}-{end_display} of {total} ({} limit). Use offset={next} to continue.]",
                tr.content,
                format_size(DEFAULT_MAX_BYTES)
            )
        }
    } else if let Some(ul) = user_limited {
        if start + ul < all.len() {
            let remaining = all.len() - (start + ul);
            let next = start + ul + 1;
            format!(
                "{}\n\n[{remaining} more lines in file. Use offset={next} to continue.]",
                tr.content
            )
        } else {
            tr.content
        }
    } else {
        tr.content
    };

    Ok(text_block(out))
}

pub fn read_tool(filesystem: FilesystemCapability) -> AgentTool {
    read_tool_with_operations(filesystem, Arc::new(RealReadOperations))
}

pub fn read_tool_with_operations(
    filesystem: FilesystemCapability,
    ops: Arc<dyn ReadOperations>,
) -> AgentTool {
    let execute: ToolFn = Arc::new(move |args, _on_update| {
        let filesystem = filesystem.clone();
        let ops = ops.clone();
        Box::pin(async move {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            filesystem.resolve_path(path).map_err(|e| e.to_string())?;
            read_execute_with_operations(&filesystem.cwd, args, ops)
                .await
                .map(AgentToolOutput::new)
        })
    });
    AgentTool {
        name: "read".into(),
        description: DESCRIPTION.into(),
        parameters: schema(),
        execution_mode: None,
        execute,
    }
}
