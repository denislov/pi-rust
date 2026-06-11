use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use unicode_segmentation::UnicodeSegmentation;

use crate::{Component, truncate_to_width, visible_width};

pub struct Markdown {
    text: String,
    padding_x: usize,
    padding_y: usize,
}

impl Markdown {
    pub fn new(text: impl Into<String>) -> Self {
        Self::with_padding(text, 0, 0)
    }

    pub fn with_padding(text: impl Into<String>, padding_x: usize, padding_y: usize) -> Self {
        Self {
            text: text.into(),
            padding_x,
            padding_y,
        }
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }
}

impl Component for Markdown {
    fn render(&mut self, width: usize) -> Vec<String> {
        if width == 0 {
            return Vec::new();
        }

        let content_width = width.saturating_sub(self.padding_x.saturating_mul(2));
        let content_width = content_width.max(1);
        let mut lines = markdown_to_lines(&self.text, content_width);
        let prefix = " ".repeat(self.padding_x);
        if self.padding_x > 0 {
            lines = lines
                .into_iter()
                .map(|line| format!("{prefix}{line}{prefix}"))
                .collect();
        }
        for _ in 0..self.padding_y {
            lines.insert(0, String::new());
            lines.push(String::new());
        }
        lines
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn markdown_to_lines(text: &str, width: usize) -> Vec<String> {
    let parser = Parser::new_ext(text, Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES);
    let mut blocks = Vec::new();
    let mut current = String::new();
    let mut in_code_block = false;
    let mut list_depth = 0usize;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { .. }) => {
                flush_current(&mut current, &mut blocks);
            }
            Event::End(TagEnd::Heading(_)) => flush_current(&mut current, &mut blocks),
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => flush_current(&mut current, &mut blocks),
            Event::Start(Tag::List(_)) => {
                list_depth += 1;
            }
            Event::End(TagEnd::List(_)) => {
                list_depth = list_depth.saturating_sub(1);
                flush_current(&mut current, &mut blocks);
            }
            Event::Start(Tag::Item) => {
                flush_current(&mut current, &mut blocks);
                if list_depth > 0 {
                    current.push_str("- ");
                }
            }
            Event::End(TagEnd::Item) => flush_current(&mut current, &mut blocks),
            Event::Start(Tag::BlockQuote(_)) => {
                flush_current(&mut current, &mut blocks);
                current.push_str("> ");
            }
            Event::End(TagEnd::BlockQuote(_)) => flush_current(&mut current, &mut blocks),
            Event::Start(Tag::CodeBlock(_)) => {
                flush_current(&mut current, &mut blocks);
                in_code_block = true;
            }
            Event::End(TagEnd::CodeBlock) => {
                flush_current(&mut current, &mut blocks);
                in_code_block = false;
            }
            Event::Text(text) | Event::Code(text) => {
                if !in_code_block && !current.is_empty() && !current.ends_with([' ', '\n']) {
                    current.push(' ');
                }
                current.push_str(&text);
            }
            Event::SoftBreak => current.push(' '),
            Event::HardBreak => flush_current(&mut current, &mut blocks),
            Event::Rule => {
                flush_current(&mut current, &mut blocks);
                blocks.push("-".repeat(width.min(20)));
            }
            _ => {}
        }
    }
    flush_current(&mut current, &mut blocks);

    let mut lines = Vec::new();
    for block in blocks {
        for source_line in block.split('\n') {
            wrap_line(source_line, width, &mut lines);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn flush_current(current: &mut String, blocks: &mut Vec<String>) {
    let block = current.trim_end();
    if !block.is_empty() {
        blocks.push(block.to_string());
    }
    current.clear();
}

fn wrap_line(source: &str, width: usize, lines: &mut Vec<String>) {
    if source.is_empty() {
        lines.push(String::new());
        return;
    }

    let mut current = String::new();
    for word in source.split_whitespace() {
        if visible_width(word) > width {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            split_long_word(word, width, lines);
            continue;
        }

        if current.is_empty() {
            current.push_str(word);
            continue;
        }

        let candidate = format!("{current} {word}");
        if visible_width(&candidate) <= width {
            current = candidate;
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }
}

fn split_long_word(word: &str, width: usize, lines: &mut Vec<String>) {
    let mut current = String::new();
    let mut current_width = 0;
    for grapheme in word.graphemes(true) {
        let grapheme_width = visible_width(grapheme);
        if current_width + grapheme_width > width && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current_width = 0;
        }
        current.push_str(grapheme);
        current_width += grapheme_width;
    }
    if !current.is_empty() {
        lines.push(truncate_to_width(&current, width));
    }
}
