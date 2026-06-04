pub const DEFAULT_MAX_LINES: usize = 2000;
pub const DEFAULT_MAX_BYTES: usize = 50 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruncatedBy {
    Lines,
    Bytes,
    None,
}

#[derive(Debug, Clone, Default)]
pub struct TruncationOptions {
    pub max_lines: Option<usize>,
    pub max_bytes: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct TruncationResult {
    pub content: String,
    pub truncated: bool,
    pub truncated_by: TruncatedBy,
    pub total_lines: usize,
    pub total_bytes: usize,
    pub output_lines: usize,
    pub output_bytes: usize,
    pub last_line_partial: bool,
    pub first_line_exceeds_limit: bool,
    pub max_lines: usize,
    pub max_bytes: usize,
}

pub fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes}B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn split_lines_for_counting(content: &str) -> Vec<&str> {
    if content.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<&str> = content.split('\n').collect();
    if content.ends_with('\n') {
        lines.pop();
    }
    lines
}

fn no_trunc(
    content: &str,
    lines: usize,
    bytes: usize,
    max_lines: usize,
    max_bytes: usize,
) -> TruncationResult {
    TruncationResult {
        content: content.to_string(),
        truncated: false,
        truncated_by: TruncatedBy::None,
        total_lines: lines,
        total_bytes: bytes,
        output_lines: lines,
        output_bytes: bytes,
        last_line_partial: false,
        first_line_exceeds_limit: false,
        max_lines,
        max_bytes,
    }
}

pub fn truncate_head(content: &str, opts: &TruncationOptions) -> TruncationResult {
    let max_lines = opts.max_lines.unwrap_or(DEFAULT_MAX_LINES);
    let max_bytes = opts.max_bytes.unwrap_or(DEFAULT_MAX_BYTES);
    let total_bytes = content.len();
    let lines = split_lines_for_counting(content);
    let total_lines = lines.len();
    if total_lines <= max_lines && total_bytes <= max_bytes {
        return no_trunc(content, total_lines, total_bytes, max_lines, max_bytes);
    }
    let first_line_bytes = lines.first().map(|l| l.len()).unwrap_or(0);
    if first_line_bytes > max_bytes {
        return TruncationResult {
            content: String::new(),
            truncated: true,
            truncated_by: TruncatedBy::Bytes,
            total_lines,
            total_bytes,
            output_lines: 0,
            output_bytes: 0,
            last_line_partial: false,
            first_line_exceeds_limit: true,
            max_lines,
            max_bytes,
        };
    }
    let mut out: Vec<&str> = Vec::new();
    let mut bytes_count = 0usize;
    let mut truncated_by = TruncatedBy::Lines;
    for (i, line) in lines.iter().enumerate() {
        if i >= max_lines {
            break;
        }
        let line_bytes = line.len() + if i > 0 { 1 } else { 0 };
        if bytes_count + line_bytes > max_bytes {
            truncated_by = TruncatedBy::Bytes;
            break;
        }
        out.push(line);
        bytes_count += line_bytes;
    }
    if out.len() >= max_lines && bytes_count <= max_bytes {
        truncated_by = TruncatedBy::Lines;
    }
    let content_out = out.join("\n");
    let output_bytes = content_out.len();
    TruncationResult {
        content: content_out,
        truncated: true,
        truncated_by,
        total_lines,
        total_bytes,
        output_lines: out.len(),
        output_bytes,
        last_line_partial: false,
        first_line_exceeds_limit: false,
        max_lines,
        max_bytes,
    }
}

fn truncate_bytes_from_end(s: &str, max_bytes: usize) -> String {
    let b = s.as_bytes();
    if b.len() <= max_bytes {
        return s.to_string();
    }
    let mut start = b.len() - max_bytes;
    while start < b.len() && (b[start] & 0xC0) == 0x80 {
        start += 1;
    }
    String::from_utf8_lossy(&b[start..]).into_owned()
}

pub fn truncate_tail(content: &str, opts: &TruncationOptions) -> TruncationResult {
    let max_lines = opts.max_lines.unwrap_or(DEFAULT_MAX_LINES);
    let max_bytes = opts.max_bytes.unwrap_or(DEFAULT_MAX_BYTES);
    let total_bytes = content.len();
    let lines = split_lines_for_counting(content);
    let total_lines = lines.len();
    if total_lines <= max_lines && total_bytes <= max_bytes {
        return no_trunc(content, total_lines, total_bytes, max_lines, max_bytes);
    }
    let mut out: Vec<String> = Vec::new();
    let mut bytes_count = 0usize;
    let mut truncated_by = TruncatedBy::Lines;
    let mut last_line_partial = false;
    for line in lines.iter().rev() {
        if out.len() >= max_lines {
            break;
        }
        let line_bytes = line.len() + if !out.is_empty() { 1 } else { 0 };
        if bytes_count + line_bytes > max_bytes {
            truncated_by = TruncatedBy::Bytes;
            if out.is_empty() {
                let t = truncate_bytes_from_end(line, max_bytes);
                bytes_count = t.len();
                out.insert(0, t);
                last_line_partial = true;
            }
            break;
        }
        out.insert(0, (*line).to_string());
        bytes_count += line_bytes;
    }
    if out.len() >= max_lines && bytes_count <= max_bytes {
        truncated_by = TruncatedBy::Lines;
    }
    let content_out = out.join("\n");
    let output_bytes = content_out.len();
    TruncationResult {
        content: content_out,
        truncated: true,
        truncated_by,
        total_lines,
        total_bytes,
        output_lines: out.len(),
        output_bytes,
        last_line_partial,
        first_line_exceeds_limit: false,
        max_lines,
        max_bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_truncation() {
        let r = truncate_head("a\nb\nc", &TruncationOptions::default());
        assert!(!r.truncated);
        assert_eq!(r.content, "a\nb\nc");
        assert_eq!(r.total_lines, 3);
    }

    #[test]
    fn head_line_limit() {
        let content = (0..10)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let r = truncate_head(
            &content,
            &TruncationOptions {
                max_lines: Some(3),
                max_bytes: None,
            },
        );
        assert!(r.truncated);
        assert_eq!(r.truncated_by, TruncatedBy::Lines);
        assert_eq!(r.output_lines, 3);
        assert_eq!(r.content, "0\n1\n2");
    }

    #[test]
    fn head_byte_limit() {
        let content = "aaaa\nbbbb\ncccc";
        let r = truncate_head(
            content,
            &TruncationOptions {
                max_lines: None,
                max_bytes: Some(6),
            },
        );
        assert!(r.truncated);
        assert_eq!(r.truncated_by, TruncatedBy::Bytes);
        assert_eq!(r.content, "aaaa");
    }

    #[test]
    fn head_first_line_exceeds() {
        let r = truncate_head(
            "aaaaaaaaaa\nb",
            &TruncationOptions {
                max_lines: None,
                max_bytes: Some(5),
            },
        );
        assert!(r.first_line_exceeds_limit);
        assert_eq!(r.content, "");
    }

    #[test]
    fn tail_line_limit() {
        let content = (0..10)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let r = truncate_tail(
            &content,
            &TruncationOptions {
                max_lines: Some(3),
                max_bytes: None,
            },
        );
        assert!(r.truncated);
        assert_eq!(r.content, "7\n8\n9");
    }

    #[test]
    fn tail_partial_last_line() {
        let r = truncate_tail(
            "héllo-world",
            &TruncationOptions {
                max_lines: None,
                max_bytes: Some(5),
            },
        );
        assert!(r.last_line_partial);
        assert!(r.content.len() <= 5);
        assert!(std::str::from_utf8(r.content.as_bytes()).is_ok());
    }

    #[test]
    fn size_fmt() {
        assert_eq!(format_size(512), "512B");
        assert_eq!(format_size(51200), "50.0KB");
    }
}
