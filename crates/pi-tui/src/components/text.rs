use crate::Component;

pub struct Text {
    text: String,
}

impl Text {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

impl Component for Text {
    fn render(&mut self, _width: usize) -> Vec<String> {
        vec![self.text.clone()]
    }
}
