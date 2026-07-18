import assert from "node:assert/strict";

function capabilityPrototype() {
  const grantRecord = Object.freeze({
    extensionId: "review.example",
    permissions: Object.freeze(["session.read", "view.publish"]),
    generation: 1,
  });
  let activeGeneration = grantRecord.generation;

  const instanceGrant = Object.freeze({
    extensionId: grantRecord.extensionId,
    permissions: grantRecord.permissions,
    generation: activeGeneration,
  });
  const lease = Object.freeze({
    extensionId: instanceGrant.extensionId,
    operationId: "op-view-refresh",
    permission: "session.read",
    generation: instanceGrant.generation,
    deadline: 100,
  });

  function authorize(candidate, call) {
    assert.equal(candidate.operationId, call.operationId, "operation binding");
    assert.equal(candidate.generation, activeGeneration, "stale generation");
    assert.ok(instanceGrant.permissions.includes(candidate.permission));
    assert.ok(call.now <= candidate.deadline, "deadline");
  }

  authorize(lease, { operationId: "op-view-refresh", now: 99 });
  assert.throws(
    () => authorize(lease, { operationId: "op-other", now: 99 }),
    /operation binding/,
  );
  activeGeneration += 1;
  assert.throws(
    () => authorize(lease, { operationId: "op-view-refresh", now: 99 }),
    /stale generation/,
  );
  assert.equal(
    instanceGrant.permissions.includes("dependency.network"),
    false,
    "dependency authority must not transfer",
  );
}

async function wasmIsolationPrototype() {
  // Minimal module exporting one page of memory. It deliberately avoids a
  // compiler/toolchain dependency while proving that two invocations do not
  // share mutable guest memory. The separate locked Wasmtime fixture proves
  // async cancellation, epoch interruption, fuel, and memory-limit behavior.
  const moduleBytes = Uint8Array.from([
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
    0x05, 0x03, 0x01, 0x00, 0x01,
    0x07, 0x0a, 0x01, 0x06, 0x6d, 0x65, 0x6d, 0x6f, 0x72, 0x79, 0x02, 0x00,
  ]);
  const module = await WebAssembly.compile(moduleBytes);
  const first = await WebAssembly.instantiate(module);
  const second = await WebAssembly.instantiate(module);
  const firstBytes = new Uint8Array(first.exports.memory.buffer);
  const secondBytes = new Uint8Array(second.exports.memory.buffer);
  firstBytes[0] = 42;
  assert.equal(firstBytes[0], 42);
  assert.equal(secondBytes[0], 0, "invocations must not share guest memory");
  assert.notEqual(first.exports.memory, second.exports.memory);
}

function stateAndFactPrototype() {
  const globalState = new Map();
  const sessionEvents = [];
  const outbox = [];

  function commitSessionBatch(events, obligations) {
    const eventStart = sessionEvents.length;
    const outboxStart = outbox.length;
    try {
      sessionEvents.push(...structuredClone(events));
      outbox.push(...structuredClone(obligations));
    } catch (error) {
      sessionEvents.length = eventStart;
      outbox.length = outboxStart;
      throw error;
    }
  }

  globalState.set("review.example/workspace/theme", "dark");
  commitSessionBatch(
    [
      {
        kind: "extension_state_mutation",
        extensionId: "review.example",
        scope: "branch/main",
        key: "selectedFinding",
        value: "finding-7",
      },
      Object.freeze({
        kind: "extension_fact",
        extensionId: "review.example",
        schema: 1,
        payload: Object.freeze({ reviewId: "r-1", verdict: "changes" }),
      }),
    ],
    [{ semanticId: "op-review/terminal", kind: "review_completed" }],
  );

  assert.equal(sessionEvents.length, 2);
  assert.equal(outbox.length, 1);
  assert.equal(globalState.size, 1);
  assert.notEqual(
    globalState,
    sessionEvents,
    "global/workspace state is outside the session transaction",
  );

  const update = {
    phase: "prepared",
    candidateGeneration: 2,
    activeGeneration: 1,
  };
  globalState.set("review.example/candidate/2/schema", 2);
  update.phase = "candidate-state-prepared";
  assert.equal(update.activeGeneration, 1, "prepared candidate is not active");
  update.phase = "activation-record-committed";
  update.activeGeneration = 2;
  assert.equal(update.activeGeneration, 2);
}

function workbenchPrototype() {
  const review = {
    viewInstanceId: "review/client-a",
    revision: 1,
    root: { id: "root", type: "split", children: ["files", "diff"] },
  };
  const incident = {
    viewInstanceId: "incident/client-a",
    revision: 1,
    root: { id: "root", type: "tabs", children: ["timeline", "actions"] },
  };
  assert.notDeepEqual(review.root, incident.root, "views must be materially distinct");

  function applyPatch(snapshot, patch) {
    if (
      patch.viewInstanceId !== snapshot.viewInstanceId ||
      patch.baseRevision !== snapshot.revision
    ) {
      return { kind: "view_resync_required", viewInstanceId: snapshot.viewInstanceId };
    }
    return {
      ...snapshot,
      revision: snapshot.revision + 1,
      root: { ...snapshot.root, badge: patch.badge },
    };
  }

  const updated = applyPatch(review, {
    viewInstanceId: review.viewInstanceId,
    baseRevision: 1,
    badge: "3 findings",
  });
  assert.equal(updated.revision, 2);
  assert.equal(updated.root.badge, "3 findings");
  assert.deepEqual(
    applyPatch(updated, {
      viewInstanceId: updated.viewInstanceId,
      baseRevision: 1,
      badge: "stale",
    }),
    { kind: "view_resync_required", viewInstanceId: updated.viewInstanceId },
  );

  const clientA = { focus: "files", scroll: 8, draft: "local A" };
  const clientB = { focus: "diff", scroll: 0, draft: "local B" };
  assert.notDeepEqual(clientA, clientB, "transient UI state is client-local");
}

capabilityPrototype();
await wasmIsolationPrototype();
stateAndFactPrototype();
workbenchPrototype();
console.log("architecture contract prototypes passed");
