<!--
  A model call in flight: the stage ("Suggesting edits…"), a bar, the seconds so far, and Cancel. With an expected
  duration the bar is determinate: it fills to 90% by the expected time, then creeps toward the end without reaching
  it, so it never claims to be done before the result arrives. Without one, a segment sweeps the track (a pulse under
  reduced motion). The bar is the only motion, and the stage text says the same thing.
-->
<script lang="ts">
  import Button from "./Button.svelte";

  interface Props {
    /** The stage, as a sentence ("Reading the requirements…"). */
    label: string;
    /** About how long it takes; null or omitted: indeterminate. */
    expectedMs?: number | null;
    /** When it started (`performance.now()` clock); defaults to when this mounted. */
    startedAt?: number;
    oncancel?: () => void;
  }

  let { label, expectedMs = null, startedAt, oncancel }: Props = $props();

  const mountedAt = performance.now();
  let elapsed = $state(0);

  $effect(() => {
    const start = startedAt ?? mountedAt;
    const tick = () => (elapsed = performance.now() - start);
    tick();
    const timer = setInterval(tick, 100);
    return () => clearInterval(timer);
  });

  const fraction = $derived.by(() => {
    if (!expectedMs) return null;
    const t = elapsed / expectedMs;
    return t <= 1 ? 0.9 * t : 0.9 + 0.09 * (1 - Math.exp(-(t - 1)));
  });
  const seconds = $derived(Math.floor(elapsed / 1000));
</script>

<div class="progress" role="status">
  <div class="head">
    <span class="label-text">{label}</span>
    <span class="meta num"
      >{seconds} s{#if expectedMs}&thinsp;/ about {Math.round(expectedMs / 1000)} s{/if}</span
    >
    {#if oncancel}<Button variant="ghost" size="sm" onclick={oncancel}>Cancel</Button>{/if}
  </div>
  <div
    class={["track", fraction === null && "track--indeterminate"]}
    role="progressbar"
    aria-label={label}
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={fraction === null ? undefined : Math.round(fraction * 100)}
  >
    <span class="fill" style:width={fraction === null ? undefined : `${fraction * 100}%`}></span>
  </div>
</div>

<style>
  .progress {
    display: grid;
    gap: var(--space-2);
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: var(--control-sm);
  }
  .label-text {
    font-weight: var(--weight-medium);
  }
  .head .meta {
    margin-inline-end: auto;
  }
  .track {
    position: relative;
    height: 4px;
    border: 1px solid transparent;
    border-radius: var(--radius-full);
    overflow: hidden;
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .fill {
    position: absolute;
    inset-block: 0;
    left: 0;
    border-radius: var(--radius-full);
    background: var(--accent);
    transition: width var(--dur-instant) var(--ease-out);
  }
  .track--indeterminate .fill {
    width: 30%;
    animation: fade-pulse var(--dur-loop) var(--ease-out) infinite alternate;
  }
  @media (prefers-reduced-motion: no-preference) {
    .track--indeterminate .fill {
      animation: sweep var(--dur-loop) var(--ease-emphasized) infinite;
    }
  }
  @keyframes sweep {
    from {
      translate: -100% 0;
    }
    to {
      translate: 340% 0;
    }
  }
  @keyframes fade-pulse {
    from {
      opacity: 0.35;
    }
  }
</style>
