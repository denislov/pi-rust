//! Product tool ownership and provider-visibility boundaries.

use std::fs;
use std::path::PathBuf;

#[test]
fn runtime_service_validates_tools_before_provider_visibility() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(crate_root.join("src/services/runtime.rs"))
        .expect("read runtime service source");
    let start = source
        .find("fn build_agent_runtime_with_diagnostics(")
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
