use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::ansi::ansi_sequence_len;

pub fn visible_width(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }

    let mut clean = String::new();
    let mut pos = 0;

    while pos < text.len() {
        if let Some(len) = ansi_sequence_len(text, pos) {
            pos += len;
            continue;
        }

        let ch = text[pos..]
            .chars()
            .next()
            .expect("pos is on a char boundary");
        if ch == '\t' {
            clean.push_str("   ");
        } else {
            clean.push(ch);
        }
        pos += ch.len_utf8();
    }

    clean.graphemes(true).map(UnicodeWidthStr::width).sum()
}

pub fn truncate_to_width(text: &str, max_width: usize) -> String {
    if max_width == 0 || text.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    let mut width = 0;
    let mut pos = 0;

    while pos < text.len() {
        if let Some(len) = ansi_sequence_len(text, pos) {
            output.push_str(&text[pos..pos + len]);
            pos += len;
            continue;
        }

        let ch = text[pos..]
            .chars()
            .next()
            .expect("pos is on a char boundary");
        if ch == '\t' {
            if width + 3 > max_width {
                break;
            }
            output.push(ch);
            width += 3;
            pos += ch.len_utf8();
            continue;
        }

        let mut graphemes = text[pos..].graphemes(true);
        let grapheme = graphemes.next().expect("grapheme exists");
        let grapheme_width = UnicodeWidthStr::width(grapheme);
        if width + grapheme_width > max_width {
            break;
        }

        output.push_str(grapheme);
        width += grapheme_width;
        pos += grapheme.len();
    }

    output
}
