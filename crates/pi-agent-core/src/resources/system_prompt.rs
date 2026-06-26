use std::sync::LazyLock;

use regex::Regex;

use crate::types::Skill;

pub fn format_skills_for_system_prompt(skills: &[Skill]) -> String {
    if skills.is_empty() {
        return String::new();
    }

    let mut out = String::from("<available_skills>\n");

    for skill in skills {
        if skill.disable_model_invocation {
            continue;
        }
        out.push_str(&format!(
            "  <skill>\n    <name>{}</name>\n    <description>{}</description>\n    <location>{}</location>\n  </skill>\n",
            xml_escape(&skill.name),
            xml_escape(&skill.description),
            xml_escape(&skill.location),
        ));
    }

    out.push_str("</available_skills>");
    out
}

pub fn format_skill_invocation(
    name: &str,
    location: &str,
    content: &str,
    additional_instructions: Option<&str>,
) -> String {
    let mut out = format!(
        "<skill name=\"{}\" location=\"{}\">\n{}\n</skill>",
        xml_escape(name),
        xml_escape(location),
        content
    );

    if let Some(instructions) = additional_instructions {
        out.push_str(&format!("\n\n{}", instructions));
    }

    out
}

/// Parse command arguments respecting quoted strings (bash-style).
///
/// Mirrors TS `parseCommandArgs` in `packages/coding-agent/src/core/prompt-templates.ts`.
pub fn parse_command_args(args_string: &str) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_quote: Option<char> = None;

    for c in args_string.chars() {
        match in_quote {
            Some(quote) => {
                if c == quote {
                    in_quote = None;
                } else {
                    current.push(c);
                }
            }
            None => {
                if c == '"' || c == '\'' {
                    in_quote = Some(c);
                } else if c.is_whitespace() {
                    if !current.is_empty() {
                        args.push(std::mem::take(&mut current));
                    }
                } else {
                    current.push(c);
                }
            }
        }
    }

    if !current.is_empty() {
        args.push(current);
    }

    args
}

static SUBSTITUTE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\$\{(\d+):-([^}]*)\}|\$\{@:(\d+)(?::(\d+))?\}|\$(ARGUMENTS|@|\d+)")
        .expect("invalid substitute_args regex")
});

/// Substitute argument placeholders in template content.
///
/// Supports:
/// - $1, $2, ... for positional args
/// - $@ and $ARGUMENTS for all args
/// - ${N:-default} for positional arg N with default when missing/empty
/// - ${@:N} for args from Nth onwards (bash-style slicing)
/// - ${@:N:L} for L args starting from Nth
///
/// Mirrors TS `substituteArgs` in `packages/coding-agent/src/core/prompt-templates.ts`.
pub fn substitute_args(content: &str, args: &[String]) -> String {
    let all_args = args.join(" ");

    SUBSTITUTE_RE
        .replace_all(content, |caps: &regex::Captures| {
            // Group 1, 2: ${N:-default}
            if let Some(default_num) = caps.get(1) {
                let index: usize = default_num.as_str().parse().unwrap_or(1) - 1;
                let default_val = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                match args.get(index) {
                    Some(v) if !v.is_empty() => v.clone(),
                    _ => default_val.to_string(),
                }
            }
            // Group 3, 4: ${@:N} or ${@:N:L}
            else if let Some(slice_start) = caps.get(3) {
                let start: usize = slice_start.as_str().parse().unwrap_or(1);
                // Treat 0 as 1 (bash convention: args start at 1)
                let start = if start == 0 { 0 } else { start - 1 };

                if start >= args.len() {
                    return String::new();
                }

                if let Some(slice_len) = caps.get(4) {
                    let len: usize = slice_len.as_str().parse().unwrap_or(0);
                    args[start..]
                        .iter()
                        .take(len)
                        .map(|s| s.as_str())
                        .collect::<Vec<&str>>()
                        .join(" ")
                } else {
                    args[start..]
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<&str>>()
                        .join(" ")
                }
            }
            // Group 5: $ARGUMENTS, $@, or $N
            else if let Some(simple) = caps.get(5) {
                let s = simple.as_str();
                if s == "ARGUMENTS" || s == "@" {
                    all_args.clone()
                } else {
                    let index: usize = s.parse().unwrap_or(1) - 1;
                    args.get(index).cloned().unwrap_or_default()
                }
            } else {
                String::new()
            }
        })
        .to_string()
}

/// Format a prompt template invocation, substituting arguments into the template content.
///
/// Delegates to [`substitute_args`] for full TS-compatible placeholders.
pub fn format_prompt_template_invocation(_name: &str, content: &str, args: &[String]) -> String {
    substitute_args(content, args)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_skills_xml_block() {
        let skills = vec![Skill {
            name: "rust".into(),
            description: "Rust programming".into(),
            location: "/skills/rust/SKILL.md".into(),
            content: String::new(),
            disable_model_invocation: false,
        }];
        let formatted = format_skills_for_system_prompt(&skills);
        assert!(formatted.contains("<available_skills>"));
        assert!(formatted.contains("<name>rust</name>"));
        assert!(formatted.contains("Rust programming"));
        assert!(formatted.contains("</available_skills>"));
    }

    #[test]
    fn excludes_disabled_skills() {
        let skills = vec![
            Skill {
                name: "active".into(),
                description: "active".into(),
                location: "/a".into(),
                content: String::new(),
                disable_model_invocation: false,
            },
            Skill {
                name: "disabled".into(),
                description: "disabled".into(),
                location: "/d".into(),
                content: String::new(),
                disable_model_invocation: true,
            },
        ];
        let formatted = format_skills_for_system_prompt(&skills);
        assert!(formatted.contains("active"));
        assert!(!formatted.contains("disabled"));
    }

    #[test]
    fn empty_skills_returns_empty_string() {
        assert!(format_skills_for_system_prompt(&[]).is_empty());
    }

    #[test]
    fn skill_invocation_includes_location_and_content() {
        let text = format_skill_invocation(
            "rust",
            "/skills/rust/SKILL.md",
            "Rust programming guide",
            Some("use best practices"),
        );
        assert!(text.contains("rust"));
        assert!(text.contains("/skills/rust/SKILL.md"));
        assert!(text.contains("Rust programming guide"));
        assert!(text.contains("use best practices"));
    }

    #[test]
    fn template_invocation_replaces_positional_args() {
        // $N is replaced, but ${N} without :-default is NOT replaced (TS-compatible)
        let result = format_prompt_template_invocation(
            "review",
            "Review $1 and ${2}",
            &["foo".to_string(), "bar".to_string()],
        );
        assert_eq!(result, "Review foo and ${2}");
    }

    #[test]
    fn template_invocation_replaces_arg_arguments() {
        let result = format_prompt_template_invocation(
            "test",
            "Args: $ARGUMENTS",
            &["a".to_string(), "b".to_string()],
        );
        assert_eq!(result, "Args: a b");
    }

    #[test]
    fn template_invocation_replaces_at_all() {
        let result = format_prompt_template_invocation(
            "test",
            "Args: $@",
            &["a".to_string(), "b".to_string()],
        );
        assert_eq!(result, "Args: a b");
    }

    // ── parse_command_args tests ──────────────────────────────────────

    #[test]
    fn parse_args_whitespace_split() {
        assert_eq!(parse_command_args("a b c"), vec!["a", "b", "c"]);
    }

    #[test]
    fn parse_args_extra_spaces() {
        assert_eq!(parse_command_args("a  b   c"), vec!["a", "b", "c"]);
    }

    #[test]
    fn parse_args_double_quotes() {
        assert_eq!(
            parse_command_args("\"first arg\" second"),
            vec!["first arg", "second"]
        );
    }

    #[test]
    fn parse_args_single_quotes() {
        assert_eq!(
            parse_command_args("'first arg' second"),
            vec!["first arg", "second"]
        );
    }

    #[test]
    fn parse_args_empty_input() {
        let empty: Vec<String> = Vec::new();
        assert_eq!(parse_command_args(""), empty);
    }

    #[test]
    fn parse_args_whitespace_only() {
        let empty: Vec<String> = Vec::new();
        assert_eq!(parse_command_args("   \t\n"), empty);
    }

    #[test]
    fn parse_args_unicode() {
        assert_eq!(
            parse_command_args("日本語 🎉 café"),
            vec!["日本語", "🎉", "café"]
        );
    }

    // ── substitute_args tests ─────────────────────────────────────────

    #[test]
    fn substitute_arg_arguments_all_args() {
        assert_eq!(
            substitute_args("Test: $ARGUMENTS", &["a".into(), "b".into(), "c".into()]),
            "Test: a b c"
        );
    }

    #[test]
    fn substitute_at_all_args() {
        assert_eq!(
            substitute_args("Test: $@", &["a".into(), "b".into(), "c".into()]),
            "Test: a b c"
        );
    }

    #[test]
    fn substitute_at_and_arguments_identical() {
        let args = ["foo".into(), "bar".into(), "baz".into()];
        assert_eq!(
            substitute_args("Test: $@", &args),
            substitute_args("Test: $ARGUMENTS", &args)
        );
    }

    #[test]
    fn substitute_no_recursive_patterns_in_args() {
        assert_eq!(
            substitute_args("$ARGUMENTS", &["$1".into(), "$ARGUMENTS".into()]),
            "$1 $ARGUMENTS"
        );
        assert_eq!(
            substitute_args("$@", &["$100".into(), "$1".into()]),
            "$100 $1"
        );
    }

    #[test]
    fn substitute_positional_default_when_missing() {
        assert_eq!(
            substitute_args("List exactly ${1:-7} next steps", &[]),
            "List exactly 7 next steps"
        );
    }

    #[test]
    fn substitute_positional_default_when_present() {
        assert_eq!(
            substitute_args("List exactly ${1:-7} next steps", &["3".into()]),
            "List exactly 3 next steps"
        );
    }

    #[test]
    fn substitute_positional_default_when_empty() {
        assert_eq!(
            substitute_args("Mode: ${1:-brief}", &["".into()]),
            "Mode: brief"
        );
    }

    #[test]
    fn substitute_array_slice_from_n() {
        assert_eq!(
            substitute_args("${@:2}", &["a".into(), "b".into(), "c".into(), "d".into()]),
            "b c d"
        );
    }

    #[test]
    fn substitute_array_slice_with_length() {
        assert_eq!(
            substitute_args("${@:2:2}", &["a".into(), "b".into(), "c".into(), "d".into()]),
            "b c"
        );
    }

    #[test]
    fn substitute_array_slice_zero_as_one() {
        assert_eq!(
            substitute_args("${@:0}", &["a".into(), "b".into(), "c".into()]),
            "a b c"
        );
    }

    #[test]
    fn substitute_empty_args_array() {
        let empty: Vec<String> = Vec::new();
        assert_eq!(substitute_args("Test: $ARGUMENTS", &empty), "Test: ");
        assert_eq!(substitute_args("Test: $@", &empty), "Test: ");
        assert_eq!(substitute_args("Test: $1", &empty), "Test: ");
    }

    #[test]
    fn substitute_multiple_occurrences() {
        assert_eq!(
            substitute_args("$ARGUMENTS and $ARGUMENTS", &["a".into(), "b".into()]),
            "a b and a b"
        );
        assert_eq!(
            substitute_args("$@ and $@", &["a".into(), "b".into()]),
            "a b and a b"
        );
    }

    #[test]
    fn substitute_out_of_range_numbered() {
        assert_eq!(
            substitute_args("$1 $2 $3 $4 $5", &["a".into(), "b".into()]),
            "a b   "
        );
    }

    #[test]
    fn substitute_non_matching_patterns_pass_through() {
        assert_eq!(
            substitute_args("$A $$ $ $ARGS", &["a".into()]),
            "$A $$ $ $ARGS"
        );
        // Plain ${1} (without :-default) is NOT substituted
        assert_eq!(
            substitute_args("${1}", &["a".into()]),
            "${1}"
        );
    }

    #[test]
    fn substitute_case_sensitive() {
        assert_eq!(
            substitute_args("$arguments $Arguments $ARGUMENTS", &["a".into(), "b".into()]),
            "$arguments $Arguments a b"
        );
    }

    #[test]
    fn substitute_unicode() {
        assert_eq!(
            substitute_args("$ARGUMENTS", &["日本語".into(), "🎉".into(), "café".into()]),
            "日本語 🎉 café"
        );
    }

    #[test]
    fn xml_escape_works() {
        let escaped = xml_escape("<tag attr=\"val\">&amp;");
        assert_eq!(escaped, "&lt;tag attr=&quot;val&quot;&gt;&amp;amp;");
    }
}
