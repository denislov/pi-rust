use crate::CliError;
use globset::{Glob, GlobMatcher};
use pi_agent_core::api::ThinkingLevel;

#[derive(Debug)]
pub struct ModelRotation {
    pub entries: Vec<ModelRotationEntry>,
}

#[derive(Debug)]
pub struct ModelRotationEntry {
    pub pattern: String,
    pub thinking: Option<ThinkingLevel>,
    matcher: GlobMatcher,
}

impl ModelRotation {
    pub fn matches(&self, model_id: &str) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.matcher.is_match(model_id))
    }
}

pub fn parse_model_rotation(value: &str) -> Result<ModelRotation, CliError> {
    let mut entries = Vec::new();
    for raw in value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let (pattern, thinking) = match raw.rsplit_once(':') {
            Some((pattern, level)) if !pattern.is_empty() && !level.is_empty() => {
                let thinking = level.parse().map_err(CliError::InvalidInput)?;
                (pattern.to_string(), Some(thinking))
            }
            _ => (raw.to_string(), None),
        };
        let matcher = Glob::new(&pattern)
            .map_err(|error| {
                CliError::InvalidInput(format!("invalid model glob {pattern}: {error}"))
            })?
            .compile_matcher();
        entries.push(ModelRotationEntry {
            pattern,
            thinking,
            matcher,
        });
    }
    if entries.is_empty() {
        return Err(CliError::InvalidInput("--models cannot be empty".into()));
    }
    Ok(ModelRotation { entries })
}
