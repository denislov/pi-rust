use std::time::{Duration, Instant};

use super::{InputEvent, parse_key};

const ESC: &str = "\x1b";
const BRACKETED_PASTE_START: &str = "\x1b[200~";
const BRACKETED_PASTE_END: &str = "\x1b[201~";
const DEFAULT_PENDING_TIMEOUT: Duration = Duration::from_millis(10);

#[derive(Debug, Clone)]
pub struct StdinBuffer {
    buffer: String,
    paste_buffer: String,
    in_paste: bool,
    pending_timeout: Duration,
    pending_since: Option<Instant>,
}

impl Default for StdinBuffer {
    fn default() -> Self {
        Self {
            buffer: String::new(),
            paste_buffer: String::new(),
            in_paste: false,
            pending_timeout: DEFAULT_PENDING_TIMEOUT,
            pending_since: None,
        }
    }
}

impl StdinBuffer {
    /// Create a buffer using the default 10ms idle timeout for incomplete
    /// escape sequences (matches the TypeScript `pi` reference).
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a buffer that flushes incomplete escape sequences after
    /// `pending_timeout` has elapsed without further input.
    ///
    /// Use [`Duration::MAX`] to disable timeout-driven flushing (callers must
    /// then drive [`StdinBuffer::flush`] explicitly).
    pub fn with_pending_timeout(pending_timeout: Duration) -> Self {
        Self {
            pending_timeout,
            ..Self::default()
        }
    }

    /// Configure the idle timeout for incomplete escape sequences.
    pub fn set_pending_timeout(&mut self, pending_timeout: Duration) {
        self.pending_timeout = pending_timeout;
    }

    /// Returns the configured idle timeout for incomplete escape sequences.
    pub fn pending_timeout_duration(&self) -> Duration {
        self.pending_timeout
    }

    pub fn process(&mut self, data: &str) -> Vec<InputEvent> {
        self.process_at(data, Instant::now())
    }

    /// Same as [`StdinBuffer::process`], but uses the supplied instant as
    /// "now" when stamping the pending residual. Useful for deterministic
    /// tests.
    pub fn process_at(&mut self, data: &str, now: Instant) -> Vec<InputEvent> {
        self.buffer.push_str(data);
        let mut events = Vec::new();

        loop {
            if self.in_paste {
                if let Some(end_index) = self.buffer.find(BRACKETED_PASTE_END) {
                    self.paste_buffer.push_str(&self.buffer[..end_index]);
                    let remainder_start = end_index + BRACKETED_PASTE_END.len();
                    let remainder = self.buffer[remainder_start..].to_string();
                    self.buffer.clear();
                    self.buffer.push_str(&remainder);
                    self.in_paste = false;
                    events.push(InputEvent::Paste(std::mem::take(&mut self.paste_buffer)));
                    continue;
                }

                self.paste_buffer.push_str(&self.buffer);
                self.buffer.clear();
                break;
            }

            if self.buffer.is_empty() {
                break;
            }

            if self.buffer.starts_with(BRACKETED_PASTE_START) {
                let remainder = self.buffer[BRACKETED_PASTE_START.len()..].to_string();
                self.buffer.clear();
                self.buffer.push_str(&remainder);
                self.in_paste = true;
                continue;
            }

            let Some(sequence_len) = next_sequence_len(&self.buffer) else {
                break;
            };
            let sequence = self.buffer[..sequence_len].to_string();
            let remainder = self.buffer[sequence_len..].to_string();
            self.buffer.clear();
            self.buffer.push_str(&remainder);

            events.push(
                parse_key(&sequence)
                    .map(InputEvent::Key)
                    .unwrap_or_else(|| InputEvent::Raw(sequence)),
            );
        }

        self.refresh_pending_since(now);
        events
    }

    /// Flush any incomplete residual. Used when stdin is closing or the host
    /// otherwise wants to force-emit whatever is buffered.
    pub fn flush(&mut self) -> Vec<InputEvent> {
        let mut events = Vec::new();
        if self.in_paste {
            events.push(InputEvent::Paste(std::mem::take(&mut self.paste_buffer)));
            self.in_paste = false;
        }
        events.extend(self.drain_pending_residual());
        events
    }

    /// If non-paste residual has been waiting longer than the configured
    /// pending timeout, emit it as if [`StdinBuffer::flush`] had been called.
    /// Otherwise returns an empty vector.
    pub fn tick(&mut self, now: Instant) -> Vec<InputEvent> {
        match self.pending_timeout_at(now) {
            Some(remaining) if remaining.is_zero() => self.drain_pending_residual(),
            _ => Vec::new(),
        }
    }

    /// Returns the remaining time before pending residual should be flushed
    /// (using the configured timeout). `Some(Duration::ZERO)` means it is
    /// already due, `None` means no residual is pending or timeouts are
    /// disabled.
    pub fn pending_timeout_at(&self, now: Instant) -> Option<Duration> {
        let started = self.pending_since?;
        if self.pending_timeout == Duration::MAX {
            return None;
        }
        let deadline = started.checked_add(self.pending_timeout)?;
        Some(deadline.saturating_duration_since(now))
    }

    /// True when there is non-paste residual currently parked in the buffer.
    pub fn has_pending_residual(&self) -> bool {
        self.pending_since.is_some()
    }

    fn drain_pending_residual(&mut self) -> Vec<InputEvent> {
        self.pending_since = None;
        if self.buffer.is_empty() {
            return Vec::new();
        }
        let data = std::mem::take(&mut self.buffer);
        let event = parse_key(&data)
            .map(InputEvent::Key)
            .unwrap_or(InputEvent::Raw(data));
        vec![event]
    }

    fn refresh_pending_since(&mut self, now: Instant) {
        if !self.in_paste && !self.buffer.is_empty() {
            if self.pending_since.is_none() {
                self.pending_since = Some(now);
            }
        } else {
            self.pending_since = None;
        }
    }
}

fn next_sequence_len(buffer: &str) -> Option<usize> {
    if !buffer.starts_with(ESC) {
        return buffer.chars().next().map(char::len_utf8);
    }

    if buffer.len() == ESC.len() {
        return None;
    }

    if buffer.starts_with("\x1b[") {
        return csi_sequence_len(buffer);
    }
    if buffer.starts_with("\x1b]") {
        return osc_sequence_len(buffer);
    }
    if buffer.starts_with("\x1bP") || buffer.starts_with("\x1b_") {
        return string_terminated_sequence_len(buffer);
    }
    if buffer.starts_with("\x1bO") {
        return nth_char_end(buffer, 3);
    }

    nth_char_end(buffer, 2)
}

fn csi_sequence_len(buffer: &str) -> Option<usize> {
    if buffer.len() < 3 {
        return None;
    }

    for (index, byte) in buffer.as_bytes().iter().enumerate().skip(2) {
        if (0x40..=0x7e).contains(byte) {
            return Some(index + 1);
        }
    }
    None
}

fn osc_sequence_len(buffer: &str) -> Option<usize> {
    if let Some(index) = buffer.find('\x07') {
        return Some(index + 1);
    }
    string_terminated_sequence_len(buffer)
}

fn string_terminated_sequence_len(buffer: &str) -> Option<usize> {
    buffer.find("\x1b\\").map(|index| index + 2)
}

fn nth_char_end(buffer: &str, count: usize) -> Option<usize> {
    let mut end = 0;
    for (index, ch) in buffer.char_indices().take(count) {
        end = index + ch.len_utf8();
    }
    if buffer.chars().count() >= count {
        Some(end)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Key, KeyEventKind, KeyModifiers};

    #[test]
    fn lone_escape_is_held_until_timeout_elapses() {
        let start = Instant::now();
        let mut buffer = StdinBuffer::with_pending_timeout(Duration::from_millis(10));
        let immediate = buffer.process_at("\x1b", start);
        assert!(
            immediate.is_empty(),
            "lone escape must not emit until timeout elapses"
        );
        assert!(buffer.has_pending_residual());

        let still_pending = buffer.tick(start + Duration::from_millis(5));
        assert!(still_pending.is_empty());
        assert!(buffer.has_pending_residual());

        let flushed = buffer.tick(start + Duration::from_millis(10));
        assert_eq!(flushed.len(), 1);
        match &flushed[0] {
            InputEvent::Key(event) => {
                assert_eq!(event.key, Key::Escape);
                assert_eq!(event.modifiers, KeyModifiers::empty());
                assert_eq!(event.kind, KeyEventKind::Press);
            }
            other => panic!("expected key event, got {other:?}"),
        }
        assert!(!buffer.has_pending_residual());
    }

    #[test]
    fn split_csi_sequence_is_combined_when_followup_arrives_before_timeout() {
        let start = Instant::now();
        let mut buffer = StdinBuffer::with_pending_timeout(Duration::from_millis(10));
        let first = buffer.process_at("\x1b", start);
        assert!(first.is_empty());
        assert!(buffer.has_pending_residual());

        let second = buffer.process_at("[A", start + Duration::from_millis(2));
        assert_eq!(second.len(), 1);
        match &second[0] {
            InputEvent::Key(event) => assert_eq!(event.key, Key::Up),
            other => panic!("expected up arrow key, got {other:?}"),
        }
        assert!(!buffer.has_pending_residual());
    }

    #[test]
    fn pending_timeout_at_reports_remaining_time() {
        let start = Instant::now();
        let mut buffer = StdinBuffer::with_pending_timeout(Duration::from_millis(10));
        assert!(buffer.pending_timeout_at(start).is_none());
        buffer.process_at("\x1b", start);

        let remaining = buffer
            .pending_timeout_at(start + Duration::from_millis(3))
            .expect("residual is pending");
        assert_eq!(remaining, Duration::from_millis(7));

        let due = buffer
            .pending_timeout_at(start + Duration::from_millis(10))
            .expect("residual is pending");
        assert_eq!(due, Duration::ZERO);
    }

    #[test]
    fn flush_drains_residual_immediately() {
        let start = Instant::now();
        let mut buffer = StdinBuffer::with_pending_timeout(Duration::from_millis(10));
        buffer.process_at("\x1b", start);
        let flushed = buffer.flush();
        assert_eq!(flushed.len(), 1);
        match &flushed[0] {
            InputEvent::Key(event) => assert_eq!(event.key, Key::Escape),
            other => panic!("expected escape, got {other:?}"),
        }
        assert!(!buffer.has_pending_residual());
    }

    #[test]
    fn paste_in_progress_does_not_set_pending_residual() {
        let start = Instant::now();
        let mut buffer = StdinBuffer::with_pending_timeout(Duration::from_millis(10));
        let events = buffer.process_at("\x1b[200~hello", start);
        assert!(events.is_empty());
        assert!(
            !buffer.has_pending_residual(),
            "paste content should not be subject to the escape-flush timeout"
        );
    }

    #[test]
    fn complete_inputs_clear_pending_state() {
        let start = Instant::now();
        let mut buffer = StdinBuffer::with_pending_timeout(Duration::from_millis(10));
        let events = buffer.process_at("abc", start);
        assert_eq!(events.len(), 3);
        assert!(!buffer.has_pending_residual());
        assert!(buffer.pending_timeout_at(start).is_none());
    }
}
