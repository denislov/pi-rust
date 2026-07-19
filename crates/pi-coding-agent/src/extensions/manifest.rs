use std::collections::BTreeSet;

use semver::{Version, VersionReq};
use serde::Deserialize;
use thiserror::Error;

const MANIFEST_SCHEMA_VERSION: u32 = 2;
const COMPONENT_PATH: &str = "component.wasm";
const LOCK_PATH: &str = "extension.lock.json";
const WIT_WORLD: &str = "pi:extension/extension@0.1.0";

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ExtensionManifestV2 {
    schema_version: u32,
    id: String,
    version: String,
    api: ApiRequirement,
    component: ComponentArtifact,
    lock: String,
    #[serde(default)]
    dependencies: Vec<DependencyRequirement>,
    activation: Vec<ActivationKind>,
    permissions: Vec<String>,
    contributions: ContributionInventory,
    #[serde(default)]
    resources: Vec<String>,
    limits: ExtensionLimits,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApiRequirement {
    requires: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ComponentArtifact {
    path: String,
    sha256: String,
    world: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DependencyRequirement {
    pub(super) id: String,
    pub(super) requires: String,
    #[serde(default)]
    pub(super) optional: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ActivationKind {
    Workspace,
    Session,
    Explicit,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContributionInventory {
    #[serde(default)]
    tools: Vec<ContributionHandler>,
    #[serde(default)]
    commands: Vec<ContributionHandler>,
    #[serde(default)]
    prompt_hooks: Vec<ContributionHandler>,
    #[serde(default)]
    ui_actions: Vec<ContributionHandler>,
    #[serde(default)]
    dialogs: Vec<ContributionHandler>,
    #[serde(default)]
    keybindings: Vec<ContributionHandler>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContributionHandler {
    id: String,
    handler: String,
    schema_revision: u32,
    #[serde(default)]
    optional: bool,
    definition: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExtensionLimits {
    memory_bytes: u64,
    fuel: u64,
    deadline_ms: u64,
    output_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum ExtensionManifestError {
    #[error("invalid extension manifest JSON: {0}")]
    InvalidJson(String),
    #[error("invalid extension manifest: {0}")]
    Validation(String),
}

impl ExtensionManifestV2 {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, ExtensionManifestError> {
        let manifest: Self = serde_json::from_slice(bytes)
            .map_err(|error| ExtensionManifestError::InvalidJson(error.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn version(&self) -> &str {
        &self.version
    }

    pub(crate) fn component_digest(&self) -> &str {
        &self.component.sha256
    }

    pub(super) fn dependencies(&self) -> &[DependencyRequirement] {
        &self.dependencies
    }

    pub(super) fn resources(&self) -> &[String] {
        &self.resources
    }

    pub(super) fn permissions(&self) -> &[String] {
        &self.permissions
    }

    pub(super) fn contract_world(&self) -> &str {
        &self.component.world
    }

    fn validate(&self) -> Result<(), ExtensionManifestError> {
        require(
            self.schema_version == MANIFEST_SCHEMA_VERSION,
            "schemaVersion must be 2",
        )?;
        require(valid_id(&self.id), "id has an invalid extension identifier")?;
        Version::parse(&self.version)
            .map_err(|error| validation(format!("version is not semantic: {error}")))?;
        VersionReq::parse(&self.api.requires)
            .map_err(|error| validation(format!("api.requires is invalid: {error}")))?;
        require(
            self.component.path == COMPONENT_PATH,
            "component.path must be component.wasm",
        )?;
        require(
            self.component.world == WIT_WORLD,
            "component.world is unsupported",
        )?;
        require(
            valid_sha256(&self.component.sha256),
            "component.sha256 must be lowercase SHA-256",
        )?;
        require(self.lock == LOCK_PATH, "lock must be extension.lock.json")?;
        require(!self.activation.is_empty(), "activation cannot be empty")?;
        require(
            unique(self.activation.iter().map(|kind| format!("{kind:?}"))),
            "activation values must be unique",
        )?;
        require(
            unique(self.permissions.iter().map(String::as_str)),
            "permissions must be unique",
        )?;
        require(
            self.permissions
                .iter()
                .all(|permission| valid_id(permission)),
            "permission identifier is invalid",
        )?;
        require(
            unique(self.resources.iter().map(String::as_str)),
            "resources must be unique",
        )?;
        require(
            self.resources.iter().all(|path| valid_resource_path(path)),
            "resource path is invalid",
        )?;
        self.validate_dependencies()?;
        self.validate_contributions()?;
        self.validate_limits()
    }

    fn validate_dependencies(&self) -> Result<(), ExtensionManifestError> {
        require(
            self.dependencies.len() <= 64,
            "dependencies cannot contain more than 64 entries",
        )?;
        require(
            unique(
                self.dependencies
                    .iter()
                    .map(|dependency| dependency.id.as_str()),
            ),
            "dependency ids must be unique",
        )?;
        for dependency in &self.dependencies {
            require(valid_id(&dependency.id), "dependency id is invalid")?;
            VersionReq::parse(&dependency.requires).map_err(|error| {
                validation(format!(
                    "dependency {} has invalid range: {error}",
                    dependency.id
                ))
            })?;
            let _ = dependency.optional;
        }
        Ok(())
    }

    fn validate_contributions(&self) -> Result<(), ExtensionManifestError> {
        let groups: [&[ContributionHandler]; 6] = [
            &self.contributions.tools,
            &self.contributions.commands,
            &self.contributions.prompt_hooks,
            &self.contributions.ui_actions,
            &self.contributions.dialogs,
            &self.contributions.keybindings,
        ];
        let mut ids = BTreeSet::new();
        for contribution in groups.into_iter().flatten() {
            require(valid_id(&contribution.id), "contribution id is invalid")?;
            require(
                valid_id(&contribution.handler),
                "contribution handler is invalid",
            )?;
            require(
                contribution.schema_revision == 1,
                "contribution schemaRevision must be 1",
            )?;
            require(
                ids.insert(contribution.id.as_str()),
                "contribution ids must be unique across kinds",
            )?;
            let _ = contribution.optional;
            let _ = &contribution.definition;
        }
        Ok(())
    }

    fn validate_limits(&self) -> Result<(), ExtensionManifestError> {
        require(
            (65_536..=268_435_456).contains(&self.limits.memory_bytes),
            "limits.memoryBytes is out of range",
        )?;
        require(
            (1..=1_000_000_000).contains(&self.limits.fuel),
            "limits.fuel is out of range",
        )?;
        require(
            (1..=300_000).contains(&self.limits.deadline_ms),
            "limits.deadlineMs is out of range",
        )?;
        require(
            (1..=16_777_216).contains(&self.limits.output_bytes),
            "limits.outputBytes is out of range",
        )
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_lowercase()
        && value.split(['.', '-']).all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_resource_path(path: &str) -> bool {
    path.starts_with("resources/")
        && !path.ends_with('/')
        && !path.contains("//")
        && path
            .split('/')
            .all(|segment| !matches!(segment, "" | "." | ".."))
}

fn unique<T: Ord>(values: impl IntoIterator<Item = T>) -> bool {
    let mut seen = BTreeSet::new();
    values.into_iter().all(|value| seen.insert(value))
}

fn require(condition: bool, message: &str) -> Result<(), ExtensionManifestError> {
    condition.then_some(()).ok_or_else(|| validation(message))
}

fn validation(message: impl Into<String>) -> ExtensionManifestError {
    ExtensionManifestError::Validation(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> serde_json::Value {
        serde_json::json!({
            "schemaVersion": 2,
            "id": "example.review",
            "version": "1.2.3",
            "api": { "requires": ">=0.1.0, <0.2.0" },
            "component": {
                "path": "component.wasm",
                "sha256": "a".repeat(64),
                "world": "pi:extension/extension@0.1.0"
            },
            "lock": "extension.lock.json",
            "dependencies": [{ "id": "example.base", "requires": "^1.0.0" }],
            "activation": ["workspace"],
            "permissions": ["workspace.read"],
            "contributions": {
                "tools": [{
                    "id": "review.run",
                    "handler": "review.run",
                    "schemaRevision": 1,
                    "definition": { "description": "Review files" }
                }]
            },
            "resources": ["resources/prompts/review.txt"],
            "limits": {
                "memoryBytes": 1048576,
                "fuel": 100000,
                "deadlineMs": 5000,
                "outputBytes": 65536
            }
        })
    }

    #[test]
    fn parses_valid_manifest_v2() {
        let manifest =
            ExtensionManifestV2::parse(&serde_json::to_vec(&manifest()).unwrap()).unwrap();

        assert_eq!(manifest.id(), "example.review");
        assert_eq!(manifest.version(), "1.2.3");
        assert_eq!(manifest.component_digest(), "a".repeat(64));
    }

    #[test]
    fn rejects_manifest_declared_runtime_source_and_trust() {
        for field in ["runtime", "source", "trust"] {
            let mut value = manifest();
            value[field] = serde_json::json!("forged");
            let error =
                ExtensionManifestV2::parse(&serde_json::to_vec(&value).unwrap()).unwrap_err();
            assert!(
                matches!(error, ExtensionManifestError::InvalidJson(_)),
                "{error}"
            );
        }
    }

    #[test]
    fn rejects_duplicate_contribution_ids_across_kinds() {
        let mut value = manifest();
        value["contributions"]["commands"] = value["contributions"]["tools"].clone();

        let error = ExtensionManifestV2::parse(&serde_json::to_vec(&value).unwrap()).unwrap_err();

        assert!(error.to_string().contains("unique across kinds"));
    }

    #[test]
    fn rejects_invalid_digest_ranges_and_resource_escape() {
        let mutations = [
            ("component", "sha256", serde_json::json!("ABC")),
            ("api", "requires", serde_json::json!("not a range")),
        ];
        for (owner, field, replacement) in mutations {
            let mut value = manifest();
            value[owner][field] = replacement;
            assert!(ExtensionManifestV2::parse(&serde_json::to_vec(&value).unwrap()).is_err());
        }
        let mut value = manifest();
        value["resources"] = serde_json::json!(["resources/../secret"]);
        assert!(ExtensionManifestV2::parse(&serde_json::to_vec(&value).unwrap()).is_err());
    }
}
