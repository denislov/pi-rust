#![allow(dead_code)]

use std::fs;
use std::future::Future;
use std::path::{Component, Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use mlua::{Function, Lua, LuaSerdeExt, Table, Value, Variadic};
use pi_agent_core::AgentTool;
use pi_agent_core::flow::{Action, Flow, FlowError, FlowNode, FlowOutcome, FlowRunOptions};
use serde::Deserialize;

use super::CodingSessionError;
use super::plugin_service::{PluginDiagnostic, PluginService};
use crate::plugins::{
    PluginCapabilities, PluginError, PluginId, PluginMetadata, PluginRegistry, PluginSource,
    ToolProvider, ToolRegistrationHost,
};

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
    entry_path: Option<PathBuf>,
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
            entry_path: None,
        }
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    fn with_entry_path(mut self, entry_path: PathBuf) -> Self {
        self.entry_path = Some(entry_path);
        self
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
            if candidate.manifest.source == PluginSource::Lua
                && candidate.manifest.entry_path.is_none()
            {
                self.diagnostics.push(PluginDiagnostic {
                    plugin_id: Some(candidate.manifest.id().to_owned()),
                    message: "Lua plugin entry is required".to_owned(),
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
        let mut loaded = Vec::new();
        for mut candidate in std::mem::take(&mut self.validated) {
            if candidate.manifest.source == PluginSource::Lua {
                match load_lua_plugin_registry(&candidate.manifest) {
                    Ok(registry) => {
                        candidate.registry = registry;
                        loaded.push(candidate);
                    }
                    Err(diagnostic) => self.diagnostics.push(diagnostic),
                }
            } else {
                loaded.push(candidate);
            }
        }
        self.validated = loaded;
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
    entry: Option<String>,
}

impl PluginManifestFile {
    fn into_manifest(
        self,
        root_source: &PluginSource,
        manifest_dir: &Path,
    ) -> Result<PluginLoadManifest, String> {
        let source = self.plugin_source(root_source)?;
        let mut manifest = PluginLoadManifest::new(
            self.id.unwrap_or_default(),
            self.name.unwrap_or_default(),
            self.version.unwrap_or_default(),
            source,
        );
        if let Some(entry) = self
            .entry
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let entry_path = validate_plugin_entry_path(entry)?;
            manifest = manifest.with_entry_path(manifest_dir.join(entry_path));
        }
        Ok(manifest)
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
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let manifest = manifest_file
        .into_manifest(root_source, manifest_dir)
        .map_err(|message| PluginDiagnostic { plugin_id, message })?;
    Ok(PluginLoadCandidate::new(manifest, PluginRegistry::new()))
}

fn validate_plugin_entry_path(entry: &str) -> Result<PathBuf, String> {
    let path = Path::new(entry);
    if path.is_absolute() {
        return Err("plugin entry must be a relative path".to_owned());
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err("plugin entry must stay inside the plugin directory".to_owned());
    }
    Ok(path.to_path_buf())
}

#[derive(Debug, Clone)]
struct LuaToolSpec {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

struct LuaToolProvider {
    metadata: PluginMetadata,
    entry_path: PathBuf,
    source: Arc<str>,
    tools: Vec<LuaToolSpec>,
}

impl ToolProvider for LuaToolProvider {
    fn metadata(&self) -> PluginMetadata {
        self.metadata.clone()
    }

    fn tools(&self, _host: &ToolRegistrationHost) -> Result<Vec<AgentTool>, PluginError> {
        let mut tools = Vec::new();
        for spec in self.tools.iter().cloned() {
            let plugin_id = self.metadata.id.as_str().to_owned();
            let entry_path = self.entry_path.clone();
            let source = self.source.clone();
            let tool_name = spec.name.clone();
            let tool = AgentTool::new_text(
                spec.name,
                spec.description,
                spec.input_schema,
                move |args| {
                    let plugin_id = plugin_id.clone();
                    let entry_path = entry_path.clone();
                    let source = source.clone();
                    let tool_name = tool_name.clone();
                    async move { run_lua_tool(&plugin_id, &entry_path, &source, &tool_name, args) }
                },
            );
            tool.validate().map_err(|error| PluginError::Registration {
                plugin_id: self.metadata.id.as_str().to_owned(),
                message: error.to_string(),
            })?;
            tools.push(tool);
        }
        Ok(tools)
    }
}

fn load_lua_plugin_registry(
    manifest: &PluginLoadManifest,
) -> Result<PluginRegistry, PluginDiagnostic> {
    let plugin_id = Some(manifest.id().to_owned());
    let entry_path = manifest
        .entry_path
        .as_ref()
        .ok_or_else(|| PluginDiagnostic {
            plugin_id: plugin_id.clone(),
            message: "Lua plugin entry is required".to_owned(),
        })?;
    let source = fs::read_to_string(entry_path).map_err(|error| PluginDiagnostic {
        plugin_id: plugin_id.clone(),
        message: format!(
            "failed to read Lua plugin entry {}: {error}",
            entry_path.display()
        ),
    })?;
    let tools =
        collect_lua_tool_specs(entry_path, &source).map_err(|message| PluginDiagnostic {
            plugin_id: plugin_id.clone(),
            message,
        })?;
    let mut registry = PluginRegistry::new();
    registry.register_tool_provider(Arc::new(LuaToolProvider {
        metadata: PluginMetadata::new(
            PluginId::new(manifest.id.clone()),
            manifest.name.clone(),
            manifest.version.clone(),
            PluginSource::Lua,
        ),
        entry_path: entry_path.clone(),
        source: Arc::from(source),
        tools,
    }));
    Ok(registry)
}

fn collect_lua_tool_specs(entry_path: &Path, source: &str) -> Result<Vec<LuaToolSpec>, String> {
    let lua = create_lua().map_err(|error| format!("failed to create Lua runtime: {error}"))?;
    let tools = Arc::new(Mutex::new(Vec::new()));
    let host = lua
        .create_table()
        .map_err(|error| format!("failed to create Lua plugin host: {error}"))?;
    let tools_for_host = Arc::clone(&tools);
    let tool_fn = lua
        .create_function(move |lua, args: Variadic<Value>| {
            let table = lua_tool_definition_table(args)?;
            let spec = lua_tool_spec_from_table(lua, table)?;
            tools_for_host
                .lock()
                .map_err(|_| mlua::Error::external("Lua tool registry lock poisoned"))?
                .push(spec);
            Ok(())
        })
        .map_err(|error| format!("failed to create Lua tool host: {error}"))?;
    host.set("tool", tool_fn)
        .map_err(|error| format!("failed to install Lua tool host: {error}"))?;
    lua.load(source)
        .set_name(entry_path.display().to_string())
        .exec()
        .map_err(|error| {
            format!(
                "failed to execute Lua plugin entry {}: {error}",
                entry_path.display()
            )
        })?;
    let register: Function = lua
        .globals()
        .get("register")
        .map_err(|error| format!("Lua plugin entry must define register(host): {error}"))?;
    register
        .call::<()>(host)
        .map_err(|error| format!("Lua plugin register(host) failed: {error}"))?;
    let collected = tools
        .lock()
        .map_err(|_| "Lua tool registry lock poisoned".to_owned())?
        .clone();
    Ok(collected)
}

fn run_lua_tool(
    plugin_id: &str,
    entry_path: &Path,
    source: &str,
    tool_name: &str,
    args: serde_json::Value,
) -> Result<String, String> {
    let lua = create_lua().map_err(|error| format!("failed to create Lua runtime: {error}"))?;
    let run_key = Arc::new(Mutex::new(None));
    let host = lua
        .create_table()
        .map_err(|error| format!("failed to create Lua plugin host: {error}"))?;
    let target_name = tool_name.to_owned();
    let run_key_for_host = Arc::clone(&run_key);
    let tool_fn = lua
        .create_function(move |lua, args: Variadic<Value>| {
            let table = lua_tool_definition_table(args)?;
            let name = required_lua_string(&table, "name")?;
            if name == target_name {
                let run: Function = table.get("run")?;
                let key = lua.create_registry_value(run)?;
                *run_key_for_host
                    .lock()
                    .map_err(|_| mlua::Error::external("Lua tool registry lock poisoned"))? =
                    Some(key);
            }
            Ok(())
        })
        .map_err(|error| format!("failed to create Lua tool host: {error}"))?;
    host.set("tool", tool_fn)
        .map_err(|error| format!("failed to install Lua tool host: {error}"))?;
    lua.load(source)
        .set_name(entry_path.display().to_string())
        .exec()
        .map_err(|error| {
            format!(
                "failed to execute Lua plugin entry {}: {error}",
                entry_path.display()
            )
        })?;
    let register: Function = lua
        .globals()
        .get("register")
        .map_err(|error| format!("Lua plugin entry must define register(host): {error}"))?;
    register
        .call::<()>(host)
        .map_err(|error| format!("Lua plugin register(host) failed: {error}"))?;
    let key = run_key
        .lock()
        .map_err(|_| "Lua tool registry lock poisoned".to_owned())?
        .take()
        .ok_or_else(|| format!("Lua plugin {plugin_id} did not register tool {tool_name}"))?;
    let run: Function = lua
        .registry_value(&key)
        .map_err(|error| format!("failed to resolve Lua tool {tool_name}: {error}"))?;
    let lua_args = lua.to_value(&args).map_err(|error| {
        format!("failed to convert tool input for Lua tool {tool_name}: {error}")
    })?;
    let output: Value = run
        .call(lua_args)
        .map_err(|error| format!("Lua tool {tool_name} failed: {error}"))?;
    let text = lua_tool_output_text(output)
        .map_err(|error| format!("Lua tool {tool_name} returned invalid output: {error}"))?;
    lua.remove_registry_value(key)
        .map_err(|error| format!("failed to release Lua tool {tool_name}: {error}"))?;
    Ok(text)
}

fn create_lua() -> Result<Lua, mlua::Error> {
    Lua::new_with(
        mlua::StdLib::TABLE | mlua::StdLib::STRING | mlua::StdLib::MATH | mlua::StdLib::UTF8,
        mlua::LuaOptions::default(),
    )
}

fn lua_tool_definition_table(args: Variadic<Value>) -> mlua::Result<Table> {
    args.into_iter()
        .rev()
        .find_map(|value| match value {
            Value::Table(table) => Some(table),
            _ => None,
        })
        .ok_or_else(|| mlua::Error::external("host.tool requires a tool definition table"))
}

fn lua_tool_spec_from_table(lua: &Lua, table: Table) -> mlua::Result<LuaToolSpec> {
    let name = required_lua_string(&table, "name")?;
    let description = required_lua_string(&table, "description")?;
    let schema_value: Value = table
        .get("input_schema")
        .or_else(|_| table.get("parameters"))?;
    let input_schema: serde_json::Value = lua.from_value(schema_value)?;
    if !input_schema.is_object() {
        return Err(mlua::Error::external(
            "Lua tool input_schema must be a JSON object",
        ));
    }
    let _: Function = table.get("run")?;
    Ok(LuaToolSpec {
        name,
        description,
        input_schema,
    })
}

fn required_lua_string(table: &Table, field: &str) -> mlua::Result<String> {
    let value: String = table.get(field)?;
    if value.trim().is_empty() {
        return Err(mlua::Error::external(format!(
            "Lua tool {field} must not be empty"
        )));
    }
    Ok(value)
}

fn lua_tool_output_text(value: Value) -> mlua::Result<String> {
    match value {
        Value::String(text) => Ok(text.to_str()?.to_owned()),
        Value::Table(table) => {
            let content: String = table.get("content")?;
            Ok(content)
        }
        other => Err(mlua::Error::external(format!(
            "expected string or table output, got {}",
            other.type_name()
        ))),
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
