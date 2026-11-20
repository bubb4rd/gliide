// The live interview's words for time and runs, shared by the session, its readback and the prep's run rows.

import type { LiveRun, LiveTurn } from "$lib/api";

/** The session clock: "5:48", "1:02:10" past an hour. */
export function clockText(ms: number): string {
  const total = Math.floor(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}

/** A run's length in minutes: "18 min", "under a minute". */
export function runLength(ms: number): string {
  return ms < 60_000 ? "under a minute" : `${Math.round(ms / 60_000)} min`;
}

/** "Oct 28", or "Sun, Nov 1" within the week; "today 3:12 PM" today. */
export function runDay(at: string, now = new Date()): string {
  const d = new Date(at);
  if (d.toDateString() === now.toDateString()) {
    return `today ${d.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" })}`;
  }
  const week = now.getTime() - d.getTime() < 6 * 86_400_000;
  return d.toLocaleDateString(undefined, week ? { weekday: "short", month: "short", day: "numeric" } : { month: "short", day: "numeric" });
}

const count = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** "6 questions · 3 follow-ups", follow-ups left out at none. */
export function runCounts(asked: number, follows: number): string {
  return [count(asked, "question", "questions"), follows ? count(follows, "follow-up", "follow-ups") : null]
    .filter(Boolean)
    .join(" · ");
}

/** The prep's row for a past run: "Oct 28 · 6 questions · 2 follow-ups · 18 min"; unfinished, "· not ended". */
export function runLine(run: LiveRun, now = new Date()): string {
  const length = run.endedAt ? (run.activeMs !== null ? runLength(run.activeMs) : null) : "not ended";
  return [runDay(run.startedAt, now), runCounts(run.asked, run.follows), length].filter(Boolean).join(" · ");
}

/** The turn you answer now: the last, while it has no answer and wasn't skipped. */
export function currentTurn(turns: LiveTurn[]): LiveTurn | undefined {
  const last = turns.at(-1);
  return last && last.answer === null && !last.skipped ? last : undefined;
}

/** A question's turns hold an answer (main or follow-up reply). */
export const answered = (turns: LiveTurn[], question: string) =>
  turns.some((t) => t.question === question && t.answer !== null);
