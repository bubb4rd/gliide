// The agents gliide runs, as the screens name them: who makes each, whose plan its calls run on, its icon (bundled in
// `static/agents/`), where to learn to install it and the binary a path setting points at. In `agent_statuses`'
// order. Plus the icon of whoever made a model, for the lines that say which model answered.

import type { Agent } from "$lib/api";

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
