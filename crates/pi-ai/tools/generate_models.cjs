#!/usr/bin/env node
const fs = require("node:fs");
const vm = require("node:vm");

const [inputPath, outputPath] = process.argv.slice(2);
if (!inputPath || !outputPath) {
  console.error(
    "usage: node crates/pi-ai/tools/generate_models.cjs <models.generated.ts> <models_generated.json>",
  );
  process.exit(2);
}

let source = fs.readFileSync(inputPath, "utf8");
source = source.replace(/^import type .*$/gm, "");
source = source.replace(/export const MODELS\s*=\s*/, "const MODELS = ");
source = source.replace(/\s+satisfies\s+Model<[^>]+>/g, "");
source = source.replace(/\s+as\s+const\s*;?\s*/g, ";\n");
source += "\nMODELS;";

const models = vm.runInNewContext(source, {}, { filename: inputPath });

function normalizeModel(m) {
  const model = {
    id: String(m.id),
    name: String(m.name),
    api: String(m.api),
    provider: String(m.provider),
    baseUrl: String(m.baseUrl),
    reasoning: Boolean(m.reasoning),
    input: m.input || ["text"],
    cost: {
      input: Number(m.cost?.input || 0),
      output: Number(m.cost?.output || 0),
      cacheRead: Number(m.cost?.cacheRead || 0),
      cacheWrite: Number(m.cost?.cacheWrite || 0),
    },
    contextWindow: Number(m.contextWindow || 0),
    maxTokens: Number(m.maxTokens || 0),
  };
  if (m.thinkingLevelMap !== undefined) {
    model.thinkingLevelMap = m.thinkingLevelMap;
  }
  if (m.headers !== undefined) {
    model.headers = m.headers;
  }
  if (m.compat !== undefined) {
    model.compat = m.compat;
  }
  return model;
}

const out = [];
for (const provider of Object.keys(models).sort()) {
  for (const id of Object.keys(models[provider]).sort()) {
    out.push(normalizeModel(models[provider][id]));
  }
}

fs.writeFileSync(outputPath, `${JSON.stringify(out, null, 2)}\n`);
