<!--
  SeriesKey: a legend's swatch for a chart's series, drawn as its marks are, so a key matches the chart beside it: a
  dot (a scatter's point), a ring (a hollow point: partial, or a stand-in), a line, dashed or dotted (a trend's line),
  a bar (a ranked bar), an outline (a bar that stands in). In `color` (a token, `var(--maker-google)`), ink 3 without.
  Decorative: the label beside it names the series.
-->
<script lang="ts">
  interface Props {
    kind?: "dot" | "ring" | "line" | "dashed" | "dotted" | "bar" | "outline";
    /** A CSS color from the tokens (`var(--maker-anthropic)`). */
    color?: string;
  }

  let { kind = "dot", color = "var(--ink-3)" }: Props = $props();
</script>

{#if kind === "line" || kind === "dashed" || kind === "dotted"}
  <svg class="series-key series-key--line" viewBox="0 0 16 8" aria-hidden="true" style:--c={color}>
    <line x1="1" x2="15" y1="4" y2="4" class={kind} />
  </svg>
{:else}
  <span class="series-key series-key--{kind}" style:--c={color} aria-hidden="true"></span>
{/if}

<style>
  .series-key {
    display: inline-block;
    flex: none;
    vertical-align: middle;
  }
  .series-key--dot,
  .series-key--ring {
    width: var(--space-2);
    height: var(--space-2);
    border-radius: var(--radius-full);
  }
  .series-key--dot {
    background: var(--c);
  }
  .series-key--ring {
    box-shadow: inset 0 0 0 1.5px var(--c);
  }
  .series-key--bar,
  .series-key--outline {
    width: var(--space-2);
    height: var(--space-2);
    border-radius: calc(var(--radius-xs) / 2);
  }
  .series-key--bar {
    background: var(--c);
  }
  .series-key--outline {
    box-shadow: inset 0 0 0 1px var(--c);
  }
  .series-key--line {
    width: var(--space-4);
    height: var(--space-2);
    overflow: visible;
  }
  line {
    stroke: var(--c);
    stroke-width: 2;
    stroke-linecap: round;
  }
  .dashed {
    stroke-dasharray: 4 3;
  }
  .dotted {
    stroke-dasharray: 0.5 3.5;
  }

  @media (forced-colors: active) {
    .series-key {
      forced-color-adjust: none;
    }
  }
</style>
