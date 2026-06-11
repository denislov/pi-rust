use pi_tui::{Component, Editor, KeybindingsManager, StdinBuffer, TUI_KEYBINDINGS};
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
