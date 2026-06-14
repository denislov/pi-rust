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

#[test]
fn editor_home_end_and_forward_delete_update_cursor_and_text() {
    let mut editor = Editor::new(KeybindingsManager::new(
        TUI_KEYBINDINGS.clone(),
        Default::default(),
    ));
    editor.set_focused(true);
    feed(&mut editor, "hello");
    feed(&mut editor, "\x01");
    assert_eq!(editor.cursor(), 0);

    feed(&mut editor, "\x1b[3~");
    assert_eq!(editor.text(), "ello");
    assert_eq!(editor.cursor(), 0);

    feed(&mut editor, "\x05");
    assert_eq!(editor.cursor(), editor.text().len());

    feed(&mut editor, "\x1b[3~");
    assert_eq!(editor.text(), "ello");
}

#[test]
fn editor_word_navigation_and_deletion_are_grapheme_safe() {
    let mut editor = Editor::new(KeybindingsManager::new(
        TUI_KEYBINDINGS.clone(),
        Default::default(),
    ));
    feed(&mut editor, "foo 好😀 bar");

    feed(&mut editor, "\x1b[1;5D");
    assert_eq!(editor.cursor(), "foo 好😀 ".len());
    feed(&mut editor, "\x17");
    assert_eq!(editor.text(), "foo bar");
    assert_eq!(editor.cursor(), "foo ".len());

    feed(&mut editor, "\x1b[1;5C");
    assert_eq!(editor.cursor(), "foo bar".len());
}

#[test]
fn editor_kill_line_and_yank_round_trip_multiline_text() {
    let mut editor = Editor::new(KeybindingsManager::new(
        TUI_KEYBINDINGS.clone(),
        Default::default(),
    ));
    feed(&mut editor, "alpha");
    feed(&mut editor, "\x1b[13;2u");
    feed(&mut editor, "beta");

    feed(&mut editor, "\x01");
    feed(&mut editor, "\x0b");
    assert_eq!(editor.text(), "alpha\n");

    feed(&mut editor, "\x19");
    assert_eq!(editor.text(), "alpha\nbeta");
}

#[test]
fn editor_undo_and_redo_restore_text_and_cursor() {
    let mut editor = Editor::new(KeybindingsManager::new(
        TUI_KEYBINDINGS.clone(),
        Default::default(),
    ));
    feed(&mut editor, "ab");
    assert_eq!(editor.text(), "ab");

    feed(&mut editor, "\x1b[45;5u");
    assert_eq!(editor.text(), "");
    assert_eq!(editor.cursor(), 0);

    feed(&mut editor, "\x1b[45;6u");
    assert_eq!(editor.text(), "ab");
    assert_eq!(editor.cursor(), "ab".len());
}

#[test]
fn editor_yank_pop_replaces_previous_yank_in_place() {
    let mut editor = Editor::new(KeybindingsManager::new(
        TUI_KEYBINDINGS.clone(),
        Default::default(),
    ));
    feed(&mut editor, "first");
    feed(&mut editor, "\x17");
    feed(&mut editor, "second");
    feed(&mut editor, "\x17");

    feed(&mut editor, "hello world");
    feed(&mut editor, "\x01");
    for _ in 0..6 {
        feed(&mut editor, "\x1b[C");
    }

    feed(&mut editor, "\x19");
    assert_eq!(editor.text(), "hello secondworld");
    feed(&mut editor, "\x1by");
    assert_eq!(editor.text(), "hello firstworld");
}

#[test]
fn editor_word_forward_delete_and_line_start_delete_update_kill_ring() {
    let mut editor = Editor::new(KeybindingsManager::new(
        TUI_KEYBINDINGS.clone(),
        Default::default(),
    ));
    feed(&mut editor, "hello world");
    feed(&mut editor, "\x01");
    feed(&mut editor, "\x1bd");
    feed(&mut editor, "\x1bd");
    assert_eq!(editor.text(), "");

    feed(&mut editor, "\x19");
    assert_eq!(editor.text(), "hello world");

    editor.set_text("hello world");
    feed(&mut editor, "\x05");
    feed(&mut editor, "\x15");
    assert_eq!(editor.text(), "");

    feed(&mut editor, "\x01");

    feed(&mut editor, "\x19");
    assert_eq!(editor.text(), "hello world");
}

#[test]
fn editor_multiline_paste_is_inserted_and_undone_atomically() {
    let mut editor = Editor::new(KeybindingsManager::new(
        TUI_KEYBINDINGS.clone(),
        Default::default(),
    ));
    feed(&mut editor, "pre ");
    feed(&mut editor, "\x1b[200~one\ntwo\x1b[201~");
    assert_eq!(editor.text(), "pre one\ntwo");

    feed(&mut editor, "\x1b[45;5u");
    assert_eq!(editor.text(), "pre ");
    assert_eq!(editor.cursor(), "pre ".len());
}

#[test]
fn editor_up_and_down_move_between_logical_lines_by_visible_column() {
    let mut editor = Editor::new(KeybindingsManager::new(
        TUI_KEYBINDINGS.clone(),
        Default::default(),
    ));
    feed(&mut editor, "abc");
    feed(&mut editor, "\x1b[13;2u");
    feed(&mut editor, "de");

    feed(&mut editor, "\x1b[A");
    assert_eq!(editor.cursor(), "ab".len());

    feed(&mut editor, "\x1b[B");
    assert_eq!(editor.cursor(), "abc\n".len() + "de".len());
}

#[test]
fn editor_up_and_down_move_across_wrapped_visual_lines() {
    let mut editor = Editor::new(KeybindingsManager::new(
        TUI_KEYBINDINGS.clone(),
        Default::default(),
    ));
    feed(&mut editor, "abcdefgh");
    editor.render(4);

    feed(&mut editor, "\x1b[A");
    assert_eq!(editor.cursor(), 4);

    feed(&mut editor, "\x1b[B");
    assert_eq!(editor.cursor(), 8);
}

#[test]
fn editor_up_from_wrapped_line_start_moves_to_previous_visual_line() {
    let mut editor = Editor::new(KeybindingsManager::new(
        TUI_KEYBINDINGS.clone(),
        Default::default(),
    ));
    feed(&mut editor, "abcdefgh");
    editor.render(4);
    for _ in 0..4 {
        feed(&mut editor, "\x1b[D");
    }
    assert_eq!(editor.cursor(), 4);

    feed(&mut editor, "\x1b[A");
    assert_eq!(editor.cursor(), 0);
}

#[test]
fn editor_set_text_resets_stale_undo_history() {
    let mut editor = Editor::new(KeybindingsManager::new(
        TUI_KEYBINDINGS.clone(),
        Default::default(),
    ));
    feed(&mut editor, "a b");
    editor.set_text("");

    feed(&mut editor, "\x1b[45;5u");
    assert_eq!(editor.text(), "");
    assert_eq!(editor.cursor(), 0);
}
