use std::path::PathBuf;

use futures::future::BoxFuture;

use crate::execution::ExecutionError;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecutionOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecOptions {
    pub cwd: Option<PathBuf>,
}

pub trait Shell: Send + Sync {
    fn exec<'a>(
        &'a self,
        command: &'a str,
        options: Option<ExecOptions>,
    ) -> BoxFuture<'a, Result<ExecutionOutput, ExecutionError>>;
    fn cleanup_shell<'a>(&'a self) -> BoxFuture<'a, ()>;
}
