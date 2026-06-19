use crate::config::ConfigDiagnostic;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

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

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthEntry {
    ApiKey { key: String },
    Oauth(toml::Value),
}

#[derive(Debug, Default, Clone)]
pub struct AuthStore {
    entries: BTreeMap<String, AuthEntry>,
}

impl AuthStore {
    pub fn load(path: &Path, diags: &mut Vec<ConfigDiagnostic>) -> AuthStore {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return AuthStore::default(),
            Err(err) => {
                diags.push(ConfigDiagnostic::warn(
                    format!("failed to read auth: {err}"),
                    Some(path.to_path_buf()),
                ));
                return AuthStore::default();
            }
        };
        #[cfg(unix)]
        check_permissions(path, diags);
        match toml::from_str::<BTreeMap<String, AuthEntry>>(&text) {
            Ok(entries) => AuthStore { entries },
            Err(err) => {
                diags.push(ConfigDiagnostic::warn(
                    format!("failed to parse auth: {err}"),
                    Some(path.to_path_buf()),
                ));
                AuthStore::default()
            }
        }
    }

    /// Raw `api_key` value for a provider (before `$ENV` substitution). `oauth`
    /// entries return `None` in M7.
    pub fn api_key_entry(&self, provider: &str) -> Option<&str> {
        match self.entries.get(provider) {
            Some(AuthEntry::ApiKey { key }) => Some(key.as_str()),
            _ => None,
        }
    }
}

#[cfg(unix)]
fn check_permissions(path: &Path, diags: &mut Vec<ConfigDiagnostic>) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(path) {
        let mode = meta.permissions().mode();
        if mode & 0o077 != 0 {
            diags.push(ConfigDiagnostic::warn(
                format!("auth.toml has loose permissions {:o}; expected 0600", mode & 0o777),
                Some(path.to_path_buf()),
            ));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeySource {
    Cli,
    AuthFile,
    Env,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedKey {
    pub value: String,
    pub source: KeySource,
}

pub fn resolve_api_key(
    provider: &str,
    cli_key: Option<&str>,
    store: &AuthStore,
    diags: &mut Vec<ConfigDiagnostic>,
) -> Option<ResolvedKey> {
    if let Some(key) = cli_key {
        if !key.is_empty() {
            return Some(ResolvedKey {
                value: key.to_string(),
                source: KeySource::Cli,
            });
        }
    }
    if let Some(raw) = store.api_key_entry(provider) {
        if let Some(value) = resolve_config_value(raw, diags) {
            if !value.is_empty() {
                return Some(ResolvedKey {
                    value,
                    source: KeySource::AuthFile,
                });
            }
        }
    }
    if let Some(value) = pi_ai::env_api_key(provider) {
        return Some(ResolvedKey {
            value,
            source: KeySource::Env,
        });
    }
    None
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

    #[test]
    fn loads_api_key_entries_and_skips_oauth() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.toml");
        std::fs::write(
            &path,
            "[anthropic]\ntype = \"api_key\"\nkey = \"sk-x\"\n\n[openai]\ntype = \"oauth\"\naccess_token = \"t\"\n",
        )
        .unwrap();
        let mut d = Vec::new();
        let store = AuthStore::load(&path, &mut d);
        assert_eq!(store.api_key_entry("anthropic"), Some("sk-x"));
        assert_eq!(store.api_key_entry("openai"), None); // oauth skipped in M7
        assert_eq!(store.api_key_entry("missing"), None);
    }

    #[test]
    fn missing_auth_file_is_empty_no_diag() {
        let mut d = Vec::new();
        let store = AuthStore::load(std::path::Path::new("/no/such/auth.toml"), &mut d);
        assert_eq!(store.api_key_entry("anthropic"), None);
        assert!(d.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn loose_permissions_warn() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.toml");
        std::fs::write(&path, "[anthropic]\ntype = \"api_key\"\nkey = \"sk-x\"\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let mut d = Vec::new();
        let _ = AuthStore::load(&path, &mut d);
        assert!(d.iter().any(|x| x.message.contains("permissions")));
    }

    fn store_with(provider: &str, key: &str) -> AuthStore {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.toml");
        std::fs::write(&path, format!("[{provider}]\ntype = \"api_key\"\nkey = \"{key}\"\n")).unwrap();
        let mut d = Vec::new();
        AuthStore::load(&path, &mut d)
    }

    #[test]
    fn cli_key_wins() {
        let store = store_with("anthropic", "from-file");
        unsafe { std::env::set_var("ANTHROPIC_API_KEY", "from-env"); }
        let mut d = Vec::new();
        let r = resolve_api_key("anthropic", Some("from-cli"), &store, &mut d).unwrap();
        assert_eq!(r.value, "from-cli");
        assert_eq!(r.source, KeySource::Cli);
        unsafe { std::env::remove_var("ANTHROPIC_API_KEY"); }
    }

    #[test]
    fn auth_file_beats_env() {
        let store = store_with("anthropic", "from-file");
        unsafe { std::env::set_var("ANTHROPIC_API_KEY", "from-env"); }
        let mut d = Vec::new();
        let r = resolve_api_key("anthropic", None, &store, &mut d).unwrap();
        assert_eq!(r.value, "from-file");
        assert_eq!(r.source, KeySource::AuthFile);
        unsafe { std::env::remove_var("ANTHROPIC_API_KEY"); }
    }

    #[test]
    fn falls_back_to_env_then_none() {
        let store = AuthStore::default();
        unsafe { std::env::set_var("ANTHROPIC_API_KEY", "from-env"); }
        let mut d = Vec::new();
        let r = resolve_api_key("anthropic", None, &store, &mut d).unwrap();
        assert_eq!(r.source, KeySource::Env);
        unsafe { std::env::remove_var("ANTHROPIC_API_KEY"); }
        assert!(resolve_api_key("anthropic", None, &store, &mut d).is_none());
    }
}
