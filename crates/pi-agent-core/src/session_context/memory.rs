use crate::session_context::{SessionError, SessionErrorCode};
use crate::transcript::{SessionEntry, SessionHeader};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct InMemorySessionStorage {
    header: SessionHeader,
    entries: Vec<SessionEntry>,
    by_id: HashMap<String, SessionEntry>,
    leaf_id: Option<String>,
}

impl InMemorySessionStorage {
    pub fn new(id: impl Into<String>, timestamp: impl Into<String>) -> Self {
        Self {
            header: SessionHeader {
                entry_type: "session".into(),
                version: 3,
                id: id.into(),
                timestamp: timestamp.into(),
                cwd: String::new(),
                parent_session: None,
            },
            entries: Vec::new(),
            by_id: HashMap::new(),
            leaf_id: None,
        }
    }

    pub fn header(&self) -> &SessionHeader {
        &self.header
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::StoredAgentMessage;
    use pi_ai::types::ContentBlock;

    fn user_entry(id: &str, parent: Option<&str>) -> SessionEntry {
        SessionEntry::message(
            id.into(),
            parent.map(str::to_string),
            "2026-06-05T00:00:00.000Z".into(),
            StoredAgentMessage::User {
                content: vec![ContentBlock::Text {
                    text: "hello".into(),
                    text_signature: None,
                }],
                timestamp: 1,
            },
        )
    }

    #[test]
    fn in_memory_storage_appends_and_tracks_leaf() {
        let mut storage = InMemorySessionStorage::new("session-1", "2026-06-05T00:00:00.000Z");
        storage.append_entry(user_entry("a", None)).unwrap();
        storage.append_entry(user_entry("b", Some("a"))).unwrap();
        assert_eq!(storage.get_leaf_id().unwrap().as_deref(), Some("b"));
        assert_eq!(storage.get_entries().len(), 2);
    }

    #[test]
    fn in_memory_leaf_entry_tracks_target_id() {
        let mut storage = InMemorySessionStorage::new("session-2", "2026-06-05T00:00:00.000Z");
        storage.append_entry(user_entry("a", None)).unwrap();
        storage.append_entry(user_entry("b", Some("a"))).unwrap();
        let mut leaf = SessionEntry {
            entry_type: "leaf".into(),
            id: "leaf001".into(),
            parent_id: Some("b".into()),
            timestamp: "2026-06-05T00:00:01.000Z".into(),
            fields: serde_json::Map::new(),
        };
        leaf.fields
            .insert("targetId".into(), serde_json::Value::String("a".into()));
        storage.append_entry(leaf).unwrap();
        assert_eq!(storage.get_leaf_id().unwrap().as_deref(), Some("a"));
    }

    #[test]
    fn rejects_duplicate_entry_id() {
        let mut storage = InMemorySessionStorage::new("session-3", "2026-06-05T00:00:00.000Z");
        storage.append_entry(user_entry("a", None)).unwrap();
        let err = storage
            .append_entry(user_entry("a", Some("b")))
            .unwrap_err();
        assert!(err.message.contains("duplicate entry id"));
    }

    #[test]
    fn header_is_accessible() {
        let storage = InMemorySessionStorage::new("session-4", "2026-06-05T00:00:00.000Z");
        assert_eq!(storage.header().id, "session-4");
    }
}
