<!--
  Scatter: one value against another, a point each, drawn at the width it's given (its text stays 11px at any width).
  Hairline grid rows at the y ticks with their values in ink 3 at the start, the baseline in `--line-strong`, the x
  ticks under it and the x axis's title under those (ink 3, centered). A point is a 9px dot in its series color
  (`hollow`: a ring, for one with a stand-in in it) on a 20px hit area, its label (11px, ink 2) beside it wherever it
  hits no other label, point or edge (right, left, above, below, then the corners; none where every spot does, its
  tip still naming it). Optional: a `quadrant` from the
  start to `x` and from the top to `y`, washed in `--hover`, its label in ink 3 (where "better on both" is); a
  `frontier`, the points no other beats on both, joined by a dashed `--ink-3` line; a `target`, a dashed rule up from
  the x axis, "target" by its foot. Hovering a point opens its `tip`; one tab stop: arrows, Home and End move between
  points left to right, a focused point ringed in `--accent`. No motion.
-->
<script module lang="ts">
  import type { TipContent } from "./tip.svelte";

  export interface ScatterPoint {
    key: string;
    x: number;
    y: number;
    label: string;
    /** A token (`var(--maker-google)`). */
    color: string;
    hollow?: boolean;
    tip: TipContent;
  }

  export interface ScatterAxis {
    min: number;
    max: number;
    log?: boolean;
    ticks: number[];
    format: (v: number) => string;
    /** Under the x axis ("Cost to run the suite, log"). */
    title?: string;
  }
</script>

<script lang="ts">
  import { rove } from "./rove";
  import { tip } from "./tip.svelte";

  interface Props {
    points: ScatterPoint[];
    x: ScatterAxis;
    y: ScatterAxis;
    /** Better on both is the top start corner up to here. */
    quadrant?: { x: number; y: number; label: string };
    /** Keys of the points no other beats on both, in x order. */
    frontier?: string[];
    target?: { x: number; label: string };
    /** What it plots, for assistive tech. */
    label: string;
    height?: number;
  }

  let { points, x, y, quadrant, frontier = [], target, label, height = 240 }: Props = $props();

  const PAD = { l: 32, r: 8, t: 8, b: 40 };
  const TICK_DY = 16;
  const TITLE_DY = 34;
  const LABEL_PX = 11;

  let width = $state(0);
  let svg = $state<SVGSVGElement>();

  const x0 = PAD.l;
  const x1 = $derived(Math.max(x0 + 1, width - PAD.r));
  const y0 = PAD.t;
  const y1 = $derived(height - PAD.b);

  function along(a: ScatterAxis, v: number) {
    if (a.log) {
      const lo = Math.log10(a.min);
      const hi = Math.log10(a.max);
      return (Math.log10(Math.max(v, a.min / 10)) - lo) / (hi - lo);
    }
    return (v - a.min) / (a.max - a.min);
  }
  const sx = (v: number) => x0 + along(x, v) * (x1 - x0);
  const sy = (v: number) => y1 - along(y, v) * (y1 - y0);

  /** Text widths, measured in the label's font once there's an element to read it from. */
  let measure: ((s: string) => number) | null = null;
  function textWidth(s: string) {
    if (!measure && svg) {
      const ctx = document.createElement("canvas").getContext("2d");
      const family = getComputedStyle(svg).fontFamily;
      if (ctx) {
        ctx.font = `500 ${LABEL_PX}px ${family}`;
        measure = (t) => ctx.measureText(t).width;
      }
    }
    return measure ? measure(s) : s.length * LABEL_PX * 0.56;
  }

  type Placed = { anchor: "start" | "end" | "middle"; lx: number; ly: number };

  /**
   * Each point's label where it hits nothing: greedy, in the order given (the first gets the first pick). A point with
   * no free spot goes unlabeled; its tip and accessible name still say what it is.
   */
  const placed = $derived.by((): Map<string, Placed> => {
    const out = new Map<string, Placed>();
    if (!width) return out;
    const marks = points.map((p) => [sx(p.x), sy(p.y)] as const);
    /** The target's rule: a label never crosses it. */
    const rule = target ? sx(target.x) : null;
    const boxes: [number, number, number, number][] = [];
    const clash = (b: [number, number, number, number]) =>
      b[0] < 2 ||
      b[2] > width - 2 ||
      b[1] < 0 ||
      b[3] > y1 - 2 ||
      boxes.some((p) => !(b[2] + 4 < p[0] || b[0] > p[2] + 4 || b[3] + 5 < p[1] || b[1] > p[3] + 5)) ||
      marks.some(([mx, my]) => b[0] - 6 < mx && mx < b[2] + 6 && b[1] - 6 < my && my < b[3] + 6) ||
      (rule !== null && b[0] - 2 < rule && rule < b[2] + 2);
    for (const p of points) {
      const px = sx(p.x);
      const py = sy(p.y);
      const w = textWidth(p.label);
      const spots: [Placed["anchor"], number, number][] = [
        ["start", px + 10, py + 4],
        ["end", px - 10, py + 4],
        ["middle", px, py - 10],
        ["middle", px, py + 18],
        ["start", px + 8, py - 8],
        ["start", px + 8, py + 16],
        ["end", px - 8, py - 8],
        ["end", px - 8, py + 16],
      ];
      for (const s of spots) {
        const left = s[0] === "start" ? s[1] : s[0] === "end" ? s[1] - w : s[1] - w / 2;
        const box: [number, number, number, number] = [left, s[2] - 10, left + w, s[2] + 3];
        if (clash(box)) continue;
        boxes.push(box);
        out.set(p.key, { anchor: s[0], lx: s[1], ly: s[2] });
        break;
      }
    }
    return out;
  });

  const yTicks = $derived(y.ticks.filter((t) => t >= y.min && t <= y.max));
  const xTicks = $derived(x.ticks.filter((t) => t >= x.min && t <= x.max));
  const line = $derived(
    frontier
      .map((k) => points.find((p) => p.key === k))
      .filter((p): p is ScatterPoint => !!p)
      .map((p) => `${sx(p.x).toFixed(1)},${sy(p.y).toFixed(1)}`)
      .join(" "),
  );

  /** Left to right for the keys. */
  const order = $derived([...points].sort((a, b) => a.x - b.x));
  let active = $state<number | null>(null);
  const current = $derived(order.length ? Math.min(active ?? 0, order.length - 1) : -1);

  function onkeydown(e: KeyboardEvent) {
    const next = rove(e, current, order.length);
    if (next === null) return;
    active = next;
    svg?.querySelectorAll<SVGGElement>(".point")[next]?.focus();
  }
</script>

<div class="scatter" bind:clientWidth={width} style:height="{height}px">
  {#if width}
    <!-- One tab stop; arrows, Home and End move between points. -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <svg bind:this={svg} {width} {height} viewBox="0 0 {width} {height}" role="group" aria-label={label} {onkeydown}>
      {#if quadrant}
        {@const qx = Math.min(x1, Math.max(x0, sx(quadrant.x)))}
        {@const qy = Math.min(y1, Math.max(y0, sy(quadrant.y)))}
        <rect class="quad" x={x0} y={y0} width={qx - x0} height={qy - y0} rx="4" />
        <text class="quad-label" x={x0 + 8} y={y0 + 16}>{quadrant.label}</text>
      {/if}
      {#each yTicks as t (t)}
        <line class="grid" x1={x0} x2={x1} y1={sy(t)} y2={sy(t)} />
        <text class="tick" x={x0 - 8} y={sy(t) + 3.5} text-anchor="end">{y.format(t)}</text>
      {/each}
      <line class="axis" x1={x0} x2={x1} y1={y1} y2={y1} />
      {#each xTicks as t, i (t)}
        <text
          class="tick"
          x={sx(t)}
          y={y1 + TICK_DY}
          text-anchor={sx(t) - x0 < 16 && i === 0 ? "start" : x1 - sx(t) < 16 ? "end" : "middle"}>{x.format(t)}</text
        >
      {/each}
      {#if x.title}<text class="tick" x={(x0 + x1) / 2} y={y1 + TITLE_DY} text-anchor="middle">{x.title}</text>{/if}
      {#if target}
        {@const tx = sx(target.x)}
        <line class="target" x1={tx} x2={tx} y1={y0} y2={y1} />
        <text class="tick" x={tx + 4} y={y1 - 6}>{target.label}</text>
      {/if}
      {#if line}<polyline class="frontier" points={line} />{/if}
      {#each order as p, i (p.key)}
        {@const at = placed.get(p.key)}
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <g
          class="point"
          role="img"
          aria-label={typeof p.tip === "string" ? p.tip : `${p.label}, ${x.format(p.x)}, ${y.format(p.y)}`}
          tabindex={i === current ? 0 : -1}
          onfocus={() => (active = i)}
          style:--c={p.color}
          {@attach tip(() => p.tip)}
        >
          <circle class="hit" r="10" cx={sx(p.x)} cy={sy(p.y)} />
          <circle class="ring" r="8" cx={sx(p.x)} cy={sy(p.y)} />
          <circle class={p.hollow ? "mark mark--hollow" : "mark"} r="4.5" cx={sx(p.x)} cy={sy(p.y)} />
          {#if at}<text class="label-text" x={at.lx} y={at.ly} text-anchor={at.anchor}>{p.label}</text>{/if}
        </g>
      {/each}
    </svg>
  {/if}
</div>

<style>
  .scatter {
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
  .tick,
  .quad-label {
    fill: var(--ink-3);
  }
  .quad {
    fill: var(--hover);
  }
  .target,
  .frontier {
    fill: none;
    stroke: var(--ink-3);
    stroke-width: 1;
    stroke-dasharray: 3 3;
  }
  .hit {
    fill: transparent;
  }
  .mark {
    fill: var(--c);
  }
  .mark--hollow {
    fill: var(--surface);
    stroke: var(--c);
    stroke-width: 1.5;
  }
  .ring {
    fill: none;
    stroke: none;
  }
  .point {
    outline: none;
  }
  .point:focus-visible .ring {
    stroke: var(--accent);
    stroke-width: 2;
  }
  .label-text {
    fill: var(--ink-2);
    font-weight: var(--weight-medium);
  }

  @media (forced-colors: active) {
    .point {
      forced-color-adjust: none;
    }
    .point:focus-visible .ring {
      stroke: Highlight;
    }
  }
</style>
