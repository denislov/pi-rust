#![allow(clippy::result_large_err)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::collapsible_if)]

mod adapters;
mod app;
mod authorization;
mod events;
mod operations;
mod plugins;
mod profiles;
mod runtime;
mod services;

#[cfg(test)]
extern crate self as pi_coding_agent;
#[cfg(test)]
mod internal_tests;

mod config;
mod protocol;
mod resources;
mod session;
mod theme;
mod tools;

/// Stable, scenario-oriented library facade for embedding or scripting.
///
/// The categories below are the complete supported surface. Implementation
/// owners stay private, and this module intentionally has no flat re-exports.
pub mod api {
    /// Tool invocation authorization requests and decisions.
    pub mod authorization {
        pub use crate::authorization::{
            ToolAuthorizationDecision, ToolAuthorizationMode, ToolAuthorizationPreview,
            ToolAuthorizationRequest, ToolAuthorizationRisk, ToolAuthorizationScope,
        };
    }

    /// Session lifecycle and the product runtime entry point.
    pub mod runtime {
        pub use crate::runtime::facade::{
            CodingAgentCapabilityControl, CodingAgentCapabilityRevocationOutcome,
            CodingAgentOperationTask, CodingAgentRuntimeShutdownHandle, CodingAgentSession,
            CodingAgentSessionOptions, CodingAgentShutdownOutcome, CodingSessionError,
        };
    }

    /// Commands and outcomes accepted by [`runtime::CodingAgentSession`].
    pub mod operation {
        pub use crate::runtime::facade::{
            AgentInvocationOptions, AgentInvocationOutcome, AgentTeamMemberOutcome,
            AgentTeamOptions, AgentTeamOutcome, BranchSummaryReusePolicy, CodingAgentOperation,
            CodingAgentOperationOutcome, CodingAgentPluginLoadOutcome, DelegationConfirmationMode,
            DelegationPolicy, PendingDelegationConfirmation, PromptTurnMode, PromptTurnOptions,
            PromptTurnOutcome, SelfHealingEditCheckOutput, SelfHealingEditDiagnostic,
            SelfHealingEditModelRepairOptions, SelfHealingEditOutcome,
            SelfHealingEditRepairAttempt, SelfHealingEditReplacement, SelfHealingEditRequest,
            SupervisionPolicy,
        };
    }

    /// Durable and live product-event contracts.
    pub mod event {
        pub use crate::runtime::facade::{
            CodingAgentAgentProductEvent, CodingAgentCapabilityProductEvent,
            CodingAgentDelegationEventContext, CodingAgentDelegationProductEvent,
            CodingAgentDiagnosticProductEvent, CodingAgentImageContent,
            CodingAgentMessageProductEvent, CodingAgentProductEvent,
            CodingAgentProductEventCapabilityRevocation, CodingAgentProductEventCheckOutput,
            CodingAgentProductEventDeliveryClass, CodingAgentProductEventDiagnostic,
            CodingAgentProductEventDurability, CodingAgentProductEventError,
            CodingAgentProductEventFamily, CodingAgentProductEventKind,
            CodingAgentProductEventProfileKind, CodingAgentProductEventReceiver,
            CodingAgentProductEventReplacement, CodingAgentProductEventTerminalOperation,
            CodingAgentProductEventTerminalOperationKind, CodingAgentProductEventTerminalStatus,
            CodingAgentProductEventUsage, CodingAgentProfileProductEvent,
            CodingAgentRuntimeProductEvent, CodingAgentSessionProductEvent,
            CodingAgentSessionWriteFailureStatus, CodingAgentSubmittedEventDurability,
            CodingAgentTeamProductEvent, CodingAgentToolProductEvent,
            CodingAgentWorkflowProductEvent,
        };
    }

    /// Client connection, submission, snapshot, and recovery contracts.
    pub mod client {
        pub use crate::runtime::facade::{
            CodingAgentClientConnection, CodingAgentClientId, CodingAgentConnectionGeneration,
            CodingAgentControlId, CodingAgentControlKind, CodingAgentControlReceipt,
            CodingAgentControlRejection, CodingAgentControlRejectionReason,
            CodingAgentDetachOutcome, CodingAgentDraft, CodingAgentDraftId, CodingAgentDraftKind,
            CodingAgentFreshSnapshotRecovery, CodingAgentLifecycleRejection,
            CodingAgentMutationRejection, CodingAgentOperationControl,
            CodingAgentOutcomeAcknowledgementId, CodingAgentPromptControl, CodingAgentReconnect,
            CodingAgentReconnectDelivery, CodingAgentReconnectReceiver, CodingAgentRecoveryReason,
            CodingAgentSnapshot, CodingAgentSnapshotCursor, CodingAgentSubmissionLease,
            CodingAgentSubmittedOperation, CodingAgentSubmittedOperationStatus,
            CodingAgentSubmittedTerminalAnchor, CodingAgentTerminalUncertainty,
        };
    }

    /// Read-only product views and presentation DTOs.
    pub mod view {
        pub use crate::runtime::facade::{
            AgentProfile, CapabilityStatus, CodingAgentCapabilities, CodingAgentPluginDiagnostic,
            CodingAgentSessionExport, CodingAgentSessionExportItem, CodingAgentSessionSummary,
            CodingAgentSessionView, CodingDiagnostic, CodingDiagnosticSeverity, ProfileDiagnostic,
            ProfileId, ProfileKind, ProfileSource, TeamProfile, TeamStrategy, TeamSupervisor,
        };
    }

    /// Versioned machine-readable adapter and wire contracts.
    pub mod protocol {
        pub use crate::adapters::events::CodingProtocolEventAdapter;
        pub use crate::adapters::rpc::{run_rpc_mode_for_io, run_rpc_mode_stdio};
        pub use crate::protocol::jsonl::{JsonlLineReader, read_jsonl_lines, serialize_json_line};
        pub use crate::protocol::types::{
            CompactionProtocolResult, CompactionReason, ProtocolDelegationFoldedBlock,
            ProtocolEvent, ProtocolSelfHealingEditCheckOutput, ProtocolSelfHealingEditReplacement,
            RpcCapabilities, RpcCapabilityStatus, RpcCommand, RpcDelegationCapabilityStatus,
            RpcDelegationRenderingMetadata, RpcDetachLifecycleEvent, RpcDetachRequest,
            RpcDetachResponse, RpcDetachStatus, RpcHelloResponse, RpcNegotiatedProtocolState,
            RpcResponse, RpcSelfHealingEditModelRepair, RpcSelfHealingEditReplacement,
            RpcSessionState, RpcShutdownLifecycleEvent, RpcShutdownRequest, RpcShutdownResponse,
            RpcShutdownStatus, StreamingBehavior, ToolExecutionResult,
        };
        pub use crate::protocol::version::{
            PRODUCT_EVENT_PROTOCOL_VERSION, ProtocolFamilyVersion, RPC_PROTOCOL_VERSION,
            RequestedProtocolVersion, UI_SNAPSHOT_PROTOCOL_VERSION,
        };
    }

    /// Supported command-line scripting contracts.
    pub mod cli {
        /// Argument parsing and resolved command/request values.
        pub mod command {
            pub use crate::app::cli::args::{CliArgs, CliMode, help_text, parse_args};
            pub use crate::app::cli::error::CliError;
            pub use crate::app::cli::request::{
                CliDiagnostic, CliDiagnosticSeverity, ResolvedCliContext, ResolvedPromptRequest,
                render_diagnostics, resolve_cli_context, resolve_prompt_request,
                resolve_session_target,
            };
            pub use crate::app::session::{ResolvedSessionTarget, encode_cwd};
        }

        /// Configuration, credentials, settings, and model selection.
        pub mod configuration {
            pub use crate::app::bootstrap::{
                DEFAULT_MODEL_ID, DEFAULT_SYSTEM_PROMPT, build_agent_config,
                effective_no_context_files, effective_session_dir, select_model,
            };
            pub use crate::app::cli::models::{
                ModelRotation, ModelRotationEntry, parse_model_rotation,
            };
            pub use crate::config::auth::{
                AuthMaterialKind, KeySource, ResolvedKey, resolve_api_key,
            };
            pub use crate::config::settings::{
                CompactionSettings, PartialCompaction, PartialRetry, PartialSettings,
                PartialTerminal, PartialWarnings, RetrySettings, Settings, SettingsScope,
                TerminalSettings, TuiMode, merge_and_save_settings,
            };
            pub use crate::config::{
                AuthStore, Config, ConfigDiagnostic, ConfigPaths, DiagnosticSeverity,
                drain_diagnostics, load_config, resolve_paths,
            };
        }

        /// Prompt input preprocessing and attachment expansion.
        pub mod input {
            pub use crate::app::cli::input::{
                ImageAttachment, ImageProcessingOptions, ImageResizeOptions, ProcessedPromptInput,
                merge_stdin_prompt, process_at_file_references,
                process_at_file_references_with_options,
                process_at_file_references_with_processing_options,
            };
        }

        /// Print-mode orchestration.
        pub mod print {
            pub use crate::adapters::print::{PrintModeOptions, run_print_mode};
        }

        /// Resource discovery and product tool construction.
        pub mod resources {
            pub use crate::resources::{
                ContextFile, LoadedResources, ResourceLoadOptions, ThemeResource,
                build_agent_resources, discover_context_files, find_skill, find_template,
                load_cli_resources, load_cli_resources_with_options, resolve_resource_paths,
                tui_theme_from_resource,
            };
            pub use crate::tools::{ToolFilter, builtin_tools, filter_tools};
        }

        /// CLI runtime options and invocation values.
        pub mod runtime {
            pub use crate::app::bootstrap::{
                CliRunOptions, PromptInvocation, SessionMode, SessionRunOptions,
            };
            pub use crate::app::cli::prompt_options::PromptRunOptions;
        }

        /// High-level process runner entrypoints.
        pub mod runner {
            pub use crate::app::cli::{
                CliOutput, run_cli, run_cli_with_options, run_cli_with_options_and_stdin,
            };
        }

        /// Theme loading, resolution, export, and syntax presentation values.
        pub mod theme {
            pub use crate::theme::{
                ColorValue, DARK_JSON, DetectionConfidence, DetectionSource, ExportSection,
                LIGHT_JSON, REQUIRED_TOKEN_KEYS, ResolveError, ResolvedColor, ResolvedTheme, Rgb,
                SCHEMA_JSON, TerminalBackgroundDetection, TerminalTheme, ThemeBg, ThemeColor,
                ThemeExportColors, ThemeJson, ThemeReloadSignal, ThemeWatcher, builtin_dark,
                builtin_light, detect_terminal_background, get_default_theme,
                get_language_from_path, get_resolved_theme_colors, get_theme_export_colors,
                get_theme_for_rgb_color, highlight_code, is_light_theme,
                parse_osc11_background_color, resolve, should_watch_target,
            };
        }
    }

    /// Deterministic public test fixtures belong here when an external test
    /// support feature is introduced. Production authority handles are never
    /// exported through this category.
    #[cfg(any(test, feature = "test-support"))]
    pub mod testing {}
}

#[cfg(any(test, feature = "test-harness", debug_assertions))]
#[allow(deprecated)]
pub(crate) mod test_support {
    use std::ffi::{OsStr, OsString};
    use std::sync::{Arc, Mutex, MutexGuard};

    use pi_ai::api::client::AiClient;
    use pi_ai::api::provider::ApiProvider;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    pub(crate) fn env_lock() -> MutexGuard<'static, ()> {
        ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[cfg(test)]
    pub(crate) fn make_writable(path: &std::path::Path) {
        let mut permissions = std::fs::metadata(path)
            .unwrap_or_else(|error| {
                panic!("failed to read permissions for {}: {error}", path.display())
            })
            .permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(permissions.mode() | 0o200);
        }
        #[cfg(not(unix))]
        permissions.set_readonly(false);
        std::fs::set_permissions(path, permissions).unwrap_or_else(|error| {
            panic!(
                "failed to restore permissions for {}: {error}",
                path.display()
            )
        });
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

        pub(crate) fn with_pi_rust_dir<V: AsRef<OsStr>>(value: V) -> Self {
            let guard = Self::new(&["PI_RUST_DIR"]);
            guard.set_pi_rust_dir(value);
            guard
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
