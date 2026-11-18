<!--
  A 0–100 score that isn't a fit (Settings → Benchmark's leaderboard): an 8px sunken pill, 24 wide, filled in ink 3 up
  to the score, then the score as a whole number (500, ink) at the cell's edge. Neutral: never green or accent, since
  a higher score here is a ranking, not points you gained. Any score above 0 fills at least 2px. Updates are instant.
  The rest props go on its element, so `tip()` attaches to it.
-->
<script lang="ts">
  import type { HTMLAttributes } from "svelte/elements";

  interface Props extends HTMLAttributes<HTMLSpanElement> {
    /** 0–100, unrounded; shown rounded. */
    value: number;
    /** The score in words, for assistive tech ("Score 91"). */
    label: string;
  }

  let { value, label, class: className, ...rest }: Props = $props();

  const share = $derived(Math.min(100, Math.max(0, value)));
</script>

<span class={["score-pill", className]} role="img" aria-label={label} {...rest}>
  <span class="track" aria-hidden="true">
    {#if share > 0}<span class="fill" style:--share={share / 100}></span>{/if}
  </span>
  <span class="value num">{Math.round(share)}</span>
</span>

<style>
  .score-pill {
    display: inline-flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--space-2);
    vertical-align: middle;
  }
  .track {
    position: relative;
    flex: none;
    width: var(--space-6);
    height: var(--space-2);
    border: 1px solid transparent;
    border-radius: var(--radius-full);
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  /* Over the track's edge: the pill's whole 8px, from its outer start. */
  .fill {
    position: absolute;
    inset-block: -1px;
    inset-inline-start: -1px;
    width: calc(var(--share) * (100% + 2px));
    min-width: 2px;
    border-radius: var(--radius-full);
    background: var(--ink-3);
  }
  /* Three digits wide, so 9, 91 and 100 keep the pill in place. */
  .value {
    min-width: 3ch;
    text-align: end;
    font-weight: var(--weight-medium);
    color: var(--ink);
  }

  @media (forced-colors: active) {
    .fill {
      forced-color-adjust: none;
      background: CanvasText;
    }
  }
</style>
