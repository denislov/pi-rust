#![allow(clippy::result_large_err)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::collapsible_if)]

mod coding_session;
mod plugins;

use crate::args::{CliMode, help_text, parse_args};
use crate::error::CliError;
use crate::runtime::{CliRunOptions, SessionRunOptions};
use crate::tools::builtin_tools;

mod args;
mod config;
mod error;
mod input;
#[doc(hidden)]
pub mod interactive;
mod list_models;
mod models;
mod print_mode;
mod prompt_options;
mod protocol;
mod request;
mod resources;
mod runtime;
mod session;
mod theme;
#[doc(hidden)]
pub mod tools;

/// Stable library facade for embedding or scripting `pi-coding-agent`.
///
/// The root modules remain public during the migration, but downstream crates
/// should prefer this module for APIs that are intended to stay stable.
pub mod api {
    pub use crate::args::{CliArgs, CliMode, help_text, parse_args};
    pub use crate::coding_session::{
        AgentInvocationOptions, AgentInvocationOutcome, AgentProfile, AgentTeamMemberOutcome,
        AgentTeamOptions, AgentTeamOutcome, BranchSummaryReusePolicy, CapabilityRevocationPolicy,
        CapabilityStatus, CodingAgentAgentProductEvent, CodingAgentCapabilities,
        CodingAgentCapabilityProductEvent, CodingAgentClientConnection, CodingAgentClientId,
        CodingAgentConnectionGeneration, CodingAgentControlId, CodingAgentControlKind,
        CodingAgentControlReceipt, CodingAgentControlRejection, CodingAgentControlRejectionReason,
        CodingAgentDelegationEventContext, CodingAgentDelegationProductEvent,
        CodingAgentDetachOutcome, CodingAgentDiagnosticProductEvent, CodingAgentDraft,
        CodingAgentDraftId, CodingAgentDraftKind, CodingAgentFreshSnapshotRecovery,
        CodingAgentLifecycleRejection, CodingAgentMessageProductEvent,
        CodingAgentMutationRejection, CodingAgentOperation, CodingAgentOperationOutcome,
        CodingAgentOutcomeAcknowledgementId, CodingAgentPluginDiagnostic,
        CodingAgentPluginLoadOutcome, CodingAgentProductEvent,
        CodingAgentProductEventCapabilityRevocation, CodingAgentProductEventCheckOutput,
        CodingAgentProductEventDiagnostic, CodingAgentProductEventDurability,
        CodingAgentProductEventError, CodingAgentProductEventFamily, CodingAgentProductEventKind,
        CodingAgentProductEventProfileKind, CodingAgentProductEventReceiver,
        CodingAgentProductEventReplacement, CodingAgentProductEventTerminalOperation,
        CodingAgentProductEventTerminalOperationKind, CodingAgentProductEventTerminalStatus,
        CodingAgentProductEventUsage, CodingAgentProfileProductEvent, CodingAgentPromptControl,
        CodingAgentReconnect, CodingAgentReconnectDelivery, CodingAgentReconnectReceiver,
        CodingAgentRecoveryReason, CodingAgentRuntimeProductEvent,
        CodingAgentRuntimeShutdownHandle, CodingAgentSession, CodingAgentSessionExport,
        CodingAgentSessionExportItem, CodingAgentSessionOptions, CodingAgentSessionProductEvent,
        CodingAgentSessionSummary, CodingAgentSessionView, CodingAgentShutdownOutcome,
        CodingAgentSnapshot, CodingAgentSnapshotCursor, CodingAgentSubmissionLease,
        CodingAgentSubmittedEventDurability, CodingAgentSubmittedOperation,
        CodingAgentSubmittedOperationStatus, CodingAgentSubmittedTerminalAnchor,
        CodingAgentTeamProductEvent, CodingAgentTerminalUncertainty, CodingAgentToolProductEvent,
        CodingAgentWorkflowProductEvent, CodingDiagnostic, CodingDiagnosticSeverity,
        CodingSessionError, DelegationConfirmationMode, DelegationPolicy,
        PendingDelegationConfirmation, ProfileDiagnostic, ProfileId, ProfileKind, ProfileSource,
        PromptTurnMode, PromptTurnOptions, PromptTurnOutcome, SelfHealingEditCheckOutput,
        SelfHealingEditDiagnostic, SelfHealingEditModelRepairOptions, SelfHealingEditOutcome,
        SelfHealingEditRepairAttempt, SelfHealingEditReplacement, SelfHealingEditRequest,
        SupervisionPolicy, TeamProfile, TeamStrategy, TeamSupervisor,
    };
    pub use crate::config::auth::{AuthMaterialKind, KeySource, ResolvedKey, resolve_api_key};
    pub use crate::config::settings::{
        CompactionSettings, PartialCompaction, PartialRetry, PartialSettings, PartialTerminal,
        PartialWarnings, RetrySettings, Settings, SettingsScope, TerminalSettings,
        WarningsSettings, merge_and_save_settings,
    };
    pub use crate::config::{
        AuthStore, Config, ConfigDiagnostic, ConfigPaths, DiagnosticSeverity, drain_diagnostics,
        load_config, resolve_paths,
    };
    pub use crate::error::CliError;
    pub use crate::input::{
        ImageAttachment, ImageProcessingOptions, ImageResizeOptions, ProcessedPromptInput,
        merge_stdin_prompt, process_at_file_references, process_at_file_references_with_options,
        process_at_file_references_with_processing_options,
    };
    pub use crate::models::{ModelRotation, ModelRotationEntry, parse_model_rotation};
    pub use crate::print_mode::{PrintModeOptions, run_print_mode};
    pub use crate::prompt_options::PromptRunOptions;
    pub use crate::protocol::events::CodingProtocolEventAdapter;
    pub use crate::protocol::jsonl::{JsonlLineReader, read_jsonl_lines, serialize_json_line};
    pub use crate::protocol::rpc::{run_rpc_mode_for_io, run_rpc_mode_stdio};
    pub use crate::protocol::types::{
        CompactionProtocolResult, CompactionReason, ProtocolDelegationFoldedBlock, ProtocolEvent,
        ProtocolSelfHealingEditCheckOutput, ProtocolSelfHealingEditReplacement, RpcCapabilities,
        RpcCapabilityStatus, RpcCommand, RpcDelegationCapabilityStatus,
        RpcDelegationRenderingMetadata, RpcDetachLifecycleEvent, RpcDetachRequest,
        RpcDetachResponse, RpcDetachStatus, RpcHelloResponse, RpcNegotiatedProtocolState,
        RpcResponse, RpcSelfHealingEditModelRepair, RpcSelfHealingEditReplacement, RpcSessionState,
        RpcShutdownLifecycleEvent, RpcShutdownRequest, RpcShutdownResponse, RpcShutdownStatus,
        StreamingBehavior, ToolExecutionResult,
    };
    pub use crate::protocol::version::{
        PRODUCT_EVENT_PROTOCOL_VERSION, ProtocolFamilyVersion, RPC_PROTOCOL_VERSION,
        RequestedProtocolVersion, UI_SNAPSHOT_PROTOCOL_VERSION,
    };
    pub use crate::request::{
        CliDiagnostic, CliDiagnosticSeverity, ResolvedCliContext, ResolvedPromptRequest,
        render_diagnostics, resolve_cli_context, resolve_prompt_request, resolve_session_target,
    };
    pub use crate::resources::{
        ContextFile, LoadedResources, ResourceLoadOptions, ThemeResource, build_agent_resources,
        discover_context_files, find_skill, find_template, load_cli_resources,
        load_cli_resources_with_options, resolve_resource_paths, tui_theme_from_resource,
    };
    pub use crate::runtime::{
        CliRunOptions, DEFAULT_MODEL_ID, DEFAULT_SYSTEM_PROMPT, PromptInvocation, SessionMode,
        SessionRunOptions, build_agent_config, effective_no_context_files, effective_session_dir,
        select_model,
    };
    pub use crate::session::{ResolvedSessionTarget, encode_cwd};
    pub use crate::theme::{
        ColorValue, DARK_JSON, DetectionConfidence, DetectionSource, ExportSection, LIGHT_JSON,
        REQUIRED_TOKEN_KEYS, ResolveError, ResolvedColor, ResolvedTheme, Rgb, SCHEMA_JSON,
        TerminalBackgroundDetection, TerminalTheme, ThemeBg, ThemeColor, ThemeExportColors,
        ThemeJson, ThemeReloadSignal, ThemeWatcher, builtin_dark, builtin_light,
        detect_terminal_background, get_default_theme, get_language_from_path,
        get_resolved_theme_colors, get_theme_export_colors, get_theme_for_rgb_color,
        highlight_code, is_light_theme, parse_osc11_background_color, resolve, should_watch_target,
    };
    pub use crate::tools::{ToolFilter, builtin_tools, filter_tools};
    pub use crate::{CliOutput, run_cli, run_cli_with_options, run_cli_with_options_and_stdin};
}

#[cfg(any(test, feature = "test-harness", debug_assertions))]
#[allow(deprecated)]
pub(crate) mod test_support {
    use std::ffi::{OsStr, OsString};
    use std::sync::{Arc, Mutex, MutexGuard};

    use pi_ai::api::AiClient;
    use pi_ai::api::ApiProvider;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    pub(crate) fn env_lock() -> MutexGuard<'static, ()> {
        ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(crate) struct EnvGuard<'a> {
        _lock: MutexGuard<'a, ()>,
        saved: Vec<(&'static str, Option<OsString>)>,
    }

    #[allow(dead_code)]
    impl EnvGuard<'static> {
        pub(crate) fn new(names: &[&'static str]) -> Self {
            let lock = env_lock();
            let saved = names
                .iter()
                .map(|name| (*name, std::env::var_os(name)))
                .collect();
            Self { _lock: lock, saved }
        }
    }

    #[allow(dead_code)]
    impl EnvGuard<'_> {
        pub(crate) fn set<V: AsRef<OsStr>>(&self, name: &str, value: V) {
            unsafe {
                std::env::set_var(name, value);
            }
        }

        pub(crate) fn remove(&self, name: &str) {
            unsafe {
                std::env::remove_var(name);
            }
        }

        pub(crate) fn set_pi_rust_dir<V: AsRef<OsStr>>(&self, value: V) {
            self.set("PI_RUST_DIR", value);
        }
    }

    impl Drop for EnvGuard<'_> {
        fn drop(&mut self) {
            for (name, value) in self.saved.iter().rev() {
                unsafe {
                    match value {
                        Some(value) => std::env::set_var(name, value),
                        None => std::env::remove_var(name),
                    }
                }
            }
        }
    }

    pub(crate) struct ProviderGuard {
        ai_client: AiClient,
    }

    #[allow(dead_code)]
    impl ProviderGuard {
        pub(crate) fn register(api: impl Into<String>, provider: Arc<dyn ApiProvider>) -> Self {
            Self::register_many(vec![(api.into(), provider)])
        }

        pub(crate) fn register_many(providers: Vec<(String, Arc<dyn ApiProvider>)>) -> Self {
            let ai_client = AiClient::new();
            for (api, provider) in providers {
                ai_client.register_provider(api, provider);
            }
            Self { ai_client }
        }

        pub(crate) fn ai_client(&self) -> AiClient {
            self.ai_client.clone()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CliOutput {
    fn success(stdout: String) -> Self {
        Self {
            exit_code: 0,
            stdout,
            stderr: String::new(),
        }
    }

    fn failure(error: CliError) -> Self {
        Self {
            exit_code: 1,
            stdout: String::new(),
            stderr: format!("{error}\n"),
        }
    }

    fn with_stderr(mut self, stderr: String) -> Self {
        if !stderr.is_empty() {
            self.stderr = format!("{stderr}{}", self.stderr);
        }
        self
    }
}

fn stdout_with_trailing_newline(text: String) -> String {
    if text.is_empty() {
        String::new()
    } else if text.ends_with('\n') {
        text
    } else {
        format!("{text}\n")
    }
}

fn default_cli_options(cwd: std::path::PathBuf) -> CliRunOptions {
    CliRunOptions {
        model_override: None,
        tools: builtin_tools(cwd.clone()),
        register_builtins: true,
        ai_client: None,
        session: SessionRunOptions::enabled(cwd),
    }
}

pub async fn run_cli(args: impl IntoIterator<Item = String>) -> CliOutput {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    run_cli_with_options(args, default_cli_options(cwd)).await
}

pub async fn run_cli_with_options(
    args: impl IntoIterator<Item = String>,
    options: CliRunOptions,
) -> CliOutput {
    run_cli_with_options_and_stdin(args, options, None).await
}

pub async fn run_cli_with_options_and_stdin(
    args: impl IntoIterator<Item = String>,
    options: CliRunOptions,
    stdin: Option<String>,
) -> CliOutput {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(error) => return CliOutput::failure(error),
    };

    if parsed.help {
        return CliOutput::success(help_text());
    }

    if parsed.version {
        return CliOutput::success(format!("{}\n", env!("CARGO_PKG_VERSION")));
    }

    if let Some(search) = parsed.list_models.as_ref() {
        return match list_models::list_models_output(
            search.as_deref(),
            parsed.provider.as_deref(),
            parsed.json,
        ) {
            Ok(stdout) => CliOutput::success(stdout),
            Err(error) => CliOutput::failure(error),
        };
    }

    if !parsed.print && !parsed.mode_explicit {
        return interactive::run_interactive_mode(parsed, options).await;
    }

    if parsed.mode == CliMode::Rpc {
        return CliOutput::failure(CliError::UnsupportedMode(
            "rpc requires the streaming binary entry point".into(),
        ));
    }

    if let Some(models) = parsed.models.as_deref()
        && let Err(error) = models::parse_model_rotation(models)
    {
        return CliOutput::failure(error);
    }

    let config_paths = config::resolve_paths(&cwd);
    let resolved = match request::resolve_prompt_request(
        parsed.clone(),
        options,
        stdin,
        cwd,
        config_paths.global_dir,
    ) {
        Ok(resolved) => resolved,
        Err(error) => return CliOutput::failure(error),
    };
    let session_prompt_options = resolved.session_options;
    let diagnostic_text = request::render_diagnostics(&resolved.context.diagnostics);

    match parsed.mode {
        CliMode::Print => {
            match print_mode::run_print_prompt_options(session_prompt_options).await {
                Ok(text) => CliOutput::success(stdout_with_trailing_newline(text))
                    .with_stderr(diagnostic_text),
                Err(error) => CliOutput::failure(error).with_stderr(diagnostic_text),
            }
        }
        CliMode::Json => protocol::json_mode::run_json_mode(session_prompt_options)
            .await
            .with_stderr(diagnostic_text),
        CliMode::Rpc => CliOutput::failure(CliError::UnsupportedMode(
            "rpc requires the streaming binary entry point".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cli_options_include_builtin_tools() {
        let options = default_cli_options(std::path::PathBuf::from("."));
        let names: Vec<_> = options.tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["read", "write", "edit", "bash", "grep", "find", "ls"]
        );
        assert!(options.register_builtins);
        assert!(options.model_override.is_none());
    }
}
