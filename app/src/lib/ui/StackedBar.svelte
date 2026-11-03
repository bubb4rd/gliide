<!--
  StackedBar: up to four parts of a whole as one 8px pill across the width, each part a pill 2px apart sized by its
  share, shaded by the data steps from the accent (the first) to the lightest (the fourth). A part of 0 is left out;
  any other shows at least 2px. Nothing at all: the empty sunken track. Its parts and shares are in its accessible
  name; a legend beside it uses `StepKey` with `segmentStep`.
-->
<script module lang="ts">
  /** The data step a segment is shaded in, by its place: the first the accent (4), the fourth the lightest (1). */
  export function segmentStep(index: number): 1 | 2 | 3 | 4 {
    return Math.max(1, 4 - index) as 1 | 2 | 3 | 4;
  }
</script>

<script lang="ts">
  interface Props {
    /** In order, up to four. */
    segments: { label: string; value: number }[];
    /** What it shows, for assistive tech ("Tokens by kind"). */
    label: string;
  }

  let { segments, label }: Props = $props();

  const parts = $derived(
    segments.slice(0, 4).map((s, i) => ({ ...s, value: Math.max(0, s.value), step: segmentStep(i) })),
  );
  const total = $derived(parts.reduce((sum, s) => sum + s.value, 0));
  const shown = $derived(parts.filter((s) => s.value > 0));
  const name = $derived(
    total ? `${label}: ${shown.map((s) => `${s.label} ${Math.round((s.value / total) * 100)}%`).join(", ")}` : label,
  );
</script>

<div class={["stacked-bar", !total && "stacked-bar--empty"]} role="img" aria-label={name}>
  {#each shown as part (part.step)}
    <span class="seg" style:flex-grow={part.value} style:background="var(--accent-step-{part.step})"></span>
  {/each}
</div>

<style>
  .stacked-bar {
    display: flex;
    gap: var(--space-0-5);
    height: var(--space-2);
    min-width: 0;
  }
  .stacked-bar--empty {
    border: 1px solid transparent;
    border-radius: var(--radius-full);
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .seg {
    flex-basis: 0;
    min-width: 2px;
    border-radius: var(--radius-full);
  }

  @media (forced-colors: active) {
    .seg {
      forced-color-adjust: none;
    }
  }
</style>
