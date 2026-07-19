use std::path::{Component, Path};

use thiserror::Error;

use super::grant::{
    ExtensionGrantError, ExtensionOperationScope, ExtensionPermission, OperationCapabilityLease,
};

const MAX_PATH_BYTES: usize = 4096;
const MAX_TEXT_BYTES: usize = 1024 * 1024;
const MAX_ARGUMENTS: usize = 256;
const MAX_ARGUMENT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct ExtensionHostApiHandles {
    pub(crate) workspace: WorkspaceHostHandle,
    pub(crate) model: ModelHostHandle,
    pub(crate) process: ProcessHostHandle,
    pub(crate) ui: UiHostHandle,
}

#[derive(Debug, Clone)]
pub(crate) struct WorkspaceHostHandle(OperationCapabilityLease);

#[derive(Debug, Clone)]
pub(crate) struct ModelHostHandle(OperationCapabilityLease);

#[derive(Debug, Clone)]
pub(crate) struct ProcessHostHandle(OperationCapabilityLease);

#[derive(Debug, Clone)]
pub(crate) struct UiHostHandle(OperationCapabilityLease);

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum AuthorizedHostCall {
    WorkspaceReadText {
        relative_path: String,
    },
    WorkspaceWriteText {
        relative_path: String,
        text: String,
    },
    ModelInvoke {
        prompt: String,
    },
    ProcessExec {
        program: String,
        arguments: Vec<String>,
    },
    UiInteract {
        action_id: String,
        input: Vec<u8>,
    },
}

impl std::fmt::Debug for AuthorizedHostCall {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = formatter.debug_struct("AuthorizedHostCall");
        match self {
            Self::WorkspaceReadText { relative_path } => debug
                .field("kind", &"workspace_read_text")
                .field("path_bytes", &relative_path.len()),
            Self::WorkspaceWriteText {
                relative_path,
                text,
            } => debug
                .field("kind", &"workspace_write_text")
                .field("path_bytes", &relative_path.len())
                .field("text_bytes", &text.len()),
            Self::ModelInvoke { prompt } => debug
                .field("kind", &"model_invoke")
                .field("prompt_bytes", &prompt.len()),
            Self::ProcessExec { program, arguments } => debug
                .field("kind", &"process_exec")
                .field("program_bytes", &program.len())
                .field("argument_count", &arguments.len()),
            Self::UiInteract { action_id, input } => debug
                .field("kind", &"ui_interact")
                .field("action_id_bytes", &action_id.len())
                .field("input_bytes", &input.len()),
        };
        debug.finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum ExtensionHostCallError {
    #[error(transparent)]
    Grant(#[from] ExtensionGrantError),
    #[error("invalid extension Host API input: {0}")]
    InvalidInput(&'static str),
}

impl ExtensionHostApiHandles {
    pub(crate) fn new(lease: OperationCapabilityLease) -> Self {
        Self {
            workspace: WorkspaceHostHandle(lease.clone()),
            model: ModelHostHandle(lease.clone()),
            process: ProcessHostHandle(lease.clone()),
            ui: UiHostHandle(lease),
        }
    }
}

impl WorkspaceHostHandle {
    pub(crate) fn authorize_read_text(
        &self,
        operation_id: &str,
        scope: &ExtensionOperationScope,
        relative_path: String,
    ) -> Result<AuthorizedHostCall, ExtensionHostCallError> {
        validate_relative_path(&relative_path)?;
        self.0
            .authorize(operation_id, scope, ExtensionPermission::WorkspaceRead)?;
        Ok(AuthorizedHostCall::WorkspaceReadText { relative_path })
    }

    pub(crate) fn authorize_write_text(
        &self,
        operation_id: &str,
        scope: &ExtensionOperationScope,
        relative_path: String,
        text: String,
    ) -> Result<AuthorizedHostCall, ExtensionHostCallError> {
        validate_relative_path(&relative_path)?;
        validate_bytes(text.len(), MAX_TEXT_BYTES, "workspace text exceeds limit")?;
        self.0
            .authorize(operation_id, scope, ExtensionPermission::WorkspaceWrite)?;
        Ok(AuthorizedHostCall::WorkspaceWriteText {
            relative_path,
            text,
        })
    }
}

impl ModelHostHandle {
    pub(crate) fn authorize_invoke(
        &self,
        operation_id: &str,
        scope: &ExtensionOperationScope,
        prompt: String,
    ) -> Result<AuthorizedHostCall, ExtensionHostCallError> {
        if prompt.is_empty() {
            return Err(ExtensionHostCallError::InvalidInput(
                "model prompt cannot be empty",
            ));
        }
        validate_bytes(prompt.len(), MAX_TEXT_BYTES, "model prompt exceeds limit")?;
        self.0
            .authorize(operation_id, scope, ExtensionPermission::ModelInvoke)?;
        Ok(AuthorizedHostCall::ModelInvoke { prompt })
    }
}

impl ProcessHostHandle {
    pub(crate) fn authorize_exec(
        &self,
        operation_id: &str,
        scope: &ExtensionOperationScope,
        program: String,
        arguments: Vec<String>,
    ) -> Result<AuthorizedHostCall, ExtensionHostCallError> {
        if program.is_empty() || program.len() > MAX_PATH_BYTES || program.as_bytes().contains(&0) {
            return Err(ExtensionHostCallError::InvalidInput(
                "process program is invalid",
            ));
        }
        if arguments.len() > MAX_ARGUMENTS
            || arguments
                .iter()
                .any(|argument| argument.as_bytes().contains(&0))
            || arguments.iter().map(String::len).sum::<usize>() > MAX_ARGUMENT_BYTES
        {
            return Err(ExtensionHostCallError::InvalidInput(
                "process arguments exceed limits",
            ));
        }
        self.0
            .authorize(operation_id, scope, ExtensionPermission::ProcessExec)?;
        Ok(AuthorizedHostCall::ProcessExec { program, arguments })
    }
}

impl UiHostHandle {
    pub(crate) fn authorize_interact(
        &self,
        operation_id: &str,
        scope: &ExtensionOperationScope,
        action_id: String,
        input: Vec<u8>,
    ) -> Result<AuthorizedHostCall, ExtensionHostCallError> {
        validate_key(&action_id)?;
        validate_bytes(input.len(), MAX_TEXT_BYTES, "UI input exceeds limit")?;
        self.0
            .authorize(operation_id, scope, ExtensionPermission::UiInteract)?;
        Ok(AuthorizedHostCall::UiInteract { action_id, input })
    }
}

fn validate_relative_path(path: &str) -> Result<(), ExtensionHostCallError> {
    if path.is_empty()
        || path.len() > MAX_PATH_BYTES
        || path.as_bytes().contains(&0)
        || Path::new(path).is_absolute()
        || Path::new(path)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ExtensionHostCallError::InvalidInput(
            "workspace path must be a bounded relative path",
        ));
    }
    Ok(())
}

fn validate_key(key: &str) -> Result<(), ExtensionHostCallError> {
    if key.is_empty()
        || key.len() > 256
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'/'))
    {
        return Err(ExtensionHostCallError::InvalidInput(
            "Host API identifier is invalid",
        ));
    }
    Ok(())
}

fn validate_bytes(
    actual: usize,
    maximum: usize,
    message: &'static str,
) -> Result<(), ExtensionHostCallError> {
    if actual > maximum {
        Err(ExtensionHostCallError::InvalidInput(message))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::time::{Duration, Instant};

    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::extensions::grant::{
        ExtensionGrantIdentity, ExtensionGrantRegistry, ExtensionGrantScope,
        ExtensionSourceChannel, ExtensionTrustLevel, GrantRecord,
    };

    fn scope() -> ExtensionOperationScope {
        ExtensionOperationScope {
            workspace_id: "workspace-1".into(),
            session_id: Some("session-1".into()),
        }
    }

    fn handles() -> ExtensionHostApiHandles {
        let permissions = [
            ("model.invoke", ExtensionPermission::ModelInvoke),
            ("process.exec", ExtensionPermission::ProcessExec),
            ("ui.interact", ExtensionPermission::UiInteract),
            ("workspace.read", ExtensionPermission::WorkspaceRead),
            ("workspace.write", ExtensionPermission::WorkspaceWrite),
        ];
        let registry = ExtensionGrantRegistry::default();
        registry
            .install(
                GrantRecord::new(
                    ExtensionGrantIdentity {
                        id: "example.host".into(),
                        version: "1.0.0".into(),
                        package_digest: "a".repeat(64),
                    },
                    ExtensionSourceChannel::Local,
                    "b".repeat(64),
                    ExtensionTrustLevel::Untrusted,
                    ExtensionGrantScope {
                        workspace_id: "workspace-1".into(),
                        session_ids: BTreeSet::from(["session-1".into()]),
                    },
                    permissions.iter().map(|(name, _)| *name),
                    permissions.iter().map(|(_, permission)| *permission),
                    "pi:extension/extension@0.1.0".into(),
                )
                .unwrap(),
            )
            .unwrap();
        let lease = registry
            .admit(
                "workspace-1",
                "example.host",
                "op-host".into(),
                scope(),
                Instant::now() + Duration::from_secs(30),
                CancellationToken::new(),
            )
            .unwrap();
        ExtensionHostApiHandles::new(lease)
    }

    #[test]
    fn each_host_family_authorizes_a_structured_bounded_call() {
        let handles = handles();
        let scope = scope();

        assert!(matches!(
            handles
                .workspace
                .authorize_read_text("op-host", &scope, "src/lib.rs".into()),
            Ok(AuthorizedHostCall::WorkspaceReadText { .. })
        ));
        assert!(matches!(
            handles.workspace.authorize_write_text(
                "op-host",
                &scope,
                "notes.txt".into(),
                "text".into()
            ),
            Ok(AuthorizedHostCall::WorkspaceWriteText { .. })
        ));
        assert!(
            handles
                .model
                .authorize_invoke("op-host", &scope, "prompt".into())
                .is_ok()
        );
        assert!(
            handles
                .process
                .authorize_exec("op-host", &scope, "git".into(), vec!["status".into()])
                .is_ok()
        );
        assert!(
            handles
                .ui
                .authorize_interact("op-host", &scope, "dialog.open".into(), vec![])
                .is_ok()
        );
    }

    #[test]
    fn host_handles_reject_escape_wrong_operation_and_oversized_input() {
        let handles = handles();
        let scope = scope();

        assert!(matches!(
            handles
                .workspace
                .authorize_read_text("op-host", &scope, "../secret".into()),
            Err(ExtensionHostCallError::InvalidInput(_))
        ));
        assert!(matches!(
            handles
                .model
                .authorize_invoke("op-other", &scope, "prompt".into()),
            Err(ExtensionHostCallError::Grant(
                ExtensionGrantError::IdentityMismatch
            ))
        ));
        assert!(matches!(
            handles.ui.authorize_interact(
                "op-host",
                &scope,
                "dialog.open".into(),
                vec![0; MAX_TEXT_BYTES + 1]
            ),
            Err(ExtensionHostCallError::InvalidInput(_))
        ));
    }

    #[test]
    fn authorized_call_debug_redacts_guest_payloads() {
        let call = handles()
            .model
            .authorize_invoke("op-host", &scope(), "sensitive prompt".into())
            .unwrap();
        let debug = format!("{call:?}");

        assert!(debug.contains("prompt_bytes"));
        assert!(!debug.contains("sensitive prompt"));
    }
}
