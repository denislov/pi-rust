---
status: testing
phase: 03-production-adapter-convergence
source: [03-VERIFICATION.md]
started: 2026-07-12T10:19:57Z
updated: 2026-07-12T10:19:57Z
---

## Current Test

number: 1
name: Real interactive operation-failure owner continuity
expected: |
  A real profile mutation, delegation rejection, or prompt runner failure returns the
  same live owner and exact CliError through PromptTask.done. finish_prompt restores
  the owner before projecting the error, and a subsequent canonical operation succeeds.
  At least one case preserves an explicit PartialCommit error unchanged.
awaiting: user response

## Tests

### 1. Real interactive operation-failure owner continuity
expected: A real post-acquisition profile, rejection, or prompt failure carries the same owner and exact error through the task channel; finish_prompt restores it before error projection; a subsequent canonical operation succeeds; one case covers PartialCommit.
result: pending

### 2. Real ForkSession failure continuity
expected: A deterministic real ForkSession failure preserves the pre-fork owner, subscriber continuity, and old session target; finish_prompt projects the exact error and the restored session remains usable without opening a replacement owner.
result: pending

## Summary

total: 2
passed: 0
issues: 0
pending: 2
skipped: 0
blocked: 0

## Gaps

