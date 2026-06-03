pub fn visible_width(text: &str) -> usize {
    text.chars().count()
}

pub fn truncate_to_width(text: &str, max_width: usize) -> String {
    text.chars().take(max_width).collect()
}
