use crate::tools::path::resolve_to_cwd;
use pi_agent_core::{AgentTool, ToolFn};
use pi_ai::types::ContentBlock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const DESCRIPTION: &str = "Write content to a file. Creates the file if it doesn't exist, overwrites if it does. Automatically creates parent directories.";

fn schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "path": { "type": "string", "description": "Path to the file to write (relative or absolute)" },
            "content": { "type": "string", "description": "Content to write to the file" }
        },
        "required": ["path", "content"],
        "additionalProperties": false
    })
}

fn arg_str(args: &serde_json::Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("write: missing or non-string '{key}' argument"))
}

pub async fn write_execute(
    cwd: &Path,
    args: serde_json::Value,
) -> Result<Vec<ContentBlock>, String> {
    let path = arg_str(&args, "path")?;
    let content = arg_str(&args, "content")?;
    let abs = resolve_to_cwd(&path, cwd);
    if let Some(parent) = abs.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("write: failed to create {}: {e}", parent.display()))?;
    }
    tokio::fs::write(&abs, &content)
        .await
        .map_err(|e| format!("write: failed to write {}: {e}", abs.display()))?;
    let n = content.as_bytes().len();
    Ok(vec![ContentBlock::Text {
        text: format!("Successfully wrote {n} bytes to {path}"),
        text_signature: None,
    }])
}

pub fn write_tool(cwd: PathBuf) -> AgentTool {
    let execute: ToolFn = Arc::new(move |args| {
        let cwd = cwd.clone();
        Box::pin(async move { write_execute(&cwd, args).await })
    });
    AgentTool {
        name: "write".into(),
        description: DESCRIPTION.into(),
        parameters: schema(),
        execution_mode: None,
        execute,
    }
}
