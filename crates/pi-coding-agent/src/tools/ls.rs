use crate::tools::path::resolve_to_cwd;
use crate::tools::truncate::{DEFAULT_MAX_BYTES, TruncationOptions, format_size, truncate_head};
use pi_agent_core::{AgentTool, AgentToolOutput, ToolFn};
use pi_ai::types::ContentBlock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const DESCRIPTION: &str = "List directory contents. Returns entries sorted alphabetically, with '/' suffix for directories. Includes dotfiles. Output is truncated to 500 entries or 50KB (whichever is hit first).";
const DEFAULT_LIMIT: usize = 500;

fn schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "path": { "type": "string", "description": "Directory to list (default: current directory)" },
            "limit": { "type": "number", "description": "Maximum number of entries to return (default: 500)" }
        }
    })
}

fn text_block(text: String) -> Vec<ContentBlock> {
    vec![ContentBlock::Text {
        text,
        text_signature: None,
    }]
}

fn limit_arg(args: &serde_json::Value, default: usize) -> usize {
    args.get("limit")
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_i64().and_then(|n| u64::try_from(n).ok()))
                .or_else(|| value.as_f64().map(|n| n.max(1.0) as u64))
        })
        .map(|n| n.max(1) as usize)
        .unwrap_or(default)
}

pub async fn ls_execute(cwd: &Path, args: serde_json::Value) -> Result<Vec<ContentBlock>, String> {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    let limit = limit_arg(&args, DEFAULT_LIMIT);
    let abs = resolve_to_cwd(path, cwd);

    let meta = tokio::fs::metadata(&abs)
        .await
        .map_err(|_| format!("ls: path not found: {}", abs.display()))?;
    if !meta.is_dir() {
        return Err(format!("ls: not a directory: {}", abs.display()));
    }

    let mut entries = Vec::new();
    let mut read_dir = tokio::fs::read_dir(&abs)
        .await
        .map_err(|e| format!("ls: cannot read directory {}: {e}", abs.display()))?;
    while let Some(entry) = read_dir
        .next_entry()
        .await
        .map_err(|e| format!("ls: cannot read directory {}: {e}", abs.display()))?
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        let file_type = match entry.file_type().await {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        entries.push(if file_type.is_dir() {
            format!("{name}/")
        } else {
            name
        });
    }

    entries.sort_by(|a, b| {
        a.to_lowercase()
            .cmp(&b.to_lowercase())
            .then_with(|| a.cmp(b))
    });

    if entries.is_empty() {
        return Ok(text_block("(empty directory)".to_string()));
    }

    let entry_limit_reached = entries.len() > limit;
    let output = entries
        .into_iter()
        .take(limit)
        .collect::<Vec<_>>()
        .join("\n");
    let truncation = truncate_head(
        &output,
        &TruncationOptions {
            max_lines: Some(usize::MAX),
            max_bytes: Some(DEFAULT_MAX_BYTES),
        },
    );
    let mut output = truncation.content;

    let mut notices = Vec::new();
    if entry_limit_reached {
        notices.push(format!(
            "{limit} entries limit reached. Use limit={} for more",
            limit * 2
        ));
    }
    if truncation.truncated {
        notices.push(format!("{} limit reached", format_size(DEFAULT_MAX_BYTES)));
    }
    if !notices.is_empty() {
        output.push_str(&format!("\n\n[{}]", notices.join(". ")));
    }

    Ok(text_block(output))
}

pub fn ls_tool(cwd: PathBuf) -> AgentTool {
    let execute: ToolFn = Arc::new(move |args, _on_update| {
        let cwd = cwd.clone();
        Box::pin(async move { ls_execute(&cwd, args).await.map(AgentToolOutput::new) })
    });
    AgentTool {
        name: "ls".into(),
        description: DESCRIPTION.into(),
        parameters: schema(),
        execution_mode: None,
        execute,
    }
}
