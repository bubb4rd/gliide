<!--
  LineTrend: a value per series across a few snapshots, drawn at the width it's given (text stays 11px). The y ticks
  as hairline rows with their values in ink 3 at the start, the baseline in `--line-strong`, a label under each
  snapshot (the first at the start, the last at the end, the rest centered; past six, every other). A series is a 2px
  line in its color (solid, dashed or dotted, to tell apart series of one color) through its snapshots, skipping any it
  has no value at, a 4.5px dot at each; a series that reaches the last snapshot has its name and value at the end of the
  plot (ink 2, the value in ink 3), pushed 16px apart where they'd collide (one that stops early has none: a legend
  beside the chart names every series). A `floor` is a dashed rule in ink 3 with its label under its trailing end.
  Hovering a dot opens its tip; one tab stop: arrows, Home and End move between series, a focused one ringed in
  `--accent` at its last value. No motion.
-->
<script module lang="ts">
  import type { TipContent } from "./tip.svelte";

  export interface TrendLine {
    key: string;
    label: string;
    /** A token (`var(--maker-anthropic)`). */
    color: string;
    dash?: "solid" | "dashed" | "dotted";
    /** One per snapshot; null where it has none. */
    values: (number | null)[];
    /** A dot's tooltip, by snapshot. */
    tip: (i: number) => TipContent;
  }
</script>

<script lang="ts">
  import { rove } from "./rove";
  import { tip } from "./tip.svelte";

  interface Props {
    lines: TrendLine[];
    /** A label under each snapshot ("Nov 17 · reference"). */
    snapshots: string[];
    /** `format` for the ticks; `value` for a series' values (its end label, assistive tech), `format` without. */
    y: { min: number; max: number; ticks: number[]; format: (v: number) => string; value?: (v: number) => string };
    floor?: { value: number; label: string };
    /** What it plots, for assistive tech. */
    label: string;
    height?: number;
  }

  let { lines, snapshots, y, floor, label, height = 240 }: Props = $props();

  const PAD = { l: 32, t: 8, b: 28 };
  /** Room after the plot for the end labels. */
  const END = 152;
  const TICK_DY = 16;

  let width = $state(0);
  let svg = $state<SVGSVGElement>();

  const x0 = PAD.l + 8;
  const x1 = $derived(Math.max(x0 + 1, width - END));
  const y0 = PAD.t;
  const y1 = $derived(height - PAD.b);
  const n = $derived(snapshots.length);
  const sx = (i: number) => (n < 2 ? (x0 + x1) / 2 : x0 + (i / (n - 1)) * (x1 - x0));
  const sy = (v: number) => y1 - ((v - y.min) / (y.max - y.min)) * (y1 - y0);

  const valueOf = (v: number) => (y.value ?? y.format)(v);
  const yTicks = $derived(y.ticks.filter((t) => t >= y.min && t <= y.max));
  const shown = (i: number) => n <= 6 || i % 2 === 0 || i === n - 1;

  /** Each series' points, its last one, and where its end label sits (pushed apart top to bottom). */
  const drawn = $derived.by(() => {
    const out = lines.map((l) => {
      const pts = l.values.flatMap((v, i) => (v === null ? [] : [{ i, v }]));
      const last = pts.at(-1) ?? null;
      return { line: l, pts, last, labelY: last ? sy(last.v) + 4 : 0 };
    });
    const ends = out.filter((d) => d.last?.i === n - 1).sort((a, b) => a.labelY - b.labelY);
    for (let k = 1; k < ends.length; k++) ends[k].labelY = Math.max(ends[k].labelY, ends[k - 1].labelY + 16);
    return out;
  });

  let active = $state<number | null>(null);
  const current = $derived(lines.length ? Math.min(active ?? 0, lines.length - 1) : -1);

  function onkeydown(e: KeyboardEvent) {
    const next = rove(e, current, lines.length);
    if (next === null) return;
    active = next;
    svg?.querySelectorAll<SVGGElement>(".series")[next]?.focus();
  }
</script>

<div class="line-trend" bind:clientWidth={width} style:height="{height}px">
  {#if width}
    <!-- One tab stop; arrows, Home and End move between series. -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <svg bind:this={svg} {width} {height} viewBox="0 0 {width} {height}" role="group" aria-label={label} {onkeydown}>
      {#each yTicks as t (t)}
        <line class="grid" x1={x0 - 8} x2={x1} y1={sy(t)} y2={sy(t)} />
        <text class="tick" x={x0 - 16} y={sy(t) + 3.5} text-anchor="end">{y.format(t)}</text>
      {/each}
      <line class="axis" x1={x0 - 8} x2={x1} y1={y1} y2={y1} />
      {#each snapshots as s, i (i)}
        {#if shown(i)}
          <text
            class="tick"
            x={sx(i)}
            y={y1 + TICK_DY}
            text-anchor={n < 2 ? "middle" : i === 0 ? "start" : i === n - 1 ? "end" : "middle"}>{s}</text
          >
        {/if}
      {/each}
      {#if floor && floor.value >= y.min && floor.value <= y.max}
        <line class="floor" x1={x0 - 8} x2={x1} y1={sy(floor.value)} y2={sy(floor.value)} />
        <text class="tick" x={x1} y={sy(floor.value) + 14} text-anchor="end">{floor.label}</text>
      {/if}
      {#each drawn as d (d.line.key)}
        {#if d.pts.length > 1}
          <polyline
            class={["line", d.line.dash ?? "solid"]}
            style:--c={d.line.color}
            points={d.pts.map((p) => `${sx(p.i).toFixed(1)},${sy(p.v).toFixed(1)}`).join(" ")}
          />
        {/if}
      {/each}
      {#each drawn as d, k (d.line.key)}
        {#each d.pts as p (p.i)}
          <g class="dot" style:--c={d.line.color} {@attach tip(() => d.line.tip(p.i))}>
            <circle class="hit" r="10" cx={sx(p.i)} cy={sy(p.v)} />
            <circle class="mark" r="4.5" cx={sx(p.i)} cy={sy(p.v)} />
          </g>
        {/each}
        {#if d.last}
          {@const last = d.last}
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
          <g
            class="series"
            role="img"
            aria-label="{d.line.label}: {d.pts.map((p) => `${snapshots[p.i]} ${valueOf(p.v)}`).join(', ')}"
            tabindex={k === current ? 0 : -1}
            onfocus={() => (active = k)}
            {@attach tip(() => d.line.tip(last.i))}
          >
            <circle class="ring" r="8" cx={sx(last.i)} cy={sy(last.v)} />
            {#if last.i === n - 1}
              <text class="end-label" x={x1 + 12} y={d.labelY}
                >{d.line.label} <tspan class="end-value">{valueOf(last.v)}</tspan></text
              >
            {/if}
          </g>
        {/if}
      {/each}
    </svg>
  {/if}
</div>

<style>
  .line-trend {
    min-width: 0;
  }
  svg {
    display: block;
    overflow: visible;
    font-size: var(--text-xs);
    font-variant-numeric: tabular-nums;
  }
  .grid {
    stroke: var(--line);
  }
  .axis {
    stroke: var(--line-strong);
  }
  .tick {
    fill: var(--ink-3);
  }
  .floor {
    stroke: var(--ink-3);
    stroke-dasharray: 3 3;
  }
  .line {
    fill: none;
    stroke: var(--c);
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .dashed {
    stroke-dasharray: 6 4;
  }
  .dotted {
    stroke-dasharray: 0.5 4;
  }
  .hit {
    fill: transparent;
  }
  .mark {
    fill: var(--c);
  }
  .series {
    outline: none;
  }
  .ring {
    fill: none;
    stroke: none;
  }
  .series:focus-visible .ring {
    stroke: var(--accent);
    stroke-width: 2;
  }
  .end-label {
    fill: var(--ink-2);
    font-weight: var(--weight-medium);
  }
  .end-value {
    fill: var(--ink-3);
    font-weight: var(--weight-regular);
  }

  @media (forced-colors: active) {
    .line,
    .dot {
      forced-color-adjust: none;
    }
    .series:focus-visible .ring {
      stroke: Highlight;
    }
  }
</style>
