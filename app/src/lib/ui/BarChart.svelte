<!--
  BarChart: one accent bar per entry (a data series, not "good"), equal widths across the width, 2px apart (1px past
  60 bars), rounded tops; any value above 0 shows at least 2px, 0 shows nothing. Hairline gridlines at round steps of
  the busiest bar (the baseline stronger), each labeled with `format` at the trailing edge; a bar's `tick` under it.
  Hovering or focusing a bar washes its column and shows its `tip`. One tab stop: ←/→ move between bars, Home/End to
  the ends, the focused bar showing its tooltip. Nothing to show (no bars, or all 0): the baseline alone. Updates are
  instant: no motion.
-->
<script module lang="ts">
  import type { TipContent } from "./tip.svelte";

  export interface ChartBar {
    key: string;
    value: number;
    /** A label under the bar, from its leading edge; give one to a few bars, not each. */
    tick?: string;
    tip: TipContent;
  }
</script>

<script lang="ts">
  import { tip } from "./tip.svelte";

  interface Props {
    bars: ChartBar[];
    /** A gridline's label ("$2"). */
    format: (value: number) => string;
    /** What the chart shows, for assistive tech ("Estimated cost by day, last 30 days"). */
    label: string;
    /** The plot's height in px. */
    height?: number;
  }

  let { bars, format, label, height = 128 }: Props = $props();

  /** At most three gridlines over the baseline, at 1, 2, 2.5 or 5 times a power of ten. */
  function niceStep(max: number): number {
    const raw = max / 3;
    const power = 10 ** Math.floor(Math.log10(raw));
    const f = raw / power;
    return (f <= 1 ? 1 : f <= 2 ? 2 : f <= 2.5 ? 2.5 : f <= 5 ? 5 : 10) * power;
  }

  const scale = $derived.by(() => {
    const max = Math.max(0, ...bars.map((b) => b.value));
    if (!max) return { top: 0, lines: [0] };
    const step = niceStep(max);
    const steps = Math.ceil(max / step - 1e-9);
    return { top: steps * step, lines: Array.from({ length: steps + 1 }, (_, i) => +(i * step).toPrecision(12)) };
  });
  const ticked = $derived(bars.some((b) => b.tick));

  /** The bar that holds the tab stop: the last one focused, else the last. */
  let active = $state<number | null>(null);
  const current = $derived(bars.length ? Math.min(active ?? bars.length - 1, bars.length - 1) : -1);

  const barLabel = (bar: ChartBar) =>
    typeof bar.tip === "string" ? bar.tip : [bar.tip.title, format(bar.value)].filter(Boolean).join(", ");

  function onkeydown(e: KeyboardEvent) {
    const last = bars.length - 1;
    const next =
      e.key === "ArrowLeft"
        ? Math.max(0, current - 1)
        : e.key === "ArrowRight"
          ? Math.min(last, current + 1)
          : e.key === "Home"
            ? 0
            : e.key === "End"
              ? last
              : null;
    if (next === null || last < 0) return;
    e.preventDefault();
    active = next;
    (e.currentTarget as HTMLElement).querySelectorAll<HTMLElement>(".col")[next]?.focus();
  }
</script>

<div class="bar-chart" style:--bars={bars.length} style:--height="{height}px">
  <!-- The bars are one tab stop; arrows, Home and End move along them. -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class={["plot", bars.length > 60 && "plot--dense"]} role="group" aria-label={label} {onkeydown}>
    {#each scale.lines as line (line)}
      <span class={["line", line === 0 && "line--base"]} style:--at={scale.top ? line / scale.top : 0}></span>
    {/each}
    {#each bars as bar, i (bar.key)}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <span
        class="col"
        role="img"
        aria-label={barLabel(bar)}
        tabindex={i === current ? 0 : -1}
        onfocus={() => (active = i)}
        {@attach tip(() => bar.tip)}
      >
        {#if bar.value > 0 && scale.top}
          <span class="bar" style:--at={bar.value / scale.top}></span>
        {/if}
      </span>
    {/each}
  </div>
  <div class="y" aria-hidden="true">
    {#if scale.top}
      {#each scale.lines as line (line)}
        <span style:--at={line / scale.top}>{format(line)}</span>
      {/each}
    {/if}
  </div>
  {#if ticked}
    <div class={["ticks", bars.length > 60 && "plot--dense"]} aria-hidden="true">
      {#each bars as bar, i (bar.key)}
        {#if bar.tick}<span style:grid-column={i + 1}>{bar.tick}</span>{/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  .bar-chart {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    grid-template-rows: var(--height) auto;
    min-width: 0;
    font-size: var(--text-xs);
    line-height: var(--leading-xs);
    font-variant-numeric: tabular-nums;
    color: var(--ink-3);
  }
  .plot,
  .ticks {
    display: grid;
    grid-template-columns: repeat(var(--bars), minmax(0, 1fr));
    column-gap: var(--space-0-5);
  }
  .plot--dense {
    column-gap: 1px;
  }
  .plot {
    position: relative;
  }
  /* A gridline's bottom edge sits 1px under its value, so the baseline lies just under the bars. */
  .line {
    position: absolute;
    inset-inline: 0;
    bottom: calc(var(--at) * 100% - 1px);
    height: 1px;
    background: var(--line);
  }
  .line--base {
    background: var(--line-strong);
  }
  .col {
    position: relative;
    display: flex;
    align-items: flex-end;
    min-width: 0;
    border-radius: calc(var(--radius-xs) / 2) calc(var(--radius-xs) / 2) 0 0;
  }
  .col:focus-visible {
    background: var(--hover);
  }
  @media (hover: hover) {
    .col:hover {
      background: var(--hover);
    }
  }
  .bar {
    width: 100%;
    height: calc(var(--at) * 100%);
    min-height: 2px;
    border-radius: inherit;
    background: var(--accent);
  }
  /* The labels share one cell (its width the widest), each centered on its gridline. */
  .y {
    display: grid;
    grid-template-rows: minmax(0, 1fr);
  }
  .y span {
    position: relative;
    grid-area: 1 / 1;
    align-self: end;
    bottom: calc(var(--at) * 100%);
    translate: 0 50%;
    padding-inline-start: var(--space-2);
    white-space: nowrap;
  }
  .ticks {
    grid-column: 1;
    margin-top: var(--space-1-5);
  }
  .ticks span {
    grid-row: 1;
    min-width: 0;
    white-space: nowrap;
  }

  @media (forced-colors: active) {
    .line,
    .bar {
      forced-color-adjust: none;
    }
    .line {
      background: GrayText;
    }
    .line--base {
      background: CanvasText;
    }
    .bar {
      background: Highlight;
    }
  }
</style>
