---
status: findings
findings:
  blocker: 1
  warning: 2
  info: 0
  total: 3
---

# Phase 05 Code Review

## BLOCKER

### 1. Session method inventory silently misses valid `impl` formatting

`crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs:968-1019` only enters the owner implementation when a line both starts with `impl CodingAgentSession` and contains `{`. It therefore misses valid Rust such as:

```rust
impl CodingAgentSession
{
    pub async fn prompt(...) { ... }
}
```

The same line-oriented parser also requires a complete visible method signature on one line. A multiline `pub async fn`/argument list can be skipped. In either case the absence ledger can report the deleted method as absent and the retained-method ledger can report zero definitions while production still exposes a compatibility method. Because this is the primary guard for `CodingAgentSession`'s public boundary, a formatting-only change can create a false negative and allow the exact regression Phase 5 is intended to prevent. Use a token/brace-aware parser (or compiler-backed inspection) that accepts whitespace/newline variation and multiline signatures, with fixtures for both forms.

## WARNING

### 2. Adapter inventory does not assert that every discovered path is uniquely represented by the declared roots

`adapter_inventory_is_recursive_and_receiver_aware` checks duplicate insertion while iterating the three roots, but it does not compare the resulting set against an independently enumerated source set. The current roots are recursive, so a future adapter added under another first-party adapter directory can remain completely unowned while the test still passes; the two hard-coded `contains` checks only cover one interactive and one RPC file. Add an independent expected-root/path assertion (or a repository-level adapter-directory inventory) so an adapter root cannot silently be omitted.

### 3. Negative fixture success is not tied to the fixture's own diagnostic

`crates/pi-coding-agent/tests/api_boundary_guards.rs:144-170` accepts any compiler output containing `error[E0432]` or `error[E0603]`. It does not verify that the diagnostic points at the copied fixture source or names the forbidden symbol/category. A dependency/build regression that emits one of those codes can make a negative fixture appear to enforce privacy even when its import is no longer testing the intended path. Preserve the category-level diagnostic requirement, but also assert the fixture filename and at least one forbidden symbol appear in stderr for each case.

