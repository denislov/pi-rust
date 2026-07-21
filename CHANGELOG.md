# Changes

## 0.5.3 - 2026-07-21

### Fullscreen TUI Runtime Hardening

- Converged fullscreen interaction on one canonical client
  snapshot/replay/live/ack connection and one typed running/idle event loop.
- Bounded stdin and operation controls, removed the independent progress writer,
  added signal-driven resize, and hardened synchronized render/terminal cleanup
  across injected I/O failures and panics.
- Added viewport-first transcript rendering with fixed steady-frame work for
  1,000/10,000-block histories, deterministic baseline and 20-iteration soak
  gates, and tmux lifecycle coverage.
- Reduced `dead_code` allowances from 68 to 23, with zero unreasoned exceptions;
  provider wire ordering/type fields now fail closed and obsolete DTO/test
  helpers were deleted.
- Preserved RPC `2.1`, ProductEvent `2.2`, UI snapshot `2.1`, crate boundaries,
  and the explicitly skipped Extension contribution-dispatch decision.

## 0.5.2 - 2026-07-20

### `pi-coding-agent` Lean Product-Runtime Convergence

Completed `PCLR-001` through `PCLR-011` and released all workspace packages at
`0.5.2`.

- Completed `PCLR-001`: captured post-`0.5.1` production/test/API/dependency/
  build/runtime baselines under `target/perf-baseline/0.5.2-coding-agent/`.
  Non-test `src/` baseline was 108,017 lines across 169 files; 292 API symbols
  across 29 `pub use` statements; 477 dependency tree nodes; clean
  `cargo check` in 10.84s with 1061 MB RSS. Inventoried the zero-state
  `WorkflowService` (226 lines, 41 usage sites), all 8 runner Step-enum loops,
  30+ adapter impossible-variant `unreachable!()` matches, 4 crate-wide Clippy
  allowances, `debug_assertions` in `test_support` cfg, and the unused
  `test-harness` feature.
- Completed `PCLR-002`: removed the zero-state `WorkflowService` pass-through
  layer (`services/workflow.rs`, 226 lines) and the `RuntimeHost.workflow_service`
  field. Each operation module now invokes its dedicated runner directly and
  normalizes its own outcome/error exactly once. Updated all 41 usage sites
  across `runtime/dispatch.rs`, `runtime/execution.rs`,
  `runtime/facade/lifecycle.rs`, `runtime/facade/view.rs`, `runtime/owners.rs`,
  8 `operations/*/mod.rs` files, `operations/delegation/execution.rs`, and 3
  runner internal subflow/delegation call sites. Preserved cancellation
  propagation, root/sub-operation/structured-child invocation, and session
  write/outbox/event authorization boundaries. No generic runner registry,
  trait object, or service container was introduced.
- Completed `PCLR-003`: converted all fixed-runner `Step enum + loop +
  unreachable!()` patterns into direct typed async pipelines. Export,
  ManualCompaction, BranchSummary, and SelfHealingEdit runners now use direct
  sequential `?` propagation with local `check_cancellation` helpers; their
  `failure_error` fields, `take_failure_error()`, `fail()` methods, and
  `CodingSessionError::Workflow` wrapping were removed. AgentInvocation and
  AgentTeam runners use an async block with unified `fail()` on error to
  preserve the then-current failure terminal semantics required by other
  context methods; that temporary state is removed by `PCLR-009`. PromptTurn's
  dynamic provider/tool/control loop remains specialized. PluginLoad was
  already direct. Removed 6 `Step` enums, 6 `loop` blocks, and 6
  `unreachable!()` sites.
- Completed `PCLR-004`: converged operation descriptor, outcome, terminal, and
  adapter projections. Added typed extraction methods (`into_prompt`,
  `into_compact`, `into_branch_summary`, `into_self_healing_edit`,
  `into_agent_invocation`, `into_agent_team`, `into_plugin_load`, `into_export`,
  `into_export_html`, `into_delegation_approved`, `into_delegation_rejected`,
  `into_default_agent_profile_changed`, `into_session_forked`,
  `into_session_tree_label_changed`) to `CodingAgentOperationOutcome` so that
  each public-to-internal outcome conversion is defined once. Updated all 4
  adapter surfaces (print, JSON, interactive, RPC) to use these methods,
  eliminating 22 repeated operation-family `unreachable!()` matches. Removed
  dead code: `Operation::kind()`, `Operation::origin()`, `Operation::class()`
  moved to `#[cfg(test)]`; `OperationIdempotencyKey::as_str()` moved to
  `#[cfg(test)]`; `OperationOrigin::RuntimeInternal` variant removed; incorrect
  `#[allow(dead_code)]` removed from `OperationOrigin`, `OperationClass`,
  `OperationOutcome::ForkSession`, and `OperationOutcome::SwitchActiveLeaf`.
  Preserved the exhaustive descriptor table as sole metadata authority,
  immutable `OperationExecution`, single terminal authority, and
  durable/versioned ProductEvent terminal semantics.
- Completed `PCLR-005`: contracted the stable product/CLI facade. Deleted the
  empty `api::testing` module. Moved `PromptInvocation` and `PromptRunOptions`
  from `api::cli::runtime` to `api::operation` (operation types). Moved
  `CliRunOptions`, `SessionRunOptions`, and `SessionMode` from `api::cli::runtime`
  to `api::runtime` (runtime types). Moved `PrintModeOptions` and `run_print_mode`
  from `api::cli::print` to `api::protocol` (adapter entrypoint). Removed the
  now-empty `api::cli::runtime` and `api::cli::print` submodules. The final
  release audit then removed low-level `api::cli::{command, configuration,
  input, resources, theme}` implementation categories and added the high-level
  `run_cli_stdio` runner used by the product binary. Owner behavior tests now
  compile against crate-private fixtures. Direct facade exports fell from 292
  to 189 while runtime/operation/event/client/view/protocol/Extension and
  high-level CLI contracts remain supported.
- Completed `PCLR-006`: unified adapter event entrypoints and confirmed outcome
  projection convergence. Removed the parallel `push_internal_product_event`
  entrypoint from `CodingProtocolEventAdapter` (identical to `push_product_event`
  since `ProductEvent` is a type alias for `CodingAgentProductEvent`). Removed
  the parallel `push_public_product_event` entrypoint from `RpcCodingEventAdapter`.
  All 4 adapter surfaces (print, JSON, interactive, RPC) now consume the typed
  `CodingAgentProductEvent` contract through a single `push_product_event` method
  and project `CodingAgentOperationOutcome` through the PCLR-004 typed extraction
  methods without operation-specific impossible-variant matches. Preserved
  adapter-specific presentation and machine wire DTOs, bounded queues,
  sequence/cursor gaps, detach/reconnect/fresh-snapshot recovery, control
  receipts, and stdout/stderr cleanliness.
- Completed `PCLR-007`: replaced the mutually exclusive prompt/follow-up,
  compact, branch-summary, fork, agent, team, self-healing-edit, and default
  profile pending fields with one exhaustive `PendingInteractiveCommand` slot.
  Queueing a new interactive command now replaces the prior command and updates
  its action atomically; the event loop consumes and exhaustively reduces that
  slot once per input dispatch. Authorization decisions and delegation
  confirmations remain independently typed because they may coexist with a
  running operation. Added `InteractiveLocalState` as the TUI-only owner for
  editor/keybinding input state, transcript disclosure/scroll state, render
  cache, focus/context selection, mouse hit regions, and transient overlays,
  removing 21 local fields from `InteractiveRoot`. Moved the ordered
  `UiProjection` into `InteractiveRoot` as the single snapshot/ProductEvent
  projection owner and deleted the event-loop-owned projection plus cloned
  context/capability mirrors. Projection-local timing now captures only prior
  running operation IDs rather than cloning the complete context on every
  streaming delta, preserving render coalescing. Session/model projection
  convergence remains in progress. Moved another 17 model/session/tree/settings
  selection and pending-configuration fields into `InteractiveLocalState`, for
  38 explicitly TUI-local fields removed from the root in total.
  `UiProjection` now also owns the snapshot session view and applies ordered
  default-profile ProductEvents; profile menus, agent context, and the status
  bar read that confirmed projection with a pre-session configuration fallback.
  Confirmed model selection and active session path/leaf remain adapter-owned
  next-run/navigation targets because the public snapshot contract does not
  claim them as shared projection facts.
- Completed `PCLR-008`: replaced the duplicate product size formatter,
  truncation result/type machinery, head implementation, test-only tail
  implementation, and shell tail loop with the frozen
  `pi-agent-core::api::execution` contract. Retained a thin head adapter for the
  established product convention that empty output has zero lines and a
  trailing newline adds no empty line. Read/find/grep/ls limits and notices,
  shell streaming/process control and marker text, filesystem capabilities,
  authorization, and workspace policy remain product-owned. Added an exact
  upstream edge allowlist and a boundary guard against restoring duplicate
  mechanics. Fixed shell truncation metadata for an unterminated long line,
  which previously reported zero total lines; it now reports one line while
  retaining UTF-8-safe tail output.
- Completed `PCLR-009`: removed the unused `test-harness` feature and stopped
  exposing environment/provider mutation helpers in ordinary debug builds;
  helpers now require `cfg(test)` or explicit non-default `test-support`.
  Removed crate-wide Clippy allowances for `result_large_err`,
  `large_enum_variant`, `too_many_arguments`, and `collapsible_if`. Collapsed
  23 nested conditionals, boxed the large self-healing check payload in
  `CodingSessionError`, simplified the RPC queue test helper, and documented
  the remaining exceptions at their exact typed boundary. Added a regression
  guard preventing the feature, debug cfg, and crate-wide allowances from
  returning. Consolidated ProductEvent DTO conversion under the production
  event owner and parameterized shared model/prompt runtime fixtures, removing
  approximately 230 lines of repeated test mapping and builder code while
  retaining distinct behavior matrices. Removed the AgentInvocation,
  AgentTeam, and PluginLoad `failure_error` fields and take-back protocol;
  typed errors now return directly while a separate boolean preserves
  exactly-once failure terminal publication where required.
- Completed `PCLR-010`: re-ran the retained Extension/Wasm security and
  PluginLoad contract. All 33 Extension contract tests, TypeScript SDK/WIT
  conformance, the production Wasm vertical slice, the four-case Wasmtime
  isolation/resource prototype, public trusted-host install/activate/reload,
  and durable PluginLoad restart/outbox evidence pass. The debug cold vertical
  slice completed in 182.23 seconds against the approximately 187 second frozen
  baseline. Wasmtime remains pinned to `46.0.1`, Rust 1.94 remains the minimum,
  and previously Skipped Extension productization work remains Skipped.
- Completed `PCLR-011`: advanced every workspace package to `0.5.2`, added the
  migration guide and release evidence, refreshed current-state architecture,
  froze and verified the 0.5.2 API snapshot, recorded the unchanged RPC 2.1,
  ProductEvent 2.2, and UI snapshot 2.1 inventory, and passed architecture,
  full workspace Clippy/tests, Extension, release, and TUI smoke gates.
- Committed the product workspace and independent public-API snapshot tool
  lockfiles so the release gate's `--locked` API freeze is reproducible from a
  clean checkout. The checked-in API manifest now freezes normalized public API
  surfaces and the toolchain while retaining path-dependent raw rustdoc and
  workspace metadata as diagnostic artifacts rather than unstable baselines.
- Retained the TypeScript/Wasm Extension framework as an explicit product
  decision. `0.5.2` must preserve package integrity, grants/leases, Wasmtime
  isolation, Host-call authorization, PluginLoad durability, and current public
  Extension contracts while performing ordinary cleanup.
- The workspace is released at `0.5.2`; detailed measurements and gate output
  are recorded in `docs/0.5.2-release-evidence.md`.

## 0.5.1 - 2026-07-20

### `pi-agent-core` Lean Runtime Convergence

Completed the `pi-agent-core` lean-runtime convergence, sequenced after `0.5.0`.
Workspace packages are versioned `0.5.1`; architecture, API, workspace, and TUI
release gates pass.

- Completed `ACLR-001`: captured post-`0.5.0` source, API, dependency,
  copy/allocation, and test baselines under `target/perf-baseline/0.5.1-agent-core/`.
  Non-`test-support` source baseline was 8,160 lines; `test-support` was 1,852
  lines; 197 test cases passed; `reqwest` was a direct dependency used only by
  `testing/proxy.rs`.
- Completed `ACLR-002`: removed the unused core Branch Summary workflow
  alternative. Deleted `compaction/branch.rs` (495 lines) and
  `compaction/branch_error.rs` (32 lines), removed their facade exports and
  branch-summary-only tests, and preserved `serialize_conversation`, token
  estimation, and `summarize_with_provider_streamer` as provider-neutral
  primitives. `pi-coding-agent` remains the sole BranchSummary workflow owner.
- Completed `ACLR-003`: removed the test-only Session Context/Memory/Error
  subsystem. Deleted `context/assembly.rs`, `context/memory.rs`,
  `context/error.rs`, `SessionContext`, `InMemorySessionStorage`, and their
  `api::testing` exports. Preserved `context/conversion.rs` as the
  provider-neutral conversion owner. Updated legacy-session boundary tests to
  assert the subsystem is absent.
- Completed `ACLR-004`: replaced residual string Agent-turn actions/errors with
  exhaustive typed transitions. Deleted all `ACTION_*` string constants,
  `as_str()`, `action(&str)`, and unknown-string transition paths. Introduced a
  private typed `AgentTurnError` for invariant and compaction failures. Made
  state/decision combinations exhaustive at compile time in `transition_from_decision`.
  Removed node functions, `AgentTurnContext`, `PendingToolCall`,
  `RuntimeCompactionState`, and decisions from the `test-support` facade.
- Completed `ACLR-005`: made turn commit consuming and reduced state/history
  cloning. Deleted the unused `AgentTurnContext.events` mirror; production
  events now flow through the Agent stream once. `apply_to_state` now uses
  `std::mem::take` for messages and queues instead of cloning. Removed the
  duplicate `resources` field from `AgentTurnContext` (`config.resources`
  remains the single owner). `emit` no longer clones events.
- Completed `ACLR-006`: deleted the parallel test Harness/Proxy runtime.
  Removed `testing/harness.rs` (1,000 lines), `testing/proxy.rs` (430 lines),
  and `testing/error.rs` (44 lines). Removed the direct `reqwest` dependency
  from `pi-agent-core`. Retained `InMemoryExecutionEnv` under non-default
  `test-support`. `test-support` implementation reduced by 79.7% (1,852 to 375
  lines).
- Completed `ACLR-007`: consolidated shared resource discovery/read/provenance/
  diagnostic mechanics. Extracted `read_resource_file` and
  `parse_frontmatter_at_path` shared helpers used by both skills and prompt
  templates. Preserved separately typed Skill and PromptTemplate naming,
  validation, collision, and invocation semantics.
- Completed `ACLR-008`: contracted and recategorized the stable facade. Moved
  `TreeFilterMode` to `pi-coding-agent` (`/tree` selector filtering is product
  presentation policy). Removed deleted branch/session/node/harness/proxy
  contracts from the facade. Added `BeforeProviderRequestHook` to `api::agent`.
- Completed `ACLR-009`: migrated downstream consumers, advanced every workspace
  package to `0.5.1`, updated root and crate changelogs, added
  `docs/0.5.1-migration-guide.md`, updated boundary allowlists and architecture
  ownership tests. `pi-coding-agent` imports `TreeFilterMode` from its own
  `tree_selector` module instead of `pi_agent_core::api::transcript`.

### Release Evidence

- Generated and verified `docs/api-snapshots/0.5.1/SHA256SUMS`, refreshed the
  current architecture evidence, and updated the default release gate to
  validate version `0.5.1` against its own API manifest.
- Passed formatting, full workspace Clippy and tests, architecture gates,
  binary version validation, TUI smoke, and `git diff --check` through the
  default `scripts/release-gates.sh` entry point.

## 0.5.0 - 2026-07-20

### Provider Runtime

- Added the `pi-ai` lean-runtime version plan. The plan removes Bedrock/AWS
  support and its model catalog records, converges repeated retained-provider
  stream mechanics, tightens implementation visibility, and requires migration,
  API-snapshot, architecture, and downstream boundary evidence before release.
- Completed the 0.5.0 task ledger, advanced every workspace package, froze the
  public API snapshot, and passed the complete offline release gate.
- Removed the complete Bedrock/AWS runtime and authentication surface, all 90
  Bedrock catalog records, and the private test-only image-generation mapper.
  The retained catalog contains 831 records and the scoped built-in provider
  matrix remains green offline.
- Removed downstream Bedrock option-patch fields and added migration/source
  boundary evidence. Workspace version advancement remains deferred until the
  remaining stream convergence, facade audit, release snapshot, and full gates
  close.

## 0.4.2 - 2026-07-20

### Extension Kernel Replacement

- Started `EKR-001`: accepted ADR-007 package quarantine and ADR-008 independent
  contract versioning, published hashed Manifest v2/contribution/WIT candidates,
  and added a strict internal Manifest v2 parser and offline contract gate.
- Added dependency lock v1 and strict directory quarantine candidates. Lock
  validation binds exact dependency versions/digests to Manifest requirements;
  quarantine rejects unsafe or ambiguous layouts and verifies component/resource
  integrity before immutable installation.
- Added the candidate immutable package store: store-owned staging is revalidated,
  dependencies are matched by exact package digest and identity, complete package
  bytes are content-addressed, and installation atomically publishes read-only,
  idempotently reloadable package trees.
- Completed `EKR-001` and corrected its downstream ownership: grant-backed
  permission/activation checks are owned by `EKR-003`, while Wasm Component
  import/export validation is owned by `EKR-004`; the accepted ADR-007
  fail-closed pipeline remains unchanged across those owners.
- Completed `EKR-002` with a locked private TypeScript SDK precursor and an
  offline harness that generates WIT/schema declarations, strict-typechecks,
  bundles, componentizes with ambient WASI disabled, validates embedded WIT and
  forbidden loading behavior, and hashes all inputs/outputs under project
  `target/extension-sdk/`.
- Completed `EKR-003` with independently versioned GrantRecord/workspace activation
  contracts, a host-owned permission catalog, per-instance generations and
  operation leases, revoke/cancel/deadline/scope/late-result checks, and durable
  grant-backed activation over immutable packages. Trusted embedding APIs now
  provide store-owned staging, installation, and activation; PluginLoad restores
  the bounded `0600` activation record on restart without transferring dependency
  permissions. Lease-only workspace/model/structured-process/UI Host handles,
  generation-bound registration closure, active revoke cancellation, late-result
  fencing, bounded inputs, and redacted Debug surfaces complete the authorization
  boundary. This closes `EKR-D009`; `EKR-004` owns real guest dispatch.
- Completed `EKR-006` and accepted ADR-012. Shared contribution projections now
  distinguish product-owned `CoreHandlerRef` values from immutable,
  package-bound `ExtensionHandlerRef` values. Manifest activation can only
  produce extension targets, fails closed on malformed projection, and exposes
  neither core-handler addressing nor raw Rust authority. `EKR-004` consumes
  the extension branch; full `EKR-005` contribution parity is Skipped.
- Completed the reduced-scope `EKR-004` minimum Wasm framework with pinned
  Wasmtime `46.0.1`, authoritative WIT-generated async bindings, pre-admission
  package-digest Component preparation, fresh per-invocation Store/Instance,
  epoch/fuel/memory/deadline/output enforcement, and a real TypeScript fixture
  calling the lease-backed UI Host boundary. This raises the MSRV to Rust 1.94.
- Reduced later Extension scope: contribution parity, advanced services/state,
  extension-driven Workbench, full Extension DX, and full runtime baselines are
  explicitly Skipped.
- Completed `EKR-007`: removed `mlua`, Lua discovery/execution, runtime/source
  selection, native Rust contribution-provider traits and registries, and their
  compatibility fixtures. PluginLoad now reloads only durable Wasm activation;
  the empty PluginService, prompt-hook plumbing, and plugin capability set are
  also gone.
- Completed `CLC-042-001`: removed the generic `pi-agent-core` Flow engine and
  public facade, replaced the remaining agent action with `AgentTurnDecision`,
  renamed product operation modules to typed runners, and converged
  `FlowService` to `WorkflowService` with absence guards.
- Completed `CLC-042-002`: deleted the unreachable `PluginCommand` public and
  internal operation/outcome, RPC wire command, interactive slash/task/action,
  keybinding/dialog/form paths, adapter-only contribution DTOs, and empty
  `PluginCapabilities` carrier. `PluginLoad` remains the minimum Wasm activation
  reload owner; contribution productization remains Skipped.
- Advanced every workspace package to `0.4.2`; focused Extension conformance,
  architecture/API boundaries, the frozen 0.4.2 API snapshot, strict Clippy,
  full workspace tests, and TUI smoke passed on 2026-07-20.
- Closed the reduced 0.4.x train at `0.4.2`: the `0.4.0` through `0.4.2`
  releases are complete, while the reserved `0.4.3` through `0.4.5` Extension
  release plans and all of their tasks are explicitly Skipped without empty
  package releases or implied implementation evidence.
- Added an architecture-gate check that rejects active task rows, inconsistent
  plan statuses, post-0.4.2 workspace versions, or changelog releases for the
  skipped `0.4.3` through `0.4.5` plans.

## 0.4.1 - 2026-07-19

### Agent And Workflow Convergence

Workspace packages are versioned `0.4.1`; architecture, API snapshot, full
workspace, strict Clippy, provisional runtime baseline, and offline TUI release
gates passed on 2026-07-19.

- Started the 0.4.1 workflow convergence plan and completed `AWC-001` active
  cancellation semantics. Provider streams, provider/context hooks, sequential
  and parallel tools, and tool hooks now have host-enforced cancellation waits.
- Added an active-tool cancellation regression proving a cancelled tool wait
  returns promptly even when the tool emits no progress updates.
- Accepted `ADR-013` and recorded the complete Flow inventory. Generic Flow is
  now explicitly non-durable and retained only for tests/compatibility and the
  temporary AgentTurn migration scaffold; fixed product workflows are assigned
  to typed pipelines or structured concurrency.
- Completed `AWC-005` correctness convergence: concurrent Agent admission now
  returns typed `AgentAdmissionError` values without panicking, empty ToolUse
  terminals fail deterministically, runtime/queue/turn message IDs avoid
  collisions, and Unicode resource limits count characters rather than bytes.
- Closed the 0.4.1 message-identity debt across replay/hydration as well:
  duplicate `Agent::add_message` IDs are normalized and replay hydration now
  has an explicit uniqueness assertion.
- Completed `AWC-002`: the production Agent turn now uses a private exhaustive
  typed state/transition runner with a bounded per-turn state-step guard. The
  generic Flow wrapper remains only for tests and compatibility callers.
- Started `AWC-003` with the ManualCompaction vertical slice: production manual
  compaction now runs through a typed eight-step pipeline with cancellation
  checks between steps while retaining the existing transaction boundary.
- Added the Export typed pipeline vertical slice: production export now uses
  explicit start/replay/view/render/write/completion steps while preserving
  read-only admission and typed invalid-target errors.
- Added the PromptTurn typed pipeline vertical slice: production prompt turns
  now execute explicit request/input/runtime/session/Agent/finalization steps,
  while Agent execution remains delegated to the typed AWC-002 runner.
- Added the AgentInvocation typed pipeline vertical slice: delegated-agent
  production paths now use explicit profile/child-prompt/execution/finalization
  steps, including cancellation-aware execution, and nested PromptTurn uses its
  typed runner. The generic graph remains compatibility-only.
- Added the AgentTeam typed pipeline vertical slice: team production paths now
  use explicit planning, member execution, result collection, merge, and
  finalization steps, with typed PromptTurn children. Member child contexts now
  run with bounded concurrency of two and results are restored in profile order.
- Added the PluginLoad typed pipeline vertical slice: production plugin loading
  now uses explicit discovery, validation, loading, capability registration,
  diagnostics, and finalization steps with boundary cancellation checks. The
  generic graph remains compatibility-only.
- Added the BranchSummary typed pipeline vertical slice: production branch
  summaries now use explicit replay/range/prompt/model/record/finalization steps,
  with provider cancellation propagated through the typed runner. The generic
  graph remains compatibility-only.
- Added the SelfHealingEdit typed pipeline vertical slice: the filesystem edit
  workflow now uses explicit read/propose/validate/apply/check/repair/record
  steps in both service and tool paths. The generic graph remains
  compatibility-only.
- Completed `AWC-003`: all fixed product workflows now use typed production
  pipelines, AgentTeam member work is bounded structured concurrency, and
  workflow-owned contexts/ProductEvents remain the observation boundary instead
  of introducing a replacement generic step observer.
- Completed `AWC-006` compaction convergence: persistent sessions now disable
  ephemeral runtime automatic compaction and direct callers to durable manual
  compaction; non-persistent runtimes retain core automatic compaction. Persistent
  RPC sessions reject `set_auto_compaction=true`, while durable summaries remain
  transaction/outbox backed and restart-hydratable.
- Fixed the canonical navigation workflow stack overflow introduced by the
  converged operation dispatch future: the operation match now crosses an
  explicit heap boundary, and the default-stack durability regression passes.
- Stabilized interactive Ctrl-C coverage for pre-start child cancellation:
  AgentInvocation and AgentTeam now let child contexts consume an already
  cancelled token at their execution boundary, while interactive tests assert
  the observable cancelled/idle/terminal contract without requiring a provider
  stream to have been polled before Ctrl-C arrived.
- Added heap boundaries around recursive delegated Agent/Team execution futures;
  the depth-budget delegation test now passes on the default stack.
- Preserved provider error classification through the typed PromptTurn runner;
  print mode continues to expose provider stop errors as `AgentFailure` rather
  than a generic session failure.
- Stabilized RPC background-operation tests for cancellation races: a blocked
  provider may be dropped by targeted abort, so tests now assert the typed RPC
  cancellation response rather than requiring a provider poll side effect.

## 0.4.0 - 2026-07-19

Workspace packages are versioned `0.4.0`; API/protocol snapshots and the offline
release gates passed on 2026-07-19.

### Architecture And Planning

- Added the dependency-reviewed `0.4.x` release train and independent `0.4.0`
  through `0.4.5` release contracts.
- Split the monolithic architecture document into normative principles, runtime,
  extension-platform, dependency, and testing contracts, plus versioned current
  state, ADRs, and migration evidence.
- Accepted runtime-owner/finalization, `RecoveryPending`, extension grant/lease,
  state/fact, Workbench protocol, and isolated Wasm invocation decisions. Their
  disposable evidence now includes locked real-engine cancellation, epoch, fuel,
  memory/disposal, and TypeScript-to-WIT Component fixtures.

### Runtime Integrity

- Fixed definite-failure terminal publication for AgentInvocation and AgentTeam:
  `DefinitelyFailed` results now retain and publish their root terminal draft,
  matching committed-success publication and preventing child-only failure
  projections.
- Fixed runtime-owned RPC submission finalization when no submission lease is
  present. Background AgentInvocation/AgentTeam operations now always freeze
  their terminal decision and publish the deferred root draft after finalization.
- Closed the post-admission exit debt for runtime-owned roots: success, definite
  failure, and cancellation now all retain supervisor-owned terminal ownership
  through lease-free submission and operation-identity cancellation paths.
- Closed projection root-invention debt: unknown local events no longer create
  running operation roots without admitted kind, terminal evidence, or explicit
  root identity.
- Closed durable publication/snapshot consistency debt: session commit, outbox
  cursor, manifest-failure reopen, and startup redelivery now have explicit
  cross-checks in the session matrix.
- Closed the BranchSummary/SelfHealingEdit lifecycle debt. BranchSummary is
  explicitly outcome-acknowledged without a ProductEvent terminal, while
  SelfHealingEdit requires ProductEvent Completed/Failed/Aborted root evidence.
- Added the reproducible offline `scripts/runtime-baseline.sh` harness and
  `docs/architecture/runtime-baseline-0.4.0.md` evidence for admission, writer
  pressure, session/outbox commit, snapshot/reconnect, and recovery scan paths.
- Refreshed the 0.4.x roadmap status: all current 0.4.0 implementation/debt
  rows, offline reconnect/lag matrices, API snapshots, and release gates are
  closed. Provider/live reconnect stress is deferred to later hardening.
- Corrected `scripts/release-gates.sh` defaults to the `0.4.0` workspace and API
  snapshot baseline; the script no longer points at retired pre-train defaults.

- Added a typed `RecoveryPending` admission rejection. `SessionWriteRoot`
  operations now inspect durable recovery evidence before
  `OperationScheduler::admit` and fail closed for the affected session, while
  read-only/query and explicit recovery-control paths remain usable.
- Added authenticated RPC `recovery_inspect` and `recovery_retry` commands;
  both require `PI_RUST_RPC_AUTH_TOKEN`, and retry honors the existing
  idempotency ledger while preserving durable evidence checks.
- Added authenticated RPC `recovery_resolve`; its durable recovery audit fact
  records `rpc_token` as the authorization subject while trusted-host Rust
  resolution records `trusted_host`.

- Converged PluginLoad on the admitted operation identity and typed
  Completed/Failed/Aborted ProductEvent terminal evidence, with completion after
  capability generation installation.
- Completed `RIF-001` with immutable permit-owned `OperationExecution` identity,
  freezing descriptor revision, origin, capability/session association, and
  resolved root/parent lineage at admission. Durable PluginLoad,
  SelfHealingEdit, and BranchSummary transactions now reuse that admitted
  identity; Agent/Team contexts receive it at construction and obtain nested
  child IDs through the scheduler. Root, child, session-copy correlation, and
  recovery allocation are explicitly separated, while delegation approval facts
  reuse the admitted approval operation ID. Submission and terminal projection
  retain the admitted execution instead of detached identity/descriptor copies.
- Replaced duplicated internal operation metadata values with one exhaustive
  descriptor table and orthogonal validated claims for lineage, session/runtime
  access, priority, capacity, durability, cancellation, children, outcomes, and
  terminal policy.
- Completed `RIF-007` by making scheduler/capability admission consume the
  execution-owned descriptor directly, enforcing descriptor-declared structured
  children, and removing the transitional operation metadata registry.
- Moved external API compile fixtures and their Cargo targets under the project
  `target/api-boundary-fixtures/` tree.
- Began `RIF-008` by making `CodingAgentSession` a facade over one `RuntimeHost`
  with explicit OperationSupervisor, SessionCoordinator, EventHub, and
  ClientProjectionCoordinator ownership. Added the identity-bearing session
  writer command/reply protocol and routed default-profile, fork, active-leaf,
  tree-label, and delegation approval/rejection mutations through it.
- Removed raw SessionLogStore/SessionHandle authority from TurnTransaction;
  workflow-local staging now reaches persistence through typed checkpoint and
  finalize writer commands over a shared bounded transaction actor, preserving
  checkpoint and PartialCommit semantics while rejecting queue saturation and
  closed writers. Authorization request/resolution facts now use the same
  bounded actor instead of retaining raw repository handles. Last writer-handle
  drop and drained RuntimeHost shutdown close and join the actor before shutdown
  publication.
- Made the bounded writer own and refresh its mutable session handle after
  manifest commits. Tree labels, active-leaf changes, default-profile changes,
  delegation durable facts, and startup recovery now submit event-plus-manifest
  mutations through that writer; the live `SessionService` no longer appends or
  patches its own repository handle directly.
- Routed `SessionCreated` and copy/fork target payload installation through
  typed writer commands. Copy provenance, copied facts, and the target manifest
  now form one writer commit, and failed target writers are closed before
  session cleanup.
- Added a canonical-session writer registry with per-open owner leases. Separate
  `SessionService::open()` calls for one session reuse one actor; shutting down
  one owner leaves other owners usable, and the last owner closes and joins the
  actor. Closed actors are never reused by a later open.
- Added a writer-owned manifest snapshot for independent-open reads. Session
  view, tree, summary, active-leaf, and default-profile reads now observe
  successful mutations made by another open owner without relying on a stale
  local manifest handle.
- Removed SessionService handle replacement after writes. Repository handles are
  now read/path authority only; mutable manifest owner state remains in the
  writer, and deterministic tests cover independent-session concurrency while a
  different writer is blocked.
- Closed `RIF-D007`: the owner graph and boundary evidence now show that runtime,
  session, and event mutation authority no longer overlaps through mutable
  service handles.
- Closed `RIF-008`: the per-session writer owner, cross-open registry, immutable
  read authority, bounded pressure behavior, shutdown drain, independent-session
  concurrency, navigation projection ordering, and workspace release gates now
  pass. The next planned slice is `RIF-009` outbox/snapshot consistency.
- Completed `RIF-009-001`: added the typed `DurableOutboxRecord` contract with
  schema/version, semantic identity, session/operation correlation, record kind,
  and structured product draft payload. The retained EventService window remains
  explicitly process-local replay state, not durable outbox proof.
- Completed `RIF-009-002`: session manifests now provision `outbox.jsonl`, writer
  mutation batches accept typed outbox records, and prompt success/failure/abort
  plus non-leaf commit/failure paths write a `SessionWriteCommitted` draft
  correlated to source session event IDs.
  Outbox intent is written before session facts under one append lock, so a
  later fact failure returns `PartialCommit` while leaving restart-visible
  recovery evidence.
- Activated `RIF-009-003` for the committed projection/snapshot cursor. Durable
  operation-terminal and recovery records remain sequenced behind the
  supervisor state machine in `RIF-002`.
- Added the first `RIF-009-003` cursor slice: outbox schema v2 records the
  `committed_through_session_sequence`; transaction code passes an uncommitted
  candidate, and only the repository assigns its durable cursor after validating
  the candidate session and source event IDs against the sequenced fact batch.
- `SessionReplay` now derives and carries the last committed session fact cursor;
  public snapshot cursors expose `last_session_sequence`. Atomic writer to
  read-model handoff is now implemented through typed `SessionCommitReceipt`
  responses and a monotonic SessionService cursor cache; projection refresh no
  longer replays the session log. `RIF-009-003` is complete and `RIF-009-004`
  recovery/redelivery matrix work is active.
- Began the `RIF-009-004` restart slice: opening a session now validates durable
  outbox schema/version, session identity, monotonic commit cursors, source
  event IDs, and duplicate semantic identities before runtime startup.
- Added runtime-local idempotent outbox redelivery: startup records are emitted
  through EventService after `SessionOpened`, and duplicate record IDs are
  suppressed within the runtime.
- Added the first `RIF-009-004` matrix test covering duplicate suppression,
  retained-gap recovery, replay-through ordering, and current-cursor reconnect.
- Added a full restart failure case: manifest failure after outbox/fact append,
  writer shutdown, reopen, replay cursor validation, and startup outbox evidence
  for redelivery.
- Startup scan now commits a durable non-terminal `OperationRecoveryPending`
  fact and `Recovery` outbox record in the same writer batch. Recovery IDs are
  stable per session/operation, replay remains `InDoubt`, and repeated opens
  redeliver the same evidence without duplicating facts or records instead of
  speculatively marking incomplete work recovered. Recovery evidence v1 carries
  record version, descriptor revision, and capability generation through the
  durable fact/outbox, public Rust API/events, and JSON/RPC protocol, with
  defaults for pending facts written before these fields were introduced.
- Added trusted-host `resolve_recovery()` control for durable pending operations.
  Requests must match the recovery/operation identity, record version,
  descriptor revision, and capability generation, and may resolve only to
  Failed or Aborted. The bounded reason is secret-redacted before the writer
  atomically commits the audit fact, terminal fact, terminal marker, and
  `OperationTerminal` outbox; EventHub publishes only after commit and restart
  redelivery preserves the original operation family and terminal status.
- Added bounded trusted-host `retry_recovery()` inspection attempts. Each retry
  appends a pending snapshot and Recovery outbox record with durable attempt
  count and timestamps, caps attempts at three, and never reruns an external
  side effect. `with_backoff()` records deterministic `+1s`, `+2s`, and `+4s`
  next-attempt timestamps. Session startup executes one elapsed inspection
  attempt atomically, preserves the three-attempt cap, and never reruns external
  side effects.
- Began `RIF-002`: `OperationSupervisor` now owns typed immutable
  `FinalizationDecision` creation for every dispatch path. Decisions freeze the
  admitted identity, lineage, descriptor, capability generation, terminal
  policy/class, semantic event ID, and safe payload; submission projection
  validates the decision instead of classifying a detached status. Typed
  Prompt/Compact/BranchSummary failure outcomes now correctly resolve Failed
  rather than Completed.
- Partial commit paths with durable fact/outbox evidence now resolve to a stable
  `InDoubt(recovery_id)` result and project non-terminal `RecoveryPending`
  status. EventHub and protocol adapters emit `OperationRecoveryPending` with
  no `terminal_status` or `terminal_operation`; when evidence is absent, the
  original `PartialCommit` error is preserved and the handoff fails closed.
- Added the first normal terminal persistence slice for Prompt: the coordinator
  writes an `operation.terminal.recorded` SessionEvent and `OperationTerminal`
  outbox draft atomically, publishes the Prompt terminal only after that commit,
  and restart redelivery reconstructs the Prompt root terminal metadata.
  Compact success/failure now use the same terminal fact/outbox batch, publish
  only after commit, and reconstruct `Compact` terminal metadata after restart.
  Operation-terminal outbox records carry an optional operation-kind recovery
  hint so shared payloads retain their admitted family. PluginLoad
  success/failure/abort now use the same coordinator path and restart as typed
  PluginLoad terminals. SelfHealingEdit success/failure/abort now follow the
  same commit-before-publication and typed restart path; cancellation arriving
  after Flow success no longer drops its session transaction. BranchSummary is
  intentionally outcome-acknowledged, while SelfHealingEdit uses typed terminal
  evidence. Standalone
  AgentInvocation/AgentTeam roots now publish terminal ProductEvents after
  supervisor finalization while delegated child ownership remains explicit.
  Added a read-only `recovery_pending()` session inspection surface that reports
  stable recovery IDs and persisted operation kinds without synthesizing a
  terminal outcome. Authenticated inspect/retry/resolve operator controls now
  persist evidence and audit authority.

### Release Status

- `RIF-001`, `RIF-003`, `RIF-005`, `RIF-006`, `RIF-007`, `RIF-008`, and
  `RIF-010` are complete. `RIF-004` and `RIF-009` remain in progress.
  Workspace packages remain at `0.3.1` until all `0.4.0` implementation, debt,
  and release gates close.

## 0.3.1 - 2026-07-18

### Added

- Added explicit known/unknown model-cost state and propagated it through
  transcripts, product events, replay, and RPC session statistics.
- Added AWS standard credential-chain and official SigV4 support for Bedrock,
  including named profiles, session credentials, cancellation, and redaction.
- Added a deterministic cross-provider contract matrix for every built-in API.

### Changed

- Provider streams now fail closed on truncation, malformed terminals, timeout,
  cancellation, and provider-declared failure; all HTTP providers share one
  retry, hook, option-validation, cancellation, and deadline implementation.
- SSE and OpenAI Responses parsing now support chunk-safe protocol processing,
  multiple interleaved outputs, structured failures, and safe unknown events.
- Compatibility metadata now exposes explicit request/runtime versus
  catalog-only dispositions, and generated catalog records are validated.
- RPC advances additively to protocol `2.1`; ProductEvent advances additively
  to `2.2`; UI Snapshot remains `2.1`.

### Removed

- Removed the DTO-only standalone image-generation category from the stable
  `pi_ai::api` facade; multimodal conversation images remain supported.
- Removed unused Codex WebSocket helpers. Explicit WebSocket or unknown
  transport selection now fails before network I/O.

### Migration

- See `docs/0.3.1-migration-guide.md` for public API, terminal, cost, transport,
  compatibility, and Bedrock credential changes.

### Release Status

- `PAIR-001` through `PAIR-012` and all completion criteria in
  `docs/0.3.1-pi-ai-remediation-plan.md` are closed.
- All workspace packages use version `0.3.1`; formatting, strict Clippy, full
  workspace tests, and the `0.3.1` public API freeze pass.
- The offline tmux TUI smoke suite was rerun successfully on 2026-07-18; no live
  provider credentials were used.

## 0.3.0 - 2026-07-17

### Added

- Added model-visible delegation target inventories, uniform asynchronous
  operation cancellation, durable session-tree labels, executable plugin UI
  actions, effective configuration handling, and runtime-backed tool
  authorization.
- Added a full-screen interactive application shell with responsive context,
  tips, composer, status, transcript interaction, mouse support, and focused
  overlays while retaining the supported inline path.
- Added truthful runtime-backed RPC capabilities, commands, state, statistics,
  multimodal prompts, compaction, session forks, cancellation, and
  authorization controls.

### Runtime Contracts

- Tool authorization is capability-bounded, cancellation-aware, reconnectable,
  redacted, and durably audited; unresolved requests recover as interrupted
  rather than approved.
- Product events and UI snapshots now project operation, change, delegation,
  usage, cost, context, and pending-authorization state from runtime owners.
- RPC remains protocol family `2.0`; ProductEvent and UI Snapshot advance
  additively to protocol family `2.1`. The durable session writer remains
  version `1`.

### Release Status

- All tasks and execution-debt entries in `docs/0.3-plan.md` are complete and
  closed.
- All workspace packages use the `0.3.0` workspace version.

## 0.2.0 - 2026-07-16

### Changed

- Unified all workspace packages under the root workspace version policy.
- Completed the breaking architecture convergence release train.
- Added reproducible architecture, public API snapshot, compatibility, and
  release gates.

### Boundaries

- The root `pi-rust` binary remains a placeholder and does not own provider,
  agent-runtime, session, product, or terminal UI behavior.
- The user-facing executable remains `pi-coding-agent`.

### Release Artifacts

- RPC, ProductEvent, and UI Snapshot protocol families are version `2.0`.
- The durable session writer remains version `1`.
- Public API freeze manifests are stored under `docs/api-snapshots/`.
- The completed architecture migration and release evidence are summarized in
  `docs/0.2-architecture-convergence-record.md`.
