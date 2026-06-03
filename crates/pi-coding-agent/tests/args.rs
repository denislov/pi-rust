use pi_coding_agent::{CliError, parse_args};

fn parse(values: &[&str]) -> Result<pi_coding_agent::CliArgs, CliError> {
    parse_args(values.iter().map(|value| value.to_string()))
}

#[test]
fn parses_short_print_with_prompt() {
    let args = parse(&["-p", "hello"]).unwrap();
    assert!(args.print);
    assert_eq!(args.prompt.as_deref(), Some("hello"));
}

#[test]
fn parses_long_print_with_prompt() {
    let args = parse(&["--print", "hello"]).unwrap();
    assert!(args.print);
    assert_eq!(args.prompt.as_deref(), Some("hello"));
}

#[test]
fn parses_prompt_after_flags() {
    let args = parse(&[
        "--model",
        "claude-haiku-4-5",
        "--api-key",
        "sk-test",
        "--system-prompt",
        "Be terse.",
        "--max-turns",
        "7",
        "-p",
        "say hi",
    ])
    .unwrap();

    assert_eq!(args.model.as_deref(), Some("claude-haiku-4-5"));
    assert_eq!(args.api_key.as_deref(), Some("sk-test"));
    assert_eq!(args.system_prompt.as_deref(), Some("Be terse."));
    assert_eq!(args.max_turns, 7);
    assert_eq!(args.prompt.as_deref(), Some("say hi"));
}

#[test]
fn joins_multiple_positional_words_into_one_prompt() {
    let args = parse(&["-p", "say", "hello", "now"]).unwrap();
    assert_eq!(args.prompt.as_deref(), Some("say hello now"));
}

#[test]
fn print_does_not_consume_following_option_as_prompt() {
    let args = parse(&["-p", "--model", "claude-haiku-4-5", "hello"]).unwrap();
    assert!(args.print);
    assert_eq!(args.model.as_deref(), Some("claude-haiku-4-5"));
    assert_eq!(args.prompt.as_deref(), Some("hello"));
}

#[test]
fn print_consumes_yaml_frontmatter_prompt() {
    let prompt = "---\ntitle: hello\n---\nSay hi.";
    let args = parse(&["-p", prompt]).unwrap();
    assert_eq!(args.prompt.as_deref(), Some(prompt));
}

#[test]
fn parses_help_and_version() {
    let help = parse(&["--help"]).unwrap();
    let version = parse(&["-v"]).unwrap();
    assert!(help.help);
    assert!(version.version);
}

#[test]
fn rejects_missing_flag_values() {
    assert_eq!(
        parse(&["--model"]).unwrap_err(),
        CliError::MissingValue("--model".into())
    );
    assert_eq!(
        parse(&["--api-key"]).unwrap_err(),
        CliError::MissingValue("--api-key".into())
    );
    assert_eq!(
        parse(&["--system-prompt"]).unwrap_err(),
        CliError::MissingValue("--system-prompt".into())
    );
    assert_eq!(
        parse(&["--max-turns"]).unwrap_err(),
        CliError::MissingValue("--max-turns".into())
    );
}

#[test]
fn rejects_invalid_max_turns() {
    assert_eq!(
        parse(&["--max-turns", "0"]).unwrap_err(),
        CliError::InvalidMaxTurns("0".into())
    );
    assert_eq!(
        parse(&["--max-turns", "abc"]).unwrap_err(),
        CliError::InvalidMaxTurns("abc".into())
    );
}

#[test]
fn rejects_unknown_flags() {
    assert_eq!(
        parse(&["--json"]).unwrap_err(),
        CliError::UnknownFlag("--json".into())
    );
    assert_eq!(
        parse(&["-x"]).unwrap_err(),
        CliError::UnknownFlag("-x".into())
    );
}
