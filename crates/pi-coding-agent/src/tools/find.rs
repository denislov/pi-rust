use crate::tools::path::resolve_to_cwd;
use crate::tools::truncate::{DEFAULT_MAX_BYTES, TruncationOptions, format_size, truncate_head};
use globset::{GlobBuilder, GlobMatcher};
use ignore::{DirEntry, WalkBuilder};
use pi_agent_core::{AgentTool, AgentToolOutput, ToolFn};
use pi_ai::types::ContentBlock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const DESCRIPTION: &str = "Search for files by glob pattern. Returns matching file paths relative to the search directory. Respects .gitignore. Output is truncated to 1000 results or 50KB (whichever is hit first).";
const DEFAULT_LIMIT: usize = 1000;

fn schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "pattern": { "type": "string", "description": "Glob pattern to match files, e.g. '*.rs', '**/*.json', or 'src/**/*.spec.ts'" },
            "path": { "type": "string", "description": "Directory to search in (default: current directory)" },
            "limit": { "type": "number", "description": "Maximum number of results to return (default: 1000)" }
        },
        "required": ["pattern"]
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

fn compile_glob(pattern: &str) -> Result<GlobMatcher, String> {
    GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map(|glob| glob.compile_matcher())
        .map_err(|e| format!("find: invalid glob: {e}"))
}

fn is_skipped_dir(entry: &DirEntry) -> bool {
    entry
        .file_type()
        .map(|file_type| file_type.is_dir())
        .unwrap_or(false)
        && entry
            .file_name()
            .to_str()
            .map(|name| name == ".git" || name == "node_modules")
            .unwrap_or(false)
}

fn include_entry(entry: &DirEntry) -> bool {
    !is_skipped_dir(entry)
}

fn relative_posix(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    if relative.as_os_str().is_empty() {
        return None;
    }
    Some(
        relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

fn basename(path: &Path) -> Option<String> {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

fn sort_paths(paths: &mut [String]) {
    paths.sort_by(|a, b| {
        a.to_lowercase()
            .cmp(&b.to_lowercase())
            .then_with(|| a.cmp(b))
    });
}

pub async fn find_execute(
    cwd: &Path,
    args: serde_json::Value,
) -> Result<Vec<ContentBlock>, String> {
    let pattern = args
        .get("pattern")
        .and_then(|v| v.as_str())
        .ok_or("find: missing or non-string 'pattern' argument")?;
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    let limit = limit_arg(&args, DEFAULT_LIMIT);
    let abs = resolve_to_cwd(path, cwd);

    let metadata = tokio::fs::metadata(&abs)
        .await
        .map_err(|_| format!("find: path not found: {}", abs.display()))?;
    if !metadata.is_dir() {
        return Err(format!("find: not a directory: {}", abs.display()));
    }

    let matcher = compile_glob(pattern)?;
    let match_path = pattern.contains('/');
    let mut builder = WalkBuilder::new(&abs);
    builder
        .standard_filters(true)
        .hidden(false)
        .require_git(false)
        .follow_links(false)
        .filter_entry(include_entry);

    let mut matches = Vec::new();
    for result in builder.build() {
        let entry = match result {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let Some(relative) = relative_posix(&abs, entry.path()) else {
            continue;
        };
        let target = if match_path {
            relative.clone()
        } else {
            basename(entry.path()).unwrap_or_default()
        };
        if !matcher.is_match(&target) {
            continue;
        }
        let is_dir = entry
            .file_type()
            .map(|file_type| file_type.is_dir())
            .unwrap_or(false);
        matches.push(if is_dir {
            format!("{relative}/")
        } else {
            relative
        });
    }

    sort_paths(&mut matches);
    if matches.is_empty() {
        return Ok(text_block("No files found matching pattern".to_string()));
    }

    let result_limit_reached = matches.len() > limit;
    let output = matches
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
    if result_limit_reached {
        notices.push(format!(
            "{limit} results limit reached. Use limit={} for more, or refine pattern",
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

pub fn find_tool(cwd: PathBuf) -> AgentTool {
    let execute: ToolFn = Arc::new(move |args, _on_update| {
        let cwd = cwd.clone();
        Box::pin(async move { find_execute(&cwd, args).await.map(AgentToolOutput::new) })
    });
    AgentTool {
        name: "find".into(),
        description: DESCRIPTION.into(),
        parameters: schema(),
        execution_mode: None,
        execute,
    }
}
