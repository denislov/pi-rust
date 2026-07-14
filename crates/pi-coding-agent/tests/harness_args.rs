use pi_agent_core::{ThinkingLevel, ToolExecutionMode};
use pi_coding_agent::parse_args;

#[test]
fn parses_thinking_flag() {
    let args = parse_args(
        ["-p", "hello", "--thinking", "high"]
            .map(String::from)
            .to_vec(),
    )
    .unwrap();
    assert_eq!(args.thinking, Some(ThinkingLevel::High));
}

#[test]
fn parses_tool_execution_flag() {
    let args = parse_args(
        ["-p", "hello", "--tool-execution", "sequential"]
            .map(String::from)
            .to_vec(),
    )
    .unwrap();
    assert_eq!(args.tool_execution, Some(ToolExecutionMode::Sequential));
}

#[test]
fn parses_skills_repeated() {
    let args = parse_args(
        ["-p", "hello", "--skills", "dir1", "--skills", "dir2"]
            .map(String::from)
            .to_vec(),
    )
    .unwrap();
    assert_eq!(args.skills, vec!["dir1", "dir2"]);
}

#[test]
fn parses_skill_and_prompt_template_rejected_together() {
    let result = parse_args(
        [
            "-p",
            "hello",
            "--skill",
            "rust",
            "--prompt-template",
            "review",
        ]
        .map(String::from)
        .to_vec(),
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("cannot be used together"));
}

#[test]
fn parses_template_args() {
    let args = parse_args(
        [
            "-p",
            "hello",
            "--prompt-template",
            "review",
            "--template-arg",
            "arg1",
            "--template-arg",
            "arg2",
        ]
        .map(String::from)
        .to_vec(),
    )
    .unwrap();
    assert_eq!(args.prompt_template, Some("review".to_string()));
    assert_eq!(args.template_args, vec!["arg1", "arg2"]);
}

#[test]
fn rejects_invalid_thinking_value() {
    let result = parse_args(
        ["-p", "hello", "--thinking", "extreme"]
            .map(String::from)
            .to_vec(),
    );
    assert!(result.is_err());
}

#[test]
fn help_mentions_m4_flags() {
    let text = pi_coding_agent::help_text();
    assert!(text.contains("--thinking"));
    assert!(text.contains("--tool-execution"));
    assert!(text.contains("--skills"));
    assert!(text.contains("--prompt-templates"));
    assert!(text.contains("--skill"));
    assert!(text.contains("--prompt-template"));
    assert!(text.contains("--template-arg"));
}

#[test]
fn skill_flag_parsed() {
    let args = parse_args(
        ["-p", "hello", "--skill", "rust"]
            .map(String::from)
            .to_vec(),
    )
    .unwrap();
    assert_eq!(args.skill, Some("rust".to_string()));
}

#[test]
fn prompt_template_flag_parsed() {
    let args = parse_args(
        ["-p", "hello", "--prompt-template", "review"]
            .map(String::from)
            .to_vec(),
    )
    .unwrap();
    assert_eq!(args.prompt_template, Some("review".to_string()));
}

#[test]
fn default_m4_fields_are_empty() {
    let args = parse_args(["-p", "hello"].map(String::from).to_vec()).unwrap();
    assert_eq!(args.thinking, None);
    assert_eq!(args.tool_execution, None);
    assert!(args.skills.is_empty());
    assert!(args.prompt_templates.is_empty());
    assert_eq!(args.skill, None);
    assert_eq!(args.prompt_template, None);
    assert!(args.template_args.is_empty());
}

#[test]
fn rejects_invalid_tool_execution_value() {
    let result = parse_args(
        ["-p", "hello", "--tool-execution", "serial"]
            .map(String::from)
            .to_vec(),
    );
    assert!(result.is_err());
}
