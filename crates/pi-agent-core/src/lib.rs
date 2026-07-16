mod agent;
mod compaction;
mod context;
mod execution;
mod flow;
mod hooks;
mod resources;
#[cfg(any(test, feature = "test-support"))]
mod testing;
mod transcript;

/// Stable low-level runtime facade for `pi-agent-core`.
///
/// Product session ownership, adapter wire events, and workflow ownership belong
/// in `pi-coding-agent`. This module intentionally exposes low-level agent,
/// tool, hook, resource, and environment contracts.
pub mod api;
