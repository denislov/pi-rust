use std::time::{Duration, Instant};

use pi_tui::{
    Component, InputEvent, Key, KeyEvent, KeyEventKind, KeyModifiers, RenderScheduler, Tui,
    VirtualTerminal,
};

#[derive(Default)]
struct RecordingComponent {
    focused: bool,
    inputs: Vec<InputEvent>,
}

impl Component for RecordingComponent {
    fn render(&mut self, _width: usize) -> Vec<String> {
        vec![if self.focused { "focused" } else { "idle" }.to_string()]
    }

    fn handle_input(&mut self, event: &InputEvent) {
        self.inputs.push(event.clone());
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

#[test]
fn focused_component_receives_input() {
    let terminal = VirtualTerminal::new(20, 5);
    let mut tui = Tui::new(terminal);
    let id = tui.add_child_with_id(Box::new(RecordingComponent::default()));
    tui.set_focus(Some(id));
    tui.dispatch_input(&InputEvent::Key(KeyEvent {
        key: Key::Char("x".to_string()),
        modifiers: KeyModifiers::empty(),
        kind: KeyEventKind::Press,
    }));
    let component = tui.component_as::<RecordingComponent>(id).unwrap();
    assert_eq!(component.inputs.len(), 1);
    assert!(component.focused);
}

#[test]
fn render_scheduler_coalesces_requests_until_interval_elapses() {
    let mut scheduler = RenderScheduler::new(Duration::from_millis(16));
    let start = Instant::now();
    scheduler.request(false);
    assert!(scheduler.should_render_now(start));
    assert!(scheduler.mark_rendered(start));

    scheduler.request(false);
    assert!(!scheduler.should_render_now(start + Duration::from_millis(8)));
    assert!(scheduler.should_render_now(start + Duration::from_millis(16)));
}
