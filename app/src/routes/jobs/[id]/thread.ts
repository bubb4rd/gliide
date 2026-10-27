// What you asked the agent to do on one review and what came back: the chat's turns. Kept per job in localStorage, a
// thread per review (by its analysis id), so switching jobs or relaunching keeps the conversation; the proposals
// themselves live in the review's draft, and their state is read from it.

import type { CustomEdit, Skipped } from "$lib/api";

/**
 * A place an instruction is limited to: an entry, a bullet, a skills line or the summary, as its chip names it. A
 * quoted line (`quote`) is a bullet or the summary picked by its words: its label is the line as the page reads it.
 */
export interface Mention {
  /** What `instruct` takes in `scope`: an entry, bullet or skills-line id, or `"summary"`. */
  id: string;
  label: string;
  quote?: boolean;
  /**
   * Where it sits in the ask's words: the start of its `token`. Missing on an ask from before parts sat in the words
   * (they came first).
   */
  at?: number;
}

/** An ask being written: its words, each part's token where it sits, and the parts with where (`at`). */
export interface Draft {
  text: string;
  scope: Mention[];
}

/** `draft` without the spaces around its words, its parts' places moved with them. */
export function trimmed({ text, scope }: Draft): Draft {
  const cut = text.length - text.trimStart().length;
  return { text: text.trim(), scope: scope.map((m) => (m.at === undefined ? m : { ...m, at: m.at - cut })) };
}

/** What a chip names: an entry's name (its label up to the first " · "), or a quoted line whole. */
export function mentionName(m: Pick<Mention, "label" | "quote">): string {
  return m.quote ? m.label : m.label.split(" · ")[0] || m.label;
}

/** A part in the ask's words, as the model reads them: `@` and its name, or the line in quotes. */
export function token(m: Pick<Mention, "label" | "quote">): string {
  return m.quote ? `“${m.label}”` : `@${mentionName(m)}`;
}

/**
 * The ask as you wrote it, in order: its words, and each part where you put it. A part whose token isn't where it
 * says (an ask from before parts sat in the words) comes first.
 */
export function pieces(ask: string, scope: Mention[]): (string | Mention)[] {
  const placed = scope
    .filter((m): m is Mention & { at: number } => m.at !== undefined && ask.startsWith(token(m), m.at))
    .sort((a, b) => a.at - b.at);
  const inWords = new Set<Mention>(placed);
  const out: (string | Mention)[] = scope.filter((m) => !inWords.has(m)).flatMap((m) => [m, " "]);
  let from = 0;
  for (const m of placed) {
    if (m.at < from) continue;
    if (m.at > from) out.push(ask.slice(from, m.at));
    out.push(m);
    from = m.at + token(m).length;
  }
  if (from < ask.length) out.push(ask.slice(from));
  return out;
}

export interface TurnProposal {
  /** The change's key in the review (your own edit, added off). */
  key: string;
  edit: CustomEdit;
  why: string;
  /** The line as it read before; null for a new bullet or skill. */
  was: string | null;
  flags: string[];
  /** Changes accepting it turned off, to turn back on if you take it back. */
  displaced: string[];
  /** You rejected it (removed from the draft); a `Restore` adds it again. */
  rejected: boolean;
}

/**
 * A part the agent left undone. With a `term` (a skill your resume doesn't show), the chat asks whether you have it;
 * `answer` is what you said, and `at` the entry you said you used it at (kept as a fact there), by name.
 */
export interface TurnSkip extends Skipped {
  answer?: "yes" | "no";
  at?: string;
}

export interface Turn {
  id: string;
  /** `Date.now()` when you sent it. */
  at: number;
  ask: string;
  scope: Mention[];
  /** Skills you said you have though your resume doesn't show them, sent with the ask. */
  confirmed?: string[];
  status: "running" | "done" | "failed" | "stopped";
  /** The agent's answer, once done. */
  reply: string;
  /** The model that answered (an id, else the alias asked); missing on turns saved before it was kept. */
  model?: string;
  proposals: TurnProposal[];
  skipped: TurnSkip[];
  /** How long the call took. */
  ms: number;
  error: string | null;
}

export type ProposalState = "pending" | "accepted" | "rejected" | "gone";

const PREFIX = "ez-thread:";

/** The job's threads, by review id, oldest saved first. */
function read(job: string): Record<string, Turn[]> {
  if (typeof localStorage === "undefined") return {};
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(PREFIX + job) ?? "{}");
    return saved && typeof saved === "object" && !Array.isArray(saved) ? (saved as Record<string, Turn[]>) : {};
  } catch {
    return {};
  }
}

/**
 * Review `review`'s thread. A review without one (a re-run, which carries your draft over to a new review) takes the
 * newest thread whose proposals its draft still holds (`holds`), so the conversation comes along with the edits.
 */
export function loadThread(job: string, review: string, holds: (key: string) => boolean): Turn[] {
  const saved = read(job);
  const turns =
    saved[review] ?? Object.values(saved).findLast((t) => t.some((turn) => turn.proposals.some((p) => holds(p.key))));
  // A call can't outlive the app: one that was running when it closed stopped there.
  return (turns ?? []).map((t) => (t.status === "running" ? { ...t, status: "stopped" } : t));
}

export function saveThread(job: string, review: string, turns: Turn[]) {
  const saved = read(job);
  delete saved[review];
  if (turns.length) saved[review] = turns;
  if (Object.keys(saved).length) localStorage.setItem(PREFIX + job, JSON.stringify(saved));
  else localStorage.removeItem(PREFIX + job);
}
