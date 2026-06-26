use std::path::Path;

use pi_tui::{
    Color, Component, ERROR, Loader, Markdown, STATUS_IDLE, STATUS_RUNNING, SYSTEM, Style,
    TOOL_ERROR, TOOL_NAME, USER, paint_with, truncate_to_width, visible_width,
};

use crate::interactive::transcript::{Transcript, TranscriptItem};
use crate::theme::{ResolvedColor, ResolvedTheme, ThemeBg, ThemeColor};

/// Resolved visual styles for transcript blocks, derived from a
/// [`ResolvedTheme`] (when available) or falling back to the built-in
/// palette constants otherwise. Mirrors the TS `theme.fg`/`theme.bg`
/// calls used by the interactive transcript components.
#[derive(Debug, Clone, Copy)]
pub(super) struct TranscriptStyles {
    pub user_text: Style,
    pub user_bg: Style,
    pub thinking: Style,
    pub system: Style,
    pub error: Style,
    pub tool_title: Style,
    pub tool_output: Style,
    pub tool_pending_bg: Style,
    pub tool_success_bg: Style,
    pub tool_error_bg: Style,
    pub tool_error_text: Style,
}

impl TranscriptStyles {
    /// Resolve styles from an optional [`ResolvedTheme`]. When `None`
    /// (e.g. in unit tests without a loaded theme), falls back to the
    /// built-in pi-tui palette constants so the transcript still renders
    /// with sensible defaults.
    pub(super) fn from_theme(resolved: Option<&ResolvedTheme>) -> Self {
        match resolved {
            Some(theme) => Self::from_resolved(theme),
            None => Self::fallback(),
        }
    }

    fn from_resolved(theme: &ResolvedTheme) -> Self {
        let fg = |token: ThemeColor| Style::fg(to_color(theme.fg(token)));
        let bg = |token: ThemeBg| Style {
            fg: Color::Default,
            bg: to_color(theme.bg(token)),
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            strikethrough: false,
            reverse: false,
        };
        Self {
            user_text: fg(ThemeColor::UserMessageText),
            user_bg: bg(ThemeBg::UserMessageBg),
            thinking: fg(ThemeColor::ThinkingText).italic(),
            system: Style::fg(Color::Default).dim(),
            error: fg(ThemeColor::Error).bold(),
            tool_title: fg(ThemeColor::ToolTitle).bold(),
            tool_output: fg(ThemeColor::ToolOutput),
            tool_pending_bg: bg(ThemeBg::ToolPendingBg),
            tool_success_bg: bg(ThemeBg::ToolSuccessBg),
            tool_error_bg: bg(ThemeBg::ToolErrorBg),
            tool_error_text: fg(ThemeColor::Error),
        }
    }

    fn fallback() -> Self {
        Self {
            user_text: USER,
            user_bg: Style::default(),
            thinking: Style::fg(Color::Yellow).italic(),
            system: SYSTEM,
            error: ERROR,
            tool_title: TOOL_NAME.bold(),
            tool_output: Style::default(),
            tool_pending_bg: Style::default(),
            tool_success_bg: Style::default(),
            tool_error_bg: Style::default(),
            tool_error_text: TOOL_ERROR,
        }
    }
}

fn to_color(color: ResolvedColor) -> Color {
    match color {
        ResolvedColor::Default => Color::Default,
        ResolvedColor::Hex(r, g, b) => Color::Rgb(r, g, b),
        ResolvedColor::Ansi256(n) => Color::Ansi256(n),
    }
}

/// All inputs to transcript block rendering, bundling width, color,
/// markdown theme, thinking visibility, and resolved [`TranscriptStyles`].
/// Mirrors the props threaded through TS `UserMessageComponent` /
/// `AssistantMessageComponent` / `ToolExecutionComponent`.
#[derive(Clone)]
pub(super) struct TranscriptRenderOptions<'a> {
    pub width: usize,
    pub max_tool_result_lines: usize,
    pub color: bool,
    pub markdown_theme: pi_tui::MarkdownTheme,
    pub hide_thinking_block: bool,
    pub hidden_thinking_label: &'a str,
    pub styles: TranscriptStyles,
}

pub(super) fn render_transcript_lines(
    transcript: &Transcript,
    opts: &TranscriptRenderOptions<'_>,
) -> Vec<String> {
    let TranscriptRenderOptions {
        width,
        max_tool_result_lines,
        color,
        markdown_theme,
        hide_thinking_block,
        hidden_thinking_label,
        styles,
    } = opts.clone();

    let mut lines = Vec::new();
    // Spacing policy: insert one blank line before every visible block except
    // the very first one. "Visible" excludes leading System welcome lines,
    // which keep their existing dim treatment. This replaces the old
    // ad-hoc "rule between finished tool and assistant" separator.
    let mut emitted_visible_block = false;

    for item in transcript.items() {
        let block = render_block(
            item,
            width,
            max_tool_result_lines,
            color,
            &markdown_theme,
            hide_thinking_block,
            hidden_thinking_label,
            styles,
        );
        if block.is_empty() {
            continue;
        }
        let is_visible_block = !matches!(item, TranscriptItem::System { .. });
        if is_visible_block && emitted_visible_block {
            lines.push(String::new());
        }
        lines.extend(block);
        if is_visible_block {
            emitted_visible_block = true;
        }
    }

    lines
}

/// Render a single transcript item into zero or more lines. Each visible
/// item is a self-contained "block"; the caller inserts spacing between
/// blocks.
#[allow(clippy::too_many_arguments)]
fn render_block(
    item: &TranscriptItem,
    width: usize,
    max_tool_result_lines: usize,
    color: bool,
    markdown_theme: &pi_tui::MarkdownTheme,
    hide_thinking_block: bool,
    hidden_thinking_label: &str,
    styles: TranscriptStyles,
) -> Vec<String> {
    match item {
        TranscriptItem::User { text } => {
            render_user_message(text, width, color, markdown_theme, &styles)
        }
        TranscriptItem::System { text } => text
            .split('\n')
            .map(|line| fit_line(&paint_with(line, &styles.system, color), width))
            .collect(),
        TranscriptItem::Assistant {
            markdown, thinking, ..
        } => render_assistant_message(
            markdown,
            thinking,
            width,
            color,
            markdown_theme,
            hide_thinking_block,
            hidden_thinking_label,
            &styles,
        ),
        TranscriptItem::Tool {
            name,
            args,
            result,
            is_error,
            ..
        } => render_tool_block(
            name,
            args,
            result.as_deref(),
            *is_error,
            width,
            max_tool_result_lines,
            color,
            &styles,
        ),
        TranscriptItem::Error { text } => render_error_message(text, width, color, &styles),
    }
}

/// Render a user message as a backgrounded box (TS `UserMessageComponent`):
/// one padding row top/bottom, content padded left/right by one column,
/// painted with `userMessageBg` / `userMessageText`.
fn render_user_message(
    text: &str,
    width: usize,
    color: bool,
    markdown_theme: &pi_tui::MarkdownTheme,
    styles: &TranscriptStyles,
) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    // Inner content width after left/right padding (min 1).
    let padding_x = 1usize.min(width.saturating_sub(1) / 2);
    let content_width = width.saturating_sub(padding_x * 2).max(1);
    let left_pad = " ".repeat(padding_x);

    let mut content_lines = Vec::new();
    let mut md = Markdown::new(text).with_theme(markdown_theme.clone());
    for line in md.render(content_width) {
        content_lines.push(format!(
            "{left_pad}{}",
            paint_with(&line, &styles.user_text, color)
        ));
    }
    if content_lines.is_empty() {
        content_lines.push(left_pad.clone());
    }

    let mut lines = Vec::new();
    // Top padding row (background-filled blank line).
    lines.push(paint_bg_line("", width, &styles.user_bg, color));
    for line in content_lines {
        lines.push(paint_bg_line(&line, width, &styles.user_bg, color));
    }
    lines.push(paint_bg_line("", width, &styles.user_bg, color));
    lines
}

/// Render an assistant message (TS `AssistantMessageComponent`): no
/// background, optional thinking block, then markdown body indented by one
/// column. Thinking and body are separated by one blank line only when the
/// body has visible content.
#[allow(clippy::too_many_arguments)]
fn render_assistant_message(
    markdown: &str,
    thinking: &str,
    width: usize,
    color: bool,
    markdown_theme: &pi_tui::MarkdownTheme,
    hide_thinking_block: bool,
    hidden_thinking_label: &str,
    styles: &TranscriptStyles,
) -> Vec<String> {
    let mut lines = Vec::new();
    let has_thinking = !thinking.trim().is_empty();
    let has_body = !markdown.trim().is_empty();

    if has_thinking {
        if hide_thinking_block {
            // Hidden thinking still surfaces a static label (TS behavior),
            // so users know reasoning happened without dumping its content.
            lines.push(fit_line(
                &paint_with(hidden_thinking_label, &styles.thinking, color),
                width,
            ));
        } else {
            lines.push(fit_line(
                &paint_with("thinking", &styles.system, color),
                width,
            ));
            for line in thinking.lines() {
                let indented = format!("  {line}");
                lines.push(fit_line(
                    &paint_with(&indented, &styles.thinking, color),
                    width,
                ));
            }
        }
        if has_body {
            lines.push(String::new());
        }
    }

    if has_body {
        let mut md = Markdown::new(markdown).with_theme(markdown_theme.clone());
        let body_width = width.saturating_sub(1).max(1);
        for line in md.render(body_width) {
            lines.push(fit_line(&format!(" {line}"), width));
        }
    }

    lines
}

/// Render an error item with an `Error:` label (TS assistant-message error
/// fallback style).
fn render_error_message(
    text: &str,
    width: usize,
    color: bool,
    styles: &TranscriptStyles,
) -> Vec<String> {
    let label = paint_with("Error:", &styles.error, color);
    text.split('\n')
        .enumerate()
        .map(|(i, line)| {
            let body = paint_with(line, &styles.error, color);
            if i == 0 {
                fit_line(&format!("{label} {body}"), width)
            } else {
                fit_line(&body, width)
            }
        })
        .collect()
}

/// Paint a line with a background style, padding it to the full render
/// width so the background fills the row (mirrors `pi_tui::Box` background
/// handling). When color is disabled this collapses to a plain padded line,
/// so layout (spacing/indent) is preserved on colorless terminals.
fn paint_bg_line(text: &str, width: usize, bg: &Style, color: bool) -> String {
    let mut line = if visible_width(text) <= width {
        text.to_string()
    } else {
        truncate_to_width(text, width)
    };
    let line_width = visible_width(&line);
    if line_width < width {
        line.push_str(&" ".repeat(width - line_width));
    }
    paint_with(&line, bg, color)
}

fn render_tool_block(
    name: &str,
    args: &serde_json::Value,
    result: Option<&str>,
    is_error: bool,
    width: usize,
    max_tool_result_lines: usize,
    color: bool,
    styles: &TranscriptStyles,
) -> Vec<String> {
    let status = match (result, is_error) {
        (None, _) => "running",
        (Some(_), true) => "error",
        (Some(_), false) => "done",
    };
    let (status_style, bg) = match status {
        "running" => (STATUS_RUNNING, &styles.tool_pending_bg),
        "error" => (TOOL_ERROR, &styles.tool_error_bg),
        "done" => (STATUS_IDLE, &styles.tool_success_bg),
        _ => (Style::default(), &styles.tool_success_bg),
    };
    let header = format!(
        "{} {} {} {}",
        paint_with("tool", &styles.tool_title, color),
        paint_with(name, &styles.tool_title, color),
        tool_target(name, args),
        paint_with(status, &status_style, color),
    );
    let mut lines = vec![paint_bg_line(&header, width, bg, color)];
    let Some(result) = result else {
        return lines;
    };

    let result_lines = result.lines().collect::<Vec<_>>();
    let result_line_limit = if matches!(name, "write" | "edit") {
        result_lines.len()
    } else {
        max_tool_result_lines
    };
    let output_style = if is_error {
        styles.tool_error_text
    } else {
        styles.tool_output
    };
    lines.extend(result_lines.iter().take(result_line_limit).map(|line| {
        let painted = paint_with(line, &output_style, color);
        paint_bg_line(&format!("  {painted}"), width, bg, color)
    }));
    let omitted = result_lines.len().saturating_sub(result_line_limit);
    if omitted > 0 {
        let note = paint_with(
            &format!("... {omitted} more lines (expand tools)"),
            &styles.system,
            color,
        );
        lines.push(paint_bg_line(&format!("  {note}"), width, bg, color));
    }
    lines
}

fn tool_target(name: &str, args: &serde_json::Value) -> String {
    match name {
        "bash" => string_arg(args, &["command", "cmd"]).unwrap_or_else(|| "-".to_string()),
        "read" | "write" | "edit" => {
            string_arg(args, &["path", "file_path", "filePath"]).unwrap_or_else(|| "-".to_string())
        }
        _ => string_arg(
            args,
            &["path", "file_path", "filePath", "command", "pattern"],
        )
        .unwrap_or_else(|| "-".to_string()),
    }
}

fn string_arg(args: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        args.get(*key)
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string)
    })
}

pub(super) fn editor_border_line(width: usize, style: &Style, color: bool) -> String {
    if width == 0 {
        return String::new();
    }
    fit_line(&paint_with(&"─".repeat(width), style, color), width)
}

pub(super) fn fit_line(line: &str, width: usize) -> String {
    if visible_width(line) <= width {
        line.to_string()
    } else {
        truncate_to_width(line, width)
    }
}

pub(super) fn running_status_text(frame: usize) -> String {
    let mut loader = Loader::new("running");
    for _ in 0..frame {
        loader.tick();
    }
    loader.render_text()
}

pub(super) fn format_tokens(count: u32) -> String {
    if count < 1000 {
        count.to_string()
    } else if count < 10000 {
        format!("{:.1}k", count as f64 / 1000.0)
    } else if count < 1000000 {
        format!("{}k", count / 1000)
    } else if count < 10000000 {
        format!("{:.1}M", count as f64 / 1000000.0)
    } else {
        format!("{}M", count / 1000000)
    }
}

/// Warning style for the context-usage percentage (70–90% band), matching
/// the TypeScript footer's `theme.fg("warning", ...)`.
pub(super) const WARNING: Style = Style::fg(Color::Yellow);

pub(super) fn abbreviate_cwd(cwd: &Path) -> String {
    let display = cwd.display().to_string();
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() && display.starts_with(&home) {
            return format!("~{}", &display[home.len()..]);
        }
    }
    display
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::builtin_dark;

    #[test]
    fn transcript_styles_fallback_when_no_theme() {
        let styles = TranscriptStyles::from_theme(None);
        // Without a resolved theme we fall back to the built-in palette
        // constants, so the transcript still renders with sensible defaults.
        assert_eq!(styles.user_text, USER);
        assert!(styles.thinking.italic);
        assert_eq!(styles.thinking.fg, Color::Yellow);
        assert_eq!(styles.error, ERROR);
        // Backgrounds collapse to default (no bg fill) in fallback mode.
        assert_eq!(styles.user_bg.bg, Color::Default);
        assert_eq!(styles.tool_pending_bg.bg, Color::Default);
    }

    #[test]
    fn transcript_styles_resolve_from_dark_theme() {
        let resolved = builtin_dark()
            .resolve_colors()
            .expect("dark theme resolves");
        let styles = TranscriptStyles::from_theme(Some(&resolved));

        // userMessageText -> "text" var -> #d4d4d4
        assert_eq!(styles.user_text.fg, Color::Rgb(0xd4, 0xd4, 0xd4));
        // userMessageBg -> #343541
        assert_eq!(styles.user_bg.bg, Color::Rgb(0x34, 0x35, 0x41));
        // thinkingText -> "gray" var -> #808080, italic preserved
        assert_eq!(styles.thinking.fg, Color::Rgb(0x80, 0x80, 0x80));
        assert!(styles.thinking.italic);
        // toolPendingBg -> #282832
        assert_eq!(styles.tool_pending_bg.bg, Color::Rgb(0x28, 0x28, 0x32));
        // toolSuccessBg -> #283228
        assert_eq!(styles.tool_success_bg.bg, Color::Rgb(0x28, 0x32, 0x28));
        // toolErrorBg -> #3c2828
        assert_eq!(styles.tool_error_bg.bg, Color::Rgb(0x3c, 0x28, 0x28));
        // toolTitle bold
        assert!(styles.tool_title.bold);
    }

    /// Build render options with no resolved theme (fallback palette) and
    /// the given color flag, for layout-focused assertions.
    fn test_opts(width: usize, color: bool) -> TranscriptRenderOptions<'static> {
        TranscriptRenderOptions {
            width,
            max_tool_result_lines: 3,
            color,
            markdown_theme: pi_tui::MarkdownTheme::default(),
            hide_thinking_block: false,
            hidden_thinking_label: "Thinking...",
            styles: TranscriptStyles::from_theme(None),
        }
    }

    #[test]
    fn user_message_renders_as_backgrounded_box_not_bare_prefix() {
        // Plan stage 1: user message is a backgrounded box (TS
        // UserMessageComponent), not a bare `user: <text>` prefix. The box
        // has top/bottom padding rows and left/right content padding.
        let mut transcript = Transcript::new();
        transcript.push(TranscriptItem::user("hello"));

        let lines = render_transcript_lines(&transcript, &test_opts(20, false));
        // Top pad + content + bottom pad = 3 rows.
        assert_eq!(lines.len(), 3, "{lines:?}");
        // Content row carries the text with one-space left padding, no `user:`.
        assert!(
            !lines[1].contains("user:"),
            "bare prefix must go: {lines:?}"
        );
        assert!(lines[1].contains("hello"), "{lines:?}");
        // Every row is padded to the full width (background fill), and none
        // overflow it.
        for line in &lines {
            assert_eq!(visible_width(line), 20, "row must fill width: {lines:?}");
        }
    }

    #[test]
    fn visible_thinking_block_has_label_and_indented_content() {
        // Plan stage 1: thinking uses a `thinking` label and indented content
        // in thinkingText, distinguishing it from the assistant body.
        let mut transcript = Transcript::new();
        transcript.push(TranscriptItem::Assistant {
            id: "a".to_string(),
            markdown: "the answer".to_string(),
            thinking: "need to check".to_string(),
            done: true,
        });

        let lines = render_transcript_lines(&transcript, &test_opts(40, false));
        let joined = lines.join("\n");
        assert!(joined.contains("thinking"), "label missing: {joined}");
        assert!(
            joined.contains("  need to check"),
            "content not indented: {joined}"
        );
        // Body follows, separated by a blank line.
        assert!(joined.contains("the answer"), "body missing: {joined}");
        assert!(
            joined.contains("\n\n"),
            "no blank between thinking and body: {joined}"
        );
    }

    #[test]
    fn hidden_thinking_block_shows_static_label_instead_of_vanishing() {
        // Plan stage 1: when thinking is hidden, show `Thinking...` rather
        // than dropping the block entirely.
        let mut transcript = Transcript::new();
        transcript.push(TranscriptItem::Assistant {
            id: "a".to_string(),
            markdown: String::new(),
            thinking: "secret reasoning".to_string(),
            done: true,
        });

        let mut opts = test_opts(40, false);
        opts.hide_thinking_block = true;
        let lines = render_transcript_lines(&transcript, &opts);
        let joined = lines.join("\n");
        assert!(
            joined.contains("Thinking..."),
            "hidden label missing: {joined}"
        );
        assert!(
            !joined.contains("secret reasoning"),
            "content leaked when hidden: {joined}"
        );
    }

    #[test]
    fn blocks_are_separated_by_one_blank_line() {
        // Plan stage 1 spacing policy: every visible block (user, assistant,
        // tool, error) is separated from the previous one by exactly one
        // blank line.
        let mut transcript = Transcript::new();
        transcript.push(TranscriptItem::user("q"));
        transcript.push(TranscriptItem::assistant("a", "reply", true));

        let lines = render_transcript_lines(&transcript, &test_opts(40, false));
        // user box (3 rows) + blank + assistant body (1 row)
        assert_eq!(lines.len(), 5, "{lines:?}");
        assert_eq!(lines[3], "", "expected blank separator: {lines:?}");
    }

    #[test]
    fn no_line_overflows_render_width() {
        // Plan width contract: every rendered line must satisfy
        // visible_width(line) <= width, across color and narrow widths.
        let mut transcript = Transcript::new();
        transcript.push(TranscriptItem::user(
            "a fairly long user prompt that needs wrapping",
        ));
        transcript.push(TranscriptItem::Assistant {
            id: "a".to_string(),
            markdown: "# Title\n\nsome *markdown* body with a lot of text in it".to_string(),
            thinking: "thinking line that is also somewhat long".to_string(),
            done: true,
        });
        transcript.push(TranscriptItem::Tool {
            call_id: "c".to_string(),
            name: "read".to_string(),
            args: serde_json::json!({"path": "src/very/deeply/nested/path/file.rs"}),
            result: Some("line content here\nand more".to_string()),
            is_error: false,
        });

        for (color, label) in [(false, "colorless"), (true, "colored")] {
            for width in [40, 20] {
                let lines = render_transcript_lines(&transcript, &test_opts(width, color));
                for line in &lines {
                    assert!(
                        visible_width(line) <= width,
                        "{label} width={width} overflow: {:?}",
                        line
                    );
                }
            }
        }
    }
}
