use pi_tui::{visible_width, Component, Container, Spacer, Text};

#[test]
fn spacer_renders_empty_lines() {
    let mut spacer = Spacer::new(3);
    assert_eq!(spacer.render(10), vec!["", "", ""]);
}

#[test]
fn container_renders_children_in_order() {
    let mut container = Container::new();
    container.add_child(Box::new(Text::new("alpha")));
    container.add_child(Box::new(Spacer::new(1)));
    container.add_child(Box::new(Text::new("beta")));

    assert_eq!(
        container.render(20),
        vec!["alpha".to_string(), "".to_string(), "beta".to_string()]
    );
}

#[test]
fn text_wraps_words_to_width() {
    let mut text = Text::new("alpha beta gamma");
    assert_eq!(
        text.render(10),
        vec!["alpha beta".to_string(), "gamma".to_string()]
    );
}

#[test]
fn text_splits_long_words_without_exceeding_width() {
    let mut text = Text::new("abcdefghij");
    let lines = text.render(4);

    assert_eq!(lines, vec!["abcd".to_string(), "efgh".to_string(), "ij".to_string()]);
    assert!(lines.iter().all(|line| visible_width(line) <= 4));
}

#[test]
fn text_handles_cjk_width_when_wrapping() {
    let mut text = Text::new("你好 world");
    let lines = text.render(6);

    assert_eq!(lines, vec!["你好".to_string(), "world".to_string()]);
    assert!(lines.iter().all(|line| visible_width(line) <= 6));
}
