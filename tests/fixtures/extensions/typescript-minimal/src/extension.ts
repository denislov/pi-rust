import {
  defineExtension,
  jsonOutput,
  type Invocation,
} from "../../../../../sdk/typescript/src/index.js";

const extension = defineExtension({
  activate() {
    return [
      {
        kind: "tools",
        id: "fixture.echo",
        schemaRevision: 1,
        definition: new TextEncoder().encode(
          JSON.stringify({ description: "Echo structured input" }),
        ),
      },
    ];
  },
  invoke(request: Invocation) {
    return jsonOutput({
      handlerId: request.handlerId,
      inputBytes: request.input.byteLength,
    });
  },
});

export const guest = extension;
