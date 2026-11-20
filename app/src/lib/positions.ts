// Settings → Agents' trade-off (DESIGN.md Settings → Agents): every route the benchmark timed for a role is a position
// on that role's line, placed by its median time against the role's target; Tailoring's is a plot, time against the
// benchmark's quality. A whole setup (the roles' routes) is a position too: model time a month, each role's median
// times its calls in the last 30 days, and Tailoring's quality. Pure: the pane and the specimen feed it.

import type { Agent, BenchResult, BenchRole, BenchRoute, BenchSource, KeptShare, Role, UsageDay } from "$lib/api";
import { ROLES } from "$lib/agents";

export interface Route {
  agent: Agent;
  model: string;
}

/** A role's routes, as a preset or the settings name them. */
export type Routes = Record<Role, Route>;

export const routeKey = (r: Route) => `${r.agent}:${r.model}`;
export const sameRoute = (a: Route | null | undefined, b: Route | null | undefined) =>
  !!a && !!b && a.agent === b.agent && a.model === b.model;

/** A line runs to this many times its role's target, so the target sits halfway along every line (`RouteLine`). */
export const SPAN = 2;

export const resultOf = (r: BenchRoute, from: BenchSource | null): BenchResult | null =>
  from === "thisMac" ? r.thisMac : from === "reference" ? r.reference : null;
/** Where time and first-try validity come from: this Mac's runs where there are any, else the reference. */
export const timedOf = (r: BenchRoute) => resultOf(r, r.timingFrom);
/** Where quality comes from: the source with more runs (this Mac on a tie). */
export const judgedOf = (r: BenchRoute) => resultOf(r, r.qualityFrom);

/** A route's place on its role's line. */
export interface Position extends Route {
  key: string;
  /** The median call, in seconds. */
  seconds: number;
  /** Where the time came from: a benchmark run (this Mac's or the reference) or, with none, your own calls. */
  source: BenchSource | "yours";
  /** The share of runs valid on the first try; null when only your own calls timed it. */
  valid: number | null;
  /** The benchmark's quality, 0–100; null where the role has none (Writing) or the route wasn't judged. */
  quality: number | null;
  /** Your own Tailoring edits kept on this route; null until you've decided and exported some. */
  kept: KeptShare | null;
  /** The role runs on it now. */
  current: boolean;
}

/** Every route with a median time for the role, fastest first. */
export function positionsOf(board: BenchRole): Position[] {
  const out: Position[] = [];
  for (const r of board.routes) {
    const t = timedOf(r);
    const fromBench = t?.p50S ?? null;
    const seconds = fromBench ?? r.yours?.p50S ?? null;
    if (seconds === null) continue;
    out.push({
      agent: r.agent,
      model: r.model,
      key: routeKey(r),
      seconds,
      source: fromBench !== null && t ? (r.timingFrom ?? "reference") : "yours",
      valid: fromBench !== null && t && t.runs > 0 ? t.valid / t.runs : null,
      quality: r.score?.quality ?? judgedOf(r)?.quality ?? null,
      kept: r.yours?.kept ?? null,
      current: r.current,
    });
  }
  return out.sort((a, b) => a.seconds - b.seconds);
}

/** The route's position for the role, or null when nothing timed it. */
export const positionOf = (positions: Position[], route: Route) => positions.find((p) => sameRoute(p, route)) ?? null;

/** Valid on every first try (a route timed only by your own calls says nothing either way). */
export const alwaysValid = (p: Position) => p.valid === null || p.valid >= 1;

/** Where on its line a time sits, from 0 to 1; past the line's end it stays at the end. */
export const along = (seconds: number, target: number) => Math.min(Math.max(seconds / (target * SPAN), 0), 1);

/** The keys of the positions no other beats on both time and quality (faster and at least as good), fastest first. */
export function frontier(positions: Position[]): string[] {
  const judged = positions.filter((p) => p.quality !== null).sort((a, b) => a.seconds - b.seconds);
  const out: string[] = [];
  let best = -Infinity;
  for (const p of judged) {
    if (p.quality! > best) {
      best = p.quality!;
      out.push(p.key);
    }
  }
  return out;
}

/** Each role's calls in the `days` days to `today` (`YYYY-MM-DD`, today included). */
export function callsByRole(rows: UsageDay[], today: string, days = 30): Record<Role, number> {
  const out: Record<Role, number> = { read: 0, tailor: 0, write: 0, coach: 0 };
  const ymd = /^(\d{4})-(\d{2})-(\d{2})$/.exec(today);
  if (!ymd) return out;
  const start = new Date(+ymd[1], +ymd[2] - 1, +ymd[3] - (days - 1));
  const from = `${start.getFullYear()}-${String(start.getMonth() + 1).padStart(2, "0")}-${String(start.getDate()).padStart(2, "0")}`;
  for (const r of rows) if (r.day >= from && r.day <= today) out[r.role] += r.calls;
  return out;
}

/** A whole setup as a position. */
export interface Mix {
  /** Model time a month: each role's median times its calls in the last 30 days, in minutes; null while a role with
   *  calls isn't timed (`missing`). */
  minutes: number | null;
  missing: Role[];
  /** Tailoring's quality on its route, 0–100; null when it wasn't judged. */
  quality: number | null;
}

export function mixOf(routes: Routes, boards: BenchRole[], calls: Record<Role, number>): Mix {
  let seconds = 0;
  const missing: Role[] = [];
  let quality: number | null = null;
  for (const { id } of ROLES) {
    const board = boards.find((b) => b.role === id);
    const p = board ? positionOf(positionsOf(board), routes[id]) : null;
    if (p) seconds += p.seconds * calls[id];
    else if (calls[id] > 0) missing.push(id);
    if (id === "tailor") quality = p?.quality ?? null;
  }
  return { minutes: missing.length ? null : seconds / 60, missing, quality };
}

/**
 * The closest position that can run, for a role whose agent can't: the least time and quality apart from where the
 * role is (in units of its target and of the quality scale), a route not valid every first try counting half a target
 * further. Null when none can run.
 */
export function nearest(
  positions: Position[],
  current: Route,
  target: number,
  canRun: (r: Route) => boolean,
): Position | null {
  const here = positionOf(positions, current);
  const cost = (p: Position) =>
    (here ? Math.abs(p.seconds - here.seconds) / target : p.seconds / target) +
    (here?.quality != null && p.quality != null ? Math.abs(p.quality - here.quality) / 100 : 0) +
    (alwaysValid(p) ? 0 : 0.5);
  const ok = positions.filter((p) => !sameRoute(p, current) && canRun(p));
  return ok.length ? ok.reduce((a, b) => (cost(b) < cost(a) ? b : a)) : null;
}

// ── Words ──

export const sec = (s: number) => `${s.toFixed(1)} s`;

/** "27 min less", "about the same time". */
export function minutesDelta(d: number) {
  const a = Math.abs(d);
  if (a < 0.5) return "about the same time";
  return `${Math.round(a)} min ${d < 0 ? "less" : "more"}`;
}

/** "4 of 7 kept" from your own decisions, or null. */
export const keptText = (k: KeptShare | null) => (k && k.decided > 0 ? `${k.kept} of ${k.decided} kept` : null);
