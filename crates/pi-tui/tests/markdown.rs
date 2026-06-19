use pi_tui::{Color, Component, Markdown, Style, paint_with, visible_width};

fn bold() -> Style {
    Style::fg(Color::Default).bold()
}

fn reverse() -> Style {
    Style::default().reverse()
}

fn dim() -> Style {
    Style::fg(Color::Default).dim()
}

#[test]
fn markdown_renders_common_blocks() {
    let mut markdown = Markdown::new("# Title\n\n- one\n- two\n\n```rust\nfn main() {}\n```");
    let lines = markdown.render(40);
    let joined = lines.join("\n");
    assert!(joined.contains("Title"));
    assert!(joined.contains("one"));
    assert!(joined.contains("fn main() {}"));
}

#[test]
fn markdown_lines_do_not_exceed_width() {
    let mut markdown =
        Markdown::new("A long paragraph with **bold** text and `inline code` that must wrap.");
    for line in markdown.render(18) {
        assert!(
            visible_width(&line) <= 18,
            "line exceeded width: {:?}",
            line
        );
    }
}

#[test]
fn markdown_heading_is_bold_when_color_enabled() {
    let mut markdown = Markdown::new("# Title");
    let lines = markdown.render(40);
    let expected = paint_with("Title", &bold(), true);
    assert_eq!(lines, vec![expected]);
}

#[test]
fn markdown_inline_code_is_reverse_when_color_enabled() {
    let mut markdown = Markdown::new("see `code` here");
    let lines = markdown.render(40);
    let joined = lines.join("\n");
    assert!(
        joined.contains(&paint_with("code", &reverse(), true)),
        "expected reverse-styled inline code in: {joined:?}"
    );
}

#[test]
fn markdown_blockquote_is_dim_when_color_enabled() {
    let mut markdown = Markdown::new("> quoted text");
    let lines = markdown.render(40);
    let joined = lines.join("\n");
    assert!(
        joined.contains(&paint_with("> quoted text", &dim(), true)),
        "expected dim-styled blockquote in: {joined:?}"
    );
}

#[test]
fn markdown_rule_is_dim_when_color_enabled() {
    let mut markdown = Markdown::new("---");
    let lines = markdown.render(40);
    let joined = lines.join("\n");
    let dim_rule = paint_with(&"-".repeat(20), &dim(), true);
    assert!(
        joined.contains(&dim_rule),
        "expected dim-styled rule in: {joined:?}"
    );
}

#[test]
fn markdown_preserves_inline_punctuation_spacing() {
    let mut markdown = Markdown::new("A paragraph with **bold** text and `code`.");
    let lines = markdown.render(80);
    let joined = lines.join("\n");
    // The visible text (ignoring ANSI) must still read correctly.
    assert!(joined.contains("A paragraph with bold text and"));
    assert!(joined.contains(&paint_with("code", &reverse(), true)));
}

#[test]
fn markdown_code_block_has_dim_fence_rows_and_dim_content() {
    let mut markdown = Markdown::new("```rust\nfn main() {}\n```");
    let lines = markdown.render(40);
    let joined = lines.join("\n");

    let dim_fence = paint_with("```", &dim(), true);
    let dim_content = paint_with("   fn main() {}", &dim(), true);

    assert!(
        joined.contains(&dim_fence),
        "expected dim fence row in: {joined:?}"
    );
    assert!(
        joined.contains(&dim_content),
        "expected dim indented content in: {joined:?}"
    );
    // Two fence rows (open + close).
    assert_eq!(
        joined.matches(&dim_fence).count(),
        2,
        "expected two fence rows in: {joined:?}"
    );
}

#[test]
fn markdown_code_block_multiline_content_each_line_indented_and_dim() {
    let mut markdown = Markdown::new("```\nlet a = 1;\nlet b = 2;\n```");
    let lines = markdown.render(40);
    let joined = lines.join("\n");
    assert!(
        joined.contains(&paint_with("   let a = 1;", &dim(), true)),
        "expected dim indented first line in: {joined:?}"
    );
    assert!(
        joined.contains(&paint_with("   let b = 2;", &dim(), true)),
        "expected dim indented second line in: {joined:?}"
    );
}
