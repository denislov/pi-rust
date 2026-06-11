use super::{InputEvent, parse_key};

const ESC: &str = "\x1b";
const BRACKETED_PASTE_START: &str = "\x1b[200~";
const BRACKETED_PASTE_END: &str = "\x1b[201~";

#[derive(Debug, Clone, Default)]
pub struct StdinBuffer {
    buffer: String,
    paste_buffer: String,
    in_paste: bool,
}

impl StdinBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn process(&mut self, data: &str) -> Vec<InputEvent> {
        self.buffer.push_str(data);
        let mut events = Vec::new();

        loop {
            if self.in_paste {
                if let Some(end_index) = self.buffer.find(BRACKETED_PASTE_END) {
                    self.paste_buffer.push_str(&self.buffer[..end_index]);
                    let remainder_start = end_index + BRACKETED_PASTE_END.len();
                    let remainder = self.buffer[remainder_start..].to_string();
                    self.buffer.clear();
                    self.buffer.push_str(&remainder);
                    self.in_paste = false;
                    events.push(InputEvent::Paste(std::mem::take(&mut self.paste_buffer)));
                    continue;
                }

                self.paste_buffer.push_str(&self.buffer);
                self.buffer.clear();
                break;
            }

            if self.buffer.is_empty() {
                break;
            }

            if self.buffer.starts_with(BRACKETED_PASTE_START) {
                let remainder = self.buffer[BRACKETED_PASTE_START.len()..].to_string();
                self.buffer.clear();
                self.buffer.push_str(&remainder);
                self.in_paste = true;
                continue;
            }

            let Some(sequence_len) = next_sequence_len(&self.buffer) else {
                break;
            };
            let sequence = self.buffer[..sequence_len].to_string();
            let remainder = self.buffer[sequence_len..].to_string();
            self.buffer.clear();
            self.buffer.push_str(&remainder);

            events.push(
                parse_key(&sequence)
                    .map(InputEvent::Key)
                    .unwrap_or_else(|| InputEvent::Raw(sequence)),
            );
        }

        events
    }

    pub fn flush(&mut self) -> Vec<InputEvent> {
        let mut events = Vec::new();
        if self.in_paste {
            events.push(InputEvent::Paste(std::mem::take(&mut self.paste_buffer)));
            self.in_paste = false;
        }
        if !self.buffer.is_empty() {
            let data = std::mem::take(&mut self.buffer);
            events.push(
                parse_key(&data)
                    .map(InputEvent::Key)
                    .unwrap_or(InputEvent::Raw(data)),
            );
        }
        events
    }
}

fn next_sequence_len(buffer: &str) -> Option<usize> {
    if !buffer.starts_with(ESC) {
        return buffer.chars().next().map(char::len_utf8);
    }

    if buffer.len() == ESC.len() {
        return None;
    }

    if buffer.starts_with("\x1b[") {
        return csi_sequence_len(buffer);
    }
    if buffer.starts_with("\x1b]") {
        return osc_sequence_len(buffer);
    }
    if buffer.starts_with("\x1bP") || buffer.starts_with("\x1b_") {
        return string_terminated_sequence_len(buffer);
    }
    if buffer.starts_with("\x1bO") {
        return nth_char_end(buffer, 3);
    }

    nth_char_end(buffer, 2)
}

fn csi_sequence_len(buffer: &str) -> Option<usize> {
    if buffer.len() < 3 {
        return None;
    }

    for (index, byte) in buffer.as_bytes().iter().enumerate().skip(2) {
        if (0x40..=0x7e).contains(byte) {
            return Some(index + 1);
        }
    }
    None
}

fn osc_sequence_len(buffer: &str) -> Option<usize> {
    if let Some(index) = buffer.find('\x07') {
        return Some(index + 1);
    }
    string_terminated_sequence_len(buffer)
}

fn string_terminated_sequence_len(buffer: &str) -> Option<usize> {
    buffer.find("\x1b\\").map(|index| index + 2)
}

fn nth_char_end(buffer: &str, count: usize) -> Option<usize> {
    let mut end = 0;
    for (index, ch) in buffer.char_indices().take(count) {
        end = index + ch.len_utf8();
    }
    if buffer.chars().count() >= count {
        Some(end)
    } else {
        None
    }
}
