use unicode_segmentation::UnicodeSegmentation;

use crate::overlay::{OverlayEntry, OverlayHandle, OverlayOptions};
use crate::{
    Component, ComponentId, CursorPosition, InputEvent, Terminal, extract_cursor_marker,
    is_key_release, truncate_to_width, visible_width,
};
use crate::{OverlayAnchor, SizeValue};

const SYNC_START: &str = "\x1b[?2026h";
const SYNC_END: &str = "\x1b[?2026l";
const LINE_RESET: &str = "\x1b[0m\x1b]8;;\x07";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderStrategy {
    FullRedraw,
    Differential { first_changed_line: usize },
    NoChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOutcome {
    pub strategy: RenderStrategy,
    pub line_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderSurface {
    Inline,
    Clearing,
}

impl Default for RenderSurface {
    fn default() -> Self {
        Self::Inline
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TuiError {
    #[error("terminal I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("line {line_index} is {width} columns wide, exceeding max width {max_width}: {line:?}")]
    LineTooWide {
        line_index: usize,
        width: usize,
        max_width: usize,
        line: String,
    },
}

pub struct Tui<T: Terminal> {
    terminal: T,
    children: Vec<(ComponentId, Box<dyn Component>)>,
    overlays: Vec<OverlayEntry>,
    next_component_id: ComponentId,
    next_overlay_id: usize,
    focused_component: Option<ComponentId>,
    previous_lines: Vec<String>,
    previous_width: usize,
    previous_height: usize,
    previous_viewport_top: usize,
    cursor_row: usize,
    owned_rows: usize,
    rendered_once: bool,
    render_surface: RenderSurface,
    hardware_cursor_row: usize,
    hardware_cursor_col: usize,
    hardware_cursor_visible: bool,
    clear_on_shrink: bool,
    full_redraws: usize,
}

impl<T: Terminal> Tui<T> {
    pub fn new(terminal: T) -> Self {
        Self {
            terminal,
            children: Vec::new(),
            overlays: Vec::new(),
            next_component_id: 1,
            next_overlay_id: 1,
            focused_component: None,
            previous_lines: Vec::new(),
            previous_width: 0,
            previous_height: 0,
            previous_viewport_top: 0,
            cursor_row: 0,
            owned_rows: 0,
            rendered_once: false,
            render_surface: RenderSurface::Inline,
            hardware_cursor_row: 0,
            hardware_cursor_col: 0,
            hardware_cursor_visible: false,
            clear_on_shrink: true,
            full_redraws: 0,
        }
    }

    pub fn with_surface(terminal: T, surface: RenderSurface) -> Self {
        let mut tui = Self::new(terminal);
        tui.render_surface = surface;
        tui
    }

    pub fn terminal(&self) -> &T {
        &self.terminal
    }

    pub fn terminal_mut(&mut self) -> &mut T {
        &mut self.terminal
    }

    pub fn add_child(&mut self, child: Box<dyn Component>) {
        self.add_child_with_id(child);
    }

    pub fn add_child_with_id(&mut self, child: Box<dyn Component>) -> ComponentId {
        let id = self.next_component_id;
        self.next_component_id += 1;
        self.children.push((id, child));
        id
    }

    pub fn clear_children(&mut self) {
        self.focused_component = None;
        self.children.clear();
    }

    pub fn remove_child(&mut self, id: ComponentId) -> Option<Box<dyn Component>> {
        let index = self
            .children
            .iter()
            .position(|(component_id, _)| *component_id == id)?;
        if self.focused_component == Some(id) {
            self.focused_component = None;
        }
        Some(self.children.remove(index).1)
    }

    pub fn show_overlay(
        &mut self,
        component: Box<dyn Component>,
        options: OverlayOptions,
    ) -> OverlayHandle {
        let id = self.next_overlay_id;
        self.next_overlay_id += 1;
        let component_id = self.next_component_id;
        self.next_component_id += 1;
        self.overlays.push(OverlayEntry {
            id,
            component_id,
            component,
            options,
            hidden: false,
            restore_focus: None,
        });
        OverlayHandle { id }
    }

    pub fn hide_overlay(&mut self, handle: OverlayHandle) {
        self.set_overlay_hidden(handle, true);
    }

    pub fn set_overlay_hidden(&mut self, handle: OverlayHandle, hidden: bool) {
        let Some(index) = self.overlay_index(handle.id) else {
            return;
        };
        self.overlays[index].hidden = hidden;
        if hidden && self.focused_component == Some(self.overlays[index].component_id) {
            let restore_focus = self.overlays[index].restore_focus;
            self.set_focus(restore_focus);
        }
    }

    pub fn has_overlay(&self, handle: OverlayHandle) -> bool {
        self.overlays
            .iter()
            .any(|overlay| overlay.id == handle.id && !overlay.hidden)
    }

    pub fn focus_overlay(&mut self, handle: OverlayHandle) {
        let Some(index) = self.overlay_index(handle.id) else {
            return;
        };
        if self.overlays[index].options.non_capturing {
            return;
        }
        self.overlays[index].restore_focus = self.focused_component;
        let component_id = self.overlays[index].component_id;
        self.set_focus(Some(component_id));
    }

    pub fn unfocus_overlay(&mut self, handle: OverlayHandle, target: Option<ComponentId>) {
        let Some(index) = self.overlay_index(handle.id) else {
            return;
        };
        if self.focused_component == Some(self.overlays[index].component_id) {
            self.set_focus(target.or(self.overlays[index].restore_focus));
        }
    }

    pub fn set_focus(&mut self, id: Option<ComponentId>) {
        if self.focused_component == id {
            return;
        }

        if let Some(previous) = self.focused_component {
            if let Some(component) = self.child_mut(previous) {
                component.set_focused(false);
            }
        }

        self.focused_component = id;
        if let Some(next) = id {
            if let Some(component) = self.child_mut(next) {
                component.set_focused(true);
            } else {
                self.focused_component = None;
            }
        }
    }

    pub fn dispatch_input(&mut self, event: &InputEvent) {
        let Some(id) = self.focused_component else {
            return;
        };
        let Some(component) = self.child_mut(id) else {
            self.focused_component = None;
            return;
        };
        if is_key_release(event) && !component.wants_key_release() {
            return;
        }
        component.handle_input(event);
    }

    pub fn component_as<C: 'static>(&self, id: ComponentId) -> Option<&C> {
        self.children
            .iter()
            .find(|(component_id, _)| *component_id == id)
            .and_then(|(_, component)| component.as_any().downcast_ref::<C>())
            .or_else(|| {
                self.overlays
                    .iter()
                    .find(|overlay| overlay.component_id == id)
                    .and_then(|overlay| overlay.component.as_any().downcast_ref::<C>())
            })
    }

    pub fn component_as_mut<C: 'static>(&mut self, id: ComponentId) -> Option<&mut C> {
        if let Some(index) = self
            .children
            .iter()
            .position(|(component_id, _)| *component_id == id)
        {
            return self.children[index].1.as_any_mut().downcast_mut::<C>();
        }
        if let Some(index) = self
            .overlays
            .iter()
            .position(|overlay| overlay.component_id == id)
        {
            return self.overlays[index]
                .component
                .as_any_mut()
                .downcast_mut::<C>();
        }
        None
    }

    pub fn full_redraws(&self) -> usize {
        self.full_redraws
    }

    pub fn set_clear_on_shrink(&mut self, enabled: bool) {
        self.clear_on_shrink = enabled;
    }

    pub fn set_render_surface(&mut self, surface: RenderSurface) {
        self.render_surface = surface;
    }

    pub fn render_once(&mut self) -> Result<RenderOutcome, TuiError> {
        let size = self.terminal.size();
        let width = size.columns;
        let height = size.rows;
        let mut lines = self.render_lines(width);
        let cursor = extract_cursor_marker(&mut lines, height);
        validate_lines(&lines, width)?;

        let strategy = self.choose_strategy(&lines, width, height);
        match strategy {
            RenderStrategy::NoChange => {}
            RenderStrategy::FullRedraw => {
                self.render_full(&lines, height)?;
            }
            RenderStrategy::Differential { first_changed_line } => {
                self.render_differential(&lines, first_changed_line, height)?;
            }
        }

        self.previous_viewport_top = viewport_top(lines.len(), height);
        self.owned_rows = lines.len().min(height);
        self.rendered_once = true;
        self.position_hardware_cursor(cursor)?;

        self.previous_lines = lines.clone();
        self.previous_width = width;
        self.previous_height = height;
        self.cursor_row = lines.len().saturating_sub(1);

        Ok(RenderOutcome {
            strategy,
            line_count: lines.len(),
        })
    }

    fn render_lines(&mut self, width: usize) -> Vec<String> {
        let mut lines = Vec::new();
        for (_, child) in &mut self.children {
            lines.extend(child.render(width));
        }
        self.composite_overlays(&mut lines, width);
        lines
    }

    fn child_mut(&mut self, id: ComponentId) -> Option<&mut Box<dyn Component>> {
        if let Some(index) = self
            .children
            .iter()
            .position(|(component_id, _)| *component_id == id)
        {
            return Some(&mut self.children[index].1);
        }
        if let Some(index) = self
            .overlays
            .iter()
            .position(|overlay| overlay.component_id == id)
        {
            return Some(&mut self.overlays[index].component);
        }
        None
    }

    fn overlay_index(&self, id: usize) -> Option<usize> {
        self.overlays.iter().position(|overlay| overlay.id == id)
    }

    fn composite_overlays(&mut self, base_lines: &mut Vec<String>, terminal_width: usize) {
        let terminal_height = self.terminal.size().rows;
        for overlay in &mut self.overlays {
            if overlay.hidden {
                continue;
            }

            let overlay_width = resolve_overlay_width(&overlay.options, terminal_width).max(1);
            let mut overlay_lines = overlay.component.render(overlay_width);
            if let Some(max_height) = overlay
                .options
                .max_height
                .map(|size| resolve_size(size, terminal_height))
            {
                overlay_lines.truncate(max_height);
            }
            if overlay_lines.is_empty() {
                continue;
            }

            let (row, col) = overlay_position(
                &overlay.options,
                terminal_width,
                terminal_height,
                overlay_width,
                overlay_lines.len(),
            );

            let required_rows = row + overlay_lines.len();
            while base_lines.len() < required_rows {
                base_lines.push(String::new());
            }

            for (line_offset, overlay_line) in overlay_lines.iter().enumerate() {
                let fitted = fit_to_width(overlay_line, overlay_width);
                let base_line = &mut base_lines[row + line_offset];
                *base_line = splice_by_columns(base_line, col, overlay_width, &fitted);
            }
        }
    }

    fn choose_strategy(&self, lines: &[String], width: usize, height: usize) -> RenderStrategy {
        if self.previous_width == 0 || self.previous_height == 0 {
            return RenderStrategy::FullRedraw;
        }
        if self.previous_width != width || self.previous_height != height {
            return RenderStrategy::FullRedraw;
        }
        if self.clear_on_shrink && lines.len() < self.previous_lines.len() {
            return RenderStrategy::FullRedraw;
        }
        first_changed_line(&self.previous_lines, lines)
            .map(|first_changed_line| RenderStrategy::Differential { first_changed_line })
            .unwrap_or(RenderStrategy::NoChange)
    }

    fn render_full(&mut self, lines: &[String], height: usize) -> Result<(), TuiError> {
        self.full_redraws += 1;
        match self.render_surface {
            RenderSurface::Inline => self.render_full_inline(lines, height),
            RenderSurface::Clearing => self.render_full_clearing(lines, height),
        }
    }

    fn render_full_clearing(&mut self, lines: &[String], height: usize) -> Result<(), TuiError> {
        self.terminal.write(SYNC_START)?;
        self.terminal.hide_cursor()?;
        self.hardware_cursor_visible = false;
        self.terminal.clear_screen()?;
        self.write_lines(lines)?;
        self.terminal.write(SYNC_END)?;
        self.terminal.flush()?;
        self.hardware_cursor_row = lines.len().saturating_sub(1);
        self.hardware_cursor_col = last_line_width(lines);
        self.owned_rows = lines.len().min(height);
        Ok(())
    }

    fn render_full_inline(&mut self, lines: &[String], height: usize) -> Result<(), TuiError> {
        self.terminal.write(SYNC_START)?;
        self.terminal.hide_cursor()?;
        self.hardware_cursor_visible = false;

        if self.rendered_once {
            let next_viewport_top = viewport_top(lines.len(), height);
            let visible_lines = &lines[next_viewport_top..];
            let rows_to_clear = self.owned_rows.max(visible_lines.len()).min(height);
            if rows_to_clear > 0 {
                self.move_to_logical_row(self.previous_viewport_top)?;
                self.terminal.move_to_column(0)?;
                self.hardware_cursor_col = 0;
                self.rewrite_rows(next_viewport_top, visible_lines, rows_to_clear)?;
            }
        } else {
            self.write_lines(lines)?;
            self.hardware_cursor_row = lines.len().saturating_sub(1);
            self.hardware_cursor_col = last_line_width(lines);
        }

        self.terminal.write(SYNC_END)?;
        self.terminal.flush()?;
        Ok(())
    }

    fn render_differential(
        &mut self,
        lines: &[String],
        first_changed_line: usize,
        height: usize,
    ) -> Result<(), TuiError> {
        match self.render_surface {
            RenderSurface::Inline => {
                self.render_differential_inline(lines, first_changed_line, height)
            }
            RenderSurface::Clearing => self.render_differential_clearing(lines, first_changed_line),
        }
    }

    fn render_differential_clearing(
        &mut self,
        lines: &[String],
        first_changed_line: usize,
    ) -> Result<(), TuiError> {
        self.terminal.write(SYNC_START)?;
        let target = first_changed_line as i16;
        let current = self.hardware_cursor_row as i16;
        self.terminal.move_by(target - current)?;
        self.terminal.move_to_column(0)?;
        self.terminal.clear_from_cursor()?;
        self.write_lines(&lines[first_changed_line..])?;
        self.terminal.write(SYNC_END)?;
        self.terminal.flush()?;
        self.hardware_cursor_row = lines.len().saturating_sub(1);
        self.hardware_cursor_col = last_line_width(lines);
        Ok(())
    }

    fn render_differential_inline(
        &mut self,
        lines: &[String],
        first_changed_line: usize,
        height: usize,
    ) -> Result<(), TuiError> {
        if first_changed_line < self.previous_viewport_top {
            return self.render_full_inline(lines, height);
        }

        let appended_lines = lines.len() > self.previous_lines.len()
            && first_changed_line == self.previous_lines.len();
        self.terminal.write(SYNC_START)?;

        if appended_lines && first_changed_line > 0 {
            self.move_to_logical_row(first_changed_line - 1)?;
            self.terminal.move_to_column(0)?;
            self.hardware_cursor_col = 0;
            self.terminal.write("\r\n")?;
            self.write_lines(&lines[first_changed_line..])?;
            self.hardware_cursor_row = lines.len().saturating_sub(1);
            self.hardware_cursor_col = last_line_width(lines);
        } else {
            self.move_to_logical_row(first_changed_line)?;
            self.terminal.move_to_column(0)?;
            self.hardware_cursor_col = 0;

            let rows_to_write = lines.len().saturating_sub(first_changed_line);
            let old_rows_to_clear = self.previous_lines.len().saturating_sub(first_changed_line);
            let visible_capacity = height
                .saturating_sub(first_changed_line.saturating_sub(self.previous_viewport_top));
            let rows_to_clear = rows_to_write.max(old_rows_to_clear).min(visible_capacity);
            self.rewrite_rows(
                first_changed_line,
                &lines[first_changed_line..],
                rows_to_clear,
            )?;
        }

        self.terminal.write(SYNC_END)?;
        self.terminal.flush()?;
        Ok(())
    }

    fn write_lines(&mut self, lines: &[String]) -> Result<(), TuiError> {
        for (index, line) in lines.iter().enumerate() {
            self.terminal.write(line)?;
            self.terminal.write(LINE_RESET)?;
            if index + 1 < lines.len() {
                self.terminal.write("\r\n")?;
            }
        }
        Ok(())
    }

    fn rewrite_rows(
        &mut self,
        start_row: usize,
        lines: &[String],
        rows_to_clear: usize,
    ) -> Result<(), TuiError> {
        if rows_to_clear == 0 {
            return Ok(());
        }

        for row_offset in 0..rows_to_clear {
            self.terminal.write("\r")?;
            self.terminal.clear_line()?;
            if let Some(line) = lines.get(row_offset) {
                self.terminal.write(line)?;
                self.terminal.write(LINE_RESET)?;
            }
            if row_offset + 1 < rows_to_clear {
                self.terminal.write("\r\n")?;
            }
        }

        self.hardware_cursor_row = start_row + rows_to_clear - 1;
        self.hardware_cursor_col = lines
            .get(rows_to_clear.saturating_sub(1))
            .map(|line| visible_width(line))
            .unwrap_or(0);
        Ok(())
    }

    fn position_hardware_cursor(&mut self, cursor: Option<CursorPosition>) -> Result<(), TuiError> {
        let Some(cursor) = cursor else {
            if self.hardware_cursor_visible {
                self.terminal.hide_cursor()?;
                self.hardware_cursor_visible = false;
                self.terminal.flush()?;
            }
            return Ok(());
        };

        let target = cursor.row.saturating_sub(self.previous_viewport_top) as i16;
        let current = self
            .hardware_cursor_row
            .saturating_sub(self.previous_viewport_top) as i16;
        self.terminal.move_by(target - current)?;
        self.terminal.move_to_column(cursor.col)?;
        if !self.hardware_cursor_visible {
            self.terminal.show_cursor()?;
            self.hardware_cursor_visible = true;
        }
        self.hardware_cursor_row = cursor.row;
        self.hardware_cursor_col = cursor.col;
        self.terminal.flush()?;
        Ok(())
    }

    fn move_to_logical_row(&mut self, target_row: usize) -> Result<(), TuiError> {
        let target = target_row.saturating_sub(self.previous_viewport_top) as i16;
        let current = self
            .hardware_cursor_row
            .saturating_sub(self.previous_viewport_top) as i16;
        self.terminal.move_by(target - current)?;
        self.hardware_cursor_row = target_row;
        Ok(())
    }
}

fn validate_lines(lines: &[String], max_width: usize) -> Result<(), TuiError> {
    for (line_index, line) in lines.iter().enumerate() {
        let width = visible_width(line);
        if width > max_width {
            return Err(TuiError::LineTooWide {
                line_index,
                width,
                max_width,
                line: line.clone(),
            });
        }
    }
    Ok(())
}

fn viewport_top(line_count: usize, height: usize) -> usize {
    line_count.saturating_sub(height)
}

fn last_line_width(lines: &[String]) -> usize {
    lines.last().map(|line| visible_width(line)).unwrap_or(0)
}

fn first_changed_line(previous: &[String], next: &[String]) -> Option<usize> {
    let shared = previous.len().min(next.len());
    for index in 0..shared {
        if previous[index] != next[index] {
            return Some(index);
        }
    }
    if previous.len() != next.len() {
        Some(shared)
    } else {
        None
    }
}

fn resolve_overlay_width(options: &OverlayOptions, terminal_width: usize) -> usize {
    let available = terminal_width.saturating_sub(options.margin.left + options.margin.right);
    let mut width = options
        .width
        .map(|size| resolve_size(size, available))
        .unwrap_or(available);
    if let Some(min_width) = options.min_width {
        width = width.max(min_width);
    }
    width.min(available).max(1)
}

fn resolve_size(size: SizeValue, available: usize) -> usize {
    match size {
        SizeValue::Columns(columns) => columns,
        SizeValue::Percent(percent) => available.saturating_mul(percent as usize) / 100,
    }
}

fn overlay_position(
    options: &OverlayOptions,
    terminal_width: usize,
    terminal_height: usize,
    overlay_width: usize,
    overlay_height: usize,
) -> (usize, usize) {
    let min_row = options.margin.top;
    let min_col = options.margin.left;
    let max_row = terminal_height
        .saturating_sub(options.margin.bottom)
        .saturating_sub(overlay_height);
    let max_col = terminal_width
        .saturating_sub(options.margin.right)
        .saturating_sub(overlay_width);

    let (mut row, mut col) = match options.anchor {
        OverlayAnchor::Center => (
            terminal_height.saturating_sub(overlay_height) / 2,
            terminal_width.saturating_sub(overlay_width) / 2,
        ),
        OverlayAnchor::TopLeft => (min_row, min_col),
        OverlayAnchor::TopRight => (min_row, max_col),
        OverlayAnchor::BottomLeft => (max_row, min_col),
        OverlayAnchor::BottomRight => (max_row, max_col),
        OverlayAnchor::TopCenter => (min_row, terminal_width.saturating_sub(overlay_width) / 2),
        OverlayAnchor::BottomCenter => (max_row, terminal_width.saturating_sub(overlay_width) / 2),
        OverlayAnchor::LeftCenter => (terminal_height.saturating_sub(overlay_height) / 2, min_col),
        OverlayAnchor::RightCenter => (terminal_height.saturating_sub(overlay_height) / 2, max_col),
    };

    if let Some(size) = options.row {
        row = resolve_size(size, terminal_height);
    }
    if let Some(size) = options.col {
        col = resolve_size(size, terminal_width);
    }

    row = apply_offset(row, options.offset_y).clamp(min_row, max_row.max(min_row));
    col = apply_offset(col, options.offset_x).clamp(min_col, max_col.max(min_col));
    (row, col)
}

fn apply_offset(value: usize, offset: isize) -> usize {
    if offset.is_negative() {
        value.saturating_sub(offset.unsigned_abs())
    } else {
        value.saturating_add(offset as usize)
    }
}

fn fit_to_width(line: &str, width: usize) -> String {
    let mut fitted = truncate_to_width(line, width);
    let visible = visible_width(&fitted);
    if visible < width {
        fitted.push_str(&" ".repeat(width - visible));
    }
    fitted
}

fn splice_by_columns(base: &str, col: usize, width: usize, replacement: &str) -> String {
    let mut prefix = truncate_to_width(base, col);
    let prefix_width = visible_width(&prefix);
    if prefix_width < col {
        prefix.push_str(&" ".repeat(col - prefix_width));
    }

    let suffix = drop_columns(base, col + width);
    format!("{prefix}{replacement}{suffix}")
}

fn drop_columns(text: &str, columns: usize) -> String {
    if columns == 0 {
        return text.to_string();
    }

    let mut skipped = 0;
    let mut output = String::new();
    let mut collecting = false;
    for grapheme in text.graphemes(true) {
        if collecting {
            output.push_str(grapheme);
            continue;
        }

        let width = visible_width(grapheme);
        if skipped + width <= columns {
            skipped += width;
        } else {
            collecting = true;
            output.push_str(grapheme);
        }
    }
    output
}
