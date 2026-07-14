use pi_agent_core::AgentTool;
use std::collections::BTreeSet;
use std::path::PathBuf;

pub use crate::coding_session::{FilesystemCapability, ShellCapability};

pub mod bash;
pub mod edit;
mod edit_diff;
pub mod file_mutation_queue;
pub mod find;
pub mod grep;
pub mod ls;
pub mod path;
pub mod read;
pub mod truncate;
pub mod write;

pub fn builtin_tools(cwd: PathBuf) -> Vec<AgentTool> {
    let filesystem = FilesystemCapability::new(cwd.clone());
    let shell = ShellCapability::new(cwd);
    vec![
        read::read_tool(filesystem.clone()),
        write::write_tool(filesystem.clone()),
        edit::edit_tool(filesystem.clone()),
        bash::bash_tool(shell),
        grep::grep_tool(filesystem.clone()),
        find::find_tool(filesystem.clone()),
        ls::ls_tool(filesystem),
    ]
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
