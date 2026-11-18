<!--
  How long a call typically takes against its target (Settings → Benchmark's Typical time): a 4px sunken track, 24
  wide, running from 0 to 4× the target (where a call's speed scores nothing), the median in ink 3 over the slowest 1
  in 10 in `--line-strong`, a 1px tick in ink 2 at the target (a quarter of the way along); the median's seconds after
  it (tabular, one decimal); with `yours`, your own calls' median under it in meta ("Yours 41.2 s"). Anything past 4×
  the target fills the track. Neutral, never green or red: the seconds say how far off it is. Updates are instant. The
  rest props go on its element, so `tip()` attaches to it.
-->
<script lang="ts">
  import type { HTMLAttributes } from "svelte/elements";

  interface Props extends HTMLAttributes<HTMLSpanElement> {
    /** The median call, in seconds. */
    seconds: number;
    /** The slowest 1 in 10, in seconds. */
    slow?: number | null;
    /** The role's target, in seconds. */
    target: number;
    /** Your own calls' median on this Mac, in seconds. */
    yours?: number | null;
  }

  let { seconds, slow = null, target, yours = null, class: className, ...rest }: Props = $props();

  const at = (s: number) => Math.min(1, Math.max(0, s / (4 * target)));
  const sec = (s: number) => `${s.toFixed(1)} s`;
</script>

<span
  class={["time-track", className]}
  role="img"
  aria-label="Typical {sec(seconds)}, target {target} s{yours !== null ? `, yours ${sec(yours)}` : ''}"
  {...rest}
>
  <span class="top">
    <span class="track" aria-hidden="true">
      {#if slow !== null && slow > 0}<span class="fill fill--slow" style:--at={at(slow)}></span>{/if}
      {#if seconds > 0}<span class="fill" style:--at={at(seconds)}></span>{/if}
      <span class="tick"></span>
    </span>
    <span class="num seconds">{sec(seconds)}</span>
  </span>
  {#if yours !== null}<span class="meta num">Yours {sec(yours)}</span>{/if}
</span>

<style>
  .time-track {
    display: inline-grid;
    justify-items: end;
    vertical-align: middle;
  }
  .top {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
  }
  .track {
    position: relative;
    flex: none;
    width: var(--space-6);
    height: var(--space-1);
    border: 1px solid transparent;
    border-radius: var(--radius-full);
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  /* Over the track's edge, from its outer start; any time above 0 shows 2px. */
  .fill {
    position: absolute;
    inset-block: -1px;
    inset-inline-start: -1px;
    width: calc(var(--at) * (100% + 2px));
    min-width: 2px;
    border-radius: var(--radius-full);
    background: var(--ink-3);
  }
  .fill--slow {
    background: var(--line-strong);
  }
  /* The target: a quarter of the way along, 12px tall over the 4px track. */
  .tick {
    position: absolute;
    inset-block: calc(-1 * var(--space-1) - 1px);
    inset-inline-start: calc(25% - 1px);
    width: 1px;
    background: var(--ink-2);
  }
  /* "37.9 s": five characters' room, so the tracks of a column line up (past 99.9 s, the track moves 1ch). */
  .seconds {
    min-width: 5ch;
    text-align: end;
  }

  @media (forced-colors: active) {
    .fill,
    .tick {
      forced-color-adjust: none;
      background: CanvasText;
    }
    .fill--slow {
      background: GrayText;
    }
  }
</style>
