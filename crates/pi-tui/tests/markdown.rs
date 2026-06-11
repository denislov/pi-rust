use pi_tui::{Component, Markdown};

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
            pi_tui::visible_width(&line) <= 18,
            "line exceeded width: {:?}",
            line
        );
    }
}
