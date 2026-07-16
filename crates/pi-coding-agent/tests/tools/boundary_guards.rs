//! Product tool ownership and provider-visibility boundaries.

use std::fs;
use std::path::PathBuf;

#[test]
fn runtime_service_validates_tools_before_provider_visibility() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(crate_root.join("src/services/runtime.rs"))
        .expect("read runtime service source");
    let start = source
        .find("fn build_agent_runtime_with_plugins_and_diagnostics(")
        .expect("runtime builder should exist");
    let end = source[start..]
        .find("fn apply_tool_policy(")
        .map(|offset| start + offset)
        .expect("runtime builder should be followed by apply_tool_policy");
    let body = &source[start..end];

    assert!(
        body.contains(".try_add_tool(tool)"),
        "RuntimeService must validate each provider-visible tool through Agent::try_add_tool"
    );
    assert!(
        !body.contains(".add_tool(tool)"),
        "RuntimeService must not bypass tool validation with Agent::add_tool(tool)"
    );
}

#[test]
fn plugin_tool_hosts_remain_capability_scoped() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(crate_root.join("src/plugins/contributions/tool.rs"))
        .expect("read plugin tool host source");

    for forbidden in [
        "CodingAgentSession",
        "SessionService",
        "RuntimeService",
        "FlowService",
        "AiClient",
        "ProviderRegistry",
        "ExecutionEnv",
        "FileSystem",
        "Shell",
    ] {
        assert!(
            !source.contains(forbidden),
            "ToolRegistrationHost must not expose raw `{forbidden}` internals"
        );
    }
}

#[test]
fn plugins_do_not_expose_arbitrary_flow_extension_surface() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    assert!(
        !crate_root.join("src/plugins/flow_extension.rs").exists(),
        "arbitrary plugin Flow extensions are an architecture non-goal"
    );

    for relative in [
        "src/plugins/mod.rs",
        "src/plugins/registry.rs",
        "src/plugins/capability.rs",
        "src/services/plugin.rs",
        "src/runtime/capability.rs",
    ] {
        let source = fs::read_to_string(crate_root.join(relative)).expect("read plugin owner");
        for forbidden in ["FlowExtension", "flow_extension", "flow_extensions"] {
            assert!(
                !source.contains(forbidden),
                "{relative} must not reintroduce unsupported `{forbidden}` plugin Flow surface"
            );
        }
    }
}
