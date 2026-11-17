// The agents gliide runs, as the screens name them: who makes each, whose plan its calls run on, its icon (bundled in
// `static/agents/`), where to learn to install it and the binary a path setting points at. In `agent_statuses`'
// order. Then the roles each runs on an agent of its own, and the icon of whoever made a model, for the lines that
// say which model answered.

import type { Agent, Role } from "$lib/api";

export interface AgentInfo {
  id: Agent;
  name: string;
  maker: string;
  /** Whose plan its calls run on, said after what they do: "with your Claude plan". */
  plan: string;
  icon: string;
  install: string;
  /** The binary gliide runs, which a path setting points at. */
  bin: string;
}

export const AGENTS: Record<Agent, AgentInfo> = {
  claude: {
    id: "claude",
    name: "Claude Code",
    maker: "Anthropic",
    plan: "with your Claude plan",
    icon: "/agents/claude.png",
    install: "https://code.claude.com/docs/en/setup",
    bin: "claude",
  },
  codex: {
    id: "codex",
    name: "Codex",
    maker: "OpenAI",
    plan: "with your ChatGPT plan",
    icon: "/agents/openai.png",
    install: "https://developers.openai.com/codex/cli",
    bin: "codex",
  },
  antigravity: {
    id: "antigravity",
    name: "Antigravity",
    maker: "Google",
    plan: "with your Google AI plan",
    icon: "/agents/antigravity.png",
    install: "https://antigravity.google/docs/getting-started",
    bin: "agy",
  },
};

/** The agents in the order Settings and the setup list them. */
export const AGENT_LIST: AgentInfo[] = [AGENTS.claude, AGENTS.codex, AGENTS.antigravity];

export interface RoleInfo {
  id: Role;
  /** "Reading": the name Settings and errors give it (`Role::name` in dto.rs). */
  name: string;
  /** The calls it covers, in a line under its name. */
  covers: string;
}

/** The kinds of work, each on the agent and model Settings names for it, in Settings' order. */
export const ROLES: RoleInfo[] = [
  { id: "read", name: "Reading", covers: "Postings, feed checks, resume imports, interview invites" },
  { id: "tailor", name: "Tailoring", covers: "Suggestions, rewrites, chat" },
  { id: "write", name: "Writing", covers: "Cover letters" },
  { id: "coach", name: "Coaching", covers: "Prep, practice, nudges, Dig, debriefs" },
];

/** A route per role, as a preset sets them. */
export type PresetRoutes = Record<Role, { agent: Agent; model: string }>;

export interface Preset {
  /** Set all to…'s value for it: `preset:<id>`. */
  id: string;
  name: string;
  routes: PresetRoutes;
}

const sonnet = { agent: "claude", model: "sonnet" } as const;

/**
 * Set all to…'s mixes, from `ez eval` on 2026-11-16 (`local/eval/2026-11-16/report.md`, ARCHITECTURE.md §12). Fastest:
 * each role's quickest route that meets its time target and is valid on the first try (Reading sonnet 6.8 s, Tailoring
 * Flash Low 10.5 s, Writing sonnet 6.2 s, Coaching sonnet 12.2 s). Best edits: Tailoring on the route whose changes the
 * blind review kept (opus, 4 of 7 on the one posting reviewed; 34.5 s, over Tailoring's 30 s target), the rest as
 * Fastest. Offered only while every agent it names is on and signed in and lists its models.
 */
export const PRESETS: Preset[] = [
  {
    id: "fastest",
    name: "Fastest",
    routes: { read: sonnet, tailor: { agent: "antigravity", model: "gemini-3.8-flash-low" }, write: sonnet, coach: sonnet },
  },
  {
    id: "best",
    name: "Best edits",
    routes: { read: sonnet, tailor: { agent: "claude", model: "opus" }, write: sonnet, coach: sonnet },
  },
];

/**
 * The icon of the company that made `model` (an id, alias or name): Anthropic's for Claude's models, OpenAI's for GPT
 * and o-series ones, Gemini's for Gemini; null for any other, so the caller shows the agent's own icon.
 */
export function makerIcon(model: string): string | null {
  const id = model.trim().toLowerCase();
  if (/^(claude|sonnet|opus|haiku)\b/.test(id)) return "/agents/claude.png";
  if (/^(gpt|o\d)/.test(id)) return "/agents/openai.png";
  if (/^gemini/.test(id)) return "/agents/gemini.png";
  return null;
}
