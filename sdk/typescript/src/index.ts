import type {
  Invocation,
  InvocationOutput,
  Registration,
} from "pi:extension/types@0.1.0";

export type { Invocation, InvocationOutput, Registration };

export interface ExtensionDefinition {
  activate(): Registration[];
  invoke(request: Invocation): InvocationOutput;
}

export function defineExtension(
  definition: ExtensionDefinition,
): ExtensionDefinition {
  return Object.freeze(definition);
}

export function encodeJson(value: unknown): Uint8Array {
  return new TextEncoder().encode(JSON.stringify(value));
}

export function decodeJson<T>(bytes: Uint8Array): T {
  return JSON.parse(new TextDecoder().decode(bytes)) as T;
}

export function jsonOutput(value: unknown): InvocationOutput {
  return {
    outputSchemaRevision: 1,
    output: encodeJson(value),
  };
}
