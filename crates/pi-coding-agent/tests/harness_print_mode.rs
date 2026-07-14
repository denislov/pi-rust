mod support;

use pi_agent_core::api::{AgentResources, ThinkingLevel, ToolExecutionMode};
use pi_ai::api::{Model, ModelCost, ModelInput};
use pi_ai::providers::faux::FauxProvider;
use pi_coding_agent::{
    CliRunOptions, PrintModeOptions, PromptInvocation, build_agent_config, run_cli_with_options,
    run_print_mode,
};
use std::sync::Arc;
use support::ProviderGuard;

fn faux_model(api: &str) -> Model {
    Model {
        id: "faux-model".into(),
        name: "Faux Model".into(),
        api: api.into(),
        provider: "faux".into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 0,
        max_tokens: 0,
        headers: None,
        compat: None,
    }
}

#[test]
fn thinking_level_propagates_through_print_mode_options_to_agent_config() {
    let model = faux_model("harness-print-thinking");
    let config = build_agent_config(
        model.clone(),
        None,
        Some(5),
        None,
        Some(ThinkingLevel::High),
        Some(ToolExecutionMode::Sequential),
        AgentResources::default(),
        None,
    );
    assert_eq!(config.thinking_level, ThinkingLevel::High);
    assert_eq!(config.tool_execution, ToolExecutionMode::Sequential);
}

#[test]
fn default_thinking_level_is_off_when_not_specified() {
    let model = faux_model("harness-print-default");
    let config = build_agent_config(
        model.clone(),
        None,
        Some(5),
        None,
        None,
        None,
        AgentResources::default(),
        None,
    );
    assert_eq!(config.thinking_level, ThinkingLevel::Off);
    assert_eq!(config.tool_execution, ToolExecutionMode::Parallel);
}

#[test]
fn prompt_invocation_text_variant_holds_string() {
    let inv = PromptInvocation::Text("hello world".to_string());
    match inv {
        PromptInvocation::Text(t) => assert_eq!(t, "hello world"),
        _ => panic!("expected Text variant"),
    }
}

#[test]
fn prompt_invocation_skill_variant_holds_name() {
    let inv = PromptInvocation::Skill {
        name: "rust".to_string(),
        additional_instructions: Some("be concise".to_string()),
    };
    match inv {
        PromptInvocation::Skill {
            name,
            additional_instructions,
        } => {
            assert_eq!(name, "rust");
            assert_eq!(additional_instructions, Some("be concise".to_string()));
        }
        _ => panic!("expected Skill variant"),
    }
}

#[test]
fn prompt_invocation_template_variant_holds_name_and_args() {
    let inv = PromptInvocation::PromptTemplate {
        name: "review".to_string(),
        args: vec!["arg1".to_string(), "arg2".to_string()],
    };
    match inv {
        PromptInvocation::PromptTemplate { name, args } => {
            assert_eq!(name, "review");
            assert_eq!(args, vec!["arg1", "arg2"]);
        }
        _ => panic!("expected PromptTemplate variant"),
    }
}

#[tokio::test]
async fn print_mode_runs_with_thinking_flag() {
    let api = "pi-coding-harness-thinking";
    let _provider_guard = ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("ok")));

    let output = run_print_mode(PrintModeOptions {
        prompt: "hi".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: Some(1),
        tools: Vec::new(),
        register_builtins: false,
        ai_client: Some(_provider_guard.ai_client()),
        session: None,
        session_target: None,
        session_name: None,
        thinking_level: Some(ThinkingLevel::High),
        tool_execution: Some(ToolExecutionMode::Parallel),
        resources: AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("hi".into()),
    })
    .await
    .unwrap();

    assert_eq!(output, "ok");
}

#[tokio::test]
async fn cli_accepts_thinking_flag() {
    let api = "pi-coding-harness-cli-thinking";
    let _provider_guard = ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("done")));

    let output = run_cli_with_options(
        ["-p", "hello", "--thinking", "high"]
            .map(String::from)
            .to_vec(),
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ai_client: Some(_provider_guard.ai_client()),
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 0);
    assert_eq!(output.stdout.trim(), "done");
}

#[tokio::test]
async fn cli_accepts_tool_execution_flag() {
    let api = "pi-coding-harness-cli-tool-exec";
    let _provider_guard = ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("done")));

    let output = run_cli_with_options(
        ["-p", "hello", "--tool-execution", "sequential"]
            .map(String::from)
            .to_vec(),
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ai_client: Some(_provider_guard.ai_client()),
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 0);
}

#[tokio::test]
async fn cli_rejects_invalid_thinking_level() {
    let api = "pi-coding-harness-cli-bad-thinking";
    let _provider_guard = ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("nope")));

    let output = run_cli_with_options(
        ["-p", "hello", "--thinking", "extreme"]
            .map(String::from)
            .to_vec(),
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ai_client: Some(_provider_guard.ai_client()),
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 1);
    assert!(!output.stderr.is_empty());
}
