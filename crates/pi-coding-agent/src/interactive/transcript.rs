#[derive(Debug, Clone, PartialEq)]
pub enum TranscriptItem {
    User {
        text: String,
    },
    Assistant {
        id: String,
        markdown: String,
        done: bool,
    },
    Tool {
        call_id: String,
        name: String,
        args: serde_json::Value,
        result: Option<String>,
        is_error: bool,
    },
    Error {
        text: String,
    },
}

impl TranscriptItem {
    pub fn user(text: impl Into<String>) -> Self {
        Self::User { text: text.into() }
    }

    pub fn assistant(id: impl Into<String>, markdown: impl Into<String>, done: bool) -> Self {
        Self::Assistant {
            id: id.into(),
            markdown: markdown.into(),
            done,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self::Error { text: text.into() }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Transcript {
    items: Vec<TranscriptItem>,
    scroll_offset: usize,
}

impl Transcript {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, item: TranscriptItem) {
        self.items.push(item);
        self.scroll_to_bottom();
    }

    pub fn items(&self) -> &[TranscriptItem] {
        &self.items
    }

    pub fn scroll_page_up(&mut self, rows: usize) {
        self.scroll_offset = (self.scroll_offset + rows).min(self.max_scroll_offset());
    }

    pub fn scroll_page_down(&mut self, rows: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(rows);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    pub fn apply_event(&mut self, event: UiEvent) {
        match event {
            UiEvent::AgentStarted | UiEvent::TurnStarted => {}
            UiEvent::AssistantDelta { text } => self.append_assistant_delta(&text),
            UiEvent::AssistantDone => self.mark_assistant_done(),
            UiEvent::ToolStarted {
                call_id,
                name,
                args,
            } => self.push(TranscriptItem::Tool {
                call_id,
                name,
                args,
                result: None,
                is_error: false,
            }),
            UiEvent::ToolFinished {
                call_id,
                result,
                is_error,
            } => self.finish_tool(&call_id, result, is_error),
            UiEvent::AgentError { error } => self.push(TranscriptItem::Error { text: error }),
            UiEvent::CompactionNotice { summary } => self.push(TranscriptItem::Assistant {
                id: format!("compaction_{}", self.items.len()),
                markdown: summary,
                done: true,
            }),
        }
    }

    fn max_scroll_offset(&self) -> usize {
        self.items.len().saturating_sub(1)
    }

    fn append_assistant_delta(&mut self, text: &str) {
        if let Some(TranscriptItem::Assistant { markdown, done, .. }) = self
            .items
            .iter_mut()
            .rev()
            .find(|item| matches!(item, TranscriptItem::Assistant { done: false, .. }))
        {
            markdown.push_str(text);
            *done = false;
            self.scroll_to_bottom();
            return;
        }

        self.push(TranscriptItem::Assistant {
            id: format!("assistant_{}", self.items.len()),
            markdown: text.to_string(),
            done: false,
        });
    }

    fn mark_assistant_done(&mut self) {
        if let Some(TranscriptItem::Assistant { done, .. }) = self
            .items
            .iter_mut()
            .rev()
            .find(|item| matches!(item, TranscriptItem::Assistant { done: false, .. }))
        {
            *done = true;
            self.scroll_to_bottom();
            return;
        }

        self.push(TranscriptItem::Assistant {
            id: format!("assistant_{}", self.items.len()),
            markdown: String::new(),
            done: true,
        });
    }

    fn finish_tool(&mut self, call_id: &str, result: String, is_error: bool) {
        if let Some(TranscriptItem::Tool {
            result: existing,
            is_error: existing_error,
            ..
        }) = self.items.iter_mut().find(|item| {
            matches!(
                item,
                TranscriptItem::Tool {
                    call_id: existing,
                    ..
                } if existing == call_id
            )
        }) {
            *existing = Some(result);
            *existing_error = is_error;
            self.scroll_to_bottom();
        }
    }
}
use super::UiEvent;
