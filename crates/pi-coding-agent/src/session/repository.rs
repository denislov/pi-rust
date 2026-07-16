use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Component, Path, PathBuf};
#[cfg(test)]
use std::sync::{Arc, Mutex};

use serde_json::Value;

use super::manifest::{
    EVENT_SCHEMA, EVENT_VERSION, SESSION_EVENT_LOG_FILE, SESSION_MANIFEST_FILE, SESSION_SCHEMA,
    SESSION_VERSION, SessionManifest, default_agent_profile_id,
};
use super::replay::{SessionReplay, fold_events};
use crate::runtime::facade::{CodingSessionError, ProfileId};
use crate::session::event::SessionEventEnvelope;

#[derive(Debug, Clone)]
pub(crate) struct SessionLogStore {
    root: PathBuf,
    #[cfg(test)]
    failures: Arc<Mutex<StoreFailureState>>,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StoreFailurePoint {
    CreateBlobs,
    CreateIndex,
    WriteManifest,
    CreateEventLog,
    AppendEvents,
    UpdateManifest,
    RemoveSession,
}

#[cfg(test)]
#[derive(Debug, Default)]
struct StoreFailureState {
    create_blobs: Option<usize>,
    create_index: Option<usize>,
    write_manifest: Option<usize>,
    create_event_log: Option<usize>,
    append_events: Option<usize>,
    update_manifest: Option<usize>,
    remove_session: Option<usize>,
}

#[derive(Debug)]
pub(crate) enum SessionCreateError {
    Create(CodingSessionError),
    CleanupFailed {
        session_id: String,
        session_dir: PathBuf,
        create_error: CodingSessionError,
        cleanup_error: CodingSessionError,
    },
}

impl SessionCreateError {
    #[cfg(test)]
    pub(crate) fn code(&self) -> &'static str {
        "session"
    }
}

impl fmt::Display for SessionCreateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Create(error) => error.fmt(formatter),
            Self::CleanupFailed {
                session_id,
                session_dir,
                create_error,
                cleanup_error,
            } => write!(
                formatter,
                "session initialization failed for {session_id} at {}: {create_error}; cleanup failed: {cleanup_error}",
                session_dir.display()
            ),
        }
    }
}

impl std::error::Error for SessionCreateError {}

impl From<CodingSessionError> for SessionCreateError {
    fn from(error: CodingSessionError) -> Self {
        Self::Create(error)
    }
}

impl From<SessionCreateError> for CodingSessionError {
    fn from(error: SessionCreateError) -> Self {
        match error {
            SessionCreateError::Create(error) => error,
            cleanup_failed @ SessionCreateError::CleanupFailed { .. } => {
                CodingSessionError::Session {
                    message: cleanup_failed.to_string(),
                }
            }
        }
    }
}

impl SessionLogStore {
    pub(crate) fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            #[cfg(test)]
            failures: Arc::new(Mutex::new(StoreFailureState::default())),
        }
    }

    #[cfg(test)]
    pub(crate) fn fail_after(&self, point: StoreFailurePoint, successful_calls: usize) {
        let mut failures = self.failures.lock().unwrap();
        let target = match point {
            StoreFailurePoint::CreateBlobs => &mut failures.create_blobs,
            StoreFailurePoint::CreateIndex => &mut failures.create_index,
            StoreFailurePoint::WriteManifest => &mut failures.write_manifest,
            StoreFailurePoint::CreateEventLog => &mut failures.create_event_log,
            StoreFailurePoint::AppendEvents => &mut failures.append_events,
            StoreFailurePoint::UpdateManifest => &mut failures.update_manifest,
            StoreFailurePoint::RemoveSession => &mut failures.remove_session,
        };
        *target = Some(successful_calls);
    }

    #[cfg(test)]
    fn fail_if_injected(&self, point: StoreFailurePoint) -> Result<(), CodingSessionError> {
        let mut failures = self.failures.lock().unwrap();
        let target = match point {
            StoreFailurePoint::CreateBlobs => &mut failures.create_blobs,
            StoreFailurePoint::CreateIndex => &mut failures.create_index,
            StoreFailurePoint::WriteManifest => &mut failures.write_manifest,
            StoreFailurePoint::CreateEventLog => &mut failures.create_event_log,
            StoreFailurePoint::AppendEvents => &mut failures.append_events,
            StoreFailurePoint::UpdateManifest => &mut failures.update_manifest,
            StoreFailurePoint::RemoveSession => &mut failures.remove_session,
        };
        let Some(remaining) = target.as_mut() else {
            return Ok(());
        };
        if *remaining > 0 {
            *remaining -= 1;
            return Ok(());
        }
        *target = None;
        Err(session_error(format!(
            "injected session store failure at {point:?}"
        )))
    }

    pub(crate) fn create_session(
        &self,
        options: CreateSessionOptions,
    ) -> Result<SessionHandle, SessionCreateError> {
        let session_id = normalize_session_id(&options.session_id)?;
        fs::create_dir_all(&self.root).map_err(|error| {
            session_error(format!(
                "failed to create session log root {}: {error}",
                self.root.display()
            ))
        })?;

        let session_dir = self.root.join(&session_id);
        if session_dir.exists() {
            return Err(session_error(format!(
                "session directory already exists: {}",
                session_dir.display()
            ))
            .into());
        }

        fs::create_dir(&session_dir).map_err(|error| {
            session_error(format!(
                "failed to create session directory {}: {error}",
                session_dir.display()
            ))
        })?;
        let manifest = SessionManifest::new(session_id.clone(), options.created_at)
            .with_default_agent_profile_id(options.default_agent_profile_id);
        let initialization = (|| -> Result<(), CodingSessionError> {
            #[cfg(test)]
            self.fail_if_injected(StoreFailurePoint::CreateBlobs)?;
            fs::create_dir(session_dir.join("blobs")).map_err(|error| {
                session_error(format!(
                    "failed to create blobs directory for {session_id}: {error}"
                ))
            })?;
            #[cfg(test)]
            self.fail_if_injected(StoreFailurePoint::CreateIndex)?;
            fs::create_dir(session_dir.join("index")).map_err(|error| {
                session_error(format!(
                    "failed to create index directory for {session_id}: {error}"
                ))
            })?;
            #[cfg(test)]
            self.fail_if_injected(StoreFailurePoint::WriteManifest)?;
            write_manifest(&session_dir, &manifest)?;
            #[cfg(test)]
            self.fail_if_injected(StoreFailurePoint::CreateEventLog)?;
            create_empty_event_log(&session_dir)
        })();
        if let Err(create_error) = initialization {
            return Err(match self.remove_created_session_dir(&session_dir) {
                Ok(()) => SessionCreateError::Create(create_error),
                Err(cleanup_error) => SessionCreateError::CleanupFailed {
                    session_id,
                    session_dir,
                    create_error,
                    cleanup_error,
                },
            });
        }

        Ok(SessionHandle {
            session_dir,
            manifest,
        })
    }

    pub(crate) fn open_session(&self, path: &Path) -> Result<SessionHandle, CodingSessionError> {
        let session_dir = self.resolve_existing_session_dir(path)?;
        let manifest = read_manifest(&session_dir)?;

        validate_manifest(&manifest)?;
        let event_log_path = event_log_path(&session_dir, &manifest)?;
        if !event_log_path.is_file() {
            return Err(session_error(format!(
                "session event log is missing: {}",
                event_log_path.display()
            )));
        }

        Ok(SessionHandle {
            session_dir,
            manifest,
        })
    }

    pub(crate) fn open_session_id(
        &self,
        session_id: &str,
    ) -> Result<SessionHandle, CodingSessionError> {
        let session_id = normalize_session_id(session_id)?;
        self.open_session(Path::new(&session_id))
    }

    pub(crate) fn try_open_session_id(
        &self,
        session_id: &str,
    ) -> Result<Option<SessionHandle>, CodingSessionError> {
        let session_id = normalize_session_id(session_id)?;
        let candidate = self.root.join(&session_id);
        if !candidate.exists() {
            return Ok(None);
        }
        self.open_session(Path::new(&session_id)).map(Some)
    }

    pub(crate) fn list_sessions(&self) -> Result<Vec<SessionSummary>, CodingSessionError> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(session_error(format!(
                    "failed to read session log root {}: {error}",
                    self.root.display()
                )));
            }
        };

        let mut sessions = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| {
                session_error(format!(
                    "failed to read session log root entry {}: {error}",
                    self.root.display()
                ))
            })?;
            let file_type = entry.file_type().map_err(|error| {
                session_error(format!(
                    "failed to inspect session log root entry {}: {error}",
                    entry.path().display()
                ))
            })?;
            if !file_type.is_dir() {
                continue;
            }

            let session_dir = entry.path();
            if !session_dir.join(SESSION_MANIFEST_FILE).is_file() {
                continue;
            }

            let handle = self.open_session(&session_dir)?;
            sessions.push(SessionSummary::from_handle(&handle));
        }

        sessions.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.session_id.cmp(&right.session_id))
        });
        Ok(sessions)
    }

    pub(crate) fn append_events(
        &self,
        handle: &SessionHandle,
        events: &[SessionEventEnvelope],
    ) -> Result<(), CodingSessionError> {
        #[cfg(test)]
        self.fail_if_injected(StoreFailurePoint::AppendEvents)?;
        let event_log_path = event_log_path(&handle.session_dir, &handle.manifest)?;
        let next_sequence = next_session_sequence(&event_log_path, &handle.manifest.session_id)?;
        let file = OpenOptions::new()
            .append(true)
            .open(&event_log_path)
            .map_err(|error| {
                session_error(format!(
                    "failed to open session event log {}: {error}",
                    event_log_path.display()
                ))
            })?;
        let mut writer = BufWriter::new(file);

        for (next_sequence, event) in (next_sequence..).zip(events) {
            let event = event.clone().with_session_sequence(next_sequence);
            validate_event_for_session(&event, &handle.manifest.session_id)?;
            serde_json::to_writer(&mut writer, &event).map_err(|error| {
                session_error(format!("failed to serialize session event: {error}"))
            })?;
            writer.write_all(b"\n").map_err(|error| {
                session_error(format!(
                    "failed to append session event to {}: {error}",
                    event_log_path.display()
                ))
            })?;
        }

        writer.flush().map_err(|error| {
            session_error(format!(
                "failed to flush session event log {}: {error}",
                event_log_path.display()
            ))
        })
    }

    pub(crate) fn read_events(
        &self,
        handle: &SessionHandle,
    ) -> Result<Vec<SessionEventEnvelope>, CodingSessionError> {
        let event_log_path = event_log_path(&handle.session_dir, &handle.manifest)?;
        let content = fs::read_to_string(&event_log_path).map_err(|error| {
            session_error(format!(
                "failed to read session event log {}: {error}",
                event_log_path.display()
            ))
        })?;

        let mut events = Vec::new();
        let mut compatibility_sequence = 0_u64;
        for (index, line) in content.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            compatibility_sequence += 1;
            let mut event = decode_event_line(line, index + 1, &event_log_path)?;
            if event.session_sequence.is_none() {
                event.session_sequence = Some(compatibility_sequence);
            }
            validate_contiguous_session_sequence(&event, compatibility_sequence)?;
            validate_event_for_session(&event, &handle.manifest.session_id)?;
            events.push(event);
        }

        Ok(events)
    }

    pub(crate) fn replay_session(
        &self,
        handle: &SessionHandle,
    ) -> Result<SessionReplay, CodingSessionError> {
        let events = self.read_events(handle)?;
        Ok(fold_events(&events))
    }

    pub(crate) fn update_manifest(
        &self,
        handle: &SessionHandle,
        patch: ManifestPatch,
    ) -> Result<(), CodingSessionError> {
        #[cfg(test)]
        self.fail_if_injected(StoreFailurePoint::UpdateManifest)?;
        let mut manifest = read_manifest(&handle.session_dir)?;
        patch.apply(&mut manifest);
        validate_manifest(&manifest)?;
        write_manifest(&handle.session_dir, &manifest)
    }

    pub(crate) fn remove_session(&self, handle: &SessionHandle) -> Result<(), CodingSessionError> {
        let session_dir = self.resolve_existing_session_dir(handle.session_dir())?;
        self.remove_created_session_dir(&session_dir)
    }

    fn remove_created_session_dir(&self, session_dir: &Path) -> Result<(), CodingSessionError> {
        #[cfg(test)]
        self.fail_if_injected(StoreFailurePoint::RemoveSession)?;
        fs::remove_dir_all(session_dir).map_err(|error| {
            session_error(format!(
                "failed to remove session directory {}: {error}",
                session_dir.display()
            ))
        })
    }

    fn resolve_existing_session_dir(&self, path: &Path) -> Result<PathBuf, CodingSessionError> {
        let root = self.root.canonicalize().map_err(|error| {
            session_error(format!(
                "failed to resolve session log root {}: {error}",
                self.root.display()
            ))
        })?;
        let candidate = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };
        let session_dir = candidate.canonicalize().map_err(|error| {
            session_error(format!(
                "failed to resolve session directory {}: {error}",
                candidate.display()
            ))
        })?;
        if !session_dir.starts_with(&root) {
            return Err(session_error(format!(
                "session directory is outside store root: {}",
                session_dir.display()
            )));
        }
        if !session_dir.is_dir() {
            return Err(session_error(format!(
                "session path is not a directory: {}",
                session_dir.display()
            )));
        }
        Ok(session_dir)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreateSessionOptions {
    pub(crate) session_id: String,
    pub(crate) created_at: String,
    pub(crate) default_agent_profile_id: ProfileId,
}

impl CreateSessionOptions {
    pub(crate) fn new(session_id: impl Into<String>, created_at: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            created_at: created_at.into(),
            default_agent_profile_id: default_agent_profile_id(),
        }
    }

    pub(crate) fn default_agent_profile_id(mut self, profile_id: ProfileId) -> Self {
        self.default_agent_profile_id = profile_id;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionHandle {
    session_dir: PathBuf,
    manifest: SessionManifest,
}

impl SessionHandle {
    pub(crate) fn session_dir(&self) -> &Path {
        &self.session_dir
    }

    pub(crate) fn manifest(&self) -> &SessionManifest {
        &self.manifest
    }

    #[cfg(test)]
    pub(crate) fn event_log_path(&self) -> Result<PathBuf, CodingSessionError> {
        event_log_path(&self.session_dir, &self.manifest)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionSummary {
    pub(crate) session_id: String,
    pub(crate) session_dir: PathBuf,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    pub(crate) active_leaf_id: Option<String>,
}

impl SessionSummary {
    fn from_handle(handle: &SessionHandle) -> Self {
        Self {
            session_id: handle.manifest.session_id.clone(),
            session_dir: handle.session_dir.clone(),
            created_at: handle.manifest.created_at.clone(),
            updated_at: handle.manifest.updated_at.clone(),
            active_leaf_id: handle.manifest.active_leaf_id.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ManifestPatch {
    updated_at: Option<String>,
    active_branch_id: Option<Option<String>>,
    active_leaf_id: Option<Option<String>>,
    default_agent_profile_id: Option<ProfileId>,
}

impl ManifestPatch {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn updated_at(mut self, updated_at: impl Into<String>) -> Self {
        self.updated_at = Some(updated_at.into());
        self
    }

    #[cfg(test)]
    pub(crate) fn active_branch_id(mut self, active_branch_id: Option<String>) -> Self {
        self.active_branch_id = Some(active_branch_id);
        self
    }

    pub(crate) fn active_leaf_id(mut self, active_leaf_id: Option<String>) -> Self {
        self.active_leaf_id = Some(active_leaf_id);
        self
    }

    pub(crate) fn default_agent_profile_id(mut self, profile_id: ProfileId) -> Self {
        self.default_agent_profile_id = Some(profile_id);
        self
    }

    fn apply(self, manifest: &mut SessionManifest) {
        if let Some(updated_at) = self.updated_at {
            manifest.updated_at = updated_at;
        }
        if let Some(active_branch_id) = self.active_branch_id {
            manifest.active_branch_id = active_branch_id;
        }
        if let Some(active_leaf_id) = self.active_leaf_id {
            manifest.active_leaf_id = active_leaf_id;
        }
        if let Some(default_agent_profile_id) = self.default_agent_profile_id {
            manifest.default_agent_profile_id = default_agent_profile_id;
        }
    }
}

fn normalize_session_id(value: &str) -> Result<String, CodingSessionError> {
    let session_id = value.trim();
    if session_id.is_empty() {
        return Err(session_error("session id must not be empty"));
    }
    if !session_id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(session_error(format!(
            "session id contains unsupported path characters: {session_id}"
        )));
    }
    Ok(session_id.to_owned())
}

fn write_manifest(
    session_dir: &Path,
    manifest: &SessionManifest,
) -> Result<(), CodingSessionError> {
    let manifest_path = session_dir.join(SESSION_MANIFEST_FILE);
    let mut bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|error| session_error(format!("failed to serialize session manifest: {error}")))?;
    bytes.push(b'\n');
    fs::write(&manifest_path, bytes).map_err(|error| {
        session_error(format!(
            "failed to write session manifest {}: {error}",
            manifest_path.display()
        ))
    })
}

fn read_manifest(session_dir: &Path) -> Result<SessionManifest, CodingSessionError> {
    let manifest_path = session_dir.join(SESSION_MANIFEST_FILE);
    let content = fs::read_to_string(&manifest_path).map_err(|error| {
        session_error(format!(
            "failed to read session manifest {}: {error}",
            manifest_path.display()
        ))
    })?;
    decode_manifest(&content, &manifest_path)
}

fn decode_manifest(
    content: &str,
    manifest_path: &Path,
) -> Result<SessionManifest, CodingSessionError> {
    let value: Value = serde_json::from_str(content).map_err(|error| {
        session_error(format!(
            "failed to parse session manifest {}: {error}",
            manifest_path.display()
        ))
    })?;
    let schema = json_string_field(&value, "schema");
    let version = json_u32_field(&value, "version");
    match (schema.as_deref(), version) {
        (Some(SESSION_SCHEMA), Some(SESSION_VERSION)) => {
            serde_json::from_value(value).map_err(|error| {
                session_error(format!(
                    "failed to decode v{SESSION_VERSION} session manifest {}: {error}",
                    manifest_path.display()
                ))
            })
        }
        _ => Err(session_error(format!(
            "unsupported session manifest decoder: schema={}, version={}; recovery: open with a compatible pi-rust release or migrate the manifest",
            schema.as_deref().unwrap_or("<missing>"),
            version.map_or_else(|| "<missing>".to_owned(), |value| value.to_string()),
        ))),
    }
}

fn decode_event_line(
    line: &str,
    line_number: usize,
    event_log_path: &Path,
) -> Result<SessionEventEnvelope, CodingSessionError> {
    let value: Value = serde_json::from_str(line).map_err(|error| {
        session_error(format!(
            "failed to parse session event at line {line_number} in {}: {error}",
            event_log_path.display()
        ))
    })?;
    let schema = json_string_field(&value, "schema");
    let version = json_u32_field(&value, "version");
    let event_id = json_string_field(&value, "event_id");
    match (schema.as_deref(), version) {
        (Some(EVENT_SCHEMA), Some(EVENT_VERSION)) => serde_json::from_value(value).map_err(|error| {
            session_error(format!(
                "failed to decode v{EVENT_VERSION} session event at line {line_number} in {}: {error}",
                event_log_path.display()
            ))
        }),
        _ => Err(session_error(format!(
            "unsupported session event decoder: schema={}, version={}, event_id={}; recovery: open with a compatible pi-rust release or migrate the session event log",
            schema.as_deref().unwrap_or("<missing>"),
            version.map_or_else(|| "<missing>".to_owned(), |value| value.to_string()),
            event_id.as_deref().unwrap_or("<missing>"),
        ))),
    }
}

fn json_string_field(value: &Value, field: &str) -> Option<String> {
    value.get(field)?.as_str().map(str::to_owned)
}

fn json_u32_field(value: &Value, field: &str) -> Option<u32> {
    value.get(field)?.as_u64()?.try_into().ok()
}

fn create_empty_event_log(session_dir: &Path) -> Result<(), CodingSessionError> {
    let event_log_path = session_dir.join(SESSION_EVENT_LOG_FILE);
    File::create_new(&event_log_path)
        .map(|_| ())
        .map_err(|error| {
            session_error(format!(
                "failed to create session event log {}: {error}",
                event_log_path.display()
            ))
        })
}

fn validate_manifest(manifest: &SessionManifest) -> Result<(), CodingSessionError> {
    if manifest.schema != SESSION_SCHEMA {
        return Err(session_error(format!(
            "unsupported session manifest schema: {}",
            manifest.schema
        )));
    }
    if manifest.version != SESSION_VERSION {
        return Err(session_error(format!(
            "unsupported session manifest version: {}",
            manifest.version
        )));
    }
    validate_relative_manifest_path(&manifest.event_log)?;
    Ok(())
}

fn validate_relative_manifest_path(path: &str) -> Result<(), CodingSessionError> {
    let path = Path::new(path);
    if path.as_os_str().is_empty() {
        return Err(session_error("manifest event log path must not be empty"));
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => {
                return Err(session_error(format!(
                    "manifest event log path must be relative and contained: {}",
                    path.display()
                )));
            }
        }
    }
    Ok(())
}

fn event_log_path(
    session_dir: &Path,
    manifest: &SessionManifest,
) -> Result<PathBuf, CodingSessionError> {
    validate_relative_manifest_path(&manifest.event_log)?;
    Ok(session_dir.join(&manifest.event_log))
}

fn next_session_sequence(
    event_log_path: &Path,
    session_id: &str,
) -> Result<u64, CodingSessionError> {
    let content = fs::read_to_string(event_log_path).map_err(|error| {
        session_error(format!(
            "failed to read session event log {}: {error}",
            event_log_path.display()
        ))
    })?;

    let mut compatibility_sequence = 0_u64;
    for (index, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        compatibility_sequence += 1;
        let mut event = decode_event_line(line, index + 1, event_log_path)?;
        if event.session_sequence.is_none() {
            event.session_sequence = Some(compatibility_sequence);
        }
        validate_contiguous_session_sequence(&event, compatibility_sequence)?;
        validate_event_for_session(&event, session_id)?;
    }

    Ok(compatibility_sequence + 1)
}

fn validate_contiguous_session_sequence(
    event: &SessionEventEnvelope,
    expected_sequence: u64,
) -> Result<(), CodingSessionError> {
    let actual_sequence = event
        .session_sequence
        .expect("sequence is normalized before validation");
    if actual_sequence != expected_sequence {
        return Err(session_error(format!(
            "session event sequence is not contiguous: event_id={}, expected={}, actual={}",
            event.event_id, expected_sequence, actual_sequence
        )));
    }
    Ok(())
}

fn validate_event_for_session(
    event: &SessionEventEnvelope,
    session_id: &str,
) -> Result<(), CodingSessionError> {
    if event.schema != EVENT_SCHEMA {
        return Err(session_error(format!(
            "unsupported session event schema: {}",
            event.schema
        )));
    }
    if event.version != EVENT_VERSION {
        return Err(session_error(format!(
            "unsupported session event version: {}",
            event.version
        )));
    }
    if event.session_id != session_id {
        return Err(session_error(format!(
            "session event {} belongs to {}, expected {}",
            event.event_id, event.session_id, session_id
        )));
    }
    Ok(())
}

fn session_error(message: impl Into<String>) -> CodingSessionError {
    CodingSessionError::Session {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::event::{
        OperationKind, PersistedContentBlock, PersistedRole, PersistedToolResult, SessionEventData,
    };
    use crate::session::replay::{MessageStatus, ToolCallStatus, TranscriptItem};

    fn create_options(session_id: &str) -> CreateSessionOptions {
        CreateSessionOptions::new(session_id, "2026-06-29T00:00:00Z")
    }

    fn event(session_id: &str, event_id: &str, data: SessionEventData) -> SessionEventEnvelope {
        SessionEventEnvelope::new(session_id, event_id, "2026-06-29T00:00:01Z", data)
    }

    #[test]
    fn create_session_writes_manifest_event_log_and_directories() {
        let temp = tempfile::tempdir().unwrap();
        let store = SessionLogStore::new(temp.path());

        let handle = store.create_session(create_options("sess_store")).unwrap();

        assert!(handle.session_dir().is_dir());
        assert!(handle.session_dir().join("blobs").is_dir());
        assert!(handle.session_dir().join("index").is_dir());
        assert!(handle.session_dir().join(SESSION_MANIFEST_FILE).is_file());
        assert!(handle.event_log_path().unwrap().is_file());
        assert_eq!(handle.manifest().session_id, "sess_store");
        assert_eq!(handle.manifest().created_at, "2026-06-29T00:00:00Z");
        assert_eq!(handle.manifest().event_log, SESSION_EVENT_LOG_FILE);

        let event_log = fs::read_to_string(handle.event_log_path().unwrap()).unwrap();
        assert!(event_log.is_empty());
    }

    #[test]
    fn create_session_cleans_up_every_failed_initialization_stage() {
        for (stage, session_id) in [
            (StoreFailurePoint::CreateBlobs, "sess_fail_blobs"),
            (StoreFailurePoint::CreateIndex, "sess_fail_index"),
            (StoreFailurePoint::WriteManifest, "sess_fail_manifest"),
            (StoreFailurePoint::CreateEventLog, "sess_fail_event_log"),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let store = SessionLogStore::new(temp.path());
            store.fail_after(stage, 0);

            let error = store
                .create_session(create_options(session_id))
                .unwrap_err();

            assert_eq!(error.code(), "session");
            assert!(
                !temp.path().join(session_id).exists(),
                "failed stage {stage:?} should not leave a visible target"
            );
        }
    }

    #[test]
    fn open_session_reads_manifest_and_rejects_paths_outside_root() {
        let temp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let store = SessionLogStore::new(temp.path());
        let handle = store.create_session(create_options("sess_open")).unwrap();

        let opened = store.open_session(handle.session_dir()).unwrap();
        assert_eq!(opened.manifest(), handle.manifest());

        let error = store.open_session(outside.path()).unwrap_err();
        assert_eq!(error.code(), "session");
        assert!(
            error
                .to_string()
                .contains("session directory is outside store root")
        );
    }

    #[test]
    fn try_open_session_id_returns_none_for_missing_and_opens_existing() {
        let temp = tempfile::tempdir().unwrap();
        let store = SessionLogStore::new(temp.path());

        assert_eq!(store.try_open_session_id("sess_missing").unwrap(), None);

        let created = store
            .create_session(create_options("sess_try_open"))
            .unwrap();
        let opened = store
            .try_open_session_id(" sess_try_open ")
            .unwrap()
            .unwrap();

        assert_eq!(opened.manifest(), created.manifest());
        assert_eq!(opened.session_dir(), created.session_dir());
    }

    #[test]
    fn list_sessions_returns_native_sessions_sorted_by_updated_at() {
        let temp = tempfile::tempdir().unwrap();
        let store = SessionLogStore::new(temp.path());
        let old = store
            .create_session(CreateSessionOptions::new(
                "sess_old",
                "2026-06-29T00:00:00Z",
            ))
            .unwrap();
        let new = store
            .create_session(CreateSessionOptions::new(
                "sess_new",
                "2026-06-29T00:00:01Z",
            ))
            .unwrap();
        fs::create_dir(temp.path().join("legacy-jsonl-directory")).unwrap();
        fs::write(temp.path().join("not-a-session"), "{}\n").unwrap();

        store
            .update_manifest(
                &old,
                ManifestPatch::new()
                    .updated_at("2026-06-29T00:00:03Z")
                    .active_leaf_id(Some("leaf_old".into())),
            )
            .unwrap();

        let summaries = store.list_sessions().unwrap();

        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].session_id, "sess_old");
        assert_eq!(summaries[0].session_dir, old.session_dir().to_path_buf());
        assert_eq!(summaries[0].created_at, "2026-06-29T00:00:00Z");
        assert_eq!(summaries[0].updated_at, "2026-06-29T00:00:03Z");
        assert_eq!(summaries[0].active_leaf_id.as_deref(), Some("leaf_old"));
        assert_eq!(summaries[1].session_id, "sess_new");
        assert_eq!(summaries[1].session_dir, new.session_dir().to_path_buf());
    }

    #[test]
    fn list_sessions_returns_empty_for_missing_root() {
        let temp = tempfile::tempdir().unwrap();
        let store = SessionLogStore::new(temp.path().join("missing"));

        assert!(store.list_sessions().unwrap().is_empty());
    }

    #[test]
    fn append_and_read_events_round_trip_jsonl() {
        let temp = tempfile::tempdir().unwrap();
        let store = SessionLogStore::new(temp.path());
        let handle = store.create_session(create_options("sess_events")).unwrap();
        let events = vec![
            event(
                "sess_events",
                "evt_1",
                SessionEventData::SessionCreated {
                    cwd: Some("/tmp/project".into()),
                },
            ),
            event("sess_events", "evt_2", SessionEventData::TurnStarted {})
                .with_operation_id("op_1")
                .with_turn_id("turn_1"),
        ];

        store.append_events(&handle, &events).unwrap();

        let raw = fs::read_to_string(handle.event_log_path().unwrap()).unwrap();
        assert_eq!(raw.lines().count(), 2);
        assert!(raw.contains("\"kind\":\"session.created\""));
        assert!(raw.contains("\"kind\":\"turn.started\""));

        let decoded = store.read_events(&handle).unwrap();
        assert_eq!(
            decoded,
            vec![
                events[0].clone().with_session_sequence(1),
                events[1].clone().with_session_sequence(2),
            ]
        );
    }

    #[test]
    fn append_events_assigns_contiguous_session_sequences() {
        let temp = tempfile::tempdir().unwrap();
        let store = SessionLogStore::new(temp.path());
        let handle = store
            .create_session(create_options("sess_sequence"))
            .unwrap();
        let events = vec![
            event(
                "sess_sequence",
                "evt_1",
                SessionEventData::SessionCreated { cwd: None },
            ),
            event("sess_sequence", "evt_2", SessionEventData::TurnStarted {}),
        ];

        store.append_events(&handle, &events).unwrap();

        let decoded = store.read_events(&handle).unwrap();
        assert_eq!(
            decoded
                .iter()
                .map(|event| event.session_sequence)
                .collect::<Vec<_>>(),
            vec![Some(1), Some(2)]
        );

        let raw = fs::read_to_string(handle.event_log_path().unwrap()).unwrap();
        let raw_sequences = raw
            .lines()
            .map(|line| {
                serde_json::from_str::<serde_json::Value>(line).unwrap()["session_sequence"]
                    .as_u64()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(raw_sequences, vec![1, 2]);
    }

    #[test]
    fn read_events_synthesizes_sequences_for_legacy_logs() {
        let temp = tempfile::tempdir().unwrap();
        let store = SessionLogStore::new(temp.path());
        let handle = store
            .create_session(create_options("sess_legacy_sequence"))
            .unwrap();
        let legacy_events = [
            event(
                "sess_legacy_sequence",
                "evt_legacy_1",
                SessionEventData::SessionCreated { cwd: None },
            ),
            event(
                "sess_legacy_sequence",
                "evt_legacy_2",
                SessionEventData::TurnStarted {},
            ),
        ];
        let raw = legacy_events
            .iter()
            .map(|event| serde_json::to_string(event).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(handle.event_log_path().unwrap(), format!("{raw}\n")).unwrap();

        let decoded = store.read_events(&handle).unwrap();

        assert_eq!(
            decoded
                .iter()
                .map(|event| event.session_sequence)
                .collect::<Vec<_>>(),
            vec![Some(1), Some(2)]
        );
    }

    #[test]
    fn read_events_rejects_non_contiguous_durable_sequences() {
        let temp = tempfile::tempdir().unwrap();
        let store = SessionLogStore::new(temp.path());
        let handle = store
            .create_session(create_options("sess_non_contiguous_sequence"))
            .unwrap();
        let events = [
            event(
                "sess_non_contiguous_sequence",
                "evt_1",
                SessionEventData::SessionCreated { cwd: None },
            )
            .with_session_sequence(1),
            event(
                "sess_non_contiguous_sequence",
                "evt_3",
                SessionEventData::TurnStarted {},
            )
            .with_session_sequence(3),
        ];
        let raw = events
            .iter()
            .map(|event| serde_json::to_string(event).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(handle.event_log_path().unwrap(), format!("{raw}\n")).unwrap();

        let error = store.read_events(&handle).unwrap_err();
        assert!(error.to_string().contains("event_id=evt_3"));
        assert!(error.to_string().contains("expected=2"));
        assert!(error.to_string().contains("actual=3"));
    }

    #[test]
    fn decoder_matrix_rejects_unknown_manifest_and_event_versions_with_recovery_context() {
        let manifest_error = decode_manifest(
            r#"{"schema":"pi-rust.session","version":99}"#,
            Path::new("/tmp/session.json"),
        )
        .unwrap_err();
        assert!(
            manifest_error
                .to_string()
                .contains("schema=pi-rust.session")
        );
        assert!(manifest_error.to_string().contains("version=99"));
        assert!(manifest_error.to_string().contains("recovery:"));

        let event_error = decode_event_line(
            r#"{"schema":"pi-rust.session.event","version":99,"event_id":"evt-future"}"#,
            7,
            Path::new("/tmp/events.jsonl"),
        )
        .unwrap_err();
        assert!(
            event_error
                .to_string()
                .contains("schema=pi-rust.session.event")
        );
        assert!(event_error.to_string().contains("version=99"));
        assert!(event_error.to_string().contains("event_id=evt-future"));
        assert!(event_error.to_string().contains("recovery:"));
    }

    #[test]
    fn replay_session_folds_canonical_event_log_into_transcript() {
        let temp = tempfile::tempdir().unwrap();
        let store = SessionLogStore::new(temp.path());
        let handle = store
            .create_session(create_options("sess_replay_store"))
            .unwrap();
        let events = vec![
            event(
                "sess_replay_store",
                "evt_1",
                SessionEventData::OperationStarted {
                    operation: OperationKind::Prompt,
                    runtime_generation: Default::default(),
                },
            )
            .with_operation_id("op_1")
            .with_turn_id("turn_1"),
            event(
                "sess_replay_store",
                "evt_2",
                SessionEventData::TurnInputRecorded {
                    content: vec![PersistedContentBlock::Text {
                        text: "hello".into(),
                    }],
                },
            )
            .with_operation_id("op_1")
            .with_turn_id("turn_1"),
            event(
                "sess_replay_store",
                "evt_3",
                SessionEventData::MessageStarted {
                    message_id: "msg_1".into(),
                    role: PersistedRole::Assistant,
                },
            )
            .with_operation_id("op_1")
            .with_turn_id("turn_1"),
            event(
                "sess_replay_store",
                "evt_4",
                SessionEventData::MessageCompleted {
                    message_id: "msg_1".into(),
                    content: vec![PersistedContentBlock::Text { text: "hi".into() }],
                    finish_reason: Some("stop".into()),
                    usage: Default::default(),
                },
            )
            .with_operation_id("op_1")
            .with_turn_id("turn_1"),
            event(
                "sess_replay_store",
                "evt_6",
                SessionEventData::ToolCallStarted {
                    tool_call_id: "tool_1".into(),
                    name: "read".into(),
                    arguments: serde_json::json!({"path": "src/lib.rs"}),
                },
            )
            .with_operation_id("op_1")
            .with_turn_id("turn_1"),
            event(
                "sess_replay_store",
                "evt_7",
                SessionEventData::ToolCallCompleted {
                    tool_call_id: "tool_1".into(),
                    result: PersistedToolResult::Text { text: "ok".into() },
                },
            )
            .with_operation_id("op_1")
            .with_turn_id("turn_1"),
            event(
                "sess_replay_store",
                "evt_8",
                SessionEventData::OperationCommitted {
                    new_leaf_id: Some("leaf_1".into()),
                },
            )
            .with_operation_id("op_1")
            .with_turn_id("turn_1"),
        ];

        store.append_events(&handle, &events).unwrap();

        let replay = store.replay_session(&handle).unwrap();

        assert_eq!(replay.session_id, "sess_replay_store");
        assert_eq!(replay.active_leaf_id.as_deref(), Some("leaf_1"));
        assert_eq!(
            replay.transcript,
            vec![
                TranscriptItem::UserInput {
                    turn_id: "turn_1".into(),
                    text: "hello".into(),
                },
                TranscriptItem::AssistantMessage {
                    message_id: "msg_1".into(),
                    content: vec![PersistedContentBlock::Text { text: "hi".into() }],
                    status: MessageStatus::Completed,
                },
                TranscriptItem::ToolCall {
                    tool_call_id: "tool_1".into(),
                    name: "read".into(),
                    arguments: serde_json::json!({"path": "src/lib.rs"}),
                    status: ToolCallStatus::Completed,
                    summary: "ok".into(),
                },
            ]
        );
    }

    #[test]
    fn append_rejects_events_for_another_session() {
        let temp = tempfile::tempdir().unwrap();
        let store = SessionLogStore::new(temp.path());
        let handle = store
            .create_session(create_options("sess_expected"))
            .unwrap();
        let wrong_event = event(
            "sess_other",
            "evt_1",
            SessionEventData::SessionCreated { cwd: None },
        );

        let error = store.append_events(&handle, &[wrong_event]).unwrap_err();

        assert_eq!(error.code(), "session");
        assert!(
            error
                .to_string()
                .contains("belongs to sess_other, expected sess_expected")
        );
    }

    #[test]
    fn update_manifest_persists_patch() {
        let temp = tempfile::tempdir().unwrap();
        let store = SessionLogStore::new(temp.path());
        let handle = store
            .create_session(create_options("sess_manifest"))
            .unwrap();

        store
            .update_manifest(
                &handle,
                ManifestPatch::new()
                    .updated_at("2026-06-29T00:00:02Z")
                    .active_branch_id(Some("branch_1".into()))
                    .active_leaf_id(Some("leaf_1".into())),
            )
            .unwrap();

        let opened = store.open_session(handle.session_dir()).unwrap();
        assert_eq!(opened.manifest().updated_at, "2026-06-29T00:00:02Z");
        assert_eq!(
            opened.manifest().active_branch_id.as_deref(),
            Some("branch_1")
        );
        assert_eq!(opened.manifest().active_leaf_id.as_deref(), Some("leaf_1"));
    }

    #[test]
    fn create_session_rejects_path_like_session_id() {
        let temp = tempfile::tempdir().unwrap();
        let store = SessionLogStore::new(temp.path());

        let error = store
            .create_session(create_options("../escape"))
            .unwrap_err();

        assert_eq!(error.code(), "session");
        assert!(
            error
                .to_string()
                .contains("session id contains unsupported path characters")
        );
    }

    #[test]
    fn open_session_rejects_manifest_event_log_escape() {
        let temp = tempfile::tempdir().unwrap();
        let store = SessionLogStore::new(temp.path());
        let handle = store
            .create_session(create_options("sess_bad_manifest"))
            .unwrap();
        let mut manifest = handle.manifest().clone();
        manifest.event_log = "../events.jsonl".into();
        write_manifest(handle.session_dir(), &manifest).unwrap();

        let error = store.open_session(handle.session_dir()).unwrap_err();

        assert_eq!(error.code(), "session");
        assert!(
            error
                .to_string()
                .contains("manifest event log path must be relative and contained")
        );
    }
}
