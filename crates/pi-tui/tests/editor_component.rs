use pi_tui::{
    CURSOR_MARKER, Component, Editor, KeybindingsManager, StdinBuffer, TUI_KEYBINDINGS,
    extract_cursor_marker,
};
use std::sync::{Arc, Mutex};

fn feed(editor: &mut Editor, data: &str) {
    let mut buffer = StdinBuffer::new();
    for event in buffer.process(data) {
        editor.handle_input(&event);
    }
}

#[test]
fn editor_shift_enter_inserts_newline_and_enter_submits() {
    let mut editor = Editor::new(KeybindingsManager::new(
        TUI_KEYBINDINGS.clone(),
        Default::default(),
    ));
    feed(&mut editor, "hello");
    feed(&mut editor, "\x1b[13;2u");
    feed(&mut editor, "world");
    assert_eq!(editor.text(), "hello\nworld");

    let submitted = Arc::new(Mutex::new(None));
    let submitted_for_callback = Arc::clone(&submitted);
    editor.set_on_submit(Box::new(move |text| {
        *submitted_for_callback.lock().unwrap() = Some(text.to_string());
    }));
    feed(&mut editor, "\r");
    assert_eq!(submitted.lock().unwrap().as_deref(), Some("hello\nworld"));
    assert_eq!(editor.text(), "");
}

#[test]
fn editor_wraps_to_width_and_keeps_lines_bounded() {
    let mut editor = Editor::new(KeybindingsManager::new(
        TUI_KEYBINDINGS.clone(),
        Default::default(),
    ));
    feed(&mut editor, "abcdef");
    for line in editor.render(4) {
        assert!(pi_tui::visible_width(&line) <= 4);
    }
}

#[test]
fn editor_treats_cursor_marker_as_zero_width_when_wrapping() {
    let mut editor = Editor::new(KeybindingsManager::new(
        TUI_KEYBINDINGS.clone(),
        Default::default(),
    ));
    editor.set_focused(true);
    feed(&mut editor, "abcd");

    let lines = editor.render(4);

    assert_eq!(lines, vec![format!("abcd{CURSOR_MARKER}")]);
    assert_eq!(pi_tui::visible_width(&lines[0]), 4);
}

#[test]
fn editor_left_and_right_move_cursor_marker_by_grapheme() {
    let mut editor = Editor::new(KeybindingsManager::new(
        TUI_KEYBINDINGS.clone(),
        Default::default(),
    ));
    editor.set_focused(true);
    feed(&mut editor, "a好");
    feed(&mut editor, "\x1b[D");
    let mut lines = editor.render(10);
    let cursor = extract_cursor_marker(&mut lines, 10).unwrap();
    assert_eq!(cursor.col, 1);
    assert_eq!(lines, vec!["a好".to_string()]);

    feed(&mut editor, "\x1b[C");
    let mut lines = editor.render(10);
    let cursor = extract_cursor_marker(&mut lines, 10).unwrap();
    assert_eq!(cursor.col, 3);
    assert_eq!(lines, vec!["a好".to_string()]);
}
