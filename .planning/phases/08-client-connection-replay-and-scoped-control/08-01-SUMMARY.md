---
phase: 08-client-connection-replay-and-scoped-control
plan: 01
status: complete
---

# Phase 08 Plan 01 Summary

## Delivered

- Added stable client connection generation, draft, submitted-operation, replay/recovery, and Prompt-control value contracts.
- Replaced the public count-only snapshot projection with complete typed drafts and submitted-operation state.
- Added an opaque, non-Clone `CodingAgentSubmissionLease` preparation surface while retaining `CodingAgentSession::run` as the ordinary-operation dispatcher.
- Curated all new contracts through `pi_coding_agent::api` and preserved private runtime/service/control ownership.

## Verification

- `cargo fmt --check`
- `cargo test -p pi-coding-agent --lib client_projection --quiet`
- `cargo test -p pi-coding-agent --test public_api --test api_boundary_guards --quiet`
- `cargo check -p pi-coding-agent --all-targets --quiet`
- `git diff --check`

## Deviations

- Existing session/capability/version projection types do not implement Serde, so serialization derives remain limited to standalone new value contracts; no runtime behavior or dependency changes were introduced.
