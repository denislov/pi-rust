use std::path::{Path, PathBuf};

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

pub fn resolve_to_cwd(path: &str, cwd: &Path) -> PathBuf {
    if path == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from(path));
    }
    if let Some(rest) = path.strip_prefix("~/") {
        return match home_dir() {
            Some(h) => h.join(rest),
            None => PathBuf::from(path),
        };
    }
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn relative_joins_cwd() {
        assert_eq!(
            resolve_to_cwd("a/b.txt", &PathBuf::from("/work")),
            PathBuf::from("/work/a/b.txt")
        );
    }

    #[test]
    fn absolute_kept() {
        assert_eq!(
            resolve_to_cwd("/etc/hosts", &PathBuf::from("/work")),
            PathBuf::from("/etc/hosts")
        );
    }

    #[test]
    fn tilde_expands_or_keeps() {
        let r = resolve_to_cwd("~/x", &PathBuf::from("/work"));
        match home_dir() {
            Some(h) => assert_eq!(r, h.join("x")),
            _ => assert_eq!(r, PathBuf::from("~/x")),
        }
    }
}
