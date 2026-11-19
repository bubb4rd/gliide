// The specimen's Benchmark overview: the gliide Index, cost, time and history as the reference of Nov 16–17, 2026 had
// them (six routes, three of them partial, one without a price), so the overview and its charts show in every state without the app.

import type { BenchmarkView, IndexRoute, Role } from "$lib/api";

const ROLES: Role[] = ["read", "tailor", "coach"];

function route(
  agent: IndexRoute["agent"],
  model: string,
  parts: [number, number, number],
  needs: Role[],
  seconds: number,
  cost: number | null,
  costEstimated = false,
): IndexRoute {
  return {
    agent,
    model,
    index: parts.reduce((a, b) => a + b, 0) / 3,
    parts: ROLES.map((role, i) => ({
      role,
      value: parts[i],
      placeholder: needs.includes(role),
    })),
    complete: needs.length === 0,
    costUsd: cost,
    costEstimated: costEstimated || (needs.length > 0 && cost !== null),
    seconds,
    secondsEstimated: needs.length > 0,
  };
}

export const benchSample: BenchmarkView = {
  roles: [],
  referenceDate: "2026-11-17",
  run: null,
  keptAverage: null,
  index: {
    roles: ROLES,
    targetS: 58,
    routes: [
      route("claude", "sonnet", [90.3, 46.5, 97.6], [], 36.6, 2.04),
      route("antigravity", "gemini-4-hydrogen-high", [89.7, 39.1, 98.6], [], 173.7, null),
      route("codex", "gpt-6-luna", [85.0, 0, 99.2], [], 59.8, 0.11),
      route("antigravity", "gemini-3.8-flash-low", [89.3, 57.1, 98.7], ["coach"], 18.7, 1.04),
      route("claude", "opus", [89.7, 51.2, 98.7], ["read"], 55.5, 3.84),
      route("claude", "haiku", [90.5, 46.5, 98.7], ["tailor", "coach"], 20.3, 0.1),
    ],
  },
  history: [
    {
      role: "read",
      snapshots: [{ at: "2026-11-17", source: "reference" }],
      routes: [
        { agent: "claude", model: "sonnet", values: [90.3] },
        { agent: "codex", model: "gpt-6-luna", values: [85.0] },
      ],
    },
    {
      role: "tailor",
      snapshots: [
        { at: "2026-11-16", source: "reference" },
        { at: "2026-11-17T18:20:00Z", source: "thisMac" },
        { at: "2026-11-18T16:05:00Z", source: "thisMac" },
      ],
      routes: [
        {
          agent: "antigravity",
          model: "gemini-3.8-flash-low",
          values: [85.5, 85.7, 87.9],
        },
        {
          agent: "antigravity",
          model: "gemini-4-hydrogen-high",
          values: [80.9, null, null],
        },
        { agent: "claude", model: "opus", values: [84.4, 83.7, null] },
        { agent: "claude", model: "sonnet", values: [78.6, 82.2, 80.0] },
        { agent: "codex", model: "gpt-6-luna", values: [65.8, 63.0, 74.6] },
      ],
    },
    { role: "coach", snapshots: [], routes: [] },
  ],
};
