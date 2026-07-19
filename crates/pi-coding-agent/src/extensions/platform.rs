use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use super::activation::{ExtensionActivationCoordinator, WorkspaceActivationSnapshot};
use super::api::{
    CodingAgentExtensionActivation, CodingAgentExtensionActivationRequest,
    CodingAgentInstalledExtensionPackage,
};
use super::grant::{
    ExtensionGrantIdentity, ExtensionGrantRegistry, ExtensionGrantScope, GrantRecord,
};
use super::store::ExtensionPackageStore;
use crate::runtime::facade::CodingSessionError;

#[derive(Debug)]
pub(crate) struct ExtensionPlatformOwner {
    package_store_root: PathBuf,
    activation_state_directory: PathBuf,
    grants: ExtensionGrantRegistry,
    observed_activation: Mutex<Option<String>>,
}

impl ExtensionPlatformOwner {
    pub(crate) fn new(package_store_root: PathBuf, activation_state_directory: PathBuf) -> Self {
        Self {
            package_store_root,
            activation_state_directory,
            grants: ExtensionGrantRegistry::default(),
            observed_activation: Mutex::new(None),
        }
    }

    pub(crate) fn reload_if_configured(
        &self,
    ) -> Result<Option<ReloadedExtensionActivation>, CodingSessionError> {
        let activation_file = self.activation_state_directory.join("activation-v1.json");
        match fs::symlink_metadata(&activation_file) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(platform_error("activation_state")),
        }
        let store = ExtensionPackageStore::open(&self.package_store_root).map_err(|_| {
            CodingSessionError::Plugin {
                message: "extension activation failed (package_store)".into(),
            }
        })?;
        let snapshot = ExtensionActivationCoordinator::new(&store, &self.grants)
            .load(&self.activation_state_directory)
            .map_err(|error| CodingSessionError::Plugin {
                message: format!("extension activation failed ({})", error.safe_code()),
            })?;
        let fingerprint = snapshot.activation_digest.clone();
        let mut observed = self
            .observed_activation
            .lock()
            .expect("extension activation observation lock poisoned");
        let capability_changed = observed.as_ref() != Some(&fingerprint);
        *observed = Some(fingerprint);
        Ok(Some(ReloadedExtensionActivation {
            snapshot,
            capability_changed,
        }))
    }

    pub(crate) fn create_staging_directory(&self) -> Result<PathBuf, CodingSessionError> {
        self.open_store()?
            .create_staging_directory()
            .map_err(|_| platform_error("package_staging"))
    }

    pub(crate) fn install_staged(
        &self,
        staging: PathBuf,
    ) -> Result<CodingAgentInstalledExtensionPackage, CodingSessionError> {
        let installed = self
            .open_store()?
            .install_staged(staging)
            .map_err(|_| platform_error("package_install"))?;
        Ok(CodingAgentInstalledExtensionPackage {
            id: installed.id().into(),
            version: installed.version().into(),
            package_digest: installed.package_digest().into(),
            path: installed.path().into(),
        })
    }

    pub(crate) fn activate(
        &self,
        request: CodingAgentExtensionActivationRequest,
    ) -> Result<CodingAgentExtensionActivation, CodingSessionError> {
        let store = self.open_store()?;
        let mut records = Vec::with_capacity(request.grants.len());
        for requested_grant in request.grants {
            let package = store
                .load_validated(&requested_grant.package_digest)
                .map_err(|_| platform_error("package_validation"))?;
            records.push(
                GrantRecord::new(
                    ExtensionGrantIdentity {
                        id: package.id().into(),
                        version: package.version().into(),
                        package_digest: package.package_digest().into(),
                    },
                    requested_grant.source_channel.into(),
                    requested_grant.source_digest,
                    requested_grant.trust.into(),
                    ExtensionGrantScope {
                        workspace_id: request.workspace_id.clone(),
                        session_ids: requested_grant.session_ids,
                    },
                    package.requested_permissions(),
                    requested_grant.permissions.into_iter().map(Into::into),
                    package.contract_world().into(),
                )
                .map_err(|_| platform_error("grant_denied"))?,
            );
        }
        self.ensure_activation_parent()?;
        let snapshot = ExtensionActivationCoordinator::new(&store, &self.grants)
            .activate(
                &self.activation_state_directory,
                request.workspace_id,
                request.root_package_digests,
                records,
            )
            .map_err(|error| platform_error(error.safe_code()))?;
        Ok(CodingAgentExtensionActivation {
            workspace_id: snapshot.workspace_id,
            root_package_digests: snapshot.roots,
            packages: snapshot
                .packages
                .into_values()
                .map(|package| CodingAgentInstalledExtensionPackage {
                    path: self
                        .package_store_root
                        .join("packages/sha256")
                        .join(&package.package_digest),
                    id: package.id,
                    version: package.version,
                    package_digest: package.package_digest,
                })
                .collect(),
        })
    }

    fn open_store(&self) -> Result<ExtensionPackageStore, CodingSessionError> {
        let parent = self
            .package_store_root
            .parent()
            .ok_or_else(|| platform_error("package_store"))?;
        fs::create_dir_all(parent).map_err(|_| platform_error("package_store"))?;
        ExtensionPackageStore::open(&self.package_store_root)
            .map_err(|_| platform_error("package_store"))
    }

    fn ensure_activation_parent(&self) -> Result<(), CodingSessionError> {
        let parent = self
            .activation_state_directory
            .parent()
            .ok_or_else(|| platform_error("activation_state"))?;
        fs::create_dir_all(parent).map_err(|_| platform_error("activation_state"))
    }
}

pub(crate) struct ReloadedExtensionActivation {
    pub(crate) snapshot: WorkspaceActivationSnapshot,
    pub(crate) capability_changed: bool,
}

fn platform_error(code: &str) -> CodingSessionError {
    CodingSessionError::Plugin {
        message: format!("extension platform failed ({code})"),
    }
}
