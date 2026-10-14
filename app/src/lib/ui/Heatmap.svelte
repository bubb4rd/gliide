<!--
  Heatmap: one cell per day, a column per week (Sunday on top), the last `weeks` weeks ending on `end`. A day's shade is
  its count against the busiest day shown: an empty well, then four steps of the accent. Months label the column they
  start in; hovering a cell shows what happened that day. Under the grid, the `caption` and a Less–More legend.
-->
<script module lang="ts">
  /** A local day as the grid keys it: `2026-10-13`. */
  export function dayKey(date: Date): string {
    const m = String(date.getMonth() + 1).padStart(2, "0");
    const d = String(date.getDate()).padStart(2, "0");
    return `${date.getFullYear()}-${m}-${d}`;
  }
</script>

<script lang="ts">
  import type { Snippet } from "svelte";
  import { tip, type TipContent } from "./tip.svelte";

  interface Props {
    /** Counts by local day (`dayKey`); a missing day counts 0. */
    counts: Map<string, number>;
    weeks?: number;
    /** The last day shown; today when omitted. */
    end?: Date;
    /** What the grid shows, for assistive tech ("Activity, last 26 weeks"). */
    label: string;
    /** A day's tooltip; its date and count when omitted. */
    describe?: (day: Date, count: number) => TipContent;
    /** Under the grid, before the legend. */
    caption?: Snippet;
  }

  let { counts, weeks = 26, end, label, describe, caption }: Props = $props();

  const LEVELS = [0, 1, 2, 3, 4] as const;
  const monthName = new Intl.DateTimeFormat(undefined, { month: "short" });
  const dayName = new Intl.DateTimeFormat(undefined, { weekday: "short", month: "short", day: "numeric" });

  const grid = $derived.by(() => {
    const last = end ?? new Date();
    const lastKey = dayKey(last);
    const start = new Date(last.getFullYear(), last.getMonth(), last.getDate() - last.getDay() - (weeks - 1) * 7);
    const days = Array.from({ length: weeks * 7 }, (_, i) => {
      const date = new Date(start.getFullYear(), start.getMonth(), start.getDate() + i);
      const key = dayKey(date);
      return { key, date, count: counts.get(key) ?? 0, future: key > lastKey };
    });
    const max = Math.max(0, ...days.map((d) => d.count));
    const cells = days.map((d) => ({ ...d, level: d.count && max ? Math.ceil((d.count / max) * 4) : 0 }));

    // A month labels the first column whose Sunday falls in it; a label with no room before the next one is dropped.
    const months: { column: number; name: string }[] = [];
    for (let w = 0; w < weeks; w++) {
      const sunday = days[w * 7].date;
      if (w > 0 && sunday.getMonth() === days[(w - 1) * 7].date.getMonth()) continue;
      if (months.length && w - months[months.length - 1].column < 3) months.pop();
      months.push({ column: w, name: monthName.format(sunday) });
    }
    return { cells, months };
  });

  const describeDay = (date: Date, count: number) => describe?.(date, count) ?? { title: dayName.format(date), value: count };
</script>

<div class="heatmap" style:--weeks={weeks}>
  <div class="months" aria-hidden="true">
    {#each grid.months as month (month.column)}
      <span style:grid-column={month.column + 1}>{month.name}</span>
    {/each}
  </div>
  <div class="grid" role="img" aria-label={label}>
    {#each grid.cells as cell (cell.key)}
      {#if cell.future}
        <span class="cell cell--future"></span>
      {:else}
        <span class="cell cell--{cell.level}" {@attach tip(() => describeDay(cell.date, cell.count))}></span>
      {/if}
    {/each}
  </div>
  <div class="foot">
    <div class="caption">{@render caption?.()}</div>
    <div class="legend meta" aria-hidden="true">
      Less
      {#each LEVELS as level (level)}<span class="cell cell--{level}"></span>{/each}
      More
    </div>
  </div>
</div>

<style>
  .heatmap {
    display: grid;
    gap: var(--space-2);
    min-width: 0;
  }
  .months,
  .grid {
    display: grid;
    grid-template-columns: repeat(var(--weeks), minmax(0, 1fr));
    column-gap: var(--space-1);
  }
  .months {
    font-size: var(--text-xs);
    line-height: var(--leading-xs);
    color: var(--ink-3);
  }
  .months span {
    grid-row: 1;
    white-space: nowrap;
  }
  .grid {
    grid-template-rows: repeat(7, auto);
    grid-auto-flow: column;
    row-gap: var(--space-1);
  }
  .cell {
    aspect-ratio: 1;
    border: 1px solid transparent;
    border-radius: var(--radius-xs);
  }
  .cell--0 {
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  /* Mixed in oklab: oklch would swing the hue from the near-gray well toward the accent through green. */
  .cell--1 {
    background: color-mix(in oklab, var(--accent) 25%, var(--sunken));
  }
  .cell--2 {
    background: color-mix(in oklab, var(--accent) 50%, var(--sunken));
  }
  .cell--3 {
    background: color-mix(in oklab, var(--accent) 75%, var(--sunken));
  }
  .cell--4 {
    background: var(--accent);
  }
  .cell--future {
    visibility: hidden;
  }
  .foot {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    min-width: 0;
  }
  .caption {
    flex: 1;
    min-width: 0;
    color: var(--ink-2);
    text-wrap: pretty;
  }
  .legend {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-1);
  }
  .legend .cell {
    width: 12px;
  }
  .legend .cell:first-of-type {
    margin-inline-start: var(--space-1);
  }
  .legend .cell:last-of-type {
    margin-inline-end: var(--space-1);
  }
  /* Forced colors: the accent becomes Highlight, the steps its opacity. */
  @media (forced-colors: active) {
    .cell--1,
    .cell--2,
    .cell--3,
    .cell--4 {
      forced-color-adjust: none;
      background: Highlight;
    }
    .cell--1 {
      opacity: 0.4;
    }
    .cell--2 {
      opacity: 0.6;
    }
    .cell--3 {
      opacity: 0.8;
    }
  }
</style>
