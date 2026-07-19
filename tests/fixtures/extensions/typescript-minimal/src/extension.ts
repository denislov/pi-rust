import {
  defineExtension,
  jsonOutput,
  type Invocation,
} from "../../../../../sdk/typescript/src/index.js";
import { interact } from "pi:extension/host-ui@0.1.0";

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
    const hostEcho = interact("fixture.echo", request.input);
    return jsonOutput({
      handlerId: request.handlerId,
      inputBytes: request.input.byteLength,
      hostEcho: new TextDecoder().decode(hostEcho),
    });
  },
});

export const guest = extension;
