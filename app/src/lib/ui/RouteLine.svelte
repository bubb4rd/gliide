<!--
  RouteLine: a role's routes as positions (Settings → Agents' Who does what). Each route the benchmark timed is its
  agent's 20px mark on a line from 0 to twice the role's target, so the target, a 1px ink-2 tick with "Target 30 s"
  over it and a dashed rule down from it, sits halfway along every line. Routes slower than the line's end are one
  "+3 slower" marker at its end (meta; its tooltip lists them; they're still in the role's route menu), unless one is
  where the role runs, which stays, pinned to the end. With `plot` (Tailoring) it's a plot instead: time across, the
  benchmark's quality up, hairlines at the top and bottom of the quality range with their values, the routes no other
  beats on both joined by a dashed line (`frontier`). Labels ("Sonnet 6.8 s", tabular) go where they fit, most telling
  first: where the role runs, one in hand, then the routes no other beats on both time and quality, then the fastest;
  on a line, two rows under it and one over it beside the target's label, on a plot beside or over or under the mark,
  never over another label, mark or axis; a route without room keeps its mark, named in its tooltip and accessible name.
  The route the role runs on is ringed in `--accent`, its label accent; one in `preview` (a named position in hand) gets
  a dashed accent ring; a route whose agent can't run says why after its time ("· signed out", ink 3) and can't be
  chosen; one not valid on every first try shows circle-alert in `--warn`. The logos are never grayed. One tab stop:
  arrows, Home and End move between routes left to right, ⏎ or Space (or a click) chooses; hovering or focusing one
  calls `onpreview` with it (null on leaving), and opens its `tip`. No motion but the rings' 100ms.
-->
<script module lang="ts">
  import type { TipContent } from "./tip.svelte";

  export interface RoutePoint {
    key: string;
    /** The agent's icon. */
    icon: string;
    /** The model's name. */
    label: string;
    seconds: number;
    /** The benchmark's quality, 0–100 (the plot's height; with time, which labels go first). */
    quality?: number | null;
    /** Not valid on every first try. */
    invalid?: boolean;
    /** Why its agent can't run, in a few words ("signed out"); it can't be chosen. */
    blocked?: string | null;
    /** The role runs on it. */
    current?: boolean;
    /** A named position in hand would put the role here. */
    preview?: boolean;
    /** What assistive tech reads for it. */
    description: string;
    tip: TipContent;
  }
</script>

<script lang="ts">
  import CircleAlert from "@lucide/svelte/icons/circle-alert";
  import CompanyMark from "./CompanyMark.svelte";
  import Icon from "./Icon.svelte";
  import { rove } from "./rove";
  import { tip } from "./tip.svelte";

  interface Props {
    points: RoutePoint[];
    /** The role's target, in seconds. */
    target: number;
    /** What it shows, for assistive tech ("Tailoring: routes by time and quality"). */
    label: string;
    /** A plot of quality against time, over this range of quality. */
    plot?: { min: number; max: number } | null;
    /** Keys of the routes no other beats on both, fastest first (with `plot`: joined by a dashed line). */
    frontier?: string[];
    onchoose?: (key: string) => void;
    onpreview?: (key: string | null) => void;
  }

  let { points, target, label, plot = null, frontier = [], onchoose, onpreview }: Props = $props();

  /** A line runs to twice its target. */
  const SPAN = 2;
  const LINE_Y = 38;
  /** The label rows, in the order they're tried: two under the line, then the one over it. */
  const LANE_TOP = [52, 68, 2];
  const PLOT_TOP = 26;
  const PLOT_H = 126;
  const LABEL_PX = 12;
  const LABEL_H = 16;

  let width = $state(0);
  let root = $state<HTMLDivElement>();

  const sec = (s: number) => `${s.toFixed(1)} s`;
  const end = $derived(target * SPAN);
  const xOf = (s: number) => Math.min(Math.max(s / end, 0), 1) * width;
  const yOf = (q: number | null | undefined) =>
    !plot || q == null ? PLOT_TOP + PLOT_H : PLOT_TOP + (1 - (q - plot.min) / Math.max(plot.max - plot.min, 1)) * PLOT_H;
  const tx = $derived((1 / SPAN) * width);

  /** Text widths, measured in the label's font once there's an element to read it from. */
  let measure: ((s: string, weight: number) => number) | null = null;
  function textWidth(s: string, weight = 400) {
    if (!measure && root) {
      const ctx = document.createElement("canvas").getContext("2d");
      const family = getComputedStyle(root).fontFamily;
      if (ctx)
        measure = (t, w) => {
          ctx.font = `${w} ${LABEL_PX}px ${family}`;
          return ctx.measureText(t).width;
        };
    }
    return measure ? measure(s, weight) : s.length * LABEL_PX * 0.56;
  }
  /** A label's width: the model, its time, the alert and why it can't run, with 2px a side. */
  function labelWidth(p: RoutePoint) {
    const text = `${p.label} ${sec(p.seconds)}${p.blocked && !p.current ? ` · ${p.blocked}` : ""}`;
    return textWidth(text, p.current ? 500 : 400) + (p.invalid ? 12 + 4 : 0) + 4;
  }

  /** The routes drawn, left to right (for the keys too): all but those past the line's end, unless one is where the
   *  role runs or in hand. */
  const order = $derived(
    [...points].filter((p) => p.seconds <= end || p.current || p.preview).sort((a, b) => a.seconds - b.seconds),
  );
  /** Past the line's end, fastest first: one marker at its end. */
  const slower = $derived(points.filter((p) => !order.includes(p)).sort((a, b) => a.seconds - b.seconds));
  const slowerText = $derived(`+${slower.length} slower`);

  /** Most telling first: where the role runs, one in hand, then those no other beats on both time and quality
   *  (faster and at least as good), then the rest, fastest first. */
  const ranked = $derived.by(() => {
    const best = new Set<string>();
    let top = -Infinity;
    for (const p of order) {
      const q = p.quality ?? -Infinity;
      if (q > top) {
        top = q;
        best.add(p.key);
      }
    }
    const rank = (p: RoutePoint) => (p.current ? 0 : p.preview ? 1 : best.has(p.key) ? 2 : 3);
    return [...order].sort((a, b) => rank(a) - rank(b) || a.seconds - b.seconds);
  });

  type Placed = { left: number; top: number };
  type Box = [number, number, number, number];
  /** Each label's place, where it hits no other label, mark or axis; none where there's no room. */
  const placed = $derived.by((): Map<string, Placed> => {
    const out = new Map<string, Placed>();
    if (!width) return out;
    const boxes: Box[] = [];
    const hits = (b: Box) => boxes.some((p) => !(b[2] + 8 <= p[0] || b[0] >= p[2] + 8 || b[3] <= p[1] || b[1] >= p[3]));
    const markAt = (p: RoutePoint) => [xOf(p.seconds), plot ? yOf(p.quality) : LINE_Y] as const;
    const onMark = (b: Box, self: string) =>
      order.some((p) => {
        if (p.key === self) return false;
        const [mx, my] = markAt(p);
        return b[0] - 12 < mx && mx < b[2] + 12 && b[1] - 12 < my && my < b[3] + 12;
      });
    if (slower.length) {
      const w = textWidth(slowerText) + 4;
      const top = plot ? PLOT_TOP + PLOT_H - LABEL_H - 4 : LINE_Y - LABEL_H / 2;
      boxes.push([width - w, top, width, top + LABEL_H]);
    }
    if (!plot) {
      const tw = textWidth(`Target ${target} s`) + 4;
      boxes.push([tx - tw / 2, LANE_TOP[2], tx + tw / 2, LANE_TOP[2] + LABEL_H]);
      for (const p of ranked) {
        const w = labelWidth(p);
        const cx = xOf(p.seconds);
        const left = Math.max(0, Math.min(cx - w / 2, width - w));
        const top = LANE_TOP.find((t) => !hits([left, t, left + w, t + LABEL_H]));
        if (top === undefined) continue;
        boxes.push([left, top, left + w, top + LABEL_H]);
        out.set(p.key, { left, top });
      }
      return out;
    }
    // The axis labels hold their places first: the quality range's ends, the target's and the line's end.
    const base = PLOT_TOP + PLOT_H + 6;
    const axisTop = PLOT_TOP - LABEL_H - 4;
    const tw = textWidth(`Target ${target} s`) + 4;
    boxes.push(
      [0, axisTop, textWidth(`Quality ${plot.max}`) + 4, axisTop + LABEL_H],
      [0, base, textWidth(String(plot.min)) + 4, base + LABEL_H],
      [tx - tw / 2, base, tx + tw / 2, base + LABEL_H],
      [width - textWidth(sec(end)) - 4, base, width, base + LABEL_H],
    );
    for (const p of ranked) {
      const w = labelWidth(p);
      const [px, py] = markAt(p);
      const mid = py - LABEL_H / 2;
      const spots: [number, number][] = [
        [px + 16, mid],
        [px - 16 - w, mid],
        [px - w / 2, py - 14 - LABEL_H],
        [px - w / 2, py + 14],
        [px + 8, py - 14 - LABEL_H],
        [px - 8 - w, py - 14 - LABEL_H],
        [px + 8, py + 14],
        [px - 8 - w, py + 14],
      ];
      const spot = spots.find(([l, t]) => {
        const b: Box = [l, t, l + w, t + LABEL_H];
        return l >= 0 && l + w <= width && t >= axisTop && t + LABEL_H <= base + LABEL_H && !hits(b) && !onMark(b, p.key);
      });
      if (!spot) continue;
      const [left, top] = spot;
      boxes.push([left, top, left + w, top + LABEL_H]);
      out.set(p.key, { left, top });
    }
    return out;
  });
  const height = $derived(plot ? PLOT_TOP + PLOT_H + 28 : LANE_TOP[1] + LABEL_H + 2);

  const line = $derived(
    frontier
      .map((k) => order.find((p) => p.key === k))
      .filter((p): p is RoutePoint => !!p)
      .map((p) => `${xOf(p.seconds).toFixed(1)},${yOf(p.quality).toFixed(1)}`)
      .join(" "),
  );

  /** The route the keys start from: the current one, else the first. */
  let active = $state<number | null>(null);
  const focusAt = $derived(
    order.length ? Math.min(active ?? Math.max(0, order.findIndex((p) => p.current)), order.length - 1) : -1,
  );

  function onkeydown(e: KeyboardEvent) {
    const next = rove(e, focusAt, order.length);
    if (next === null) return;
    active = next;
    root?.querySelectorAll<HTMLButtonElement>(".pt")[next]?.focus();
  }

  function choose(p: RoutePoint) {
    if (p.current || p.blocked) return;
    onchoose?.(p.key);
  }
</script>

<!-- One tab stop; arrows, Home and End move between routes. -->
<div
  bind:this={root}
  bind:clientWidth={width}
  class={["route-line", plot && "route-line--plot"]}
  style:height="{height}px"
  role="listbox"
  aria-orientation="horizontal"
  aria-label={label}
  tabindex="-1"
  {onkeydown}
>
  {#if width}
    {#if plot}
      <span class="grid" style:top="{PLOT_TOP}px" aria-hidden="true"></span>
      <span class="grid grid--base" style:top="{PLOT_TOP + PLOT_H}px" aria-hidden="true"></span>
      <span class="axis num" style:top="{PLOT_TOP - LABEL_H - 4}px" aria-hidden="true">Quality {plot.max}</span>
      <span class="axis num" style:top="{PLOT_TOP + PLOT_H + 6}px" aria-hidden="true">{plot.min}</span>
      <span class="axis axis--end num" style:top="{PLOT_TOP + PLOT_H + 6}px" aria-hidden="true">{sec(end)}</span>
      <span class="rule" style:left="{tx}px" style:top="{PLOT_TOP}px" style:bottom="0" aria-hidden="true"></span>
      <span class="target-label num" style:left="{tx}px" style:top="{PLOT_TOP + PLOT_H + 6}px" aria-hidden="true"
        >Target {target} s</span
      >
      {#if line}
        <svg class="frontier" {width} {height} viewBox="0 0 {width} {height}" aria-hidden="true">
          <polyline points={line} />
        </svg>
      {/if}
    {:else}
      <span class="track" style:top="{LINE_Y - 2}px" aria-hidden="true"></span>
      <span class="rule" style:left="{tx}px" style:top="{LANE_TOP[2] + LABEL_H}px" style:bottom="0" aria-hidden="true"
      ></span>
      <span class="tick" style:left="{tx}px" style:top="{LINE_Y - 6}px" aria-hidden="true"></span>
      <span class="target-label num" style:left="{tx}px" style:top="{LANE_TOP[2]}px" aria-hidden="true"
        >Target {target} s</span
      >
    {/if}
    {#if slower.length}
      <span
        class="slower num"
        style:top="{plot ? PLOT_TOP + PLOT_H - LABEL_H - 4 : LINE_Y - LABEL_H / 2}px"
        role="img"
        aria-label="{slower.length} more past {sec(end)}: {slower.map((p) => `${p.label} ${sec(p.seconds)}`).join(', ')}"
        {@attach tip({
          title: `Past ${sec(end)}`,
          rows: slower.map((p) => ({ label: p.label, value: sec(p.seconds) })),
          note: "Choose one from the route menu.",
        })}>{slowerText}</span
      >
    {/if}
    {#each order as p, i (p.key)}
      {@const at = placed.get(p.key)}
      <button
        type="button"
        class={["pt", p.current && "pt--current", p.preview && "pt--preview", p.blocked && !p.current && "pt--blocked"]}
        style:left="{xOf(p.seconds)}px"
        style:top="{plot ? yOf(p.quality) : LINE_Y}px"
        role="option"
        aria-selected={!!p.current}
        aria-disabled={!!p.blocked && !p.current}
        aria-label={p.description}
        tabindex={i === focusAt ? 0 : -1}
        onclick={() => choose(p)}
        onfocus={() => {
          active = i;
          onpreview?.(p.key);
        }}
        onblur={() => onpreview?.(null)}
        onpointerenter={() => onpreview?.(p.key)}
        onpointerleave={() => onpreview?.(null)}
        {@attach tip(() => p.tip)}
      >
        <span class="mk"><CompanyMark size={20} src={p.icon} /></span>
      </button>
      {#if at}
        <span class={["lab", p.blocked && !p.current && "lab--blocked"]} style:left="{at.left}px" style:top="{at.top}px" aria-hidden="true">
          <span class={["lab__text", p.current && "lab__text--current", p.preview && "lab__text--preview"]}>
            {p.label} <span class="num lab__s">{sec(p.seconds)}</span>
          </span>
          {#if p.invalid}<span class="lab__alert"><Icon icon={CircleAlert} size={12} weight="medium" /></span>{/if}
          {#if p.blocked && !p.current}<span class="lab__s">· {p.blocked}</span>{/if}
        </span>
      {/if}
    {/each}
  {/if}
</div>

<style>
  .route-line {
    position: relative;
    min-width: 0;
    outline: none;
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
  }
  /* The line: a 4px sunken track, as TimeTrack's. */
  .track {
    position: absolute;
    inset-inline: 0;
    height: var(--space-1);
    border: 1px solid transparent;
    border-radius: var(--radius-full);
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .tick {
    position: absolute;
    width: 1px;
    height: var(--space-3);
    translate: -0.5px 0;
    background: var(--ink-2);
  }
  .rule {
    position: absolute;
    width: 0;
    border-inline-start: 1px dashed var(--line-strong);
  }
  .target-label,
  .axis {
    position: absolute;
    font-size: var(--text-xs);
    line-height: var(--leading-xs);
    color: var(--ink-3);
    white-space: nowrap;
  }
  .target-label {
    translate: -50% 0;
  }
  .axis {
    inset-inline-start: 0;
  }
  .axis--end {
    inset-inline: auto 0;
  }
  .grid {
    position: absolute;
    inset-inline: 0;
    height: 1px;
    background: var(--line);
  }
  .grid--base {
    background: var(--line-strong);
  }
  .frontier {
    position: absolute;
    inset: 0;
    overflow: visible;
    pointer-events: none;
  }
  .frontier polyline {
    fill: none;
    stroke: var(--ink-3);
    stroke-width: 1;
    stroke-dasharray: 3 3;
  }
  /* Past the line's end: a count on the surface at its end, the routes in its tooltip. */
  .slower {
    position: absolute;
    inset-inline-end: 0;
    padding-inline: var(--space-1);
    border-radius: var(--radius-xs);
    background: var(--surface);
    color: var(--ink-3);
    white-space: nowrap;
  }
  /* A route: its agent's mark on a 24px hit area centered on its place. */
  .pt {
    position: absolute;
    display: grid;
    place-items: center;
    width: var(--control-sm);
    height: var(--control-sm);
    padding: 0;
    border: 0;
    translate: -50% -50%;
    background: none;
    color: inherit;
    font: inherit;
    cursor: default;
    outline: none;
    -webkit-tap-highlight-color: transparent;
  }
  .mk {
    display: grid;
    border-radius: var(--radius-xs);
    background: var(--surface);
    transition-property: box-shadow;
    transition-duration: var(--dur-instant);
    transition-timing-function: var(--ease-out);
  }
  .pt--current .mk {
    box-shadow:
      0 0 0 2px var(--surface),
      0 0 0 3.5px var(--accent);
  }
  .pt--preview .mk {
    outline: 1.5px dashed var(--accent);
    outline-offset: 3px;
  }
  /* The focus ring's 2px gap in the card's surface, not the window's paper. */
  .pt:focus-visible .mk {
    box-shadow:
      0 0 0 2px var(--surface),
      0 0 0 4px var(--accent);
  }
  @media (hover: hover) {
    .pt:not(.pt--current, .pt--blocked):hover .mk {
      box-shadow:
        0 0 0 2px var(--surface),
        0 0 0 3.5px var(--line-strong);
    }
  }
  /* Its label: the model and its time, on the surface so the rule and frontier pass behind it. */
  .lab {
    position: absolute;
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    padding-inline: 2px;
    border-radius: var(--radius-xs);
    background: var(--surface);
    color: var(--ink-2);
    white-space: nowrap;
    pointer-events: none;
  }
  .lab__s {
    color: var(--ink-3);
  }
  .lab__text--current,
  .lab__text--current .lab__s {
    color: var(--accent-text);
    font-weight: var(--weight-medium);
  }
  .lab__text--preview {
    color: var(--ink);
  }
  .lab__alert {
    display: inline-flex;
    color: var(--warn);
  }
  .lab--blocked .lab__text {
    color: var(--ink-3);
  }

  @media (forced-colors: active) {
    .tick,
    .grid {
      forced-color-adjust: none;
      background: CanvasText;
    }
    .pt--current .mk {
      outline: 2px solid Highlight;
      outline-offset: 2px;
    }
    .pt:focus-visible .mk {
      outline: 2px solid CanvasText;
      outline-offset: 4px;
    }
  }
</style>
