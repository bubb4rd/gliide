<!--
  Steps: where a sequence is (the first-run setup). A 4px bar per step, no names on screen: the steps done and the
  current one filled in accent, which sweeps in from the leading edge as you move on (and back out as you go back).
  Each step's name is its tooltip and what assistive tech reads. Done steps are buttons back to themselves when
  `onselect` is given.
-->
<script lang="ts">
  import { tip } from "./tip.svelte";

  interface Props {
    /** Each step's name, in order. */
    steps: string[];
    /** The step you're on, from 0; `steps.length` once all are done. */
    current: number;
    /** What the sequence is, for assistive tech ("Setup"). */
    label: string;
    /** Makes the done steps buttons that go back to them. */
    onselect?: (index: number) => void;
  }

  let { steps, current, label, onselect }: Props = $props();
</script>

<ol class="list" aria-label={label} style:--count={steps.length}>
  {#each steps as step, i (i)}
    {@const state = i < current ? "done" : i === current ? "current" : "next"}
    <li class={["step", `step--${state}`]} aria-current={state === "current" ? "step" : undefined}>
      {#if onselect && state === "done"}
        <button type="button" class="cell" {@attach tip(step)} onclick={() => onselect(i)}>
          <span class="bar" aria-hidden="true"></span>
          <span class="visually-hidden">{step}, done</span>
        </button>
      {:else}
        <span class="cell" {@attach tip(step)}>
          <span class="bar" aria-hidden="true"></span>
          <span class="visually-hidden">{step}{#if state === "done"}, done{/if}</span>
        </span>
      {/if}
    </li>
  {/each}
</ol>

<style>
  .list {
    display: grid;
    grid-template-columns: repeat(var(--count), minmax(0, 1fr));
    gap: var(--space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .step {
    min-width: 0;
  }
  /* The 4px bar in a 20px cell, so it's a target worth aiming at. */
  .cell {
    display: grid;
    width: 100%;
    padding: var(--space-2) 0;
    border: 0;
    border-radius: var(--radius-xs);
    font: inherit;
    background: none;
    cursor: default;
  }
  .bar {
    position: relative;
    height: 4px;
    border-radius: var(--radius-full);
    background: var(--line-strong);
    overflow: hidden;
  }
  /* Reduced motion: the fill fades in place. */
  .bar::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: var(--accent);
    opacity: 0;
    transition: opacity var(--dur-base) var(--ease-out);
  }
  .step--done .bar::after,
  .step--current .bar::after {
    opacity: 1;
  }
  @media (prefers-reduced-motion: no-preference) {
    .bar::after {
      opacity: 1;
      scale: 0 1;
      transform-origin: 0 50%;
      transition: scale var(--dur-base) var(--ease-emphasized);
    }
    .step--done .bar::after,
    .step--current .bar::after {
      scale: 1 1;
    }
  }
  /* A done step you can go back to: its fill eases toward the track on hover. */
  @media (hover: hover) {
    button.cell:hover .bar::after {
      opacity: 0.72;
    }
  }
</style>
