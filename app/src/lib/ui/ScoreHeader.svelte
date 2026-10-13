<!--
  The score: now → projected in the serif, the ceiling, and a bar (now | gain | headroom). Updates are instant: the
  numbers and the bar change with every toggle, so they never animate. Hover or focus the numbers for the parts.
-->
<script lang="ts">
  import type { Snippet } from "svelte";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Icon from "./Icon.svelte";
  import Popover from "./Popover.svelte";

  interface Part {
    label: string;
    /** 0–1. */
    value: number;
    weight: number;
    /** The posting specifies it; absent parts don't count. */
    present: boolean;
    detail: string;
  }

  interface Props {
    /** 0–100. */
    now: number;
    projected: number;
    ceiling: number;
    components?: Part[];
    /** What "now" is ("Your SWE master as it is"). */
    nowTitle?: string;
    /** The line under the bar ("+25 points from 4 changes · fits on 1 page"). */
    children?: Snippet;
  }

  let { now, projected, ceiling, components = [], nowTitle, children }: Props = $props();

  const id = $props.id();
  let numbers = $state<HTMLElement>();
  let open = $state(false);

  const clamp = (n: number) => Math.min(100, Math.max(0, n));
  const nowPct = $derived(Math.round(clamp(now)));
  const projectedPct = $derived(Math.round(clamp(projected)));
  const ceilingPct = $derived(Math.round(clamp(ceiling)));
  const gain = $derived(Math.max(0, projectedPct - nowPct));
  const weightTotal = $derived(components.filter((c) => c.present).reduce((s, c) => s + c.weight, 0));
</script>

{#snippet figures()}
  <span class="value value--now num" title={nowTitle}>{nowPct}<span class="unit">%</span></span>
  <Icon icon={ChevronRight} class="arrow" />
  <span class="value value--projected num">{projectedPct}<span class="unit">%</span></span>
  <span class="ceiling"><span class="meta">Ceiling</span><strong class="num">{ceilingPct}%</strong></span>
{/snippet}

<div class="score">
  {#if components.length}
    <button
      bind:this={numbers}
      type="button"
      class="numbers"
      aria-label="Now {nowPct}%, projected {projectedPct}%, ceiling {ceilingPct}%"
      aria-describedby="{id}-parts"
      aria-expanded={open}
      onclick={() => (open = true)}
      onpointerenter={(e) => e.pointerType === "mouse" && (open = true)}
      onpointerleave={(e) => e.pointerType === "mouse" && (open = false)}
      onfocus={() => (open = true)}
      onblur={() => (open = false)}
      onkeydown={(e) => {
        if (e.key !== "Escape" || !open) return;
        open = false;
        e.stopPropagation();
      }}
    >
      {@render figures()}
    </button>
  {:else}
    <div class="numbers" role="group" aria-label="Now {nowPct}%, projected {projectedPct}%, ceiling {ceilingPct}%">
      {@render figures()}
    </div>
  {/if}
  <div class="bar" aria-hidden="true">
    <span class="bar__now" style:width="{nowPct}%"></span>
    {#if gain > 0}<span class="bar__gain" style:width="{gain}%"></span>{/if}
  </div>
  {#if children}<div class="meta below">{@render children()}</div>{/if}
</div>

{#if components.length}
  <Popover bind:open anchor={numbers} mode="manual" id="{id}-parts" role="tooltip">
    <div class="parts__body">
      <p class="label">What the score counts</p>
      {#each components as part (part.label)}
        <div class={["part", !part.present && "part--absent"]}>
          <span class="part__label">{part.label}</span>
          <span class="part__value num">
            {#if part.present}{Math.round(part.value * 100)}%{:else}Not asked{/if}
          </span>
          <span class="part__track" aria-hidden="true">
            {#if part.present}<span class="part__fill" style:width="{clamp(part.value * 100)}%"></span>{/if}
          </span>
          <span class="part__detail meta">
            {part.present && weightTotal > 0
              ? `${Math.round((part.weight / weightTotal) * 100)}% of the score · ${part.detail}`
              : part.detail}
          </span>
        </div>
      {/each}
    </div>
  </Popover>
{/if}

<style>
  .score {
    display: grid;
    gap: var(--space-3);
  }
  .numbers {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    width: 100%;
    padding: 0;
    border: 0;
    border-radius: var(--radius-sm);
    font: inherit;
    text-align: start;
    color: inherit;
    background: transparent;
    cursor: default;
  }
  .value {
    font-family: var(--font-display);
    font-optical-sizing: auto;
    font-size: var(--text-score);
    line-height: var(--leading-score);
    letter-spacing: var(--tracking-score);
    font-weight: var(--weight-regular);
    font-variant-numeric: tabular-nums;
  }
  .value--now {
    font-size: var(--text-headline);
    line-height: 1;
    color: var(--ink-3);
  }
  .value--projected {
    color: var(--ink);
  }
  .numbers :global(.arrow) {
    align-self: center;
    color: var(--ink-4);
  }
  .unit {
    margin-inline-start: 2px;
    font-size: 0.5em;
    letter-spacing: 0;
    color: var(--ink-3);
  }
  .ceiling {
    margin-inline-start: auto;
    text-align: end;
  }
  .ceiling strong {
    display: block;
    font: var(--weight-medium) var(--text-lg) / var(--leading-lg) var(--font-ui);
    font-variant-numeric: tabular-nums;
    color: var(--ink-2);
  }

  /* now | gain | headroom; widths from the scores, updated instantly. */
  .bar {
    display: flex;
    gap: 2px;
    height: 8px;
    border: 1px solid transparent;
    border-radius: var(--radius-full);
    overflow: hidden;
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .bar__now {
    border-radius: var(--radius-full);
    background: var(--ink-3);
  }
  .bar__gain {
    border-radius: var(--radius-full);
    background: var(--gain);
  }
  .below :global(strong) {
    color: var(--gain-text);
    font-weight: var(--weight-medium);
  }

  /* Width on the body, not the popover: a global `.parts` rule would reach every screen's `.parts`. */
  .parts__body {
    width: 320px;
    display: grid;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4) var(--space-4);
  }
  .part {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: var(--space-1) var(--space-3);
  }
  .part__label {
    font-weight: var(--weight-medium);
  }
  .part__value {
    color: var(--ink-2);
  }
  .part__track {
    grid-column: 1 / -1;
    height: 4px;
    border: 1px solid transparent;
    border-radius: var(--radius-full);
    overflow: hidden;
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .part__fill {
    display: block;
    height: 100%;
    border-radius: var(--radius-full);
    background: var(--ink-3);
  }
  .part__detail {
    grid-column: 1 / -1;
    text-wrap: pretty;
  }
  .part--absent .part__label,
  .part--absent .part__value {
    color: var(--ink-3);
  }
</style>
