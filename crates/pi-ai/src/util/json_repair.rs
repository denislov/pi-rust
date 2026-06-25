/// Repairs common JSON formatting issues: escapes raw control characters
/// (0x00-0x1F, excluding \t \n \r) and fixes invalid escape sequences.
pub fn repair_json(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                out.push('\\');
                if let Some(&next) = chars.peek() {
                    if next == '\\'
                        || next == '"'
                        || next == '/'
                        || next == 'b'
                        || next == 'f'
                        || next == 'n'
                        || next == 'r'
                        || next == 't'
                        || next == 'u'
                    {
                        // valid escape, keep it
                    } else {
                        // invalid escape, double-escape
                        out.push('\\');
                    }
                }
            }
            c if (c as u32) < 0x20 && c != '\t' && c != '\n' && c != '\r' => {
                // raw control char, escape it
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            _ => out.push(c),
        }
    }
    out
}

/// Attempts to parse streaming (possibly incomplete) JSON.
///
/// This parser is intentionally permissive for partial deltas: it tries strict
/// JSON, repaired JSON, and a best-effort completion of unclosed constructs.
/// Callers that are about to execute tool arguments should use
/// try_parse_streaming_json instead so malformed arguments fail closed.
pub fn parse_streaming_json(input: &str) -> serde_json::Value {
    try_parse_streaming_json(input).unwrap_or_else(|_| serde_json::Value::Object(serde_json::Map::new()))
}

/// Parses streaming JSON and returns an error instead of silently falling back
/// to `{}`. Use this for final tool-call arguments where an empty object could
/// cause the wrong tool invocation.
pub fn try_parse_streaming_json(input: &str) -> Result<serde_json::Value, String> {
    if let Ok(v) = serde_json::from_str(input) {
        return Ok(v);
    }
    let repaired = repair_json(input);
    if let Ok(v) = serde_json::from_str(&repaired) {
        return Ok(v);
    }
    if let Ok(v) = serde_json::from_str(&close_incomplete(&repaired)) {
        return Ok(v);
    }
    Err("malformed streaming JSON".to_string())
}

/// Appends closing characters to make incomplete JSON parseable.
fn close_incomplete(s: &str) -> String {
    let mut out = s.to_string();
    let mut stack: Vec<char> = Vec::new();
    let mut in_string = false;
    let mut escaped = false;

    for c in s.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' && in_string {
            escaped = true;
            continue;
        }
        match c {
            '"' => in_string = !in_string,
            '{' if !in_string => stack.push('}'),
            '[' if !in_string => stack.push(']'),
            '}' | ']' if !in_string => {
                stack.pop();
            }
            _ => {}
        }
    }
    if in_string {
        out.push('"');
    }
    while let Some(bracket) = stack.pop() {
        out.push(bracket);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_escapes_control_chars() {
        let input = "hello\x01world";
        let repaired = repair_json(input);
        assert!(!repaired.contains('\x01'));
        assert!(repaired.contains("\\u0001"));
    }

    #[test]
    fn repair_fixes_bad_backslash() {
        let input = r#"{"key": "val\x"}"#;
        let repaired = repair_json(input);
        assert!(repaired.contains(r#"\\x"#));
    }

    #[test]
    fn parse_valid_json() {
        let v = parse_streaming_json(r#"{"a": 1}"#);
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn parse_truncated_object() {
        let v = parse_streaming_json(r#"{"a": 1, "b": {"#);
        assert!(v.is_object());
    }

    #[test]
    fn parse_truncated_array() {
        let v = parse_streaming_json(r#"[1, 2, {"#);
        assert!(v.is_array());
    }

    #[test]
    fn parse_garbage_returns_empty_object() {
        let v = parse_streaming_json("not json at all!!!");
        assert!(v.is_object());
        assert!(v.as_object().unwrap().is_empty());
    }

    #[test]
    fn try_parse_garbage_returns_error() {
        assert!(try_parse_streaming_json("not json at all!!!").is_err());
    }
}
