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
