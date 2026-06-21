use super::migrations::migrate_session_values;
use crate::session::{
    JsonlSessionMetadata, SessionEntry, SessionError, SessionErrorCode, SessionHeader,
};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct JsonlSessionStorage {
    path: PathBuf,
    header: SessionHeader,
    entries: Vec<SessionEntry>,
    by_id: HashMap<String, SessionEntry>,
    leaf_id: Option<String>,
}

impl JsonlSessionStorage {
    pub fn create(
        path: impl AsRef<Path>,
        cwd: impl Into<String>,
        session_id: impl Into<String>,
        timestamp: impl Into<String>,
        parent_session_path: Option<PathBuf>,
    ) -> Result<Self, SessionError> {
        let path = path.as_ref().to_path_buf();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                SessionError::new(
                    SessionErrorCode::Storage,
                    format!("failed to create directory for session file: {}", e),
                )
            })?;
        }

        let header = SessionHeader {
            entry_type: "session".into(),
            version: 3,
            id: session_id.into(),
            timestamp: timestamp.into(),
            cwd: cwd.into(),
            parent_session: parent_session_path.map(|p| p.display().to_string()),
        };

        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| {
                SessionError::new(
                    SessionErrorCode::Storage,
                    format!("failed to create session file {}: {}", path.display(), e),
                )
            })?;

        let header_line = serde_json::to_string(&header).map_err(|e| {
            SessionError::new(
                SessionErrorCode::InvalidSession,
                format!("failed to serialize session header: {}", e),
            )
        })?;

        writeln!(file, "{}", header_line).map_err(|e| {
            SessionError::new(
                SessionErrorCode::Storage,
                format!("failed to write session header: {}", e),
            )
        })?;

        Ok(Self {
            path,
            header,
            entries: Vec::new(),
            by_id: HashMap::new(),
            leaf_id: None,
        })
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, SessionError> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path).map_err(|e| {
            SessionError::new(
                SessionErrorCode::NotFound,
                format!("session file not found {}: {}", path.display(), e),
            )
        })?;

        let reader = BufReader::new(file);
        let mut lines: Vec<String> = Vec::new();
        for (line_num, line_result) in reader.lines().enumerate() {
            let line = line_result.map_err(|e| {
                SessionError::new(
                    SessionErrorCode::Storage,
                    format!(
                        "failed to read line {} in {}: {}",
                        line_num + 1,
                        path.display(),
                        e
                    ),
                )
            })?;
            let trimmed = line.trim().to_string();
            if trimmed.is_empty() {
                continue;
            }
            lines.push(trimmed);
        }

        if lines.is_empty() {
            return Err(SessionError::new(
                SessionErrorCode::InvalidSession,
                "empty session file",
            ));
        }

        let mut values: Vec<serde_json::Value> = Vec::with_capacity(lines.len());
        for (line_num, line) in lines.iter().enumerate() {
            let value = serde_json::from_str(line).map_err(|e| {
                let message = if line_num == 0 {
                    format!("first line is not a valid session header: {}", e)
                } else {
                    format!("failed to parse entry at line {}: {}", line_num + 1, e)
                };
                SessionError::new(
                    if line_num == 0 {
                        SessionErrorCode::InvalidSession
                    } else {
                        SessionErrorCode::InvalidEntry
                    },
                    message,
                )
            })?;
            values.push(value);
        }

        let migrated = migrate_session_values(&mut values)?;
        let header: SessionHeader = serde_json::from_value(values[0].clone()).map_err(|e| {
            SessionError::new(
                SessionErrorCode::InvalidSession,
                format!("first line is not a valid session header: {}", e),
            )
        })?;

        if header.entry_type != "session" {
            return Err(SessionError::new(
                SessionErrorCode::InvalidSession,
                "first line is not a valid session header: missing type=session",
            ));
        }
        if header.version != 3 {
            return Err(SessionError::new(
                SessionErrorCode::InvalidSession,
                format!("unsupported session version: {}", header.version),
            ));
        }
        if header.id.is_empty() {
            return Err(SessionError::new(
                SessionErrorCode::InvalidSession,
                "session header missing id",
            ));
        }
        if header.timestamp.is_empty() {
            return Err(SessionError::new(
                SessionErrorCode::InvalidSession,
                "session header missing timestamp",
            ));
        }
        if header.cwd.is_empty() {
            return Err(SessionError::new(
                SessionErrorCode::InvalidSession,
                "session header missing cwd",
            ));
        }

        let mut entries = Vec::new();
        let mut by_id = HashMap::new();
        let mut leaf_id: Option<String> = None;

        for (line_num, value) in values.iter().enumerate().skip(1) {
            let entry: SessionEntry = serde_json::from_value(value.clone()).map_err(|e| {
                SessionError::new(
                    SessionErrorCode::InvalidEntry,
                    format!("failed to parse entry at line {}: {}", line_num + 1, e),
                )
            })?;

            if by_id.contains_key(&entry.id) {
                return Err(SessionError::new(
                    SessionErrorCode::InvalidEntry,
                    format!("duplicate entry id at line {}: {}", line_num + 1, entry.id),
                ));
            }

            leaf_id = if entry.entry_type == "leaf" {
                entry
                    .field("targetId")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
            } else {
                Some(entry.id.clone())
            };

            by_id.insert(entry.id.clone(), entry.clone());
            entries.push(entry);
        }

        if migrated {
            rewrite_session_file(&path, &values)?;
        }

        Ok(Self {
            path,
            header,
            entries,
            by_id,
            leaf_id,
        })
    }

    pub fn header(&self) -> &SessionHeader {
        &self.header
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn metadata(&self) -> JsonlSessionMetadata {
        JsonlSessionMetadata {
            id: self.header.id.clone(),
            created_at: self.header.timestamp.clone(),
            cwd: self.header.cwd.clone(),
            path: self.path.clone(),
            parent_session_path: self.header.parent_session.as_ref().map(PathBuf::from),
        }
    }

    pub fn get_entries(&self) -> Vec<SessionEntry> {
        self.entries.clone()
    }

    pub fn get_leaf_id(&self) -> Result<Option<String>, SessionError> {
        Ok(self.leaf_id.clone())
    }

    pub fn append_entry(&mut self, entry: SessionEntry) -> Result<(), SessionError> {
        if self.by_id.contains_key(&entry.id) {
            return Err(SessionError::new(
                SessionErrorCode::InvalidEntry,
                format!("duplicate entry id: {}", entry.id),
            ));
        }

        let line = serde_json::to_string(&entry).map_err(|e| {
            SessionError::new(
                SessionErrorCode::InvalidEntry,
                format!("failed to serialize entry: {}", e),
            )
        })?;

        let mut file = OpenOptions::new()
            .append(true)
            .open(&self.path)
            .map_err(|e| {
                SessionError::new(
                    SessionErrorCode::Storage,
                    format!(
                        "failed to open session file for append {}: {}",
                        self.path.display(),
                        e
                    ),
                )
            })?;

        writeln!(file, "{}", line).map_err(|e| {
            SessionError::new(
                SessionErrorCode::Storage,
                format!("failed to append entry to session file: {}", e),
            )
        })?;

        self.leaf_id = if entry.entry_type == "leaf" {
            entry
                .field("targetId")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        } else {
            Some(entry.id.clone())
        };

        self.by_id.insert(entry.id.clone(), entry.clone());
        self.entries.push(entry);
        Ok(())
    }
}

fn rewrite_session_file(path: &Path, values: &[serde_json::Value]) -> Result<(), SessionError> {
    let mut content = String::new();
    for value in values {
        let line = serde_json::to_string(value).map_err(|e| {
            SessionError::new(
                SessionErrorCode::InvalidSession,
                format!("failed to serialize migrated session: {}", e),
            )
        })?;
        content.push_str(&line);
        content.push('\n');
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("session.jsonl");
    let tmp_path = path.with_file_name(format!(".{}.tmp", file_name));

    fs::write(&tmp_path, content).map_err(|e| {
        SessionError::new(
            SessionErrorCode::Storage,
            format!(
                "failed to write migrated session file {}: {}",
                tmp_path.display(),
                e
            ),
        )
    })?;
    fs::rename(&tmp_path, path).map_err(|e| {
        SessionError::new(
            SessionErrorCode::Storage,
            format!(
                "failed to replace migrated session file {}: {}",
                path.display(),
                e
            ),
        )
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::StoredAgentMessage;
    use pi_ai::types::ContentBlock;

    fn user_entry(id: &str, parent: Option<&str>, text: &str) -> SessionEntry {
        SessionEntry::message(
            id.into(),
            parent.map(str::to_string),
            "2026-06-05T00:00:01.000Z".into(),
            StoredAgentMessage::User {
                content: vec![ContentBlock::Text {
                    text: text.into(),
                    text_signature: None,
                }],
                timestamp: 1,
            },
        )
    }

    #[test]
    fn creates_header_and_appends_entries() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("session.jsonl");
        let mut storage = JsonlSessionStorage::create(
            &file,
            "/tmp/project",
            "session-1",
            "2026-06-05T00:00:00.000Z",
            None,
        )
        .unwrap();
        storage
            .append_entry(user_entry("entry001", None, "hello"))
            .unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains(r#""type":"session""#));
        assert!(lines[1].contains(r#""role":"user""#));
    }

    #[test]
    fn opens_existing_file_and_tracks_latest_leaf() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("session.jsonl");
        std::fs::write(
            &file,
            concat!(
                r#"{"type":"session","version":3,"id":"session-1","timestamp":"2026-06-05T00:00:00.000Z","cwd":"/tmp/project"}"#,
                "\n",
                r#"{"type":"message","id":"entry001","parentId":null,"timestamp":"2026-06-05T00:00:01.000Z","message":{"role":"user","content":[{"type":"text","text":"hello"}],"timestamp":1}}"#,
                "\n"
            ),
        )
        .unwrap();
        let storage = JsonlSessionStorage::open(&file).unwrap();
        assert_eq!(storage.header().id, "session-1");
        assert_eq!(storage.get_leaf_id().unwrap().as_deref(), Some("entry001"));
        assert_eq!(storage.get_entries().len(), 1);
    }

    #[test]
    fn rejects_missing_header() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("bad.jsonl");
        std::fs::write(
            &file,
            r#"{"type":"message","id":"x","parentId":null,"timestamp":"now"}"#,
        )
        .unwrap();
        let error = JsonlSessionStorage::open(&file).unwrap_err();
        assert!(
            error
                .message
                .contains("first line is not a valid session header")
        );
    }

    #[test]
    fn opens_v2_sessions_and_renames_hook_message_role() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("v2.jsonl");
        std::fs::write(
            &file,
            concat!(
                r#"{"type":"session","version":2,"id":"s1","timestamp":"2026-06-05T00:00:00.000Z","cwd":"/tmp"}"#,
                "\n",
                r#"{"type":"message","id":"entry001","parentId":null,"timestamp":"2026-06-05T00:00:01.000Z","message":{"role":"hookMessage","customType":"listener","content":[{"type":"text","text":"hello"}],"display":true,"timestamp":1}}"#,
                "\n"
            ),
        )
        .unwrap();
        let storage = JsonlSessionStorage::open(&file).unwrap();
        assert_eq!(storage.header().version, 3);
        let entries = storage.get_entries();
        let message = entries[0].field("message").unwrap();
        assert_eq!(message["role"], "custom");
        let stored: StoredAgentMessage = serde_json::from_value(message.clone()).unwrap();
        match stored {
            StoredAgentMessage::Custom { custom_type, .. } => assert_eq!(custom_type, "listener"),
            other => panic!("expected migrated custom message, got {other:?}"),
        }
        let rewritten = std::fs::read_to_string(&file).unwrap();
        assert!(rewritten.contains(r#""version":3"#));
        assert!(rewritten.contains(r#""role":"custom""#));
        assert!(!rewritten.contains("hookMessage"));
    }

    #[test]
    fn opens_v1_sessions_and_adds_tree_ids() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("v1.jsonl");
        std::fs::write(
            &file,
            concat!(
                r#"{"type":"session","id":"s1","timestamp":"2026-06-05T00:00:00.000Z","cwd":"/tmp"}"#,
                "\n",
                r#"{"type":"message","timestamp":"2026-06-05T00:00:01.000Z","message":{"role":"user","content":[{"type":"text","text":"hello"}],"timestamp":1}}"#,
                "\n",
                r#"{"type":"compaction","timestamp":"2026-06-05T00:00:02.000Z","summary":"old summary","firstKeptEntryIndex":1,"tokensBefore":42}"#,
                "\n"
            ),
        )
        .unwrap();
        let storage = JsonlSessionStorage::open(&file).unwrap();
        assert_eq!(storage.header().version, 3);
        let entries = storage.get_entries();
        assert_eq!(entries.len(), 2);
        assert!(!entries[0].id.is_empty());
        assert!(!entries[1].id.is_empty());
        assert_ne!(entries[0].id, entries[1].id);
        assert_eq!(entries[0].parent_id, None);
        assert_eq!(
            entries[1].parent_id.as_deref(),
            Some(entries[0].id.as_str())
        );
        assert_eq!(
            entries[1]
                .field("firstKeptEntryId")
                .and_then(|value| value.as_str()),
            Some(entries[0].id.as_str())
        );
        assert!(entries[1].field("firstKeptEntryIndex").is_none());
        let rewritten = std::fs::read_to_string(&file).unwrap();
        assert!(rewritten.contains(r#""version":3"#));
        assert!(rewritten.contains(r#""parentId":"#));
        assert!(rewritten.contains("firstKeptEntryId"));
        assert!(!rewritten.contains("firstKeptEntryIndex"));
    }

    #[test]
    fn rejects_future_session_version() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("future_version.jsonl");
        std::fs::write(
            &file,
            r#"{"type":"session","version":4,"id":"s1","timestamp":"2026-06-05T00:00:00.000Z","cwd":"/tmp"}"#,
        )
        .unwrap();
        let error = JsonlSessionStorage::open(&file).unwrap_err();
        assert!(error.message.contains("unsupported session version"));
    }

    #[test]
    fn skips_empty_lines() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("with_blanks.jsonl");
        std::fs::write(
            &file,
            concat!(
                r#"{"type":"session","version":3,"id":"s1","timestamp":"2026-06-05T00:00:00.000Z","cwd":"/tmp"}"#,
                "\n\n",
                r#"{"type":"message","id":"entry001","parentId":null,"timestamp":"2026-06-05T00:00:01.000Z","message":{"role":"user","content":[{"type":"text","text":"hello"}],"timestamp":1}}"#,
                "\n"
            ),
        )
        .unwrap();
        let storage = JsonlSessionStorage::open(&file).unwrap();
        assert_eq!(storage.get_entries().len(), 1);
    }

    #[test]
    fn metadata_reflects_header() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("meta.jsonl");
        let storage = JsonlSessionStorage::create(
            &file,
            "/tmp/project",
            "meta-session",
            "2026-06-05T12:00:00.000Z",
            None,
        )
        .unwrap();
        let meta = storage.metadata();
        assert_eq!(meta.id, "meta-session");
        assert_eq!(meta.cwd, "/tmp/project");
        assert_eq!(meta.path, file);
    }

    #[test]
    fn opens_file_with_parent_session() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("with_parent.jsonl");
        std::fs::write(
            &file,
            r#"{"type":"session","version":3,"id":"s1","timestamp":"2026-06-05T00:00:00.000Z","cwd":"/tmp","parentSession":"/tmp/source.jsonl"}"#,
        )
        .unwrap();
        let storage = JsonlSessionStorage::open(&file).unwrap();
        assert_eq!(
            storage.header().parent_session.as_deref(),
            Some("/tmp/source.jsonl")
        );
    }
}
