use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use thiserror::Error;

use super::package::{ExtensionPackageError, ValidatedPackageDirectory};

static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(1);

const STAGING_DIR: &str = "staging";
const PACKAGES_DIR: &str = "packages/sha256";

#[derive(Debug)]
pub(crate) struct ExtensionPackageStore {
    root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstalledExtensionPackage {
    path: PathBuf,
    id: String,
    version: String,
    package_digest: String,
}

#[derive(Debug, Error)]
pub(crate) enum PackageStoreError {
    #[error("extension package store I/O failed at {path}: {message}")]
    Io { path: PathBuf, message: String },
    #[error("invalid package staging ownership")]
    InvalidStagingOwnership,
    #[error(transparent)]
    Package(#[from] ExtensionPackageError),
    #[error("locked dependency {id}@{version} ({digest}) is not installed")]
    MissingDependency {
        id: String,
        version: String,
        digest: String,
    },
    #[error("installed dependency does not match lock entry for {0}")]
    DependencyMismatch(String),
    #[error("content-addressed package path contains different bytes")]
    ContentAddressCollision,
}

impl ExtensionPackageStore {
    pub(crate) fn open(root: impl AsRef<Path>) -> Result<Self, PackageStoreError> {
        let root = root.as_ref();
        create_real_directory(root)?;
        create_real_directory(&root.join(STAGING_DIR))?;
        create_real_directory(&root.join("packages"))?;
        create_real_directory(&root.join(PACKAGES_DIR))?;
        let root = fs::canonicalize(root).map_err(|error| io_error(root, error))?;
        Ok(Self { root })
    }

    pub(crate) fn create_staging_directory(&self) -> Result<PathBuf, PackageStoreError> {
        let staging_root = self.root.join(STAGING_DIR);
        for _ in 0..1024 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = staging_root.join(format!("{}-{sequence}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io_error(&path, error)),
            }
        }
        Err(PackageStoreError::Io {
            path: staging_root,
            message: "could not allocate a unique staging directory".into(),
        })
    }

    pub(crate) fn install_staged(
        &self,
        staging: impl AsRef<Path>,
    ) -> Result<InstalledExtensionPackage, PackageStoreError> {
        let staging = staging.as_ref();
        self.require_owned_staging(staging)?;
        let candidate = ValidatedPackageDirectory::validate(staging)?;
        self.validate_dependencies(&candidate)?;
        let destination = self.package_path(candidate.package_digest());

        if destination.exists() {
            let existing = ValidatedPackageDirectory::validate(&destination)?;
            if existing.package_digest() != candidate.package_digest()
                || existing.id() != candidate.id()
                || existing.version() != candidate.version()
            {
                return Err(PackageStoreError::ContentAddressCollision);
            }
            remove_directory(staging)?;
            return Ok(installed(&existing));
        }

        fs::rename(staging, &destination).map_err(|error| io_error(&destination, error))?;
        let installed_package = match ValidatedPackageDirectory::validate(&destination) {
            Ok(package) => package,
            Err(error) => {
                remove_directory(&destination)?;
                return Err(error.into());
            }
        };
        if installed_package.package_digest() != candidate.package_digest() {
            remove_directory(&destination)?;
            return Err(PackageStoreError::ContentAddressCollision);
        }
        if let Err(error) = make_tree_read_only(&destination) {
            let _ = remove_directory(&destination);
            return Err(error);
        }
        Ok(installed(&installed_package))
    }

    #[cfg(test)]
    pub(crate) fn load(
        &self,
        package_digest: &str,
    ) -> Result<InstalledExtensionPackage, PackageStoreError> {
        if !valid_digest(package_digest) {
            return Err(PackageStoreError::ContentAddressCollision);
        }
        let package = ValidatedPackageDirectory::validate(self.package_path(package_digest))?;
        if package.package_digest() != package_digest {
            return Err(PackageStoreError::ContentAddressCollision);
        }
        Ok(installed(&package))
    }

    pub(super) fn load_validated(
        &self,
        package_digest: &str,
    ) -> Result<ValidatedPackageDirectory, PackageStoreError> {
        if !valid_digest(package_digest) {
            return Err(PackageStoreError::ContentAddressCollision);
        }
        let package = ValidatedPackageDirectory::validate(self.package_path(package_digest))?;
        if package.package_digest() != package_digest {
            return Err(PackageStoreError::ContentAddressCollision);
        }
        Ok(package)
    }

    fn validate_dependencies(
        &self,
        candidate: &ValidatedPackageDirectory,
    ) -> Result<(), PackageStoreError> {
        for dependency in candidate.locked_dependencies() {
            let path = self.package_path(dependency.package_digest);
            if !path.is_dir() {
                return Err(PackageStoreError::MissingDependency {
                    id: dependency.id.into(),
                    version: dependency.version.into(),
                    digest: dependency.package_digest.into(),
                });
            }
            let installed = ValidatedPackageDirectory::validate(path)?;
            if installed.package_digest() != dependency.package_digest
                || installed.id() != dependency.id
                || installed.version() != dependency.version
            {
                return Err(PackageStoreError::DependencyMismatch(dependency.id.into()));
            }
        }
        Ok(())
    }

    fn require_owned_staging(&self, staging: &Path) -> Result<(), PackageStoreError> {
        let parent = staging
            .parent()
            .ok_or(PackageStoreError::InvalidStagingOwnership)?;
        let parent = fs::canonicalize(parent).map_err(|error| io_error(parent, error))?;
        if parent != self.root.join(STAGING_DIR) {
            return Err(PackageStoreError::InvalidStagingOwnership);
        }
        Ok(())
    }

    fn package_path(&self, package_digest: &str) -> PathBuf {
        self.root.join(PACKAGES_DIR).join(package_digest)
    }
}

impl InstalledExtensionPackage {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn version(&self) -> &str {
        &self.version
    }

    pub(crate) fn package_digest(&self) -> &str {
        &self.package_digest
    }
}

fn installed(package: &ValidatedPackageDirectory) -> InstalledExtensionPackage {
    InstalledExtensionPackage {
        path: package.root().to_path_buf(),
        id: package.id().into(),
        version: package.version().into(),
        package_digest: package.package_digest().into(),
    }
}

fn create_real_directory(path: &Path) -> Result<(), PackageStoreError> {
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
            if metadata.is_dir() && !metadata.file_type().is_symlink() {
                Ok(())
            } else {
                Err(PackageStoreError::Io {
                    path: path.to_path_buf(),
                    message: "expected a real directory".into(),
                })
            }
        }
        Err(error) => Err(io_error(path, error)),
    }
}

fn remove_directory(path: &Path) -> Result<(), PackageStoreError> {
    fs::remove_dir_all(path).map_err(|error| io_error(path, error))
}

fn make_tree_read_only(root: &Path) -> Result<(), PackageStoreError> {
    let mut directories = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        directories.push(directory.clone());
        for entry in fs::read_dir(&directory).map_err(|error| io_error(&directory, error))? {
            let entry = entry.map_err(|error| io_error(&directory, error))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).map_err(|error| io_error(&path, error))?;
            if metadata.is_dir() {
                pending.push(path);
            } else {
                set_read_only(&path, false)?;
            }
        }
    }
    for directory in directories.into_iter().rev() {
        set_read_only(&directory, true)?;
    }
    Ok(())
}

#[cfg(unix)]
fn set_read_only(path: &Path, directory: bool) -> Result<(), PackageStoreError> {
    use std::os::unix::fs::PermissionsExt;

    let mode = if directory { 0o555 } else { 0o444 };
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|error| io_error(path, error))
}

#[cfg(not(unix))]
fn set_read_only(path: &Path, _directory: bool) -> Result<(), PackageStoreError> {
    let mut permissions = fs::metadata(path)
        .map_err(|error| io_error(path, error))?
        .permissions();
    permissions.set_readonly(true);
    fs::set_permissions(path, permissions).map_err(|error| io_error(path, error))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn io_error(path: &Path, error: std::io::Error) -> PackageStoreError {
    PackageStoreError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};
    use tempfile::TempDir;

    use super::*;

    fn write_package(path: &Path) {
        let component = b"stored component";
        fs::write(path.join("component.wasm"), component).unwrap();
        fs::write(
            path.join("extension.json"),
            serde_json::json!({
                "schemaVersion": 2,
                "id": "example.stored",
                "version": "1.0.0",
                "api": { "requires": "^0.1" },
                "component": {
                    "path": "component.wasm",
                    "sha256": format!("{:x}", Sha256::digest(component)),
                    "world": "pi:extension/extension@0.1.0"
                },
                "lock": "extension.lock.json",
                "dependencies": [],
                "activation": ["workspace"],
                "permissions": [],
                "contributions": {},
                "resources": [],
                "limits": {
                    "memoryBytes": 65536,
                    "fuel": 1,
                    "deadlineMs": 1,
                    "outputBytes": 1
                }
            })
            .to_string(),
        )
        .unwrap();
        fs::write(
            path.join("extension.lock.json"),
            serde_json::json!({
                "schemaVersion": 1,
                "extension": { "id": "example.stored", "version": "1.0.0" },
                "dependencies": []
            })
            .to_string(),
        )
        .unwrap();
    }

    #[test]
    fn installs_and_reloads_content_addressed_package() {
        let directory = TempDir::new().unwrap();
        let store = ExtensionPackageStore::open(directory.path().join("store")).unwrap();
        let staging = store.create_staging_directory().unwrap();
        write_package(&staging);

        let installed = store.install_staged(&staging).unwrap();
        assert!(!staging.exists());
        assert_eq!(installed.id(), "example.stored");
        assert_eq!(installed.version(), "1.0.0");
        assert_eq!(installed.package_digest().len(), 64);
        assert_eq!(store.load(installed.package_digest()).unwrap(), installed);

        let duplicate = store.create_staging_directory().unwrap();
        write_package(&duplicate);
        assert_eq!(store.install_staged(&duplicate).unwrap(), installed);
        assert!(!duplicate.exists());

        #[cfg(unix)]
        assert!(
            fs::metadata(installed.path())
                .unwrap()
                .permissions()
                .readonly()
        );
    }

    #[test]
    fn rejects_staging_outside_store_ownership() {
        let directory = TempDir::new().unwrap();
        let store = ExtensionPackageStore::open(directory.path().join("store")).unwrap();
        let outside = directory.path().join("outside");
        fs::create_dir(&outside).unwrap();
        write_package(&outside);

        assert!(matches!(
            store.install_staged(outside),
            Err(PackageStoreError::InvalidStagingOwnership)
        ));
    }

    #[test]
    fn rejects_lock_that_names_uninstalled_dependency() {
        let directory = TempDir::new().unwrap();
        let store = ExtensionPackageStore::open(directory.path().join("store")).unwrap();
        let staging = store.create_staging_directory().unwrap();
        write_package(&staging);

        let manifest_path = staging.join("extension.json");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        manifest["dependencies"] = serde_json::json!([{
            "id": "example.base",
            "requires": "^2.0.0"
        }]);
        fs::write(&manifest_path, manifest.to_string()).unwrap();
        fs::write(
            staging.join("extension.lock.json"),
            serde_json::json!({
                "schemaVersion": 1,
                "extension": { "id": "example.stored", "version": "1.0.0" },
                "dependencies": [{
                    "id": "example.base",
                    "version": "2.1.0",
                    "sha256": "a".repeat(64)
                }]
            })
            .to_string(),
        )
        .unwrap();

        assert!(matches!(
            store.install_staged(staging),
            Err(PackageStoreError::MissingDependency { .. })
        ));
    }
}
