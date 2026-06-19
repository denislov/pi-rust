use pi_tui::{
    CancellableLoader, Component, Container, KeybindingsManager, Loader, ProcessTerminal, Spacer,
    TUI_KEYBINDINGS, Terminal, TerminalSize, Text, Tui, VirtualTerminal, visible_width,
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

    let _ = std::mem::size_of::<ProcessTerminal>();
}
