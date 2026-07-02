#![allow(dead_code)]

use std::future::Future;
use std::pin::Pin;

use pi_agent_core::flow::{Action, Flow, FlowError, FlowNode, FlowOutcome, FlowRunOptions};

use super::CodingSessionError;
use super::plugin_service::{PluginDiagnostic, PluginService};
use crate::plugins::{PluginCapabilities, PluginRegistry, PluginSource};

const DEFAULT_ACTION: &str = "default";

pub(crate) const PLUGIN_LOAD_NODE_IDS: &[&str] = &[
    "start_plugin_load",
    "discover_plugins",
    "validate_manifests",
    "load_first_party_plugins",
    "load_lua_plugins_later",
    "register_capabilities",
    "emit_diagnostics",
    "finalize_plugin_load",
];

const PLUGIN_LOAD_NODE_SPECS: &[PluginLoadNodeSpec] = &[
    PluginLoadNodeSpec {
        id: "start_plugin_load",
        name: "StartPluginLoad",
        kind: PluginLoadNodeKind::StartPluginLoad,
    },
    PluginLoadNodeSpec {
        id: "discover_plugins",
        name: "DiscoverPlugins",
        kind: PluginLoadNodeKind::DiscoverPlugins,
    },
    PluginLoadNodeSpec {
        id: "validate_manifests",
        name: "ValidateManifests",
        kind: PluginLoadNodeKind::ValidateManifests,
    },
    PluginLoadNodeSpec {
        id: "load_first_party_plugins",
        name: "LoadFirstPartyPlugins",
        kind: PluginLoadNodeKind::LoadFirstPartyPlugins,
    },
    PluginLoadNodeSpec {
        id: "load_lua_plugins_later",
        name: "LoadLuaPluginsLater",
        kind: PluginLoadNodeKind::LoadLuaPluginsLater,
    },
    PluginLoadNodeSpec {
        id: "register_capabilities",
        name: "RegisterCapabilities",
        kind: PluginLoadNodeKind::RegisterCapabilities,
    },
    PluginLoadNodeSpec {
        id: "emit_diagnostics",
        name: "EmitDiagnostics",
        kind: PluginLoadNodeKind::EmitDiagnostics,
    },
    PluginLoadNodeSpec {
        id: "finalize_plugin_load",
        name: "FinalizePluginLoad",
        kind: PluginLoadNodeKind::FinalizePluginLoad,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PluginLoadNodeSpec {
    id: &'static str,
    name: &'static str,
    kind: PluginLoadNodeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PluginLoadNodeKind {
    StartPluginLoad,
    DiscoverPlugins,
    ValidateManifests,
    LoadFirstPartyPlugins,
    LoadLuaPluginsLater,
    RegisterCapabilities,
    EmitDiagnostics,
    FinalizePluginLoad,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PluginLoadManifest {
    id: String,
    name: String,
    version: String,
    source: PluginSource,
}

impl PluginLoadManifest {
    pub(crate) fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
        source: PluginSource,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            source,
        }
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.id.trim().is_empty() {
            errors.push("plugin id must not be empty".to_owned());
        }
        if self.name.trim().is_empty() {
            errors.push("plugin name must not be empty".to_owned());
        }
        if self.version.trim().is_empty() {
            errors.push("plugin version must not be empty".to_owned());
        }
        errors
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PluginLoadCandidate {
    manifest: PluginLoadManifest,
    registry: PluginRegistry,
}

impl PluginLoadCandidate {
    pub(crate) fn new(manifest: PluginLoadManifest, registry: PluginRegistry) -> Self {
        Self { manifest, registry }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct PluginLoadOptions {
    candidates: Vec<PluginLoadCandidate>,
}

impl PluginLoadOptions {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_candidate(mut self, candidate: PluginLoadCandidate) -> Self {
        self.candidates.push(candidate);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PluginLoadOutcome {
    pub(crate) loaded_plugin_ids: Vec<String>,
    pub(crate) diagnostics: Vec<PluginDiagnostic>,
    pub(crate) capabilities: PluginCapabilities,
    pub(crate) capability_changed: bool,
}

pub(crate) struct PluginLoadContext {
    options: PluginLoadOptions,
    discovered: Vec<PluginLoadCandidate>,
    validated: Vec<PluginLoadCandidate>,
    loaded_plugin_ids: Vec<String>,
    diagnostics: Vec<PluginDiagnostic>,
    loaded_plugin_service: Option<PluginService>,
    outcome: Option<PluginLoadOutcome>,
    failure_error: Option<CodingSessionError>,
}

impl PluginLoadContext {
    pub(crate) fn new(options: PluginLoadOptions) -> Self {
        Self {
            options,
            discovered: Vec::new(),
            validated: Vec::new(),
            loaded_plugin_ids: Vec::new(),
            diagnostics: Vec::new(),
            loaded_plugin_service: None,
            outcome: None,
            failure_error: None,
        }
    }

    pub(crate) fn outcome(&self) -> Option<&PluginLoadOutcome> {
        self.outcome.as_ref()
    }

    pub(crate) fn loaded_plugin_service(&self) -> Option<&PluginService> {
        self.loaded_plugin_service.as_ref()
    }

    pub(crate) fn take_loaded_plugin_service(&mut self) -> Option<PluginService> {
        self.loaded_plugin_service.take()
    }

    pub(crate) fn take_failure_error(&mut self) -> Option<CodingSessionError> {
        self.failure_error.take()
    }

    pub(crate) fn finish_success(&self) -> Result<PluginLoadOutcome, CodingSessionError> {
        self.outcome
            .clone()
            .ok_or_else(|| CodingSessionError::Session {
                message: "plugin load cannot finish without an outcome".into(),
            })
    }

    fn fail(&mut self, error: CodingSessionError) -> String {
        let message = error.to_string();
        self.failure_error = Some(error);
        message
    }

    fn start_plugin_load(&mut self) -> Result<(), CodingSessionError> {
        if self.outcome.is_some() {
            return Err(CodingSessionError::Session {
                message: "plugin load has already finalized".into(),
            });
        }
        Ok(())
    }

    fn discover_plugins(&mut self) -> Result<(), CodingSessionError> {
        if !self.discovered.is_empty() {
            return Ok(());
        }
        self.discovered = std::mem::take(&mut self.options.candidates);
        Ok(())
    }

    fn validate_manifests(&mut self) -> Result<(), CodingSessionError> {
        if !self.validated.is_empty() || self.discovered.is_empty() {
            return Ok(());
        }
        for candidate in self.discovered.iter().cloned() {
            if candidate.manifest.source == PluginSource::Lua {
                self.diagnostics.push(PluginDiagnostic {
                    plugin_id: Some(candidate.manifest.id().to_owned()),
                    message: "Lua plugin loading is not implemented yet".to_owned(),
                });
                continue;
            }
            let errors = candidate.manifest.validate();
            if errors.is_empty() {
                self.validated.push(candidate);
            } else {
                self.diagnostics.push(PluginDiagnostic {
                    plugin_id: Some(candidate.manifest.id().to_owned()),
                    message: errors.join("; "),
                });
            }
        }
        Ok(())
    }

    fn load_first_party_plugins(&mut self) -> Result<(), CodingSessionError> {
        Ok(())
    }

    fn load_lua_plugins_later(&mut self) -> Result<(), CodingSessionError> {
        Ok(())
    }

    fn register_capabilities(&mut self) -> Result<(), CodingSessionError> {
        if self.loaded_plugin_service.is_some() {
            return Ok(());
        }
        let mut registry = PluginRegistry::new();
        for candidate in self.validated.iter().cloned() {
            self.loaded_plugin_ids
                .push(candidate.manifest.id().to_owned());
            registry.extend(candidate.registry);
        }
        self.loaded_plugin_service = Some(PluginService::with_registry(registry));
        Ok(())
    }

    fn emit_diagnostics(&mut self) -> Result<(), CodingSessionError> {
        Ok(())
    }

    fn finalize_plugin_load(&mut self) -> Result<(), CodingSessionError> {
        if self.outcome.is_some() {
            return Ok(());
        }
        let service =
            self.loaded_plugin_service
                .as_ref()
                .ok_or_else(|| CodingSessionError::Session {
                    message: "plugin load cannot finalize before registering capabilities".into(),
                })?;
        let mut capabilities = service.capabilities();
        capabilities.diagnostics += self.diagnostics.len();
        self.outcome = Some(PluginLoadOutcome {
            loaded_plugin_ids: self.loaded_plugin_ids.clone(),
            diagnostics: self.diagnostics.clone(),
            capability_changed: capabilities != PluginCapabilities::new(),
            capabilities,
        });
        Ok(())
    }
}

pub(crate) struct PluginLoadFlow {
    flow: Flow<PluginLoadContext>,
}

impl PluginLoadFlow {
    pub(crate) fn new() -> Result<Self, CodingSessionError> {
        let mut flow = Flow::new(PLUGIN_LOAD_NODE_IDS[0]).map_err(flow_error)?;
        for spec in PLUGIN_LOAD_NODE_SPECS {
            flow.add_node(spec.id, PluginLoadNode::new(spec.name, spec.kind))
                .map_err(flow_error)?;
        }
        for pair in PLUGIN_LOAD_NODE_IDS.windows(2) {
            flow.edge(pair[0], pair[1]).map_err(flow_error)?;
        }
        Ok(Self { flow })
    }

    pub(crate) fn node_ids() -> &'static [&'static str] {
        PLUGIN_LOAD_NODE_IDS
    }

    pub(crate) async fn run(
        &self,
        ctx: &mut PluginLoadContext,
    ) -> Result<FlowOutcome, CodingSessionError> {
        self.flow.run(ctx).await.map_err(flow_error)
    }

    pub(crate) async fn run_with_options(
        &self,
        ctx: &mut PluginLoadContext,
        options: FlowRunOptions,
    ) -> Result<FlowOutcome, CodingSessionError> {
        self.flow
            .run_with_options(ctx, options)
            .await
            .map_err(flow_error)
    }
}

#[derive(Debug, Clone, Copy)]
struct PluginLoadNode {
    name: &'static str,
    kind: PluginLoadNodeKind,
}

impl PluginLoadNode {
    fn new(name: &'static str, kind: PluginLoadNodeKind) -> Self {
        Self { name, kind }
    }
}

impl FlowNode<PluginLoadContext> for PluginLoadNode {
    fn name(&self) -> &str {
        self.name
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut PluginLoadContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move {
            let result = match self.kind {
                PluginLoadNodeKind::StartPluginLoad => ctx.start_plugin_load(),
                PluginLoadNodeKind::DiscoverPlugins => ctx.discover_plugins(),
                PluginLoadNodeKind::ValidateManifests => ctx.validate_manifests(),
                PluginLoadNodeKind::LoadFirstPartyPlugins => ctx.load_first_party_plugins(),
                PluginLoadNodeKind::LoadLuaPluginsLater => ctx.load_lua_plugins_later(),
                PluginLoadNodeKind::RegisterCapabilities => ctx.register_capabilities(),
                PluginLoadNodeKind::EmitDiagnostics => ctx.emit_diagnostics(),
                PluginLoadNodeKind::FinalizePluginLoad => ctx.finalize_plugin_load(),
            };
            match result {
                Ok(()) => default_action(),
                Err(error) => Err(ctx.fail(error)),
            }
        })
    }
}

fn default_action() -> Result<Action, String> {
    Action::new(DEFAULT_ACTION).map_err(|error| error.to_string())
}

fn flow_error(error: FlowError) -> CodingSessionError {
    CodingSessionError::Flow {
        message: error.to_string(),
    }
}
