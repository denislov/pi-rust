use std::collections::BTreeMap;

use semver::{Version, VersionReq};
use serde::Deserialize;
use thiserror::Error;

use super::manifest::ExtensionManifestV2;

const LOCK_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ExtensionLockV1 {
    schema_version: u32,
    extension: LockedExtension,
    #[serde(default)]
    dependencies: Vec<LockedDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct LockedExtension {
    id: String,
    version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct LockedDependency {
    id: String,
    version: String,
    sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(super) enum ExtensionLockError {
    #[error("invalid extension lock JSON: {0}")]
    InvalidJson(String),
    #[error("invalid extension lock: {0}")]
    Validation(String),
}

impl ExtensionLockV1 {
    pub(super) fn parse_for_manifest(
        bytes: &[u8],
        manifest: &ExtensionManifestV2,
    ) -> Result<Self, ExtensionLockError> {
        let lock: Self = serde_json::from_slice(bytes)
            .map_err(|error| ExtensionLockError::InvalidJson(error.to_string()))?;
        lock.validate(manifest)?;
        Ok(lock)
    }

    fn validate(&self, manifest: &ExtensionManifestV2) -> Result<(), ExtensionLockError> {
        require(
            self.schema_version == LOCK_SCHEMA_VERSION,
            "schemaVersion must be 1",
        )?;
        require(
            self.extension.id == manifest.id(),
            "extension.id does not match manifest",
        )?;
        require(
            self.extension.version == manifest.version(),
            "extension.version does not match manifest",
        )?;
        require(
            self.dependencies.len() <= 64,
            "dependencies cannot contain more than 64 entries",
        )?;

        let declared = manifest
            .dependencies()
            .iter()
            .map(|dependency| (dependency.id.as_str(), dependency))
            .collect::<BTreeMap<_, _>>();
        let mut locked = BTreeMap::new();
        for dependency in &self.dependencies {
            require(
                locked.insert(dependency.id.as_str(), dependency).is_none(),
                "dependency ids must be unique",
            )?;
            let Some(requirement) = declared.get(dependency.id.as_str()) else {
                return Err(validation(format!(
                    "dependency {} is not declared by the manifest",
                    dependency.id
                )));
            };
            let version = Version::parse(&dependency.version).map_err(|error| {
                validation(format!(
                    "dependency {} has invalid version: {error}",
                    dependency.id
                ))
            })?;
            let range = VersionReq::parse(&requirement.requires).map_err(|error| {
                validation(format!(
                    "dependency {} has invalid manifest range: {error}",
                    dependency.id
                ))
            })?;
            require(
                range.matches(&version),
                &format!(
                    "dependency {} does not satisfy manifest range",
                    dependency.id
                ),
            )?;
            require(
                valid_sha256(&dependency.sha256),
                &format!(
                    "dependency {} sha256 must be lowercase SHA-256",
                    dependency.id
                ),
            )?;
        }

        for requirement in manifest.dependencies() {
            if !requirement.optional {
                require(
                    locked.contains_key(requirement.id.as_str()),
                    &format!("required dependency {} is missing", requirement.id),
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn dependencies(&self) -> impl Iterator<Item = LockedDependencyRef<'_>> {
        self.dependencies
            .iter()
            .map(|dependency| LockedDependencyRef {
                id: &dependency.id,
                version: &dependency.version,
                package_digest: &dependency.sha256,
            })
    }
}

pub(super) struct LockedDependencyRef<'a> {
    pub(super) id: &'a str,
    pub(super) version: &'a str,
    pub(super) package_digest: &'a str,
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn require(condition: bool, message: &str) -> Result<(), ExtensionLockError> {
    condition.then_some(()).ok_or_else(|| validation(message))
}

fn validation(message: impl Into<String>) -> ExtensionLockError {
    ExtensionLockError::Validation(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> ExtensionManifestV2 {
        ExtensionManifestV2::parse(
            serde_json::json!({
                "schemaVersion": 2,
                "id": "example.review",
                "version": "1.2.3",
                "api": { "requires": "^0.1" },
                "component": {
                    "path": "component.wasm",
                    "sha256": "a".repeat(64),
                    "world": "pi:extension/extension@0.1.0"
                },
                "lock": "extension.lock.json",
                "dependencies": [
                    { "id": "example.base", "requires": "^1.0.0" },
                    { "id": "example.optional", "requires": "^2.0.0", "optional": true }
                ],
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
            .to_string()
            .as_bytes(),
        )
        .unwrap()
    }

    fn lock() -> serde_json::Value {
        serde_json::json!({
            "schemaVersion": 1,
            "extension": { "id": "example.review", "version": "1.2.3" },
            "dependencies": [{
                "id": "example.base",
                "version": "1.4.0",
                "sha256": "b".repeat(64)
            }]
        })
    }

    #[test]
    fn validates_complete_dependency_lock() {
        ExtensionLockV1::parse_for_manifest(lock().to_string().as_bytes(), &manifest()).unwrap();
    }

    #[test]
    fn rejects_missing_required_and_out_of_range_dependencies() {
        let mut missing = lock();
        missing["dependencies"] = serde_json::json!([]);
        assert!(
            ExtensionLockV1::parse_for_manifest(missing.to_string().as_bytes(), &manifest())
                .unwrap_err()
                .to_string()
                .contains("required dependency")
        );

        let mut out_of_range = lock();
        out_of_range["dependencies"][0]["version"] = serde_json::json!("2.0.0");
        assert!(
            ExtensionLockV1::parse_for_manifest(out_of_range.to_string().as_bytes(), &manifest())
                .unwrap_err()
                .to_string()
                .contains("does not satisfy")
        );
    }

    #[test]
    fn rejects_undeclared_dependency_and_forged_field() {
        let mut undeclared = lock();
        undeclared["dependencies"][0]["id"] = serde_json::json!("example.other");
        assert!(
            ExtensionLockV1::parse_for_manifest(undeclared.to_string().as_bytes(), &manifest())
                .unwrap_err()
                .to_string()
                .contains("not declared")
        );

        let mut forged = lock();
        forged["source"] = serde_json::json!("trusted");
        assert!(matches!(
            ExtensionLockV1::parse_for_manifest(forged.to_string().as_bytes(), &manifest()),
            Err(ExtensionLockError::InvalidJson(_))
        ));
    }
}
