// The posting's keywords as the page has them (on it as yours, added by an edit that's on, missing, or not yours),
// each list highest priority first; and what the chat's keyword meters show on hover and say to assistive tech.

import type { ChipState, ReviewView } from "$lib/api";
import type { TipData, TipGroup } from "$lib/ui";

export interface Keywords {
  have: string[];
  add: string[];
  missing: string[];
  /** You said they aren't yours: off the page, not a gap to fill. */
  notMine: string[];
  of: number;
}

export function keywordsOf(review: ReviewView): Keywords {
  const terms = (state: ChipState) =>
    review.skills
      .filter((c) => c.state === state)
      .sort((a, b) => a.priority - b.priority)
      .map((c) => c.term);
  return {
    have: terms("have"),
    add: terms("add"),
    missing: terms("missing"),
    notMine: terms("not-mine"),
    of: review.skills.length,
  };
}

/** The meter's tooltip: the count, then each list under its cell's key; the missing ones in amber. */
export function keywordTip(k: Keywords): TipData {
  const groups: TipGroup[] = [
    { label: "On your resume", key: "have", items: k.have },
    { label: "Added by your edits", key: "add", items: k.add },
    { label: "Missing", key: "missing", tone: "warn", items: k.missing },
    { label: "Not yours", key: "missing", items: k.notMine },
  ];
  return {
    title: "Keywords",
    value: `${k.have.length + k.add.length} of ${k.of}`,
    groups: groups.filter((g) => g.items.length),
  };
}

/** The meter as a sentence: "8 of 14 keywords; missing Docker, Kubernetes". */
export function keywordLabel(k: Keywords): string {
  const count = `${k.have.length + k.add.length} of ${k.of} keywords`;
  return k.missing.length ? `${count}; missing ${k.missing.join(", ")}` : count;
}
