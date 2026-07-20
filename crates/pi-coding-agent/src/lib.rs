mod adapters;
mod app;
mod authorization;
mod events;
mod extensions;
mod operations;
mod profiles;
mod runtime;
mod services;

#[cfg(test)]
extern crate self as pi_coding_agent;
#[cfg(test)]
mod internal_tests;

mod config;
mod contributions;
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
        pub use crate::app::bootstrap::{CliRunOptions, SessionMode, SessionRunOptions};
        pub use crate::runtime::facade::{
            CodingAgentCapabilityControl, CodingAgentCapabilityRevocationOutcome,
            CodingAgentOperationTask, CodingAgentRecoveryResolutionRequest,
            CodingAgentRecoveryResolutionResult, CodingAgentRecoveryRetryRequest,
            CodingAgentRecoveryRetryResult, CodingAgentRuntimeShutdownHandle, CodingAgentSession,
            CodingAgentSessionOptions, CodingAgentShutdownOutcome, CodingSessionError,
        };
    }

    /// Trusted-host installation, grant, and workspace activation contracts.
    pub mod extension {
        pub use crate::runtime::facade::{
            CodingAgentExtensionActivation, CodingAgentExtensionActivationRequest,
            CodingAgentExtensionGrantRequest, CodingAgentExtensionPermission,
            CodingAgentExtensionSourceChannel, CodingAgentExtensionTrustLevel,
            CodingAgentInstalledExtensionPackage,
        };
    }

    /// Commands and outcomes accepted by [`runtime::CodingAgentSession`].
    pub mod operation {
        pub use crate::app::bootstrap::PromptInvocation;
        pub use crate::app::cli::prompt_options::PromptRunOptions;
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
            CodingAgentRecoveryResolution, CodingAgentRuntimeProductEvent,
            CodingAgentSessionProductEvent, CodingAgentSessionWriteFailureStatus,
            CodingAgentSubmittedEventDurability, CodingAgentTeamProductEvent,
            CodingAgentToolProductEvent, CodingAgentWorkflowProductEvent,
        };
    }

    /// Client connection, submission, snapshot, and recovery contracts.
    pub mod client {
        pub use crate::runtime::facade::{
            CodingAgentClientConnection, CodingAgentClientId, CodingAgentConnectionGeneration,
            CodingAgentContextSnapshot, CodingAgentControlId, CodingAgentControlKind,
            CodingAgentControlReceipt, CodingAgentControlRejection,
            CodingAgentControlRejectionReason, CodingAgentDelegationSnapshot,
            CodingAgentDetachOutcome, CodingAgentDraft, CodingAgentDraftId, CodingAgentDraftKind,
            CodingAgentFileChangeSnapshot, CodingAgentFreshSnapshotRecovery,
            CodingAgentLifecycleRejection, CodingAgentMutationRejection,
            CodingAgentOperationControl, CodingAgentOperationSnapshot, CodingAgentOperationStatus,
            CodingAgentOutcomeAcknowledgementId, CodingAgentPromptControl, CodingAgentReconnect,
            CodingAgentReconnectDelivery, CodingAgentReconnectReceiver, CodingAgentRecoveryPending,
            CodingAgentRecoveryReason, CodingAgentRecoveryResolutionRequest,
            CodingAgentRecoveryResolutionResult, CodingAgentRecoveryRetryRequest,
            CodingAgentRecoveryRetryResult, CodingAgentSnapshot, CodingAgentSnapshotCursor,
            CodingAgentSubmissionLease, CodingAgentSubmittedOperation,
            CodingAgentSubmittedOperationStatus, CodingAgentSubmittedTerminalAnchor,
            CodingAgentTerminalUncertainty, CodingAgentTurnUsageSnapshot, CodingAgentUsageSnapshot,
        };
    }

    /// Read-only product views and presentation DTOs.
    pub mod view {
        pub use crate::runtime::facade::{
            AgentProfile, CapabilityStatus, CodingAgentCapabilities, CodingAgentPluginDiagnostic,
            CodingAgentRecoveryPending, CodingAgentSessionExport, CodingAgentSessionExportItem,
            CodingAgentSessionSummary, CodingAgentSessionView, CodingDiagnostic,
            CodingDiagnosticSeverity, ProfileDiagnostic, ProfileId, ProfileKind, ProfileSource,
            TeamProfile, TeamStrategy, TeamSupervisor,
        };
    }

    /// Versioned machine-readable adapter and wire contracts.
    pub mod protocol {
        pub use crate::adapters::events::CodingProtocolEventAdapter;
        pub use crate::adapters::print::{PrintModeOptions, run_print_mode};
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
        /// High-level process runner entrypoints.
        pub mod runner {
            pub use crate::app::cli::{
                CliOutput, run_cli, run_cli_stdio, run_cli_with_options,
                run_cli_with_options_and_stdin,
            };
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
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
