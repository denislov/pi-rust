use std::any::Any;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};

use pi_agent_core::AgentTool;

use crate::plugins::{
    CommandDefinition, CommandProvider, CommandRegistrationHost, HookProvider, HookRegistration,
    HookRegistrationHost, PluginCapabilities, PluginError, PluginRegistry, ToolProvider,
    ToolRegistrationHost,
};

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
        let host = ToolRegistrationHost;
        let mut tools = Vec::new();
        for provider in self.registry.tool_providers() {
            match collect_provider_tools(provider.as_ref(), &host) {
                Ok(mut provided) => tools.append(&mut provided),
                Err(error) => self.record_plugin_error(error),
            }
        }
        tools
    }

    #[allow(dead_code)]
    pub(crate) fn collect_commands(&self) -> Vec<CommandDefinition> {
        let host = CommandRegistrationHost;
        let mut commands = Vec::new();
        for provider in self.registry.command_providers() {
            match collect_provider_commands(provider.as_ref(), &host) {
                Ok(mut provided) => commands.append(&mut provided),
                Err(error) => self.record_plugin_error(error),
            }
        }
        commands
    }

    #[allow(dead_code)]
    pub(crate) fn collect_prompt_hooks(&self) -> Vec<HookRegistration> {
        let host = HookRegistrationHost;
        let mut hooks = Vec::new();
        for provider in self.registry.hook_providers() {
            match collect_provider_hooks(provider.as_ref(), &host) {
                Ok(mut provided) => hooks.append(&mut provided),
                Err(error) => self.record_plugin_error(error),
            }
        }
        hooks
    }

    pub(crate) fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            tool_providers: self.registry.tool_providers().len(),
            command_providers: self.registry.command_providers().len(),
            hook_providers: self.registry.hook_providers().len(),
            ui_providers: 0,
            keybind_providers: 0,
            flow_extensions: 0,
            diagnostics: self.diagnostics().len(),
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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

fn provider_plugin_id(metadata_id: impl FnOnce() -> String) -> String {
    catch_unwind(AssertUnwindSafe(metadata_id))
        .unwrap_or_else(|panic| format!("<panic:{}>", panic_message(panic)))
}

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

    use pi_agent_core::AgentTool;

    use super::*;
    use crate::plugins::{
        CommandDefinition, CommandProvider, CommandRegistrationHost, HookFailurePolicy,
        HookProvider, HookRegistration, HookRegistrationHost, PluginError, PluginId,
        PluginMetadata, PluginRegistry, PluginSource, PromptHookPoint, ToolProvider,
        ToolRegistrationHost,
    };

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
                |_| async { Ok("plugin output".to_string()) },
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
    fn collect_prompt_hooks_returns_registered_hook_definitions() {
        let mut registry = PluginRegistry::new();
        registry.register_hook_provider(Arc::new(StaticHookProvider));
        let service = PluginService::with_registry(registry);

        let hooks = service.collect_prompt_hooks();

        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].point, PromptHookPoint::BeforeAgentTurn);
        assert_eq!(hooks[0].policy, HookFailurePolicy::FailOpen);
        assert!(service.diagnostics().is_empty());
    }

    #[test]
    fn collect_prompt_hooks_isolates_provider_failures_as_diagnostics() {
        let mut registry = PluginRegistry::new();
        registry.register_hook_provider(Arc::new(FailingHookProvider));
        registry.register_hook_provider(Arc::new(StaticHookProvider));
        let service = PluginService::with_registry(registry);

        let hooks = service.collect_prompt_hooks();

        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].point, PromptHookPoint::BeforeAgentTurn);
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
    fn capabilities_report_registered_plugin_capabilities() {
        let mut registry = PluginRegistry::new();
        registry.register_tool_provider(Arc::new(StaticToolProvider {
            plugin_id: "tools-plugin",
            tool_name: "plugin_echo",
        }));
        registry.register_command_provider(Arc::new(StaticCommandProvider));
        registry.register_hook_provider(Arc::new(StaticHookProvider));
        let service = PluginService::with_registry(registry);

        let capabilities = service.capabilities();

        assert_eq!(capabilities.tool_providers, 1);
        assert_eq!(capabilities.command_providers, 1);
        assert_eq!(capabilities.hook_providers, 1);
        assert!(service.diagnostics().is_empty());
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
