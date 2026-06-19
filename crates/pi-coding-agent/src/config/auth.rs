use crate::config::ConfigDiagnostic;

/// Expand `$VAR` / `${VAR}` from the environment, with `$$` → `$` and `$!` → `!`.
/// Returns `None` (plus a diagnostic) if a referenced variable is unset.
pub fn resolve_config_value(raw: &str, diags: &mut Vec<ConfigDiagnostic>) -> Option<String> {
    let mut out = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '$' {
            out.push(c);
            continue;
        }
        match chars.peek().copied() {
            Some('$') => {
                chars.next();
                out.push('$');
            }
            Some('!') => {
                chars.next();
                out.push('!');
            }
            Some('{') => {
                chars.next(); // consume '{'
                let mut var = String::new();
                let mut closed = false;
                for ch in chars.by_ref() {
                    if ch == '}' {
                        closed = true;
                        break;
                    }
                    var.push(ch);
                }
                if !closed {
                    out.push('$');
                    out.push('{');
                    out.push_str(&var);
                    continue;
                }
                match std::env::var(&var) {
                    Ok(value) => out.push_str(&value),
                    Err(_) => {
                        diags.push(ConfigDiagnostic::warn(
                            format!("env var {var} referenced by auth.toml is unset"),
                            None,
                        ));
                        return None;
                    }
                }
            }
            Some(first) if first.is_ascii_alphabetic() || first == '_' => {
                let mut var = String::new();
                while let Some(&next) = chars.peek() {
                    if next.is_ascii_alphanumeric() || next == '_' {
                        var.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                match std::env::var(&var) {
                    Ok(value) => out.push_str(&value),
                    Err(_) => {
                        diags.push(ConfigDiagnostic::warn(
                            format!("env var {var} referenced by auth.toml is unset"),
                            None,
                        ));
                        return None;
                    }
                }
            }
            _ => out.push('$'),
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_passthrough() {
        let mut d = Vec::new();
        assert_eq!(resolve_config_value("sk-literal", &mut d), Some("sk-literal".into()));
        assert!(d.is_empty());
    }

    #[test]
    fn dollar_var_and_braced_var() {
        unsafe { std::env::set_var("PI_TEST_KEY", "secret"); }
        let mut d = Vec::new();
        assert_eq!(resolve_config_value("$PI_TEST_KEY", &mut d), Some("secret".into()));
        assert_eq!(resolve_config_value("pre-${PI_TEST_KEY}-post", &mut d), Some("pre-secret-post".into()));
        unsafe { std::env::remove_var("PI_TEST_KEY"); }
    }

    #[test]
    fn escapes() {
        let mut d = Vec::new();
        assert_eq!(resolve_config_value("$$literal", &mut d), Some("$literal".into()));
        assert_eq!(resolve_config_value("a$!b", &mut d), Some("a!b".into()));
    }

    #[test]
    fn unset_var_returns_none_with_diag() {
        unsafe { std::env::remove_var("PI_TEST_MISSING"); }
        let mut d = Vec::new();
        assert_eq!(resolve_config_value("$PI_TEST_MISSING", &mut d), None);
        assert_eq!(d.len(), 1);
    }
}
