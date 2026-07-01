use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TRANSCRIPT_CACHE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq)]
pub enum TranscriptItem {
    User {
        text: String,
    },
    Assistant {
        id: String,
        markdown: String,
        thinking: String,
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
    System {
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
            thinking: String::new(),
            done,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self::Error { text: text.into() }
    }

    pub fn system(text: impl Into<String>) -> Self {
        Self::System { text: text.into() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TranscriptRenderKey {
    pub(crate) transcript_id: u64,
    pub(crate) item_id: u64,
    pub(crate) item_revision: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TranscriptMutation {
    changed_indices: Vec<usize>,
}

impl TranscriptMutation {
    fn none() -> Self {
        Self::default()
    }

    fn single(index: usize) -> Self {
        Self {
            changed_indices: vec![index],
        }
    }

    pub(crate) fn extend(&mut self, other: Self) {
        self.changed_indices.extend(other.changed_indices);
    }

    pub(crate) fn changed_indices(&self) -> &[usize] {
        &self.changed_indices
    }
}

#[derive(Debug, Clone)]
struct TranscriptItemMeta {
    item_id: u64,
    revision: u64,
}

#[derive(Debug)]
pub struct Transcript {
    items: Vec<TranscriptItem>,
    item_meta: Vec<TranscriptItemMeta>,
    scroll_offset: usize,
    new_output_below: bool,
    content_revision: u64,
    revision: u64,
    cache_id: u64,
    next_item_id: u64,
}

impl Default for Transcript {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            item_meta: Vec::new(),
            scroll_offset: 0,
            new_output_below: false,
            content_revision: 0,
            revision: 0,
            cache_id: NEXT_TRANSCRIPT_CACHE_ID.fetch_add(1, Ordering::Relaxed),
            next_item_id: 0,
        }
    }
}

impl Clone for Transcript {
    fn clone(&self) -> Self {
        Self {
            items: self.items.clone(),
            item_meta: self.item_meta.clone(),
            scroll_offset: self.scroll_offset,
            new_output_below: self.new_output_below,
            content_revision: self.content_revision,
            revision: self.revision,
            cache_id: NEXT_TRANSCRIPT_CACHE_ID.fetch_add(1, Ordering::Relaxed),
            next_item_id: self.next_item_id,
        }
    }
}

impl Transcript {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, item: TranscriptItem) {
        self.push_with_index(item);
    }

    fn push_with_index(&mut self, item: TranscriptItem) -> usize {
        self.record_output_below();
        let item_id = self.next_item_id;
        self.next_item_id = self.next_item_id.wrapping_add(1);
        let index = self.items.len();
        self.items.push(item);
        self.item_meta.push(TranscriptItemMeta {
            item_id,
            revision: 0,
        });
        self.bump_content_revision();
        index
    }

    pub fn items(&self) -> &[TranscriptItem] {
        &self.items
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn content_revision(&self) -> u64 {
        self.content_revision
    }

    pub(crate) fn render_cache_id(&self) -> u64 {
        self.cache_id
    }

    pub(crate) fn render_entries(
        &self,
    ) -> impl Iterator<Item = (TranscriptRenderKey, &TranscriptItem)> {
        debug_assert_eq!(self.items.len(), self.item_meta.len());
        let transcript_id = self.cache_id;
        self.items
            .iter()
            .zip(self.item_meta.iter())
            .map(move |(item, meta)| {
                (
                    TranscriptRenderKey {
                        transcript_id,
                        item_id: meta.item_id,
                        item_revision: meta.revision,
                    },
                    item,
                )
            })
    }

    pub(crate) fn render_entry_at(
        &self,
        index: usize,
    ) -> Option<(TranscriptRenderKey, &TranscriptItem)> {
        debug_assert_eq!(self.items.len(), self.item_meta.len());
        let item = self.items.get(index)?;
        let meta = self.item_meta.get(index)?;
        Some((
            TranscriptRenderKey {
                transcript_id: self.cache_id,
                item_id: meta.item_id,
                item_revision: meta.revision,
            },
            item,
        ))
    }

    pub fn scroll_page_up(&mut self, rows: usize) {
        let previous = self.scroll_offset;
        self.scroll_offset = self.scroll_offset.saturating_add(rows);
        if self.scroll_offset != previous {
            self.bump_revision();
        }
    }

    pub fn scroll_page_down(&mut self, rows: usize) {
        let previous_offset = self.scroll_offset;
        let previous_new_output_below = self.new_output_below;
        self.scroll_offset = self.scroll_offset.saturating_sub(rows);
        if self.scroll_offset == 0 {
            self.new_output_below = false;
        }
        if self.scroll_offset != previous_offset
            || self.new_output_below != previous_new_output_below
        {
            self.bump_revision();
        }
    }

    pub fn scroll_to_bottom(&mut self) {
        let previous_offset = self.scroll_offset;
        let previous_new_output_below = self.new_output_below;
        self.scroll_offset = 0;
        self.new_output_below = false;
        if self.scroll_offset != previous_offset
            || self.new_output_below != previous_new_output_below
        {
            self.bump_revision();
        }
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    pub fn has_new_output_below(&self) -> bool {
        self.new_output_below
    }

    pub(crate) fn preserve_scrolled_view_after_hidden_change(
        &mut self,
        previous_scroll_offset: usize,
        added_rows: usize,
    ) {
        if previous_scroll_offset == 0 {
            return;
        }
        let previous_offset = self.scroll_offset;
        let previous_new_output_below = self.new_output_below;
        self.scroll_offset = previous_scroll_offset.saturating_add(added_rows);
        self.new_output_below = true;
        if self.scroll_offset != previous_offset
            || self.new_output_below != previous_new_output_below
        {
            self.bump_revision();
        }
    }

    pub fn apply_event(&mut self, event: UiEvent) {
        self.apply_event_with_mutation(event);
    }

    pub(crate) fn apply_event_with_mutation(&mut self, event: UiEvent) -> TranscriptMutation {
        match event {
            UiEvent::AgentStarted => TranscriptMutation::none(),
            UiEvent::TurnStarted => self.close_open_assistant(),
            UiEvent::AssistantDelta { text } => self.append_assistant_delta(&text),
            UiEvent::ThinkingDelta { text } => self.append_assistant_thinking(&text),
            UiEvent::AssistantDone => self.mark_assistant_done(),
            UiEvent::ToolStarted {
                call_id,
                name,
                args,
            } => {
                let mut mutation = self.close_open_assistant();
                mutation.extend(TranscriptMutation::single(self.push_with_index(
                    TranscriptItem::Tool {
                        call_id,
                        name,
                        args,
                        result: None,
                        is_error: false,
                    },
                )));
                mutation
            }
            UiEvent::ToolFinished {
                call_id,
                result,
                is_error,
            } => self.finish_tool(&call_id, result, is_error),
            UiEvent::ToolUpdated { call_id, result } => self.update_tool(&call_id, result),
            UiEvent::AgentError { error } => {
                let mut mutation = self.close_open_assistant();
                mutation.extend(TranscriptMutation::single(
                    self.push_with_index(TranscriptItem::Error { text: error }),
                ));
                mutation
            }
            UiEvent::CompactionNotice { summary } => {
                TranscriptMutation::single(self.push_with_index(TranscriptItem::Assistant {
                    id: format!("compaction_{}", self.items.len()),
                    markdown: summary,
                    thinking: String::new(),
                    done: true,
                }))
            }
            UiEvent::UsageUpdate { .. } => TranscriptMutation::none(),
        }
    }

    fn append_assistant_delta(&mut self, text: &str) -> TranscriptMutation {
        let was_scrolled = self.scroll_offset > 0;
        if let Some(index) = self
            .items
            .iter()
            .rposition(|item| matches!(item, TranscriptItem::Assistant { done: false, .. }))
        {
            let TranscriptItem::Assistant { markdown, done, .. } = &mut self.items[index] else {
                unreachable!("rposition matched unfinished assistant");
            };
            if was_scrolled {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
                self.new_output_below = true;
            } else {
                self.scroll_offset = 0;
                self.new_output_below = false;
            }
            markdown.push_str(text);
            *done = false;
            self.bump_item_revision(index);
            self.bump_content_revision();
            return TranscriptMutation::single(index);
        }

        TranscriptMutation::single(self.push_with_index(TranscriptItem::Assistant {
            id: format!("assistant_{}", self.items.len()),
            markdown: text.to_string(),
            thinking: String::new(),
            done: false,
        }))
    }

    fn append_assistant_thinking(&mut self, text: &str) -> TranscriptMutation {
        let was_scrolled = self.scroll_offset > 0;
        if let Some(index) = self
            .items
            .iter()
            .rposition(|item| matches!(item, TranscriptItem::Assistant { done: false, .. }))
        {
            let TranscriptItem::Assistant { thinking, done, .. } = &mut self.items[index] else {
                unreachable!("rposition matched unfinished assistant");
            };
            if was_scrolled {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
                self.new_output_below = true;
            } else {
                self.scroll_offset = 0;
                self.new_output_below = false;
            }
            thinking.push_str(text);
            *done = false;
            self.bump_item_revision(index);
            self.bump_content_revision();
            return TranscriptMutation::single(index);
        }

        TranscriptMutation::single(self.push_with_index(TranscriptItem::Assistant {
            id: format!("assistant_{}", self.items.len()),
            markdown: String::new(),
            thinking: text.to_string(),
            done: false,
        }))
    }

    fn mark_assistant_done(&mut self) -> TranscriptMutation {
        if let Some(index) = self
            .items
            .iter()
            .rposition(|item| matches!(item, TranscriptItem::Assistant { done: false, .. }))
        {
            let TranscriptItem::Assistant { done, .. } = &mut self.items[index] else {
                unreachable!("rposition matched unfinished assistant");
            };
            *done = true;
            self.bump_item_revision(index);
            self.bump_content_revision();
            return TranscriptMutation::single(index);
        }

        TranscriptMutation::single(self.push_with_index(TranscriptItem::Assistant {
            id: format!("assistant_{}", self.items.len()),
            markdown: String::new(),
            thinking: String::new(),
            done: true,
        }))
    }

    fn close_open_assistant(&mut self) -> TranscriptMutation {
        let Some(index) = self
            .items
            .iter()
            .rposition(|item| matches!(item, TranscriptItem::Assistant { done: false, .. }))
        else {
            return TranscriptMutation::none();
        };
        let TranscriptItem::Assistant { done, .. } = &mut self.items[index] else {
            unreachable!("rposition matched unfinished assistant");
        };
        *done = true;
        self.bump_item_revision(index);
        self.bump_content_revision();
        TranscriptMutation::single(index)
    }

    fn finish_tool(&mut self, call_id: &str, result: String, is_error: bool) -> TranscriptMutation {
        if let Some(index) = self.items.iter().position(|item| {
            matches!(item, TranscriptItem::Tool { call_id: existing, .. } if existing == call_id)
        }) {
            let TranscriptItem::Tool {
                result: existing,
                is_error: existing_error,
                ..
            } = &mut self.items[index]
            else {
                unreachable!("position matched tool item");
            };
            *existing = Some(result);
            *existing_error = is_error;
            self.record_output_below();
            self.bump_item_revision(index);
            self.bump_content_revision();
            return TranscriptMutation::single(index);
        }
        TranscriptMutation::none()
    }

    fn update_tool(&mut self, call_id: &str, result: String) -> TranscriptMutation {
        if let Some(index) = self.items.iter().position(|item| {
            matches!(item, TranscriptItem::Tool { call_id: existing, .. } if existing == call_id)
        }) {
            let TranscriptItem::Tool {
                result: existing, ..
            } = &mut self.items[index]
            else {
                unreachable!("position matched tool item");
            };
            *existing = Some(result);
            self.record_output_below();
            self.bump_item_revision(index);
            self.bump_content_revision();
            return TranscriptMutation::single(index);
        }
        TranscriptMutation::none()
    }

    fn record_output_below(&mut self) {
        if self.scroll_offset == 0 {
            self.scroll_offset = 0;
            self.new_output_below = false;
        } else {
            self.scroll_offset = self.scroll_offset.saturating_add(1);
            self.new_output_below = true;
        }
    }

    fn bump_revision(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    fn bump_content_revision(&mut self) {
        self.content_revision = self.content_revision.wrapping_add(1);
        self.bump_revision();
    }

    fn bump_item_revision(&mut self, index: usize) {
        if let Some(meta) = self.item_meta.get_mut(index) {
            meta.revision = meta.revision.wrapping_add(1);
        }
    }
}
use super::UiEvent;
