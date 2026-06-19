use pi_tui::{Color, Style, paint, paint_with};

#[test]
fn paint_with_disabled_returns_plain_text() {
    let style = Style::fg(Color::Red).bold();
    assert_eq!(paint_with("hi", &style, false), "hi");
}

#[test]
fn paint_with_enabled_single_fg() {
    let style = Style::fg(Color::Red);
    assert_eq!(paint_with("hi", &style, true), "\x1b[31mhi\x1b[0m");
}

#[test]
fn paint_with_enabled_bold_and_fg_merge_into_single_sgr() {
    let style = Style::fg(Color::Red).bold();
    assert_eq!(paint_with("hi", &style, true), "\x1b[1;31mhi\x1b[0m");
}

#[test]
fn paint_with_enabled_bold_reverse_and_fg_merge() {
    let style = Style::fg(Color::Red).bold().reverse();
    assert_eq!(paint_with("hi", &style, true), "\x1b[1;7;31mhi\x1b[0m");
}

#[test]
fn paint_with_default_color_emits_no_fg_sequence() {
    let style = Style::fg(Color::Default).bold();
    assert_eq!(paint_with("hi", &style, true), "\x1b[1mhi\x1b[0m");
}

#[test]
fn paint_with_default_style_emits_nothing() {
    let style = Style::default();
    assert_eq!(paint_with("hi", &style, true), "hi");
}

#[test]
fn paint_with_enabled_dim() {
    let style = Style::fg(Color::Default).dim();
    assert_eq!(paint_with("hi", &style, true), "\x1b[2mhi\x1b[0m");
}

#[test]
fn paint_with_enabled_bg() {
    let mut style = Style::default();
    style.bg = Color::Blue;
    assert_eq!(paint_with("hi", &style, true), "\x1b[44mhi\x1b[0m");
}

#[test]
fn paint_delegates_to_color_enabled() {
    // paint() uses color_enabled(); we cannot reset the OnceLock cache here,
    // so only assert that paint() output is one of the two valid forms.
    let style = Style::fg(Color::Red);
    let out = paint("hi", &style);
    assert!(
        out == "hi" || out == "\x1b[31mhi\x1b[0m",
        "unexpected paint output: {out:?}"
    );
}
