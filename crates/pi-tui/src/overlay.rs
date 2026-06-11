use crate::{Component, ComponentId, Terminal, Tui};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayAnchor {
    Center,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    TopCenter,
    BottomCenter,
    LeftCenter,
    RightCenter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeValue {
    Columns(usize),
    Percent(u8),
}

impl From<usize> for SizeValue {
    fn from(value: usize) -> Self {
        Self::Columns(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OverlayMargin {
    pub top: usize,
    pub right: usize,
    pub bottom: usize,
    pub left: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayOptions {
    pub width: Option<SizeValue>,
    pub min_width: Option<usize>,
    pub max_height: Option<SizeValue>,
    pub anchor: OverlayAnchor,
    pub offset_x: isize,
    pub offset_y: isize,
    pub row: Option<SizeValue>,
    pub col: Option<SizeValue>,
    pub margin: OverlayMargin,
    pub non_capturing: bool,
}

impl Default for OverlayOptions {
    fn default() -> Self {
        Self {
            width: None,
            min_width: None,
            max_height: None,
            anchor: OverlayAnchor::Center,
            offset_x: 0,
            offset_y: 0,
            row: None,
            col: None,
            margin: OverlayMargin::default(),
            non_capturing: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlayHandle {
    pub(crate) id: usize,
}

impl OverlayHandle {
    pub fn hide<T: Terminal>(self, tui: &mut Tui<T>) {
        tui.hide_overlay(self);
    }

    pub fn set_hidden<T: Terminal>(self, tui: &mut Tui<T>, hidden: bool) {
        tui.set_overlay_hidden(self, hidden);
    }

    pub fn focus<T: Terminal>(self, tui: &mut Tui<T>) {
        tui.focus_overlay(self);
    }

    pub fn unfocus<T: Terminal>(self, tui: &mut Tui<T>, target: Option<ComponentId>) {
        tui.unfocus_overlay(self, target);
    }
}

pub(crate) struct OverlayEntry {
    pub id: usize,
    pub component_id: ComponentId,
    pub component: Box<dyn Component>,
    pub options: OverlayOptions,
    pub hidden: bool,
    pub restore_focus: Option<ComponentId>,
}
