use serde::Deserialize;

#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PartialCompaction {
    pub enabled: Option<bool>,
    pub reserve_tokens: Option<u64>,
    pub keep_recent_tokens: Option<u64>,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PartialRetry {
    pub enabled: Option<bool>,
    pub max_retries: Option<u32>,
    pub base_delay_ms: Option<u64>,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PartialSettings {
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    pub default_thinking_level: Option<String>,
    pub transport: Option<String>,
    pub steering_mode: Option<String>,
    pub follow_up_mode: Option<String>,
    pub session_dir: Option<String>,
    pub compaction: Option<PartialCompaction>,
    pub retry: Option<PartialRetry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompactionSettings {
    pub enabled: bool,
    pub reserve_tokens: u64,
    pub keep_recent_tokens: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetrySettings {
    pub enabled: bool,
    pub max_retries: u32,
    pub base_delay_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    pub default_thinking_level: Option<String>,
    pub transport: String,
    pub steering_mode: String,
    pub follow_up_mode: String,
    pub session_dir: Option<String>,
    pub compaction: CompactionSettings,
    pub retry: RetrySettings,
}

fn merge_compaction(
    base: Option<PartialCompaction>,
    over: Option<PartialCompaction>,
) -> Option<PartialCompaction> {
    match (base, over) {
        (None, x) | (x, None) => x,
        (Some(b), Some(o)) => Some(PartialCompaction {
            enabled: o.enabled.or(b.enabled),
            reserve_tokens: o.reserve_tokens.or(b.reserve_tokens),
            keep_recent_tokens: o.keep_recent_tokens.or(b.keep_recent_tokens),
        }),
    }
}

fn merge_retry(base: Option<PartialRetry>, over: Option<PartialRetry>) -> Option<PartialRetry> {
    match (base, over) {
        (None, x) | (x, None) => x,
        (Some(b), Some(o)) => Some(PartialRetry {
            enabled: o.enabled.or(b.enabled),
            max_retries: o.max_retries.or(b.max_retries),
            base_delay_ms: o.base_delay_ms.or(b.base_delay_ms),
        }),
    }
}

impl PartialSettings {
    pub fn merge(self, over: PartialSettings) -> PartialSettings {
        PartialSettings {
            default_provider: over.default_provider.or(self.default_provider),
            default_model: over.default_model.or(self.default_model),
            default_thinking_level: over.default_thinking_level.or(self.default_thinking_level),
            transport: over.transport.or(self.transport),
            steering_mode: over.steering_mode.or(self.steering_mode),
            follow_up_mode: over.follow_up_mode.or(self.follow_up_mode),
            session_dir: over.session_dir.or(self.session_dir),
            compaction: merge_compaction(self.compaction, over.compaction),
            retry: merge_retry(self.retry, over.retry),
        }
    }

    pub fn resolve(self) -> Settings {
        let c = self.compaction.unwrap_or_default();
        let r = self.retry.unwrap_or_default();
        Settings {
            default_provider: self.default_provider,
            default_model: self.default_model,
            default_thinking_level: self.default_thinking_level,
            transport: self.transport.unwrap_or_else(|| "auto".to_string()),
            steering_mode: self.steering_mode.unwrap_or_else(|| "one-at-a-time".to_string()),
            follow_up_mode: self.follow_up_mode.unwrap_or_else(|| "one-at-a-time".to_string()),
            session_dir: self.session_dir,
            compaction: CompactionSettings {
                enabled: c.enabled.unwrap_or(true),
                reserve_tokens: c.reserve_tokens.unwrap_or(16384),
                keep_recent_tokens: c.keep_recent_tokens.unwrap_or(20000),
            },
            retry: RetrySettings {
                enabled: r.enabled.unwrap_or(true),
                max_retries: r.max_retries.unwrap_or(3),
                base_delay_ms: r.base_delay_ms.unwrap_or(2000),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_applied_on_empty() {
        let s = PartialSettings::default().resolve();
        assert_eq!(s.transport, "auto");
        assert_eq!(s.steering_mode, "one-at-a-time");
        assert!(s.compaction.enabled);
        assert_eq!(s.compaction.reserve_tokens, 16384);
        assert_eq!(s.compaction.keep_recent_tokens, 20000);
        assert_eq!(s.retry.max_retries, 3);
        assert_eq!(s.retry.base_delay_ms, 2000);
        assert!(s.default_model.is_none());
    }

    #[test]
    fn project_overrides_global_scalars() {
        let global = PartialSettings {
            default_model: Some("a".into()),
            transport: Some("sse".into()),
            ..Default::default()
        };
        let project = PartialSettings {
            default_model: Some("b".into()),
            ..Default::default()
        };
        let s = global.merge(project).resolve();
        assert_eq!(s.default_model.as_deref(), Some("b")); // project wins
        assert_eq!(s.transport, "sse"); // global survives where project is silent
    }

    #[test]
    fn nested_objects_merge_field_wise() {
        let global = PartialSettings {
            compaction: Some(PartialCompaction {
                reserve_tokens: Some(100),
                keep_recent_tokens: Some(200),
                ..Default::default()
            }),
            ..Default::default()
        };
        let project = PartialSettings {
            compaction: Some(PartialCompaction {
                reserve_tokens: Some(999),
                ..Default::default()
            }),
            ..Default::default()
        };
        let s = global.merge(project).resolve();
        assert_eq!(s.compaction.reserve_tokens, 999); // project overrides
        assert_eq!(s.compaction.keep_recent_tokens, 200); // global field survives
        assert!(s.compaction.enabled); // default fills the gap
    }
}
