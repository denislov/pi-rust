//! Behavior tests for the theme system, mirroring TypeScript
//! `packages/coding-agent/test/test-theme-colors.ts` and `theme/theme.ts`.

use pi_coding_agent::theme::{ColorValue, ResolveError, ResolvedColor, builtin_dark, resolve};
use serde_json::json;
use std::collections::HashMap;

#[test]
fn parses_color_value_formats() {
    use ColorValue::*;
    assert_eq!(
        ColorValue::parse(&json!("#ff0000")),
        Some(Hex(0xff, 0x00, 0x00))
    );
    assert_eq!(
        ColorValue::parse(&json!("#FFAA00")),
        Some(Hex(0xff, 0xaa, 0x00))
    );
    assert_eq!(ColorValue::parse(&json!(0)), Some(Ansi256(0)));
    assert_eq!(ColorValue::parse(&json!(255)), Some(Ansi256(255)));
    assert_eq!(ColorValue::parse(&json!("")), Some(Default));
    assert_eq!(
        ColorValue::parse(&json!("primary")),
        Some(Var("primary".into()))
    );
    assert_eq!(ColorValue::parse(&json!(256)), None);
    assert_eq!(ColorValue::parse(&json!("#fff")), None);
    assert_eq!(ColorValue::parse(&json!(true)), None);
}

#[test]
fn resolves_variable_references_recursively() {
    let vars = HashMap::from([
        ("primary".to_string(), ColorValue::Hex(0x00, 0xaa, 0xff)),
        ("muted".to_string(), ColorValue::Var("primary".to_string())),
        ("gray".to_string(), ColorValue::Ansi256(242)),
    ]);

    assert_eq!(
        resolve(&ColorValue::Var("primary".into()), &vars),
        Ok(ResolvedColor::Hex(0x00, 0xaa, 0xff))
    );
    assert_eq!(
        resolve(&ColorValue::Var("muted".into()), &vars),
        Ok(ResolvedColor::Hex(0x00, 0xaa, 0xff))
    );
    assert_eq!(
        resolve(&ColorValue::Hex(1, 2, 3), &vars),
        Ok(ResolvedColor::Hex(1, 2, 3))
    );
    assert_eq!(
        resolve(&ColorValue::Ansi256(39), &vars),
        Ok(ResolvedColor::Ansi256(39))
    );
    assert_eq!(
        resolve(&ColorValue::Default, &vars),
        Ok(ResolvedColor::Default)
    );
    assert_eq!(
        resolve(&ColorValue::Var("missing".into()), &vars),
        Err(ResolveError::UnknownVar("missing".into()))
    );

    let circular = HashMap::from([
        ("a".to_string(), ColorValue::Var("b".to_string())),
        ("b".to_string(), ColorValue::Var("a".to_string())),
    ]);
    assert_eq!(
        resolve(&ColorValue::Var("a".into()), &circular),
        Err(ResolveError::Circular("a".into()))
    );
}

#[test]
fn parses_builtin_dark_theme_structure() {
    let theme = builtin_dark();
    assert_eq!(theme.name, "dark");
    assert!(theme.vars.contains_key("accent"));
    assert!(theme.vars.contains_key("cyan"));
    assert_eq!(theme.colors.len(), 51);
    assert!(theme.colors.contains_key("accent"));
    assert!(theme.colors.contains_key("bashMode"));
    assert!(theme.colors.contains_key("thinkingXhigh"));
    assert!(theme.colors.contains_key("userMessageBg"));
    let export = theme.export.expect("dark theme has export section");
    assert_eq!(export.page_bg, Some(ColorValue::Hex(0x18, 0x18, 0x1e)));
    assert_eq!(export.card_bg, Some(ColorValue::Hex(0x1e, 0x1e, 0x24)));
    assert_eq!(export.info_bg, Some(ColorValue::Hex(0x3c, 0x37, 0x28)));
}

#[test]
fn resolves_builtin_dark_colors_via_vars() {
    use pi_coding_agent::theme::{ThemeBg, ThemeColor};
    let theme = builtin_dark();
    let resolved = theme.resolve_colors().expect("dark theme resolves");

    // accent -> "accent" var -> "#8abeb7"
    assert_eq!(
        resolved.fg(ThemeColor::Accent),
        ResolvedColor::Hex(0x8a, 0xbe, 0xb7)
    );
    // border -> "blue" var -> "#5f87ff"
    assert_eq!(
        resolved.fg(ThemeColor::Border),
        ResolvedColor::Hex(0x5f, 0x87, 0xff)
    );
    // thinkingMedium is a literal hex (no var) -> "#81a2be"
    assert_eq!(
        resolved.fg(ThemeColor::ThinkingMedium),
        ResolvedColor::Hex(0x81, 0xa2, 0xbe)
    );
    // bashMode -> "green" var -> "#b5bd68"
    assert_eq!(
        resolved.fg(ThemeColor::BashMode),
        ResolvedColor::Hex(0xb5, 0xbd, 0x68)
    );
    // bg token: selectedBg -> "selectedBg" var -> "#3a3a4a"
    assert_eq!(
        resolved.bg(ThemeBg::SelectedBg),
        ResolvedColor::Hex(0x3a, 0x3a, 0x4a)
    );
    // userMessageText -> "text" var -> "#d4d4d4"
    assert_eq!(
        resolved.fg(ThemeColor::UserMessageText),
        ResolvedColor::Hex(0xd4, 0xd4, 0xd4)
    );
}

#[test]
fn reports_missing_required_tokens() {
    let json = r#"{ "name": "broken", "colors": { "accent": 39, "border": 0 } }"#;
    let theme: pi_coding_agent::theme::ThemeJson = serde_json::from_str(json).unwrap();
    let missing = theme.missing_tokens();
    // 51 required - 2 present = 49 missing
    assert_eq!(missing.len(), 49);
    assert!(missing.contains(&"borderAccent"));
    assert!(missing.contains(&"bashMode"));
    assert!(missing.contains(&"thinkingXhigh"));
    // a complete built-in theme has no missing tokens
    assert!(builtin_dark().missing_tokens().is_empty());
}

#[test]
fn resource_loader_parses_ts_theme_format_and_reports_missing_tokens() {
    use pi_coding_agent::resources::{ResourceLoadOptions, load_cli_resources_with_options};
    use pi_coding_agent::theme::DARK_JSON;
    use std::fs;
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("work");
    let agent_dir = temp.path().join("agent");
    let themes = agent_dir.join("themes");
    fs::create_dir_all(&cwd).unwrap();
    fs::create_dir_all(&themes).unwrap();
    // complete theme (built-in dark, renamed) -> no missing-token diagnostic
    let full_json = DARK_JSON.replace(r#""name": "dark""#, r#""name": "full""#);
    fs::write(themes.join("full.json"), full_json).unwrap();
    // incomplete theme -> missing-token diagnostic
    fs::write(
        themes.join("partial.json"),
        r##"{"name":"partial","colors":{"accent":"#00aaff"}}"##,
    )
    .unwrap();

    let loaded =
        load_cli_resources_with_options(&[], &[], &cwd, &agent_dir, ResourceLoadOptions::default())
            .unwrap();

    let full = loaded
        .themes
        .iter()
        .find(|t| t.name == "full")
        .expect("full theme loaded");
    assert_eq!(full.theme.colors.len(), 51);
    let partial = loaded
        .themes
        .iter()
        .find(|t| t.name == "partial")
        .expect("partial theme loaded");
    assert_eq!(partial.theme.colors.len(), 1);
    assert_eq!(partial.theme.missing_tokens().len(), 50);

    // a diagnostic flags the missing tokens for the partial theme
    let diag = loaded
        .diagnostics
        .iter()
        .find(|d| d.path == partial.path)
        .expect("diagnostic for partial theme");
    assert!(
        diag.message.to_lowercase().contains("missing"),
        "diagnostic message was: {}",
        diag.message
    );
}

#[test]
fn parses_builtin_light_theme_and_exposes_schema() {
    let theme = pi_coding_agent::theme::builtin_light();
    assert_eq!(theme.name, "light");
    assert_eq!(theme.colors.len(), 51);
    // light resolves its vars too (accent -> "teal" var -> "#5a8080")
    let resolved = theme.resolve_colors().expect("light theme resolves");
    assert_eq!(
        resolved.fg(pi_coding_agent::theme::ThemeColor::Accent),
        ResolvedColor::Hex(0x5a, 0x80, 0x80)
    );
    // the schema is embedded and parses as valid JSON
    let schema: serde_json::Value =
        serde_json::from_str(pi_coding_agent::theme::SCHEMA_JSON).unwrap();
    assert_eq!(schema["title"], "Pi Coding Agent Theme");
}
