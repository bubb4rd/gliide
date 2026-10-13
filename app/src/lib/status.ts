// An application's status as every screen shows it: its word and its badge tone. Amber means "left out" only
// (DESIGN.md), so no status is warn.

import type { AppStatus } from "$lib/api";

export const STATUS = {
  saved: { label: "Saved", tone: "neutral" },
  applied: { label: "Applied", tone: "accent" },
  interviewing: { label: "Interviewing", tone: "accent" },
  offer: { label: "Offer", tone: "gain" },
  rejected: { label: "Rejected", tone: "danger" },
} as const satisfies Record<AppStatus, { label: string; tone: string }>;
