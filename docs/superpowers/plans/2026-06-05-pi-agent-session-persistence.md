# M3 Rust Session Persistence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Do not commit unless the user explicitly requests a commit.

**Goal:** Add JSONL v3 session persistence to Rust `pi-agent-core` and `pi-coding-agent` so print-mode CLI runs can create, continue, resume, and fork conversations across process invocations.

**Architecture:** `pi-agent-core` owns session wire types, context rebuilding, in-memory storage, JSONL storage, and cwd-scoped repos. `pi-coding-agent` owns CLI flag parsing and session target resolution, then hydrates `Agent` from the selected session and appends the new message batch after print mode settles. Normal Rust writes use the TypeScript coding-agent-compatible v3 subset while the reader accepts richer v3 files.

**Tech Stack:** Rust edition 2024, serde/serde_json, tokio tests, tempfile, uuid v7, time/RFC3339 formatting, existing faux provider. Behavioral references: `pi/packages/agent/src/harness/session/*`, `pi/packages/coding-agent/src/core/session-manager.ts`, `pi/packages/coding-agent/src/cli/args.ts`.

**Spec:** `docs/superpowers/specs/2026-06-05-pi-agent-session-persistence-design.md`

---

## File Structure

- Modify `crates/pi-agent-core/Cargo.toml` - add `thiserror`, `uuid`, `time`; add dev-dep `tempfile`.
- Modify `crates/pi-agent-core/src/lib.rs` - export `session`.
- Modify `crates/pi-agent-core/src/agent.rs` - add `Agent::with_messages` and `replace_messages`.
- Create `crates/pi-agent-core/src/session/mod.rs` - public exports.
- Create `crates/pi-agent-core/src/session/error.rs` - typed `SessionError`.
- Create `crates/pi-agent-core/src/session/id.rs` - UUIDv7 ids, short entry ids, test clock/id generator.
- Create `crates/pi-agent-core/src/session/types.rs` - header, entry envelope, stored message wire shape, metadata.
- Create `crates/pi-agent-core/src/session/context.rs` - path traversal and `AgentMessage` conversion.
- Create `crates/pi-agent-core/src/session/memory.rs` - in-memory storage.
- Create `crates/pi-agent-core/src/session/jsonl.rs` - JSONL open/create/append/load.
- Create `crates/pi-agent-core/src/session/repo.rs` - cwd-scoped create/open/list/delete/fork and target resolution.
- Create tests:
  - `crates/pi-agent-core/tests/session_wire.rs`
  - `crates/pi-agent-core/tests/session_context.rs`
  - `crates/pi-agent-core/tests/session_jsonl.rs`
  - `crates/pi-agent-core/tests/session_repo.rs`
  - `crates/pi-agent-core/tests/agent_hydration.rs`
- Modify `crates/pi-coding-agent/src/args.rs` - add session flags and help.
- Modify `crates/pi-coding-agent/src/error.rs` - add session parse/runtime errors.
- Modify `crates/pi-coding-agent/src/runtime.rs` - add session runtime options and cwd/session config.
- Modify `crates/pi-coding-agent/src/print_mode.rs` - hydrate/persist session messages.
- Modify `crates/pi-coding-agent/src/lib.rs` - pass cwd/session defaults into runtime.
- Create `crates/pi-coding-agent/src/session.rs` - CLI-facing session resolution.
- Create tests:
  - `crates/pi-coding-agent/tests/session_args.rs`
  - `crates/pi-coding-agent/tests/session_print_mode.rs`
  - `crates/pi-coding-agent/tests/session_cli.rs`

---

## Task 1: Session Wire Types and Fixture Compatibility

**Files:**
- Modify: `crates/pi-agent-core/Cargo.toml`
- Modify: `crates/pi-agent-core/src/lib.rs`
- Create: `crates/pi-agent-core/src/session/{mod.rs,error.rs,id.rs,types.rs}`
- Test: `crates/pi-agent-core/tests/session_wire.rs`

- [ ] **Step 1: Add dependencies**

Add these dependencies to `crates/pi-agent-core/Cargo.toml`:

```toml
thiserror = "2"
time = { version = "0.3", features = ["formatting", "macros"] }
uuid = { version = "1", features = ["v7"] }

[dev-dependencies]
tempfile = "3"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

Keep existing dependencies intact.

- [ ] **Step 2: Write failing wire tests**

Create `crates/pi-agent-core/tests/session_wire.rs`:

```rust
use pi_agent_core::session::{
    SessionEntry, SessionHeader, StoredAgentMessage, StoredUsage, StoredUsageCost,
};
use pi_ai::types::{ContentBlock, StopReason};

#[test]
fn header_serializes_as_jsonl_v3_header() {
    let header = SessionHeader {
        entry_type: "session".into(),
        version: 3,
        id: "019de8c2-de29-73e9-ae0c-e134db34c447".into(),
        timestamp: "2026-06-05T00:00:00.000Z".into(),
        cwd: "/tmp/project".into(),
        parent_session: Some("/tmp/source.jsonl".into()),
    };
    let json = serde_json::to_string(&header).unwrap();
    assert_eq!(
        json,
        r#"{"type":"session","version":3,"id":"019de8c2-de29-73e9-ae0c-e134db34c447","timestamp":"2026-06-05T00:00:00.000Z","cwd":"/tmp/project","parentSession":"/tmp/source.jsonl"}"#
    );
}

#[test]
fn user_message_entry_matches_typescript_shape() {
    let entry = SessionEntry::message(
        "entry001".into(),
        None,
        "2026-06-05T00:00:01.000Z".into(),
        StoredAgentMessage::User {
            content: vec![ContentBlock::Text {
                text: "hello".into(),
                text_signature: None,
            }],
            timestamp: 1_780_588_800_000,
        },
    );
    let value = serde_json::to_value(&entry).unwrap();
    assert_eq!(value["type"], "message");
    assert_eq!(value["parentId"], serde_json::Value::Null);
    assert_eq!(value["message"]["role"], "user");
    assert_eq!(value["message"]["content"][0]["type"], "text");
}

#[test]
fn assistant_usage_uses_typescript_total_field() {
    let entry = SessionEntry::message(
        "entry002".into(),
        Some("entry001".into()),
        "2026-06-05T00:00:02.000Z".into(),
        StoredAgentMessage::Assistant {
            content: vec![ContentBlock::Text {
                text: "hi".into(),
                text_signature: None,
            }],
            api: "anthropic-messages".into(),
            provider: "anthropic".into(),
            model: "claude-sonnet-4-5".into(),
            response_model: None,
            response_id: None,
            usage: StoredUsage {
                input: 0,
                output: 0,
                cache_read: 0,
                cache_write: 0,
                total: 0,
                cost: StoredUsageCost::default(),
            },
            stop_reason: StopReason::Stop,
            error_message: None,
            timestamp: 1_780_588_801_000,
        },
    );
    let value = serde_json::to_value(&entry).unwrap();
    assert_eq!(value["message"]["role"], "assistant");
    assert!(value["message"]["usage"].get("total").is_some());
    assert!(value["message"]["usage"].get("totalTokens").is_none());
    assert_eq!(value["message"]["stopReason"], "stop");
}

#[test]
fn leaf_entry_roundtrips_without_losing_target_id() {
    let raw = r#"{"type":"leaf","id":"leaf0001","parentId":"entry002","timestamp":"2026-06-05T00:00:03.000Z","targetId":"entry001"}"#;
    let entry: SessionEntry = serde_json::from_str(raw).unwrap();
    assert_eq!(entry.entry_type, "leaf");
    assert_eq!(entry.field("targetId").and_then(|v| v.as_str()), Some("entry001"));
    assert_eq!(serde_json::to_string(&entry).unwrap(), raw);
}
```

- [ ] **Step 3: Run the failing test**

Run:

```bash
cargo test -p pi-agent-core --test session_wire
```

Expected: FAIL because `pi_agent_core::session` does not exist.

- [ ] **Step 4: Implement session exports and wire structs**

Create `src/session/mod.rs`:

```rust
pub mod error;
pub mod id;
pub mod types;

pub use error::{SessionError, SessionErrorCode};
pub use id::{SessionIdGenerator, create_session_id, create_timestamp, generate_entry_id};
pub use types::{
    JsonlSessionMetadata, SessionEntry, SessionHeader, SessionMetadata, StoredAgentMessage,
    StoredUsage, StoredUsageCost,
};
```

Add `pub mod session;` to `crates/pi-agent-core/src/lib.rs`.

Implement `error.rs` with:

```rust
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionErrorCode {
    NotFound,
    InvalidSession,
    InvalidEntry,
    InvalidForkTarget,
    Storage,
    Unknown,
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct SessionError {
    pub code: SessionErrorCode,
    pub message: String,
}

impl SessionError {
    pub fn new(code: SessionErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
}
```

Implement `id.rs` with UUIDv7 ids and RFC3339 millisecond timestamps:

```rust
use crate::session::{SessionError, SessionErrorCode};
use std::collections::HashSet;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

pub fn create_session_id() -> String {
    Uuid::now_v7().to_string()
}

pub fn create_timestamp() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
        .replace("+00:00", "Z")
}

pub fn generate_entry_id(existing: &HashSet<String>) -> String {
    for _ in 0..100 {
        let full = create_session_id();
        let short = full.chars().take(8).collect::<String>();
        if !existing.contains(&short) {
            return short;
        }
    }
    create_session_id()
}

#[derive(Debug, Clone)]
pub struct SessionIdGenerator {
    pub session_id: String,
    pub entry_ids: Vec<String>,
    pub timestamp: String,
}

impl SessionIdGenerator {
    pub fn fixed(session_id: &str, entry_ids: Vec<&str>, timestamp: &str) -> Self {
        Self {
            session_id: session_id.into(),
            entry_ids: entry_ids.into_iter().map(str::to_string).collect(),
            timestamp: timestamp.into(),
        }
    }

    pub fn next_entry_id(&mut self) -> Result<String, SessionError> {
        if self.entry_ids.is_empty() {
            return Err(SessionError::new(SessionErrorCode::Unknown, "test entry id generator exhausted"));
        }
        Ok(self.entry_ids.remove(0))
    }
}
```

Implement `types.rs` using an envelope that preserves unknown fields:

```rust
use pi_ai::types::{ContentBlock, StopReason};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionHeader {
    #[serde(rename = "type")]
    pub entry_type: String,
    pub version: u32,
    pub id: String,
    pub timestamp: String,
    pub cwd: String,
    #[serde(rename = "parentSession", skip_serializing_if = "Option::is_none")]
    pub parent_session: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionEntry {
    #[serde(rename = "type")]
    pub entry_type: String,
    pub id: String,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
    pub timestamp: String,
    #[serde(flatten)]
    pub fields: Map<String, Value>,
}

impl SessionEntry {
    pub fn message(id: String, parent_id: Option<String>, timestamp: String, message: StoredAgentMessage) -> Self {
        let mut fields = Map::new();
        fields.insert("message".into(), serde_json::to_value(message).expect("stored message serializes"));
        Self { entry_type: "message".into(), id, parent_id, timestamp, fields }
    }

    pub fn session_info(id: String, parent_id: Option<String>, timestamp: String, name: String) -> Self {
        let mut fields = Map::new();
        fields.insert("name".into(), Value::String(name));
        Self { entry_type: "session_info".into(), id, parent_id, timestamp, fields }
    }

    pub fn field(&self, key: &str) -> Option<&Value> {
        self.fields.get(key)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "role")]
pub enum StoredAgentMessage {
    #[serde(rename = "user")]
    User { content: Vec<ContentBlock>, timestamp: u64 },
    #[serde(rename = "assistant")]
    Assistant {
        content: Vec<ContentBlock>,
        api: String,
        provider: String,
        model: String,
        #[serde(rename = "responseModel", skip_serializing_if = "Option::is_none")]
        response_model: Option<String>,
        #[serde(rename = "responseId", skip_serializing_if = "Option::is_none")]
        response_id: Option<String>,
        usage: StoredUsage,
        #[serde(rename = "stopReason")]
        stop_reason: StopReason,
        #[serde(rename = "errorMessage", skip_serializing_if = "Option::is_none")]
        error_message: Option<String>,
        timestamp: u64,
    },
    #[serde(rename = "toolResult")]
    ToolResult {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        content: Vec<ContentBlock>,
        #[serde(rename = "isError")]
        is_error: bool,
        timestamp: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct StoredUsageCost {
    pub input: f64,
    pub output: f64,
    #[serde(rename = "cacheRead")]
    pub cache_read: f64,
    #[serde(rename = "cacheWrite")]
    pub cache_write: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct StoredUsage {
    pub input: u32,
    pub output: u32,
    #[serde(rename = "cacheRead")]
    pub cache_read: u32,
    #[serde(rename = "cacheWrite")]
    pub cache_write: u32,
    pub total: u32,
    pub cost: StoredUsageCost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionMetadata {
    pub id: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonlSessionMetadata {
    pub id: String,
    pub created_at: String,
    pub cwd: String,
    pub path: std::path::PathBuf,
    pub parent_session_path: Option<std::path::PathBuf>,
}
```

- [ ] **Step 5: Run the wire test**

Run:

```bash
cargo test -p pi-agent-core --test session_wire
```

Expected: PASS.

---

## Task 2: Context Builder and In-Memory Storage

**Files:**
- Modify: `crates/pi-agent-core/src/session/mod.rs`
- Create: `crates/pi-agent-core/src/session/{context.rs,memory.rs}`
- Test: `crates/pi-agent-core/tests/session_context.rs`

- [ ] **Step 1: Write failing context/storage tests**

Create `crates/pi-agent-core/tests/session_context.rs`:

```rust
use pi_agent_core::session::{
    InMemorySessionStorage, SessionContext, SessionEntry, StoredAgentMessage, build_session_context,
};
use pi_ai::types::ContentBlock;

fn user(text: &str, id: &str, parent: Option<&str>) -> SessionEntry {
    SessionEntry::message(
        id.into(),
        parent.map(str::to_string),
        "2026-06-05T00:00:00.000Z".into(),
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
fn builds_context_from_latest_linear_leaf() {
    let entries = vec![user("one", "a", None), user("two", "b", Some("a"))];
    let context = build_session_context(&entries, None).unwrap();
    assert_eq!(context.messages.len(), 2);
}

#[test]
fn builds_context_from_explicit_leaf_entry_target() {
    let mut leaf = SessionEntry {
        entry_type: "leaf".into(),
        id: "leaf0001".into(),
        parent_id: Some("b".into()),
        timestamp: "2026-06-05T00:00:01.000Z".into(),
        fields: serde_json::Map::new(),
    };
    leaf.fields.insert("targetId".into(), serde_json::Value::String("a".into()));
    let entries = vec![user("one", "a", None), user("two", "b", Some("a")), leaf];
    let context = build_session_context(&entries, None).unwrap();
    assert_eq!(context.messages.len(), 1);
}

#[test]
fn in_memory_storage_appends_and_tracks_leaf() {
    let mut storage = InMemorySessionStorage::new("session-1", "2026-06-05T00:00:00.000Z");
    storage.append_entry(user("one", "a", None)).unwrap();
    storage.append_entry(user("two", "b", Some("a"))).unwrap();
    assert_eq!(storage.get_leaf_id().unwrap().as_deref(), Some("b"));
    assert_eq!(storage.get_entries().len(), 2);
}
```

- [ ] **Step 2: Run failing context tests**

Run:

```bash
cargo test -p pi-agent-core --test session_context
```

Expected: FAIL because context and memory storage are not exported.

- [ ] **Step 3: Implement context conversion**

In `session/context.rs`, implement:

```rust
use crate::types::AgentMessage;
use crate::session::{SessionEntry, SessionError, SessionErrorCode, StoredAgentMessage};
use pi_ai::types::{AssistantMessage, Usage, Cost};
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct SessionContext {
    pub messages: Vec<AgentMessage>,
    pub thinking_level: String,
    pub model: Option<(String, String)>,
    pub active_tool_names: Option<Vec<String>>,
}

pub fn build_session_context(
    entries: &[SessionEntry],
    explicit_leaf_id: Option<&str>,
) -> Result<SessionContext, SessionError> {
    let by_id: HashMap<&str, &SessionEntry> = entries.iter().map(|entry| (entry.id.as_str(), entry)).collect();
    let leaf_id = explicit_leaf_id.map(str::to_string).or_else(|| infer_leaf_id(entries));
    let path = path_to_root(leaf_id.as_deref(), &by_id)?;
    let mut context = SessionContext { thinking_level: "off".into(), ..Default::default() };
    for entry in path {
        match entry.entry_type.as_str() {
            "message" => {
                if let Some(message) = entry.field("message").and_then(|value| {
                    serde_json::from_value::<StoredAgentMessage>(value.clone()).ok()
                }) {
                    if let Some(agent_message) = stored_to_agent_message(&entry.id, message) {
                        context.messages.push(agent_message);
                    }
                }
            }
            "thinking_level_change" => {
                if let Some(level) = entry.field("thinkingLevel").and_then(|value| value.as_str()) {
                    context.thinking_level = level.to_string();
                }
            }
            "model_change" => {
                let provider = entry.field("provider").and_then(|value| value.as_str());
                let model_id = entry.field("modelId").and_then(|value| value.as_str());
                if let (Some(provider), Some(model_id)) = (provider, model_id) {
                    context.model = Some((provider.to_string(), model_id.to_string()));
                }
            }
            "active_tools_change" => {
                if let Some(names) = entry.field("activeToolNames").and_then(|value| value.as_array()) {
                    context.active_tool_names = Some(
                        names.iter().filter_map(|value| value.as_str().map(str::to_string)).collect(),
                    );
                }
            }
            "compaction" => {
                if let Some(summary) = entry.field("summary").and_then(|value| value.as_str()) {
                    context.messages.push(AgentMessage::UserText {
                        message_id: entry.id.clone(),
                        text: format!(
                            "The conversation history before this point was compacted into the following summary:\n\n<summary>\n{summary}\n</summary>"
                        ),
                    });
                }
            }
            "branch_summary" => {
                if let Some(summary) = entry.field("summary").and_then(|value| value.as_str()) {
                    context.messages.push(AgentMessage::UserText {
                        message_id: entry.id.clone(),
                        text: format!(
                            "The following is a summary of a branch that this conversation came back from:\n\n<summary>\n{summary}\n</summary>"
                        ),
                    });
                }
            }
            _ => {}
        }
    }
    Ok(context)
}
```

Also implement `infer_leaf_id`, `path_to_root`, and `stored_to_agent_message`. `infer_leaf_id`
honors a final `leaf.targetId` when present, otherwise the latest non-header entry id. For
`StoredAgentMessage::Assistant`, construct `pi_ai::types::AssistantMessage` and map `StoredUsage`
into Rust `Usage`; `StoredUsage.total` maps to `Usage.total_tokens`.

- [ ] **Step 4: Implement in-memory storage**

In `session/memory.rs`, implement:

```rust
use crate::session::{SessionEntry, SessionError, SessionErrorCode, SessionHeader};
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

    pub fn header(&self) -> &SessionHeader { &self.header }
    pub fn get_entries(&self) -> Vec<SessionEntry> { self.entries.clone() }
    pub fn get_leaf_id(&self) -> Result<Option<String>, SessionError> { Ok(self.leaf_id.clone()) }

    pub fn append_entry(&mut self, entry: SessionEntry) -> Result<(), SessionError> {
        if self.by_id.contains_key(&entry.id) {
            return Err(SessionError::new(SessionErrorCode::InvalidEntry, format!("duplicate entry id: {}", entry.id)));
        }
        self.leaf_id = if entry.entry_type == "leaf" {
            entry.field("targetId").and_then(|value| value.as_str()).map(str::to_string)
        } else {
            Some(entry.id.clone())
        };
        self.by_id.insert(entry.id.clone(), entry.clone());
        self.entries.push(entry);
        Ok(())
    }
}
```

Export `build_session_context`, `SessionContext`, and `InMemorySessionStorage` from
`session/mod.rs`.

- [ ] **Step 5: Run context tests**

Run:

```bash
cargo test -p pi-agent-core --test session_context
```

Expected: PASS.

---

## Task 3: JSONL Storage

**Files:**
- Modify: `crates/pi-agent-core/src/session/mod.rs`
- Create: `crates/pi-agent-core/src/session/jsonl.rs`
- Test: `crates/pi-agent-core/tests/session_jsonl.rs`

- [ ] **Step 1: Write failing JSONL tests**

Create `crates/pi-agent-core/tests/session_jsonl.rs`:

```rust
use pi_agent_core::session::{JsonlSessionStorage, SessionEntry, StoredAgentMessage};
use pi_ai::types::ContentBlock;

fn user_entry(id: &str, parent: Option<&str>, text: &str) -> SessionEntry {
    SessionEntry::message(
        id.into(),
        parent.map(str::to_string),
        "2026-06-05T00:00:01.000Z".into(),
        StoredAgentMessage::User {
            content: vec![ContentBlock::Text { text: text.into(), text_signature: None }],
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
    ).unwrap();
    storage.append_entry(user_entry("entry001", None, "hello")).unwrap();
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
    ).unwrap();
    let storage = JsonlSessionStorage::open(&file).unwrap();
    assert_eq!(storage.header().id, "session-1");
    assert_eq!(storage.get_leaf_id().unwrap().as_deref(), Some("entry001"));
    assert_eq!(storage.get_entries().len(), 1);
}

#[test]
fn rejects_missing_header() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("bad.jsonl");
    std::fs::write(&file, r#"{"type":"message","id":"x","parentId":null,"timestamp":"now"}"#).unwrap();
    let error = JsonlSessionStorage::open(&file).unwrap_err();
    assert!(error.message.contains("first line is not a valid session header"));
}
```

- [ ] **Step 2: Run failing JSONL tests**

Run:

```bash
cargo test -p pi-agent-core --test session_jsonl
```

Expected: FAIL because `JsonlSessionStorage` is missing.

- [ ] **Step 3: Implement JSONL storage**

In `session/jsonl.rs`, implement a synchronous append-only storage:

```rust
use crate::session::{JsonlSessionMetadata, SessionEntry, SessionError, SessionErrorCode, SessionHeader};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
```

Required public API:

```rust
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
    ) -> Result<Self, SessionError>;

    pub fn open(path: impl AsRef<Path>) -> Result<Self, SessionError>;
    pub fn header(&self) -> &SessionHeader;
    pub fn path(&self) -> &Path;
    pub fn metadata(&self) -> JsonlSessionMetadata;
    pub fn get_entries(&self) -> Vec<SessionEntry>;
    pub fn get_leaf_id(&self) -> Result<Option<String>, SessionError>;
    pub fn append_entry(&mut self, entry: SessionEntry) -> Result<(), SessionError>;
}
```

Implementation rules:

- `create()` creates parent directories, writes exactly one header line, and fails if the path cannot
  be written.
- `open()` reads non-empty lines, validates the first line as `SessionHeader`, requires
  `type=session`, `version=3`, non-empty `id`, non-empty `timestamp`, and non-empty `cwd`.
- Entry parse failures return `SessionErrorCode::InvalidEntry` with line number.
- Unknown entry types are accepted because `SessionEntry` preserves flattened fields.
- `append_entry()` appends one JSON line, updates `by_id`, and updates `leaf_id`; for `leaf`,
  `targetId: null` clears the leaf.
- Use `OpenOptions::new().append(true).open(path)` for append.

- [ ] **Step 4: Export storage**

Add to `session/mod.rs`:

```rust
pub mod jsonl;
pub use jsonl::JsonlSessionStorage;
```

- [ ] **Step 5: Run JSONL tests**

Run:

```bash
cargo test -p pi-agent-core --test session_jsonl
```

Expected: PASS.

---

## Task 4: JSONL Repo, Cwd Encoding, Open/Continue/Fork

**Files:**
- Modify: `crates/pi-agent-core/src/session/mod.rs`
- Create: `crates/pi-agent-core/src/session/repo.rs`
- Test: `crates/pi-agent-core/tests/session_repo.rs`

- [ ] **Step 1: Write failing repo tests**

Create `crates/pi-agent-core/tests/session_repo.rs`:

```rust
use pi_agent_core::session::{JsonlSessionRepo, SessionEntry, StoredAgentMessage};
use pi_ai::types::ContentBlock;

fn user(id: &str, parent: Option<&str>, text: &str) -> SessionEntry {
    SessionEntry::message(
        id.into(),
        parent.map(str::to_string),
        "2026-06-05T00:00:01.000Z".into(),
        StoredAgentMessage::User {
            content: vec![ContentBlock::Text { text: text.into(), text_signature: None }],
            timestamp: 1,
        },
    )
}

#[test]
fn encodes_cwd_like_typescript() {
    assert_eq!(
        JsonlSessionRepo::encode_cwd("/home/me/project"),
        "--home-me-project--"
    );
}

#[test]
fn creates_lists_and_opens_by_id_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let repo = JsonlSessionRepo::new(dir.path());
    let mut session = repo.create("/tmp/project", Some("019de8c2-de29-73e9-ae0c-e134db34c447")).unwrap();
    session.append_entry(user("entry001", None, "hello")).unwrap();
    let listed = repo.list(Some("/tmp/project")).unwrap();
    assert_eq!(listed.len(), 1);
    let opened = repo.open_target("/tmp/project", "019de8c2").unwrap();
    assert_eq!(opened.header().id, "019de8c2-de29-73e9-ae0c-e134db34c447");
}

#[test]
fn forks_session_with_parent_session_header() {
    let dir = tempfile::tempdir().unwrap();
    let repo = JsonlSessionRepo::new(dir.path());
    let mut source = repo.create("/tmp/project", Some("source-session")).unwrap();
    source.append_entry(user("entry001", None, "hello")).unwrap();
    let fork = repo.fork(source.path(), "/tmp/project", Some("fork-session"), None).unwrap();
    assert_eq!(fork.header().parent_session.as_deref(), Some(source.path().to_str().unwrap()));
    assert_eq!(fork.get_entries().len(), 1);
}
```

- [ ] **Step 2: Run failing repo tests**

Run:

```bash
cargo test -p pi-agent-core --test session_repo
```

Expected: FAIL because `JsonlSessionRepo` is missing.

- [ ] **Step 3: Implement repo**

In `session/repo.rs`, implement:

```rust
use crate::session::{
    JsonlSessionMetadata, JsonlSessionStorage, SessionError, SessionErrorCode,
    create_session_id, create_timestamp,
};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct JsonlSessionRepo {
    sessions_root: PathBuf,
}
```

Required public API:

```rust
impl JsonlSessionRepo {
    pub fn new(sessions_root: impl AsRef<Path>) -> Self;
    pub fn encode_cwd(cwd: &str) -> String;
    pub fn session_dir(&self, cwd: &str) -> PathBuf;
    pub fn create(&self, cwd: &str, id: Option<&str>) -> Result<JsonlSessionStorage, SessionError>;
    pub fn open(&self, metadata: &JsonlSessionMetadata) -> Result<JsonlSessionStorage, SessionError>;
    pub fn list(&self, cwd: Option<&str>) -> Result<Vec<JsonlSessionMetadata>, SessionError>;
    pub fn open_target(&self, cwd: &str, target: &str) -> Result<JsonlSessionStorage, SessionError>;
    pub fn most_recent(&self, cwd: &str) -> Result<Option<JsonlSessionStorage>, SessionError>;
    pub fn fork(
        &self,
        source_path: impl AsRef<Path>,
        target_cwd: &str,
        id: Option<&str>,
        entry_id: Option<&str>,
    ) -> Result<JsonlSessionStorage, SessionError>;
}
```

Implementation rules:

- `session_dir(cwd)` returns `sessions_root/encode_cwd(cwd)`.
- `create()` writes file name `<timestamp-with-:-.-replaced>_<id>.jsonl`.
- `list(Some(cwd))` reads only that cwd directory. `list(None)` scans all cwd directories under
  `sessions_root`.
- `open_target()` treats an existing file path as path first; otherwise it matches exact id or unique
  id prefix for the cwd. Multiple prefix matches return `InvalidSession`.
- `most_recent()` sorts by filesystem modified time descending and ignores invalid JSONL files.
- `fork()` opens the source, copies the active branch path, writes a new header with
  `parentSession=<source path>`, and appends copied entries in path order. When `entry_id` is `Some`,
  fork through that entry; when it is `None`, fork through the active leaf.

- [ ] **Step 4: Export repo**

Add to `session/mod.rs`:

```rust
pub mod repo;
pub use repo::JsonlSessionRepo;
```

- [ ] **Step 5: Run repo tests**

Run:

```bash
cargo test -p pi-agent-core --test session_repo
```

Expected: PASS.

---

## Task 5: Agent Hydration Helpers

**Files:**
- Modify: `crates/pi-agent-core/src/agent.rs`
- Test: `crates/pi-agent-core/tests/agent_hydration.rs`

- [ ] **Step 1: Write failing hydration test**

Create `crates/pi-agent-core/tests/agent_hydration.rs`:

```rust
mod common;

use common::{faux_model, text_turn};
use pi_agent_core::{Agent, AgentConfig, AgentEvent, AgentMessage};
use pi_ai::providers::faux::FauxProvider;
use pi_ai::registry;
use std::sync::Arc;
use futures::StreamExt;

#[tokio::test]
async fn prompt_starts_after_hydrated_messages() {
    let api = "agent-hydration-history";
    registry::register(api, Arc::new(FauxProvider::new(vec![text_turn("second answer")])));
    let config = AgentConfig {
        model: faux_model(api),
        system_prompt: Some("system".into()),
        max_turns: 5,
        stream_options: None,
    };
    let agent = Agent::with_messages(config, vec![AgentMessage::UserText {
        message_id: "entry001".into(),
        text: "first".into(),
    }]);
    let baseline = agent.messages().len();
    let mut stream = agent.prompt("second");
    while let Some(event) = stream.next().await {
        if matches!(event, AgentEvent::AgentError { .. }) {
            panic!("unexpected agent error");
        }
    }
    let messages = agent.messages();
    assert_eq!(baseline, 1);
    assert!(matches!(messages[0], AgentMessage::UserText { .. }));
    assert!(messages.len() >= 3);
    registry::unregister(api);
}
```

- [ ] **Step 2: Run failing hydration test**

Run:

```bash
cargo test -p pi-agent-core --test agent_hydration
```

Expected: FAIL because `Agent::with_messages` is missing.

- [ ] **Step 3: Implement hydration helpers**

In `agent.rs`, add:

```rust
impl Agent {
    pub fn with_messages(config: AgentConfig, messages: Vec<AgentMessage>) -> Self {
        let agent = Self::new(config);
        agent.replace_messages(messages);
        agent
    }

    pub fn replace_messages(&self, messages: Vec<AgentMessage>) {
        self.state.write().unwrap().messages = messages;
    }
}
```

Do not change `prompt()`, `add_message()`, or current loop behavior.

- [ ] **Step 4: Run hydration and agent loop tests**

Run:

```bash
cargo test -p pi-agent-core --test agent_hydration
cargo test -p pi-agent-core --test agent_loop
```

Expected: PASS.

---

## Task 6: CLI Session Flags and Runtime Options

**Files:**
- Modify: `crates/pi-coding-agent/src/args.rs`
- Modify: `crates/pi-coding-agent/src/error.rs`
- Modify: `crates/pi-coding-agent/src/runtime.rs`
- Modify: `crates/pi-coding-agent/src/lib.rs`
- Create: `crates/pi-coding-agent/src/session.rs`
- Test: `crates/pi-coding-agent/tests/session_args.rs`

- [ ] **Step 1: Write failing arg tests**

Create `crates/pi-coding-agent/tests/session_args.rs`:

```rust
use pi_coding_agent::parse_args;

#[test]
fn parses_session_flags() {
    let args = parse_args(vec![
        "-p".into(),
        "hi".into(),
        "--continue".into(),
        "--session-dir".into(),
        "/tmp/sessions".into(),
        "--name".into(),
        "work".into(),
    ]).unwrap();
    assert!(args.continue_session);
    assert_eq!(args.session_dir.as_deref(), Some("/tmp/sessions"));
    assert_eq!(args.name.as_deref(), Some("work"));
}

#[test]
fn rejects_no_session_with_session_target() {
    let err = parse_args(vec!["-p".into(), "hi".into(), "--no-session".into(), "--continue".into()]).unwrap_err();
    assert_eq!(err.to_string(), "--no-session cannot be combined with session selection flags");
}

#[test]
fn help_mentions_session_flags() {
    let help = pi_coding_agent::help_text();
    assert!(help.contains("--continue"));
    assert!(help.contains("--session <path|id>"));
    assert!(help.contains("--no-session"));
}
```

- [ ] **Step 2: Run failing arg tests**

Run:

```bash
cargo test -p pi-coding-agent --test session_args
```

Expected: FAIL because the flags are missing.

- [ ] **Step 3: Extend `CliArgs`**

Add fields to `CliArgs`:

```rust
pub continue_session: bool,
pub resume: bool,
pub no_session: bool,
pub session: Option<String>,
pub session_id: Option<String>,
pub fork: Option<String>,
pub session_dir: Option<String>,
pub name: Option<String>,
```

Parse:

- `--continue` and `-c` -> `continue_session = true`
- `--resume` and `-r` -> `resume = true`
- `--no-session` -> `no_session = true`
- `--session <path|id>`
- `--session-id <id>`
- `--fork <path|id>`
- `--session-dir <dir>`
- `--name <name>` and `-n <name>`

After parsing, reject `--no-session` combined with any session selection flag or `--name`.

- [ ] **Step 4: Extend errors**

In `error.rs`, add variants:

```rust
InvalidSessionFlags(String),
SessionFailure(String),
```

Render `InvalidSessionFlags(message)` as `message`.

- [ ] **Step 5: Add runtime session options**

In `runtime.rs`, add:

```rust
#[derive(Clone, Debug)]
pub enum SessionMode {
    Enabled,
    Disabled,
}

#[derive(Clone, Debug)]
pub struct SessionRunOptions {
    pub mode: SessionMode,
    pub cwd: std::path::PathBuf,
    pub session_dir: Option<std::path::PathBuf>,
}

impl SessionRunOptions {
    pub fn disabled(cwd: std::path::PathBuf) -> Self {
        Self { mode: SessionMode::Disabled, cwd, session_dir: None }
    }

    pub fn enabled(cwd: std::path::PathBuf) -> Self {
        Self { mode: SessionMode::Enabled, cwd, session_dir: None }
    }
}
```

Add `pub session: SessionRunOptions` to `CliRunOptions`. Update `Default` to use
`SessionRunOptions::disabled(PathBuf::from("."))` so existing injection tests stay isolated.
Update `default_cli_options(cwd)` in `lib.rs` to pass `SessionRunOptions::enabled(cwd.clone())`.

- [ ] **Step 6: Create session resolver helpers**

Create `src/session.rs` with exported types and pure helpers:

```rust
pub fn encode_cwd(cwd: &std::path::Path) -> String;
pub fn default_sessions_root() -> Result<std::path::PathBuf, crate::CliError>;
pub fn resolve_session_dir(
    cwd: &std::path::Path,
    cli_session_dir: Option<&str>,
    runtime_session_dir: Option<&std::path::Path>,
) -> Result<std::path::PathBuf, crate::CliError>;
```

Use environment variables in this order:

1. CLI `--session-dir`
2. runtime injected `session_dir`
3. `PI_SESSION_DIR`
4. `PI_AGENT_DIR/sessions`
5. `$HOME/.pi/agent/sessions`

- [ ] **Step 7: Export session module**

Add `pub mod session;` in `lib.rs` and re-export `ResolvedSessionTarget` from `runtime.rs` only if
that type lives there. Keep helper functions public because integration tests use them to verify
path and environment precedence.

- [ ] **Step 8: Run arg tests and existing CLI tests**

Run:

```bash
cargo test -p pi-coding-agent --test session_args
cargo test -p pi-coding-agent --test cli
cargo test -p pi-coding-agent --test args
```

Expected: PASS.

---

## Task 7: Print Mode Session Persistence

**Files:**
- Modify: `crates/pi-coding-agent/src/print_mode.rs`
- Modify: `crates/pi-coding-agent/src/session.rs`
- Modify: `crates/pi-coding-agent/src/runtime.rs`
- Test: `crates/pi-coding-agent/tests/session_print_mode.rs`

- [ ] **Step 1: Write failing print-mode session tests**

Create `crates/pi-coding-agent/tests/session_print_mode.rs`:

```rust
use pi_ai::providers::faux::FauxProvider;
use pi_ai::registry;
use pi_ai::types::{Model, ModelCost, ModelInput};
use pi_coding_agent::{PrintModeOptions, run_print_mode};
use std::sync::Arc;

fn faux_model(api: &str) -> Model {
    Model {
        id: "faux-model".into(),
        name: "Faux Model".into(),
        api: api.into(),
        provider: "faux".into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost { input: 0.0, output: 0.0, cache_read: 0.0, cache_write: 0.0 },
        context_window: 0,
        max_tokens: 0,
        headers: None,
        compat: None,
    }
}

#[tokio::test]
async fn persists_new_print_mode_session() {
    let api = "session-print-persist";
    registry::register(api, Arc::new(FauxProvider::simple_text("hello")));
    let dir = tempfile::tempdir().unwrap();

    let output = run_print_mode(PrintModeOptions {
        prompt: "hi".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: 5,
        tools: Vec::new(),
        register_builtins: false,
        session: Some(pi_coding_agent::runtime::SessionRunOptions {
            mode: pi_coding_agent::runtime::SessionMode::Enabled,
            cwd: dir.path().join("project"),
            session_dir: Some(dir.path().join("sessions")),
        }),
        session_target: None,
        session_name: None,
    }).await.unwrap();

    assert_eq!(output, "hello");
    fn collect_jsonl_files(root: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect_jsonl_files(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                out.push(path);
            }
        }
    }

    let mut files = Vec::new();
    collect_jsonl_files(dir.path(), &mut files);
    assert_eq!(files.len(), 1);
    let text = std::fs::read_to_string(&files[0]).unwrap();
    assert!(text.contains(r#""type":"session""#));
    assert!(text.contains(r#""role":"user""#));
    assert!(text.contains(r#""role":"assistant""#));
    registry::unregister(api);
}
```

- [ ] **Step 2: Run failing print-mode session test**

Run:

```bash
cargo test -p pi-coding-agent --test session_print_mode
```

Expected: FAIL because `PrintModeOptions` has no session fields.

- [ ] **Step 3: Extend `PrintModeOptions`**

Add:

```rust
pub session: Option<SessionRunOptions>,
pub session_target: Option<ResolvedSessionTarget>,
pub session_name: Option<String>,
```

Keep `PrintModeOptions::new` session-disabled by default.

- [ ] **Step 4: Implement session target resolution**

In `src/session.rs`, add:

```rust
pub enum ResolvedSessionTarget {
    New,
    ContinueMostRecent,
    OpenTarget(String),
    OpenOrCreateId(String),
    ForkTarget(String),
}

pub struct ActiveSession {
    pub storage: pi_agent_core::session::JsonlSessionStorage,
    pub baseline_messages: usize,
}

pub fn open_active_session(
    target: ResolvedSessionTarget,
    options: &SessionRunOptions,
) -> Result<ActiveSession, CliError>;
```

`open_active_session()` creates a `JsonlSessionRepo`, resolves the target, builds context, and
returns baseline length. It does not run the agent.

- [ ] **Step 5: Persist after the loop**

In `run_print_mode`:

1. If session is enabled, call `open_active_session`.
2. Build `Agent::with_messages(config, session_context.messages)` instead of `Agent::new`.
3. Run the stream exactly as before.
4. Always inspect `agent.messages()` after the stream ends, even on `AgentError`.
5. Append `messages[baseline_messages..]` to the active storage as `message` entries.
6. If `session_name` is set, append a `session_info` entry before prompt messages.

Implement conversion from new `AgentMessage` to `StoredAgentMessage` in `pi-agent-core::session`.
Use the session entry timestamp for user/tool result message timestamps. For assistant messages,
preserve the assistant timestamp if non-zero; otherwise use the entry timestamp converted to Unix
milliseconds.

- [ ] **Step 6: Run print-mode session tests**

Run:

```bash
cargo test -p pi-coding-agent --test session_print_mode
cargo test -p pi-coding-agent --test print_mode
```

Expected: PASS.

---

## Task 8: CLI End-to-End Session Flows

**Files:**
- Modify: `crates/pi-coding-agent/src/lib.rs`
- Modify: `crates/pi-coding-agent/src/session.rs`
- Test: `crates/pi-coding-agent/tests/session_cli.rs`

- [ ] **Step 1: Write failing CLI session tests**

Create `crates/pi-coding-agent/tests/session_cli.rs`:

```rust
use pi_ai::providers::faux::{FauxCall, FauxProvider, FauxResponse};
use pi_ai::registry;
use pi_coding_agent::{CliRunOptions, run_cli_with_options};
use std::sync::Arc;

mod support {
    pub use pi_coding_agent::runtime::{SessionMode, SessionRunOptions};
    pub fn text_response(text: &str) -> pi_ai::providers::faux::FauxResponse {
        pi_ai::providers::faux::FauxResponse {
            text_deltas: vec![text.into()],
            thinking_deltas: vec![],
            tool_calls: vec![],
        }
    }
}

#[tokio::test]
async fn continue_uses_previous_session_context() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let sessions = dir.path().join("sessions");

    let api1 = "session-cli-first";
    registry::register(api1, Arc::new(FauxProvider::simple_text("first answer")));
    let options1 = test_options(api1, &cwd, &sessions);
    let first = run_cli_with_options(vec!["-p".into(), "first".into()], options1).await;
    assert_eq!(first.exit_code, 0);
    registry::unregister(api1);

    let api2 = "session-cli-second";
    registry::register(api2, Arc::new(FauxProvider::with_call_queue(vec![FauxCall {
        responses: vec![support::text_response("second answer")],
        stop_reason: pi_ai::types::StopReason::Stop,
    }])));
    let options2 = test_options(api2, &cwd, &sessions);
    let second = run_cli_with_options(vec!["--continue".into(), "-p".into(), "second".into()], options2).await;
    assert_eq!(second.exit_code, 0);
    assert_eq!(second.stdout, "second answer\n");
    registry::unregister(api2);
}
```

Add local `test_options(api, cwd, sessions)` helper using a faux `Model` and
`SessionRunOptions { mode: Enabled, cwd, session_dir: Some(sessions) }`.

Add sibling tests:

- `no_session_does_not_write_files`
- `session_path_appends_to_specific_file`
- `session_id_creates_and_reopens`
- `fork_creates_parent_session_header`
- `name_appends_session_info`

- [ ] **Step 2: Run failing CLI session tests**

Run:

```bash
cargo test -p pi-coding-agent --test session_cli
```

Expected: FAIL until `run_cli_with_options` wires parsed flags into `run_print_mode`.

- [ ] **Step 3: Wire parsed flags to session targets**

In `run_cli_with_options`, after parsing and prompt validation:

- If `parsed.no_session`, pass `session: None`.
- Else if runtime `options.session.mode` is disabled and no session flag is present, pass
  `session: None`.
- Else pass `session: Some(options.session.clone())`.
- Map flags to `ResolvedSessionTarget` in this precedence:
  1. `--fork`
  2. `--session`
  3. `--session-id`
  4. `--continue` or `--resume`
  5. `New`
- Reject multiple target flags except `--session-dir` and `--name`.

- [ ] **Step 4: Preserve current stdout/stderr contract**

Keep final assistant text on stdout and session errors on stderr. Do not print session paths during
successful text mode. This keeps existing print-mode scripting behavior stable.

- [ ] **Step 5: Run CLI session tests and existing tests**

Run:

```bash
cargo test -p pi-coding-agent --test session_cli
cargo test -p pi-coding-agent --test cli
cargo test -p pi-coding-agent --test runtime
cargo test -p pi-coding-agent --test public_api
```

Expected: PASS.

---

## Task 9: Full Verification and Documentation Check

**Files:**
- Review: `docs/superpowers/specs/2026-06-05-pi-agent-session-persistence-design.md`
- Review: `docs/superpowers/plans/2026-06-05-pi-agent-session-persistence.md`
- Optional modify: `pi-rust/ROADMAP.md` only if the user asks for roadmap status updates.

- [ ] **Step 1: Run formatting**

Run:

```bash
cargo fmt --check
```

Expected: PASS.

- [ ] **Step 2: Run focused tests**

Run:

```bash
cargo test -p pi-agent-core
cargo test -p pi-coding-agent
```

Expected: PASS.

- [ ] **Step 3: Run workspace tests**

Run:

```bash
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 4: Run workspace check**

Run:

```bash
cargo check --workspace
```

Expected: PASS.

- [ ] **Step 5: Inspect docs for stale scope**

Run:

```bash
rg -n "T[B]D|T[O]DO|implement[ ]later|fill[ ]in[ ]details|session picker|interactive bridge" docs/superpowers/specs/2026-06-05-pi-agent-session-persistence-design.md docs/superpowers/plans/2026-06-05-pi-agent-session-persistence.md
```

Expected: no placeholder terms. Mentions of interactive bridge are acceptable only in out-of-scope or M6 coordination text.

---

## Parallel M6 Work During M3

These work packages can run while M3 is implemented because they only touch `crates/pi-tui` and do
not modify `pi-coding-agent` session/runtime files.

### M6-P1: Key Parser and Keybindings

**Files:**
- Create: `crates/pi-tui/src/input/mod.rs`
- Create: `crates/pi-tui/src/input/keys.rs`
- Create: `crates/pi-tui/src/input/keybindings.rs`
- Modify: `crates/pi-tui/src/lib.rs`
- Test: `crates/pi-tui/tests/input_keys.rs`
- Test: `crates/pi-tui/tests/keybindings.rs`

Steps:

- [ ] Port typed key ids as a Rust enum/string newtype with normalized display strings such as
  `ctrl+c`, `shift+enter`, `alt+left`.
- [ ] Port legacy escape parsing from `pi/packages/tui/src/keys.ts`: arrows, home/end, page up/down,
  delete, backspace, tab, enter, escape, F1-F12, alt/meta prefix, and printable Unicode.
- [ ] Port Kitty CSI-u parsing for printable codepoints and modifier masks.
- [ ] Port default keybinding definitions from `keybindings.ts`.
- [ ] Add conflict detection tests for duplicate user bindings.
- [ ] Verify with `cargo test -p pi-tui --test input_keys --test keybindings`.

### M6-P2: Stdin Buffer and Bracketed Paste Framing

**Files:**
- Create: `crates/pi-tui/src/input/stdin_buffer.rs`
- Test: `crates/pi-tui/tests/stdin_buffer.rs`

Steps:

- [ ] Port sequence completeness checks for ESC, CSI, OSC, DCS, APC, and SS3.
- [ ] Cover split escape sequences such as `ESC`, then `[<35`, then `;20;5m`.
- [ ] Cover bracketed paste start/end sequences `ESC[200~` and `ESC[201~`.
- [ ] Cover WezTerm/Kitty concatenated escape behavior from the TS comments.
- [ ] Verify with `cargo test -p pi-tui --test stdin_buffer`.

### M6-P3: Single-Line Input Component Foundation

**Files:**
- Create: `crates/pi-tui/src/components/input.rs`
- Create: `crates/pi-tui/src/input/kill_ring.rs`
- Create: `crates/pi-tui/src/input/undo_stack.rs`
- Create: `crates/pi-tui/src/input/word_navigation.rs`
- Modify: `crates/pi-tui/src/components/mod.rs`
- Modify: `crates/pi-tui/src/lib.rs`
- Test: `crates/pi-tui/tests/input_component.rs`

Steps:

- [ ] Implement state-only editing first: value, grapheme cursor, submit/cancel callbacks.
- [ ] Add keybinding-driven movement: left/right, line start/end, word left/right.
- [ ] Add deletion: backspace, delete, delete word backward/forward, delete to line start/end.
- [ ] Add kill/yank/yank-pop and undo stack.
- [ ] Add bracketed paste insertion path.
- [ ] Add width-safe render tests using existing `visible_width` and `truncate_to_width`.
- [ ] Verify with `cargo test -p pi-tui --test input_component`.

### M6-P4: SelectList and Markdown Rendering

**Files:**
- Create: `crates/pi-tui/src/components/select_list.rs`
- Create: `crates/pi-tui/src/components/markdown.rs`
- Modify: `crates/pi-tui/Cargo.toml` - add `pulldown-cmark`.
- Modify: `crates/pi-tui/src/components/mod.rs`
- Modify: `crates/pi-tui/src/lib.rs`
- Test: `crates/pi-tui/tests/select_list.rs`
- Test: `crates/pi-tui/tests/markdown.rs`

Steps:

- [ ] Port `SelectList` item filtering, wrapping movement, page movement, confirm/cancel callbacks,
  and width-aware two-column rendering.
- [ ] Add tests for narrow widths, long labels, descriptions with newlines, no-match state, and scroll
  indicators.
- [ ] Implement Markdown heading, paragraph, list, block quote, code block, inline code, link text,
  bold, italic, strikethrough, and horizontal rule rendering.
- [ ] Add ANSI-aware wrapping tests using existing width utilities.
- [ ] Verify with `cargo test -p pi-tui --test select_list --test markdown`.

### M6 Work To Defer Until After M3

- `pi-coding-agent` interactive runtime bridge.
- Session selector UI and resume picker.
- Focus manager, overlay/modal stack, and async render loop integration.
- Import/export session commands.

These pieces need the M3 session manager semantics or touch files that M3 changes, so they should
start after M3 lands.
