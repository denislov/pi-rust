use crate::session::{
    JsonlSessionMetadata, JsonlSessionStorage, SessionEntry, SessionError, SessionErrorCode,
    create_session_id, create_timestamp,
};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct JsonlSessionRepo {
    sessions_root: PathBuf,
}

impl JsonlSessionRepo {
    pub fn new(sessions_root: impl AsRef<Path>) -> Self {
        Self {
            sessions_root: sessions_root.as_ref().to_path_buf(),
        }
    }

    pub fn encode_cwd(cwd: &str) -> String {
        let stripped = cwd.trim_start_matches(|c: char| c == '/' || c == '\\');
        let sanitized = stripped.replace(['/', '\\', ':'], "-");
        format!("--{sanitized}--")
    }

    pub fn session_dir(&self, cwd: &str) -> PathBuf {
        self.sessions_root.join(Self::encode_cwd(cwd))
    }

    pub fn create(&self, cwd: &str, id: Option<&str>) -> Result<JsonlSessionStorage, SessionError> {
        let dir = self.session_dir(cwd);
        fs::create_dir_all(&dir)
            .map_err(|e| SessionError::new(SessionErrorCode::Storage, format!("mkdir: {e}")))?;
        let sid = id.map(str::to_string).unwrap_or_else(create_session_id);
        let ts = create_timestamp();
        let ts_sanitized = ts.replace(':', "_").replace('.', "_");
        let filename = format!("{ts_sanitized}_{sid}.jsonl");
        let path = dir.join(&filename);
        JsonlSessionStorage::create(&path, cwd, &sid, &ts, None)
    }

    pub fn open(
        &self,
        metadata: &JsonlSessionMetadata,
    ) -> Result<JsonlSessionStorage, SessionError> {
        JsonlSessionStorage::open(&metadata.path)
    }

    pub fn list(&self, cwd: Option<&str>) -> Result<Vec<JsonlSessionMetadata>, SessionError> {
        let dirs: Vec<PathBuf> = if let Some(cwd) = cwd {
            let dir = self.session_dir(cwd);
            if dir.is_dir() {
                vec![dir]
            } else {
                return Ok(Vec::new());
            }
        } else {
            if !self.sessions_root.is_dir() {
                return Ok(Vec::new());
            }
            let mut dirs = Vec::new();
            for entry in fs::read_dir(&self.sessions_root).map_err(|e| {
                SessionError::new(SessionErrorCode::Storage, format!("read root: {e}"))
            })? {
                let entry = entry.map_err(|e| {
                    SessionError::new(SessionErrorCode::Storage, format!("read entry: {e}"))
                })?;
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    dirs.push(entry.path());
                }
            }
            dirs
        };

        let mut results = Vec::new();
        for dir in &dirs {
            let read_dir = match fs::read_dir(dir) {
                Ok(rd) => rd,
                Err(_) => continue,
            };
            for entry in read_dir {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }
                match JsonlSessionStorage::open(&path) {
                    Ok(storage) => results.push(storage.metadata()),
                    Err(_) => continue,
                }
            }
        }
        results.sort_by(|a, b| {
            let mtime = |p: &PathBuf| fs::metadata(p).and_then(|m| m.modified()).ok();
            mtime(&b.path).cmp(&mtime(&a.path))
        });
        Ok(results)
    }

    pub fn open_target(
        &self,
        cwd: &str,
        target: &str,
    ) -> Result<JsonlSessionStorage, SessionError> {
        if let Ok(storage) = JsonlSessionStorage::open(target) {
            return Ok(storage);
        }

        let all = self.list(Some(cwd))?;
        let exact: Vec<_> = all.iter().filter(|m| m.id == target).collect();
        if exact.len() == 1 {
            return self.open(exact[0]);
        }
        if exact.len() > 1 {
            return Err(SessionError::new(
                SessionErrorCode::InvalidSession,
                "multiple sessions match id",
            ));
        }

        let prefix_matches: Vec<_> = all.iter().filter(|m| m.id.starts_with(target)).collect();
        if prefix_matches.len() == 1 {
            return self.open(prefix_matches[0]);
        }
        if prefix_matches.len() > 1 {
            return Err(SessionError::new(
                SessionErrorCode::InvalidSession,
                format!("ambiguous prefix: {} sessions match", prefix_matches.len()),
            ));
        }

        Err(SessionError::new(
            SessionErrorCode::NotFound,
            format!("no session found for target: {target}"),
        ))
    }

    pub fn most_recent(&self, cwd: &str) -> Result<Option<JsonlSessionStorage>, SessionError> {
        let mut all = self.list(Some(cwd))?;
        if all.is_empty() {
            return Ok(None);
        }
        all.sort_by(|a, b| {
            let mtime = |p: &PathBuf| fs::metadata(p).and_then(|m| m.modified()).ok();
            mtime(&b.path).cmp(&mtime(&a.path))
        });
        for meta in &all {
            if let Ok(storage) = JsonlSessionStorage::open(&meta.path) {
                return Ok(Some(storage));
            }
        }
        Ok(None)
    }

    pub fn fork(
        &self,
        source_path: impl AsRef<Path>,
        target_cwd: &str,
        id: Option<&str>,
        entry_id: Option<&str>,
    ) -> Result<JsonlSessionStorage, SessionError> {
        let source = JsonlSessionStorage::open(source_path.as_ref())?;
        let entries = source.get_entries();
        let leaf_id = if let Some(eid) = entry_id {
            Some(eid.to_string())
        } else {
            source.get_leaf_id().unwrap_or(None)
        };

        let new_id = id.map(str::to_string).unwrap_or_else(create_session_id);
        let new_ts = create_timestamp();
        let dir = self.session_dir(target_cwd);
        fs::create_dir_all(&dir)
            .map_err(|e| SessionError::new(SessionErrorCode::Storage, format!("mkdir: {e}")))?;
        let ts_sanitized = new_ts.replace(':', "_").replace('.', "_");
        let filename = format!("{ts_sanitized}_{new_id}.jsonl");
        let path = dir.join(&filename);

        let mut target = JsonlSessionStorage::create(
            &path,
            target_cwd,
            &new_id,
            &new_ts,
            Some(source_path.as_ref().to_path_buf()),
        )?;

        if let Some(ref lid) = leaf_id {
            let by_id: std::collections::HashMap<&str, &SessionEntry> =
                entries.iter().map(|e| (e.id.as_str(), e)).collect();
            if !by_id.contains_key(lid.as_str()) {
                return Err(SessionError::new(
                    SessionErrorCode::InvalidForkTarget,
                    format!("entry id not found in source session: {lid}"),
                ));
            }
            let mut path_entries = Vec::new();
            let mut current: Option<&SessionEntry> = by_id.get(lid.as_str()).copied();
            while let Some(entry) = current {
                path_entries.push(entry.clone());
                current = entry
                    .parent_id
                    .as_deref()
                    .and_then(|pid| by_id.get(pid).copied());
            }
            path_entries.reverse();
            for entry in &path_entries {
                target.append_entry(entry.clone())?;
            }
        }

        Ok(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::StoredAgentMessage;
    use pi_ai::types::ContentBlock;
    use std::thread;
    use std::time::Duration;

    fn user_entry(id: &str, parent: Option<&str>, text: &str) -> SessionEntry {
        SessionEntry::message(
            id.into(),
            parent.map(str::to_string),
            crate::session::create_timestamp(),
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
    fn test_most_recent_returns_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let repo = JsonlSessionRepo::new(dir.path());

        let mut older = repo.create("/tmp/project", Some("session-older")).unwrap();
        older
            .append_entry(user_entry("entry001", None, "older"))
            .unwrap();
        drop(older);

        thread::sleep(Duration::from_millis(10));

        let mut newer = repo.create("/tmp/project", Some("session-newer")).unwrap();
        newer
            .append_entry(user_entry("entry001", None, "newer"))
            .unwrap();
        drop(newer);

        let most_recent = repo.most_recent("/tmp/project").unwrap();
        assert!(most_recent.is_some());
        assert_eq!(most_recent.unwrap().header().id, "session-newer");
    }

    #[test]
    fn test_fork_unknown_entry_id_errors() {
        let dir = tempfile::tempdir().unwrap();
        let repo = JsonlSessionRepo::new(dir.path());

        let source = repo.create("/tmp/project", Some("source-session")).unwrap();
        let source_path = source.path().to_path_buf();
        drop(source);

        let result = repo.fork(&source_path, "/tmp/fork", None, Some("nonexistent-entry"));
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().code,
            SessionErrorCode::InvalidForkTarget
        );
    }
}
