// Interview rounds as the screens name them: their kinds, and when they are, in words relative to now.

import type { Round, RoundKind } from "$lib/api";

export const ROUND_KINDS: { value: RoundKind; label: string }[] = [
  { value: "recruiter-screen", label: "Recruiter screen" },
  { value: "hiring-manager", label: "Hiring manager" },
  { value: "technical", label: "Technical" },
  { value: "take-home", label: "Take-home" },
  { value: "system-design", label: "System design" },
  { value: "behavioral", label: "Behavioral" },
  { value: "onsite", label: "Onsite" },
  { value: "final", label: "Final" },
  { value: "other", label: "Interview" },
];

export const kindLabel = (kind: RoundKind) => ROUND_KINDS.find((k) => k.value === kind)?.label ?? "Interview";

const DAY = 86_400_000;

/** Midnight of `d`'s day, local time. */
const dayOf = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();

/** "2:00 PM", or "2 PM" with `short` on the hour. */
function clock(d: Date, short: boolean): string {
  const whole = d.getMinutes() === 0;
  return d.toLocaleTimeString(undefined, { hour: "numeric", minute: short && whole ? undefined : "2-digit" });
}

/**
 * When a round starts, in words: "today 2:00 PM", "tomorrow 2:00 PM", "Tue 2:00 PM" within the week, else
 * "Nov 3, 2:00 PM". `short` drops ":00" on the hour and says the weekday for today and tomorrow too ("Tue 2 PM"), for
 * a status line.
 */
export function when(startsAt: string, now = new Date(), short = false): string {
  const d = new Date(startsAt);
  const days = Math.round((dayOf(d) - dayOf(now)) / DAY);
  const time = clock(d, short);
  if (!short && days === 0) return `today ${time}`;
  if (!short && days === 1) return `tomorrow ${time}`;
  if (days >= 0 && days < 7) return `${d.toLocaleDateString(undefined, { weekday: "short" })} ${time}`;
  return `${d.toLocaleDateString(undefined, { month: "short", day: "numeric" })}, ${time}`;
}

/** "Technical · tomorrow 2:00 PM": a round's kind and when, or "not scheduled". */
export function roundLine(round: Round, now = new Date()): string {
  return `${kindLabel(round.kind)} · ${round.startsAt ? when(round.startsAt, now) : "not scheduled"}`;
}

/** When the round ends: its start plus its length (an hour when unknown); null while unscheduled. */
export function endsAt(round: Round): number | null {
  return round.startsAt ? new Date(round.startsAt).getTime() + (round.minutes ?? 60) * 60_000 : null;
}

/** The round has ended (not cancelled) and has no debrief yet. */
export function awaitingDebrief(round: Round, now = new Date()): boolean {
  const end = endsAt(round);
  return round.state === "upcoming" && end !== null && end < now.getTime() && round.debriefedAt === null;
}

/** When a round ended, for a line about it: "12:00 PM" today, "yesterday 12:00 PM", else the day ("Oct 24"). */
export function endedWhen(round: Round, now = new Date()): string {
  const end = new Date(endsAt(round) ?? 0);
  const days = Math.round((dayOf(now) - dayOf(end)) / DAY);
  if (days === 0) return clock(end, false);
  if (days === 1) return `yesterday ${clock(end, false)}`;
  return end.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

/** How long ago a round ended: "Ended 5 minutes ago", "Ended 1 hour ago", "Ended yesterday", "Ended 3 days ago". */
export function endedAgo(round: Round, now = new Date()): string {
  const end = endsAt(round);
  if (end === null) return "";
  const minutes = Math.max(0, Math.round((now.getTime() - end) / 60_000));
  if (minutes < 1) return "Ended just now";
  if (minutes < 60) return `Ended ${minutes} minute${minutes === 1 ? "" : "s"} ago`;
  const days = Math.round((dayOf(now) - dayOf(new Date(end))) / DAY);
  if (days === 0) {
    const hours = Math.round(minutes / 60);
    return `Ended ${hours} hour${hours === 1 ? "" : "s"} ago`;
  }
  return days === 1 ? "Ended yesterday" : `Ended ${days} days ago`;
}
