# Architecture Principles

## Status

Normative for the `0.4.x` architecture train. Existing code that differs is
current-state debt owned by a version task; it is not an alternate contract.

## Invariants

1. Dependencies point from products toward reusable lower layers:
   `pi-coding-agent -> pi-agent-core -> pi-ai`, plus
   `pi-coding-agent -> pi-ai` and `pi-coding-agent -> pi-tui`.
2. Durable coding-session truth is a Rust-native `SessionEvent` log. Manifests,
   indexes, ProductEvents, snapshots, and UI projections are derived or
   independently versioned artifacts, never a second session truth.
3. One admitted root operation has one immutable identity and stable lineage
   across permits, transactions, facts, ProductEvents, outcomes, snapshots,
   controls, recovery, and adapters.
4. One owner has terminal authority. Workflows, adapters, event fan-out, session
   storage, and projections cannot independently decide or synthesize a terminal.
5. One persistent session has one bounded logical writer. Different sessions and
   non-session work remain concurrent; provider, tool, extension, publication,
   and client work stays outside the writer consistency point.
6. Durable success follows required commit. Unknown commit state is represented
   as durable recovery evidence and a non-terminal `RecoveryPending` state, never
   a speculative success or failure.
7. Product adapters consume typed intents, ProductEvents, snapshots, and narrow
   public APIs. They do not start internal Flow nodes, mutate repositories, call
   providers, or repair durable state.
8. Capabilities are explicit, scoped, generation-aware, revocable, and frozen for
   each admitted operation. Possession of a dependency never transfers its
   authority.
9. Third-party code receives schemas and bounded handles, not Rust service
   containers, repositories, provider clients, raw channels, mutable operation
   contexts, or adapter state.
10. Correctness, cancellation, pressure, redaction, offline determinism, and
    compatibility decisions are release gates, not optional hardening after a
    feature ships.

## Owner Versus Collaborator

An owner controls mutation, lifetime, and recovery for one concern. A
collaborator receives an immutable value, typed command, snapshot, lease, or
bounded handle. Passing a whole mutable runtime object to avoid defining an
interface violates ownership even when crate dependencies remain acyclic.

`RuntimeHost` is a composition and lifetime root. It constructs owners, wires
narrow collaborators, coordinates shutdown, and exposes supported facades. It is
not a service locator available to workflows, extensions, adapters, or clients.

## Facts, Events, And Views

```text
SessionEvent --project--> committed ProductEvent --apply--> client projection
live runtime ------------> live ProductEvent ------apply--> transient overlay
```

Live optimistic output cannot enter committed history. Client-local draft,
cursor, selection, focus, scroll, viewport, IME, and unsubmitted form state are
disposable and never written as session facts.

## Change Rule

Breaking an unstable internal or extension contract is allowed during the
pre-product train when the owning plan and ADR say so. Supported durable user data
and public product protocols always require an explicit decoder/migration/support
decision and evidence. "No compatibility shim" never authorizes deletion of an
owned durable decoder inside its declared support window.
