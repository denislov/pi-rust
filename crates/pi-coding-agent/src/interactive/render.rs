use std::path::Path;

use pi_tui::{
    Component, ERROR, Loader, Markdown, STATUS_IDLE, STATUS_RUNNING, SYSTEM, Style, TOOL_ERROR,
    TOOL_NAME, USER, paint_with, truncate_to_width, visible_width,
};

use crate::interactive::transcript::{Transcript, TranscriptItem};

pub(super) fn render_transcript_lines(
    transcript: &Transcript,
    width: usize,
    max_tool_result_lines: usize,
    color: bool,
) -> Vec<String> {
    transcript
        .items()
        .iter()
        .flat_map(|item| match item {
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
            TranscriptItem::Assistant { markdown, .. } => {
                let mut markdown = Markdown::new(markdown);
                markdown
                    .render(width)
                    .into_iter()
                    .map(|line| fit_line(&line, width))
                    .collect::<Vec<_>>()
            }
            TranscriptItem::Tool {
                call_id,
                name,
                result,
                is_error,
                ..
            } => render_tool_lines(
                call_id,
                name,
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
        })
        .collect()
}

fn render_tool_lines(
    call_id: &str,
    name: &str,
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
        call_id,
        paint_with(status, &status_style, color),
    );
    let mut lines = vec![fit_line(&header, width)];
    let Some(result) = result else {
        return lines;
    };

    let result_lines = result.lines().collect::<Vec<_>>();
    lines.extend(result_lines.iter().take(max_tool_result_lines).map(|line| {
        if is_error {
            fit_line(&paint_with(line, &TOOL_ERROR, color), width)
        } else {
            fit_line(line, width)
        }
    }));
    let omitted = result_lines.len().saturating_sub(max_tool_result_lines);
    if omitted > 0 {
        lines.push(fit_line(
            &paint_with(&format!("... truncated {omitted} lines"), &SYSTEM, color),
            width,
        ));
    }
    lines
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
    } else if count < 1000000 {
        format!("{}k", count / 1000)
    } else {
        format!("{}M", count / 1000000)
    }
}

pub(super) fn abbreviate_cwd(cwd: &Path) -> String {
    let display = cwd.display().to_string();
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() && display.starts_with(&home) {
            return format!("~{}", &display[home.len()..]);
        }
    }
    display
}
