<!--
  A model call in flight, the way assistants show they're working: the stage ("Suggesting edits…") with a band of the
  accent sweeping through its ink, the seconds it has been at it, Cancel; above it, the steps of this run already
  done, each with a check and its seconds. No bar and no icon: the state is the words, the sweep says it's alive.
  Reduced motion: the stage stays in the accent and fades in and out, nothing moves.
-->
<script lang="ts">
  import Check from "@lucide/svelte/icons/check";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";

  interface Step {
    /** What was done, past tense ("Read the requirements"). */
    label: string;
    ms: number;
  }

  interface Props {
    /** The stage, as a sentence ("Reading the requirements…"). */
    label: string;
    /** Steps of this run already done, in order. */
    steps?: Step[];
    /** When the stage started (`performance.now()` clock); defaults to when this mounted. */
    startedAt?: number;
    oncancel?: () => void;
  }

  let { label, steps = [], startedAt, oncancel }: Props = $props();

  const mountedAt = performance.now();
  let elapsed = $state(0);

  $effect(() => {
    const start = startedAt ?? mountedAt;
    const tick = () => (elapsed = performance.now() - start);
    tick();
    const timer = setInterval(tick, 250);
    return () => clearInterval(timer);
  });

  const seconds = $derived(Math.floor(elapsed / 1000));
</script>

<div class="thinking" role="status">
  {#each steps as step, i (i)}
    <div class="step meta">
      <span class="tick"><Icon icon={Check} size={14} weight="medium" /></span>
      <span>{step.label}</span>
      <span class="num">{Math.max(1, Math.round(step.ms / 1000))} s</span>
    </div>
  {/each}
  <div class="now">
    <span class="stage">{label}</span>
    <span class="meta num">{seconds} s</span>
    {#if oncancel}<Button variant="ghost" size="sm" onclick={oncancel}>Cancel</Button>{/if}
  </div>
</div>

<style>
  .thinking {
    display: grid;
    gap: var(--space-1);
  }
  .step {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--ink-3);
  }
  .tick {
    display: inline-flex;
    color: var(--gain-text);
  }
  .now {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: var(--control-sm);
  }
  .now > .meta {
    margin-inline-end: auto;
  }

  /* Reduced motion (and forced colors, where the text keeps its own color): the stage in the accent, fading. */
  .stage {
    font-weight: var(--weight-medium);
    color: var(--accent-text);
    animation: fade-breathe var(--dur-loop) var(--ease-out) infinite alternate;
  }
  @keyframes fade-breathe {
    from {
      opacity: 0.55;
    }
  }

  /* Otherwise: ink, with a band of the accent sweeping through it left to right. */
  @media (prefers-reduced-motion: no-preference) and (forced-colors: none) {
    .stage {
      background: linear-gradient(
          90deg,
          var(--ink-2) 0%,
          var(--ink-2) 28%,
          var(--accent-text) 50%,
          var(--ink-2) 72%,
          var(--ink-2) 100%
        )
        0 0 / 250% 100%;
      background-clip: text;
      -webkit-background-clip: text;
      color: transparent;
      animation: sweep var(--dur-loop) linear infinite;
    }
  }
  @keyframes sweep {
    from {
      background-position: 100% 0;
    }
    to {
      background-position: 0 0;
    }
  }
</style>
