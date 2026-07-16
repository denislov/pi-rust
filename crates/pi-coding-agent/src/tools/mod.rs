use pi_agent_core::api::tool::AgentTool;
use std::collections::BTreeSet;
use std::path::PathBuf;

pub use crate::runtime::facade::{FilesystemCapability, ShellCapability};

pub(crate) mod filesystem;
pub(crate) mod mutation_queue;
pub(crate) mod output;
pub(crate) mod shell;

pub fn builtin_tools(cwd: PathBuf) -> Vec<AgentTool> {
    let filesystem = FilesystemCapability::new(cwd.clone());
    let shell = ShellCapability::new(cwd);
    builtin_tools_with_capabilities(&filesystem, &shell)
}

fn builtin_tools_with_capabilities(
    filesystem: &FilesystemCapability,
    shell: &ShellCapability,
) -> Vec<AgentTool> {
    vec![
        filesystem::read::read_tool(filesystem.clone()),
        filesystem::write::write_tool(filesystem.clone()),
        filesystem::edit::edit_tool(filesystem.clone()),
        shell::bash_tool(shell.clone()),
        filesystem::grep::grep_tool(filesystem.clone()),
        filesystem::find::find_tool(filesystem.clone()),
        filesystem::ls::ls_tool(filesystem.clone()),
    ]
}

pub(crate) fn bind_builtin_tool_to_capabilities(
    tool: AgentTool,
    filesystem: Option<&FilesystemCapability>,
    shell: Option<&ShellCapability>,
) -> Option<AgentTool> {
    match tool.name.as_str() {
        "read" => filesystem.cloned().map(filesystem::read::read_tool),
        "write" => filesystem.cloned().map(filesystem::write::write_tool),
        "edit" => filesystem.cloned().map(filesystem::edit::edit_tool),
        "grep" => filesystem.cloned().map(filesystem::grep::grep_tool),
        "find" => filesystem.cloned().map(filesystem::find::find_tool),
        "ls" => filesystem.cloned().map(filesystem::ls::ls_tool),
        "bash" => shell.cloned().map(shell::bash_tool),
        _ => Some(tool),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolFilter {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
    pub no_tools: bool,
    pub no_builtin_tools: bool,
}

pub fn filter_tools(tools: Vec<AgentTool>, filter: &ToolFilter) -> Vec<AgentTool> {
    if filter.no_tools {
        return Vec::new();
    }
    let allow: BTreeSet<_> = filter.allow.iter().map(String::as_str).collect();
    let deny: BTreeSet<_> = filter.deny.iter().map(String::as_str).collect();
    let builtins = BTreeSet::from(["read", "write", "edit", "bash", "grep", "find", "ls"]);
    tools
        .into_iter()
        .filter(|tool| !filter.no_builtin_tools || !builtins.contains(tool.name.as_str()))
        .filter(|tool| allow.is_empty() || allow.contains(tool.name.as_str()))
        .filter(|tool| !deny.contains(tool.name.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pi_ai::api::conversation::ContentBlock;

    #[tokio::test]
    async fn builtin_binding_replaces_bootstrap_closure_with_snapshot_filesystem() {
        let bootstrap = tempfile::tempdir().unwrap();
        let admitted = tempfile::tempdir().unwrap();
        std::fs::write(bootstrap.path().join("scope.txt"), "bootstrap").unwrap();
        std::fs::write(admitted.path().join("scope.txt"), "admitted").unwrap();
        let original = builtin_tools(bootstrap.path().to_path_buf())
            .into_iter()
            .find(|tool| tool.name == "read")
            .unwrap();
        let filesystem = FilesystemCapability::new(admitted.path().to_path_buf());

        let rebound = bind_builtin_tool_to_capabilities(original, Some(&filesystem), None).unwrap();
        let output = (rebound.execute)(
            pi_agent_core::api::tool::ToolExecutionContext::standalone("read"),
            serde_json::json!({"path": "scope.txt"}),
            None,
        )
        .await
        .unwrap();
        let text = output
            .content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<String>();

        assert!(text.contains("admitted"));
        assert!(!text.contains("bootstrap"));
    }

    #[test]
    fn builtin_binding_drops_tools_without_the_required_handle() {
        let cwd = tempfile::tempdir().unwrap();
        let mut tools = builtin_tools(cwd.path().to_path_buf()).into_iter();
        let read = tools.find(|tool| tool.name == "read").unwrap();
        let bash = tools.find(|tool| tool.name == "bash").unwrap();

        assert!(bind_builtin_tool_to_capabilities(read, None, None).is_none());
        assert!(bind_builtin_tool_to_capabilities(bash, None, None).is_none());
    }
}
