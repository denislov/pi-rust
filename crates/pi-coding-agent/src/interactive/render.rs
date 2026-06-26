use std::path::Path;

use pi_tui::{
    Color, Component, ERROR, Loader, Markdown, STATUS_IDLE, STATUS_RUNNING, SYSTEM, Style,
    TOOL_ERROR, TOOL_NAME, USER, paint_with, truncate_to_width, visible_width,
};

use crate::interactive::transcript::{Transcript, TranscriptItem};

pub(super) fn render_transcript_lines(
    transcript: &Transcript,
    width: usize,
    max_tool_result_lines: usize,
    color: bool,
    markdown_theme: &pi_tui::MarkdownTheme,
    hide_thinking_block: bool,
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut previous_was_finished_tool = false;

    for item in transcript.items() {
        if previous_was_finished_tool && matches!(item, TranscriptItem::Assistant { .. }) {
            lines.push(fit_line(
                &paint_with(&"─".repeat(width), &SYSTEM, color),
                width,
            ));
        }

        let item_lines = match item {
            TranscriptItem::User { text } => {
                vec![fit_line(
                    &format!("{}: {}", paint_with("user", &USER, color), text),
                    width,
                )]
            }
            TranscriptItem::System { text } => text
                .split('\n')
                .map(|line| fit_line(&paint_with(line, &SYSTEM, color), width))
                .collect(),
            TranscriptItem::Assistant {
                markdown, thinking, ..
            } => {
                let mut lines = Vec::new();
                if !thinking.is_empty() && !hide_thinking_block {
                    let thinking_style = Style::fg(Color::Yellow).italic();
                    for line in thinking.lines() {
                        lines.push(fit_line(&paint_with(line, &thinking_style, color), width));
                    }
                }
                if !markdown.is_empty() {
                    let mut markdown = Markdown::new(markdown).with_theme(markdown_theme.clone());
                    lines.extend(
                        markdown
                            .render(width)
                            .into_iter()
                            .map(|line| fit_line(&line, width)),
                    );
                }
                lines
            }
            TranscriptItem::Tool {
                name,
                args,
                result,
                is_error,
                ..
            } => render_tool_lines(
                name,
                args,
                result.as_deref(),
                *is_error,
                width,
                max_tool_result_lines,
                color,
            ),
            TranscriptItem::Error { text } => {
                vec![fit_line(
                    &format!(
                        "{}: {}",
                        paint_with("error", &ERROR, color),
                        paint_with(text, &ERROR, color)
                    ),
                    width,
                )]
            }
        };
        lines.extend(item_lines);
        previous_was_finished_tool = matches!(
            item,
            TranscriptItem::Tool {
                result: Some(_),
                ..
            }
        );
    }

    lines
}

fn render_tool_lines(
    name: &str,
    args: &serde_json::Value,
    result: Option<&str>,
    is_error: bool,
    width: usize,
    max_tool_result_lines: usize,
    color: bool,
) -> Vec<String> {
    let status = match (result, is_error) {
        (None, _) => "running",
        (Some(_), true) => "error",
        (Some(_), false) => "done",
    };
    let status_style = match status {
        "running" => STATUS_RUNNING,
        "error" => TOOL_ERROR,
        "done" => STATUS_IDLE,
        _ => Style::default(),
    };
    let header = format!(
        "{} {} {} {}",
        paint_with("tool", &TOOL_NAME, color),
        paint_with(name, &TOOL_NAME, color),
        tool_target(name, args),
        paint_with(status, &status_style, color),
    );
    let mut lines = vec![fit_line(&header, width)];
    let Some(result) = result else {
        return lines;
    };

    let result_lines = result.lines().collect::<Vec<_>>();
    let result_line_limit = if matches!(name, "write" | "edit") {
        result_lines.len()
    } else {
        max_tool_result_lines
    };
    lines.extend(result_lines.iter().take(result_line_limit).map(|line| {
        if is_error {
            fit_line(&paint_with(line, &TOOL_ERROR, color), width)
        } else {
            fit_line(line, width)
        }
    }));
    let omitted = result_lines.len().saturating_sub(result_line_limit);
    if omitted > 0 {
        lines.push(fit_line(
            &paint_with(&format!("... truncated {omitted} lines"), &SYSTEM, color),
            width,
        ));
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
