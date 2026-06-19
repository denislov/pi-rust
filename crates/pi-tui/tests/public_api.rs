use pi_tui::{
    AutocompleteItem, AutocompleteOptions, Box as TuiBox, CancellableLoader, Component, Container,
    KeybindingsManager, Loader, ProcessTerminal, SettingItem, SettingsList, SettingsListOptions,
    SlashCommand, Spacer, TUI_KEYBINDINGS, Terminal, TerminalSize, Text, TruncatedText, Tui,
    VirtualTerminal, color_level, detect_color_level_from_env, paint_with_level, visible_width,
};

#[test]
fn public_api_symbols_are_importable() {
    assert_eq!(visible_width("abc"), 3);

    let mut container = Container::new();
    container.add_child(Box::new(Text::new("hello")));
    container.add_child(Box::new(Spacer::new(1)));
    let lines = container.render(20);
    assert_eq!(lines, vec!["hello".to_string(), "".to_string()]);

    let terminal = VirtualTerminal::new(20, 5);
    let tui = Tui::new(terminal);
    assert_eq!(
        tui.terminal().size(),
        TerminalSize {
            columns: 20,
            rows: 5
        }
    );

    let mut loader = Loader::new("Loading");
    loader.tick();
    let _ = CancellableLoader::new(
        loader,
        KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default()),
    );

    let mut panel = TuiBox::new();
    panel.add_child(std::boxed::Box::new(TruncatedText::new("Loading")));
    let _ = panel.render(20);

    let _ = SettingsList::new(
        vec![SettingItem::new("theme", "Theme", "dark")],
        5,
        KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default()),
    );
    let _ = SettingsListOptions::default();

    let provider = pi_tui::CombinedAutocompleteProvider::new(
        vec![SlashCommand::new("model")],
        std::path::Path::new("."),
    );
    let _ = provider.get_suggestions(&["/m".to_string()], 0, 2, AutocompleteOptions::default());
    let _ = AutocompleteItem::new("model", "model");

    let _ = paint_with_level(
        "x",
        &pi_tui::Style::fg(pi_tui::Color::Ansi256(1)),
        pi_tui::ColorLevel::Ansi256,
    );
    let _ = detect_color_level_from_env([("TERM", "xterm-256color")]);
    let _ = color_level();

    let _ = std::mem::size_of::<ProcessTerminal>();
}
