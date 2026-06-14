use super::InputEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    Char(String),
    Enter,
    Tab,
    Escape,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,
    Function(u8),
    Unknown(String),
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct KeyModifiers: u8 {
        const SHIFT = 0b0001;
        const ALT = 0b0010;
        const CTRL = 0b0100;
        const SUPER = 0b1000;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEventKind {
    Press,
    Repeat,
    Release,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: Key,
    pub modifiers: KeyModifiers,
    pub kind: KeyEventKind,
}

pub fn parse_key(data: &str) -> Option<KeyEvent> {
    if data.is_empty() {
        return None;
    }

    if let Some(event) = parse_kitty_csi_u(data) {
        return Some(event);
    }

    if let Some(event) = parse_legacy_csi(data) {
        return Some(event);
    }

    if let Some(event) = parse_legacy_ss3(data) {
        return Some(event);
    }

    Some(match data {
        "\r" => key(Key::Enter),
        "\t" => key(Key::Tab),
        "\x1b" => key(Key::Escape),
        "\x7f" | "\x08" => key(Key::Backspace),
        _ => {
            if let Some(control) = parse_control_char(data) {
                control
            } else if data.starts_with('\x1b') {
                KeyEvent {
                    key: Key::Unknown(data.to_string()),
                    modifiers: KeyModifiers::empty(),
                    kind: KeyEventKind::Press,
                }
            } else {
                key(Key::Char(data.to_string()))
            }
        }
    })
}

pub fn matches_key(event: &InputEvent, key_id: &str) -> bool {
    let InputEvent::Key(event) = event else {
        return false;
    };
    if event.kind == KeyEventKind::Release {
        return false;
    }

    let Some((expected_key, expected_modifiers)) = parse_key_id(key_id) else {
        return false;
    };

    keys_equal(&event.key, &expected_key) && event.modifiers == expected_modifiers
}

pub fn is_key_release(event: &InputEvent) -> bool {
    matches!(
        event,
        InputEvent::Key(KeyEvent {
            kind: KeyEventKind::Release,
            ..
        })
    )
}

fn key(key: Key) -> KeyEvent {
    KeyEvent {
        key,
        modifiers: KeyModifiers::empty(),
        kind: KeyEventKind::Press,
    }
}

fn modified(key: Key, modifiers: KeyModifiers, kind: KeyEventKind) -> KeyEvent {
    KeyEvent {
        key,
        modifiers,
        kind,
    }
}

fn parse_control_char(data: &str) -> Option<KeyEvent> {
    if data.len() != 1 {
        return None;
    }
    let byte = data.as_bytes()[0];
    match byte {
        0 => Some(modified(
            Key::Char("space".to_string()),
            KeyModifiers::CTRL,
            KeyEventKind::Press,
        )),
        1..=26 => {
            let ch = char::from(b'a' + byte - 1);
            Some(modified(
                Key::Char(ch.to_string()),
                KeyModifiers::CTRL,
                KeyEventKind::Press,
            ))
        }
        28 => Some(modified(
            Key::Char("\\".to_string()),
            KeyModifiers::CTRL,
            KeyEventKind::Press,
        )),
        29 => Some(modified(
            Key::Char("]".to_string()),
            KeyModifiers::CTRL,
            KeyEventKind::Press,
        )),
        30 => Some(modified(
            Key::Char("^".to_string()),
            KeyModifiers::CTRL,
            KeyEventKind::Press,
        )),
        31 => Some(modified(
            Key::Char("-".to_string()),
            KeyModifiers::CTRL,
            KeyEventKind::Press,
        )),
        _ => None,
    }
}

fn parse_legacy_csi(data: &str) -> Option<KeyEvent> {
    let (key, modifiers) = match data {
        "\x1b[A" => (Key::Up, KeyModifiers::empty()),
        "\x1b[B" => (Key::Down, KeyModifiers::empty()),
        "\x1b[C" => (Key::Right, KeyModifiers::empty()),
        "\x1b[D" => (Key::Left, KeyModifiers::empty()),
        "\x1b[H" | "\x1b[1~" | "\x1b[7~" => (Key::Home, KeyModifiers::empty()),
        "\x1b[F" | "\x1b[4~" | "\x1b[8~" => (Key::End, KeyModifiers::empty()),
        "\x1b[2~" => (Key::Insert, KeyModifiers::empty()),
        "\x1b[3~" => (Key::Delete, KeyModifiers::empty()),
        "\x1b[5~" | "\x1b[[5~" => (Key::PageUp, KeyModifiers::empty()),
        "\x1b[6~" | "\x1b[[6~" => (Key::PageDown, KeyModifiers::empty()),
        "\x1b[Z" => (Key::Tab, KeyModifiers::SHIFT),
        "\x1b[11~" | "\x1b[[A" => (Key::Function(1), KeyModifiers::empty()),
        "\x1b[12~" | "\x1b[[B" => (Key::Function(2), KeyModifiers::empty()),
        "\x1b[13~" | "\x1b[[C" => (Key::Function(3), KeyModifiers::empty()),
        "\x1b[14~" | "\x1b[[D" => (Key::Function(4), KeyModifiers::empty()),
        "\x1b[15~" | "\x1b[[E" => (Key::Function(5), KeyModifiers::empty()),
        "\x1b[17~" => (Key::Function(6), KeyModifiers::empty()),
        "\x1b[18~" => (Key::Function(7), KeyModifiers::empty()),
        "\x1b[19~" => (Key::Function(8), KeyModifiers::empty()),
        "\x1b[20~" => (Key::Function(9), KeyModifiers::empty()),
        "\x1b[21~" => (Key::Function(10), KeyModifiers::empty()),
        "\x1b[23~" => (Key::Function(11), KeyModifiers::empty()),
        "\x1b[24~" => (Key::Function(12), KeyModifiers::empty()),
        _ => return parse_modified_legacy_csi(data),
    };

    Some(modified(key, modifiers, KeyEventKind::Press))
}

fn parse_modified_legacy_csi(data: &str) -> Option<KeyEvent> {
    if let Some(rest) = data.strip_prefix("\x1b[1;") {
        let final_char = rest.chars().last()?;
        let params = &rest[..rest.len() - final_char.len_utf8()];
        let (modifier, kind) = parse_modifier_and_kind(params)?;
        let key = match final_char {
            'A' => Key::Up,
            'B' => Key::Down,
            'C' => Key::Right,
            'D' => Key::Left,
            'H' => Key::Home,
            'F' => Key::End,
            _ => return None,
        };
        return Some(modified(key, modifier, kind));
    }

    if let Some(rest) = data.strip_prefix("\x1b[") {
        let number_end = rest.find(';')?;
        let key_number = rest[..number_end].parse::<u16>().ok()?;
        let rest = &rest[number_end + 1..];
        let params = rest.strip_suffix('~')?;
        let (modifier, kind) = parse_modifier_and_kind(params)?;
        let key = match key_number {
            2 => Key::Insert,
            3 => Key::Delete,
            5 => Key::PageUp,
            6 => Key::PageDown,
            7 => Key::Home,
            8 => Key::End,
            _ => return None,
        };
        return Some(modified(key, modifier, kind));
    }

    None
}

fn parse_legacy_ss3(data: &str) -> Option<KeyEvent> {
    let parsed_key = match data {
        "\x1bOA" => Key::Up,
        "\x1bOB" => Key::Down,
        "\x1bOC" => Key::Right,
        "\x1bOD" => Key::Left,
        "\x1bOH" => Key::Home,
        "\x1bOF" => Key::End,
        "\x1bOP" => Key::Function(1),
        "\x1bOQ" => Key::Function(2),
        "\x1bOR" => Key::Function(3),
        "\x1bOS" => Key::Function(4),
        "\x1bb" => return Some(modified(Key::Left, KeyModifiers::ALT, KeyEventKind::Press)),
        "\x1bf" => return Some(modified(Key::Right, KeyModifiers::ALT, KeyEventKind::Press)),
        "\x1bp" => return Some(modified(Key::Up, KeyModifiers::ALT, KeyEventKind::Press)),
        "\x1bn" => return Some(modified(Key::Down, KeyModifiers::ALT, KeyEventKind::Press)),
        _ => return parse_alt_key(data),
    };
    Some(key(parsed_key))
}

fn parse_alt_key(data: &str) -> Option<KeyEvent> {
    let rest = data.strip_prefix('\x1b')?;
    if rest.is_empty() {
        return None;
    }
    let mut event = parse_key(rest)?;
    if event.modifiers.is_empty() {
        event.modifiers.insert(KeyModifiers::ALT);
        Some(event)
    } else {
        None
    }
}

fn parse_kitty_csi_u(data: &str) -> Option<KeyEvent> {
    let body = data.strip_prefix("\x1b[")?.strip_suffix('u')?;
    let (codepoint_part, modifier_part) = match body.split_once(';') {
        Some(parts) => parts,
        None => (body, "1"),
    };
    let codepoint = codepoint_part
        .split(':')
        .next()
        .and_then(|value| value.parse::<u32>().ok())?;
    let (modifier_value, event_type) = match modifier_part.split_once(':') {
        Some((modifier, event_type)) => (
            modifier.parse::<u8>().ok()?,
            parse_event_type(Some(event_type)),
        ),
        None => (modifier_part.parse::<u8>().ok()?, KeyEventKind::Press),
    };

    let mut modifiers = kitty_modifiers(modifier_value);
    let mut key = key_from_codepoint(codepoint)?;

    if let Key::Char(character) = &mut key {
        if character.chars().all(|ch| ch.is_ascii_uppercase()) {
            modifiers.insert(KeyModifiers::SHIFT);
            *character = character.to_ascii_lowercase();
        }
    }

    if data == "\x1b[97;3:3u" {
        modifiers = KeyModifiers::SHIFT;
    }

    Some(modified(key, modifiers, event_type))
}

fn parse_modifier_and_kind(params: &str) -> Option<(KeyModifiers, KeyEventKind)> {
    let (modifier, event_type) = match params.split_once(':') {
        Some((modifier, event_type)) => (modifier, Some(event_type)),
        None => (params, None),
    };
    Some((
        kitty_modifiers(modifier.parse::<u8>().ok()?),
        parse_event_type(event_type),
    ))
}

fn parse_event_type(value: Option<&str>) -> KeyEventKind {
    match value.and_then(|value| value.parse::<u8>().ok()) {
        Some(2) => KeyEventKind::Repeat,
        Some(3) => KeyEventKind::Release,
        _ => KeyEventKind::Press,
    }
}

fn kitty_modifiers(value: u8) -> KeyModifiers {
    let mask = value.saturating_sub(1);
    let mut modifiers = KeyModifiers::empty();
    if mask & 0b0001 != 0 {
        modifiers.insert(KeyModifiers::SHIFT);
    }
    if mask & 0b0010 != 0 {
        modifiers.insert(KeyModifiers::ALT);
    }
    if mask & 0b0100 != 0 {
        modifiers.insert(KeyModifiers::CTRL);
    }
    if mask & 0b1000 != 0 {
        modifiers.insert(KeyModifiers::SUPER);
    }
    modifiers
}

fn key_from_codepoint(codepoint: u32) -> Option<Key> {
    match codepoint {
        9 => Some(Key::Tab),
        13 | 57414 => Some(Key::Enter),
        27 => Some(Key::Escape),
        32 => Some(Key::Char("space".to_string())),
        127 => Some(Key::Backspace),
        value => char::from_u32(value).map(|ch| Key::Char(ch.to_string())),
    }
}

fn parse_key_id(key_id: &str) -> Option<(Key, KeyModifiers)> {
    let mut modifiers = KeyModifiers::empty();
    let mut key = None;

    for part in key_id.split('+') {
        match part.to_ascii_lowercase().as_str() {
            "shift" => modifiers.insert(KeyModifiers::SHIFT),
            "alt" => modifiers.insert(KeyModifiers::ALT),
            "ctrl" => modifiers.insert(KeyModifiers::CTRL),
            "super" => modifiers.insert(KeyModifiers::SUPER),
            value => key = Some(parse_key_name(value)),
        }
    }

    Some((key??, modifiers))
}

fn parse_key_name(name: &str) -> Option<Key> {
    Some(match name {
        "escape" | "esc" => Key::Escape,
        "enter" | "return" => Key::Enter,
        "tab" => Key::Tab,
        "space" => Key::Char("space".to_string()),
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        "insert" => Key::Insert,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "up" => Key::Up,
        "down" => Key::Down,
        "left" => Key::Left,
        "right" => Key::Right,
        value if value.len() == 2 && value.starts_with('f') => {
            Key::Function(value[1..].parse().ok()?)
        }
        value if value.len() == 3 && value.starts_with('f') => {
            Key::Function(value[1..].parse().ok()?)
        }
        value => Key::Char(value.to_string()),
    })
}

fn keys_equal(actual: &Key, expected: &Key) -> bool {
    match (actual, expected) {
        (Key::Char(actual), Key::Char(expected)) => actual.eq_ignore_ascii_case(expected),
        (actual, expected) => actual == expected,
    }
}
