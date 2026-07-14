use std::path::{Path, PathBuf};

pub struct ConfigPaths {
    pub global_dir: PathBuf,
    pub project_dir: PathBuf,
}

impl ConfigPaths {
    pub fn global_settings(&self) -> PathBuf {
        self.global_dir.join("settings.toml")
    }
    pub fn project_settings(&self) -> PathBuf {
        self.project_dir.join("settings.toml")
    }
    pub fn global_auth(&self) -> PathBuf {
        self.global_dir.join("auth.toml")
    }
}

pub fn resolve(cwd: &Path) -> ConfigPaths {
    let global_dir = match std::env::var_os("PI_RUST_DIR") {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".pi-rust"),
    };
    ConfigPaths {
        global_dir,
        project_dir: cwd.join(".pi-rust"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_dir_is_cwd_dot_pi_rust() {
        let p = resolve(Path::new("/tmp/work"));
        assert_eq!(p.project_dir, PathBuf::from("/tmp/work/.pi-rust"));
        assert_eq!(
            p.project_settings(),
            PathBuf::from("/tmp/work/.pi-rust/settings.toml")
        );
    }

    #[test]
    fn pi_rust_dir_env_overrides_global() {
        let env = crate::test_support::EnvGuard::new(&["PI_RUST_DIR"]);
        env.set_pi_rust_dir("/custom/cfg");
        let p = resolve(Path::new("/tmp/work"));
        assert_eq!(p.global_dir, PathBuf::from("/custom/cfg"));
        assert_eq!(p.global_auth(), PathBuf::from("/custom/cfg/auth.toml"));
    }
}
