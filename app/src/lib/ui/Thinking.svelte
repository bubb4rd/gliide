<!--
  A model call in flight, the way assistants show they're working: the stage ("Suggesting edits…") with a band of the
  accent sweeping through its ink, the seconds it has been at it, Cancel; above it, the steps of this run already
  done, each with a check and its seconds. No bar and no icon: the state is the words, the sweep says it's alive.
  Reduced motion: the stage stays in the accent and fades in and out, nothing moves.

  `center` (a pane being written, inside a `WorkingRing`): centered in its box, the stage line held still on the
  box's middle, the steps stacked above it and Cancel under it, so a new stage never moves what you're reading. A
  stage that finishes leaves with `exit`; the next enters as `.enter` once it's gone, and the finished one's step
  enters above.
-->
<script lang="ts">
  import Check from "@lucide/svelte/icons/check";
  import { untrack } from "svelte";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import { exit } from "./motion";

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
    /** Centered in its box, the stage line on its middle (a pane being written). */
    center?: boolean;
    oncancel?: () => void;
  }

  let { label, steps = [], startedAt, center = false, oncancel }: Props = $props();

  const mountedAt = performance.now();
  let elapsed = $state(0);
  // What was on screen when it mounted shows at once; what arrives later enters.
  const firstLabel = untrack(() => label);
  const firstSteps = untrack(() => steps.length);

  $effect(() => {
    const start = startedAt ?? mountedAt;
    const tick = () => (elapsed = performance.now() - start);
    tick();
    const timer = setInterval(tick, 250);
    return () => clearInterval(timer);
  });

  const seconds = $derived(Math.floor(elapsed / 1000));
</script>

{#snippet stage()}
  <span class="stage sweep">{label}</span>
  <span class="meta num">{seconds} s</span>
{/snippet}

<div class={["thinking", center && "thinking--center"]} role="status">
  {#if steps.length}
    <div class="steps">
      {#each steps as step, i (i)}
        <div class={["step meta", center && i >= firstSteps && "enter"]}>
          <span class="tick"><Icon icon={Check} size={14} weight="medium" /></span>
          <span>{step.label}</span>
          <span class="num">{Math.max(1, Math.round(step.ms / 1000))} s</span>
        </div>
      {/each}
    </div>
  {/if}
  {#if center}
    <div class="stack">
      {#key label}
        <div class={["now", label !== firstLabel && "enter"]} out:exit>{@render stage()}</div>
      {/key}
    </div>
    {#if oncancel}
      <div class="cancel"><Button variant="ghost" size="sm" onclick={oncancel}>Cancel</Button></div>
    {/if}
  {:else}
    <div class="now">
      {@render stage()}
      {#if oncancel}<Button variant="ghost" size="sm" onclick={oncancel}>Cancel</Button>{/if}
    </div>
  {/if}
</div>

<style>
  .thinking,
  .steps {
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

  .stage {
    font-weight: var(--weight-medium);
  }

  /* Out of the flow above and below, so the stage line alone is what centers. */
  .thinking--center {
    position: relative;
    justify-items: center;
  }
  .thinking--center .steps {
    position: absolute;
    bottom: 100%;
    left: 50%;
    translate: -50% 0;
    justify-items: center;
    width: max-content;
    padding-bottom: var(--space-1);
  }
  .stack {
    display: grid;
    justify-items: center;
  }
  /* The leaving stage and the arriving one share the line; the next waits for the exit. */
  .stack > .now {
    grid-area: 1 / 1;
    white-space: nowrap;
  }
  .stack > .now > .meta {
    margin-inline-end: 0;
  }
  .stack > .enter {
    animation-delay: var(--dur-fast);
  }
  .cancel {
    position: absolute;
    top: 100%;
    left: 50%;
    translate: -50% 0;
    padding-top: var(--space-2);
  }
</style>
