use crate::{
    Component, InputEvent, Key, KeyEventKind, KeybindingsManager, truncate_to_width, visible_width,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectItem {
    pub value: String,
    pub label: String,
    pub description: Option<String>,
}

impl SelectItem {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            description: None,
        }
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

pub struct SelectList {
    items: Vec<SelectItem>,
    filtered_indices: Vec<usize>,
    selected: usize,
    max_visible: usize,
    filter: String,
    keybindings: KeybindingsManager,
    on_confirm: Option<Box<dyn FnMut(&SelectItem)>>,
    on_cancel: Option<Box<dyn FnMut()>>,
}

impl SelectList {
    pub fn new(
        items: Vec<SelectItem>,
        max_visible: usize,
        keybindings: KeybindingsManager,
    ) -> Self {
        let mut list = Self {
            items,
            filtered_indices: Vec::new(),
            selected: 0,
            max_visible,
            filter: String::new(),
            keybindings,
            on_confirm: None,
            on_cancel: None,
        };
        list.rebuild_filter();
        list
    }

    pub fn set_filter(&mut self, filter: impl Into<String>) {
        self.filter = filter.into();
        self.selected = 0;
        self.rebuild_filter();
    }

    pub fn selected_item(&self) -> Option<&SelectItem> {
        self.filtered_indices
            .get(self.selected)
            .and_then(|index| self.items.get(*index))
    }

    pub fn set_on_confirm(&mut self, callback: Box<dyn FnMut(&SelectItem)>) {
        self.on_confirm = Some(callback);
    }

    pub fn set_on_cancel(&mut self, callback: Box<dyn FnMut()>) {
        self.on_cancel = Some(callback);
    }

    fn rebuild_filter(&mut self) {
        let needle = self.filter.to_ascii_lowercase();
        self.filtered_indices = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                if needle.is_empty() || item_matches(item, &needle) {
                    Some(index)
                } else {
                    None
                }
            })
            .collect();
        if self.selected >= self.filtered_indices.len() {
            self.selected = self.filtered_indices.len().saturating_sub(1);
        }
    }

    fn move_selection(&mut self, delta: isize) {
        let len = self.filtered_indices.len();
        if len == 0 {
            self.selected = 0;
            return;
        }
        self.selected = ((self.selected as isize + delta).rem_euclid(len as isize)) as usize;
    }
}

impl Component for SelectList {
    fn render(&mut self, width: usize) -> Vec<String> {
        if width == 0 {
            return Vec::new();
        }

        let mut lines = Vec::new();
        for (visible_index, item_index) in self
            .filtered_indices
            .iter()
            .copied()
            .take(self.max_visible)
            .enumerate()
        {
            let item = &self.items[item_index];
            let marker = if visible_index == self.selected {
                "> "
            } else {
                "  "
            };
            let mut line = format!("{marker}{}", item.label);
            if let Some(description) = &item.description {
                line.push_str(" - ");
                line.push_str(description);
            }
            lines.push(fit_line(&line, width));
        }
        if lines.is_empty() {
            lines.push(String::new());
        }
        lines
    }

    fn handle_input(&mut self, event: &InputEvent) {
        match event {
            InputEvent::Key(key_event) if key_event.kind != KeyEventKind::Release => {
                if self.keybindings.matches(event, "tui.select.up") {
                    self.move_selection(-1);
                    return;
                }
                if self.keybindings.matches(event, "tui.select.down") {
                    self.move_selection(1);
                    return;
                }
                if self.keybindings.matches(event, "tui.select.pageUp") {
                    self.move_selection(-(self.max_visible as isize));
                    return;
                }
                if self.keybindings.matches(event, "tui.select.pageDown") {
                    self.move_selection(self.max_visible as isize);
                    return;
                }
                if self.keybindings.matches(event, "tui.select.confirm") {
                    if let (Some(callback), Some(index)) = (
                        &mut self.on_confirm,
                        self.filtered_indices.get(self.selected),
                    ) {
                        callback(&self.items[*index]);
                    }
                    return;
                }
                if self.keybindings.matches(event, "tui.select.cancel") {
                    if let Some(callback) = &mut self.on_cancel {
                        callback();
                    }
                    return;
                }

                match &key_event.key {
                    Key::Char(text) if text != "space" => {
                        self.filter.push_str(text);
                        self.selected = 0;
                        self.rebuild_filter();
                    }
                    Key::Char(text) if text == "space" => {
                        self.filter.push(' ');
                        self.selected = 0;
                        self.rebuild_filter();
                    }
                    Key::Backspace => {
                        self.filter.pop();
                        self.selected = 0;
                        self.rebuild_filter();
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn item_matches(item: &SelectItem, needle: &str) -> bool {
    item.value.to_ascii_lowercase().contains(needle)
        || item.label.to_ascii_lowercase().contains(needle)
        || item
            .description
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains(needle)
}

fn fit_line(line: &str, width: usize) -> String {
    let mut line = truncate_to_width(line, width);
    if visible_width(&line) < width {
        line.push_str(&" ".repeat(width - visible_width(&line)));
    }
    line
}
