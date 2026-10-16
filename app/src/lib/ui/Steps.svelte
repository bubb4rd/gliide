<!--
  Steps: where a sequence is (the first-run setup). A 4px bar per step over its name; the steps done and the current
  one filled in accent, which sweeps in from the leading edge as you move on (and back out as you go back). Done steps
  are buttons back to themselves when `onselect` is given. Under 480px (where six names would truncate) they give way
  to one caption, "Step 2 of 6 · Appearance".
-->
<script lang="ts">
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

  const at = $derived(Math.min(current, steps.length - 1));
</script>

<div class="steps">
  <ol class="list" aria-label={label} style:--count={steps.length}>
    {#each steps as step, i (i)}
      {@const state = i < current ? "done" : i === current ? "current" : "next"}
      <li class={["step", `step--${state}`]} aria-current={state === "current" ? "step" : undefined}>
        {#if onselect && state === "done"}
          <button type="button" class="cell" onclick={() => onselect(i)}>
            <span class="bar" aria-hidden="true"></span>
            <span class="name">{step}<span class="visually-hidden">, done</span></span>
          </button>
        {:else}
          <span class="cell">
            <span class="bar" aria-hidden="true"></span>
            <span class="name">{step}{#if state === "done"}<span class="visually-hidden">, done</span>{/if}</span>
          </span>
        {/if}
      </li>
    {/each}
  </ol>
  <p class="caption meta num" aria-hidden="true">
    {#if current >= steps.length}All {steps.length} done{:else}Step {at + 1} of {steps.length} · {steps[at]}{/if}
  </p>
</div>

<style>
  .steps {
    container-type: inline-size;
  }
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
  .cell {
    display: grid;
    gap: var(--space-2);
    width: 100%;
    padding: 0;
    border: 0;
    border-radius: var(--radius-xs);
    font: inherit;
    text-align: start;
    color: var(--ink-3);
    background: none;
    cursor: default;
    transition: color var(--dur-instant) var(--ease-out);
  }
  .bar {
    position: relative;
    height: 4px;
    overflow: hidden;
    border-radius: var(--radius-full);
    background: var(--line-strong);
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
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
  }
  .step--done .cell {
    color: var(--ink-2);
  }
  .step--current .cell {
    color: var(--ink);
    font-weight: var(--weight-medium);
  }
  .caption {
    display: none;
    margin: var(--space-2) 0 0;
  }
  @container (width < 480px) {
    .name {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
    }
    .cell {
      gap: 0;
      padding-block: var(--space-1);
    }
    .caption {
      display: block;
    }
  }
  @media (hover: hover) {
    button.cell:hover {
      color: var(--ink);
    }
  }
</style>
