---
phase: 08
slug: client-connection-replay-and-scoped-control
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-07-13
---

# Phase 08 - Validation Strategy

Runtime status remains pending until implementation executes these commands. This document records planning-time structure, not checker or runtime success.

## Test Infrastructure

| Property | Value |
|----------|-------|
| Framework | Cargo test, Rust built-in harness, Tokio tests |
| Quick run | `cargo test -p pi-coding-agent --lib client_projection` |
| Focused contract | `cargo test -p pi-coding-agent --test public_api --test protocol_events` |
| Full suite | `cargo test --workspace` |
| Feedback target | Per-task focused command under 30 seconds; split filters if measured slower |

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 08-01-T1 | 08-01 | 1 | CLIENT-01, CLIENT-02, CLIENT-03, CONTROL-01 | T-08-01-API-LEAK | RED public contract and privacy boundary forbid a second dispatcher/private authority. | contract/source | `cargo test -p pi-coding-agent --test public_api --test api_boundary_guards client_contract --quiet` | present in plan | pending |
| 08-01-T2 | 08-01 | 1 | CLIENT-01, CLIENT-02, CLIENT-03, CONTROL-01 | T-08-01-AMBIGUITY | Typed drafts/recovery/submitted/control and rejection ownership are exhaustive. | projection/API | `cargo test -p pi-coding-agent --lib client_projection --quiet && cargo test -p pi-coding-agent --test public_api client_contract --quiet` | present in plan | pending |
| 08-01-T3 | 08-01 | 1 | CLIENT-01, CLIENT-02, CLIENT-03, CONTROL-01 | T-08-01-API-LEAK | Curated exports keep internals private and run canonical. | facade/source | `cargo test -p pi-coding-agent --test public_api --test api_boundary_guards --quiet` | present in plan | pending |
| 08-02-T1 | 08-02 | 2 | CLIENT-03 | T-08-STALE-HANDLE, T-08-RESOURCE-BOUND | Generation/draft/ack/submitted/capacity invariants are RED-first. | state unit | `cargo test -p pi-coding-agent --lib client_service --quiet` | present in plan | pending |
| 08-02-T2 | 08-02 | 2 | CLIENT-03 | T-08-RESOURCE-BOUND | SnapshotState transitions and zero-authority facade preserve accepted receipts. | state unit | `cargo test -p pi-coding-agent --lib client_service --quiet && cargo test -p pi-coding-agent --lib client_projection --quiet` | present in plan | pending |
| 08-02-T3 | 08-02 | 2 | CLIENT-03 | T-08-02-CROSS-SESSION | Every constructor shares one coordinator state and no duplicate map. | owner/concurrency | `cargo test -p pi-coding-agent --lib client_service --quiet` | present in plan | pending |
| 08-03-T1 | 08-03 | 3 | CLIENT-01, CLIENT-02 | T-08-REPLAY-GAP | Atomic replay/live boundary preserves sequence authority. | recovery unit | `cargo test -p pi-coding-agent --lib coding_session::event_service::tests::recovery --quiet` | present in plan | pending |
| 08-03-T2 | 08-03 | 3 | CLIENT-01, CLIENT-02 | T-08-03-LAG-CONFLATION | Retained gap and live lag remain typed and distinct. | event unit | `cargo test -p pi-coding-agent --lib coding_session::event_service::tests --quiet` | present in plan | pending |
| 08-04-T1 | 08-04 | 4 | CLIENT-01, CLIENT-02, CLIENT-03 | T-08-04-DUPLICATE | Sole SnapshotState and stateless ClientService topology are guarded. | topology unit/API | `cargo test -p pi-coding-agent --lib snapshot_coordinator --quiet && cargo test -p pi-coding-agent --lib client_service --quiet && cargo test -p pi-coding-agent --test public_api snapshot_topology --quiet` | present in plan | pending |
| 08-04-T2 | 08-04 | 4 | CLIENT-01, CLIENT-02, CLIENT-03 | T-08-04-DEADLOCK, T-08-04-ORDER | Six writer algorithms have deadlock-timeout and mixed-revision assertions. | concurrency/API | `cargo test -p pi-coding-agent --test public_api snapshot_writers --quiet && cargo test -p pi-coding-agent --lib coding_session::event_service::tests --quiet && cargo test -p pi-coding-agent --lib coding_session::operation_control::tests --quiet` | present in plan | pending |
| 08-05-T1 | 08-05 | 5 | CLIENT-01, CLIENT-02, CLIENT-03 | T-08-05-COMPAT | Public recovery plus no-lease Prompt/non-Prompt adapter compatibility is RED-first. | public/adapter | `cargo test -p pi-coding-agent --test public_api client_connection --quiet && cargo test -p pi-coding-agent --test public_api legacy_run --quiet && cargo test -p pi-coding-agent --test api_boundary_guards --quiet` | present in plan | pending |
| 08-05-T2 | 08-05 | 5 | CLIENT-01, CLIENT-02, CLIENT-03 | T-08-05-STALE | Arc-backed connection and exceptional error codes reject stale/capacity failures. | public/error | `cargo test -p pi-coding-agent --test public_api client_connection --quiet && cargo test -p pi-coding-agent --test public_api client_errors --quiet && cargo test -p pi-coding-agent --lib coding_session::error::tests --quiet` | present in plan | pending |
| 08-05-T3 | 08-05 | 5 | CLIENT-03 | T-08-05-PROVENANCE | Lease abandonment, cancellation, takeover, mismatch, double-consume, commit, and terminal failure are exact. | lease unit/API | `cargo test -p pi-coding-agent --test public_api submission_lease --quiet && cargo test -p pi-coding-agent --test public_api legacy_run --quiet && cargo test -p pi-coding-agent --lib coding_session::tests::submission_commit --quiet` | present in plan | pending |
| 08-06-T1 | 08-06 | 6 | CLIENT-03, CONTROL-01 | T-08-CONTROL-AUTH, T-08-CONTROL-REPLAY | Owner/target/signature/conflict/retry/order cases are RED-first. | control API/unit | `cargo test -p pi-coding-agent --test public_api scoped_control --quiet && cargo test -p pi-coding-agent --lib operation_control --quiet` | present in plan | pending |
| 08-06-T2 | 08-06 | 6 | CLIENT-03, CONTROL-01 | T-08-06-QUEUE, T-08-CONTROL-REPLAY | Key-first receipt admission, capacity, channel, FIFO send, and draft rules pass. | control integration | `cargo test -p pi-coding-agent --test public_api scoped_control --quiet && cargo test -p pi-coding-agent --lib operation_control --quiet && cargo test -p pi-coding-agent --lib client_service --quiet` | present in plan | pending |
| 08-07-T1 | 08-07 | 7 | CLIENT-01, CLIENT-02, CLIENT-03, CONTROL-01 | T-08-07-RPC-SPOOF, T-08-07-MIRROR-DRIFT | RPC wire/event parity is frozen before mirror removal. | RPC integration | `cargo test -p pi-coding-agent --test rpc_mode client_connection --quiet && cargo test -p pi-coding-agent --test protocol_events --quiet` | present in plan | pending |
| 08-07-T2 | 08-07 | 7 | CLIENT-01, CLIENT-02, CLIENT-03, CONTROL-01 | T-08-07-MIRROR-DRIFT, T-08-07-LEAK | RPC delegates to connection and retains only adapter-local projection state. | protocol integration | `cargo test -p pi-coding-agent --test rpc_mode --quiet && cargo test -p pi-coding-agent --test protocol_events --quiet && cargo test -p pi-coding-agent --lib protocol::rpc --quiet` | present in plan | pending |
| 08-07-T3 | 08-07 | 7 | CLIENT-01, CLIENT-02, CLIENT-03, CONTROL-01 | T-08-07-LEAK, T-08-07-INPUT | Source guards and full workspace closure cover every requirement/threat. | full closure | `cargo fmt --check && cargo test -p pi-coding-agent --test public_api --test protocol_events --test rpc_mode --test api_boundary_guards --test product_runtime_boundary_guards --quiet && cargo test -p pi-coding-agent --quiet && cargo test --workspace --quiet && cargo check --workspace && git diff --check` | present in plan | pending |

## Planning-Time Mechanical Cross-Check

The expected ordered task-id array is explicit and derived from the seven final plan files: `['08-01-T1','08-01-T2','08-01-T3','08-02-T1','08-02-T2','08-02-T3','08-03-T1','08-03-T2','08-04-T1','08-04-T2','08-05-T1','08-05-T2','08-05-T3','08-06-T1','08-06-T2','08-07-T1','08-07-T2','08-07-T3']`.

Run:

`node -e 'const f=require("fs"),d=".planning/phases/08-client-connection-replay-and-scoped-control",e=["08-01-T1","08-01-T2","08-01-T3","08-02-T1","08-02-T2","08-02-T3","08-03-T1","08-03-T2","08-04-T1","08-04-T2","08-05-T1","08-05-T2","08-05-T3","08-06-T1","08-06-T2","08-07-T1","08-07-T2","08-07-T3"];let t=[];f.readdirSync(d).filter(x=>/^08-\d\d-PLAN\.md$/.test(x)).sort().forEach(x=>{let s=f.readFileSync(d+"/"+x,"utf8"),p=x.slice(0,5),w=s.match(/^wave: (\d+)/m)[1],r=/<task\b[\s\S]*?<name>Task (\d+):[\s\S]*?<verify><automated>([\s\S]*?)<\/automated><\/verify>/g,m;while(m=r.exec(s))t.push([p+"-T"+m[1],p,w,m[2].replace(/\s+/g," ").trim()])});let rows=f.readFileSync(d+"/08-VALIDATION.md","utf8").split("\n").filter(x=>/^\| 08-\d\d-T\d /.test(x)).map(x=>x.split("|").slice(1,-1).map(y=>y.trim())),eq=(a,b)=>JSON.stringify(a)===JSON.stringify(b);if(!eq(t.map(x=>x[0]),e))throw Error("PLAN ids");if(!eq(rows.map(x=>x[0]),e)||new Set(rows.map(x=>x[0])).size!==e.length)throw Error("table ids");if(rows.some(x=>x.length!==10))throw Error("columns");rows.forEach((r,i)=>{if(r[1]!==t[i][1]||r[2]!==t[i][2])throw Error("plan/wave "+r[0]);if(r[7].slice(1,-1).replace(/\s+/g," ").trim()!==t[i][3])throw Error("command "+r[0])});console.log("18 exact ordered tasks; 10 columns; plan/wave/commands exact")'`

Receipt-language closure audit (historical marker line is excluded, but no copyable old implementation is allowed):

`! rg -n 'FIFO eviction|pop_front|directly reusable' .planning/phases/08-client-connection-replay-and-scoped-control/08-{RESEARCH,0[1-7]-PLAN}.md && ! rg -n 'FIFO eviction|pop_front|directly reusable' .planning/phases/08-client-connection-replay-and-scoped-control/08-PATTERNS.md && node -e 'const f=require("fs"),d=".planning/phases/08-client-connection-replay-and-scoped-control";for(const x of ["08-PATTERNS.md","08-RESEARCH.md",...Array.from({length:7},(_,i)=>`08-0${i+1}-PLAN.md`)])for(const line of f.readFileSync(`${d}/${x}`,"utf8").split("\\n")){const t=line.search(/target-running|channel-open|volatile[^;,.]{0,40}(target|channel)/i),r=line.search(/receipt lookup/i);if(t>=0&&r>=0&&t<r)throw Error(`volatile-before-receipt ${x}`)}console.log("receipt instructions authoritative")'`

## Required Scenario Matrix

- Atomic emit-during-recovery, at-least-once reconnect, explicit acknowledgement, retained gap, and live lag.
- Same-id takeover with stale old snapshot/draft/ack/control/lease; distinct client denial.
- One authoritative SnapshotState and the six writer algorithms, each with deadlock timeout and mixed-state checks.
- Complete typed drafts/submitted state; terminal remains until matching acknowledgement.
- No-lease Prompt and non-Prompt public/adapter compatibility with no client-state mutation.
- Abandoned/precommit/postcommit cancellation, wrapper-drop-after-consume, mismatch, admission error, double consume, next matching run, and provider/flow/persistence failure.
- Scoped receipt identical retry, payload conflict, capacity-before-target, post-target response-loss retry, distinct IDs, ordered Abort/Steer/FollowUp, and draft preservation.
- RPC wire/JSON/event ordering, PartialCommit, navigation, and overflow parity.

## Security Verification

All high threats have task-bound automated rows: stale generation, control spoof/replay, replay loss, resource bounds, duplicate authority, deadlock/mixed revision, provenance cancellation, API leakage, and RPC mirror drift.

## Sign-Off

- [ ] Every task command has executed and its row status is updated from pending.
- [ ] Exact-array mechanical cross-check passes after final plan edit.
- [ ] Receipt-language closure audit passes.
- [ ] All D-01 through D-21 and CLIENT-01/02/03 plus CONTROL-01 remain covered.
- [ ] Phase 9 exclusions remain absent.
- [ ] `wave_0_complete` changes only after intended RED failures are observed.

**Approval:** pending
