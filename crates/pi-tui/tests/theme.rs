use pi_tui::{
    Color, Component, KeybindingsManager, Markdown, SelectItem, SelectList, SelectListTheme, Style,
    TUI_KEYBINDINGS, ThemeMode, TuiTheme, dark_theme, light_theme,
};

fn keybindings() -> KeybindingsManager {
    KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default())
}

#[test]
fn built_in_themes_expose_dark_and_light_component_styles() {
    let dark = dark_theme();
    let light = light_theme();

    assert_eq!(dark.mode, ThemeMode::Dark);
    assert_eq!(light.mode, ThemeMode::Light);
    assert_ne!(dark.palette.background, light.palette.background);
    assert_eq!(dark.markdown.heading.bold, true);
    assert_eq!(light.select_list.selected_text.bold, true);
}

#[test]
fn custom_theme_derives_component_themes_from_palette() {
    let theme = TuiTheme::custom(
        "ocean",
        pi_tui::ThemePalette {
            accent: Color::Rgb(20, 120, 200),
            muted: Color::Ansi256(244),
            text: Color::White,
            background: Color::Default,
            error: Color::Red,
            success: Color::Green,
            warning: Color::Yellow,
            path: Color::Cyan,
        },
    );

    assert_eq!(theme.mode, ThemeMode::Custom);
    assert_eq!(theme.select_list.selected_text.fg, Color::Rgb(20, 120, 200));
    assert_eq!(theme.markdown.quote.fg, Color::Ansi256(244));
}

#[test]
fn markdown_accepts_theme_for_heading_and_quote_styles() {
    let mut markdown = Markdown::new("# Heading\n\n> quoted").with_theme(pi_tui::MarkdownTheme {
        heading: Style::fg(Color::Rgb(1, 2, 3)).bold(),
        quote: Style::fg(Color::Ansi256(244)).dim(),
        ..pi_tui::MarkdownTheme::default()
    });

    let rendered = markdown.render(40).join("\n");

    assert_eq!(markdown.theme().heading.fg, Color::Rgb(1, 2, 3));
    assert_eq!(markdown.theme().quote.fg, Color::Ansi256(244));
    assert!(rendered.contains("Heading"), "{rendered:?}");
    assert!(rendered.contains("> quoted"), "{rendered:?}");
}

#[test]
fn select_list_accepts_theme_for_selected_and_description_text() {
    let theme = SelectListTheme {
        selected_prefix: Style::fg(Color::Green).bold(),
        selected_text: Style::fg(Color::Green).bold(),
        description: Style::fg(Color::Ansi256(245)).dim(),
        ..SelectListTheme::default()
    };
    let mut list = SelectList::new(
        vec![SelectItem::new("model", "Model").description("Switch model")],
        5,
        keybindings(),
    )
    .with_theme(theme);

    let line = list.render(80).remove(0);

    assert_eq!(list.theme().selected_text.fg, Color::Green);
    assert_eq!(list.theme().description.fg, Color::Ansi256(245));
    assert!(line.contains("> "), "{line:?}");
    assert!(line.contains("Model"), "{line:?}");
    assert!(line.contains("Switch model"), "{line:?}");
}
