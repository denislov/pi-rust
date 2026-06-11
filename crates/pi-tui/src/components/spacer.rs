use crate::Component;

pub struct Spacer {
    height: usize,
}

impl Spacer {
    pub fn new(height: usize) -> Self {
        Self { height }
    }
}

impl Component for Spacer {
    fn render(&mut self, _width: usize) -> Vec<String> {
        vec![String::new(); self.height]
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
