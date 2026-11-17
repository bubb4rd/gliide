<!--
  BarChart, the ledger: a column per entry across the full width, equal widths (8px apart up to 14, 2px up to 60, 1px
  past), each bar in `--accent` or stacked from the baseline up in its `parts`, shaded as `StackedBar`'s segments (the
  first the accent, then the lighter steps; a 1px slit between parts once each has 4px), top corners rounded 4px (at
  most half the bar's width). Any value above 0 shows at least 2px; 0 keeps a 2px stub on the baseline, so a quiet day
  reads as a day. One scale mark: a dashed rule at the busiest bar's top, its value (`format`) over the rule's trailing
  end. A bar's `tick` under it (the first and last at the chart's edges once the bars are narrow, the rest centered).
  Hovering or focusing a column dims the other bars and opens its `tip` beside it. One tab stop: ←/→ move between
  bars, Home/End to the ends. Nothing to show: no bars, the baseline alone; all 0, the stubs, no rule. Updates are
  instant; the dimming fades over `--dur-fast`.
-->
<script module lang="ts">
  import type { TipContent } from "./tip.svelte";

  export interface ChartBar {
    key: string;
    /** The bar's height against the busiest. */
    value: number;
    /** Up to four parts of the value, from the baseline up; their shares split the bar. Without, one accent bar. */
    parts?: number[];
    /** A label under the bar; give one to a few bars, not each. */
    tick?: string;
    tip: TipContent;
  }
</script>

<script lang="ts">
  import { segmentStep } from "./StackedBar.svelte";
  import { tip } from "./tip.svelte";

  interface Props {
    bars: ChartBar[];
    /** The busiest bar's value, over the rule ("$6.20"); also each bar's accessible value. */
    format: (value: number) => string;
    /** What the chart shows, for assistive tech ("Estimated cost by day, last 30 days"). */
    label: string;
    /** The plot's height in px. */
    height?: number;
  }

  let { bars, format, label, height = 128 }: Props = $props();

  /** A slit between parts once each has this much room. */
  const SLIT_ROOM = 4;

  const peak = $derived(Math.max(0, ...bars.map((b) => b.value)));
  const ticked = $derived(bars.some((b) => b.tick));
  const spacing = $derived(bars.length > 60 ? "dense" : bars.length > 14 ? "narrow" : "wide");

  /** A bar's height in px and its parts, from the baseline up; parts of 0 are left out. */
  function stack(bar: ChartBar) {
    const px = Math.max(2, (bar.value / peak) * height);
    const parts = (bar.parts ?? [])
      .slice(0, 4)
      .map((value, i) => ({ value: Math.max(0, value), step: segmentStep(i) }))
      .filter((p) => p.value > 0);
    return { px, parts, slit: parts.length > 1 && px >= parts.length * SLIT_ROOM };
  }

  /** The bar that holds the tab stop: the last one focused, else the last. */
  let active = $state<number | null>(null);
  const current = $derived(bars.length ? Math.min(active ?? bars.length - 1, bars.length - 1) : -1);

  const barLabel = (bar: ChartBar) =>
    typeof bar.tip === "string" ? bar.tip : [bar.tip.title, format(bar.value)].filter(Boolean).join(", ");

  /** Where a tick sits under its bar: at the chart's edges for the first and last once bars are narrow. */
  function tickAlign(i: number): string {
    if (spacing === "wide") return "center";
    return i === 0 ? "start" : i === bars.length - 1 ? "end" : "center";
  }

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
  <span class="peak" aria-hidden="true">{peak ? format(peak) : ""}</span>
  <!-- The bars are one tab stop; arrows, Home and End move along them. -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="plot plot--{spacing}" role="group" aria-label={label} {onkeydown}>
    {#if peak}<span class="rule"></span>{/if}
    {#each bars as bar, i (bar.key)}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <span
        class="col"
        role="img"
        aria-label={barLabel(bar)}
        tabindex={i === current ? 0 : -1}
        onfocus={() => (active = i)}
        {@attach tip(() => bar.tip, "beside")}
      >
        {#if bar.value > 0 && peak}
          {@const s = stack(bar)}
          <span class={["bar", s.slit && "bar--slit"]} style:height="{s.px}px">
            {#each s.parts as part (part.step)}
              <span class="part" style:flex-grow={part.value} style:background="var(--accent-step-{part.step})"></span>
            {/each}
          </span>
        {:else}
          <span class="stub"></span>
        {/if}
      </span>
    {/each}
  </div>
  {#if ticked}
    <div class="ticks plot--{spacing}" aria-hidden="true">
      {#each bars as bar, i (bar.key)}
        {#if bar.tick}<span style:grid-column={i + 1} style:justify-self={tickAlign(i)}>{bar.tick}</span>{/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  .bar-chart {
    display: grid;
    grid-template-rows: auto var(--height) auto;
    min-width: 0;
    font-size: var(--text-xs);
    line-height: var(--leading-xs);
    font-variant-numeric: tabular-nums;
    color: var(--ink-3);
  }
  /* The busiest bar's value, over the rule's trailing end; its row kept while empty, so the plot never moves. */
  .peak {
    min-height: var(--leading-xs);
    margin-bottom: var(--space-0-5);
    text-align: end;
    white-space: nowrap;
  }
  .plot,
  .ticks {
    display: grid;
    grid-template-columns: repeat(var(--bars), minmax(0, 1fr));
  }
  .plot--wide {
    column-gap: var(--space-2);
  }
  .plot--narrow {
    column-gap: var(--space-0-5);
  }
  .plot--dense {
    column-gap: 1px;
  }
  /* The baseline lies just under the bars. */
  .plot {
    position: relative;
    box-shadow: 0 1px 0 var(--line-strong);
  }
  /* A dashed hairline at the busiest bar's top: 2px dashes, 2px apart. */
  .rule {
    position: absolute;
    inset-inline: 0;
    top: 0;
    height: 1px;
    background: repeating-linear-gradient(to right, var(--line-strong) 0 2px, transparent 2px 4px);
  }
  .col {
    position: relative;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    min-width: 0;
    border-radius: var(--radius-xs) var(--radius-xs) 0 0;
  }
  .bar {
    display: flex;
    flex-direction: column-reverse;
    overflow: hidden;
    border-radius: min(var(--radius-xs), 50%) min(var(--radius-xs), 50%) 0 0;
    background: var(--accent);
    transition-property: opacity;
    transition-duration: var(--dur-fast);
    transition-timing-function: var(--ease-out);
  }
  .bar--slit {
    gap: 1px;
    background: transparent;
  }
  .part {
    flex-basis: 0;
  }
  .stub {
    height: 2px;
    border-radius: 1px 1px 0 0;
    background: var(--line-strong);
  }
  /* The column in hand keeps its ink; the other bars step back. */
  .plot:has(.col:focus-visible) .col:not(:focus-visible) .bar {
    opacity: var(--data-dim);
  }
  @media (hover: hover) {
    .plot:has(.col:hover) .col:not(:hover) .bar {
      opacity: var(--data-dim);
    }
  }
  .ticks {
    margin-top: var(--space-1-5);
  }
  .ticks span {
    grid-row: 1;
    min-width: 0;
    white-space: nowrap;
  }

  @media (forced-colors: active) {
    .plot {
      box-shadow: none;
      border-bottom: 1px solid CanvasText;
    }
    .rule,
    .bar,
    .part,
    .stub {
      forced-color-adjust: none;
    }
    .rule,
    .stub {
      background: GrayText;
    }
    .bar {
      background: Highlight;
    }
  }
</style>
