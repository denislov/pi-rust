#[test]
fn legacy_agent_loop_module_is_deprecated_with_agent_turn_flow_replacement() {
    let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = std::fs::read_to_string(crate_dir.join("src/lib.rs"))
        .expect("crate lib should be readable");
    let agent_loop_source = std::fs::read_to_string(crate_dir.join("src/agent_loop.rs"))
        .expect("legacy agent_loop module should be readable");

    let module_index = lib_rs
        .find("pub mod agent_loop;")
        .expect("legacy agent_loop module should remain exported for compatibility");
    let window_start = module_index.saturating_sub(240);
    let preceding = &lib_rs[window_start..module_index];
    assert!(
        preceding.contains("#[deprecated(")
            && preceding.contains("Agent::run()")
            && preceding.contains("AgentTurnFlow"),
        "legacy agent_loop module should be deprecated with explicit Agent::run()/AgentTurnFlow replacement guidance"
    );

    assert!(
        agent_loop_source.contains("Compatibility wrapper"),
        "legacy agent_loop module should document that it is only a compatibility wrapper"
    );
    assert!(
        agent_loop_source.contains("Agent::run()") && agent_loop_source.contains("AgentTurnFlow"),
        "legacy agent_loop docs should name the replacement runtime paths"
    );
    assert!(
        agent_loop_source.contains("AgentTurnFlow::run_state(state)"),
        "legacy agent_loop wrapper should delegate to AgentTurnFlow instead of owning loop behavior"
    );
}

#[test]
fn agent_turn_flow_runtime_entrypoint_does_not_delegate_to_monolithic_loop() {
    let runtime_source = include_str!("../src/agent_turn_flow/runtime.rs");

    assert!(
        !runtime_source.contains("run_loop(state)"),
        "AgentTurnFlow::run_state should drive the graph runtime instead of delegating to run_loop"
    );
    assert!(
        runtime_source.contains("AgentTurnFlow::new()"),
        "AgentTurnFlow::run_state should construct the graph runtime"
    );
    assert!(
        runtime_source.contains(".run_with_options("),
        "AgentTurnFlow::run_state should execute Flow<AgentTurnContext>"
    );
}
