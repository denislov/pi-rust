use pi_tui::{Component, OverlayAnchor, OverlayOptions, Tui, VirtualTerminal};

struct Lines(Vec<String>);

impl Component for Lines {
    fn render(&mut self, _width: usize) -> Vec<String> {
        self.0.clone()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
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
    assert!(output.contains("....XX...."));
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
