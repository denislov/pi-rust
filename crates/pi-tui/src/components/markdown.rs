use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use unicode_segmentation::UnicodeSegmentation;

use crate::terminal_image::hyperlink;
use crate::{
    Component, MarkdownTheme, Style, color_enabled, paint_with, truncate_to_width, visible_width,
};

pub struct Markdown {
    text: String,
    padding_x: usize,
    padding_y: usize,
    theme: MarkdownTheme,
    hyperlinks_enabled: bool,
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
            theme: MarkdownTheme::default(),
            hyperlinks_enabled: false,
        }
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    pub fn with_theme(mut self, theme: MarkdownTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn set_theme(&mut self, theme: MarkdownTheme) {
        self.theme = theme;
    }

    pub fn theme(&self) -> MarkdownTheme {
        self.theme
    }

    pub fn set_hyperlinks_enabled(&mut self, enabled: bool) {
        self.hyperlinks_enabled = enabled;
    }
}

impl Component for Markdown {
    fn render(&mut self, width: usize) -> Vec<String> {
        if width == 0 {
            return Vec::new();
        }

        let content_width = width.saturating_sub(self.padding_x.saturating_mul(2));
        let content_width = content_width.max(1);
        let mut lines = markdown_to_lines(
            &self.text,
            content_width,
            &self.theme,
            self.hyperlinks_enabled,
        );
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

fn markdown_to_lines(
    text: &str,
    width: usize,
    theme: &MarkdownTheme,
    hyperlinks_enabled: bool,
) -> Vec<String> {
    let parser = Parser::new_ext(text, Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES);
    let mut blocks = Vec::new();
    let mut current = String::new();
    let mut context = BlockContext::default();
    let mut list_depth = 0usize;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { .. }) => {
                flush_current(
                    &mut current,
                    &mut blocks,
                    &mut context,
                    theme,
                    hyperlinks_enabled,
                );
                context.heading = true;
            }
            Event::End(TagEnd::Heading(_)) => {
                flush_current(
                    &mut current,
                    &mut blocks,
                    &mut context,
                    theme,
                    hyperlinks_enabled,
                );
            }
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => flush_current(
                &mut current,
                &mut blocks,
                &mut context,
                theme,
                hyperlinks_enabled,
            ),
            Event::Start(Tag::List(_)) => {
                list_depth += 1;
            }
            Event::End(TagEnd::List(_)) => {
                list_depth = list_depth.saturating_sub(1);
                flush_current(
                    &mut current,
                    &mut blocks,
                    &mut context,
                    theme,
                    hyperlinks_enabled,
                );
            }
            Event::Start(Tag::Item) => {
                flush_current(
                    &mut current,
                    &mut blocks,
                    &mut context,
                    theme,
                    hyperlinks_enabled,
                );
                if list_depth > 0 {
                    current.push_str("- ");
                }
            }
            Event::End(TagEnd::Item) => flush_current(
                &mut current,
                &mut blocks,
                &mut context,
                theme,
                hyperlinks_enabled,
            ),
            Event::Start(Tag::BlockQuote(_)) => {
                flush_current(
                    &mut current,
                    &mut blocks,
                    &mut context,
                    theme,
                    hyperlinks_enabled,
                );
                context.in_quote = true;
                current.push_str("> ");
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                flush_current(
                    &mut current,
                    &mut blocks,
                    &mut context,
                    theme,
                    hyperlinks_enabled,
                );
            }
            Event::Start(Tag::CodeBlock(_)) => {
                flush_current(
                    &mut current,
                    &mut blocks,
                    &mut context,
                    theme,
                    hyperlinks_enabled,
                );
                context.in_code_block = true;
                blocks.push(paint_markdown("```", &theme.code_block_border));
            }
            Event::End(TagEnd::CodeBlock) => {
                // Flush accumulated code text as dim indented lines, then close fence.
                let code = current.trim_end();
                for source_line in code.split('\n') {
                    let line = if source_line.is_empty() {
                        paint_markdown("   ", &theme.code_block)
                    } else {
                        paint_markdown(&format!("   {source_line}"), &theme.code_block)
                    };
                    blocks.push(line);
                }
                current.clear();
                context.in_code_block = false;
                blocks.push(paint_markdown("```", &theme.code_block_border));
            }
            Event::Text(text) => {
                if context.in_code_block {
                    current.push_str(&text);
                } else {
                    append_inline_text(&mut current, &text, false);
                }
            }
            Event::Code(text) => {
                let start = current.len();
                current.push_str(&text);
                context.inline_spans.push(InlineSpan {
                    start,
                    end: current.len(),
                    kind: InlineKind::Code,
                });
            }
            Event::Start(Tag::Strong) => context.strong_starts.push(current.len()),
            Event::End(TagEnd::Strong) => {
                if let Some(start) = context.strong_starts.pop() {
                    context.inline_spans.push(InlineSpan {
                        start,
                        end: current.len(),
                        kind: InlineKind::Strong,
                    });
                }
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                context.link_starts.push(LinkStart {
                    start: current.len(),
                    url: dest_url.to_string(),
                });
            }
            Event::End(TagEnd::Link) => {
                if let Some(start) = context.link_starts.pop() {
                    context.inline_spans.push(InlineSpan {
                        start: start.start,
                        end: current.len(),
                        kind: InlineKind::Link { url: start.url },
                    });
                }
            }
            Event::SoftBreak => current.push(' '),
            Event::HardBreak => flush_current(
                &mut current,
                &mut blocks,
                &mut context,
                theme,
                hyperlinks_enabled,
            ),
            Event::Rule => {
                flush_current(
                    &mut current,
                    &mut blocks,
                    &mut context,
                    theme,
                    hyperlinks_enabled,
                );
                blocks.push(paint_markdown(&"-".repeat(width.min(20)), &theme.hr));
            }
            _ => {}
        }
    }
    flush_current(
        &mut current,
        &mut blocks,
        &mut context,
        theme,
        hyperlinks_enabled,
    );

    let mut lines = Vec::new();
    for block in blocks {
        if block.contains("\x1b[2m") {
            // Pre-styled code-block line; do not word-wrap.
            lines.push(block);
            continue;
        }
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
    inline_spans: Vec<InlineSpan>,
    strong_starts: Vec<usize>,
    link_starts: Vec<LinkStart>,
}

#[derive(Clone)]
struct InlineSpan {
    start: usize,
    end: usize,
    kind: InlineKind,
}

#[derive(Clone)]
enum InlineKind {
    Code,
    Strong,
    Link { url: String },
}

#[derive(Clone)]
struct LinkStart {
    start: usize,
    url: String,
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

fn flush_current(
    current: &mut String,
    blocks: &mut Vec<String>,
    context: &mut BlockContext,
    theme: &MarkdownTheme,
    hyperlinks_enabled: bool,
) {
    let block = current.trim_end();
    if block.is_empty() {
        current.clear();
        context.inline_spans.clear();
        context.strong_starts.clear();
        context.link_starts.clear();
        context.heading = false;
        context.in_quote = false;
        return;
    }

    let styled = style_block(block, context, theme, hyperlinks_enabled);
    blocks.push(styled);

    current.clear();
    context.inline_spans.clear();
    context.strong_starts.clear();
    context.link_starts.clear();
    context.heading = false;
    context.in_quote = false;
}

fn style_block(
    block: &str,
    context: &BlockContext,
    theme: &MarkdownTheme,
    hyperlinks_enabled: bool,
) -> String {
    // Code blocks are emitted directly in markdown_to_lines (fence rows + dim lines),
    // so this function only handles headings, quotes, and plain paragraphs.
    let with_inline = apply_inline_spans(block, &context.inline_spans, theme, hyperlinks_enabled);
    if context.heading {
        return paint_markdown(&with_inline, &theme.heading);
    }
    if context.in_quote {
        return paint_markdown(&with_inline, &theme.quote);
    }
    with_inline
}

fn apply_inline_spans(
    block: &str,
    spans: &[InlineSpan],
    theme: &MarkdownTheme,
    hyperlinks_enabled: bool,
) -> String {
    if spans.is_empty() {
        return block.to_string();
    }
    let mut spans = spans.to_vec();
    spans.sort_by_key(|span| (span.start, span.end));
    let mut out = String::new();
    let mut cursor = 0usize;
    for span in spans {
        // Spans are byte offsets into the original `current` before trim_end.
        // block is current.trim_end(), so spans may be clamped.
        let start = span.start.min(block.len());
        let end = span.end.min(block.len());
        if start < cursor {
            continue;
        }
        if start > cursor {
            out.push_str(&block[cursor..start]);
        }
        if end > start {
            out.push_str(&apply_inline_span(
                &block[start..end],
                &span.kind,
                theme,
                hyperlinks_enabled,
            ));
        }
        cursor = end;
    }
    if cursor < block.len() {
        out.push_str(&block[cursor..]);
    }
    out
}

fn apply_inline_span(
    text: &str,
    kind: &InlineKind,
    theme: &MarkdownTheme,
    hyperlinks_enabled: bool,
) -> String {
    match kind {
        InlineKind::Code => paint_markdown(text, &theme.code),
        InlineKind::Strong => paint_markdown(text, &theme.bold),
        InlineKind::Link { url } => {
            let styled = paint_markdown(text, &theme.link);
            if hyperlinks_enabled {
                hyperlink(&styled, url)
            } else {
                let href_for_comparison = url.strip_prefix("mailto:").unwrap_or(url);
                if text == url || text == href_for_comparison {
                    styled
                } else {
                    format!(
                        "{styled}{}",
                        paint_markdown(&format!(" ({url})"), &theme.link_url)
                    )
                }
            }
        }
    }
}

fn paint_markdown(text: &str, style: &Style) -> String {
    paint_with(text, style, color_enabled())
}

fn wrap_line(source: &str, width: usize, lines: &mut Vec<String>) {
    if source.is_empty() {
        lines.push(String::new());
        return;
    }

    if source.trim().is_empty() {
        lines.push(truncate_to_width(source, width));
        return;
    }

    let leading_whitespace: String = source.chars().take_while(|ch| ch.is_whitespace()).collect();
    let mut current = leading_whitespace;
    if visible_width(&current) >= width {
        lines.push(truncate_to_width(&current, width));
        current.clear();
    }

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

        let separator = if current.trim().is_empty() { "" } else { " " };
        let candidate = format!("{current}{separator}{word}");
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
