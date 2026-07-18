# Flow Inventory For 0.4.1

This inventory is the evidence for `AWC-004`. It covers production Flow
owners, the generic graph API, and the migration destination for each execution
form. A Flow entry below is not permission to add another dynamic graph: new
production work must use the listed destination.

| Current owner | Execution form | Current role | 0.4.1 decision | Migration owner |
| --- | --- | --- | --- | --- |
| `pi-agent-core::AgentTurnFlow` | dynamic state machine | provider/tool turn loop with branching actions | retain temporarily as the migration scaffold; replace dispatch with typed state/transition enums | `AWC-002` |
| `PromptTurnFlow` | fixed pipeline | prompt preparation, runtime/session open, agent turn, commit/finalization | replace graph construction with typed async pipeline and one typed step observer | `AWC-003` |
| `AgentInvocationFlow` | fixed pipeline | delegated agent invocation | replace graph construction; preserve operation identity and cancellation | `AWC-003` |
| `AgentTeamFlow` | structured concurrency | bounded member execution and join/finalization | replace graph wrapper with typed parent/child scope and structured join | `AWC-003` |
| `PluginLoadFlow` | fixed pipeline | manifest/registry/capability installation | replace graph wrapper; lifecycle remains under the 0.4.0 operation framework | `AWC-003` |
| `ExportFlow` | fixed pipeline | session export/read/format/write sequence | replace graph wrapper with typed sequential steps | `AWC-003` |
| `BranchSummaryFlow` | fixed pipeline | branch summary provider call and acknowledgement | replace graph wrapper; keep cancellation and outcome acknowledgement | `AWC-003` |
| `SelfHealingEditFlow` | fixed pipeline with bounded repair loop | inspect/repair/check/commit edit cycle | replace graph wrapper with an explicit bounded loop and typed outcomes | `AWC-003` |
| `ManualCompactionFlow` | fixed pipeline | summarize, record facts, commit compaction | replace graph wrapper with typed durable pipeline | `AWC-003` |
| `Flow<C>` public generic | in-memory cooperative graph runner | test support, compatibility surface, and temporary AgentTurn scaffold | retain as non-durable internal/test graph API; no new product workflow may depend on it after 0.4.1 | `AWC-004`, then `AWC-002`/`AWC-003` |

## Frozen Rules

1. `Flow<C>` never owns durable state, retries, leases, terminal outcomes,
   session writes, or recovery decisions.
2. A fixed sequence must be expressed as typed async code; a graph is not used
   merely to sequence calls.
3. A dynamic state machine must use typed states/actions at its stable boundary;
   private node labels may remain only as migration diagnostics until the typed
   implementation lands.
4. Structured concurrency uses admitted child operations and bounded joins. It
   does not encode fan-out as graph edges.
5. Cancellation is cooperative at the operation/tool/provider boundary and
   host-enforced at every await point that can otherwise wait indefinitely.
6. `FlowRunOptions` behavior remains explicit: cancellation returns
   `FlowError::Cancelled`; max steps fail deterministically; missing transitions
   are either strict errors or explicit lenient completion; callbacks are
   observational and cannot mutate durable ownership.
7. The generic Flow API remains available for existing tests and compatibility
   until its consumers migrate. This is an owned compatibility surface, not an
   unowned production shim.

## Verification

The inventory is checked against `rg -n "Flow::new|run_with_options"` in the
workspace and the focused `pi-agent-core` Flow tests. Product event boundary
guards continue to require canonical operation entry points for the production
flows listed above.
