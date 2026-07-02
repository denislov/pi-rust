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
    CommandDefinition, CommandProvider, CommandRegistrationHost, HookDiagnostic, HookFailurePolicy,
    HookOutcome, HookProvider, HookRegistration, HookRegistrationHost, KeybindDefinition,
    KeybindProvider, KeybindRegistrationHost, PluginCapabilities, PluginError, PluginId,
    PluginMetadata, PluginRegistry, PluginSource, PromptHookContext, PromptHookPoint, ToolProvider,
    ToolRegistrationHost, UiActionDefinition, UiProvider, UiRegistrationHost,
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

#[derive(Debug, Clone)]
struct LuaCommandSpec {
    id: String,
    description: String,
}

#[derive(Debug, Clone)]
struct LuaHookSpec {
    index: usize,
    point: PromptHookPoint,
    policy: HookFailurePolicy,
}

#[derive(Debug, Clone)]
struct LuaUiActionSpec {
    id: String,
    label: String,
    description: String,
    action_id: String,
}

#[derive(Debug, Clone)]
struct LuaKeybindSpec {
    id: String,
    key: String,
    description: String,
    action_id: String,
}

#[derive(Debug, Clone, Default)]
struct LuaPluginSpecs {
    tools: Vec<LuaToolSpec>,
    commands: Vec<LuaCommandSpec>,
    hooks: Vec<LuaHookSpec>,
    ui_actions: Vec<LuaUiActionSpec>,
    keybindings: Vec<LuaKeybindSpec>,
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

struct LuaCommandProvider {
    metadata: PluginMetadata,
    entry_path: PathBuf,
    source: Arc<str>,
    commands: Vec<LuaCommandSpec>,
}

impl CommandProvider for LuaCommandProvider {
    fn metadata(&self) -> PluginMetadata {
        self.metadata.clone()
    }

    fn commands(
        &self,
        _host: &CommandRegistrationHost,
    ) -> Result<Vec<CommandDefinition>, PluginError> {
        Ok(self
            .commands
            .iter()
            .cloned()
            .map(|spec| CommandDefinition::new(spec.id, spec.description))
            .collect())
    }

    fn run_command(
        &self,
        command_id: &str,
        args: serde_json::Value,
    ) -> Result<String, PluginError> {
        run_lua_command(
            self.metadata.id.as_str(),
            &self.entry_path,
            &self.source,
            command_id,
            args,
        )
        .map_err(|message| PluginError::Execution {
            plugin_id: self.metadata.id.as_str().to_owned(),
            message,
        })
    }
}

struct LuaHookProvider {
    metadata: PluginMetadata,
    entry_path: PathBuf,
    source: Arc<str>,
    hook: LuaHookSpec,
}

impl HookProvider for LuaHookProvider {
    fn metadata(&self) -> PluginMetadata {
        self.metadata.clone()
    }

    fn hooks(&self, _host: &HookRegistrationHost) -> Result<Vec<HookRegistration>, PluginError> {
        Ok(vec![HookRegistration {
            point: self.hook.point,
            policy: self.hook.policy,
        }])
    }

    fn run_hook(&self, ctx: &PromptHookContext) -> Result<HookOutcome, PluginError> {
        run_lua_hook(
            self.metadata.id.as_str(),
            &self.entry_path,
            &self.source,
            &self.hook,
            ctx,
        )
        .map_err(|message| PluginError::Execution {
            plugin_id: self.metadata.id.as_str().to_owned(),
            message,
        })
    }
}

struct LuaUiProvider {
    metadata: PluginMetadata,
    actions: Vec<LuaUiActionSpec>,
}

impl UiProvider for LuaUiProvider {
    fn metadata(&self) -> PluginMetadata {
        self.metadata.clone()
    }

    fn ui_actions(
        &self,
        _host: &UiRegistrationHost,
    ) -> Result<Vec<UiActionDefinition>, PluginError> {
        Ok(self
            .actions
            .iter()
            .cloned()
            .map(|spec| {
                UiActionDefinition::new(spec.id, spec.label, spec.description, spec.action_id)
            })
            .collect())
    }
}

struct LuaKeybindProvider {
    metadata: PluginMetadata,
    keybindings: Vec<LuaKeybindSpec>,
}

impl KeybindProvider for LuaKeybindProvider {
    fn metadata(&self) -> PluginMetadata {
        self.metadata.clone()
    }

    fn keybindings(
        &self,
        _host: &KeybindRegistrationHost,
    ) -> Result<Vec<KeybindDefinition>, PluginError> {
        Ok(self
            .keybindings
            .iter()
            .cloned()
            .map(|spec| KeybindDefinition::new(spec.id, spec.key, spec.description, spec.action_id))
            .collect())
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
    let specs =
        collect_lua_plugin_specs(entry_path, &source).map_err(|message| PluginDiagnostic {
            plugin_id: plugin_id.clone(),
            message,
        })?;
    let metadata = PluginMetadata::new(
        PluginId::new(manifest.id.clone()),
        manifest.name.clone(),
        manifest.version.clone(),
        PluginSource::Lua,
    );
    let source: Arc<str> = Arc::from(source);
    let mut registry = PluginRegistry::new();
    if !specs.tools.is_empty() {
        registry.register_tool_provider(Arc::new(LuaToolProvider {
            metadata: metadata.clone(),
            entry_path: entry_path.clone(),
            source: Arc::clone(&source),
            tools: specs.tools,
        }));
    }
    if !specs.commands.is_empty() {
        registry.register_command_provider(Arc::new(LuaCommandProvider {
            metadata: metadata.clone(),
            entry_path: entry_path.clone(),
            source: Arc::clone(&source),
            commands: specs.commands,
        }));
    }
    if !specs.ui_actions.is_empty() {
        registry.register_ui_provider(Arc::new(LuaUiProvider {
            metadata: metadata.clone(),
            actions: specs.ui_actions,
        }));
    }
    if !specs.keybindings.is_empty() {
        registry.register_keybind_provider(Arc::new(LuaKeybindProvider {
            metadata: metadata.clone(),
            keybindings: specs.keybindings,
        }));
    }
    for hook in specs.hooks {
        registry.register_hook_provider(Arc::new(LuaHookProvider {
            metadata: metadata.clone(),
            entry_path: entry_path.clone(),
            source: Arc::clone(&source),
            hook,
        }));
    }
    Ok(registry)
}

fn collect_lua_plugin_specs(entry_path: &Path, source: &str) -> Result<LuaPluginSpecs, String> {
    let lua = create_lua().map_err(|error| format!("failed to create Lua runtime: {error}"))?;
    let tools = Arc::new(Mutex::new(Vec::new()));
    let commands = Arc::new(Mutex::new(Vec::new()));
    let hooks = Arc::new(Mutex::new(Vec::new()));
    let ui_actions = Arc::new(Mutex::new(Vec::new()));
    let keybindings = Arc::new(Mutex::new(Vec::new()));
    let host = lua
        .create_table()
        .map_err(|error| format!("failed to create Lua plugin host: {error}"))?;
    let tools_for_host = Arc::clone(&tools);
    let tool_fn = lua
        .create_function(move |lua, args: Variadic<Value>| {
            let table = lua_definition_table(args, "tool")?;
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
    let commands_for_host = Arc::clone(&commands);
    let command_fn = lua
        .create_function(move |lua, args: Variadic<Value>| {
            let table = lua_definition_table(args, "command")?;
            let spec = lua_command_spec_from_table(lua, table)?;
            commands_for_host
                .lock()
                .map_err(|_| mlua::Error::external("Lua command registry lock poisoned"))?
                .push(spec);
            Ok(())
        })
        .map_err(|error| format!("failed to create Lua command host: {error}"))?;
    host.set("command", command_fn)
        .map_err(|error| format!("failed to install Lua command host: {error}"))?;
    let hooks_for_host = Arc::clone(&hooks);
    let hook_fn = lua
        .create_function(move |_lua, args: Variadic<Value>| {
            let table = lua_definition_table(args, "hook")?;
            let mut hooks = hooks_for_host
                .lock()
                .map_err(|_| mlua::Error::external("Lua hook registry lock poisoned"))?;
            let spec = lua_hook_spec_from_table(table, hooks.len())?;
            hooks.push(spec);
            Ok(())
        })
        .map_err(|error| format!("failed to create Lua hook host: {error}"))?;
    host.set("hook", hook_fn)
        .map_err(|error| format!("failed to install Lua hook host: {error}"))?;
    let ui_actions_for_host = Arc::clone(&ui_actions);
    let ui_action_fn = lua
        .create_function(move |_lua, args: Variadic<Value>| {
            let table = lua_definition_table(args, "ui_action")?;
            let spec = lua_ui_action_spec_from_table(table)?;
            ui_actions_for_host
                .lock()
                .map_err(|_| mlua::Error::external("Lua UI action registry lock poisoned"))?
                .push(spec);
            Ok(())
        })
        .map_err(|error| format!("failed to create Lua ui_action host: {error}"))?;
    host.set("ui_action", ui_action_fn)
        .map_err(|error| format!("failed to install Lua ui_action host: {error}"))?;
    let keybindings_for_host = Arc::clone(&keybindings);
    let keybind_fn = lua
        .create_function(move |_lua, args: Variadic<Value>| {
            let table = lua_definition_table(args, "keybind")?;
            let spec = lua_keybind_spec_from_table(table)?;
            keybindings_for_host
                .lock()
                .map_err(|_| mlua::Error::external("Lua keybind registry lock poisoned"))?
                .push(spec);
            Ok(())
        })
        .map_err(|error| format!("failed to create Lua keybind host: {error}"))?;
    host.set("keybind", keybind_fn)
        .map_err(|error| format!("failed to install Lua keybind host: {error}"))?;
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
    Ok(LuaPluginSpecs {
        tools: tools
            .lock()
            .map_err(|_| "Lua tool registry lock poisoned".to_owned())?
            .clone(),
        commands: commands
            .lock()
            .map_err(|_| "Lua command registry lock poisoned".to_owned())?
            .clone(),
        hooks: hooks
            .lock()
            .map_err(|_| "Lua hook registry lock poisoned".to_owned())?
            .clone(),
        ui_actions: ui_actions
            .lock()
            .map_err(|_| "Lua UI action registry lock poisoned".to_owned())?
            .clone(),
        keybindings: keybindings
            .lock()
            .map_err(|_| "Lua keybind registry lock poisoned".to_owned())?
            .clone(),
    })
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
            let table = lua_definition_table(args, "tool")?;
            let name = required_lua_string(&table, "name", "Lua tool")?;
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
    let command_fn = lua
        .create_function(|_, _args: Variadic<Value>| Ok(()))
        .map_err(|error| format!("failed to create Lua command host: {error}"))?;
    host.set("command", command_fn)
        .map_err(|error| format!("failed to install Lua command host: {error}"))?;
    let hook_fn = lua
        .create_function(|_, _args: Variadic<Value>| Ok(()))
        .map_err(|error| format!("failed to create Lua hook host: {error}"))?;
    host.set("hook", hook_fn)
        .map_err(|error| format!("failed to install Lua hook host: {error}"))?;
    install_lua_noop_host(&lua, &host, "ui_action")?;
    install_lua_noop_host(&lua, &host, "keybind")?;
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
    let text = lua_text_output(output)
        .map_err(|error| format!("Lua tool {tool_name} returned invalid output: {error}"))?;
    lua.remove_registry_value(key)
        .map_err(|error| format!("failed to release Lua tool {tool_name}: {error}"))?;
    Ok(text)
}

fn run_lua_command(
    plugin_id: &str,
    entry_path: &Path,
    source: &str,
    command_id: &str,
    args: serde_json::Value,
) -> Result<String, String> {
    let lua = create_lua().map_err(|error| format!("failed to create Lua runtime: {error}"))?;
    let run_key = Arc::new(Mutex::new(None));
    let host = lua
        .create_table()
        .map_err(|error| format!("failed to create Lua plugin host: {error}"))?;
    let tool_fn = lua
        .create_function(|_, _args: Variadic<Value>| Ok(()))
        .map_err(|error| format!("failed to create Lua tool host: {error}"))?;
    host.set("tool", tool_fn)
        .map_err(|error| format!("failed to install Lua tool host: {error}"))?;
    let target_id = command_id.to_owned();
    let run_key_for_host = Arc::clone(&run_key);
    let command_fn = lua
        .create_function(move |lua, args: Variadic<Value>| {
            let table = lua_definition_table(args, "command")?;
            let id = lua_command_id_from_table(&table)?;
            if id == target_id {
                let run: Function = table.get("run")?;
                let key = lua.create_registry_value(run)?;
                *run_key_for_host
                    .lock()
                    .map_err(|_| mlua::Error::external("Lua command registry lock poisoned"))? =
                    Some(key);
            }
            Ok(())
        })
        .map_err(|error| format!("failed to create Lua command host: {error}"))?;
    host.set("command", command_fn)
        .map_err(|error| format!("failed to install Lua command host: {error}"))?;
    let hook_fn = lua
        .create_function(|_, _args: Variadic<Value>| Ok(()))
        .map_err(|error| format!("failed to create Lua hook host: {error}"))?;
    host.set("hook", hook_fn)
        .map_err(|error| format!("failed to install Lua hook host: {error}"))?;
    install_lua_noop_host(&lua, &host, "ui_action")?;
    install_lua_noop_host(&lua, &host, "keybind")?;
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
        .map_err(|_| "Lua command registry lock poisoned".to_owned())?
        .take()
        .ok_or_else(|| format!("Lua plugin {plugin_id} did not register command {command_id}"))?;
    let run: Function = lua
        .registry_value(&key)
        .map_err(|error| format!("failed to resolve Lua command {command_id}: {error}"))?;
    let lua_args = lua.to_value(&args).map_err(|error| {
        format!("failed to convert command input for Lua command {command_id}: {error}")
    })?;
    let output: Value = run
        .call(lua_args)
        .map_err(|error| format!("Lua command {command_id} failed: {error}"))?;
    let text = lua_text_output(output)
        .map_err(|error| format!("Lua command {command_id} returned invalid output: {error}"))?;
    lua.remove_registry_value(key)
        .map_err(|error| format!("failed to release Lua command {command_id}: {error}"))?;
    Ok(text)
}

fn run_lua_hook(
    plugin_id: &str,
    entry_path: &Path,
    source: &str,
    spec: &LuaHookSpec,
    ctx: &PromptHookContext,
) -> Result<HookOutcome, String> {
    let lua = create_lua().map_err(|error| format!("failed to create Lua runtime: {error}"))?;
    let run_key = Arc::new(Mutex::new(None));
    let seen_hooks = Arc::new(Mutex::new(0usize));
    let host = lua
        .create_table()
        .map_err(|error| format!("failed to create Lua plugin host: {error}"))?;
    let tool_fn = lua
        .create_function(|_, _args: Variadic<Value>| Ok(()))
        .map_err(|error| format!("failed to create Lua tool host: {error}"))?;
    host.set("tool", tool_fn)
        .map_err(|error| format!("failed to install Lua tool host: {error}"))?;
    let command_fn = lua
        .create_function(|_, _args: Variadic<Value>| Ok(()))
        .map_err(|error| format!("failed to create Lua command host: {error}"))?;
    host.set("command", command_fn)
        .map_err(|error| format!("failed to install Lua command host: {error}"))?;
    let target_index = spec.index;
    let target_point = spec.point;
    let run_key_for_host = Arc::clone(&run_key);
    let seen_hooks_for_host = Arc::clone(&seen_hooks);
    let hook_fn = lua
        .create_function(move |lua, args: Variadic<Value>| {
            let table = lua_definition_table(args, "hook")?;
            let current_index = {
                let mut seen_hooks = seen_hooks_for_host
                    .lock()
                    .map_err(|_| mlua::Error::external("Lua hook registry lock poisoned"))?;
                let current_index = *seen_hooks;
                *seen_hooks += 1;
                current_index
            };
            let hook_spec = lua_hook_spec_from_table(table.clone(), current_index)?;
            if current_index == target_index {
                if hook_spec.point != target_point {
                    return Err(mlua::Error::external(
                        "Lua hook registration changed while running",
                    ));
                }
                let run: Function = table.get("run")?;
                let key = lua.create_registry_value(run)?;
                *run_key_for_host
                    .lock()
                    .map_err(|_| mlua::Error::external("Lua hook registry lock poisoned"))? =
                    Some(key);
            }
            Ok(())
        })
        .map_err(|error| format!("failed to create Lua hook host: {error}"))?;
    host.set("hook", hook_fn)
        .map_err(|error| format!("failed to install Lua hook host: {error}"))?;
    install_lua_noop_host(&lua, &host, "ui_action")?;
    install_lua_noop_host(&lua, &host, "keybind")?;
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
        .map_err(|_| "Lua hook registry lock poisoned".to_owned())?
        .take()
        .ok_or_else(|| {
            format!(
                "Lua plugin {plugin_id} did not register hook {}",
                prompt_hook_point_name(spec.point)
            )
        })?;
    let run: Function = lua.registry_value(&key).map_err(|error| {
        format!(
            "failed to resolve Lua hook {}: {error}",
            prompt_hook_point_name(spec.point)
        )
    })?;
    let lua_ctx = lua
        .to_value(&lua_prompt_hook_context(ctx))
        .map_err(|error| {
            format!(
                "failed to convert hook context for Lua hook {}: {error}",
                prompt_hook_point_name(spec.point)
            )
        })?;
    let output: Value = run.call(lua_ctx).map_err(|error| {
        format!(
            "Lua hook {} failed: {error}",
            prompt_hook_point_name(spec.point)
        )
    })?;
    let outcome = lua_hook_outcome(output).map_err(|error| {
        format!(
            "Lua hook {} returned invalid output: {error}",
            prompt_hook_point_name(spec.point)
        )
    })?;
    lua.remove_registry_value(key).map_err(|error| {
        format!(
            "failed to release Lua hook {}: {error}",
            prompt_hook_point_name(spec.point)
        )
    })?;
    Ok(outcome)
}

fn create_lua() -> Result<Lua, mlua::Error> {
    Lua::new_with(
        mlua::StdLib::TABLE | mlua::StdLib::STRING | mlua::StdLib::MATH | mlua::StdLib::UTF8,
        mlua::LuaOptions::default(),
    )
}

fn install_lua_noop_host(lua: &Lua, host: &Table, capability: &str) -> Result<(), String> {
    let noop_fn = lua
        .create_function(|_, _args: Variadic<Value>| Ok(()))
        .map_err(|error| format!("failed to create Lua {capability} host: {error}"))?;
    host.set(capability, noop_fn)
        .map_err(|error| format!("failed to install Lua {capability} host: {error}"))
}

fn lua_definition_table(args: Variadic<Value>, capability: &str) -> mlua::Result<Table> {
    args.into_iter()
        .rev()
        .find_map(|value| match value {
            Value::Table(table) => Some(table),
            _ => None,
        })
        .ok_or_else(|| {
            mlua::Error::external(format!(
                "host.{capability} requires a {capability} definition table"
            ))
        })
}

fn lua_tool_spec_from_table(lua: &Lua, table: Table) -> mlua::Result<LuaToolSpec> {
    let name = required_lua_string(&table, "name", "Lua tool")?;
    let description = required_lua_string(&table, "description", "Lua tool")?;
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

fn lua_command_spec_from_table(_lua: &Lua, table: Table) -> mlua::Result<LuaCommandSpec> {
    let id = lua_command_id_from_table(&table)?;
    let description = required_lua_string(&table, "description", "Lua command")?;
    let _: Function = table.get("run")?;
    Ok(LuaCommandSpec { id, description })
}

fn lua_hook_spec_from_table(table: Table, index: usize) -> mlua::Result<LuaHookSpec> {
    let point_name = required_lua_string(&table, "point", "Lua hook")?;
    let point = lua_prompt_hook_point_from_str(&point_name)?;
    let mut policy_name: Option<String> = table.get("policy")?;
    if policy_name.is_none() {
        policy_name = table.get("failure_policy")?;
    }
    let policy = match policy_name.as_deref() {
        Some(policy_name) => lua_hook_failure_policy_from_str(policy_name)?,
        None => HookFailurePolicy::FailOpen,
    };
    let _: Function = table.get("run")?;
    Ok(LuaHookSpec {
        index,
        point,
        policy,
    })
}

fn lua_ui_action_spec_from_table(table: Table) -> mlua::Result<LuaUiActionSpec> {
    let id = required_lua_string(&table, "id", "Lua UI action")?;
    let label = required_lua_string(&table, "label", "Lua UI action")?;
    let description = required_lua_string(&table, "description", "Lua UI action")?;
    let action_id = lua_action_id_from_table(&table, "Lua UI action")?;
    Ok(LuaUiActionSpec {
        id,
        label,
        description,
        action_id,
    })
}

fn lua_keybind_spec_from_table(table: Table) -> mlua::Result<LuaKeybindSpec> {
    let id = required_lua_string(&table, "id", "Lua keybind")?;
    let key = required_lua_string(&table, "key", "Lua keybind")?;
    let description = required_lua_string(&table, "description", "Lua keybind")?;
    let action_id = lua_action_id_from_table(&table, "Lua keybind")?;
    Ok(LuaKeybindSpec {
        id,
        key,
        description,
        action_id,
    })
}

fn lua_action_id_from_table(table: &Table, kind: &str) -> mlua::Result<String> {
    let mut action_id: Option<String> = table.get("action_id")?;
    if action_id.is_none() {
        action_id = table.get("action")?;
    }
    action_id
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| mlua::Error::external(format!("{kind} action_id must not be empty")))
}

fn lua_prompt_hook_point_from_str(value: &str) -> mlua::Result<PromptHookPoint> {
    match normalize_lua_name(value).as_str() {
        "before_prompt_prepare" => Ok(PromptHookPoint::BeforePromptPrepare),
        "after_input_prepared" => Ok(PromptHookPoint::AfterInputPrepared),
        "after_resources_loaded" => Ok(PromptHookPoint::AfterResourcesLoaded),
        "before_agent_turn" => Ok(PromptHookPoint::BeforeAgentTurn),
        "after_agent_turn" => Ok(PromptHookPoint::AfterAgentTurn),
        "before_session_commit" => Ok(PromptHookPoint::BeforeSessionCommit),
        "after_session_commit" => Ok(PromptHookPoint::AfterSessionCommit),
        other => Err(mlua::Error::external(format!(
            "unsupported Lua hook point: {other}"
        ))),
    }
}

fn lua_hook_failure_policy_from_str(value: &str) -> mlua::Result<HookFailurePolicy> {
    match normalize_lua_name(value).as_str() {
        "fail_open" => Ok(HookFailurePolicy::FailOpen),
        "fail_closed" => Ok(HookFailurePolicy::FailClosed),
        other => Err(mlua::Error::external(format!(
            "unsupported Lua hook failure policy: {other}"
        ))),
    }
}

fn normalize_lua_name(value: &str) -> String {
    value
        .trim()
        .replace('-', "_")
        .replace('.', "_")
        .to_ascii_lowercase()
}

fn prompt_hook_point_name(point: PromptHookPoint) -> &'static str {
    match point {
        PromptHookPoint::BeforePromptPrepare => "before_prompt_prepare",
        PromptHookPoint::AfterInputPrepared => "after_input_prepared",
        PromptHookPoint::AfterResourcesLoaded => "after_resources_loaded",
        PromptHookPoint::BeforeAgentTurn => "before_agent_turn",
        PromptHookPoint::AfterAgentTurn => "after_agent_turn",
        PromptHookPoint::BeforeSessionCommit => "before_session_commit",
        PromptHookPoint::AfterSessionCommit => "after_session_commit",
    }
}

fn lua_prompt_hook_context(ctx: &PromptHookContext) -> serde_json::Value {
    serde_json::json!({
        "operation_id": &ctx.operation_id,
        "turn_id": &ctx.turn_id,
        "session_id": &ctx.session_id,
        "point": prompt_hook_point_name(ctx.point),
    })
}

fn lua_hook_outcome(value: Value) -> mlua::Result<HookOutcome> {
    let mut diagnostics = Vec::new();
    match value {
        Value::Nil => {}
        Value::String(message) => {
            push_lua_hook_diagnostic(&mut diagnostics, message.to_str()?.to_owned())?
        }
        Value::Table(table) => {
            if let Some(message) = table.get::<Option<String>>("diagnostic")? {
                push_lua_hook_diagnostic(&mut diagnostics, message)?;
            }
            if let Some(message) = table.get::<Option<String>>("message")? {
                push_lua_hook_diagnostic(&mut diagnostics, message)?;
            }
            if let Some(value) = table.get::<Option<Value>>("diagnostics")? {
                push_lua_hook_diagnostics_value(&mut diagnostics, value)?;
            }
        }
        other => {
            return Err(mlua::Error::external(format!(
                "expected nil, string, or table hook output, got {}",
                other.type_name()
            )));
        }
    }
    Ok(HookOutcome { diagnostics })
}

fn push_lua_hook_diagnostics_value(
    diagnostics: &mut Vec<HookDiagnostic>,
    value: Value,
) -> mlua::Result<()> {
    match value {
        Value::Nil => Ok(()),
        Value::String(message) => {
            push_lua_hook_diagnostic(diagnostics, message.to_str()?.to_owned())
        }
        Value::Table(table) => {
            if let Some(message) = table.get::<Option<String>>("message")? {
                return push_lua_hook_diagnostic(diagnostics, message);
            }
            if let Some(message) = table.get::<Option<String>>("content")? {
                return push_lua_hook_diagnostic(diagnostics, message);
            }
            for item in table.sequence_values::<Value>() {
                push_lua_hook_diagnostics_value(diagnostics, item?)?;
            }
            Ok(())
        }
        other => Err(mlua::Error::external(format!(
            "expected string or table hook diagnostic, got {}",
            other.type_name()
        ))),
    }
}

fn push_lua_hook_diagnostic(
    diagnostics: &mut Vec<HookDiagnostic>,
    message: String,
) -> mlua::Result<()> {
    if message.trim().is_empty() {
        return Err(mlua::Error::external(
            "Lua hook diagnostic message must not be empty",
        ));
    }
    diagnostics.push(HookDiagnostic { message });
    Ok(())
}

fn lua_command_id_from_table(table: &Table) -> mlua::Result<String> {
    table
        .get::<String>("id")
        .or_else(|_| table.get::<String>("name"))
        .and_then(|value| {
            if value.trim().is_empty() {
                Err(mlua::Error::external("Lua command id must not be empty"))
            } else {
                Ok(value)
            }
        })
}

fn required_lua_string(table: &Table, field: &str, kind: &str) -> mlua::Result<String> {
    let value: String = table.get(field)?;
    if value.trim().is_empty() {
        return Err(mlua::Error::external(format!(
            "{kind} {field} must not be empty"
        )));
    }
    Ok(value)
}

fn lua_text_output(value: Value) -> mlua::Result<String> {
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
