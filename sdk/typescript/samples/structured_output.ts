#!/usr/bin/env -S NODE_NO_WARNINGS=1 pnpm ts-node-esm --files

import { Maplecode } from "@openai/codex-sdk";

import { maplecodePathOverride } from "./helpers.ts";

const maplecode = new Maplecode({ maplecodePathOverride: maplecodePathOverride() });

const thread = maplecode.startThread();

const schema = {
  type: "object",
  properties: {
    summary: { type: "string" },
    status: { type: "string", enum: ["ok", "action_required"] },
  },
  required: ["summary", "status"],
  additionalProperties: false,
} as const;

const turn = await thread.run("Summarize repository status", { outputSchema: schema });
console.log(turn.finalResponse);
