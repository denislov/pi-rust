use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::grant::{
    ExtensionGrantError, ExtensionGrantIdentity, ExtensionGrantRegistry, ExtensionPermission,
    GrantRecord,
};
use super::package::ValidatedPackageDirectory;
use super::store::{ExtensionPackageStore, PackageStoreError};

const ACTIVATION_SCHEMA_VERSION: u32 = 1;
const ACTIVATION_FILE: &str = "activation-v1.json";
const MAX_ACTIVATION_BYTES: u64 = 1024 * 1024;
static ACTIVATION_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WorkspaceActivationRecord {
    schema_version: u32,
    workspace_id: String,
    roots: Vec<String>,
    packages: Vec<ExtensionGrantIdentity>,
    grants: Vec<GrantRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceActivationSnapshot {
    pub(crate) workspace_id: String,
    pub(crate) roots: Vec<String>,
    pub(crate) packages: BTreeMap<String, ExtensionGrantIdentity>,
    pub(crate) activation_digest: String,
}

#[derive(Debug, Error)]
pub(crate) enum ExtensionActivationError {
    #[error(transparent)]
    Package(#[from] PackageStoreError),
    #[error(transparent)]
    Grant(#[from] ExtensionGrantError),
    #[error("extension activation I/O failed at {path}: {message}")]
    Io { path: PathBuf, message: String },
    #[error("invalid extension activation: {0}")]
    Invalid(String),
    #[error("extension dependency cycle detected")]
    DependencyCycle,
    #[error("workspace activation selects multiple versions of extension {0}")]
    VersionConflict(String),
    #[error("workspace activation has no grant for extension {0}")]
    MissingGrant(String),
    #[error("workspace activation grant does not match package {0}")]
    GrantMismatch(String),
}

impl ExtensionActivationError {
    pub(crate) fn safe_code(&self) -> &'static str {
        match self {
            Self::Package(_) => "package_validation",
            Self::Grant(_) => "grant_denied",
            Self::Io { .. } => "state_io",
            Self::Invalid(_) => "invalid_record",
            Self::DependencyCycle => "dependency_cycle",
            Self::VersionConflict(_) => "version_conflict",
            Self::MissingGrant(_) => "missing_grant",
            Self::GrantMismatch(_) => "grant_mismatch",
        }
    }
}

pub(crate) struct ExtensionActivationCoordinator<'a> {
    package_store: &'a ExtensionPackageStore,
    grants: &'a ExtensionGrantRegistry,
}

impl<'a> ExtensionActivationCoordinator<'a> {
    pub(crate) fn new(
        package_store: &'a ExtensionPackageStore,
        grants: &'a ExtensionGrantRegistry,
    ) -> Self {
        Self {
            package_store,
            grants,
        }
    }

    pub(crate) fn activate(
        &self,
        state_directory: &Path,
        workspace_id: String,
        roots: Vec<String>,
        grants: Vec<GrantRecord>,
    ) -> Result<WorkspaceActivationSnapshot, ExtensionActivationError> {
        validate_digest_list(&roots)?;
        let resolved = self.resolve(&roots)?;
        validate_grants(&workspace_id, &resolved, &grants)?;
        let record = WorkspaceActivationRecord {
            schema_version: ACTIVATION_SCHEMA_VERSION,
            workspace_id: workspace_id.clone(),
            roots: roots.clone(),
            packages: resolved.values().map(package_identity).collect(),
            grants: grants.clone(),
        };
        let activation_digest = activation_digest(&record)?;
        persist_record(state_directory, &record)?;
        self.grants.replace_workspace(&workspace_id, grants)?;
        Ok(snapshot(workspace_id, roots, resolved, activation_digest))
    }

    pub(crate) fn load(
        &self,
        state_directory: &Path,
    ) -> Result<WorkspaceActivationSnapshot, ExtensionActivationError> {
        let path = state_directory.join(ACTIVATION_FILE);
        validate_state_file(state_directory, &path)?;
        let bytes = fs::read(&path).map_err(|error| io_error(&path, error))?;
        let record: WorkspaceActivationRecord = serde_json::from_slice(&bytes)
            .map_err(|error| invalid(format!("invalid JSON: {error}")))?;
        if record.schema_version != ACTIVATION_SCHEMA_VERSION {
            return Err(invalid("schemaVersion must be 1"));
        }
        validate_workspace_id(&record.workspace_id)?;
        validate_digest_list(&record.roots)?;
        let resolved = self.resolve(&record.roots)?;
        let expected = resolved.values().map(package_identity).collect::<Vec<_>>();
        if record.packages != expected {
            return Err(invalid(
                "persisted package snapshot does not match immutable store",
            ));
        }
        validate_grants(&record.workspace_id, &resolved, &record.grants)?;
        let activation_digest = activation_digest(&record)?;
        self.grants
            .replace_workspace(&record.workspace_id, record.grants)?;
        Ok(snapshot(
            record.workspace_id,
            record.roots,
            resolved,
            activation_digest,
        ))
    }

    fn resolve(
        &self,
        roots: &[String],
    ) -> Result<BTreeMap<String, ValidatedPackageDirectory>, ExtensionActivationError> {
        let mut resolved = BTreeMap::new();
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        for root in roots {
            self.visit(root, &mut resolved, &mut visiting, &mut visited)?;
        }
        Ok(resolved)
    }

    fn visit(
        &self,
        digest: &str,
        resolved: &mut BTreeMap<String, ValidatedPackageDirectory>,
        visiting: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
    ) -> Result<(), ExtensionActivationError> {
        if visited.contains(digest) {
            return Ok(());
        }
        if !visiting.insert(digest.into()) {
            return Err(ExtensionActivationError::DependencyCycle);
        }
        let package = self.package_store.load_validated(digest)?;
        if let Some(existing) = resolved.get(package.id())
            && existing.package_digest() != package.package_digest()
        {
            return Err(ExtensionActivationError::VersionConflict(
                package.id().into(),
            ));
        }
        let dependencies = package
            .locked_dependencies()
            .map(|dependency| dependency.package_digest.to_owned())
            .collect::<Vec<_>>();
        for dependency in dependencies {
            self.visit(&dependency, resolved, visiting, visited)?;
        }
        visiting.remove(digest);
        visited.insert(digest.into());
        resolved.insert(package.id().into(), package);
        Ok(())
    }
}

fn validate_grants(
    workspace_id: &str,
    packages: &BTreeMap<String, ValidatedPackageDirectory>,
    grants: &[GrantRecord],
) -> Result<(), ExtensionActivationError> {
    validate_workspace_id(workspace_id)?;
    if packages.len() > 256 || grants.len() > 256 {
        return Err(invalid("activation cannot contain more than 256 packages"));
    }
    let grant_by_id = grants
        .iter()
        .map(|grant| (grant.extension.id.as_str(), grant))
        .collect::<BTreeMap<_, _>>();
    if grant_by_id.len() != grants.len() {
        return Err(invalid("grant extension ids must be unique"));
    }
    if grant_by_id.len() != packages.len() {
        return Err(invalid(
            "every activated package must have exactly one grant",
        ));
    }
    for (id, package) in packages {
        let grant = grant_by_id
            .get(id.as_str())
            .ok_or_else(|| ExtensionActivationError::MissingGrant(id.clone()))?;
        if grant.scope.workspace_id != workspace_id
            || grant.extension.version != package.version()
            || grant.extension.package_digest != package.package_digest()
            || grant.contract_world != package.contract_world()
        {
            return Err(ExtensionActivationError::GrantMismatch(id.clone()));
        }
        let requested = package
            .requested_permissions()
            .iter()
            .map(|permission| ExtensionPermission::parse(permission))
            .collect::<Result<BTreeSet<_>, _>>()?;
        if !grant.permissions.is_subset(&requested) {
            return Err(ExtensionActivationError::GrantMismatch(id.clone()));
        }
    }
    Ok(())
}

fn snapshot(
    workspace_id: String,
    roots: Vec<String>,
    packages: BTreeMap<String, ValidatedPackageDirectory>,
    activation_digest: String,
) -> WorkspaceActivationSnapshot {
    WorkspaceActivationSnapshot {
        workspace_id,
        roots,
        packages: packages
            .into_iter()
            .map(|(id, package)| (id, package_identity(&package)))
            .collect(),
        activation_digest,
    }
}

fn activation_digest(
    record: &WorkspaceActivationRecord,
) -> Result<String, ExtensionActivationError> {
    let bytes = serde_json::to_vec(record)
        .map_err(|error| invalid(format!("cannot serialize activation: {error}")))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn package_identity(package: &ValidatedPackageDirectory) -> ExtensionGrantIdentity {
    ExtensionGrantIdentity {
        id: package.id().into(),
        version: package.version().into(),
        package_digest: package.package_digest().into(),
    }
}

fn persist_record(
    state_directory: &Path,
    record: &WorkspaceActivationRecord,
) -> Result<(), ExtensionActivationError> {
    create_real_directory(state_directory)?;
    let sequence = ACTIVATION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = state_directory.join(format!(
        ".{ACTIVATION_FILE}.{}.{sequence}.tmp",
        std::process::id()
    ));
    let target = state_directory.join(ACTIVATION_FILE);
    let bytes = serde_json::to_vec_pretty(record)
        .map_err(|error| invalid(format!("cannot serialize activation: {error}")))?;
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|error| io_error(&temporary, error))?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| io_error(&temporary, error))?;
    fs::rename(&temporary, &target).map_err(|error| io_error(&target, error))?;
    #[cfg(unix)]
    fs::File::open(state_directory)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| io_error(state_directory, error))?;
    Ok(())
}

fn create_real_directory(path: &Path) -> Result<(), ExtensionActivationError> {
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
            if metadata.is_dir() && !metadata.file_type().is_symlink() {
                Ok(())
            } else {
                Err(invalid("activation state path must be a real directory"))
            }
        }
        Err(error) => Err(io_error(path, error)),
    }
}

fn validate_digest_list(digests: &[String]) -> Result<(), ExtensionActivationError> {
    if digests.is_empty() || digests.len() > 256 {
        return Err(invalid("activation requires 1 to 256 root packages"));
    }
    let unique = digests.iter().collect::<BTreeSet<_>>();
    if unique.len() != digests.len()
        || digests.iter().any(|digest| {
            digest.len() != 64
                || !digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
    {
        return Err(invalid(
            "root package digests must be unique lowercase SHA-256",
        ));
    }
    Ok(())
}

fn validate_state_file(
    state_directory: &Path,
    activation_file: &Path,
) -> Result<(), ExtensionActivationError> {
    let directory =
        fs::symlink_metadata(state_directory).map_err(|error| io_error(state_directory, error))?;
    if !directory.is_dir() || directory.file_type().is_symlink() {
        return Err(invalid("activation state path must be a real directory"));
    }
    let record =
        fs::symlink_metadata(activation_file).map_err(|error| io_error(activation_file, error))?;
    if !record.is_file() || record.file_type().is_symlink() {
        return Err(invalid("activation record must be a regular file"));
    }
    if record.len() > MAX_ACTIVATION_BYTES {
        return Err(invalid("activation record exceeds size limit"));
    }
    Ok(())
}

fn validate_workspace_id(workspace_id: &str) -> Result<(), ExtensionActivationError> {
    if workspace_id.is_empty() || workspace_id.len() > 256 {
        Err(invalid("workspace id must contain 1 to 256 bytes"))
    } else {
        Ok(())
    }
}

fn io_error(path: &Path, error: std::io::Error) -> ExtensionActivationError {
    ExtensionActivationError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

fn invalid(message: impl Into<String>) -> ExtensionActivationError {
    ExtensionActivationError::Invalid(message.into())
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use sha2::{Digest, Sha256};
    use tempfile::TempDir;
    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::extensions::grant::{
        ExtensionGrantScope, ExtensionSourceChannel, ExtensionTrustLevel,
    };
    use crate::extensions::store::InstalledExtensionPackage;

    fn install_package(
        store: &ExtensionPackageStore,
        id: &str,
        version: &str,
        permissions: &[&str],
        dependencies: &[InstalledExtensionPackage],
    ) -> InstalledExtensionPackage {
        let staging = store.create_staging_directory().unwrap();
        let component = format!("component:{id}:{version}");
        fs::write(staging.join("component.wasm"), &component).unwrap();
        let manifest_dependencies = dependencies
            .iter()
            .map(|dependency| {
                serde_json::json!({
                    "id": dependency.id(),
                    "requires": format!("={}", dependency.version())
                })
            })
            .collect::<Vec<_>>();
        let locked_dependencies = dependencies
            .iter()
            .map(|dependency| {
                serde_json::json!({
                    "id": dependency.id(),
                    "version": dependency.version(),
                    "sha256": dependency.package_digest()
                })
            })
            .collect::<Vec<_>>();
        fs::write(
            staging.join("extension.json"),
            serde_json::json!({
                "schemaVersion": 2,
                "id": id,
                "version": version,
                "api": { "requires": "^0.1" },
                "component": {
                    "path": "component.wasm",
                    "sha256": format!("{:x}", Sha256::digest(component.as_bytes())),
                    "world": "pi:extension/extension@0.1.0"
                },
                "lock": "extension.lock.json",
                "dependencies": manifest_dependencies,
                "activation": ["workspace"],
                "permissions": permissions,
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
            staging.join("extension.lock.json"),
            serde_json::json!({
                "schemaVersion": 1,
                "extension": { "id": id, "version": version },
                "dependencies": locked_dependencies
            })
            .to_string(),
        )
        .unwrap();
        store.install_staged(staging).unwrap()
    }

    fn grant(
        package: &InstalledExtensionPackage,
        requested: &[&str],
        granted: &[ExtensionPermission],
    ) -> GrantRecord {
        GrantRecord::new(
            ExtensionGrantIdentity {
                id: package.id().into(),
                version: package.version().into(),
                package_digest: package.package_digest().into(),
            },
            ExtensionSourceChannel::Registry,
            "c".repeat(64),
            ExtensionTrustLevel::Verified,
            ExtensionGrantScope {
                workspace_id: "workspace-1".into(),
                session_ids: BTreeSet::new(),
            },
            requested.iter().copied(),
            granted.iter().copied(),
            "pi:extension/extension@0.1.0".into(),
        )
        .unwrap()
    }

    fn operation_scope() -> super::super::grant::ExtensionOperationScope {
        super::super::grant::ExtensionOperationScope {
            workspace_id: "workspace-1".into(),
            session_id: None,
        }
    }

    #[test]
    fn activation_persists_graph_and_does_not_transfer_dependency_permissions() {
        let directory = TempDir::new().unwrap();
        let store = ExtensionPackageStore::open(directory.path().join("store")).unwrap();
        let dependency = install_package(&store, "example.base", "1.0.0", &["workspace.read"], &[]);
        let root = install_package(
            &store,
            "example.review",
            "1.0.0",
            &[],
            std::slice::from_ref(&dependency),
        );
        let grants = ExtensionGrantRegistry::default();
        let coordinator = ExtensionActivationCoordinator::new(&store, &grants);

        let snapshot = coordinator
            .activate(
                &directory.path().join("state"),
                "workspace-1".into(),
                vec![root.package_digest().into()],
                vec![
                    grant(
                        &dependency,
                        &["workspace.read"],
                        &[ExtensionPermission::WorkspaceRead],
                    ),
                    grant(&root, &[], &[]),
                ],
            )
            .unwrap();

        assert_eq!(snapshot.packages.len(), 2);
        assert!(directory.path().join("state/activation-v1.json").is_file());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(directory.path().join("state/activation-v1.json"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        let root_lease = grants
            .admit(
                "workspace-1",
                "example.review",
                "op-root".into(),
                operation_scope(),
                Instant::now() + Duration::from_secs(10),
                CancellationToken::new(),
            )
            .unwrap();
        assert_eq!(
            root_lease.authorize(
                "op-root",
                &operation_scope(),
                ExtensionPermission::WorkspaceRead,
            ),
            Err(ExtensionGrantError::PermissionDenied)
        );
        let dependency_lease = grants
            .admit(
                "workspace-1",
                "example.base",
                "op-dependency".into(),
                operation_scope(),
                Instant::now() + Duration::from_secs(10),
                CancellationToken::new(),
            )
            .unwrap();
        dependency_lease
            .authorize(
                "op-dependency",
                &operation_scope(),
                ExtensionPermission::WorkspaceRead,
            )
            .unwrap();

        let restarted_grants = ExtensionGrantRegistry::default();
        let restarted = ExtensionActivationCoordinator::new(&store, &restarted_grants)
            .load(&directory.path().join("state"))
            .unwrap();
        assert_eq!(restarted, snapshot);
        assert!(
            restarted_grants
                .admit(
                    "workspace-1",
                    "example.review",
                    "op-restarted".into(),
                    operation_scope(),
                    Instant::now() + Duration::from_secs(10),
                    CancellationToken::new(),
                )
                .is_ok()
        );
    }

    #[test]
    fn activation_rejects_multiple_versions_of_one_id() {
        let directory = TempDir::new().unwrap();
        let store = ExtensionPackageStore::open(directory.path().join("store")).unwrap();
        let first = install_package(&store, "example.same", "1.0.0", &[], &[]);
        let second = install_package(&store, "example.same", "2.0.0", &[], &[]);
        let grants = ExtensionGrantRegistry::default();
        let coordinator = ExtensionActivationCoordinator::new(&store, &grants);

        assert!(matches!(
            coordinator.activate(
                &directory.path().join("state"),
                "workspace-1".into(),
                vec![first.package_digest().into(), second.package_digest().into()],
                vec![grant(&first, &[], &[]), grant(&second, &[], &[])],
            ),
            Err(ExtensionActivationError::VersionConflict(id)) if id == "example.same"
        ));
    }

    #[test]
    fn activation_rejects_unsupported_requested_permission() {
        let directory = TempDir::new().unwrap();
        let store = ExtensionPackageStore::open(directory.path().join("store")).unwrap();
        let package = install_package(&store, "example.unsafe", "1.0.0", &["network.raw"], &[]);
        let grants = ExtensionGrantRegistry::default();
        let coordinator = ExtensionActivationCoordinator::new(&store, &grants);

        assert!(matches!(
            coordinator.activate(
                &directory.path().join("state"),
                "workspace-1".into(),
                vec![package.package_digest().into()],
                vec![grant(&package, &[], &[])],
            ),
            Err(ExtensionActivationError::Grant(
                ExtensionGrantError::UnsupportedPermission(permission)
            )) if permission == "network.raw"
        ));
        assert!(!directory.path().join("state/activation-v1.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn restart_rejects_symlinked_activation_record() {
        use std::os::unix::fs::symlink;

        let directory = TempDir::new().unwrap();
        let store = ExtensionPackageStore::open(directory.path().join("store")).unwrap();
        let package = install_package(&store, "example.link", "1.0.0", &[], &[]);
        let grants = ExtensionGrantRegistry::default();
        let coordinator = ExtensionActivationCoordinator::new(&store, &grants);
        let state = directory.path().join("state");
        coordinator
            .activate(
                &state,
                "workspace-1".into(),
                vec![package.package_digest().into()],
                vec![grant(&package, &[], &[])],
            )
            .unwrap();
        let record = state.join("activation-v1.json");
        let moved = state.join("activation-v1.real.json");
        fs::rename(&record, &moved).unwrap();
        symlink(&moved, &record).unwrap();

        assert!(matches!(
            coordinator.load(&state),
            Err(ExtensionActivationError::Invalid(message))
                if message.contains("regular file")
        ));
    }
}
