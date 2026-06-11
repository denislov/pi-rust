use pi_tui::{Component, KeybindingsManager, SelectItem, SelectList, StdinBuffer, TUI_KEYBINDINGS};

fn feed(list: &mut SelectList, data: &str) {
    let mut buffer = StdinBuffer::new();
    for event in buffer.process(data) {
        list.handle_input(&event);
    }
}

#[test]
fn select_list_wraps_selection_and_filters_items() {
    let keybindings = KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default());
    let mut list = SelectList::new(
        vec![
            SelectItem::new("read", "read").description("Read a file"),
            SelectItem::new("write", "write").description("Write a file"),
        ],
        5,
        keybindings,
    );

    feed(&mut list, "\x1b[A");
    assert_eq!(list.selected_item().unwrap().value, "write");
    list.set_filter("r");
    assert_eq!(list.selected_item().unwrap().value, "read");
}

#[test]
fn select_list_renders_bounded_lines() {
    let keybindings = KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default());
    let mut list = SelectList::new(
        vec![
            SelectItem::new("very-long-command-name", "very-long-command-name")
                .description("long description"),
        ],
        5,
        keybindings,
    );

    for line in list.render(12) {
        assert!(pi_tui::visible_width(&line) <= 12);
    }
}
