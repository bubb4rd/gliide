// One tab stop for a chart's marks: arrows, Home and End move focus between them (BarChart's keys, for the charts
// whose marks run down as well as across).

/** The index `e` moves focus to among `count` marks from `current`, or null when it isn't a moving key. */
export function rove(e: KeyboardEvent, current: number, count: number): number | null {
  if (count < 1) return null;
  const last = count - 1;
  const next =
    e.key === "ArrowLeft" || e.key === "ArrowUp"
      ? Math.max(0, current - 1)
      : e.key === "ArrowRight" || e.key === "ArrowDown"
        ? Math.min(last, current + 1)
        : e.key === "Home"
          ? 0
          : e.key === "End"
            ? last
            : null;
  if (next !== null) e.preventDefault();
  return next;
}
