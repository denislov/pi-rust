use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ContributionKind {
    Tool,
    Command,
    PromptHook,
    UiAction,
    Dialog,
    Keybinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
// Product core contribution assembly begins in EKR-005. Keeping construction
// private prevents package/WIT data from manufacturing this target meanwhile.
#[allow(dead_code)]
pub(crate) struct CoreHandlerRef {
    pub(crate) kind: ContributionKind,
    pub(crate) handler_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ExtensionHandlerRef {
    pub(crate) extension_id: String,
    pub(crate) package_digest: String,
    pub(crate) kind: ContributionKind,
    pub(crate) handler_id: String,
    pub(crate) schema_revision: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "target", content = "handler", rename_all = "snake_case")]
pub(crate) enum HandlerTarget {
    // EKR-005 wires product-owned built-ins after the target boundary is fixed.
    #[allow(dead_code)]
    Core(CoreHandlerRef),
    Extension(ExtensionHandlerRef),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum HandlerTargetError {
    #[error("invalid contribution handler reference: {0}")]
    Invalid(&'static str),
}

#[allow(dead_code)]
impl CoreHandlerRef {
    pub(crate) fn new(
        kind: ContributionKind,
        handler_id: impl Into<String>,
    ) -> Result<Self, HandlerTargetError> {
        let handler_id = handler_id.into();
        validate_handler_id(&handler_id)?;
        Ok(Self { kind, handler_id })
    }
}

impl ExtensionHandlerRef {
    pub(crate) fn new(
        extension_id: impl Into<String>,
        package_digest: impl Into<String>,
        kind: ContributionKind,
        handler_id: impl Into<String>,
        schema_revision: u32,
    ) -> Result<Self, HandlerTargetError> {
        let extension_id = extension_id.into();
        let package_digest = package_digest.into();
        let handler_id = handler_id.into();
        validate_extension_id(&extension_id)?;
        validate_package_digest(&package_digest)?;
        validate_handler_id(&handler_id)?;
        if schema_revision != 1 {
            return Err(HandlerTargetError::Invalid(
                "schema revision must be supported",
            ));
        }
        Ok(Self {
            extension_id,
            package_digest,
            kind,
            handler_id,
            schema_revision,
        })
    }
}

impl HandlerTarget {
    pub(crate) fn extension(handler: ExtensionHandlerRef) -> Self {
        Self::Extension(handler)
    }

    pub(crate) fn dispatch<R>(
        &self,
        core: impl FnOnce(&CoreHandlerRef) -> R,
        extension: impl FnOnce(&ExtensionHandlerRef) -> R,
    ) -> R {
        match self {
            Self::Core(handler) => core(handler),
            Self::Extension(handler) => extension(handler),
        }
    }
}

fn validate_extension_id(value: &str) -> Result<(), HandlerTargetError> {
    if value.is_empty()
        || value.len() > 128
        || !value.as_bytes()[0].is_ascii_lowercase()
        || !value.split(['.', '-']).all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
    {
        return Err(HandlerTargetError::Invalid("extension id is invalid"));
    }
    Ok(())
}

fn validate_handler_id(value: &str) -> Result<(), HandlerTargetError> {
    validate_extension_id(value).map_err(|_| HandlerTargetError::Invalid("handler id is invalid"))
}

fn validate_package_digest(value: &str) -> Result<(), HandlerTargetError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(HandlerTargetError::Invalid("package digest is invalid"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_is_exhaustive_between_core_and_extension_targets() {
        let core = HandlerTarget::Core(
            CoreHandlerRef::new(ContributionKind::Command, "core.session.export").unwrap(),
        );
        let extension = HandlerTarget::Extension(
            ExtensionHandlerRef::new(
                "example.review",
                "a".repeat(64),
                ContributionKind::Tool,
                "review.run",
                1,
            )
            .unwrap(),
        );

        assert_eq!(core.dispatch(|_| "core", |_| "extension"), "core");
        assert_eq!(extension.dispatch(|_| "core", |_| "extension"), "extension");
    }

    #[test]
    fn serialized_target_is_language_neutral_and_authority_free() {
        let target = HandlerTarget::Extension(
            ExtensionHandlerRef::new(
                "example.review",
                "a".repeat(64),
                ContributionKind::Tool,
                "review.run",
                1,
            )
            .unwrap(),
        );
        let json = serde_json::to_value(target).unwrap();

        assert_eq!(json["target"], "extension");
        assert_eq!(json["handler"]["handlerId"], "review.run");
        let text = json.to_string();
        for forbidden in [
            "trait",
            "service",
            "repository",
            "provider",
            "coreHandlerId",
        ] {
            assert!(!text.contains(forbidden));
        }
    }

    #[test]
    fn malformed_extension_identity_digest_and_revision_fail_closed() {
        for result in [
            ExtensionHandlerRef::new(
                "bad..id",
                "a".repeat(64),
                ContributionKind::Tool,
                "handler.run",
                1,
            ),
            ExtensionHandlerRef::new("valid.id", "bad", ContributionKind::Tool, "handler.run", 1),
            ExtensionHandlerRef::new(
                "valid.id",
                "a".repeat(64),
                ContributionKind::Tool,
                "handler.run",
                2,
            ),
        ] {
            assert!(result.is_err());
        }
    }
}
