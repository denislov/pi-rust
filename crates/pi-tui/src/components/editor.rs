use unicode_segmentation::UnicodeSegmentation;

use crate::{
    CURSOR_MARKER, Component, InputEvent, Key, KeyEventKind, KeyModifiers, KeybindingsManager,
    visible_width,
};

pub struct Editor {
    text: String,
    cursor: usize,
    focused: bool,
    keybindings: KeybindingsManager,
    on_submit: Option<Box<dyn FnMut(&str)>>,
    on_scroll_page_up: Option<Box<dyn FnMut()>>,
    on_scroll_page_down: Option<Box<dyn FnMut()>>,
}

impl Editor {
    pub fn new(keybindings: KeybindingsManager) -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            focused: false,
            keybindings,
            on_submit: None,
            on_scroll_page_up: None,
            on_scroll_page_down: None,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cursor = self.text.len();
    }

    pub fn set_on_submit(&mut self, callback: Box<dyn FnMut(&str)>) {
        self.on_submit = Some(callback);
    }

    pub fn set_on_scroll_page_up(&mut self, callback: Box<dyn FnMut()>) {
        self.on_scroll_page_up = Some(callback);
    }

    pub fn set_on_scroll_page_down(&mut self, callback: Box<dyn FnMut()>) {
        self.on_scroll_page_down = Some(callback);
    }

    fn insert(&mut self, text: &str) {
        self.text.insert_str(self.cursor, text);
        self.cursor += text.len();
    }

    fn submit(&mut self) {
        if let Some(callback) = &mut self.on_submit {
            callback(&self.text);
        }
        self.text.clear();
        self.cursor = 0;
    }

    fn delete_backward(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let start = previous_grapheme_boundary(&self.text, self.cursor);
        self.text.replace_range(start..self.cursor, "");
        self.cursor = start;
    }
}

impl Component for Editor {
    fn render(&mut self, width: usize) -> Vec<String> {
        if width == 0 {
            return Vec::new();
        }

        let mut text = self.text.clone();
        if self.focused {
            text.insert_str(self.cursor, CURSOR_MARKER);
        }
        wrap_multiline(&text, width)
    }

    fn handle_input(&mut self, event: &InputEvent) {
        match event {
            InputEvent::Paste(text) => self.insert(text),
            InputEvent::Key(key_event) if key_event.kind != KeyEventKind::Release => {
                if self.keybindings.matches(event, "tui.input.newLine") {
                    self.insert("\n");
                    return;
                }
                if self.keybindings.matches(event, "tui.input.submit") {
                    self.submit();
                    return;
                }
                if self.keybindings.matches(event, "tui.editor.pageUp") {
                    if let Some(callback) = &mut self.on_scroll_page_up {
                        callback();
                    }
                    return;
                }
                if self.keybindings.matches(event, "tui.editor.pageDown") {
                    if let Some(callback) = &mut self.on_scroll_page_down {
                        callback();
                    }
                    return;
                }
                if self
                    .keybindings
                    .matches(event, "tui.editor.deleteCharBackward")
                {
                    self.delete_backward();
                    return;
                }

                if let Key::Char(text) = &key_event.key {
                    if key_event
                        .modifiers
                        .intersects(KeyModifiers::CTRL | KeyModifiers::ALT | KeyModifiers::SUPER)
                    {
                        return;
                    }
                    self.insert(if text == "space" { " " } else { text });
                }
            }
            _ => {}
        }
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn wrap_multiline(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for source_line in text.split('\n') {
        wrap_line(source_line, width, &mut lines);
    }
    if text.ends_with('\n') {
        lines.push(String::new());
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn wrap_line(source: &str, width: usize, lines: &mut Vec<String>) {
    if source.is_empty() {
        lines.push(String::new());
        return;
    }

    let mut current = String::new();
    let mut current_width = 0;
    for grapheme in source.graphemes(true) {
        let grapheme_width = visible_width(grapheme);
        if current_width + grapheme_width > width && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current_width = 0;
        }
        current.push_str(grapheme);
        current_width += grapheme_width;
    }
    if !current.is_empty() {
        lines.push(current);
    }
}

fn previous_grapheme_boundary(text: &str, cursor: usize) -> usize {
    text[..cursor]
        .grapheme_indices(true)
        .last()
        .map(|(index, _)| index)
        .unwrap_or(0)
}
