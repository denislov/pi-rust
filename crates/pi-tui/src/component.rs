use std::any::Any;

pub type ComponentId = usize;

pub trait Component: Any {
    fn render(&mut self, width: usize) -> Vec<String>;

    fn set_viewport_size(&mut self, _width: usize, _height: usize) {}

    fn handle_input(&mut self, _event: &crate::InputEvent) {}

    fn wants_key_release(&self) -> bool {
        false
    }

    fn set_focused(&mut self, _focused: bool) {}

    fn focused(&self) -> bool {
        false
    }

    fn invalidate(&mut self) {}
}

impl dyn Component {
    pub fn as_any(&self) -> &dyn Any {
        self
    }

    pub fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub struct Container {
    children: Vec<Box<dyn Component>>,
}

impl Container {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }

    pub fn add_child(&mut self, child: Box<dyn Component>) {
        self.children.push(child);
    }
}

impl Default for Container {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Container {
    fn render(&mut self, width: usize) -> Vec<String> {
        let mut lines = Vec::new();
        for child in &mut self.children {
            lines.extend(child.render(width));
        }
        lines
    }

    fn set_viewport_size(&mut self, width: usize, height: usize) {
        for child in &mut self.children {
            child.set_viewport_size(width, height);
        }
    }

    fn invalidate(&mut self) {
        for child in &mut self.children {
            child.invalidate();
        }
    }
}
