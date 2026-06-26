use std::path::Path;

use pi_tui::{
    Color, Component, ERROR, Loader, Markdown, SYSTEM, Style, TOOL_ERROR, TOOL_NAME, USER,
    paint_with, truncate_to_width, visible_width,
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
    pub tool_diff_added: Style,
    pub tool_diff_removed: Style,
    pub tool_diff_context: Style,
    pub bash_mode: Style,
    pub warning: Style,
    pub accent: Style,
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
            tool_diff_added: fg(ThemeColor::ToolDiffAdded),
            tool_diff_removed: fg(ThemeColor::ToolDiffRemoved),
            tool_diff_context: fg(ThemeColor::ToolDiffContext),
            bash_mode: fg(ThemeColor::BashMode).bold(),
            warning: fg(ThemeColor::Warning),
            accent: fg(ThemeColor::Accent),
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
            tool_diff_added: Style::fg(Color::Green),
            tool_diff_removed: Style::fg(Color::Red),
            tool_diff_context: Style::fg(Color::Default).dim(),
            bash_mode: Style::fg(Color::Green).bold(),
            warning: Style::fg(Color::Yellow),
            accent: Style::fg(Color::Cyan),
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
        (None, _) => ToolStatus::Running,
        (Some(_), true) => ToolStatus::Error,
        (Some(_), false) => ToolStatus::Done,
    };
    let bg = match status {
        ToolStatus::Running => &styles.tool_pending_bg,
        ToolStatus::Error => &styles.tool_error_bg,
        ToolStatus::Done => &styles.tool_success_bg,
    };

    // `edit` self-renders its diff (TS renderShell: "self") so the diff's
    // added/removed/context colors aren't swallowed by a flat tool bg.
    if name == "edit" {
        return render_edit_block(args, result, is_error, width, color, styles);
    }

    let header = render_tool_header(name, args, status, color, styles);
    let mut lines = vec![paint_bg_line(&header, width, bg, color)];
    let Some(result) = result else {
        // Bash shows a running hint while pending; other tools just stop.
        if name == "bash" {
            let hint = paint_with("Running...", &styles.system, color);
            lines.push(paint_bg_line(&format!("  {hint}"), width, bg, color));
        }
        return lines;
    };

    let body =
        render_tool_result_body(name, result, is_error, max_tool_result_lines, color, styles);
    for line in body {
        lines.push(paint_bg_line(&line, width, bg, color));
    }
    lines
}

#[derive(Clone, Copy)]
enum ToolStatus {
    Running,
    Done,
    Error,
}

impl ToolStatus {
    fn label(self) -> &'static str {
        match self {
            ToolStatus::Running => "running",
            ToolStatus::Done => "done",
            ToolStatus::Error => "error",
        }
    }
    fn style(self, styles: &TranscriptStyles) -> Style {
        match self {
            ToolStatus::Running => styles.warning,
            ToolStatus::Done => styles.tool_diff_added,
            ToolStatus::Error => styles.tool_error_text,
        }
    }
}

/// Render a tool's header line. Built-in tools get friendly, TS-parity
/// headers (`read <path>:range`, `$ <command>`, `edit <path>`); others fall
/// back to the generic `tool <name> <target> <status>` shape.
fn render_tool_header(
    name: &str,
    args: &serde_json::Value,
    status: ToolStatus,
    color: bool,
    styles: &TranscriptStyles,
) -> String {
    let status_text = paint_with(status.label(), &status.style(styles), color);
    match name {
        "read" => {
            let path = tool_target(name, args);
            let range = read_line_range(args, color, styles);
            format!(
                "{} {}{} {}",
                paint_with("read", &styles.tool_title, color),
                path,
                range,
                status_text,
            )
        }
        "bash" => {
            let command = tool_target(name, args);
            format!(
                "{} {}",
                paint_with(&format!("$ {command}"), &styles.bash_mode, color),
                status_text,
            )
        }
        "grep" => format!("{} {}", grep_header(args, color, styles), status_text),
        "find" => format!("{} {}", find_header(args, color, styles), status_text),
        "ls" => {
            let path = string_arg(args, &["path"]).unwrap_or_else(|| ".".to_string());
            format!(
                "{} {} {}",
                paint_with("ls", &styles.tool_title, color),
                path,
                status_text,
            )
        }
        "write" | "edit" => {
            let path = tool_target(name, args);
            format!(
                "{} {} {}",
                paint_with(name, &styles.tool_title, color),
                path,
                status_text,
            )
        }
        _ => format!(
            "{} {} {} {}",
            paint_with("tool", &styles.tool_title, color),
            paint_with(name, &styles.tool_title, color),
            tool_target(name, args),
            status_text,
        ),
    }
}

/// `:<start>-<end>` range suffix for `read`, mirroring TS
/// `formatReadLineRange`, in the warning color.
fn read_line_range(args: &serde_json::Value, color: bool, styles: &TranscriptStyles) -> String {
    let offset = args.get("offset").and_then(|v| v.as_u64());
    let limit = args.get("limit").and_then(|v| v.as_u64());
    if offset.is_none() && limit.is_none() {
        return String::new();
    }
    let start = offset.unwrap_or(1);
    let end = limit.map(|l| start + l - 1);
    let range = match end {
        Some(e) => format!(":{start}-{e}"),
        None => format!(":{start}"),
    };
    paint_with(&range, &styles.warning, color)
}

/// `grep /<pattern>/ in <path> (<glob>) limit <n>` header, mirroring TS
/// `formatGrepCall`. The pattern is accented; path/glob/limit use toolOutput.
fn grep_header(args: &serde_json::Value, color: bool, styles: &TranscriptStyles) -> String {
    let pattern = string_arg(args, &["pattern"]).unwrap_or_default();
    let path = string_arg(args, &["path"]).unwrap_or_else(|| ".".to_string());
    let glob = string_arg(args, &["glob"]);
    let limit = args.get("limit").and_then(|v| v.as_u64());
    let mut text = format!(
        "{} {}",
        paint_with("grep", &styles.tool_title, color),
        paint_with(&format!("/{pattern}/"), &styles.accent, color),
    );
    text.push_str(&paint_with(
        &format!(" in {path}"),
        &styles.tool_output,
        color,
    ));
    if let Some(glob) = glob {
        text.push_str(&paint_with(
            &format!(" ({glob})"),
            &styles.tool_output,
            color,
        ));
    }
    if let Some(limit) = limit {
        text.push_str(&paint_with(
            &format!(" limit {limit}"),
            &styles.tool_output,
            color,
        ));
    }
    text
}

/// `find <pattern> in <path> (limit <n>)` header, mirroring TS
/// `formatFindCall`. The pattern is accented; path/limit use toolOutput.
fn find_header(args: &serde_json::Value, color: bool, styles: &TranscriptStyles) -> String {
    let pattern = string_arg(args, &["pattern"]).unwrap_or_default();
    let path = string_arg(args, &["path"]).unwrap_or_else(|| ".".to_string());
    let limit = args.get("limit").and_then(|v| v.as_u64());
    let mut text = format!(
        "{} {}",
        paint_with("find", &styles.tool_title, color),
        paint_with(&pattern, &styles.accent, color),
    );
    text.push_str(&paint_with(
        &format!(" in {path}"),
        &styles.tool_output,
        color,
    ));
    if let Some(limit) = limit {
        text.push_str(&paint_with(
            &format!(" (limit {limit})"),
            &styles.tool_output,
            color,
        ));
    }
    text
}

/// Render a tool's result body (indented two columns). Built-in tools tailor
/// the preview: `read` replaces tabs and paints output; `bash` shows the
/// *tail* of the output (TS parity) and surfaces truncation notes; others use
/// the generic head-truncated preview.
fn render_tool_result_body(
    name: &str,
    result: &str,
    is_error: bool,
    max_tool_result_lines: usize,
    color: bool,
    styles: &TranscriptStyles,
) -> Vec<String> {
    let output_style = if is_error {
        styles.tool_error_text
    } else {
        styles.tool_output
    };
    let all_lines: Vec<&str> = result.lines().collect();

    // write/edit keep their full result (handled here for write; edit is
    // self-rendered above).
    let keep_all = matches!(name, "write");
    let limit = if keep_all {
        all_lines.len()
    } else {
        max_tool_result_lines
    };

    let (shown, omitted) = if name == "bash" && !keep_all {
        // Tail preview: show the last `limit` logical lines.
        let start = all_lines.len().saturating_sub(limit);
        (all_lines[start..].to_vec(), start)
    } else {
        (
            all_lines[..limit.min(all_lines.len())].to_vec(),
            all_lines.len().saturating_sub(limit),
        )
    };

    let mut out = Vec::new();
    for line in &shown {
        let text = if name == "read" {
            replace_tabs(line)
        } else {
            (*line).to_string()
        };
        let painted = paint_with(&text, &output_style, color);
        out.push(format!("  {painted}"));
    }
    if omitted > 0 {
        let note = paint_with(
            &format!("... {omitted} more lines (expand tools)"),
            &styles.system,
            color,
        );
        out.push(format!("  {note}"));
    }
    out
}

/// Self-rendered `edit` block: no tool bg, diff lines colored by
/// added/removed/context, mirroring TS `renderShell: "self"`.
fn render_edit_block(
    args: &serde_json::Value,
    result: Option<&str>,
    is_error: bool,
    width: usize,
    color: bool,
    styles: &TranscriptStyles,
) -> Vec<String> {
    let path = tool_target("edit", args);
    let status = match (result, is_error) {
        (None, _) => ToolStatus::Running,
        (Some(_), true) => ToolStatus::Error,
        (Some(_), false) => ToolStatus::Done,
    };
    let header = format!(
        "{} {} {}",
        paint_with("edit", &styles.tool_title, color),
        path,
        paint_with(status.label(), &status.style(styles), color),
    );
    let mut lines = vec![fit_line(&header, width)];
    let Some(result) = result else {
        return lines;
    };

    let output_style = if is_error {
        styles.tool_error_text
    } else {
        styles.tool_output
    };
    for line in result.lines() {
        let styled = paint_diff_line(line, color, styles, output_style);
        lines.push(fit_line(&format!("  {styled}"), width));
    }
    lines
}

/// Paint a single diff line with semantic colors: `+` added, `-` removed,
/// ` ` context, and hunk headers (`@@`/`---`/`+++`) dimmed.
fn paint_diff_line(line: &str, color: bool, styles: &TranscriptStyles, fallback: Style) -> String {
    let (prefix, style) = if line.starts_with("+++") || line.starts_with("---") {
        (line, styles.tool_diff_context)
    } else if let Some(rest) = line.strip_prefix('+') {
        (rest, styles.tool_diff_added)
    } else if let Some(rest) = line.strip_prefix('-') {
        (rest, styles.tool_diff_removed)
    } else if line.starts_with("@@") {
        (line, styles.tool_diff_context)
    } else if let Some(rest) = line.strip_prefix(' ') {
        (rest, styles.tool_diff_context)
    } else {
        (line, fallback)
    };
    // Preserve the leading marker (stripped above) so the diff is still
    // readable on colorless terminals.
    let marker = if line.starts_with('+') {
        "+"
    } else if line.starts_with('-') {
        "-"
    } else if line.starts_with(' ') {
        " "
    } else {
        ""
    };
    format!("{}{}", marker, paint_with(prefix, &style, color))
}

/// Replace tabs with three spaces, mirroring TS `replaceTabs`.
fn replace_tabs(text: &str) -> String {
    text.replace('\t', "   ")
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
        // tool diffs + bash + warning tokens
        assert_eq!(styles.tool_diff_added.fg, Color::Rgb(0xb5, 0xbd, 0x68));
        assert_eq!(styles.tool_diff_removed.fg, Color::Rgb(0xcc, 0x66, 0x66));
        assert_eq!(styles.bash_mode.fg, Color::Rgb(0xb5, 0xbd, 0x68));
        assert!(styles.bash_mode.bold);
        assert_eq!(styles.warning.fg, Color::Rgb(0xff, 0xff, 0x00));
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
        transcript.push(TranscriptItem::Tool {
            call_id: "c".to_string(),
            name: "grep".to_string(),
            args: serde_json::json!({
                "pattern": "someLongRegexPattern",
                "path": "src/very/deep/nested/dir",
                "glob": "*.rs",
                "limit": 100
            }),
            result: Some("src/lib.rs:1: match".to_string()),
            is_error: false,
        });
        transcript.push(TranscriptItem::Tool {
            call_id: "c".to_string(),
            name: "find".to_string(),
            args: serde_json::json!({
                "pattern": "**/*.rs",
                "path": "crates/very/deeply/nested",
                "limit": 1000
            }),
            result: Some("crates/lib.rs".to_string()),
            is_error: false,
        });
        transcript.push(TranscriptItem::Tool {
            call_id: "c".to_string(),
            name: "ls".to_string(),
            args: serde_json::json!({"path": "src/very/deeply/nested/path"}),
            result: Some("file.rs".to_string()),
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

    #[test]
    fn read_header_shows_path_and_line_range() {
        // Plan stage 3 read parity: header is `read <path>:<range>` (no
        // `tool` prefix), with the line range in the warning color.
        let mut transcript = Transcript::new();
        transcript.push(TranscriptItem::Tool {
            call_id: "c".to_string(),
            name: "read".to_string(),
            args: serde_json::json!({"path": "src/lib.rs", "offset": 10, "limit": 5}),
            result: Some("body".to_string()),
            is_error: false,
        });
        let lines = render_transcript_lines(&transcript, &test_opts(60, false));
        assert!(
            lines[0].trim().starts_with("read src/lib.rs:10-14 done"),
            "{}",
            lines[0]
        );
    }

    #[test]
    fn bash_header_uses_dollar_prefix_and_running_hint() {
        // Plan stage 3 bash parity: header is `$ <command>`; while pending
        // show `Running...`.
        let mut transcript = Transcript::new();
        transcript.push(TranscriptItem::Tool {
            call_id: "c".to_string(),
            name: "bash".to_string(),
            args: serde_json::json!({"command": "cargo test"}),
            result: None,
            is_error: false,
        });
        let lines = render_transcript_lines(&transcript, &test_opts(60, false));
        assert!(
            lines[0].trim().starts_with("$ cargo test running"),
            "{}",
            lines[0]
        );
        assert!(lines[1].trim().starts_with("Running..."), "{}", lines[1]);
    }

    #[test]
    fn bash_result_shows_tail_preview_not_head() {
        // Plan stage 3 bash parity: collapsed view shows the *last* N lines
        // (tail), not the first N, so the most recent output stays visible.
        let mut transcript = Transcript::new();
        transcript.push(TranscriptItem::Tool {
            call_id: "c".to_string(),
            name: "bash".to_string(),
            args: serde_json::json!({"command": "echo"}),
            result: Some("l1\nl2\nl3\nl4\nl5\nl6".to_string()),
            is_error: false,
        });
        let lines = render_transcript_lines(&transcript, &test_opts(60, false));
        let body: Vec<String> = lines
            .iter()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        assert!(
            body.iter().any(|l| l.starts_with("l6")),
            "tail must include l6: {body:?}"
        );
        assert!(
            body.iter().any(|l| l.starts_with("l4")),
            "tail must include l4: {body:?}"
        );
        assert!(
            !body.iter().any(|l| l.starts_with("l1")),
            "head l1 should be hidden: {body:?}"
        );
        assert!(
            body.iter().any(|l| l.contains("3 more lines")),
            "omitted hint missing: {body:?}"
        );
    }

    #[test]
    fn edit_block_self_renders_diff_with_semantic_colors() {
        // Plan stage 3 edit parity: edit self-renders (no tool bg), with
        // added/removed/context lines colored separately.
        let diff = "--- src/lib.rs\n+++ src/lib.rs\n@@ -1,2 +1,2 @@\n context\n-old\n+new";
        let mut transcript = Transcript::new();
        transcript.push(TranscriptItem::Tool {
            call_id: "c".to_string(),
            name: "edit".to_string(),
            args: serde_json::json!({"file_path": "src/lib.rs"}),
            result: Some(diff.to_string()),
            is_error: false,
        });

        let colored = render_transcript_lines(&transcript, &test_opts(60, true));
        let joined = colored.join("\n");
        // Header is `edit <path> done` with no `tool` prefix.
        assert!(joined.contains("src/lib.rs"), "path missing: {joined}");
        assert!(joined.contains("done"), "status missing: {joined}");
        assert!(
            !joined.contains("tool edit"),
            "should not use generic prefix: {joined}"
        );
        // Added/removed lines carry their semantic color escapes (green/red).
        // toolDiffAdded = green = ANSI 2, toolDiffRemoved = red = ANSI 1.
        assert!(
            joined.contains("\x1b[32m"),
            "added line not green: {joined}"
        );
        assert!(
            joined.contains("\x1b[31m"),
            "removed line not red: {joined}"
        );
        // The `+new` / `-old` markers are preserved, with added/removed
        // content colored green/red respectively.
        assert!(
            joined.contains("\x1b[32mnew"),
            "added content not green: {joined}"
        );
        assert!(
            joined.contains("\x1b[31mold"),
            "removed content not red: {joined}"
        );
        assert!(
            joined.contains("+\x1b[32m"),
            "added marker missing: {joined}"
        );
        assert!(
            joined.contains("-\x1b[31m"),
            "removed marker missing: {joined}"
        );
    }

    #[test]
    fn grep_header_shows_pattern_path_glob_and_limit() {
        // Plan stage 4 grep parity: header surfaces pattern (accent), path,
        // glob, and limit, mirroring TS formatGrepCall.
        let mut transcript = Transcript::new();
        transcript.push(TranscriptItem::Tool {
            call_id: "c".to_string(),
            name: "grep".to_string(),
            args: serde_json::json!({
                "pattern": "TODO",
                "path": "src",
                "glob": "*.rs",
                "limit": 50
            }),
            result: Some("src/lib.rs:1: TODO".to_string()),
            is_error: false,
        });
        let lines = render_transcript_lines(&transcript, &test_opts(80, false));
        let header = lines[0].trim();
        assert!(header.starts_with("grep"), "no grep prefix: {header}");
        assert!(header.contains("/TODO/"), "pattern missing: {header}");
        assert!(header.contains("in src"), "path missing: {header}");
        assert!(header.contains("(*.rs)"), "glob missing: {header}");
        assert!(header.contains("limit 50"), "limit missing: {header}");
        assert!(header.contains("done"), "status missing: {header}");
    }

    #[test]
    fn find_header_shows_pattern_path_and_limit() {
        // Plan stage 4 find parity: header surfaces pattern (accent), path,
        // and limit, mirroring TS formatFindCall.
        let mut transcript = Transcript::new();
        transcript.push(TranscriptItem::Tool {
            call_id: "c".to_string(),
            name: "find".to_string(),
            args: serde_json::json!({
                "pattern": "**/*.rs",
                "path": "crates",
                "limit": 100
            }),
            result: Some("crates/lib.rs".to_string()),
            is_error: false,
        });
        let lines = render_transcript_lines(&transcript, &test_opts(80, false));
        let header = lines[0].trim();
        assert!(header.starts_with("find"), "no find prefix: {header}");
        assert!(header.contains("**/*.rs"), "pattern missing: {header}");
        assert!(header.contains("in crates"), "path missing: {header}");
        assert!(header.contains("limit 100"), "limit missing: {header}");
    }

    #[test]
    fn ls_header_shows_path_defaulting_to_dot() {
        // Plan stage 4 ls parity: header is `ls <path>`, defaulting to `.`
        // when no path is given, mirroring TS formatLsCall.
        let mut transcript = Transcript::new();
        transcript.push(TranscriptItem::Tool {
            call_id: "c".to_string(),
            name: "ls".to_string(),
            args: serde_json::json!({}),
            result: Some("file.rs".to_string()),
            is_error: false,
        });
        let lines = render_transcript_lines(&transcript, &test_opts(40, false));
        let header = lines[0].trim();
        assert!(header.starts_with("ls ."), "default path missing: {header}");

        let mut transcript2 = Transcript::new();
        transcript2.push(TranscriptItem::Tool {
            call_id: "c".to_string(),
            name: "ls".to_string(),
            args: serde_json::json!({"path": "src"}),
            result: Some("lib.rs".to_string()),
            is_error: false,
        });
        let lines2 = render_transcript_lines(&transcript2, &test_opts(40, false));
        let header2 = lines2[0].trim();
        assert!(
            header2.starts_with("ls src"),
            "explicit path missing: {header2}"
        );
    }

    #[test]
    fn write_header_shows_path() {
        // Plan stage 4 write parity: header is `write <path>`.
        let mut transcript = Transcript::new();
        transcript.push(TranscriptItem::Tool {
            call_id: "c".to_string(),
            name: "write".to_string(),
            args: serde_json::json!({"path": "src/main.rs", "content": "fn main(){}"}),
            result: Some("Successfully wrote 12 bytes to src/main.rs".to_string()),
            is_error: false,
        });
        let lines = render_transcript_lines(&transcript, &test_opts(60, false));
        let header = lines[0].trim();
        assert!(header.starts_with("write src/main.rs done"), "{}", header);
    }
}
