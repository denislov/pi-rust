use unicode_segmentation::UnicodeSegmentation;

use crate::kill_ring::KillRing;
use crate::undo_stack::UndoStack;
use crate::utils::ansi_sequence_len;
use crate::word_navigation::{find_word_backward, find_word_forward};
use crate::{
    CURSOR_MARKER, Component, InputEvent, Key, KeyEventKind, KeyModifiers, KeybindingsManager,
    visible_width,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct EditorSnapshot {
    text: String,
    cursor: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LastAction {
    Kill,
    Yank,
    TypeWord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VisualLine {
    start: usize,
    end: usize,
}

pub struct Editor {
    text: String,
    cursor: usize,
    focused: bool,
    last_render_width: usize,
    keybindings: KeybindingsManager,
    kill_ring: KillRing,
    undo_stack: UndoStack<EditorSnapshot>,
    redo_stack: UndoStack<EditorSnapshot>,
    last_action: Option<LastAction>,
    last_yank: Option<(usize, usize)>,
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
            last_render_width: 80,
            keybindings,
            kill_ring: KillRing::default(),
            undo_stack: UndoStack::default(),
            redo_stack: UndoStack::default(),
            last_action: None,
            last_yank: None,
            on_submit: None,
            on_scroll_page_up: None,
            on_scroll_page_down: None,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cursor = self.text.len();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.last_action = None;
        self.last_yank = None;
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
        if text.is_empty() {
            return;
        }
        let should_push_undo = if is_single_plain_word_grapheme(text) {
            self.last_action != Some(LastAction::TypeWord)
        } else {
            true
        };
        if should_push_undo {
            self.push_undo_snapshot();
        }
        self.insert_without_undo(text);
        self.last_action = if is_single_plain_word_grapheme(text) {
            Some(LastAction::TypeWord)
        } else {
            None
        };
        self.last_yank = None;
    }

    fn insert_without_undo(&mut self, text: &str) {
        self.text.insert_str(self.cursor, text);
        self.cursor += text.len();
    }

    fn submit(&mut self) {
        if let Some(callback) = &mut self.on_submit {
            callback(&self.text);
        }
        self.text.clear();
        self.cursor = 0;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.last_action = None;
        self.last_yank = None;
    }

    fn delete_backward(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.push_undo_snapshot();
        let start = previous_grapheme_boundary(&self.text, self.cursor);
        self.text.replace_range(start..self.cursor, "");
        self.cursor = start;
        self.last_action = None;
        self.last_yank = None;
    }

    fn delete_forward(&mut self) {
        if self.cursor >= self.text.len() {
            return;
        }
        self.push_undo_snapshot();
        let end = next_grapheme_boundary(&self.text, self.cursor);
        self.text.replace_range(self.cursor..end, "");
        self.last_action = None;
        self.last_yank = None;
    }

    fn delete_word_backward(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let start = find_word_backward(&self.text, self.cursor);
        self.kill_range(start, self.cursor, true);
        self.cursor = start;
    }

    fn delete_word_forward(&mut self) {
        if self.cursor >= self.text.len() {
            return;
        }
        let end = find_word_forward(&self.text, self.cursor);
        self.kill_range(self.cursor, end, false);
    }

    fn delete_to_line_start(&mut self) {
        let start = current_line_start(&self.text, self.cursor);
        if start == self.cursor && self.cursor > 0 {
            let newline_start = previous_grapheme_boundary(&self.text, self.cursor);
            self.kill_range(newline_start, self.cursor, true);
            self.cursor = newline_start;
            return;
        }
        self.kill_range(start, self.cursor, true);
        self.cursor = start;
    }

    fn delete_to_line_end(&mut self) {
        let end = current_line_end(&self.text, self.cursor);
        if end == self.cursor && self.cursor < self.text.len() {
            let newline_end = next_grapheme_boundary(&self.text, self.cursor);
            self.kill_range(self.cursor, newline_end, false);
            return;
        }
        self.kill_range(self.cursor, end, false);
    }

    fn kill_range(&mut self, start: usize, end: usize, prepend: bool) {
        if start >= end {
            return;
        }
        self.push_undo_snapshot();
        let deleted = self.text[start..end].to_string();
        let accumulate = self.last_action == Some(LastAction::Kill);
        self.kill_ring.push(deleted, prepend, accumulate);
        self.text.replace_range(start..end, "");
        if self.cursor > end {
            self.cursor -= end - start;
        } else if self.cursor > start {
            self.cursor = start;
        }
        self.last_action = Some(LastAction::Kill);
        self.last_yank = None;
    }

    fn yank(&mut self) {
        let Some(text) = self.kill_ring.yank().map(str::to_string) else {
            return;
        };
        self.push_undo_snapshot();
        let start = self.cursor;
        self.insert_without_undo(&text);
        self.last_yank = Some((start, self.cursor));
        self.last_action = Some(LastAction::Yank);
    }

    fn yank_pop(&mut self) {
        if self.last_action != Some(LastAction::Yank) {
            return;
        }
        let Some((start, end)) = self.last_yank else {
            return;
        };
        let Some(replacement) = self.kill_ring.yank_pop().map(str::to_string) else {
            return;
        };
        self.push_undo_snapshot();
        self.text.replace_range(start..end, "");
        self.cursor = start;
        self.insert_without_undo(&replacement);
        self.last_yank = Some((start, self.cursor));
        self.last_action = Some(LastAction::Yank);
    }

    fn undo(&mut self) {
        let Some(snapshot) = self.undo_stack.pop() else {
            return;
        };
        self.redo_stack.push(self.snapshot());
        self.restore(snapshot);
    }

    fn redo(&mut self) {
        let Some(snapshot) = self.redo_stack.pop() else {
            return;
        };
        self.undo_stack.push(self.snapshot());
        self.restore(snapshot);
    }

    fn push_undo_snapshot(&mut self) {
        self.undo_stack.push(self.snapshot());
        self.redo_stack.clear();
    }

    fn snapshot(&self) -> EditorSnapshot {
        EditorSnapshot {
            text: self.text.clone(),
            cursor: self.cursor,
        }
    }

    fn restore(&mut self, snapshot: EditorSnapshot) {
        self.text = snapshot.text;
        self.cursor = snapshot.cursor.min(self.text.len());
        if !self.text.is_char_boundary(self.cursor) {
            self.cursor = previous_grapheme_boundary(&self.text, self.cursor);
        }
        self.last_action = None;
        self.last_yank = None;
    }

    fn move_left(&mut self) {
        self.cursor = previous_grapheme_boundary(&self.text, self.cursor);
        self.last_action = None;
        self.last_yank = None;
    }

    fn move_right(&mut self) {
        self.cursor = next_grapheme_boundary(&self.text, self.cursor);
        self.last_action = None;
        self.last_yank = None;
    }

    fn move_line_start(&mut self) {
        self.cursor = current_line_start(&self.text, self.cursor);
        self.last_action = None;
        self.last_yank = None;
    }

    fn move_line_end(&mut self) {
        self.cursor = current_line_end(&self.text, self.cursor);
        self.last_action = None;
        self.last_yank = None;
    }

    fn move_word_left(&mut self) {
        self.cursor = find_word_backward(&self.text, self.cursor);
        self.last_action = None;
        self.last_yank = None;
    }

    fn move_word_right(&mut self) {
        self.cursor = find_word_forward(&self.text, self.cursor);
        self.last_action = None;
        self.last_yank = None;
    }

    fn move_up(&mut self) {
        self.move_vertical(-1);
    }

    fn move_down(&mut self) {
        self.move_vertical(1);
    }

    fn move_vertical(&mut self, delta: isize) {
        let lines = visual_lines(&self.text, self.last_render_width.max(1));
        let Some(current_index) = visual_line_at_cursor(&lines, self.cursor, delta) else {
            return;
        };
        let target_index = current_index as isize + delta;
        if target_index < 0 || target_index >= lines.len() as isize {
            return;
        }
        let current_line = lines[current_index];
        let target_line = lines[target_index as usize];
        let desired_col = visible_width(&self.text[current_line.start..self.cursor]);
        self.cursor = cursor_at_visible_col(&self.text, target_line, desired_col);
        self.last_action = None;
        self.last_yank = None;
    }
}

impl Component for Editor {
    fn render(&mut self, width: usize) -> Vec<String> {
        if width == 0 {
            return Vec::new();
        }
        self.last_render_width = width;

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
                if self.keybindings.matches(event, "tui.editor.undo") {
                    self.undo();
                    return;
                }
                if self.keybindings.matches(event, "tui.editor.redo") {
                    self.redo();
                    return;
                }
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
                    .matches(event, "tui.editor.deleteToLineEnd")
                {
                    self.delete_to_line_end();
                    return;
                }
                if self
                    .keybindings
                    .matches(event, "tui.editor.deleteToLineStart")
                {
                    self.delete_to_line_start();
                    return;
                }
                if self
                    .keybindings
                    .matches(event, "tui.editor.deleteWordBackward")
                {
                    self.delete_word_backward();
                    return;
                }
                if self
                    .keybindings
                    .matches(event, "tui.editor.deleteWordForward")
                {
                    self.delete_word_forward();
                    return;
                }
                if self
                    .keybindings
                    .matches(event, "tui.editor.deleteCharBackward")
                {
                    self.delete_backward();
                    return;
                }
                if self
                    .keybindings
                    .matches(event, "tui.editor.deleteCharForward")
                {
                    self.delete_forward();
                    return;
                }
                if self.keybindings.matches(event, "tui.editor.yank") {
                    self.yank();
                    return;
                }
                if self.keybindings.matches(event, "tui.editor.yankPop") {
                    self.yank_pop();
                    return;
                }
                if self
                    .keybindings
                    .matches(event, "tui.editor.cursorLineStart")
                {
                    self.move_line_start();
                    return;
                }
                if self.keybindings.matches(event, "tui.editor.cursorLineEnd") {
                    self.move_line_end();
                    return;
                }
                if self.keybindings.matches(event, "tui.editor.cursorWordLeft") {
                    self.move_word_left();
                    return;
                }
                if self
                    .keybindings
                    .matches(event, "tui.editor.cursorWordRight")
                {
                    self.move_word_right();
                    return;
                }
                if self.keybindings.matches(event, "tui.editor.cursorUp") {
                    self.move_up();
                    return;
                }
                if self.keybindings.matches(event, "tui.editor.cursorDown") {
                    self.move_down();
                    return;
                }
                if self.keybindings.matches(event, "tui.editor.cursorLeft") {
                    self.move_left();
                    return;
                }
                if self.keybindings.matches(event, "tui.editor.cursorRight") {
                    self.move_right();
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
    let mut pos = 0;
    while pos < source.len() {
        if let Some(len) = ansi_sequence_len(source, pos) {
            current.push_str(&source[pos..pos + len]);
            pos += len;
            continue;
        }

        let grapheme = source[pos..]
            .graphemes(true)
            .next()
            .expect("pos is inside source");
        let grapheme_width = visible_width(grapheme);
        if current_width + grapheme_width > width && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current_width = 0;
        }
        current.push_str(grapheme);
        current_width += grapheme_width;
        pos += grapheme.len();
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

fn next_grapheme_boundary(text: &str, cursor: usize) -> usize {
    text[cursor..]
        .grapheme_indices(true)
        .nth(1)
        .map(|(index, _)| cursor + index)
        .unwrap_or(text.len())
}

fn current_line_start(text: &str, cursor: usize) -> usize {
    text[..cursor]
        .rfind('\n')
        .map(|index| index + '\n'.len_utf8())
        .unwrap_or(0)
}

fn current_line_end(text: &str, cursor: usize) -> usize {
    text[cursor..]
        .find('\n')
        .map(|index| cursor + index)
        .unwrap_or(text.len())
}

fn is_single_plain_word_grapheme(text: &str) -> bool {
    let mut graphemes = text.graphemes(true);
    let Some(grapheme) = graphemes.next() else {
        return false;
    };
    graphemes.next().is_none()
        && grapheme
            .chars()
            .all(|ch| !ch.is_whitespace() && !ch.is_ascii_punctuation())
}

fn visual_lines(text: &str, width: usize) -> Vec<VisualLine> {
    let mut lines = Vec::new();
    let mut source_start = 0;

    for source_line in text.split_inclusive('\n') {
        let content = source_line.strip_suffix('\n').unwrap_or(source_line);
        push_visual_line_ranges(text, source_start, content.len(), width, &mut lines);
        source_start += source_line.len();
    }

    if text.is_empty() || text.ends_with('\n') {
        lines.push(VisualLine {
            start: text.len(),
            end: text.len(),
        });
    }

    lines
}

fn visual_line_at_cursor(lines: &[VisualLine], cursor: usize, delta: isize) -> Option<usize> {
    lines.iter().enumerate().position(|(index, line)| {
        cursor >= line.start
            && (cursor < line.end
                || (cursor == line.end
                    && (delta > 0 || index + 1 == lines.len() || lines[index + 1].start != cursor)))
    })
}

fn push_visual_line_ranges(
    text: &str,
    source_start: usize,
    source_len: usize,
    width: usize,
    lines: &mut Vec<VisualLine>,
) {
    if source_len == 0 {
        lines.push(VisualLine {
            start: source_start,
            end: source_start,
        });
        return;
    }

    let source_end = source_start + source_len;
    let mut line_start = source_start;
    let mut current_width = 0;
    let mut pos = source_start;
    while pos < source_end {
        if let Some(len) = ansi_sequence_len(text, pos) {
            pos += len;
            continue;
        }

        let grapheme = text[pos..source_end]
            .graphemes(true)
            .next()
            .expect("pos is inside source");
        let grapheme_width = visible_width(grapheme);
        if current_width + grapheme_width > width && line_start < pos {
            lines.push(VisualLine {
                start: line_start,
                end: pos,
            });
            line_start = pos;
            current_width = 0;
        }
        current_width += grapheme_width;
        pos += grapheme.len();
    }

    lines.push(VisualLine {
        start: line_start,
        end: source_end,
    });
}

fn cursor_at_visible_col(text: &str, line: VisualLine, desired_col: usize) -> usize {
    let mut current_col = 0;
    let mut pos = line.start;
    while pos < line.end {
        if let Some(len) = ansi_sequence_len(text, pos) {
            pos += len;
            continue;
        }

        let grapheme = text[pos..line.end]
            .graphemes(true)
            .next()
            .expect("pos is inside visual line");
        let next_col = current_col + visible_width(grapheme);
        if next_col > desired_col {
            return pos;
        }
        current_col = next_col;
        pos += grapheme.len();
    }
    line.end
}
