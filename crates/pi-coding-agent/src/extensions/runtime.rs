use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use thiserror::Error;
use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder, UpdateDeadline};

use super::grant::{ExtensionGrantError, ExtensionOperationScope, OperationCapabilityLease};
use super::host::{AuthorizedHostCall, ExtensionHostApiHandles};
use super::package::ValidatedPackageDirectory;
use crate::contributions::ExtensionHandlerRef;

mod bindings {
    wasmtime::component::bindgen!({
        path: "../../contracts/extensions/0.1.0",
        world: "extension",
        imports: { default: async },
        exports: { default: async },
        require_store_data_send: true,
    });
}

use bindings::pi::extension::types::ExtensionError as GuestExtensionError;
use bindings::pi::extension::{
    host_diagnostics, host_model, host_process, host_ui, host_workspace,
};

const EPOCH_INTERVAL: Duration = Duration::from_millis(5);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExtensionInvocationOutput {
    pub(crate) schema_revision: u32,
    pub(crate) output: Vec<u8>,
}

#[derive(Debug, Error)]
pub(crate) enum ExtensionRuntimeError {
    #[error("extension component contract is invalid")]
    InvalidComponent,
    #[error("extension component was not prepared before operation admission")]
    ComponentNotPrepared,
    #[error("extension invocation identity does not match package")]
    IdentityMismatch,
    #[error("extension invocation was denied")]
    Denied,
    #[error("extension invocation was cancelled")]
    Cancelled,
    #[error("extension invocation exceeded its deadline")]
    DeadlineExceeded,
    #[error("extension invocation exceeded its output limit")]
    OutputLimit,
    #[error("extension invocation trapped")]
    Trapped,
}

#[derive(Clone)]
pub(crate) struct ExtensionComponentRuntime {
    engine: Engine,
    components: Arc<Mutex<BTreeMap<String, Arc<Component>>>>,
}

impl std::fmt::Debug for ExtensionComponentRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExtensionComponentRuntime")
            .field(
                "cached_component_count",
                &self
                    .components
                    .lock()
                    .expect("extension component cache lock poisoned")
                    .len(),
            )
            .finish_non_exhaustive()
    }
}

struct InvocationState {
    operation_id: String,
    scope: ExtensionOperationScope,
    lease: OperationCapabilityLease,
    host: ExtensionHostApiHandles,
    limits: StoreLimits,
}

impl ExtensionComponentRuntime {
    pub(crate) fn new() -> Result<Self, ExtensionRuntimeError> {
        let mut config = Config::new();
        config.consume_fuel(true);
        config.epoch_interruption(true);
        let engine = Engine::new(&config).map_err(|_| ExtensionRuntimeError::InvalidComponent)?;
        Ok(Self {
            engine,
            components: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    pub(crate) fn prepare(
        &self,
        package: &ValidatedPackageDirectory,
    ) -> Result<(), ExtensionRuntimeError> {
        let digest = package.package_digest();
        if self
            .components
            .lock()
            .expect("extension component cache lock poisoned")
            .contains_key(digest)
        {
            return Ok(());
        }
        let component = Arc::new(
            Component::from_file(&self.engine, package.component_path())
                .map_err(|_| ExtensionRuntimeError::InvalidComponent)?,
        );
        self.components
            .lock()
            .expect("extension component cache lock poisoned")
            .entry(digest.into())
            .or_insert(component);
        Ok(())
    }

    pub(crate) async fn invoke(
        &self,
        package: &ValidatedPackageDirectory,
        handler: &ExtensionHandlerRef,
        operation_id: String,
        scope: ExtensionOperationScope,
        lease: OperationCapabilityLease,
        input: Vec<u8>,
    ) -> Result<ExtensionInvocationOutput, ExtensionRuntimeError> {
        if handler.extension_id != package.id()
            || handler.package_digest != package.package_digest()
            || handler.schema_revision != 1
        {
            return Err(ExtensionRuntimeError::IdentityMismatch);
        }
        lease
            .validate_operation(&operation_id, &scope)
            .map_err(map_grant_error)?;
        let limits = package.runtime_limits();
        let component = self
            .components
            .lock()
            .expect("extension component cache lock poisoned")
            .get(package.package_digest())
            .cloned()
            .ok_or(ExtensionRuntimeError::ComponentNotPrepared)?;
        let mut linker = Linker::new(&self.engine);
        bindings::Extension::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)
            .map_err(|_| ExtensionRuntimeError::InvalidComponent)?;

        let mut store = Store::new(
            &self.engine,
            InvocationState {
                operation_id,
                scope,
                host: ExtensionHostApiHandles::new(lease.clone()),
                lease,
                limits: StoreLimitsBuilder::new()
                    .memory_size(limits.memory_bytes as usize)
                    .build(),
            },
        );
        store.limiter(|state| &mut state.limits);
        store
            .set_fuel(limits.fuel)
            .map_err(|_| ExtensionRuntimeError::InvalidComponent)?;
        store.set_epoch_deadline(1);
        store.epoch_deadline_callback(|store| {
            let state = store.data();
            state
                .lease
                .validate_operation(&state.operation_id, &state.scope)
                .map_err(|error| wasmtime::Error::msg(error.to_string()))?;
            Ok(UpdateDeadline::Yield(1))
        });

        let ticker_engine = self.engine.clone();
        let ticker = tokio::spawn(async move {
            loop {
                tokio::time::sleep(EPOCH_INTERVAL).await;
                ticker_engine.increment_epoch();
            }
        });
        let result = tokio::time::timeout(Duration::from_millis(limits.deadline_ms), async {
            let request = bindings::pi::extension::types::Invocation {
                operation_id: store.data().operation_id.clone(),
                handler_id: handler.handler_id.clone(),
                input_schema_revision: handler.schema_revision,
                input,
            };
            let bindings =
                bindings::Extension::instantiate_async(&mut store, &component, &linker).await?;
            bindings
                .pi_extension_guest()
                .call_invoke(&mut store, &request)
                .await
        })
        .await;
        ticker.abort();

        let guest_result = match result {
            Err(_) => return Err(ExtensionRuntimeError::DeadlineExceeded),
            Ok(Err(_)) => {
                if let Err(error) = store
                    .data()
                    .lease
                    .validate_operation(&store.data().operation_id, &store.data().scope)
                {
                    return Err(map_grant_error(error));
                }
                return Err(ExtensionRuntimeError::Trapped);
            }
            Ok(Ok(result)) => result,
        };
        let output = guest_result.map_err(map_guest_error)?;
        if output.output_schema_revision != 1 || output.output.len() as u64 > limits.output_bytes {
            return Err(ExtensionRuntimeError::OutputLimit);
        }
        Ok(ExtensionInvocationOutput {
            schema_revision: output.output_schema_revision,
            output: output.output,
        })
    }
}

fn map_grant_error(error: ExtensionGrantError) -> ExtensionRuntimeError {
    match error {
        ExtensionGrantError::Cancelled | ExtensionGrantError::Revoked => {
            ExtensionRuntimeError::Cancelled
        }
        ExtensionGrantError::DeadlineExceeded => ExtensionRuntimeError::DeadlineExceeded,
        _ => ExtensionRuntimeError::Denied,
    }
}

fn map_guest_error(error: GuestExtensionError) -> ExtensionRuntimeError {
    match error {
        GuestExtensionError::Cancelled => ExtensionRuntimeError::Cancelled,
        GuestExtensionError::DeadlineExceeded => ExtensionRuntimeError::DeadlineExceeded,
        GuestExtensionError::OutputLimit => ExtensionRuntimeError::OutputLimit,
        GuestExtensionError::Denied(_) | GuestExtensionError::InvalidInput(_) => {
            ExtensionRuntimeError::Denied
        }
        GuestExtensionError::Internal(_) => ExtensionRuntimeError::Trapped,
    }
}

impl host_diagnostics::Host for InvocationState {
    async fn emit(
        &mut self,
        _level: host_diagnostics::Level,
        _code: String,
        _message: String,
    ) -> Result<(), String> {
        Ok(())
    }
}

impl host_ui::Host for InvocationState {
    async fn interact(
        &mut self,
        action_id: String,
        input: Vec<u8>,
    ) -> Result<Vec<u8>, GuestExtensionError> {
        match self
            .host
            .ui
            .authorize_interact(&self.operation_id, &self.scope, action_id, input)
        {
            Ok(AuthorizedHostCall::UiInteract { input, .. }) => Ok(input),
            Ok(_) => Err(GuestExtensionError::Internal(
                "host authorization mismatch".into(),
            )),
            Err(error) => Err(host_error(error)),
        }
    }
}

impl host_workspace::Host for InvocationState {
    async fn read_text(&mut self, _relative_path: String) -> Result<String, GuestExtensionError> {
        Err(GuestExtensionError::Denied(
            "workspace backend unavailable".into(),
        ))
    }

    async fn write_text(
        &mut self,
        _relative_path: String,
        _text: String,
    ) -> Result<(), GuestExtensionError> {
        Err(GuestExtensionError::Denied(
            "workspace backend unavailable".into(),
        ))
    }
}

impl host_model::Host for InvocationState {
    async fn invoke(&mut self, _prompt: String) -> Result<String, GuestExtensionError> {
        Err(GuestExtensionError::Denied(
            "model backend unavailable".into(),
        ))
    }
}

impl host_process::Host for InvocationState {
    async fn exec(
        &mut self,
        _program: String,
        _arguments: Vec<String>,
    ) -> Result<host_process::Output, GuestExtensionError> {
        Err(GuestExtensionError::Denied(
            "process backend unavailable".into(),
        ))
    }
}

impl bindings::pi::extension::types::Host for InvocationState {}

fn host_error(error: super::host::ExtensionHostCallError) -> GuestExtensionError {
    match error {
        super::host::ExtensionHostCallError::Grant(ExtensionGrantError::Cancelled)
        | super::host::ExtensionHostCallError::Grant(ExtensionGrantError::Revoked) => {
            GuestExtensionError::Cancelled
        }
        super::host::ExtensionHostCallError::Grant(ExtensionGrantError::DeadlineExceeded) => {
            GuestExtensionError::DeadlineExceeded
        }
        super::host::ExtensionHostCallError::Grant(_) => {
            GuestExtensionError::Denied("capability denied".into())
        }
        super::host::ExtensionHostCallError::InvalidInput(_) => {
            GuestExtensionError::InvalidInput("invalid Host API input".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::time::{Duration, Instant};

    use sha2::{Digest, Sha256};
    use tempfile::TempDir;
    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::extensions::grant::{
        ExtensionGrantIdentity, ExtensionGrantRegistry, ExtensionGrantScope, ExtensionPermission,
        ExtensionSourceChannel, ExtensionTrustLevel, GrantRecord,
    };
    use crate::extensions::store::ExtensionPackageStore;

    #[test]
    fn component_runtime_enables_required_isolation_controls() {
        ExtensionComponentRuntime::new().unwrap();
    }

    #[tokio::test]
    async fn typescript_component_invokes_through_lease_backed_host() {
        let Ok(component_fixture) = std::env::var("PI_RUST_EXTENSION_COMPONENT_FIXTURE") else {
            return;
        };
        let directory = TempDir::new().unwrap();
        let store = ExtensionPackageStore::open(directory.path().join("store")).unwrap();
        let staging = store.create_staging_directory().unwrap();
        let component = fs::read(component_fixture).unwrap();
        fs::write(staging.join("component.wasm"), &component).unwrap();
        fs::write(
            staging.join("extension.json"),
            serde_json::json!({
                "schemaVersion": 2,
                "id": "fixture.echo",
                "version": "1.0.0",
                "api": { "requires": "^0.1" },
                "component": {
                    "path": "component.wasm",
                    "sha256": format!("{:x}", Sha256::digest(&component)),
                    "world": "pi:extension/extension@0.1.0"
                },
                "lock": "extension.lock.json",
                "activation": ["workspace"],
                "permissions": ["ui.interact"],
                "contributions": {
                    "tools": [{
                        "id": "fixture.echo",
                        "handler": "fixture.echo",
                        "schemaRevision": 1,
                        "definition": { "description": "runtime fixture" }
                    }]
                },
                "resources": [],
                "limits": {
                    "memoryBytes": 268435456,
                    "fuel": 1000000000,
                    "deadlineMs": 30000,
                    "outputBytes": 1048576
                }
            })
            .to_string(),
        )
        .unwrap();
        fs::write(
            staging.join("extension.lock.json"),
            serde_json::json!({
                "schemaVersion": 1,
                "extension": { "id": "fixture.echo", "version": "1.0.0" },
                "dependencies": []
            })
            .to_string(),
        )
        .unwrap();
        let installed = store.install_staged(staging).unwrap();
        let package = store.load_validated(installed.package_digest()).unwrap();
        let handler = package.handler_refs().unwrap().remove(0).dispatch(
            |_| panic!("extension fixture projected a core handler"),
            Clone::clone,
        );

        let runtime = ExtensionComponentRuntime::new().unwrap();
        runtime.prepare(&package).unwrap();
        let registry = ExtensionGrantRegistry::default();
        registry
            .install(
                GrantRecord::new(
                    ExtensionGrantIdentity {
                        id: installed.id().into(),
                        version: installed.version().into(),
                        package_digest: installed.package_digest().into(),
                    },
                    ExtensionSourceChannel::Registry,
                    "c".repeat(64),
                    ExtensionTrustLevel::Verified,
                    ExtensionGrantScope {
                        workspace_id: "workspace-1".into(),
                        session_ids: BTreeSet::new(),
                    },
                    ["ui.interact"],
                    [ExtensionPermission::UiInteract],
                    "pi:extension/extension@0.1.0".into(),
                )
                .unwrap(),
            )
            .unwrap();
        let scope = ExtensionOperationScope {
            workspace_id: "workspace-1".into(),
            session_id: None,
        };
        let lease = registry
            .admit(
                "workspace-1",
                "fixture.echo",
                "operation-1".into(),
                scope.clone(),
                Instant::now() + Duration::from_secs(30),
                CancellationToken::new(),
            )
            .unwrap();
        let input = br#"{"message":"hello"}"#.to_vec();

        let output = runtime
            .invoke(
                &package,
                &handler,
                "operation-1".into(),
                scope,
                lease,
                input.clone(),
            )
            .await
            .unwrap();

        assert_eq!(output.schema_revision, 1);
        let json: serde_json::Value = serde_json::from_slice(&output.output).unwrap();
        assert_eq!(json["handlerId"], "fixture.echo");
        assert_eq!(json["inputBytes"], input.len());
        assert_eq!(json["hostEcho"], String::from_utf8(input).unwrap());
    }
}
