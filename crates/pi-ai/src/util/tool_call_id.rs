/// Normalize a tool-call id to match the `^[a-zA-Z0-9_-]{1,64}$` pattern.
/// If the id is already valid, return as-is. Otherwise sanitize and truncate.
/// When `replacement` is Some(c), invalid chars are replaced with `c`;
/// when None, invalid chars are removed.
pub fn normalize_tool_call_id(id: &str, replacement: Option<char>) -> String {
    let is_valid = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if is_valid {
        return id.to_string();
    }

    let sanitized: String = match replacement {
        Some(replacement) => id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    replacement
                }
            })
            .collect(),
        None => id
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect(),
    };

    if sanitized.len() > 64 {
        sanitized[..64].to_string()
    } else if sanitized.is_empty() {
        "tool_0".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_id_passes_through() {
        assert_eq!(normalize_tool_call_id("toolu_01", None), "toolu_01");
        assert_eq!(normalize_tool_call_id("call-abc-123", None), "call-abc-123");
    }

    #[test]
    fn invalid_id_filtered() {
        let result = normalize_tool_call_id("tool*use!001", None);
        assert!(!result.contains('*'));
        assert!(!result.contains('!'));
    }

    #[test]
    fn invalid_id_replaced() {
        let result = normalize_tool_call_id("tool*use!001", Some('_'));
        assert_eq!(result, "tool_use_001");
    }

    #[test]
    fn empty_id_returns_placeholder() {
        assert_eq!(normalize_tool_call_id("!!!", None), "tool_0");
    }

    #[test]
    fn long_id_truncated() {
        let long = "a".repeat(100);
        let result = normalize_tool_call_id(&long, None);
        assert_eq!(result.len(), 64);
    }
}
