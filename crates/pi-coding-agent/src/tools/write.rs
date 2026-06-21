use crate::tools::file_mutation_queue::with_file_mutation_queue;
use crate::tools::path::resolve_to_cwd;
use futures::future::{BoxFuture, FutureExt};
use pi_agent_core::{AgentTool, AgentToolOutput, ToolFn};
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

pub trait WriteOperations: Send + Sync {
    fn write_file<'a>(
        &'a self,
        path: &'a Path,
        content: &'a [u8],
    ) -> BoxFuture<'a, Result<(), String>>;
}

#[derive(Debug, Default)]
pub struct RealWriteOperations;

impl WriteOperations for RealWriteOperations {
    fn write_file<'a>(
        &'a self,
        path: &'a Path,
        content: &'a [u8],
    ) -> BoxFuture<'a, Result<(), String>> {
        async move {
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| format!("write: failed to create {}: {e}", parent.display()))?;
            }
            tokio::fs::write(path, content)
                .await
                .map_err(|e| format!("write: failed to write {}: {e}", path.display()))
        }
        .boxed()
    }
}

pub async fn write_execute(
    cwd: &Path,
    args: serde_json::Value,
) -> Result<Vec<ContentBlock>, String> {
    write_execute_with_operations(cwd, args, Arc::new(RealWriteOperations)).await
}

pub async fn write_execute_with_operations(
    cwd: &Path,
    args: serde_json::Value,
    ops: Arc<dyn WriteOperations>,
) -> Result<Vec<ContentBlock>, String> {
    let path = arg_str(&args, "path")?;
    let content = arg_str(&args, "content")?;
    let abs = resolve_to_cwd(&path, cwd);
    let target = abs.clone();
    let ops = ops.clone();
    with_file_mutation_queue(&abs, move || async move {
        ops.write_file(&target, content.as_bytes()).await?;
        let n = content.as_bytes().len();
        Ok(vec![ContentBlock::Text {
            text: format!("Successfully wrote {n} bytes to {path}"),
            text_signature: None,
        }])
    })
    .await
}

pub fn write_tool(cwd: PathBuf) -> AgentTool {
    write_tool_with_operations(cwd, Arc::new(RealWriteOperations))
}

pub fn write_tool_with_operations(cwd: PathBuf, ops: Arc<dyn WriteOperations>) -> AgentTool {
    let execute: ToolFn = Arc::new(move |args, _on_update| {
        let cwd = cwd.clone();
        let ops = ops.clone();
        Box::pin(async move {
            write_execute_with_operations(&cwd, args, ops)
                .await
                .map(AgentToolOutput::new)
        })
    });
    AgentTool {
        name: "write".into(),
        description: DESCRIPTION.into(),
        parameters: schema(),
        execution_mode: None,
        execute,
    }
}
