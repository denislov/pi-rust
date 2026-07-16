//! Overlay composition behavior.

use pi_tui::api::component::{Component, OverlayAnchor, OverlayOptions};
use pi_tui::api::render::Tui;
use pi_tui::api::testing::VirtualTerminal;

struct Lines(Vec<String>);

impl Component for Lines {
    fn render(&mut self, _width: usize) -> Vec<String> {
        self.0.clone()
    }
}

#[test]
fn centered_overlay_is_composited_over_base_lines() {
    let mut tui = Tui::new(VirtualTerminal::new(10, 5));
    tui.add_child(Box::new(Lines(vec![
        "..........".to_string(),
        "..........".to_string(),
        "..........".to_string(),
    ])));
    tui.show_overlay(
        Box::new(Lines(vec!["XX".to_string()])),
        OverlayOptions {
            anchor: OverlayAnchor::Center,
            width: Some(2.into()),
            ..Default::default()
        },
    );
    tui.render_once().unwrap();
    let output = tui.terminal().written_output();
    // SEGMENT_RESET is inserted between composite segments to prevent colour bleed.
    // The overlay content "XX" is separated from the base "...." by reset sequences.
    assert!(
        output.contains("XX"),
        "expected overlay content 'XX' in output, got: {output:?}"
    );
    // The last line should contain 4 dots, then SEGMENT_RESET, then XX, then SEGMENT_RESET, then 4 dots.
    // Split by newlines and check the last non-empty line.
    let last_line = output.split('\n').rfind(|l| !l.is_empty()).unwrap_or("");
    assert!(
        last_line.starts_with("...."),
        "expected last line to start with 4 dots, got: {last_line:?}"
    );
    assert!(
        last_line.contains("XX"),
        "expected last line to contain XX, got: {last_line:?}"
    );
}

#[test]
fn hiding_overlay_restores_base_render() {
    let mut tui = Tui::new(VirtualTerminal::new(8, 4));
    tui.add_child(Box::new(Lines(vec!["base".to_string()])));
    let handle = tui.show_overlay(
        Box::new(Lines(vec!["menu".to_string()])),
        Default::default(),
    );
    handle.hide(&mut tui);
    tui.render_once().unwrap();
    assert!(tui.terminal().written_output().contains("base"));
    assert!(!tui.terminal().written_output().contains("menu"));
}
