use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use unicode_segmentation::UnicodeSegmentation;

use crate::{Color, Component, Style, paint, truncate_to_width, visible_width};

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
    let mut context = BlockContext::default();
    let mut list_depth = 0usize;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { .. }) => {
                flush_current(&mut current, &mut blocks, &mut context);
                context.heading = true;
            }
            Event::End(TagEnd::Heading(_)) => {
                flush_current(&mut current, &mut blocks, &mut context);
            }
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => flush_current(&mut current, &mut blocks, &mut context),
            Event::Start(Tag::List(_)) => {
                list_depth += 1;
            }
            Event::End(TagEnd::List(_)) => {
                list_depth = list_depth.saturating_sub(1);
                flush_current(&mut current, &mut blocks, &mut context);
            }
            Event::Start(Tag::Item) => {
                flush_current(&mut current, &mut blocks, &mut context);
                if list_depth > 0 {
                    current.push_str("- ");
                }
            }
            Event::End(TagEnd::Item) => flush_current(&mut current, &mut blocks, &mut context),
            Event::Start(Tag::BlockQuote(_)) => {
                flush_current(&mut current, &mut blocks, &mut context);
                context.in_quote = true;
                current.push_str("> ");
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                flush_current(&mut current, &mut blocks, &mut context);
            }
            Event::Start(Tag::CodeBlock(_)) => {
                flush_current(&mut current, &mut blocks, &mut context);
                context.in_code_block = true;
            }
            Event::End(TagEnd::CodeBlock) => {
                flush_current(&mut current, &mut blocks, &mut context);
                context.in_code_block = false;
            }
            Event::Text(text) => {
                append_inline_text(&mut current, &text, context.in_code_block);
            }
            Event::Code(text) => {
                let start = current.len();
                current.push_str(&text);
                context.inline_code_spans.push((start, current.len()));
            }
            Event::SoftBreak => current.push(' '),
            Event::HardBreak => flush_current(&mut current, &mut blocks, &mut context),
            Event::Rule => {
                flush_current(&mut current, &mut blocks, &mut context);
                blocks.push(paint(
                    &"-".repeat(width.min(20)),
                    &Style::fg(Color::Default).dim(),
                ));
            }
            _ => {}
        }
    }
    flush_current(&mut current, &mut blocks, &mut context);

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

#[derive(Default)]
struct BlockContext {
    heading: bool,
    in_quote: bool,
    in_code_block: bool,
    inline_code_spans: Vec<(usize, usize)>,
}

fn append_inline_text(current: &mut String, text: &str, in_code_block: bool) {
    if !in_code_block
        && !current.is_empty()
        && !current.ends_with([' ', '\n'])
        && !text.starts_with([' ', '\n'])
        && !starts_with_closing_punctuation(text)
    {
        current.push(' ');
    }
    current.push_str(text);
}

fn starts_with_closing_punctuation(text: &str) -> bool {
    matches!(
        text.chars().next(),
        Some('.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}')
    )
}

fn flush_current(current: &mut String, blocks: &mut Vec<String>, context: &mut BlockContext) {
    let block = current.trim_end();
    if block.is_empty() {
        current.clear();
        context.inline_code_spans.clear();
        context.heading = false;
        context.in_quote = false;
        return;
    }

    let styled = style_block(block, context);
    blocks.push(styled);

    current.clear();
    context.inline_code_spans.clear();
    context.heading = false;
    context.in_quote = false;
}

fn style_block(block: &str, context: &BlockContext) -> String {
    // Code blocks are handled in Task 3; for now pass through plain (Task 2 scope).
    if context.in_code_block {
        return block.to_string();
    }

    let with_inline = apply_inline_code(block, &context.inline_code_spans);
    if context.heading {
        return paint(&with_inline, &Style::fg(Color::Default).bold());
    }
    if context.in_quote {
        return paint(&with_inline, &Style::fg(Color::Default).dim());
    }
    with_inline
}

fn apply_inline_code(block: &str, spans: &[(usize, usize)]) -> String {
    if spans.is_empty() {
        return block.to_string();
    }
    let reverse_style = Style::default().reverse();
    let mut out = String::new();
    let mut cursor = 0usize;
    for &(start, end) in spans {
        // Spans are byte offsets into the original `current` before trim_end.
        // block is current.trim_end(), so spans may be clamped.
        let start = start.min(block.len());
        let end = end.min(block.len());
        if start > cursor {
            out.push_str(&block[cursor..start]);
        }
        if end > start {
            out.push_str(&paint(&block[start..end], &reverse_style));
        }
        cursor = end;
    }
    if cursor < block.len() {
        out.push_str(&block[cursor..]);
    }
    out
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
