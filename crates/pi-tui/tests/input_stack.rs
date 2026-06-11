use pi_tui::{InputEvent, Key, KeyEventKind, KeyModifiers, StdinBuffer, matches_key, parse_key};

#[test]
fn stdin_buffer_splits_batched_escape_sequences() {
    let mut buffer = StdinBuffer::new();
    let events = buffer.process("\x1b[A\x1b[Bx");
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0], InputEvent::Key(_)));
    assert!(matches!(events[1], InputEvent::Key(_)));
    assert!(matches!(events[2], InputEvent::Key(_)));
    assert!(matches_key(&events[0], "up"));
    assert!(matches_key(&events[1], "down"));
    assert!(matches_key(&events[2], "x"));
}

#[test]
fn stdin_buffer_waits_for_partial_csi_sequence() {
    let mut buffer = StdinBuffer::new();
    assert!(buffer.process("\x1b[").is_empty());
    let events = buffer.process("A");
    assert_eq!(events.len(), 1);
    assert!(matches_key(&events[0], "up"));
}

#[test]
fn bracketed_paste_is_one_paste_event() {
    let mut buffer = StdinBuffer::new();
    let events = buffer.process("\x1b[200~hello\nworld\x1b[201~");
    assert_eq!(events, vec![InputEvent::Paste("hello\nworld".to_string())]);
}

#[test]
fn parse_legacy_and_kitty_keys() {
    assert!(matches_key(
        &InputEvent::Key(parse_key("\r").unwrap()),
        "enter"
    ));
    assert!(matches_key(
        &InputEvent::Key(parse_key("\x7f").unwrap()),
        "backspace"
    ));
    assert!(matches_key(
        &InputEvent::Key(parse_key("\x1b[3~").unwrap()),
        "delete"
    ));
    assert!(matches_key(
        &InputEvent::Key(parse_key("\x1b[97u").unwrap()),
        "a"
    ));
    assert!(matches_key(
        &InputEvent::Key(parse_key("\x1b[65;5u").unwrap()),
        "ctrl+shift+a"
    ));
}

#[test]
fn kitty_release_events_are_detected() {
    let event = parse_key("\x1b[97;3:3u").unwrap();
    assert_eq!(event.key, Key::Char("a".to_string()));
    assert_eq!(event.kind, KeyEventKind::Release);
    assert_eq!(event.modifiers, KeyModifiers::SHIFT);
}
