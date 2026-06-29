#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;

use super::prompt::PromptTurnTransaction;
use super::session_log::event::{OperationKind, SessionEventData, SessionEventEnvelope};
use super::session_log::id::{Clock, IdGenerator, SystemClock, SystemIdGenerator};
use super::session_log::replay::SessionReplay;
use super::session_log::store::{
    CreateSessionOptions, SessionHandle, SessionLogStore, SessionSummary,
};
use super::session_log::transaction::TurnTransaction;
use super::{
    CodingAgentSessionOptions, CodingAgentSessionSummary, CodingAgentSessionView,
    CodingSessionError,
};

#[derive(Debug)]
pub(crate) struct SessionService {
    #[allow(dead_code)]
    store: SessionLogStore,
    handle: SessionHandle,
}

impl SessionService {
    pub(crate) fn create(options: &CodingAgentSessionOptions) -> Result<Self, CodingSessionError> {
        let root = resolve_session_log_root(options)?;
        let store = SessionLogStore::new(root);
        let mut ids = SystemIdGenerator;
        let clock = SystemClock;
        let session_id = match options.session_id() {
            Some(session_id) => normalize_session_id(session_id, "session id")?,
            None => ids.next_session_id(),
        };
        Self::create_with_id(store, session_id, &mut ids, &clock)
    }

    pub(crate) fn open(options: &CodingAgentSessionOptions) -> Result<Self, CodingSessionError> {
        let root = resolve_session_log_root(options)?;
        let store = SessionLogStore::new(root);
        let target = open_target(options)?;
        let handle = store.open_session(&target)?;

        Ok(Self { store, handle })
    }

    pub(crate) fn open_or_create(
        options: &CodingAgentSessionOptions,
    ) -> Result<Self, CodingSessionError> {
        if options.session_path().is_some() {
            return Err(CodingSessionError::Input {
                message: "open-or-create requires a session id, not a session path".into(),
            });
        }
        let session_id = options
            .session_id()
            .ok_or_else(|| CodingSessionError::Input {
                message: "open-or-create requires a session id".into(),
            })
            .and_then(|session_id| normalize_session_id(session_id, "session id"))?;
        let root = resolve_session_log_root(options)?;
        let store = SessionLogStore::new(root);

        if let Some(handle) = store.try_open_session_id(&session_id)? {
            return Ok(Self { store, handle });
        }

        let mut ids = SystemIdGenerator;
        let clock = SystemClock;
        Self::create_with_id(store, session_id, &mut ids, &clock)
    }

    pub(crate) fn list(
        options: &CodingAgentSessionOptions,
    ) -> Result<Vec<CodingAgentSessionSummary>, CodingSessionError> {
        let root = resolve_session_log_root(options)?;
        let store = SessionLogStore::new(root);
        Ok(store
            .list_sessions()?
            .into_iter()
            .map(CodingAgentSessionSummary::from)
            .collect())
    }

    pub(crate) fn session_id(&self) -> &str {
        &self.handle.manifest().session_id
    }

    pub(crate) fn begin_prompt_transaction(&self) -> PromptTurnTransaction {
        TurnTransaction::begin(
            &self.store,
            self.handle.clone(),
            SystemIdGenerator,
            SystemClock,
            OperationKind::Prompt,
        )
    }

    #[cfg(test)]
    pub(crate) fn session_dir(&self) -> &Path {
        self.handle.session_dir()
    }

    #[allow(dead_code)]
    pub(crate) fn replay(&self) -> Result<SessionReplay, CodingSessionError> {
        self.store.replay_session(&self.handle)
    }

    pub(crate) fn view(&self) -> CodingAgentSessionView {
        CodingAgentSessionView {
            session_id: self.session_id().to_owned(),
        }
    }

    fn create_with_id(
        store: SessionLogStore,
        session_id: String,
        ids: &mut impl IdGenerator,
        clock: &impl Clock,
    ) -> Result<Self, CodingSessionError> {
        let created_at = clock.now_rfc3339();
        let handle =
            store.create_session(CreateSessionOptions::new(session_id, created_at.clone()))?;
        let created = SessionEventEnvelope::new(
            handle.manifest().session_id.clone(),
            ids.next_event_id(),
            created_at,
            SessionEventData::SessionCreated {
                cwd: current_dir_string(),
            },
        );
        store.append_events(&handle, &[created])?;

        Ok(Self { store, handle })
    }
}

impl From<SessionSummary> for CodingAgentSessionSummary {
    fn from(summary: SessionSummary) -> Self {
        Self {
            session_id: summary.session_id,
            session_dir: summary.session_dir,
            created_at: summary.created_at,
            updated_at: summary.updated_at,
            active_leaf_id: summary.active_leaf_id,
        }
    }
}

fn resolve_session_log_root(
    options: &CodingAgentSessionOptions,
) -> Result<PathBuf, CodingSessionError> {
    if let Some(root) = options.session_log_root() {
        return Ok(root.to_path_buf());
    }
    crate::session::default_sessions_root().map_err(|error| CodingSessionError::Session {
        message: error.to_string(),
    })
}

fn open_target(options: &CodingAgentSessionOptions) -> Result<PathBuf, CodingSessionError> {
    if let Some(path) = options.session_path() {
        return Ok(path.to_path_buf());
    }
    let session_id = options
        .session_id()
        .ok_or_else(|| CodingSessionError::Input {
            message: "opening a coding session requires a session id or session path".into(),
        })?;
    Ok(PathBuf::from(normalize_session_id(
        session_id,
        "session id",
    )?))
}

fn normalize_session_id(value: &str, label: &str) -> Result<String, CodingSessionError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CodingSessionError::Input {
            message: format!("{label} must not be empty"),
        });
    }
    Ok(trimmed.to_owned())
}

fn current_dir_string() -> Option<String> {
    std::env::current_dir()
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_uses_explicit_session_id() {
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id(" sess_test ")
            .with_session_log_root(temp.path());
        let service = SessionService::create(&options).unwrap();

        assert_eq!(service.session_id(), "sess_test");
        assert!(service.session_dir().join("session.json").is_file());
        assert!(service.session_dir().join("events.jsonl").is_file());

        let replay = service.replay().unwrap();
        assert_eq!(replay.session_id, "sess_test");
        assert!(replay.transcript.is_empty());
    }

    #[test]
    fn open_reads_rust_native_session_by_id() {
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_open")
            .with_session_log_root(temp.path());
        let created = SessionService::create(&options).unwrap();

        let opened = SessionService::open(&options).unwrap();

        assert_eq!(opened.session_id(), "sess_open");
        assert_eq!(opened.session_dir(), created.session_dir());
    }

    #[test]
    fn open_reads_rust_native_session_by_path() {
        let temp = tempfile::tempdir().unwrap();
        let create_options = CodingAgentSessionOptions::new()
            .with_session_id("sess_path")
            .with_session_log_root(temp.path());
        let created = SessionService::create(&create_options).unwrap();
        let open_options = CodingAgentSessionOptions::new()
            .with_session_log_root(temp.path())
            .with_session_path(created.session_dir());

        let opened = SessionService::open(&open_options).unwrap();

        assert_eq!(opened.session_id(), "sess_path");
    }

    #[test]
    fn open_or_create_creates_missing_explicit_session_id() {
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_open_or_create")
            .with_session_log_root(temp.path());

        let service = SessionService::open_or_create(&options).unwrap();

        assert_eq!(service.session_id(), "sess_open_or_create");
        assert!(service.session_dir().join("session.json").is_file());
        let events = service.store.read_events(&service.handle).unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0].data,
            SessionEventData::SessionCreated { .. }
        ));
    }

    #[test]
    fn open_or_create_reopens_existing_explicit_session_id() {
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_reopen")
            .with_session_log_root(temp.path());
        let created = SessionService::create(&options).unwrap();

        let opened = SessionService::open_or_create(&options).unwrap();

        assert_eq!(opened.session_id(), "sess_reopen");
        assert_eq!(opened.session_dir(), created.session_dir());
        let events = opened.store.read_events(&opened.handle).unwrap();
        assert_eq!(events.len(), 1, "open-or-create must not recreate sessions");
        assert!(matches!(
            events[0].data,
            SessionEventData::SessionCreated { .. }
        ));
    }

    #[test]
    fn open_or_create_requires_session_id() {
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new().with_session_log_root(temp.path());

        let error = SessionService::open_or_create(&options).unwrap_err();

        assert_eq!(error.code(), "input");
        assert_eq!(
            error.to_string(),
            "invalid input: open-or-create requires a session id"
        );
    }

    #[test]
    fn list_returns_session_summaries_sorted_by_updated_at() {
        let temp = tempfile::tempdir().unwrap();
        let old_options = CodingAgentSessionOptions::new()
            .with_session_id("sess_list_old")
            .with_session_log_root(temp.path());
        let new_options = CodingAgentSessionOptions::new()
            .with_session_id("sess_list_new")
            .with_session_log_root(temp.path());
        let old = SessionService::create(&old_options).unwrap();
        let new = SessionService::create(&new_options).unwrap();
        old.store
            .update_manifest(
                &old.handle,
                super::super::session_log::store::ManifestPatch::new()
                    .updated_at("2999-01-01T00:00:00Z")
                    .active_leaf_id(Some("leaf_list_old".into())),
            )
            .unwrap();

        let summaries = SessionService::list(
            &CodingAgentSessionOptions::new().with_session_log_root(temp.path()),
        )
        .unwrap();

        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].session_id, "sess_list_old");
        assert_eq!(summaries[0].session_dir, old.session_dir());
        assert_eq!(summaries[0].updated_at, "2999-01-01T00:00:00Z");
        assert_eq!(
            summaries[0].active_leaf_id.as_deref(),
            Some("leaf_list_old")
        );
        assert_eq!(summaries[1].session_id, "sess_list_new");
        assert_eq!(summaries[1].session_dir, new.session_dir());
    }

    #[test]
    fn open_requires_session_id() {
        let error = SessionService::open(&CodingAgentSessionOptions::new()).unwrap_err();

        assert_eq!(error.code(), "input");
        assert_eq!(
            error.to_string(),
            "invalid input: opening a coding session requires a session id or session path"
        );
    }
}
