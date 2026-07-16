use std::any::Any;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};

use pi_agent_core::api::tool::AgentTool;

use crate::operations::prompt::context::CodingDiagnostic;
use crate::plugins::{
    CommandDefinition, CommandProvider, CommandRegistrationHost, HookFailurePolicy, HookOutcome,
    HookProvider, HookRegistration, HookRegistrationHost, KeybindDefinition, KeybindProvider,
    KeybindRegistrationHost, PluginCapabilities, PluginError, PluginRegistry, PromptHookContext,
    PromptHookPoint, ToolProvider, ToolRegistrationHost, UiActionDefinition, UiDialogDefinition,
    UiProvider, UiRegistrationHost,
};
use crate::runtime::capability::PluginCapabilitySet;
use crate::runtime::facade::CodingSessionError;

#[derive(Clone)]
pub(crate) struct PluginService {
    registry: PluginRegistry,
    diagnostics: Arc<Mutex<Vec<PluginDiagnostic>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PluginDiagnostic {
    pub(crate) plugin_id: Option<String>,
    pub(crate) message: String,
}

impl PluginService {
    pub(crate) fn new() -> Self {
        Self::with_registry(PluginRegistry::new())
    }

    pub(crate) fn with_registry(registry: PluginRegistry) -> Self {
        Self {
            registry,
            diagnostics: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn collect_tools(&self) -> Vec<AgentTool> {
        self.collect_tools_with_capabilities(&PluginCapabilitySet::permissive())
    }

    pub(crate) fn collect_tools_with_capabilities(
        &self,
        capabilities: &PluginCapabilitySet,
    ) -> Vec<AgentTool> {
        let host = ToolRegistrationHost::new(capabilities.clone());
        let mut tools = Vec::new();
        for provider in self.registry.tool_providers() {
            let plugin_id = provider_plugin_id(|| provider.metadata().id.as_str().to_owned());
            if !capabilities.allows_tool_provider(&plugin_id) {
                continue;
            }
            match collect_provider_tools(provider.as_ref(), &host) {
                Ok(mut provided) => tools.append(&mut provided),
                Err(error) => self.record_plugin_error(error),
            }
        }
        tools
    }

    pub(crate) fn collect_commands(&self) -> Vec<CommandDefinition> {
        let host = CommandRegistrationHost::new(PluginCapabilitySet::default());
        let mut commands = Vec::new();
        for provider in self.registry.command_providers() {
            match collect_provider_commands(provider.as_ref(), &host) {
                Ok(mut provided) => commands.append(&mut provided),
                Err(error) => self.record_plugin_error(error),
            }
        }
        commands
    }

    #[cfg(test)]
    pub(crate) fn run_command(
        &self,
        command_id: &str,
        args: serde_json::Value,
    ) -> Result<String, CodingSessionError> {
        self.run_command_with_capabilities(command_id, args, &PluginCapabilitySet::permissive())
    }

    pub(crate) fn run_command_with_capabilities(
        &self,
        command_id: &str,
        args: serde_json::Value,
        capabilities: &PluginCapabilitySet,
    ) -> Result<String, CodingSessionError> {
        let host = CommandRegistrationHost::new(capabilities.clone());
        let mut has_granted_provider = false;
        for provider in self.registry.command_providers() {
            let plugin_id = provider_plugin_id(|| provider.metadata().id.as_str().to_owned());
            if !capabilities.allows_command_provider(&plugin_id) {
                continue;
            }
            has_granted_provider = true;
            let commands = match collect_provider_commands(provider.as_ref(), &host) {
                Ok(commands) => commands,
                Err(error) => {
                    self.record_plugin_error(error);
                    continue;
                }
            };
            if !commands.iter().any(|command| command.id == command_id) {
                continue;
            }
            return match run_provider_command(provider.as_ref(), command_id, args.clone()) {
                Ok(output) => Ok(output),
                Err(error) => {
                    let message = error.to_string();
                    self.record_plugin_error(error);
                    Err(CodingSessionError::Plugin { message })
                }
            };
        }
        if capabilities.is_permissive() || has_granted_provider {
            Err(CodingSessionError::Plugin {
                message: format!("plugin command not found: {command_id}"),
            })
        } else {
            Err(CodingSessionError::UnsupportedCapability {
                capability: format!("plugin command not granted: {command_id}"),
            })
        }
    }

    pub(crate) fn collect_ui_actions(&self) -> Vec<UiActionDefinition> {
        let host = UiRegistrationHost::new(PluginCapabilitySet::default());
        let mut actions = Vec::new();
        for provider in self.registry.ui_providers() {
            match collect_provider_ui_actions(provider.as_ref(), &host) {
                Ok(mut provided) => actions.append(&mut provided),
                Err(error) => self.record_plugin_error(error),
            }
        }
        actions
    }

    pub(crate) fn collect_ui_dialogs(&self) -> Vec<UiDialogDefinition> {
        let host = UiRegistrationHost::new(PluginCapabilitySet::default());
        let mut dialogs = Vec::new();
        for provider in self.registry.ui_providers() {
            match collect_provider_ui_dialogs(provider.as_ref(), &host) {
                Ok(mut provided) => dialogs.append(&mut provided),
                Err(error) => self.record_plugin_error(error),
            }
        }
        dialogs
    }

    pub(crate) fn collect_keybindings(&self) -> Vec<KeybindDefinition> {
        let host = KeybindRegistrationHost::new(PluginCapabilitySet::default());
        let mut keybindings = Vec::new();
        for provider in self.registry.keybind_providers() {
            match collect_provider_keybindings(provider.as_ref(), &host) {
                Ok(mut provided) => keybindings.append(&mut provided),
                Err(error) => self.record_plugin_error(error),
            }
        }
        keybindings
    }

    #[cfg(test)]
    pub(crate) fn run_prompt_hook(
        &self,
        point: PromptHookPoint,
        ctx: PromptHookContext,
    ) -> Result<Vec<CodingDiagnostic>, CodingSessionError> {
        self.run_prompt_hook_with_capabilities(point, ctx, &PluginCapabilitySet::permissive())
    }

    pub(crate) fn run_prompt_hook_with_capabilities(
        &self,
        point: PromptHookPoint,
        ctx: PromptHookContext,
        capabilities: &PluginCapabilitySet,
    ) -> Result<Vec<CodingDiagnostic>, CodingSessionError> {
        let host = HookRegistrationHost::new(capabilities.clone());
        let mut diagnostics = Vec::new();
        for provider in self.registry.hook_providers() {
            let plugin_id = provider_plugin_id(|| provider.metadata().id.as_str().to_owned());
            if !capabilities.allows_hook_provider(&plugin_id) {
                continue;
            }
            let registrations = match collect_provider_hooks(provider.as_ref(), &host) {
                Ok(registrations) => registrations,
                Err(error) => {
                    self.record_plugin_error(error);
                    continue;
                }
            };
            for registration in registrations
                .into_iter()
                .filter(|registration| registration.point == point)
            {
                match run_provider_hook(provider.as_ref(), &ctx) {
                    Ok(outcome) => {
                        diagnostics.extend(outcome.diagnostics.into_iter().map(|diagnostic| {
                            CodingDiagnostic::warning(diagnostic.message).with_code("plugin_hook")
                        }))
                    }
                    Err(error) => match registration.policy {
                        HookFailurePolicy::FailOpen => {
                            let message = error.to_string();
                            self.record_plugin_error(error);
                            diagnostics
                                .push(CodingDiagnostic::warning(message).with_code("plugin_hook"));
                        }
                        HookFailurePolicy::FailClosed => {
                            let message = format!("plugin hook failed at {point:?}: {error}");
                            self.record_plugin_error(error);
                            return Err(CodingSessionError::Plugin { message });
                        }
                    },
                }
            }
        }
        Ok(diagnostics)
    }

    pub(crate) fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            tool_providers: self.registry.tool_providers().len(),
            command_providers: self.registry.command_providers().len(),
            hook_providers: self.registry.hook_providers().len(),
            ui_providers: self.registry.ui_providers().len(),
            keybind_providers: self.registry.keybind_providers().len(),
            diagnostics: self.diagnostics().len(),
            tool_provider_ids: provider_ids(self.registry.tool_providers()),
            command_provider_ids: provider_ids(self.registry.command_providers()),
            hook_provider_ids: provider_ids(self.registry.hook_providers()),
            ui_provider_ids: provider_ids(self.registry.ui_providers()),
            keybind_provider_ids: provider_ids(self.registry.keybind_providers()),
        }
    }

    pub(crate) fn diagnostics(&self) -> Vec<PluginDiagnostic> {
        self.diagnostics
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn record_plugin_error(&self, error: PluginError) {
        self.record_diagnostic(Some(error.plugin_id().to_owned()), error.to_string());
    }

    fn record_diagnostic(&self, plugin_id: Option<String>, message: String) {
        self.diagnostics
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(PluginDiagnostic { plugin_id, message });
    }
}

impl Default for PluginService {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for PluginService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PluginService")
            .field("registry", &self.registry)
            .field("diagnostics_len", &self.diagnostics().len())
            .finish()
    }
}

fn collect_provider_tools(
    provider: &dyn ToolProvider,
    host: &ToolRegistrationHost,
) -> Result<Vec<AgentTool>, PluginError> {
    let plugin_id = provider_plugin_id(|| provider.metadata().id.as_str().to_owned());
    match catch_unwind(AssertUnwindSafe(|| provider.tools(host))) {
        Ok(result) => result,
        Err(panic) => Err(PluginError::Panic {
            plugin_id,
            message: panic_message(panic),
        }),
    }
}

fn collect_provider_commands(
    provider: &dyn CommandProvider,
    host: &CommandRegistrationHost,
) -> Result<Vec<CommandDefinition>, PluginError> {
    let plugin_id = provider_plugin_id(|| provider.metadata().id.as_str().to_owned());
    match catch_unwind(AssertUnwindSafe(|| provider.commands(host))) {
        Ok(result) => result,
        Err(panic) => Err(PluginError::Panic {
            plugin_id,
            message: panic_message(panic),
        }),
    }
}

fn run_provider_command(
    provider: &dyn CommandProvider,
    command_id: &str,
    args: serde_json::Value,
) -> Result<String, PluginError> {
    let plugin_id = provider_plugin_id(|| provider.metadata().id.as_str().to_owned());
    match catch_unwind(AssertUnwindSafe(|| provider.run_command(command_id, args))) {
        Ok(result) => result,
        Err(panic) => Err(PluginError::Panic {
            plugin_id,
            message: panic_message(panic),
        }),
    }
}

fn collect_provider_hooks(
    provider: &dyn HookProvider,
    host: &HookRegistrationHost,
) -> Result<Vec<HookRegistration>, PluginError> {
    let plugin_id = provider_plugin_id(|| provider.metadata().id.as_str().to_owned());
    match catch_unwind(AssertUnwindSafe(|| provider.hooks(host))) {
        Ok(result) => result,
        Err(panic) => Err(PluginError::Panic {
            plugin_id,
            message: panic_message(panic),
        }),
    }
}

fn collect_provider_ui_actions(
    provider: &dyn UiProvider,
    host: &UiRegistrationHost,
) -> Result<Vec<UiActionDefinition>, PluginError> {
    let plugin_id = provider_plugin_id(|| provider.metadata().id.as_str().to_owned());
    match catch_unwind(AssertUnwindSafe(|| provider.ui_actions(host))) {
        Ok(result) => result,
        Err(panic) => Err(PluginError::Panic {
            plugin_id,
            message: panic_message(panic),
        }),
    }
}

fn collect_provider_ui_dialogs(
    provider: &dyn UiProvider,
    host: &UiRegistrationHost,
) -> Result<Vec<UiDialogDefinition>, PluginError> {
    let plugin_id = provider_plugin_id(|| provider.metadata().id.as_str().to_owned());
    match catch_unwind(AssertUnwindSafe(|| provider.dialogs(host))) {
        Ok(result) => result,
        Err(panic) => Err(PluginError::Panic {
            plugin_id,
            message: panic_message(panic),
        }),
    }
}

fn collect_provider_keybindings(
    provider: &dyn KeybindProvider,
    host: &KeybindRegistrationHost,
) -> Result<Vec<KeybindDefinition>, PluginError> {
    let plugin_id = provider_plugin_id(|| provider.metadata().id.as_str().to_owned());
    match catch_unwind(AssertUnwindSafe(|| provider.keybindings(host))) {
        Ok(result) => result,
        Err(panic) => Err(PluginError::Panic {
            plugin_id,
            message: panic_message(panic),
        }),
    }
}

fn run_provider_hook(
    provider: &dyn HookProvider,
    ctx: &PromptHookContext,
) -> Result<HookOutcome, PluginError> {
    let plugin_id = provider_plugin_id(|| provider.metadata().id.as_str().to_owned());
    match catch_unwind(AssertUnwindSafe(|| provider.run_hook(ctx))) {
        Ok(result) => result,
        Err(panic) => Err(PluginError::Panic {
            plugin_id,
            message: panic_message(panic),
        }),
    }
}

fn provider_plugin_id(metadata_id: impl FnOnce() -> String) -> String {
    catch_unwind(AssertUnwindSafe(metadata_id))
        .unwrap_or_else(|panic| format!("<panic:{}>", panic_message(panic)))
}

fn provider_ids<P>(providers: &[Arc<P>]) -> std::collections::BTreeSet<String>
where
    P: ProviderMetadata + ?Sized,
{
    providers
        .iter()
        .map(|provider| provider_plugin_id(|| provider.plugin_id()))
        .collect()
}

trait ProviderMetadata {
    fn plugin_id(&self) -> String;
}

macro_rules! impl_provider_metadata {
    ($($provider:ty),+ $(,)?) => {
        $(
            impl ProviderMetadata for $provider {
                fn plugin_id(&self) -> String {
                    self.metadata().id.as_str().to_owned()
                }
            }
        )+
    };
}

impl_provider_metadata!(
    dyn ToolProvider,
    dyn CommandProvider,
    dyn HookProvider,
    dyn UiProvider,
    dyn KeybindProvider,
);

fn panic_message(panic: Box<dyn Any + Send>) -> String {
    if let Some(message) = panic.downcast_ref::<String>() {
        return message.clone();
    }
    if let Some(message) = panic.downcast_ref::<&'static str>() {
        return (*message).to_owned();
    }
    "unknown panic".into()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use pi_agent_core::api::tool::AgentTool;

    use super::*;
    use crate::plugins::{
        CommandDefinition, CommandProvider, CommandRegistrationHost, HookFailurePolicy,
        HookProvider, HookRegistration, HookRegistrationHost, KeybindDefinition, KeybindProvider,
        KeybindRegistrationHost, PluginError, PluginId, PluginMetadata, PluginRegistry,
        PluginSource, PromptHookPoint, ToolProvider, ToolRegistrationHost, UiActionDefinition,
        UiProvider, UiRegistrationHost,
    };
    use crate::runtime::capability::PluginCapabilitySet;

    struct StaticToolProvider {
        plugin_id: &'static str,
        tool_name: &'static str,
    }

    impl ToolProvider for StaticToolProvider {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata::new(
                PluginId::new(self.plugin_id),
                self.plugin_id,
                "0.1.0",
                PluginSource::FirstParty,
            )
        }

        fn tools(&self, _host: &ToolRegistrationHost) -> Result<Vec<AgentTool>, PluginError> {
            Ok(vec![AgentTool::new_text(
                self.tool_name,
                "plugin test tool",
                serde_json::json!({"type": "object"}),
                |_, _| async { Ok("plugin output".to_string()) },
            )])
        }
    }

    struct FailingToolProvider;

    impl ToolProvider for FailingToolProvider {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata::new(
                PluginId::new("failing-plugin"),
                "failing-plugin",
                "0.1.0",
                PluginSource::FirstParty,
            )
        }

        fn tools(&self, _host: &ToolRegistrationHost) -> Result<Vec<AgentTool>, PluginError> {
            Err(PluginError::Registration {
                plugin_id: "failing-plugin".into(),
                message: "tool registration failed".into(),
            })
        }
    }

    struct StaticCommandProvider;

    impl CommandProvider for StaticCommandProvider {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata::new(
                PluginId::new("commands-plugin"),
                "commands-plugin",
                "0.1.0",
                PluginSource::FirstParty,
            )
        }

        fn commands(
            &self,
            _host: &CommandRegistrationHost,
        ) -> Result<Vec<CommandDefinition>, PluginError> {
            Ok(vec![CommandDefinition::new(
                "plugin.say_hello",
                "Say hello from a plugin",
            )])
        }

        fn run_command(
            &self,
            command_id: &str,
            args: serde_json::Value,
        ) -> Result<String, PluginError> {
            assert_eq!(command_id, "plugin.say_hello");
            Ok(format!(
                "hello {}",
                args.get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or("world")
            ))
        }
    }

    struct FailingCommandProvider;

    impl CommandProvider for FailingCommandProvider {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata::new(
                PluginId::new("failing-command-plugin"),
                "failing-command-plugin",
                "0.1.0",
                PluginSource::FirstParty,
            )
        }

        fn commands(
            &self,
            _host: &CommandRegistrationHost,
        ) -> Result<Vec<CommandDefinition>, PluginError> {
            Err(PluginError::Registration {
                plugin_id: "failing-command-plugin".into(),
                message: "command registration failed".into(),
            })
        }

        fn run_command(
            &self,
            _command_id: &str,
            _args: serde_json::Value,
        ) -> Result<String, PluginError> {
            Err(PluginError::Execution {
                plugin_id: "failing-command-plugin".into(),
                message: "command execution should not be reached".into(),
            })
        }
    }

    struct FailingCommandExecutionProvider;

    impl CommandProvider for FailingCommandExecutionProvider {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata::new(
                PluginId::new("failing-command-exec-plugin"),
                "failing-command-exec-plugin",
                "0.1.0",
                PluginSource::FirstParty,
            )
        }

        fn commands(
            &self,
            _host: &CommandRegistrationHost,
        ) -> Result<Vec<CommandDefinition>, PluginError> {
            Ok(vec![CommandDefinition::new(
                "plugin.fail",
                "Fails from a plugin",
            )])
        }

        fn run_command(
            &self,
            _command_id: &str,
            _args: serde_json::Value,
        ) -> Result<String, PluginError> {
            Err(PluginError::Execution {
                plugin_id: "failing-command-exec-plugin".into(),
                message: "command execution failed".into(),
            })
        }
    }

    struct StaticHookProvider;

    impl HookProvider for StaticHookProvider {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata::new(
                PluginId::new("hooks-plugin"),
                "hooks-plugin",
                "0.1.0",
                PluginSource::FirstParty,
            )
        }

        fn hooks(
            &self,
            _host: &HookRegistrationHost,
        ) -> Result<Vec<HookRegistration>, PluginError> {
            Ok(vec![HookRegistration {
                point: PromptHookPoint::BeforeAgentTurn,
                policy: HookFailurePolicy::FailOpen,
            }])
        }
    }

    struct StaticUiProvider;

    impl UiProvider for StaticUiProvider {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata::new(
                PluginId::new("ui-plugin"),
                "ui-plugin",
                "0.1.0",
                PluginSource::FirstParty,
            )
        }

        fn ui_actions(
            &self,
            _host: &UiRegistrationHost,
        ) -> Result<Vec<UiActionDefinition>, PluginError> {
            Ok(vec![UiActionDefinition::new(
                "ui.open_panel",
                "Open panel",
                "Open the plugin panel",
                "plugin.open_panel",
            )])
        }

        fn dialogs(
            &self,
            _host: &UiRegistrationHost,
        ) -> Result<Vec<UiDialogDefinition>, PluginError> {
            Ok(vec![UiDialogDefinition::new(
                "dialog.open_panel",
                "Plugin panel",
                "Panel registered by plugin",
                "plugin.submit_panel",
            )])
        }
    }

    struct FailingUiProvider;

    impl UiProvider for FailingUiProvider {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata::new(
                PluginId::new("failing-ui-plugin"),
                "failing-ui-plugin",
                "0.1.0",
                PluginSource::FirstParty,
            )
        }

        fn ui_actions(
            &self,
            _host: &UiRegistrationHost,
        ) -> Result<Vec<UiActionDefinition>, PluginError> {
            Err(PluginError::Registration {
                plugin_id: "failing-ui-plugin".into(),
                message: "ui registration failed".into(),
            })
        }
    }

    struct StaticKeybindProvider;

    impl KeybindProvider for StaticKeybindProvider {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata::new(
                PluginId::new("keybind-plugin"),
                "keybind-plugin",
                "0.1.0",
                PluginSource::FirstParty,
            )
        }

        fn keybindings(
            &self,
            _host: &KeybindRegistrationHost,
        ) -> Result<Vec<KeybindDefinition>, PluginError> {
            Ok(vec![KeybindDefinition::new(
                "keybind.open_panel",
                "ctrl+p",
                "Open the plugin panel",
                "plugin.open_panel",
            )])
        }
    }

    struct FailingKeybindProvider;

    impl KeybindProvider for FailingKeybindProvider {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata::new(
                PluginId::new("failing-keybind-plugin"),
                "failing-keybind-plugin",
                "0.1.0",
                PluginSource::FirstParty,
            )
        }

        fn keybindings(
            &self,
            _host: &KeybindRegistrationHost,
        ) -> Result<Vec<KeybindDefinition>, PluginError> {
            Err(PluginError::Registration {
                plugin_id: "failing-keybind-plugin".into(),
                message: "keybind registration failed".into(),
            })
        }
    }

    struct FailingHookProvider;

    impl HookProvider for FailingHookProvider {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata::new(
                PluginId::new("failing-hook-plugin"),
                "failing-hook-plugin",
                "0.1.0",
                PluginSource::FirstParty,
            )
        }

        fn hooks(
            &self,
            _host: &HookRegistrationHost,
        ) -> Result<Vec<HookRegistration>, PluginError> {
            Err(PluginError::Registration {
                plugin_id: "failing-hook-plugin".into(),
                message: "hook registration failed".into(),
            })
        }
    }

    struct PanickingToolProvider;

    impl ToolProvider for PanickingToolProvider {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata::new(
                PluginId::new("panic-plugin"),
                "panic-plugin",
                "0.1.0",
                PluginSource::FirstParty,
            )
        }

        fn tools(&self, _host: &ToolRegistrationHost) -> Result<Vec<AgentTool>, PluginError> {
            panic!("tool provider panicked")
        }
    }

    #[test]
    fn run_prompt_hook_executes_registered_point() {
        let mut registry = PluginRegistry::new();
        registry.register_hook_provider(Arc::new(StaticHookProvider));
        let service = PluginService::with_registry(registry);

        let diagnostics = service
            .run_prompt_hook(
                PromptHookPoint::BeforeAgentTurn,
                PromptHookContext {
                    operation_id: "op_hook".into(),
                    turn_id: "turn_hook".into(),
                    session_id: None,
                    point: PromptHookPoint::BeforeAgentTurn,
                },
            )
            .unwrap();

        assert!(diagnostics.is_empty());
        assert!(service.diagnostics().is_empty());
    }

    #[test]
    fn run_prompt_hook_isolates_registration_failures_as_diagnostics() {
        let mut registry = PluginRegistry::new();
        registry.register_hook_provider(Arc::new(FailingHookProvider));
        registry.register_hook_provider(Arc::new(StaticHookProvider));
        let service = PluginService::with_registry(registry);

        let hook_diagnostics = service
            .run_prompt_hook(
                PromptHookPoint::BeforeAgentTurn,
                PromptHookContext {
                    operation_id: "op_hook".into(),
                    turn_id: "turn_hook".into(),
                    session_id: None,
                    point: PromptHookPoint::BeforeAgentTurn,
                },
            )
            .unwrap();

        assert!(hook_diagnostics.is_empty());
        let diagnostics = service.diagnostics();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].plugin_id.as_deref(),
            Some("failing-hook-plugin")
        );
        assert!(diagnostics[0].message.contains("hook registration failed"));
    }

    #[test]
    fn collect_commands_returns_registered_command_provider_definitions() {
        let mut registry = PluginRegistry::new();
        registry.register_command_provider(Arc::new(StaticCommandProvider));
        let service = PluginService::with_registry(registry);

        let commands = service.collect_commands();

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].id, "plugin.say_hello");
        assert!(service.diagnostics().is_empty());
    }

    #[test]
    fn collect_commands_isolates_provider_failures_as_diagnostics() {
        let mut registry = PluginRegistry::new();
        registry.register_command_provider(Arc::new(FailingCommandProvider));
        registry.register_command_provider(Arc::new(StaticCommandProvider));
        let service = PluginService::with_registry(registry);

        let commands = service.collect_commands();

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].id, "plugin.say_hello");
        let diagnostics = service.diagnostics();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].plugin_id.as_deref(),
            Some("failing-command-plugin")
        );
        assert!(
            diagnostics[0]
                .message
                .contains("command registration failed")
        );
    }

    #[test]
    fn run_command_executes_registered_command_provider() {
        let mut registry = PluginRegistry::new();
        registry.register_command_provider(Arc::new(StaticCommandProvider));
        let service = PluginService::with_registry(registry);

        let output = service
            .run_command("plugin.say_hello", serde_json::json!({"name": "pi"}))
            .unwrap();

        assert_eq!(output, "hello pi");
        assert!(service.diagnostics().is_empty());
    }

    #[test]
    fn run_command_records_provider_execution_failure_as_diagnostic() {
        let mut registry = PluginRegistry::new();
        registry.register_command_provider(Arc::new(FailingCommandExecutionProvider));
        let service = PluginService::with_registry(registry);

        let error = service
            .run_command("plugin.fail", serde_json::json!({}))
            .unwrap_err();

        assert_eq!(error.code(), "plugin");
        assert!(error.to_string().contains("command execution failed"));
        let diagnostics = service.diagnostics();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].plugin_id.as_deref(),
            Some("failing-command-exec-plugin")
        );
        assert!(diagnostics[0].message.contains("command execution failed"));
    }

    #[test]
    fn capabilities_report_registered_plugin_capabilities() {
        let mut registry = PluginRegistry::new();
        registry.register_tool_provider(Arc::new(StaticToolProvider {
            plugin_id: "tools-plugin",
            tool_name: "plugin_echo",
        }));
        registry.register_command_provider(Arc::new(StaticCommandProvider));
        registry.register_hook_provider(Arc::new(StaticHookProvider));
        registry.register_ui_provider(Arc::new(StaticUiProvider));
        registry.register_keybind_provider(Arc::new(StaticKeybindProvider));
        let service = PluginService::with_registry(registry);

        let capabilities = service.capabilities();

        assert_eq!(capabilities.tool_providers, 1);
        assert_eq!(capabilities.command_providers, 1);
        assert_eq!(capabilities.hook_providers, 1);
        assert_eq!(capabilities.ui_providers, 1);
        assert_eq!(capabilities.keybind_providers, 1);
        assert!(service.diagnostics().is_empty());
    }

    #[test]
    fn collect_ui_actions_returns_registered_action_definitions() {
        let mut registry = PluginRegistry::new();
        registry.register_ui_provider(Arc::new(StaticUiProvider));
        let service = PluginService::with_registry(registry);

        let actions = service.collect_ui_actions();

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].id, "ui.open_panel");
        assert_eq!(actions[0].label, "Open panel");
        assert_eq!(actions[0].action_id, "plugin.open_panel");
        assert!(service.diagnostics().is_empty());
    }

    #[test]
    fn collect_ui_dialogs_returns_registered_dialog_definitions() {
        let mut registry = PluginRegistry::new();
        registry.register_ui_provider(Arc::new(StaticUiProvider));
        let service = PluginService::with_registry(registry);

        let dialogs = service.collect_ui_dialogs();

        assert_eq!(dialogs.len(), 1);
        assert_eq!(dialogs[0].id, "dialog.open_panel");
        assert_eq!(dialogs[0].title, "Plugin panel");
        assert_eq!(dialogs[0].action_id, "plugin.submit_panel");
        assert!(service.diagnostics().is_empty());
    }

    #[test]
    fn collect_ui_actions_isolates_provider_failures_as_diagnostics() {
        let mut registry = PluginRegistry::new();
        registry.register_ui_provider(Arc::new(FailingUiProvider));
        registry.register_ui_provider(Arc::new(StaticUiProvider));
        let service = PluginService::with_registry(registry);

        let actions = service.collect_ui_actions();

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].id, "ui.open_panel");
        let diagnostics = service.diagnostics();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].plugin_id.as_deref(),
            Some("failing-ui-plugin")
        );
        assert!(diagnostics[0].message.contains("ui registration failed"));
    }

    #[test]
    fn collect_keybindings_returns_registered_keybinding_definitions() {
        let mut registry = PluginRegistry::new();
        registry.register_keybind_provider(Arc::new(StaticKeybindProvider));
        let service = PluginService::with_registry(registry);

        let keybindings = service.collect_keybindings();

        assert_eq!(keybindings.len(), 1);
        assert_eq!(keybindings[0].id, "keybind.open_panel");
        assert_eq!(keybindings[0].key, "ctrl+p");
        assert_eq!(keybindings[0].action_id, "plugin.open_panel");
        assert!(service.diagnostics().is_empty());
    }

    #[test]
    fn collect_keybindings_isolates_provider_failures_as_diagnostics() {
        let mut registry = PluginRegistry::new();
        registry.register_keybind_provider(Arc::new(FailingKeybindProvider));
        registry.register_keybind_provider(Arc::new(StaticKeybindProvider));
        let service = PluginService::with_registry(registry);

        let keybindings = service.collect_keybindings();

        assert_eq!(keybindings.len(), 1);
        assert_eq!(keybindings[0].id, "keybind.open_panel");
        let diagnostics = service.diagnostics();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].plugin_id.as_deref(),
            Some("failing-keybind-plugin")
        );
        assert!(
            diagnostics[0]
                .message
                .contains("keybind registration failed")
        );
    }

    #[test]
    fn collect_tools_returns_registered_tool_provider_tools() {
        let mut registry = PluginRegistry::new();
        registry.register_tool_provider(Arc::new(StaticToolProvider {
            plugin_id: "tools-plugin",
            tool_name: "plugin_echo",
        }));
        let service = PluginService::with_registry(registry);

        let tools = service.collect_tools();

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "plugin_echo");
        assert!(service.diagnostics().is_empty());
    }

    #[test]
    fn collect_tools_isolates_provider_failures_as_diagnostics() {
        let mut registry = PluginRegistry::new();
        registry.register_tool_provider(Arc::new(FailingToolProvider));
        registry.register_tool_provider(Arc::new(StaticToolProvider {
            plugin_id: "healthy-plugin",
            tool_name: "healthy_tool",
        }));
        let service = PluginService::with_registry(registry);

        let tools = service.collect_tools();

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "healthy_tool");
        let diagnostics = service.diagnostics();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].plugin_id.as_deref(), Some("failing-plugin"));
        assert!(diagnostics[0].message.contains("tool registration failed"));
    }

    #[test]
    fn collect_tools_with_capabilities_suppresses_plugin_tools_when_not_granted() {
        let mut registry = PluginRegistry::new();
        registry.register_tool_provider(Arc::new(StaticToolProvider {
            plugin_id: "tools-plugin",
            tool_name: "plugin_echo",
        }));
        let service = PluginService::with_registry(registry);
        let capabilities = PluginCapabilitySet::default();

        let tools = service.collect_tools_with_capabilities(&capabilities);

        assert!(tools.is_empty());
    }

    #[test]
    fn collect_tools_with_capabilities_only_calls_granted_provider_ids() {
        let mut registry = PluginRegistry::new();
        registry.register_tool_provider(Arc::new(StaticToolProvider {
            plugin_id: "granted-tools",
            tool_name: "granted_tool",
        }));
        registry.register_tool_provider(Arc::new(StaticToolProvider {
            plugin_id: "ungranted-tools",
            tool_name: "ungranted_tool",
        }));
        let service = PluginService::with_registry(registry);
        let mut inventory = service.capabilities();
        inventory
            .tool_provider_ids
            .retain(|plugin_id| plugin_id == "granted-tools");
        let capabilities = PluginCapabilitySet::from(&inventory);

        let tools = service.collect_tools_with_capabilities(&capabilities);

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "granted_tool");
    }

    #[test]
    fn run_command_with_capabilities_rejects_ungranted_commands() {
        let mut registry = PluginRegistry::new();
        registry.register_command_provider(Arc::new(StaticCommandProvider));
        let service = PluginService::with_registry(registry);
        let capabilities = PluginCapabilitySet::default();

        let error = service
            .run_command_with_capabilities("static.command", serde_json::json!({}), &capabilities)
            .unwrap_err();

        assert_eq!(error.code(), "unsupported_capability");
    }

    #[test]
    fn run_command_with_capabilities_does_not_probe_ungranted_provider_ids() {
        let mut registry = PluginRegistry::new();
        registry.register_command_provider(Arc::new(StaticCommandProvider));
        registry.register_command_provider(Arc::new(FailingCommandExecutionProvider));
        let service = PluginService::with_registry(registry);
        let mut inventory = service.capabilities();
        inventory
            .command_provider_ids
            .retain(|plugin_id| plugin_id == "commands-plugin");
        let capabilities = PluginCapabilitySet::from(&inventory);

        let error = service
            .run_command_with_capabilities("plugin.fail", serde_json::json!({}), &capabilities)
            .unwrap_err();

        assert_eq!(error.code(), "plugin");
        assert!(error.to_string().contains("plugin command not found"));
        assert!(service.diagnostics().is_empty());
    }

    #[test]
    fn run_prompt_hook_with_capabilities_suppresses_ungranted_hooks() {
        let mut registry = PluginRegistry::new();
        registry.register_hook_provider(Arc::new(StaticHookProvider));
        let service = PluginService::with_registry(registry);

        let diagnostics = service
            .run_prompt_hook_with_capabilities(
                PromptHookPoint::BeforeAgentTurn,
                PromptHookContext {
                    operation_id: "op_hook".into(),
                    turn_id: "turn_hook".into(),
                    session_id: None,
                    point: PromptHookPoint::BeforeAgentTurn,
                },
                &PluginCapabilitySet::default(),
            )
            .unwrap();

        assert!(diagnostics.is_empty());
        assert!(service.diagnostics().is_empty());
    }

    #[test]
    fn run_prompt_hook_with_capabilities_skips_ungranted_provider_ids() {
        let mut registry = PluginRegistry::new();
        registry.register_hook_provider(Arc::new(StaticHookProvider));
        registry.register_hook_provider(Arc::new(FailingHookProvider));
        let service = PluginService::with_registry(registry);
        let mut inventory = service.capabilities();
        inventory
            .hook_provider_ids
            .retain(|plugin_id| plugin_id == "hooks-plugin");
        let capabilities = PluginCapabilitySet::from(&inventory);

        let diagnostics = service
            .run_prompt_hook_with_capabilities(
                PromptHookPoint::BeforeAgentTurn,
                PromptHookContext {
                    operation_id: "op_hook".into(),
                    turn_id: "turn_hook".into(),
                    session_id: None,
                    point: PromptHookPoint::BeforeAgentTurn,
                },
                &capabilities,
            )
            .unwrap();

        assert!(diagnostics.is_empty());
        assert!(service.diagnostics().is_empty());
    }

    #[test]
    fn collect_tools_isolates_provider_panics_as_diagnostics() {
        let mut registry = PluginRegistry::new();
        registry.register_tool_provider(Arc::new(PanickingToolProvider));
        registry.register_tool_provider(Arc::new(StaticToolProvider {
            plugin_id: "healthy-plugin",
            tool_name: "healthy_tool",
        }));
        let service = PluginService::with_registry(registry);

        let tools = service.collect_tools();

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "healthy_tool");
        let diagnostics = service.diagnostics();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].plugin_id.as_deref(), Some("panic-plugin"));
        assert!(diagnostics[0].message.contains("tool provider panicked"));
    }
}
