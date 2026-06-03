# pi-ai Rust PoC Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a `pi-ai` Rust crate with `stream()`/`complete()` over a provider abstraction, plus Anthropic + faux providers, fully verified via offline tests.

**Architecture:** Pull-based async streaming via `Pin<Box<dyn Stream>>` built with `async-stream`. Provider trait with global registry keyed by `api`. Anthropic provider splits network (thin reqwest shim) from processing (testable core consuming `Stream<Bytes>`). All serde types use `#[serde(tag = "...")]` for pi-compatible JSON wire format.

**Tech Stack:** Rust 1.96.0, edition 2024, tokio, async-stream, reqwest, serde/serde_json, bytes, tokio-util, thiserror.

---

### Task 1: Cargo.toml — Declare dependencies

**Files:**
- Modify: `crates/pi-ai/Cargo.toml`

- [ ] **Step 1: Replace Cargo.toml contents**

```toml
[package]
name = "pi-ai"
version = "0.1.0"
edition = "2024"

[dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync"] }
futures = "0.3"
async-stream = "0.3"
reqwest = { version = "0.12", default-features = false, features = ["json", "stream", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
bytes = "1"
tokio-util = "0.7"
thiserror = "2"

[dev-dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "fs"] }
```

- [ ] **Step 2: Verify dependency resolution**

Run: `cargo check -p pi-ai 2>&1 | head -20`
Expected: resolves dependencies; compile errors from old `lib.rs` are expected and will be fixed in later tasks.

---

### Task 2: Core types — ContentBlock, Message, Usage, StopReason, AssistantMessage, AssistantMessageEvent, Context, Tool, Model, StreamOptions, ThinkingConfig

**Files:**
- Create: `crates/pi-ai/src/types.rs`
- Modify: `crates/pi-ai/src/lib.rs` (remove old placeholder)

**What to build:** Define all shared types with `#[serde(tag = "...")]` / `#[serde(rename = "...")]` to produce pi-compatible JSON (camelCase wire names, snake_case Rust fields).

- [ ] **Step 1: Write types.rs with all type definitions and serde unit tests**

Complete file at `crates/pi-ai/src/types.rs`:

```rust
use serde::{Deserialize, Serialize};

// ── Content blocks ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        text_signature: Option<String>,
    },
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        thinking_signature: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        redacted: Option<bool>,
    },
    #[serde(rename = "image")]
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    #[serde(rename = "toolCall")]
    ToolCall {
        id: String,
        name: String,
        arguments: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        thought_signature: Option<String>,
    },
}

// ── Messages ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "role")]
pub enum Message {
    #[serde(rename = "user")]
    User {
        content: Vec<ContentBlock>,
    },
    #[serde(rename = "assistant")]
    Assistant {
        content: Vec<ContentBlock>,
    },
    #[serde(rename = "toolResult")]
    ToolResult {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        content: Vec<ContentBlock>,
    },
}

// ── Usage & cost ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Cost {
    pub input: f64,
    pub output: f64,
    #[serde(rename = "cacheRead")]
    pub cache_read: f64,
    #[serde(rename = "cacheWrite")]
    pub cache_write: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Usage {
    pub input: u32,
    pub output: u32,
    #[serde(rename = "cacheRead")]
    pub cache_read: u32,
    #[serde(rename = "cacheWrite")]
    pub cache_write: u32,
    #[serde(rename = "totalTokens")]
    pub total_tokens: u32,
    pub cost: Cost,
}

// ── Stop reason ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StopReason {
    Stop,
    Length,
    #[serde(rename = "toolUse")]
    ToolUse,
    Error,
    Aborted,
}

// ── Assistant message (response-side) ───────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssistantMessage {
    pub content: Vec<ContentBlock>,
    pub api: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    pub model: String,
    #[serde(rename = "responseModel", skip_serializing_if = "Option::is_none")]
    pub response_model: Option<String>,
    #[serde(rename = "responseId", skip_serializing_if = "Option::is_none")]
    pub response_id: Option<String>,
    pub usage: Usage,
    #[serde(rename = "stopReason")]
    pub stop_reason: StopReason,
    #[serde(rename = "errorMessage", skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub timestamp: u64,
}

impl AssistantMessage {
    pub fn empty(api: &str, model: &str) -> Self {
        Self {
            content: Vec::new(),
            api: api.to_string(),
            provider: None,
            model: model.to_string(),
            response_model: None,
            response_id: None,
            usage: Usage::default(),
            stop_reason: StopReason::Stop,
            error_message: None,
            timestamp: 0,
        }
    }
}

// ── Streaming events ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum AssistantMessageEvent {
    #[serde(rename = "start")]
    Start { partial: AssistantMessage },
    #[serde(rename = "textStart")]
    TextStart { partial: AssistantMessage },
    #[serde(rename = "textDelta")]
    TextDelta { delta: String, partial: AssistantMessage },
    #[serde(rename = "textEnd")]
    TextEnd { partial: AssistantMessage },
    #[serde(rename = "thinkingStart")]
    ThinkingStart { partial: AssistantMessage },
    #[serde(rename = "thinkingDelta")]
    ThinkingDelta { delta: String, partial: AssistantMessage },
    #[serde(rename = "thinkingEnd")]
    ThinkingEnd { partial: AssistantMessage },
    #[serde(rename = "toolcallStart")]
    ToolcallStart { partial: AssistantMessage },
    #[serde(rename = "toolcallDelta")]
    ToolcallDelta { delta: String, partial: AssistantMessage },
    #[serde(rename = "toolcallEnd")]
    ToolcallEnd { partial: AssistantMessage },
    #[serde(rename = "done")]
    Done {
        reason: StopReason,
        message: AssistantMessage,
    },
    #[serde(rename = "error")]
    Error {
        reason: StopReason,
        error: String,
    },
}

// ── Context, tools, models ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Context {
    #[serde(rename = "systemPrompt", skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub api: String,
    pub provider: String,
    #[serde(rename = "baseUrl")]
    pub base_url: String,
    pub reasoning: bool,
    pub input: f64,
    pub output: f64,
    #[serde(rename = "cacheRead", skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<f64>,
    #[serde(rename = "cacheWrite", skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<f64>,
    #[serde(rename = "contextWindow")]
    pub context_window: u32,
    #[serde(rename = "maxTokens", skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StreamOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(rename = "maxTokens", skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(rename = "apiKey", skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(rename = "cacheRetention", skip_serializing_if = "Option::is_none")]
    pub cache_retention: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,
    #[serde(rename = "toolChoice", skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<serde_json::Value>,
    #[serde(skip)]
    pub cancel: Option<tokio_util::sync::CancellationToken>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThinkingConfig {
    pub enabled: bool,
    #[serde(rename = "budgetTokens", skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
}

// ── Tests: serde roundtrip ──────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_block_text_roundtrip() {
        let cb = ContentBlock::Text { text: "hello".into(), text_signature: None };
        let json = serde_json::to_string(&cb).unwrap();
        assert_eq!(json, r#"{"type":"text","text":"hello"}"#);
        let back: ContentBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cb);
    }

    #[test]
    fn content_block_toolcall_roundtrip() {
        let cb = ContentBlock::ToolCall {
            id: "toolu_01".into(), name: "read".into(),
            arguments: serde_json::json!({"path": "/x"}), thought_signature: None,
        };
        let json = serde_json::to_string(&cb).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["type"], "toolCall");
        assert_eq!(parsed["id"], "toolu_01");
        let back: ContentBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cb);
    }

    #[test]
    fn message_user_roundtrip() {
        let msg = Message::User { content: vec![ContentBlock::Text { text: "hi".into(), text_signature: None }] };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""role":"user""#));
        let back: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn message_tool_result_roundtrip() {
        let msg = Message::ToolResult {
            tool_call_id: "call_1".into(),
            content: vec![ContentBlock::Text { text: "ok".into(), text_signature: None }],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""toolCallId":"call_1""#));
        let back: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn event_done_roundtrip() {
        let ev = AssistantMessageEvent::Done {
            reason: StopReason::Stop,
            message: AssistantMessage::empty("anthropic-messages", "claude-sonnet-4-5"),
        };
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains(r#""type":"done""#));
        let back: AssistantMessageEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, AssistantMessageEvent::Done { .. }));
    }

    #[test]
    fn event_error_roundtrip() {
        let ev = AssistantMessageEvent::Error { reason: StopReason::Error, error: "fail".into() };
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains(r#""type":"error""#));
    }

    #[test]
    fn stop_reason_serde() {
        assert_eq!(serde_json::to_string(&StopReason::Stop).unwrap(), r#""stop""#);
        assert_eq!(serde_json::to_string(&StopReason::ToolUse).unwrap(), r#""toolUse""#);
        let sr: StopReason = serde_json::from_str(r#""toolUse""#).unwrap();
        assert_eq!(sr, StopReason::ToolUse);
    }

    #[test]
    fn model_serde_camelcase() {
        let m = Model {
            id: "claude-sonnet-4-5".into(), name: "Claude Sonnet 4.5".into(),
            api: "anthropic-messages".into(), provider: "anthropic".into(),
            base_url: "https://api.anthropic.com".into(), reasoning: true,
            input: 3.0, output: 15.0, cache_read: None, cache_write: None,
            context_window: 200000, max_tokens: Some(8192), headers: None,
        };
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains(r#""baseUrl""#));
        assert!(json.contains(r#""contextWindow""#));
        assert!(json.contains(r#""maxTokens""#));
    }
}
```

- [ ] **Step 2: Replace lib.rs with a minimal module declaration**

Replace `crates/pi-ai/src/lib.rs` with:

```rust
pub mod types;
```

- [ ] **Step 3: Verify compilation and tests**

Run: `cargo test -p pi-ai -- --nocapture`
Expected: 8 tests pass (all serde roundtrips).

- [ ] **Step 4: Commit**

```bash
git add crates/pi-ai/Cargo.toml crates/pi-ai/src/
git commit -m "feat(pi-ai): add core types with pi-compatible serde"
```

---

### Task 3: Utility modules — env_keys and json_repair

**Files:**
- Create: `crates/pi-ai/src/util/mod.rs`
- Create: `crates/pi-ai/src/util/env_keys.rs`
- Create: `crates/pi-ai/src/util/json_repair.rs`
- Modify: `crates/pi-ai/src/lib.rs` (add `pub mod util;`)

**Env keys** resolves `ANTHROPIC_API_KEY` and common aliases from the environment.  
**JSON repair** fixes broken JSON strings (unescaped control chars, bad escapes) and provides partial-parse for incomplete JSON fragments (used for incremental tool-call arguments).

- [ ] **Step 1: Write util/mod.rs**

```rust
pub mod env_keys;
pub mod json_repair;
```

- [ ] **Step 2: Write util/env_keys.rs**

```rust
/// Resolves an API key from the environment for the given provider.
/// For "anthropic", checks ANTHROPIC_API_KEY plus common aliases.
pub fn env_api_key(provider: &str) -> Option<String> {
    let vars = match provider {
        "anthropic" => &["ANTHROPIC_API_KEY", "CLAUDE_API_KEY", "ANTHROPIC_KEY"][..],
        _ => &[],
    };
    for var in vars {
        if let Ok(val) = std::env::var(var) {
            if !val.is_empty() {
                return Some(val);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_none_when_not_set() {
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("CLAUDE_API_KEY");
        assert_eq!(env_api_key("anthropic"), None);
    }

    #[test]
    fn returns_anthropic_key() {
        std::env::set_var("ANTHROPIC_API_KEY", "sk-ant-test");
        assert_eq!(env_api_key("anthropic"), Some("sk-ant-test".into()));
        std::env::remove_var("ANTHROPIC_API_KEY");
    }

    #[test]
    fn returns_none_for_unknown_provider() {
        assert_eq!(env_api_key("nonexistent"), None);
    }
}
```

- [ ] **Step 3: Write util/json_repair.rs**

```rust
/// Repairs common JSON formatting issues: escapes raw control characters
/// (0x00-0x1F, excluding \t \n \r) and fixes invalid escape sequences.
pub fn repair_json(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                out.push('\\');
                if let Some(&next) = chars.peek() {
                    if next == '\\' || next == '"' || next == '/' || next == 'b'
                        || next == 'f' || next == 'n' || next == 'r' || next == 't'
                        || next == 'u'
                    {
                        // valid escape, keep it
                    } else {
                        // invalid escape, double-escape
                        out.push('\\');
                    }
                }
            }
            c if (c as u32) < 0x20 && c != '\t' && c != '\n' && c != '\r' => {
                // raw control char, escape it
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            _ => out.push(c),
        }
    }
    out
}

/// Attempts to parse streaming (possibly incomplete) JSON.
/// 1. Try strict serde_json::from_str
/// 2. Try repair_json then parse
/// 3. Try to close unclosed constructs (strings, arrays, objects)
/// 4. Fall back to empty object
pub fn parse_streaming_json(input: &str) -> serde_json::Value {
    if let Ok(v) = serde_json::from_str(input) {
        return v;
    }
    let repaired = repair_json(input);
    if let Ok(v) = serde_json::from_str(&repaired) {
        return v;
    }
    if let Ok(v) = serde_json::from_str(&close_incomplete(&repaired)) {
        return v;
    }
    serde_json::Value::Object(serde_json::Map::new())
}

/// Appends closing characters to make incomplete JSON parseable.
fn close_incomplete(s: &str) -> String {
    let mut out = s.to_string();
    let mut stack: Vec<char> = Vec::new();
    let mut in_string = false;
    let mut escaped = false;

    for c in s.chars() {
        if escaped { escaped = false; continue; }
        if c == '\\' && in_string { escaped = true; continue; }
        match c {
            '"' => in_string = !in_string,
            '{' if !in_string => stack.push('}'),
            '[' if !in_string => stack.push(']'),
            '}' | ']' if !in_string => { stack.pop(); }
            _ => {}
        }
    }
    if in_string { out.push('"'); }
    while let Some(bracket) = stack.pop() { out.push(bracket); }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_escapes_control_chars() {
        let input = "hello\x01world";
        let repaired = repair_json(input);
        assert!(!repaired.contains('\x01'));
        assert!(repaired.contains("\\u0001"));
    }

    #[test]
    fn repair_fixes_bad_backslash() {
        let input = r#"{"key": "val\x"}"#;
        let repaired = repair_json(input);
        assert!(repaired.contains(r#"\\x"#));
    }

    #[test]
    fn parse_valid_json() {
        let v = parse_streaming_json(r#"{"a": 1}"#);
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn parse_truncated_object() {
        let v = parse_streaming_json(r#"{"a": 1, "b": {"#);
        assert!(v.is_object());
    }

    #[test]
    fn parse_truncated_array() {
        let v = parse_streaming_json(r#"[1, 2, {"#);
        assert!(v.is_array());
    }

    #[test]
    fn parse_garbage_returns_empty_object() {
        let v = parse_streaming_json("not json at all!!!");
        assert!(v.is_object());
        assert!(v.as_object().unwrap().is_empty());
    }
}
```

- [ ] **Step 4: Update lib.rs**

Replace `crates/pi-ai/src/lib.rs` with:

```rust
pub mod types;
pub mod util;
```

- [ ] **Step 5: Verify tests**

Run: `cargo test -p pi-ai -- --nocapture`
Expected: all tests pass (8 serde + 3 env_keys + 6 json_repair = 17 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/pi-ai/src/
git commit -m "feat(pi-ai): add util modules (env_keys, json_repair)"
```

---

### Task 4: Model table and cost calculation

**Files:**
- Create: `crates/pi-ai/src/models.rs`
- Modify: `crates/pi-ai/src/lib.rs` (add `pub mod models;`)

**What to build:** A static table of current Anthropic models and a `calculate_cost()` function that multiplies token counts by per-million-token rates.

- [ ] **Step 1: Write models.rs**

```rust
use crate::types::{Model, Usage};

/// Static model lookup by id. Returns None for unknown models.
pub fn lookup_model(id: &str) -> Option<Model> {
    all_models().iter().find(|m| m.id == id).cloned()
}

/// Calculate cost for the given usage against a model's rates.
/// Rates are per million tokens. Updates usage.cost in place.
pub fn calculate_cost(model: &Model, usage: &mut Usage) {
    let input_cost = (usage.input as f64 / 1_000_000.0) * model.input;
    let output_cost = (usage.output as f64 / 1_000_000.0) * model.output;
    let cache_read_cost = model.cache_read.map_or(0.0, |rate| {
        (usage.cache_read as f64 / 1_000_000.0) * rate
    });
    let cache_write_cost = model.cache_write.map_or(0.0, |rate| {
        (usage.cache_write as f64 / 1_000_000.0) * rate
    });
    usage.cost.input = input_cost;
    usage.cost.output = output_cost;
    usage.cost.cache_read = cache_read_cost;
    usage.cost.cache_write = cache_write_cost;
}

/// Hand-crafted static model table (subset of Anthropic models).
/// Populated inline via build_models() at first access.
pub fn all_models() -> &'static [Model] {
    use std::sync::LazyLock;
    static MODELS: LazyLock<Vec<Model>> = LazyLock::new(build_models);
    &MODELS
}

fn build_models() -> Vec<Model> {
    fn m(
        id: &str, name: &str, reasoning: bool, input: f64, output: f64,
        cache_read: Option<f64>, cache_write: Option<f64>,
        context_window: u32, max_tokens: u32,
    ) -> Model {
        Model {
            id: id.into(), name: name.into(),
            api: "anthropic-messages".into(), provider: "anthropic".into(),
            base_url: "https://api.anthropic.com".into(),
            reasoning, input, output, cache_read, cache_write,
            context_window, max_tokens: Some(max_tokens), headers: None,
        }
    }
    vec![
        m("claude-sonnet-4-5",      "Claude Sonnet 4.5",       true,  3.0, 15.0, Some(0.30), Some(3.75), 200_000, 8192),
        m("claude-haiku-4-5",       "Claude Haiku 4.5",        false, 1.0, 5.0,  Some(0.10), Some(1.25), 200_000, 8192),
        m("claude-opus-4-5",        "Claude Opus 4.5",         true,  15.0, 75.0, Some(1.50), Some(18.75), 200_000, 8192),
        m("claude-sonnet-4",        "Claude Sonnet 4",         true,  3.0, 15.0, Some(0.30), Some(3.75), 200_000, 8192),
        m("claude-opus-4",          "Claude Opus 4",           true,  15.0, 75.0, Some(1.50), Some(18.75), 200_000, 8192),
        m("claude-3-5-sonnet-latest", "Claude 3.5 Sonnet",     false, 3.0, 15.0, Some(0.30), Some(3.75), 200_000, 8192),
        m("claude-3-5-haiku-latest",  "Claude 3.5 Haiku",      false, 0.80, 4.0, Some(0.08), Some(1.00), 200_000, 8192),
        m("claude-3-opus-latest",     "Claude 3 Opus",         false, 15.0, 75.0, Some(1.50), Some(18.75), 200_000, 4096),
    ]
}
```

- [ ] **Step 2: Update lib.rs — append `pub mod models;`**

Add the line after `pub mod util;`:
```rust
pub mod models;
```

- [ ] **Step 3: Write tests inside models.rs**

Append to the end of models.rs:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_known_model() {
        let m = lookup_model("claude-sonnet-4-5").unwrap();
        assert_eq!(m.id, "claude-sonnet-4-5");
        assert_eq!(m.input, 3.0);
    }

    #[test]
    fn lookup_unknown_model() {
        assert!(lookup_model("nonexistent").is_none());
    }

    #[test]
    fn cost_calculation_basic() {
        let model = lookup_model("claude-haiku-4-5").unwrap();
        let mut usage = Usage {
            input: 1_000_000, output: 500_000,
            cache_read: 0, cache_write: 0, total_tokens: 1_500_000,
            cost: Default::default(),
        };
        calculate_cost(&model, &mut usage);
        assert!((usage.cost.input - 1.0).abs() < 0.001);   // 1M tokens * $1/M
        assert!((usage.cost.output - 2.5).abs() < 0.001);  // 500K tokens * $5/M
    }

    #[test]
    fn cost_calculation_with_cache() {
        let model = lookup_model("claude-sonnet-4-5").unwrap();
        let mut usage = Usage {
            input: 0, output: 0, cache_read: 1_000_000, cache_write: 2_000_000,
            total_tokens: 3_000_000, cost: Default::default(),
        };
        calculate_cost(&model, &mut usage);
        assert!((usage.cost.cache_read - 0.30).abs() < 0.001);   // $0.30/M
        assert!((usage.cost.cache_write - 7.50).abs() < 0.001);  // $3.75/M * 2
    }
}
```

- [ ] **Step 4: Verify tests**

Run: `cargo test -p pi-ai -- --nocapture`
Expected: all tests pass (21 total).

- [ ] **Step 5: Commit**

```bash
git add crates/pi-ai/src/
git commit -m "feat(pi-ai): add model table and cost calculation"
```

---

### Task 5: Streaming model — EventStream type and complete()

**Files:**
- Create: `crates/pi-ai/src/stream.rs`
- Modify: `crates/pi-ai/src/lib.rs` (add `pub mod stream;`)

**What to build:** The `EventStream` type alias (`Pin<Box<dyn Stream<Item = AssistantMessageEvent> + Send>>`) and a `complete()` function that drains the stream to return the final `AssistantMessage` (from the `Done` event).

- [ ] **Step 1: Write stream.rs**

```rust
use std::pin::Pin;
use futures::{Stream, StreamExt};
use crate::types::{AssistantMessage, AssistantMessageEvent, StopReason};

pub type EventStream = Pin<Box<dyn Stream<Item = AssistantMessageEvent> + Send>>;

/// Consumes the stream and returns the final AssistantMessage from the terminal event.
/// Returns an error if the stream ends without a Done event or yields an Error event.
pub async fn complete(mut stream: EventStream) -> Result<AssistantMessage, String> {
    while let Some(event) = stream.next().await {
        match event {
            AssistantMessageEvent::Done { message, .. } => return Ok(message),
            AssistantMessageEvent::Error { error, .. } => return Err(error),
            _ => continue,
        }
    }
    Err("stream ended without Done event".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;
    use crate::types::{ContentBlock, Usage, StopReason};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_event_stream(events: Vec<AssistantMessageEvent>) -> EventStream {
        Box::pin(stream::iter(events))
    }

    fn dummy_message() -> AssistantMessage {
        AssistantMessage {
            content: vec![ContentBlock::Text { text: "ok".into(), text_signature: None }],
            api: "test".into(), provider: None, model: "test".into(),
            response_model: None, response_id: None,
            usage: Usage::default(), stop_reason: StopReason::Stop,
            error_message: None,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        }
    }

    #[tokio::test]
    async fn complete_returns_done_message() {
        let msg = dummy_message();
        let stream = make_event_stream(vec![
            AssistantMessageEvent::Start { partial: msg.clone() },
            AssistantMessageEvent::Done { reason: StopReason::Stop, message: msg.clone() },
        ]);
        let result = complete(stream).await.unwrap();
        assert_eq!(result, msg);
    }

    #[tokio::test]
    async fn complete_returns_error() {
        let stream = make_event_stream(vec![
            AssistantMessageEvent::Error { reason: StopReason::Error, error: "fail".into() },
        ]);
        let result = complete(stream).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "fail");
    }

    #[tokio::test]
    async fn complete_empty_stream_errors() {
        let stream = make_event_stream(vec![]);
        assert!(complete(stream).await.is_err());
    }
}
```

- [ ] **Step 2: Update lib.rs**

Add `pub mod stream;` after existing module declarations.

- [ ] **Step 3: Verify tests**

Run: `cargo test -p pi-ai -- --nocapture`
Expected: 24 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/pi-ai/src/
git commit -m "feat(pi-ai): add EventStream type and complete()"
```

---

### Task 6: Provider registry — trait ApiProvider + global registry

**Files:**
- Create: `crates/pi-ai/src/registry.rs`
- Modify: `crates/pi-ai/src/lib.rs` (add `pub mod registry;`)

**What to build:** The `ApiProvider` trait with a `stream()` method, a global registry (`HashMap<String, Arc<dyn ApiProvider>>` behind `RwLock`), and a top-level `stream()` function that resolves provider by `model.api`.

- [ ] **Step 1: Write registry.rs**

```rust
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};
use async_stream::stream;
use futures::StreamExt;
use crate::types::{
    AssistantMessage, AssistantMessageEvent, Context, Model, StopReason, StreamOptions,
};
use crate::stream::EventStream;

pub trait ApiProvider: Send + Sync {
    fn stream(
        &self,
        model: &Model,
        ctx: Context,
        opts: Option<StreamOptions>,
    ) -> EventStream;
}

static REGISTRY: LazyLock<RwLock<HashMap<String, Arc<dyn ApiProvider>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub fn register(api: &str, provider: Arc<dyn ApiProvider>) {
    REGISTRY.write().unwrap().insert(api.to_string(), provider);
}

pub fn unregister(api: &str) {
    REGISTRY.write().unwrap().remove(api);
}

pub fn lookup(api: &str) -> Option<Arc<dyn ApiProvider>> {
    REGISTRY.read().unwrap().get(api).cloned()
}

/// Top-level entry point: resolves provider by model.api, injects env API key
/// if not provided, delegates to provider.stream(). Returns a stream that
/// immediately yields Error on unknown api.
pub fn stream_model(
    model: &Model,
    ctx: Context,
    mut opts: Option<StreamOptions>,
) -> EventStream {
    let provider = match lookup(&model.api) {
        Some(p) => p,
        None => {
            return Box::pin(stream! {
                yield AssistantMessageEvent::Error {
                    reason: StopReason::Error,
                    error: format!("unknown provider api: {}", model.api),
                };
            });
        }
    };

    if let Some(ref mut o) = opts {
        if o.api_key.is_none() {
            o.api_key = crate::util::env_keys::env_api_key(&model.provider);
        }
    }

    provider.stream(model, ctx, opts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct DummyProvider;
    impl ApiProvider for DummyProvider {
        fn stream(&self, _model: &Model, _ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
            Box::pin(stream! {
                let mut msg = AssistantMessage::empty("dummy", "dummy");
                msg.content.push(crate::types::ContentBlock::Text {
                    text: "dummy response".into(), text_signature: None,
                });
                yield AssistantMessageEvent::Done { reason: StopReason::Stop, message: msg };
            })
        }
    }

    #[tokio::test]
    async fn registry_register_and_lookup() {
        register("test-api", Arc::new(DummyProvider));
        let found = lookup("test-api");
        assert!(found.is_some());
        unregister("test-api");
        assert!(lookup("test-api").is_none());
    }

    #[tokio::test]
    async fn stream_model_unknown_api_returns_error() {
        let model = Model {
            id: "x".into(), name: "x".into(), api: "nonexistent".into(),
            provider: "none".into(), base_url: "".into(), reasoning: false,
            input: 0.0, output: 0.0, cache_read: None, cache_write: None,
            context_window: 0, max_tokens: None, headers: None,
        };
        let mut stream = stream_model(&model, Context { system_prompt: None, messages: vec![], tools: None }, None);
        let event = stream.next().await.unwrap();
        assert!(matches!(event, AssistantMessageEvent::Error { .. }));
    }

    #[tokio::test]
    async fn stream_model_delegates_to_provider() {
        register("test-api", Arc::new(DummyProvider));
        let model = Model {
            id: "x".into(), name: "x".into(), api: "test-api".into(),
            provider: "test".into(), base_url: "".into(), reasoning: false,
            input: 0.0, output: 0.0, cache_read: None, cache_write: None,
            context_window: 0, max_tokens: None, headers: None,
        };
        let mut stream = stream_model(&model, Context { system_prompt: None, messages: vec![], tools: None }, None);
        let event = stream.next().await.unwrap();
        assert!(matches!(event, AssistantMessageEvent::Done { .. }));
        unregister("test-api");
    }
}
```

- [ ] **Step 2: Update lib.rs**

Add `pub mod registry;`.

- [ ] **Step 3: Verify tests**

Run: `cargo test -p pi-ai -- --nocapture`
Expected: 27 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/pi-ai/src/
git commit -m "feat(pi-ai): add provider trait and registry"
```

---

### Task 7: Anthropic SSE decoder

**Files:**
- Create directory: `crates/pi-ai/src/providers/`
- Create directory: `crates/pi-ai/src/providers/anthropic/`
- Create: `crates/pi-ai/src/providers/anthropic/sse.rs`

**What to build:** An SSE (Server-Sent Events) line decoder that processes `Stream<Bytes>` into `ServerSentEvent` structs. Handles `\n`/`\r\n`, `:`-comment lines, multi-line `data`, and events split across chunk boundaries. Ported from the TS `pi-ai` SSE decoder.

- [ ] **Step 1: Write sse.rs**

```rust
use bytes::Bytes;
use futures::{Stream, StreamExt};
use async_stream::stream;

#[derive(Debug, Clone, PartialEq)]
pub struct ServerSentEvent {
    pub event: Option<String>,
    pub data: String,
}

/// Process raw SSE chunk and return any complete events found.
/// Maintains `buf` as leftover incomplete line for the next call.
pub fn process_chunk(chunk: &[u8], buf: &mut Vec<u8>) -> Vec<ServerSentEvent> {
    buf.extend_from_slice(chunk);
    let mut events = Vec::new();
    let mut event_type: Option<String> = None;
    let mut data_parts: Vec<String> = Vec::new();

    loop {
        let pos = buf.iter().position(|&b| b == b'\n');
        let line_end = match pos {
            Some(p) => p,
            None => break,
        };
        let mut line = buf.drain(..=line_end).collect::<Vec<u8>>();
        // trim trailing \r if present
        if line.ends_with(&[b'\r', b'\n']) {
            line.truncate(line.len() - 2);
        } else if line.ends_with(&[b'\n']) {
            line.pop();
        }

        let line_str = String::from_utf8_lossy(&line);
        let trimmed = line_str.trim_end_matches('\r');

        if trimmed.is_empty() {
            // empty line = event dispatch
            if !data_parts.is_empty() {
                events.push(ServerSentEvent {
                    event: event_type.take(),
                    data: data_parts.join(""),
                });
                data_parts.clear();
            }
        } else if let Some(rest) = trimmed.strip_prefix(':') {
            // comment line, ignore
            let _ = rest;
        } else if let Some(rest) = trimmed.strip_prefix("event:") {
            event_type = Some(rest.trim().to_string());
        } else if let Some(rest) = trimmed.strip_prefix("data:") {
            data_parts.push(rest.to_string());
        } else {
            // unknown field, ignore per SSE spec
        }
    }
    events
}

/// Convert a Stream<Bytes> (from reqwest) into a Stream<ServerSentEvent>.
pub fn iterate_sse<E>(
    body: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
) -> impl Stream<Item = Result<ServerSentEvent, String>> + Send
where
    E: std::fmt::Display + Send + 'static,
{
    let mut buf: Vec<u8> = Vec::new();
    stream! {
        futures::pin_mut!(body);
        while let Some(chunk_result) = body.next().await {
            let chunk = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    yield Err(format!("SSE read error: {}", e));
                    return;
                }
            };
            for event in process_chunk(&chunk, &mut buf) {
                yield Ok(event);
            }
        }
        // flush remaining buffer
        if !buf.is_empty() {
            // try to process any trailing incomplete data
            for event in process_chunk(&[], &mut buf) {
                yield Ok(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;

    #[test]
    fn basic_sse_event() {
        let input = b"data: {\"hello\":\"world\"}\n\n";
        let mut buf = Vec::new();
        let events = process_chunk(input, &mut buf);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, None);
        assert_eq!(events[0].data, "{\"hello\":\"world\"}");
    }

    #[test]
    fn event_with_type() {
        let input = b"event: message_start\ndata: {\"type\":\"x\"}\n\n";
        let mut buf = Vec::new();
        let events = process_chunk(input, &mut buf);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event.as_deref(), Some("message_start"));
        assert_eq!(events[0].data, "{\"type\":\"x\"}");
    }

    #[test]
    fn multi_line_data() {
        let input = b"data: line1\ndata: line2\n\n";
        let mut buf = Vec::new();
        let events = process_chunk(input, &mut buf);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "line1line2");
    }

    #[test]
    fn comment_lines_ignored() {
        let input = b": this is a comment\ndata: real\n\n";
        let mut buf = Vec::new();
        let events = process_chunk(input, &mut buf);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "real");
    }

    #[test]
    fn crlf_line_endings() {
        let input = b"data: foo\r\n\r\n";
        let mut buf = Vec::new();
        let events = process_chunk(input, &mut buf);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "foo");
    }

    #[test]
    fn event_split_across_chunks() {
        let mut buf = Vec::new();
        let events1 = process_chunk(b"data: hel", &mut buf);
        assert!(events1.is_empty()); // incomplete

        let events2 = process_chunk(b"lo world\n\n", &mut buf);
        assert_eq!(events2.len(), 1);
        assert_eq!(events2[0].data, "hello world");
    }

    #[tokio::test]
    async fn iterate_sse_from_stream() {
        let body = stream::iter(vec![
            Ok::<_, String>(Bytes::from("data: chunk1\n\n")),
            Ok(Bytes::from("data: chunk2\n\n")),
        ]);
        let results: Vec<_> = iterate_sse(body).collect().await;
        let events: Vec<_> = results.into_iter().map(|r| r.unwrap()).collect();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].data, "chunk1");
        assert_eq!(events[1].data, "chunk2");
    }
}
```

- [ ] **Step 2: Verify tests**

Run: `cargo test -p pi-ai -- providers::anthropic::sse --nocapture`
Expected: 7 SSE tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/pi-ai/src/
git commit -m "feat(pi-ai): add Anthropic SSE decoder"
```

---

### Task 8: Anthropic wire types

**Files:**
- Create: `crates/pi-ai/src/providers/anthropic/wire.rs`

**What to build:** Serde structs for the Anthropic HTTP request body and SSE stream event shapes. Maps to the Anthropic Messages API wire format (`message_start`, `content_block_start`, `content_block_delta`, `content_block_stop`, `message_delta`, `message_stop`, `ping`).

- [ ] **Step 1: Write wire.rs**

```rust
use serde::{Deserialize, Serialize};

// ── Request types ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub model: String,
    #[serde(rename = "max_tokens")]
    pub max_tokens: u32,
    pub messages: Vec<RequestMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<Vec<SystemBlock>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    pub stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMessage {
    pub role: String,
    pub content: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheControl {
    #[serde(rename = "type")]
    pub cache_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    #[serde(rename = "type")]
    pub think_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<u32>,
}

// ── SSE stream event types ──────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: MessageStart },
    #[serde(rename = "content_block_start")]
    ContentBlockStart { index: u32, content_block: ContentBlockStart },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { index: u32, delta: ContentBlockDelta },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: u32 },
    #[serde(rename = "message_delta")]
    MessageDelta { delta: MessageDelta, usage: MessageUsage },
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(rename = "ping")]
    Ping,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageStart {
    pub message: MessageInfo,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub role: String,
    pub model: String,
    pub usage: MessageUsage,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct MessageUsage {
    #[serde(default)]
    pub input_tokens: u32,
    #[serde(default)]
    pub output_tokens: u32,
    #[serde(default)]
    pub cache_read_input_tokens: Option<u32>,
    #[serde(default)]
    pub cache_creation_input_tokens: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlockStart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
    #[serde(rename = "redacted_thinking")]
    RedactedThinking { data: String },
    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlockDelta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "thinking_delta")]
    ThinkingDelta { thinking: String },
    #[serde(rename = "signature_delta")]
    SignatureDelta { signature: String },
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageDelta {
    pub stop_reason: Option<String>,
    #[serde(rename = "stop_sequence")]
    pub stop_sequence: Option<String>,
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p pi-ai 2>&1`
Expected: compile succeeds (wire types are unused but valid).

- [ ] **Step 3: Commit**

```bash
git add crates/pi-ai/src/
git commit -m "feat(pi-ai): add Anthropic wire types (request + stream events)"
```

---

### Task 9: Anthropic request conversion

**Files:**
- Create: `crates/pi-ai/src/providers/anthropic/convert.rs`

**What to build:** Converts `Context` -> Anthropic `Request` JSON. Includes: system prompt with optional `cache_control`, message conversion with consecutive tool-result coalescing, tools mapped to `input_schema`, `max_tokens` with model fallback, temperature gating, thinking config, `tool_choice`, tool-call id normalization.

- [ ] **Step 1: Write convert.rs**

```rust
use crate::types::{ContentBlock, Context, Message, Model, StreamOptions, Tool};
use super::wire;

/// Normalize a tool-call id to match Anthropic's `^[a-zA-Z0-9_-]{1,64}$`.
/// If the id is already valid, return as-is. Otherwise sanitize and truncate.
pub fn normalize_tool_call_id(id: &str) -> String {
    let is_valid = !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if is_valid {
        return id.to_string();
    }
    let sanitized: String = id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    if sanitized.len() > 64 {
        sanitized[..64].to_string()
    } else if sanitized.is_empty() {
        "tool_0".to_string()
    } else {
        sanitized
    }
}

/// Map pi stop-reason string to our StopReason enum.
pub fn map_stop_reason(s: &str) -> crate::types::StopReason {
    match s {
        "end_turn" => crate::types::StopReason::Stop,
        "max_tokens" => crate::types::StopReason::Length,
        "tool_use" => crate::types::StopReason::ToolUse,
        _ => crate::types::StopReason::Error,
    }
}

/// Convert a Context to an Anthropic Request.
pub fn build_request(
    model: &Model,
    ctx: &Context,
    opts: &Option<StreamOptions>,
) -> wire::Request {
    let max_tokens = opts
        .as_ref()
        .and_then(|o| o.max_tokens)
        .or(model.max_tokens)
        .unwrap_or(4096);

    let system = ctx.system_prompt.as_ref().map(|sp| {
        vec![wire::SystemBlock {
            block_type: "text".into(),
            text: sp.clone(),
            cache_control: Some(wire::CacheControl { cache_type: "ephemeral".into() }),
        }]
    });

    let messages = convert_messages(&ctx.messages);

    let tools = ctx.tools.as_ref().map(|tools| {
        tools.iter().map(|t| wire::ToolDef {
            name: t.name.clone(),
            description: t.description.clone(),
            input_schema: t.parameters.clone(),
        }).collect()
    });

    let temperature = opts.as_ref().and_then(|o| o.temperature);

    let thinking = opts.as_ref().and_then(|o| {
        o.thinking.as_ref().filter(|t| t.enabled).map(|t| {
            wire::ThinkingConfig {
                think_type: if t.budget_tokens.is_some() { "enabled".into() } else { "auto".into() },
                budget_tokens: t.budget_tokens,
            }
        })
    });

    let tool_choice = opts.as_ref().and_then(|o| o.tool_choice.clone());

    wire::Request {
        model: model.id.clone(),
        max_tokens,
        messages,
        system,
        tools,
        temperature,
        thinking,
        tool_choice,
        stream: true,
    }
}

/// Convert pi Messages to Anthropic request messages.
/// Handles consecutive ToolResult coalescing into a single user turn.
fn convert_messages(messages: &[Message]) -> Vec<wire::RequestMessage> {
    let mut result: Vec<wire::RequestMessage> = Vec::new();

    for msg in messages {
        match msg {
            Message::User { content } => {
                result.push(wire::RequestMessage {
                    role: "user".into(),
                    content: convert_content(content),
                });
            }
            Message::Assistant { content } => {
                result.push(wire::RequestMessage {
                    role: "assistant".into(),
                    content: convert_content(content),
                });
            }
            Message::ToolResult { tool_call_id, content } => {
                let tool_content = serde_json::json!({
                    "type": "tool_result",
                    "tool_use_id": normalize_tool_call_id(tool_call_id),
                    "content": convert_content(content),
                });

                // Coalesce: if the last message is also a user-role, append
                // the tool_result to its content array; otherwise push a new user message.
                if let Some(last) = result.last_mut() {
                    if last.role == "user" {
                        if let Some(arr) = last.content.as_array_mut() {
                            arr.push(tool_content);
                            continue;
                        }
                    }
                }
                result.push(wire::RequestMessage {
                    role: "user".into(),
                    content: serde_json::json!([tool_content]),
                });
            }
        }
    }

    result
}

/// Convert pi ContentBlocks to Anthropic-compatible JSON array.
fn convert_content(blocks: &[ContentBlock]) -> serde_json::Value {
    let items: Vec<serde_json::Value> = blocks.iter().map(|b| match b {
        ContentBlock::Text { text, .. } => {
            serde_json::json!({ "type": "text", "text": text })
        }
        ContentBlock::Thinking { thinking, .. } => {
            serde_json::json!({ "type": "thinking", "thinking": thinking })
        }
        ContentBlock::Image { data, mime_type } => {
            serde_json::json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": mime_type,
                    "data": data,
                }
            })
        }
        ContentBlock::ToolCall { id, name, arguments, .. } => {
            serde_json::json!({
                "type": "tool_use",
                "id": normalize_tool_call_id(id),
                "name": name,
                "input": arguments,
            })
        }
    }).collect();
    serde_json::Value::Array(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_valid_id_passes_through() {
        assert_eq!(normalize_tool_call_id("toolu_01"), "toolu_01");
        assert_eq!(normalize_tool_call_id("call-abc-123"), "call-abc-123");
    }

    #[test]
    fn normalize_invalid_id_sanitized() {
        let result = normalize_tool_call_id("tool*use!001");
        assert!(!result.contains('*'));
        assert!(!result.contains('!'));
    }

    #[test]
    fn map_stop_reason_end_turn() {
        assert_eq!(map_stop_reason("end_turn"), crate::types::StopReason::Stop);
    }

    #[test]
    fn map_stop_reason_tool_use() {
        assert_eq!(map_stop_reason("tool_use"), crate::types::StopReason::ToolUse);
    }

    #[test]
    fn map_stop_reason_max_tokens() {
        assert_eq!(map_stop_reason("max_tokens"), crate::types::StopReason::Length);
    }

    #[test]
    fn map_stop_reason_unknown() {
        assert_eq!(map_stop_reason("weird_reason"), crate::types::StopReason::Error);
    }

    #[test]
    fn build_basic_request() {
        let model = Model {
            id: "claude-haiku-4-5".into(), name: "Haiku".into(),
            api: "anthropic-messages".into(), provider: "anthropic".into(),
            base_url: "https://api.anthropic.com".into(), reasoning: false,
            input: 1.0, output: 5.0, cache_read: None, cache_write: None,
            context_window: 200000, max_tokens: Some(8192), headers: None,
        };
        let ctx = Context {
            system_prompt: Some("Be helpful.".into()),
            messages: vec![Message::User {
                content: vec![ContentBlock::Text { text: "Hello".into(), text_signature: None }],
            }],
            tools: None,
        };
        let req = build_request(&model, &ctx, &None);
        assert_eq!(req.model, "claude-haiku-4-5");
        assert_eq!(req.max_tokens, 8192);
        assert!(req.stream);
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, "user");
        assert!(req.system.is_some());
    }

    #[test]
    fn tool_result_coalescing() {
        let messages = vec![
            Message::ToolResult {
                tool_call_id: "call_1".into(),
                content: vec![ContentBlock::Text { text: "result1".into(), text_signature: None }],
            },
            Message::ToolResult {
                tool_call_id: "call_2".into(),
                content: vec![ContentBlock::Text { text: "result2".into(), text_signature: None }],
            },
        ];
        let converted = convert_messages(&messages);
        assert_eq!(converted.len(), 1); // coalesced into one user message
        assert_eq!(converted[0].role, "user");
        let content = converted[0].content.as_array().unwrap();
        assert_eq!(content.len(), 2);
    }
}
```

- [ ] **Step 2: Verify tests**

Run: `cargo test -p pi-ai -- provider --nocapture`
Expected: SSE tests + convert tests all pass.

- [ ] **Step 3: Commit**

```bash
git add crates/pi-ai/
git commit -m "feat(pi-ai): add Anthropic request conversion"
```

---

### Task 10: Anthropic process core — SSE→EventStream transformation

**Files:**
- Create: `crates/pi-ai/src/providers/anthropic/process.rs`

**What to build:** The testable core. Takes `Stream<Bytes>` (from reqwest), runs SSE decode → wire parse → maps to `AssistantMessageEvent` sequence. Accumulates partial content, usage, cost, stop reason. Yields `Start`, `Text*`/`Thinking*`/`Toolcall*`, and finally `Done`/`Error`. All provider logic here — no reqwest dependency.

- [ ] **Step 1: Write process.rs**

```rust
use bytes::Bytes;
use async_stream::stream;
use futures::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;

use crate::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Cost, Model, StopReason, Usage,
};
use crate::util::json_repair::parse_streaming_json;
use crate::models::calculate_cost;
use super::sse::iterate_sse;
use super::wire;
use super::convert::map_stop_reason;

/// Process an SSE body stream into an EventStream.
/// This is the pure, testable core — no reqwest dependency.
pub fn process<E>(
    body: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: Model,
    cancel: Option<CancellationToken>,
) -> crate::stream::EventStream
where
    E: std::fmt::Display + Send + 'static,
{
    Box::pin(stream! {
        let mut partial = AssistantMessage::empty("anthropic-messages", &model.id);
        let mut current_block_index: u32 = 0;
        let mut block_type: Option<String> = None; // "text", "thinking", "tool_use"
        let mut block_id: Option<String> = None;
        let mut accumulated_text = String::new();
        let mut accumulated_thinking = String::new();
        let mut accumulated_tool_args = String::new();
        let mut pending_text_signature: Option<String> = None;
        let mut pending_thinking_signature: Option<String> = None;
        let mut pending_thought_signature: Option<String> = None;
        let mut message_usage = wire::MessageUsage::default();
        let mut stop_reason: Option<StopReason> = None;
        let mut first_event = true;
        let mut errored = false;

        let sse = iterate_sse(body);
        futures::pin_mut!(sse);

        loop {
            // Check cancellation
            if let Some(ref token) = cancel {
                if token.is_cancelled() {
                    partial.stop_reason = StopReason::Aborted;
                    partial.error_message = Some("cancelled".into());
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Aborted,
                        error: "cancelled".into(),
                    };
                    return;
                }
            }

            let sse_event = match sse.next().await {
                Some(Ok(e)) => e,
                Some(Err(e)) => {
                    partial.stop_reason = StopReason::Error;
                    partial.error_message = Some(e.clone());
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Error,
                        error: e,
                    };
                    return;
                }
                None => break,
            };

            let wire_event: wire::StreamEvent = match serde_json::from_str(&sse_event.data) {
                Ok(v) => v,
                Err(e) => {
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Error,
                        error: format!("SSE parse error: {}", e),
                    };
                    return;
                }
            };

            match wire_event {
                wire::StreamEvent::MessageStart { message } => {
                    partial.response_id = Some(message.message.id);
                    partial.response_model = Some(message.message.model);
                    message_usage = message.message.usage;
                    partial.timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    if first_event {
                        yield AssistantMessageEvent::Start { partial: partial.clone() };
                        first_event = false;
                    }
                }

                wire::StreamEvent::ContentBlockStart { index, content_block } => {
                    current_block_index = index;
                    match content_block {
                        wire::ContentBlockStart::Text { text } => {
                            block_type = Some("text".into());
                            accumulated_text = text;
                            let mut p = partial.clone();
                            p.content.push(ContentBlock::Text {
                                text: text.clone(),
                                text_signature: None,
                            });
                            yield AssistantMessageEvent::TextStart { partial: p };
                            if !accumulated_text.is_empty() {
                                yield AssistantMessageEvent::TextDelta {
                                    delta: text,
                                    partial: partial.clone(),
                                };
                            }
                        }
                        wire::ContentBlockStart::Thinking { thinking } => {
                            block_type = Some("thinking".into());
                            accumulated_thinking = thinking.clone();
                            let mut p = partial.clone();
                            p.content.push(ContentBlock::Thinking {
                                thinking: thinking.clone(),
                                thinking_signature: None,
                                redacted: None,
                            });
                            yield AssistantMessageEvent::ThinkingStart { partial: p };
                            if !accumulated_thinking.is_empty() {
                                yield AssistantMessageEvent::ThinkingDelta {
                                    delta: thinking,
                                    partial: partial.clone(),
                                };
                            }
                        }
                        wire::ContentBlockStart::RedactedThinking { .. } => {
                            block_type = Some("thinking".into());
                            let mut p = partial.clone();
                            p.content.push(ContentBlock::Thinking {
                                thinking: String::new(),
                                thinking_signature: None,
                                redacted: Some(true),
                            });
                            yield AssistantMessageEvent::ThinkingStart { partial: p };
                        }
                        wire::ContentBlockStart::ToolUse { id, name } => {
                            block_type = Some("tool_use".into());
                            block_id = Some(id.clone());
                            accumulated_tool_args.clear();
                            let mut p = partial.clone();
                            p.content.push(ContentBlock::ToolCall {
                                id: id.clone(),
                                name: name.clone(),
                                arguments: serde_json::json!({}),
                                thought_signature: None,
                            });
                            yield AssistantMessageEvent::ToolcallStart { partial: p };
                        }
                    }
                }

                wire::StreamEvent::ContentBlockDelta { index: _, delta } => {
                    match delta {
                        wire::ContentBlockDelta::TextDelta { text } => {
                            // Update the content block in partial
                            if let Some(ContentBlock::Text { text: ref mut t, .. }) =
                                partial.content.last_mut()
                            {
                                t.push_str(&text);
                            }
                            accumulated_text.push_str(&text);
                            yield AssistantMessageEvent::TextDelta {
                                delta: text,
                                partial: partial.clone(),
                            };
                        }
                        wire::ContentBlockDelta::ThinkingDelta { thinking } => {
                            if let Some(ContentBlock::Thinking { thinking: ref mut t, .. }) =
                                partial.content.last_mut()
                            {
                                t.push_str(&thinking);
                            }
                            accumulated_thinking.push_str(&thinking);
                            yield AssistantMessageEvent::ThinkingDelta {
                                delta: thinking,
                                partial: partial.clone(),
                            };
                        }
                        wire::ContentBlockDelta::SignatureDelta { signature } => {
                            if block_type.as_deref() == Some("thinking") {
                                pending_thinking_signature = Some(signature);
                            } else {
                                pending_text_signature = Some(signature);
                            }
                        }
                        wire::ContentBlockDelta::InputJsonDelta { partial_json } => {
                            accumulated_tool_args.push_str(&partial_json);
                            let parsed = parse_streaming_json(&accumulated_tool_args);
                            if let Some(ContentBlock::ToolCall { arguments, .. }) =
                                partial.content.last_mut()
                            {
                                *arguments = parsed;
                            }
                            yield AssistantMessageEvent::ToolcallDelta {
                                delta: partial_json,
                                partial: partial.clone(),
                            };
                        }
                    }
                }

                wire::StreamEvent::ContentBlockStop { index: _ } => {
                    match block_type.as_deref() {
                        Some("text") => {
                            if let Some(ContentBlock::Text { text_signature, .. }) =
                                partial.content.last_mut()
                            {
                                *text_signature = pending_text_signature.take();
                            }
                            yield AssistantMessageEvent::TextEnd { partial: partial.clone() };
                        }
                        Some("thinking") => {
                            if let Some(ContentBlock::Thinking { thinking_signature, .. }) =
                                partial.content.last_mut()
                            {
                                *thinking_signature = pending_thinking_signature.take();
                            }
                            yield AssistantMessageEvent::ThinkingEnd { partial: partial.clone() };
                        }
                        Some("tool_use") => {
                            if let Some(ContentBlock::ToolCall { thought_signature, .. }) =
                                partial.content.last_mut()
                            {
                                *thought_signature = pending_thought_signature.take();
                            }
                            yield AssistantMessageEvent::ToolcallEnd { partial: partial.clone() };
                        }
                        _ => {}
                    }
                    block_type = None;
                    block_id = None;
                }

                wire::StreamEvent::MessageDelta { delta, usage } => {
                    stop_reason = delta.stop_reason.as_deref().map(map_stop_reason);
                    message_usage.output_tokens = usage.output_tokens;
                    if let Some(cache_read) = usage.cache_read_input_tokens {
                        message_usage.cache_read_input_tokens = Some(cache_read);
                    }
                    if let Some(cache_write) = usage.cache_creation_input_tokens {
                        message_usage.cache_creation_input_tokens = Some(cache_write);
                    }
                }

                wire::StreamEvent::MessageStop => {
                    // Finalize usage
                    let mut usage = Usage {
                        input: message_usage.input_tokens,
                        output: message_usage.output_tokens,
                        cache_read: message_usage.cache_read_input_tokens.unwrap_or(0),
                        cache_write: message_usage.cache_creation_input_tokens.unwrap_or(0),
                        total_tokens: message_usage.input_tokens + message_usage.output_tokens,
                        cost: Cost::default(),
                    };
                    calculate_cost(&model, &mut usage);

                    partial.usage = usage;
                    partial.stop_reason = stop_reason.unwrap_or(StopReason::Stop);
                    partial.provider = Some("anthropic".into());

                    yield AssistantMessageEvent::Done {
                        reason: partial.stop_reason.clone(),
                        message: partial.clone(),
                    };
                    return;
                }

                wire::StreamEvent::Ping => {
                    // ignore heartbeats
                }
            }
        }

        // Stream ended without MessageStop
        if !errored {
            partial.stop_reason = StopReason::Error;
            partial.error_message = Some("stream ended without message_stop".into());
            yield AssistantMessageEvent::Error {
                reason: StopReason::Error,
                error: "stream ended without message_stop".into(),
            };
        }
    })
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p pi-ai 2>&1`
Expected: compile succeeds.

- [ ] **Step 3: Commit**

```bash
git add crates/pi-ai/src/
git commit -m "feat(pi-ai): add Anthropic process core (SSE→EventStream)"
```

---

### Task 11: Anthropic provider module — ApiProvider impl

**Files:**
- Create: `crates/pi-ai/src/providers/anthropic/mod.rs`

**What to build:** The `ApiProvider` implementation for Anthropic. Builds the reqwest HTTP POST, sets headers (`x-api-key`, `anthropic-version`, etc.), and delegates the response body stream to `process()`. This is intentionally thin — all logic is in `process.rs`.

- [ ] **Step 1: Write mod.rs**

```rust
pub mod sse;
pub mod wire;
pub mod convert;
pub mod process;

use std::sync::Arc;
use async_stream::stream;
use futures::StreamExt;

use crate::registry::ApiProvider;
use crate::types::{AssistantMessageEvent, Context, Model, StopReason, StreamOptions};
use crate::stream::EventStream;
use crate::util::env_keys::env_api_key;
use super::convert::build_request;

pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: Option<String>,
}

impl AnthropicProvider {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
        }
    }

    fn resolve_key(&self) -> Option<String> {
        self.api_key.clone().or_else(|| env_api_key("anthropic"))
    }
}

impl ApiProvider for AnthropicProvider {
    fn stream(
        &self,
        model: &Model,
        ctx: Context,
        opts: Option<StreamOptions>,
    ) -> EventStream {
        let key = opts.as_ref()
            .and_then(|o| o.api_key.clone())
            .or_else(|| self.resolve_key());
        let cancel = opts.as_ref().and_then(|o| o.cancel.clone());

        let Some(api_key) = key else {
            return Box::pin(stream! {
                yield AssistantMessageEvent::Error {
                    reason: StopReason::Error,
                    error: "No Anthropic API key found. Set ANTHROPIC_API_KEY or pass apiKey in options.".into(),
                };
            });
        };

        let req_body = build_request(model, &ctx, &opts);
        let base_url = model.base_url.trim_end_matches('/');
        let url = format!("{}/v1/messages", base_url);

        let mut request = self.client
            .post(&url)
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .header("accept", "text/event-stream")
            .json(&req_body);

        if let Some(opts) = &opts {
            if let Some(ref headers) = opts.headers {
                if let Some(obj) = headers.as_object() {
                    for (k, v) in obj {
                        if let Some(val) = v.as_str() {
                            request = request.header(k.as_str(), val);
                        }
                    }
                }
            }
        }

        let client = self.client.clone();
        let model = model.clone();
        Box::pin(stream! {
            let response = match request.send().await {
                Ok(r) => r,
                Err(e) => {
                    yield AssistantMessageEvent::Error {
                        reason: StopReason::Error,
                        error: format!("HTTP request failed: {}", e),
                    };
                    return;
                }
            };

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let body = response.text().await.unwrap_or_default();
                yield AssistantMessageEvent::Error {
                    reason: StopReason::Error,
                    error: format!("HTTP {} : {}", status, body),
                };
                return;
            }

            let body_stream = response
                .bytes_stream()
                .map(|r| r.map_err(|e| e.to_string()));

            let mut event_stream = process::process(body_stream, model, cancel);
            while let Some(event) = event_stream.next().await {
                yield event;
            }
        })
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p pi-ai 2>&1`
Expected: compile succeeds.

- [ ] **Step 3: Commit**

```bash
git add crates/pi-ai/src/
git commit -m "feat(pi-ai): add Anthropic ApiProvider implementation"
```

---

### Task 12: Faux provider and providers module

**Files:**
- Create: `crates/pi-ai/src/providers/faux.rs`
- Create: `crates/pi-ai/src/providers/mod.rs`

**What to build:** The faux provider replays scripted responses as delta events, exercising the same event protocol offline. The providers module registers built-in providers (Anthropic + optionally faux).

- [ ] **Step 1: Write providers/faux.rs**

```rust
use std::sync::Mutex;
use async_stream::stream;
use futures::StreamExt;
use crate::registry::ApiProvider;
use crate::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Model,
    StopReason, StreamOptions, Usage, Cost,
};
use crate::stream::EventStream;

pub struct FauxProvider {
    pub responses: Mutex<Vec<FauxResponse>>,
}

pub struct FauxResponse {
    pub text_deltas: Vec<String>,
    pub thinking_deltas: Vec<String>,
    pub tool_calls: Vec<FauxToolCall>,
}

pub struct FauxToolCall {
    pub id: String,
    pub name: String,
    pub deltas: Vec<String>,
    pub final_arguments: serde_json::Value,
}

impl FauxProvider {
    pub fn new(responses: Vec<FauxResponse>) -> Self {
        Self { responses: Mutex::new(responses) }
    }

    pub fn simple_text(text: &str) -> Self {
        Self::new(vec![FauxResponse {
            text_deltas: vec![text.to_string()],
            thinking_deltas: vec![],
            tool_calls: vec![],
        }])
    }
}

impl ApiProvider for FauxProvider {
    fn stream(&self, model: &Model, _ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
        let responses = self.responses.lock().unwrap().clone();
        let model_id = model.id.clone();
        Box::pin(stream! {
            let mut partial = AssistantMessage::empty("faux", &model_id);
            partial.provider = Some("faux".into());
            partial.timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            yield AssistantMessageEvent::Start { partial: partial.clone() };

            for resp in &responses {
                if !resp.text_deltas.is_empty() {
                    let mut p = partial.clone();
                    p.content.push(ContentBlock::Text {
                        text: resp.text_deltas.join(""),
                        text_signature: None,
                    });
                    yield AssistantMessageEvent::TextStart { partial: p };
                    for delta in &resp.text_deltas {
                        if let Some(ContentBlock::Text { text, .. }) = partial.content.last_mut() {
                            text.push_str(delta);
                        }
                        yield AssistantMessageEvent::TextDelta {
                            delta: delta.clone(),
                            partial: partial.clone(),
                        };
                    }
                    yield AssistantMessageEvent::TextEnd { partial: partial.clone() };
                }

                if !resp.thinking_deltas.is_empty() {
                    let mut p = partial.clone();
                    p.content.push(ContentBlock::Thinking {
                        thinking: resp.thinking_deltas.join(""),
                        thinking_signature: None,
                        redacted: None,
                    });
                    yield AssistantMessageEvent::ThinkingStart { partial: p };
                    for delta in &resp.thinking_deltas {
                        yield AssistantMessageEvent::ThinkingDelta {
                            delta: delta.clone(),
                            partial: partial.clone(),
                        };
                    }
                    yield AssistantMessageEvent::ThinkingEnd { partial: partial.clone() };
                }

                for tc in &resp.tool_calls {
                    let mut p = partial.clone();
                    p.content.push(ContentBlock::ToolCall {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        arguments: tc.final_arguments.clone(),
                        thought_signature: None,
                    });
                    yield AssistantMessageEvent::ToolcallStart { partial: p };
                    let mut accumulated = String::new();
                    for delta in &tc.deltas {
                        accumulated.push_str(delta);
                        if let Some(ContentBlock::ToolCall { arguments, .. }) = partial.content.last_mut() {
                            *arguments = serde_json::json!(accumulated);
                        }
                        yield AssistantMessageEvent::ToolcallDelta {
                            delta: delta.clone(),
                            partial: partial.clone(),
                        };
                    }
                    yield AssistantMessageEvent::ToolcallEnd { partial: partial.clone() };
                }
            }

            partial.usage = Usage {
                input: 10, output: 20, total_tokens: 30,
                ..Default::default()
            };
            partial.stop_reason = StopReason::Stop;

            yield AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message: partial,
            };
        })
    }
}
```

- [ ] **Step 2: Write providers/mod.rs**

```rust
pub mod faux;
pub mod anthropic;

use std::sync::Arc;
use crate::registry;

/// Register all built-in providers in the global registry.
/// Call this once at startup.
pub fn register_builtins() {
    registry::register(
        "anthropic-messages",
        Arc::new(anthropic::AnthropicProvider::new(None)),
    );
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p pi-ai 2>&1`
Expected: compile succeeds.

- [ ] **Step 4: Commit**

```bash
git add crates/pi-ai/src/
git commit -m "feat(pi-ai): add faux provider and providers module"
```

---

### Task 13: Public API — lib.rs finalization

**Files:**
- Modify: `crates/pi-ai/src/lib.rs`

**What to build:** Public re-exports of key types and functions. The crate's external API surface.

- [ ] **Step 1: Replace lib.rs with final public API**

```rust
pub mod types;
pub mod util;
pub mod models;
pub mod stream;
pub mod registry;
pub mod providers;

pub use types::{
    ContentBlock, Message, AssistantMessage, AssistantMessageEvent,
    Context, Tool, Model, StreamOptions, StopReason, Usage, Cost,
    ThinkingConfig,
};
pub use stream::{EventStream, complete};
pub use registry::{register, stream_model};
pub use models::{lookup_model, calculate_cost, all_models};
```

- [ ] **Step 2: Verify workspace build**

Run: `cargo build -p pi-ai 2>&1`
Expected: clean build, no warnings.

- [ ] **Step 3: Verify full workspace build**

Run: `cargo build 2>&1`
Expected: entire workspace builds.

- [ ] **Step 4: Commit**

```bash
git add crates/pi-ai/src/lib.rs
git commit -m "feat(pi-ai): finalize public API and re-exports"
```

---

### Task 14: Test fixtures — Anthropic SSE fixture files

**Files:**
- Create directory: `crates/pi-ai/tests/fixtures/`
- Create: `crates/pi-ai/tests/fixtures/anthropic-text.sse`
- Create: `crates/pi-ai/tests/fixtures/anthropic-thinking-tooluse.sse`

**What to build:** Hand-authored SSE fixture files matching the Anthropic streaming format. These drive the Anthropic mapping integration tests.

- [ ] **Step 1: Write anthropic-text.sse**

```
event: message_start
data: {"type":"message_start","message":{"id":"msg_001","type":"message","role":"assistant","model":"claude-sonnet-4-5","usage":{"input_tokens":5,"output_tokens":0}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" world"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":10}}

event: message_stop
data: {"type":"message_stop"}
```

- [ ] **Step 2: Write anthropic-thinking-tooluse.sse**

```
event: message_start
data: {"type":"message_start","message":{"id":"msg_002","type":"message","role":"assistant","model":"claude-sonnet-4-5","usage":{"input_tokens":10,"output_tokens":0}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"Let me think about this."}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":" The answer is 42."}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"think_sig_abc"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: content_block_start
data: {"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_01","name":"get_weather"}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"city\":\"New"}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":" York\"}"}}

event: content_block_stop
data: {"type":"content_block_stop","index":1}

event: content_block_start
data: {"type":"content_block_start","index":2,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":2,"delta":{"type":"text_delta","text":"Weather fetched."}}

event: content_block_stop
data: {"type":"content_block_stop","index":2}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":40}}

event: message_stop
data: {"type":"message_stop"}
```

- [ ] **Step 3: Commit**

```bash
git add crates/pi-ai/tests/
git commit -m "test(pi-ai): add Anthropic SSE fixture files"
```

---

### Task 15: Integration tests — Anthropic mapping test

**Files:**
- Create: `crates/pi-ai/tests/anthropic_mapping.rs`

**What to build:** Feeds fixture SSE files through `process()` and asserts the exact `AssistantMessageEvent` sequence and final `AssistantMessage`.

- [ ] **Step 1: Write anthropic_mapping.rs**

```rust
use bytes::Bytes;
use futures::stream;
use pi_ai::types::{
    AssistantMessageEvent, ContentBlock, Model, StopReason,
};

fn test_model() -> Model {
    Model {
        id: "claude-sonnet-4-5".into(),
        name: "Claude Sonnet 4.5".into(),
        api: "anthropic-messages".into(),
        provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(),
        reasoning: true,
        input: 3.0, output: 15.0,
        cache_read: Some(0.30), cache_write: Some(3.75),
        context_window: 200000, max_tokens: Some(8192),
        headers: None,
    }
}

fn fixture_bytes(path: &str) -> Vec<Bytes> {
    let content = std::fs::read_to_string(path).unwrap();
    vec![Bytes::from(content)]
}

#[tokio::test]
async fn text_only_stream() {
    let body = stream::iter(
        fixture_bytes("tests/fixtures/anthropic-text.sse")
            .into_iter()
            .map(Ok::<_, String>),
    );
    let model = test_model();
    let mut event_stream = pi_ai::providers::anthropic::process::process(body, model, None);
    use futures::StreamExt;

    let events: Vec<_> = event_stream.collect().await;
    assert!(events.len() >= 3, "expected at least 3 events, got {}", events.len());

    // First event should be Start
    assert!(matches!(events[0], AssistantMessageEvent::Start { .. }));

    // Should contain TextStart, TextDelta(s), TextEnd
    let has_text_start = events.iter().any(|e| matches!(e, AssistantMessageEvent::TextStart { .. }));
    let has_text_delta = events.iter().any(|e| matches!(e, AssistantMessageEvent::TextDelta { .. }));
    let has_text_end = events.iter().any(|e| matches!(e, AssistantMessageEvent::TextEnd { .. }));
    assert!(has_text_start);
    assert!(has_text_delta);
    assert!(has_text_end);

    // Last event should be Done
    let last = events.last().unwrap();
    match last {
        AssistantMessageEvent::Done { reason, message } => {
            assert_eq!(*reason, StopReason::Stop);
            assert!(!message.content.is_empty());
            if let ContentBlock::Text { text, .. } = &message.content[0] {
                assert!(text.contains("Hello"));
            } else {
                panic!("expected text content block");
            }
        }
        _ => panic!("expected Done event, got {:?}", last),
    }
}

#[tokio::test]
async fn thinking_and_tool_use_stream() {
    let body = stream::iter(
        fixture_bytes("tests/fixtures/anthropic-thinking-tooluse.sse")
            .into_iter()
            .map(Ok::<_, String>),
    );
    let model = test_model();
    let mut event_stream = pi_ai::providers::anthropic::process::process(body, model, None);
    use futures::StreamExt;

    let events: Vec<_> = event_stream.collect().await;

    let has_thinking = events.iter().any(|e| matches!(e, AssistantMessageEvent::ThinkingStart { .. }));
    let has_toolcall = events.iter().any(|e| matches!(e, AssistantMessageEvent::ToolcallStart { .. }));
    let has_text = events.iter().any(|e| matches!(e, AssistantMessageEvent::TextStart { .. }));
    assert!(has_thinking, "should have thinking events");
    assert!(has_toolcall, "should have tool use events");
    assert!(has_text, "should have text events");

    match events.last().unwrap() {
        AssistantMessageEvent::Done { reason, message } => {
            assert_eq!(*reason, StopReason::ToolUse);
            // Check tool call arguments were accumulated
            let has_complete_tool = message.content.iter().any(|b| {
                matches!(b, ContentBlock::ToolCall { arguments, .. } if arguments.as_object().map_or(false, |o| o.contains_key("city")))
            });
            assert!(has_complete_tool, "tool call should have parsed arguments");
        }
        _ => panic!("expected Done event"),
    }
}
```

- [ ] **Step 2: Verify tests**

Run: `cargo test -p pi-ai -- anthropic_mapping --nocapture`
Expected: 2 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/pi-ai/tests/
git commit -m "test(pi-ai): add Anthropic mapping integration tests"
```

---

### Task 16: Integration tests — request building and cost

**Files:**
- Create: `crates/pi-ai/tests/request_building.rs`
- Create: `crates/pi-ai/tests/cost.rs`

- [ ] **Step 1: Write request_building.rs**

```rust
use pi_ai::types::*;

fn test_model() -> Model {
    Model {
        id: "claude-haiku-4-5".into(), name: "Haiku".into(),
        api: "anthropic-messages".into(), provider: "anthropic".into(),
        base_url: "https://api.anthropic.com".into(), reasoning: false,
        input: 1.0, output: 5.0, cache_read: None, cache_write: None,
        context_window: 200000, max_tokens: Some(8192), headers: None,
    }
}

#[test]
fn basic_request_has_system_prompt_with_cache_control() {
    let ctx = Context {
        system_prompt: Some("Be concise.".into()),
        messages: vec![Message::User {
            content: vec![ContentBlock::Text { text: "hi".into(), text_signature: None }],
        }],
        tools: None,
    };
    let req = pi_ai::providers::anthropic::convert::build_request(&test_model(), &ctx, &None);
    let json = serde_json::to_value(&req).unwrap();
    let system = json["system"].as_array().unwrap();
    assert_eq!(system.len(), 1);
    assert_eq!(system[0]["text"], "Be concise.");
    assert_eq!(system[0]["cache_control"]["type"], "ephemeral");
}

#[test]
fn tool_result_coalescing_multiple_results() {
    let msgs = vec![
        Message::ToolResult {
            tool_call_id: "a".into(),
            content: vec![ContentBlock::Text { text: "r1".into(), text_signature: None }],
        },
        Message::ToolResult {
            tool_call_id: "b".into(),
            content: vec![ContentBlock::Text { text: "r2".into(), text_signature: None }],
        },
    ];
    let ctx = Context { system_prompt: None, messages: msgs, tools: None };
    let req = pi_ai::providers::anthropic::convert::build_request(&test_model(), &ctx, &None);
    assert_eq!(req.messages.len(), 1); // coalesced into single user turn
    assert_eq!(req.messages[0].role, "user");
}

#[test]
fn image_block_converts_to_anthropic_format() {
    let ctx = Context {
        system_prompt: None,
        messages: vec![Message::User {
            content: vec![ContentBlock::Image {
                data: "base64data".into(),
                mime_type: "image/png".into(),
            }],
        }],
        tools: None,
    };
    let req = pi_ai::providers::anthropic::convert::build_request(&test_model(), &ctx, &None);
    let json = serde_json::to_value(req.messages[0].content.clone()).unwrap();
    let first = &json.as_array().unwrap()[0];
    assert_eq!(first["type"], "image");
    assert_eq!(first["source"]["type"], "base64");
    assert_eq!(first["source"]["media_type"], "image/png");
}

#[test]
fn max_tokens_falls_back_to_model_default() {
    let ctx = Context { system_prompt: None, messages: vec![], tools: None };
    let req = pi_ai::providers::anthropic::convert::build_request(&test_model(), &ctx, &None);
    assert_eq!(req.max_tokens, 8192);
}

#[test]
fn tool_def_converts_parameters_to_input_schema() {
    let ctx = Context {
        system_prompt: None,
        messages: vec![],
        tools: Some(vec![Tool {
            name: "search".into(),
            description: Some("search the web".into()),
            parameters: serde_json::json!({"type": "object", "properties": {}}),
        }]),
    };
    let req = pi_ai::providers::anthropic::convert::build_request(&test_model(), &ctx, &None);
    let json = serde_json::to_value(&req).unwrap();
    let tools = json["tools"].as_array().unwrap();
    assert_eq!(tools[0]["name"], "search");
    assert!(tools[0].as_object().unwrap().contains_key("input_schema"));
}
```

- [ ] **Step 2: Write cost.rs**

```rust
use pi_ai::types::*;
use pi_ai::models::{lookup_model, calculate_cost};

#[test]
fn haiku_cost() {
    let model = lookup_model("claude-haiku-4-5").unwrap();
    let mut usage = Usage {
        input: 1_000_000, output: 1_000_000,
        cache_read: 0, cache_write: 0,
        total_tokens: 2_000_000,
        cost: Cost::default(),
    };
    calculate_cost(&model, &mut usage);
    assert!((usage.cost.input - 1.0).abs() < 0.01);     // $1/M
    assert!((usage.cost.output - 5.0).abs() < 0.01);    // $5/M
}

#[test]
fn opus_cost_with_cache() {
    let model = lookup_model("claude-opus-4-5").unwrap();
    let mut usage = Usage {
        input: 0, output: 0,
        cache_read: 1_000_000, cache_write: 1_000_000,
        total_tokens: 2_000_000,
        cost: Cost::default(),
    };
    calculate_cost(&model, &mut usage);
    assert!((usage.cost.cache_read - 1.50).abs() < 0.01);   // $1.50/M
    assert!((usage.cost.cache_write - 18.75).abs() < 0.01);  // $18.75/M
}

#[test]
fn zero_usage_zero_cost() {
    let model = lookup_model("claude-sonnet-4-5").unwrap();
    let mut usage = Usage::default();
    calculate_cost(&model, &mut usage);
    assert_eq!(usage.cost.input, 0.0);
    assert_eq!(usage.cost.output, 0.0);
}
```

- [ ] **Step 3: Verify tests**

Run: `cargo test -p pi-ai -- request_building cost --nocapture`
Expected: 8 tests pass (5 request + 3 cost).

- [ ] **Step 4: Commit**

```bash
git add crates/pi-ai/tests/
git commit -m "test(pi-ai): add request building and cost tests"
```

---

### Task 17: Integration tests — faux provider and serde roundtrip

**Files:**
- Create: `crates/pi-ai/tests/faux.rs`
- Create: `crates/pi-ai/tests/serde_roundtrip.rs`

- [ ] **Step 1: Write faux.rs**

```rust
use std::sync::Arc;
use futures::StreamExt;
use pi_ai::types::*;
use pi_ai::registry;
use pi_ai::providers::faux::{FauxProvider, FauxResponse, FauxToolCall};
use pi_ai::stream::complete;

fn faux_model() -> Model {
    Model {
        id: "faux-model".into(), name: "Faux Model".into(),
        api: "faux-api".into(), provider: "faux".into(),
        base_url: "".into(), reasoning: false,
        input: 0.0, output: 0.0, cache_read: None, cache_write: None,
        context_window: 0, max_tokens: None, headers: None,
    }
}

#[tokio::test]
async fn faux_simple_text() {
    let provider = Arc::new(FauxProvider::simple_text("Hello from faux!"));
    registry::register("faux-api", provider);

    let model = faux_model();
    let mut stream = registry::stream_model(
        &model,
        Context { system_prompt: None, messages: vec![], tools: None },
        None,
    );
    let events: Vec<_> = stream.collect().await;
    assert!(events.iter().any(|e| matches!(e, AssistantMessageEvent::Start { .. })));
    assert!(events.iter().any(|e| matches!(e, AssistantMessageEvent::TextDelta { .. })));

    let last = events.last().unwrap();
    match last {
        AssistantMessageEvent::Done { reason, message } => {
            assert_eq!(*reason, StopReason::Stop);
            assert_eq!(message.stop_reason, StopReason::Stop);
        }
        other => panic!("expected Done, got {:?}", other),
    }

    registry::unregister("faux-api");
}

#[tokio::test]
async fn faux_with_tool_call() {
    let provider = Arc::new(FauxProvider::new(vec![FauxResponse {
        text_deltas: vec![],
        thinking_deltas: vec![],
        tool_calls: vec![FauxToolCall {
            id: "call_1".into(),
            name: "read_file".into(),
            deltas: vec!["{\"path\":".into(), "\"/x\"}".into()],
            final_arguments: serde_json::json!({"path": "/x"}),
        }],
    }]));
    registry::register("faux-api", provider);

    let model = faux_model();
    let mut stream = registry::stream_model(
        &model,
        Context { system_prompt: None, messages: vec![], tools: None },
        None,
    );
    let events: Vec<_> = stream.collect().await;
    assert!(events.iter().any(|e| matches!(e, AssistantMessageEvent::ToolcallStart { .. })));
    assert!(events.iter().any(|e| matches!(e, AssistantMessageEvent::ToolcallDelta { .. })));
    assert!(events.iter().any(|e| matches!(e, AssistantMessageEvent::ToolcallEnd { .. })));
    registry::unregister("faux-api");
}

#[tokio::test]
async fn complete_with_faux() {
    let provider = Arc::new(FauxProvider::simple_text("complete test"));
    registry::register("faux-api", provider);

    let model = faux_model();
    let stream = registry::stream_model(
        &model,
        Context { system_prompt: None, messages: vec![], tools: None },
        None,
    );
    let result = complete(stream).await.unwrap();
    assert_eq!(result.stop_reason, StopReason::Stop);
    assert!(!result.content.is_empty());
    registry::unregister("faux-api");
}
```

- [ ] **Step 2: Write serde_roundtrip.rs**

```rust
use pi_ai::types::*;
use serde_json;

#[test]
fn assistant_message_roundtrip() {
    let msg = AssistantMessage {
        content: vec![ContentBlock::Text { text: "hello".into(), text_signature: Some("sig".into()) }],
        api: "anthropic-messages".into(),
        provider: Some("anthropic".into()),
        model: "claude-sonnet-4-5".into(),
        response_model: Some("claude-sonnet-4-5-20250219".into()),
        response_id: Some("msg_001".into()),
        usage: Usage {
            input: 100, output: 200, cache_read: 50, cache_write: 10,
            total_tokens: 300,
            cost: Cost { input: 0.0003, output: 0.003, cache_read: 0.0, cache_write: 0.0 },
        },
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp: 1717000000,
    };
    let json = serde_json::to_string_pretty(&msg).unwrap();
    let back: AssistantMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(back.content, msg.content);
    assert_eq!(back.api, msg.api);
    assert_eq!(back.model, msg.model);
    assert_eq!(back.stop_reason, msg.stop_reason);
    assert_eq!(back.usage.input, msg.usage.input);
    assert_eq!(back.timestamp, msg.timestamp);
}

#[test]
fn event_stream_all_variants_serialize() {
    let mut msg = AssistantMessage::empty("test", "test-model");

    let events = vec![
        AssistantMessageEvent::Start { partial: msg.clone() },
        AssistantMessageEvent::TextStart { partial: msg.clone() },
        AssistantMessageEvent::TextDelta { delta: "hi".into(), partial: msg.clone() },
        AssistantMessageEvent::TextEnd { partial: msg.clone() },
        AssistantMessageEvent::ThinkingStart { partial: msg.clone() },
        AssistantMessageEvent::ThinkingDelta { delta: "hmm".into(), partial: msg.clone() },
        AssistantMessageEvent::ThinkingEnd { partial: msg.clone() },
        AssistantMessageEvent::ToolcallStart { partial: msg.clone() },
        AssistantMessageEvent::ToolcallDelta { delta: "{}".into(), partial: msg.clone() },
        AssistantMessageEvent::ToolcallEnd { partial: msg.clone() },
        AssistantMessageEvent::Done { reason: StopReason::Stop, message: msg.clone() },
        AssistantMessageEvent::Error { reason: StopReason::Error, error: "oops".into() },
    ];

    for event in &events {
        let json = serde_json::to_string(event).unwrap();
        assert!(json.contains(r#""type""#), "event missing type field: {:?}", json);
    }
}

#[test]
fn context_serialization_matches_pi_format() {
    let ctx = Context {
        system_prompt: Some("Be helpful.".into()),
        messages: vec![
            Message::User { content: vec![ContentBlock::Text { text: "hi".into(), text_signature: None }] },
            Message::Assistant { content: vec![ContentBlock::Text { text: "hello!".into(), text_signature: None }] },
        ],
        tools: None,
    };
    let json = serde_json::to_string(&ctx).unwrap();
    assert!(json.contains(r#""systemPrompt""#));
    assert!(json.contains(r#""role":"user""#));
    assert!(json.contains(r#""role":"assistant""#));
    assert!(json.contains(r#""type":"text""#));
}

#[test]
fn content_block_all_variants_roundtrip() {
    let blocks = vec![
        ContentBlock::Text { text: "hi".into(), text_signature: None },
        ContentBlock::Thinking { thinking: "hmm".into(), thinking_signature: None, redacted: Some(false) },
        ContentBlock::Image { data: "base64data".into(), mime_type: "image/png".into() },
        ContentBlock::ToolCall { id: "t1".into(), name: "f".into(),
            arguments: serde_json::json!({"x": 1}), thought_signature: None },
    ];
    for block in &blocks {
        let json = serde_json::to_string(block).unwrap();
        let back: ContentBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(back, *block);
    }
}
```

- [ ] **Step 3: Verify tests**

Run: `cargo test -p pi-ai -- faux serde_roundtrip --nocapture`
Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/pi-ai/tests/
git commit -m "test(pi-ai): add faux provider e2e and serde roundtrip tests"
```

---

### Task 18: Example — faux_stream.rs

**Files:**
- Create directory: `crates/pi-ai/examples/`
- Create: `crates/pi-ai/examples/faux_stream.rs`

**What to build:** An offline example consuming the faux provider's event stream and printing deltas. No API key required.

- [ ] **Step 1: Write faux_stream.rs**

```rust
use std::sync::Arc;
use futures::StreamExt;
use pi_ai::providers::faux::{FauxProvider, FauxResponse};
use pi_ai::registry;
use pi_ai::types::*;

#[tokio::main]
async fn main() {
    // Build a faux provider with a scripted response
    let provider = Arc::new(FauxProvider::new(vec![
        FauxResponse {
            text_deltas: vec!["Thinking step-by-step...\n".into(), "The answer ".into(), "is 42.".into()],
            thinking_deltas: vec![],
            tool_calls: vec![],
        },
    ]));
    registry::register("faux-api", provider);

    let model = Model {
        id: "faux-model".into(), name: "Faux Model".into(),
        api: "faux-api".into(), provider: "faux".into(),
        base_url: String::new(), reasoning: false,
        input: 0.0, output: 0.0, cache_read: None, cache_write: None,
        context_window: 0, max_tokens: None, headers: None,
    };

    let ctx = Context {
        system_prompt: Some("Answer concisely.".into()),
        messages: vec![Message::User {
            content: vec![ContentBlock::Text {
                text: "What is the meaning of life?".into(),
                text_signature: None,
            }],
        }],
        tools: None,
    };

    let mut stream = registry::stream_model(&model, ctx, None);

    println!("=== faux provider streaming demo ===\n");
    while let Some(event) = stream.next().await {
        match event {
            AssistantMessageEvent::TextDelta { delta, .. } => {
                print!("{}", delta);
            }
            AssistantMessageEvent::ThinkingDelta { delta, .. } => {
                print!("[think] {}", delta);
            }
            AssistantMessageEvent::ToolcallDelta { delta, .. } => {
                print!("[tool: {}]", delta);
            }
            AssistantMessageEvent::Done { message, .. } => {
                println!("\n\n--- Done ---");
                println!("stop reason: {:?}", message.stop_reason);
                println!("usage: {:?}", message.usage);
            }
            AssistantMessageEvent::Error { error, .. } => {
                eprintln!("\nError: {}", error);
            }
            _ => {}
        }
    }
    println!("=== end ===");
}
```

- [ ] **Step 2: Verify example compiles**

Run: `cargo check -p pi-ai --example faux_stream`
Expected: compiles cleanly.

- [ ] **Step 3: Run example**

Run: `cargo run -p pi-ai --example faux_stream`
Expected: prints text deltas and Done message.

- [ ] **Step 4: Commit**

```bash
git add crates/pi-ai/examples/
git commit -m "feat(pi-ai): add faux_stream offline example"
```

---

### Task 19: Final verification

- [ ] **Step 1: Run all tests**

Run: `cargo test -p pi-ai -- --nocapture`
Expected: all tests pass (30+ tests across unit and integration).

- [ ] **Step 2: Verify workspace builds**

Run: `cargo build`
Expected: entire workspace builds cleanly, no warnings.

- [ ] **Step 3: Verify workspace tests**

Run: `cargo test`
Expected: all workspace tests pass.

- [ ] **Step 4: Run example smoke test**

Run: `cargo run -p pi-ai --example faux_stream`
Expected: prints streaming output and exits cleanly.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore: final verification - all tests pass, workspace builds"
```
