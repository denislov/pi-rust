use super::message::AssistantMessage;
use super::usage::StopReason;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum AssistantMessageEvent {
    #[serde(rename = "start")]
    Start {
        #[serde(rename = "contentIndex", skip_serializing_if = "Option::is_none")]
        content_index: Option<u32>,
        partial: AssistantMessage,
    },
    #[serde(rename = "text_start")]
    TextStart {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        partial: AssistantMessage,
    },
    #[serde(rename = "text_delta")]
    TextDelta {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        delta: String,
        partial: AssistantMessage,
    },
    #[serde(rename = "text_end")]
    TextEnd {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        partial: AssistantMessage,
    },
    #[serde(rename = "thinking_start")]
    ThinkingStart {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        partial: AssistantMessage,
    },
    #[serde(rename = "thinking_delta")]
    ThinkingDelta {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        delta: String,
        partial: AssistantMessage,
    },
    #[serde(rename = "thinking_end")]
    ThinkingEnd {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        partial: AssistantMessage,
    },
    #[serde(rename = "toolcall_start")]
    ToolcallStart {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        partial: AssistantMessage,
    },
    #[serde(rename = "toolcall_delta")]
    ToolcallDelta {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        delta: String,
        partial: AssistantMessage,
    },
    #[serde(rename = "toolcall_end")]
    ToolcallEnd {
        #[serde(rename = "contentIndex")]
        content_index: u32,
        partial: AssistantMessage,
    },
    #[serde(rename = "done")]
    Done {
        reason: StopReason,
        message: AssistantMessage,
    },
    #[serde(rename = "error")]
    Error {
        reason: StopReason,
        message: AssistantMessage,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut msg = AssistantMessage::empty("test", "test");
        msg.error_message = Some("fail".into());
        msg.stop_reason = StopReason::Error;
        let ev = AssistantMessageEvent::Error {
            reason: StopReason::Error,
            message: msg,
        };
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains(r#""type":"error""#));
        let back: AssistantMessageEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, AssistantMessageEvent::Error { .. }));
    }
}
