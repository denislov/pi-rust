#![allow(dead_code)]

use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use pi_agent_core::flow::{Action, Flow, FlowError, FlowNode, FlowOutcome, FlowRunOptions};
use serde::Deserialize;

use super::CodingSessionError;
use super::plugin_service::{PluginDiagnostic, PluginService};
use crate::plugins::{PluginCapabilities, PluginRegistry, PluginSource};

const DEFAULT_ACTION: &str = "default";
const PLUGIN_MANIFEST_FILE: &str = "plugin.toml";

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

#[derive(Debug, Clone)]
pub(crate) struct PluginDiscoveryRoot {
    path: PathBuf,
    source: PluginSource,
}

impl PluginDiscoveryRoot {
    pub(crate) fn new(path: impl Into<PathBuf>, source: PluginSource) -> Self {
        Self {
            path: path.into(),
            source,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct PluginLoadOptions {
    candidates: Vec<PluginLoadCandidate>,
    discovery_roots: Vec<PluginDiscoveryRoot>,
}

impl PluginLoadOptions {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_candidate(mut self, candidate: PluginLoadCandidate) -> Self {
        self.candidates.push(candidate);
        self
    }

    pub(crate) fn with_discovery_root(
        mut self,
        path: impl Into<PathBuf>,
        source: PluginSource,
    ) -> Self {
        self.discovery_roots
            .push(PluginDiscoveryRoot::new(path, source));
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
    discovery_complete: bool,
    validation_complete: bool,
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
            discovery_complete: false,
            validation_complete: false,
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
        if self.discovery_complete {
            return Ok(());
        }
        self.discovered
            .extend(std::mem::take(&mut self.options.candidates));
        for root in std::mem::take(&mut self.options.discovery_roots) {
            self.discover_plugin_root(root)?;
        }
        self.discovery_complete = true;
        Ok(())
    }

    fn discover_plugin_root(
        &mut self,
        root: PluginDiscoveryRoot,
    ) -> Result<(), CodingSessionError> {
        let manifest_paths = match discover_manifest_paths(&root.path) {
            Ok(paths) => paths,
            Err(message) => {
                self.diagnostics.push(PluginDiagnostic {
                    plugin_id: None,
                    message,
                });
                return Ok(());
            }
        };
        for manifest_path in manifest_paths {
            match read_manifest_candidate(&manifest_path, &root.source) {
                Ok(candidate) => self.discovered.push(candidate),
                Err(diagnostic) => self.diagnostics.push(diagnostic),
            }
        }
        Ok(())
    }

    fn validate_manifests(&mut self) -> Result<(), CodingSessionError> {
        if self.validation_complete {
            return Ok(());
        }
        for candidate in self.discovered.iter().cloned() {
            let errors = candidate.manifest.validate();
            if !errors.is_empty() {
                self.diagnostics.push(PluginDiagnostic {
                    plugin_id: Some(candidate.manifest.id().to_owned()),
                    message: errors.join("; "),
                });
                continue;
            }
            if candidate.manifest.source == PluginSource::Lua {
                self.diagnostics.push(PluginDiagnostic {
                    plugin_id: Some(candidate.manifest.id().to_owned()),
                    message: "Lua plugin loading is not implemented yet".to_owned(),
                });
                continue;
            }
            self.validated.push(candidate);
        }
        self.validation_complete = true;
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

#[derive(Debug, Deserialize)]
struct PluginManifestFile {
    id: Option<String>,
    name: Option<String>,
    version: Option<String>,
    runtime: Option<String>,
    source: Option<String>,
}

impl PluginManifestFile {
    fn into_manifest(self, root_source: &PluginSource) -> Result<PluginLoadManifest, String> {
        let source = self.plugin_source(root_source)?;
        Ok(PluginLoadManifest::new(
            self.id.unwrap_or_default(),
            self.name.unwrap_or_default(),
            self.version.unwrap_or_default(),
            source,
        ))
    }

    fn plugin_source(&self, root_source: &PluginSource) -> Result<PluginSource, String> {
        let declared = self
            .runtime
            .as_deref()
            .or(self.source.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let Some(declared) = declared else {
            return Ok(root_source.clone());
        };
        match declared {
            "first-party" | "first_party" => Ok(PluginSource::FirstParty),
            "project" => Ok(PluginSource::Project),
            "user" => Ok(PluginSource::User),
            "lua" => Ok(PluginSource::Lua),
            other => Err(format!("unsupported plugin runtime/source: {other}")),
        }
    }
}

fn discover_manifest_paths(root: &Path) -> Result<Vec<PathBuf>, String> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let metadata = fs::metadata(root).map_err(|error| {
        format!(
            "failed to inspect plugin directory {}: {error}",
            root.display()
        )
    })?;
    if metadata.is_file() {
        return Ok(
            if root.file_name().and_then(|name| name.to_str()) == Some(PLUGIN_MANIFEST_FILE) {
                vec![root.to_path_buf()]
            } else {
                Vec::new()
            },
        );
    }

    let mut paths = Vec::new();
    let root_manifest = root.join(PLUGIN_MANIFEST_FILE);
    if root_manifest.is_file() {
        paths.push(root_manifest);
    }
    let entries = fs::read_dir(root).map_err(|error| {
        format!(
            "failed to read plugin directory {}: {error}",
            root.display()
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "failed to read plugin directory entry under {}: {error}",
                root.display()
            )
        })?;
        let path = entry.path();
        if path.is_dir() {
            let manifest = path.join(PLUGIN_MANIFEST_FILE);
            if manifest.is_file() {
                paths.push(manifest);
            }
        }
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn read_manifest_candidate(
    manifest_path: &Path,
    root_source: &PluginSource,
) -> Result<PluginLoadCandidate, PluginDiagnostic> {
    let content = fs::read_to_string(manifest_path).map_err(|error| PluginDiagnostic {
        plugin_id: None,
        message: format!(
            "failed to read plugin manifest {}: {error}",
            manifest_path.display()
        ),
    })?;
    let manifest_file =
        toml::from_str::<PluginManifestFile>(&content).map_err(|error| PluginDiagnostic {
            plugin_id: None,
            message: format!(
                "failed to parse plugin manifest {}: {error}",
                manifest_path.display()
            ),
        })?;
    let plugin_id = manifest_file.id.clone();
    let manifest = manifest_file
        .into_manifest(root_source)
        .map_err(|message| PluginDiagnostic { plugin_id, message })?;
    Ok(PluginLoadCandidate::new(manifest, PluginRegistry::new()))
}

fn default_action() -> Result<Action, String> {
    Action::new(DEFAULT_ACTION).map_err(|error| error.to_string())
}

fn flow_error(error: FlowError) -> CodingSessionError {
    CodingSessionError::Flow {
        message: error.to_string(),
    }
}
