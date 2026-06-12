use pi_agent_core::AgentTool;
use std::path::PathBuf;

pub mod bash;
pub mod edit;
pub mod find;
pub mod grep;
pub mod ls;
pub mod path;
pub mod read;
pub mod truncate;
pub mod write;

pub fn builtin_tools(cwd: PathBuf) -> Vec<AgentTool> {
    vec![
        read::read_tool(cwd.clone()),
        write::write_tool(cwd.clone()),
        edit::edit_tool(cwd.clone()),
        bash::bash_tool(cwd.clone()),
        grep::grep_tool(cwd.clone()),
        find::find_tool(cwd.clone()),
        ls::ls_tool(cwd),
    ]
}
