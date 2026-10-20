// Where an edit sits on the page, as the chat's cards name it: the entry (or the summary, or a skills line), then
// which line of it ("Tidepool › bullet 1"). Read from the line ids the review hands out: bullets are `entry.n`, note
// lines `entry#n`, an entry or skills line its own id, the summary `summary`.

import type { CustomEdit, EditTargets } from "$lib/api";

export interface Place {
  /** The entry's name, `Summary`, a skills line's label, or the change's group when nothing finer is known. */
  main: string;
  /** Which line of it (`bullet 2`, `line 1`, `new bullet`); null when `main` says it all. */
  sub: string | null;
}

/** An entry's name: its label up to the first " · " (the subtitle follows). */
const nameOf = (label: string) => label.split(" · ")[0] || label;

/** The place of `target` (a change's line id), `group` (its group's label) standing in for what the targets don't name. */
export function placeOf(target: string | null, group: string, targets: EditTargets | null): Place {
  if (target === "summary") return { main: "Summary", sub: null };
  if (target && targets) {
    const entry = (id: string) => targets.entries.find((t) => t.id === id);
    const own = entry(target);
    if (own) return { main: nameOf(own.label), sub: "new bullet" };
    const line = targets.groups.find((t) => t.id === target);
    if (line) return { main: line.label, sub: null };
    const bullet = /^(.+)\.(\d+)$/.exec(target);
    const at = bullet && entry(bullet[1]);
    if (bullet && at) return { main: nameOf(at.label), sub: `bullet ${bullet[2]}` };
    const note = /^(.+)#(\d+)$/.exec(target);
    const of = note && entry(note[1]);
    if (note && of) return { main: nameOf(of.label), sub: `line ${note[2]}` };
  }
  return { main: group, sub: null };
}

/** The line id a custom edit lands on, as `ChangeView.target` would name it. */
export function editTarget(edit: CustomEdit): string {
  switch (edit.kind) {
    case "bullet":
      return edit.entry;
    case "skill":
      return edit.group;
    case "line":
      return edit.line;
    case "summary":
      return "summary";
  }
}

/** Points as the score line says them: `+7`, `−3`, `0`. */
export const signed = (n: number) => (n > 0 ? `+${n}` : n < 0 ? `−${-n}` : "0");
