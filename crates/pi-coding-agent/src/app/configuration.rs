use std::path::{Path, PathBuf};

use crate::config::{AuthStore, ConfigDiagnostic, SettingsScope, settings::PartialSettings};

pub(crate) fn save_provider_api_key(
    auth: &mut AuthStore,
    cwd: &Path,
    provider: &str,
    key: &str,
) -> std::io::Result<PathBuf> {
    auth.set_api_key(provider, key);
    let path = crate::config::resolve_paths(cwd).global_auth();
    auth.save(&path)?;
    Ok(path)
}

pub(crate) fn remove_provider_auth(
    auth: &mut AuthStore,
    cwd: &Path,
    provider: &str,
) -> std::io::Result<bool> {
    let removed = auth.remove_entry(provider);
    let path = crate::config::resolve_paths(cwd).global_auth();
    auth.save(&path)?;
    Ok(removed)
}

pub(crate) fn persist_global_settings(
    cwd: &Path,
    delta: &PartialSettings,
) -> Vec<ConfigDiagnostic> {
    let paths = crate::config::resolve_paths(cwd);
    let mut diagnostics = Vec::new();
    crate::config::merge_and_save_settings(&paths, SettingsScope::Global, delta, &mut diagnostics);
    diagnostics
}
