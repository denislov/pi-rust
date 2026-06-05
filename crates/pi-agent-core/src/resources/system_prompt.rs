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

pub fn format_prompt_template_invocation(_name: &str, content: &str, args: &[String]) -> String {
    let mut result = content.to_string();

    for (i, arg) in args.iter().enumerate() {
        let idx = i + 1;
        // Replace ${1}, ${2}, etc.
        let placeholder = format!("${{{}}}", idx);
        result = result.replace(&placeholder, arg);
        // Replace $1, $2, etc.
        let placeholder = format!("${}", idx);
        result = result.replace(&placeholder, arg);
    }

    result
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
    fn template_invocation_replaces_args() {
        let result = format_prompt_template_invocation(
            "review",
            "Review $1 and ${2}",
            &["foo".to_string(), "bar".to_string()],
        );
        assert!(result.contains("Review foo and bar"));
        assert!(!result.contains("$1"));
        assert!(!result.contains("${2}"));
    }

    #[test]
    fn xml_escape_works() {
        let escaped = xml_escape("<tag attr=\"val\">&amp;");
        assert_eq!(escaped, "&lt;tag attr=&quot;val&quot;&gt;&amp;amp;");
    }
}
