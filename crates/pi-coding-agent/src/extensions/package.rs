use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use thiserror::Error;

use super::lock::ExtensionLockV1;
use super::manifest::ExtensionManifestV2;
use crate::contributions::{ExtensionHandlerRef, HandlerTarget, HandlerTargetError};

const MANIFEST_PATH: &str = "extension.json";
const LOCK_PATH: &str = "extension.lock.json";
const COMPONENT_PATH: &str = "component.wasm";
const RESOURCE_ROOT: &str = "resources";
const MAX_ENTRIES: usize = 512;
const MAX_DEPTH: usize = 16;
const MAX_TOTAL_BYTES: u64 = 32 * 1024 * 1024;

/// A package directory that passed quarantine validation at one point in time.
///
/// This is not an activation capability. The immutable package store must copy
/// and revalidate it before exposing an admissible extension snapshot.
#[derive(Debug)]
pub(crate) struct ValidatedPackageDirectory {
    root: PathBuf,
    manifest: ExtensionManifestV2,
    lock: ExtensionLockV1,
    package_digest: String,
}

#[derive(Debug, Error)]
pub(crate) enum ExtensionPackageError {
    #[error("extension package I/O failed at {path}: {message}")]
    Io { path: PathBuf, message: String },
    #[error("invalid extension package layout: {0}")]
    Layout(String),
    #[error(transparent)]
    Manifest(#[from] super::manifest::ExtensionManifestError),
    #[error("invalid extension dependency lock: {0}")]
    Lock(String),
    #[error("component digest does not match extension manifest")]
    ComponentDigestMismatch,
    #[error(transparent)]
    HandlerTarget(#[from] HandlerTargetError),
}

impl ValidatedPackageDirectory {
    pub(crate) fn validate(root: impl AsRef<Path>) -> Result<Self, ExtensionPackageError> {
        let root = root.as_ref();
        validate_root(root)?;

        let mut state = ScanState::default();
        scan_directory(root, root, 0, &mut state)?;
        require_root_file(&state.files, MANIFEST_PATH)?;
        require_root_file(&state.files, LOCK_PATH)?;
        require_root_file(&state.files, COMPONENT_PATH)?;

        let manifest_bytes = read_file(&root.join(MANIFEST_PATH))?;
        let manifest = ExtensionManifestV2::parse(&manifest_bytes)?;
        let lock_bytes = read_file(&root.join(LOCK_PATH))?;
        let lock = ExtensionLockV1::parse_for_manifest(&lock_bytes, &manifest)
            .map_err(|error| ExtensionPackageError::Lock(error.to_string()))?;

        let component = read_file(&root.join(COMPONENT_PATH))?;
        let actual_digest = format!("{:x}", Sha256::digest(&component));
        if actual_digest != manifest.component_digest() {
            return Err(ExtensionPackageError::ComponentDigestMismatch);
        }

        let declared_resources = manifest
            .resources()
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let actual_resources = state
            .files
            .iter()
            .map(String::as_str)
            .filter(|path| path.starts_with("resources/"))
            .collect::<BTreeSet<_>>();
        if declared_resources != actual_resources {
            return Err(ExtensionPackageError::Layout(
                "manifest resources must exactly match packaged resource files".into(),
            ));
        }

        let package_digest = digest_package(root, &state.files)?;

        Ok(Self {
            root: root.to_path_buf(),
            manifest,
            lock,
            package_digest,
        })
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn id(&self) -> &str {
        self.manifest.id()
    }

    pub(crate) fn version(&self) -> &str {
        self.manifest.version()
    }

    pub(crate) fn package_digest(&self) -> &str {
        &self.package_digest
    }

    pub(super) fn locked_dependencies(
        &self,
    ) -> impl Iterator<Item = super::lock::LockedDependencyRef<'_>> {
        self.lock.dependencies()
    }

    pub(super) fn requested_permissions(&self) -> &[String] {
        self.manifest.permissions()
    }

    pub(super) fn contract_world(&self) -> &str {
        self.manifest.contract_world()
    }

    pub(super) fn handler_refs(&self) -> Result<Vec<HandlerTarget>, ExtensionPackageError> {
        self.manifest
            .contribution_handlers()
            .map(|handler| {
                ExtensionHandlerRef::new(
                    self.id(),
                    self.package_digest(),
                    handler.kind,
                    handler.handler_id,
                    handler.schema_revision,
                )
                .map(HandlerTarget::extension)
                .map_err(Into::into)
            })
            .collect()
    }
}

#[derive(Default)]
struct ScanState {
    entries: usize,
    total_bytes: u64,
    folded_paths: BTreeSet<String>,
    files: BTreeSet<String>,
}

fn validate_root(root: &Path) -> Result<(), ExtensionPackageError> {
    let metadata = symlink_metadata(root)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ExtensionPackageError::Layout(
            "package root must be a real directory".into(),
        ));
    }
    Ok(())
}

fn scan_directory(
    root: &Path,
    directory: &Path,
    depth: usize,
    state: &mut ScanState,
) -> Result<(), ExtensionPackageError> {
    if depth > MAX_DEPTH {
        return Err(ExtensionPackageError::Layout(format!(
            "package nesting exceeds {MAX_DEPTH} levels"
        )));
    }

    let mut entries = fs::read_dir(directory)
        .map_err(|error| io_error(directory, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io_error(directory, error))?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        state.entries += 1;
        if state.entries > MAX_ENTRIES {
            return Err(ExtensionPackageError::Layout(format!(
                "package contains more than {MAX_ENTRIES} entries"
            )));
        }

        let path = entry.path();
        let relative = relative_path(root, &path)?;
        let folded = relative.to_ascii_lowercase();
        if !state.folded_paths.insert(folded) {
            return Err(ExtensionPackageError::Layout(format!(
                "case-insensitive path collision at {relative}"
            )));
        }

        let metadata = symlink_metadata(&path)?;
        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            return Err(ExtensionPackageError::Layout(format!(
                "symbolic links are forbidden: {relative}"
            )));
        }
        if metadata.is_dir() {
            if relative != RESOURCE_ROOT && !relative.starts_with("resources/") {
                return Err(ExtensionPackageError::Layout(format!(
                    "unknown directory: {relative}"
                )));
            }
            scan_directory(root, &path, depth + 1, state)?;
            continue;
        }
        if !metadata.is_file() {
            return Err(ExtensionPackageError::Layout(format!(
                "non-regular file is forbidden: {relative}"
            )));
        }
        reject_hard_link(&metadata, &relative)?;
        if !matches!(
            relative.as_str(),
            MANIFEST_PATH | LOCK_PATH | COMPONENT_PATH
        ) && !relative.starts_with("resources/")
        {
            return Err(ExtensionPackageError::Layout(format!(
                "unknown file: {relative}"
            )));
        }

        state.total_bytes = state
            .total_bytes
            .checked_add(metadata.len())
            .ok_or_else(|| ExtensionPackageError::Layout("package size overflow".into()))?;
        if state.total_bytes > MAX_TOTAL_BYTES {
            return Err(ExtensionPackageError::Layout(format!(
                "package exceeds {MAX_TOTAL_BYTES} bytes"
            )));
        }
        state.files.insert(relative);
    }
    Ok(())
}

fn digest_package(root: &Path, files: &BTreeSet<String>) -> Result<String, ExtensionPackageError> {
    let mut digest = Sha256::new();
    for relative in files {
        let bytes = read_file(&root.join(relative))?;
        digest.update((relative.len() as u64).to_be_bytes());
        digest.update(relative.as_bytes());
        digest.update((bytes.len() as u64).to_be_bytes());
        digest.update(bytes);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(unix)]
fn reject_hard_link(metadata: &fs::Metadata, relative: &str) -> Result<(), ExtensionPackageError> {
    use std::os::unix::fs::MetadataExt;

    if metadata.nlink() > 1 {
        Err(ExtensionPackageError::Layout(format!(
            "hard links are forbidden: {relative}"
        )))
    } else {
        Ok(())
    }
}

#[cfg(not(unix))]
fn reject_hard_link(
    _metadata: &fs::Metadata,
    _relative: &str,
) -> Result<(), ExtensionPackageError> {
    Ok(())
}

fn relative_path(root: &Path, path: &Path) -> Result<String, ExtensionPackageError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| ExtensionPackageError::Layout("entry escaped package root".into()))?;
    let value = relative
        .to_str()
        .ok_or_else(|| ExtensionPackageError::Layout("package paths must be valid UTF-8".into()))?;
    Ok(value.replace(std::path::MAIN_SEPARATOR, "/"))
}

fn require_root_file(files: &BTreeSet<String>, path: &str) -> Result<(), ExtensionPackageError> {
    if files.contains(path) {
        Ok(())
    } else {
        Err(ExtensionPackageError::Layout(format!(
            "required file is missing: {path}"
        )))
    }
}

fn read_file(path: &Path) -> Result<Vec<u8>, ExtensionPackageError> {
    fs::read(path).map_err(|error| io_error(path, error))
}

fn symlink_metadata(path: &Path) -> Result<fs::Metadata, ExtensionPackageError> {
    fs::symlink_metadata(path).map_err(|error| io_error(path, error))
}

fn io_error(path: &Path, error: std::io::Error) -> ExtensionPackageError {
    ExtensionPackageError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn package() -> TempDir {
        let directory = TempDir::new().unwrap();
        let component = b"candidate component";
        let digest = format!("{:x}", Sha256::digest(component));
        fs::write(directory.path().join(COMPONENT_PATH), component).unwrap();
        fs::create_dir_all(directory.path().join("resources/prompts")).unwrap();
        fs::write(
            directory.path().join("resources/prompts/review.txt"),
            "review",
        )
        .unwrap();
        fs::write(
            directory.path().join(MANIFEST_PATH),
            serde_json::json!({
                "schemaVersion": 2,
                "id": "example.review",
                "version": "1.2.3",
                "api": { "requires": "^0.1" },
                "component": {
                    "path": "component.wasm",
                    "sha256": digest,
                    "world": "pi:extension/extension@0.1.0"
                },
                "lock": "extension.lock.json",
                "dependencies": [],
                "activation": ["workspace"],
                "permissions": ["workspace.read"],
                "contributions": {},
                "resources": ["resources/prompts/review.txt"],
                "limits": {
                    "memoryBytes": 65536,
                    "fuel": 100,
                    "deadlineMs": 100,
                    "outputBytes": 1024
                }
            })
            .to_string(),
        )
        .unwrap();
        fs::write(
            directory.path().join(LOCK_PATH),
            serde_json::json!({
                "schemaVersion": 1,
                "extension": { "id": "example.review", "version": "1.2.3" },
                "dependencies": []
            })
            .to_string(),
        )
        .unwrap();
        directory
    }

    #[test]
    fn validates_strict_package_directory() {
        let directory = package();
        let package = ValidatedPackageDirectory::validate(directory.path()).unwrap();

        assert_eq!(package.root(), directory.path());
        assert_eq!(package.id(), "example.review");
        assert_eq!(package.version(), "1.2.3");
        assert_eq!(package.package_digest().len(), 64);
    }

    #[test]
    fn rejects_unknown_file_and_case_collision() {
        let directory = package();
        fs::write(directory.path().join("README.md"), "not admitted").unwrap();
        assert!(
            ValidatedPackageDirectory::validate(directory.path())
                .unwrap_err()
                .to_string()
                .contains("unknown file")
        );

        let directory = package();
        fs::write(
            directory.path().join("resources/prompts/REVIEW.txt"),
            "collision",
        )
        .unwrap();
        assert!(
            ValidatedPackageDirectory::validate(directory.path())
                .unwrap_err()
                .to_string()
                .contains("case-insensitive")
        );
    }

    #[test]
    fn rejects_digest_and_resource_inventory_mismatch() {
        let directory = package();
        fs::write(directory.path().join(COMPONENT_PATH), "changed").unwrap();
        assert!(matches!(
            ValidatedPackageDirectory::validate(directory.path()),
            Err(ExtensionPackageError::ComponentDigestMismatch)
        ));

        let directory = package();
        fs::write(directory.path().join("resources/undeclared.txt"), "extra").unwrap();
        assert!(
            ValidatedPackageDirectory::validate(directory.path())
                .unwrap_err()
                .to_string()
                .contains("exactly match")
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symbolic_links() {
        use std::os::unix::fs::symlink;

        let directory = package();
        symlink(
            directory.path().join(COMPONENT_PATH),
            directory.path().join("resources/component-link"),
        )
        .unwrap();
        assert!(
            ValidatedPackageDirectory::validate(directory.path())
                .unwrap_err()
                .to_string()
                .contains("symbolic links")
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_hard_links() {
        let directory = package();
        fs::hard_link(
            directory.path().join(COMPONENT_PATH),
            directory.path().join("resources/component-copy"),
        )
        .unwrap();
        assert!(
            ValidatedPackageDirectory::validate(directory.path())
                .unwrap_err()
                .to_string()
                .contains("hard links")
        );
    }
}
