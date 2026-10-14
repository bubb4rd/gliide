<!--
  The score: now → projected in the serif, the ceiling, and a bar (now | gain | headroom). Updates are instant: the
  numbers and the bar change with every toggle, so they never animate. Hover or focus the numbers for the parts: one
  line each (icon, name, share of the score, how much is met), its gist under it; met in full turns green. Parts the
  posting doesn't ask for are named in one line at the foot.
-->
<script lang="ts">
  import type { Component, Snippet } from "svelte";
  import type { LucideProps } from "@lucide/svelte";
  import Briefcase from "@lucide/svelte/icons/briefcase";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import FileCheck from "@lucide/svelte/icons/file-check";
  import GraduationCap from "@lucide/svelte/icons/graduation-cap";
  import IdCard from "@lucide/svelte/icons/id-card";
  import MessagesSquare from "@lucide/svelte/icons/messages-square";
  import Wrench from "@lucide/svelte/icons/wrench";
  import Icon from "./Icon.svelte";
  import Popover from "./Popover.svelte";
  import { tip } from "./tip.svelte";

  interface Part {
    /** `hard_tool`, `domain_soft`, `title`, `experience`, `education`, `format`: picks the icon. */
    id: string;
    label: string;
    /** 0–1. */
    value: number;
    weight: number;
    /** The posting specifies it; absent parts don't count. */
    present: boolean;
    /** The gist in a few words ("2 of 3 · missing Docker"). */
    brief: string;
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

  const ICONS: Record<string, Component<LucideProps>> = {
    hard_tool: Wrench,
    domain_soft: MessagesSquare,
    title: IdCard,
    experience: Briefcase,
    education: GraduationCap,
    format: FileCheck,
  };

  const id = $props.id();
  let numbers = $state<HTMLElement>();
  let open = $state(false);

  const clamp = (n: number) => Math.min(100, Math.max(0, n));
  const nowPct = $derived(Math.round(clamp(now)));
  const projectedPct = $derived(Math.round(clamp(projected)));
  const ceilingPct = $derived(Math.round(clamp(ceiling)));
  const gain = $derived(Math.max(0, projectedPct - nowPct));
  const counted = $derived(components.filter((c) => c.present));
  const absent = $derived(components.filter((c) => !c.present));
  const weightTotal = $derived(counted.reduce((s, c) => s + c.weight, 0));
</script>

{#snippet figures()}
  <span class="value value--now num" {@attach tip(nowTitle)}>{nowPct}<span class="unit">%</span></span>
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
      <div class="part parts__head">
        <span class="label">What the score counts</span>
        <span class="label part__share">Weight</span>
        <span class="label parts__met">Met</span>
      </div>
      {#each counted as part (part.label)}
        {@const met = Math.round(clamp(part.value * 100))}
        <div class={["part", met === 100 && "part--full", met === 0 && "part--none"]}>
          <span class="part__icon">{#if ICONS[part.id]}<Icon icon={ICONS[part.id]} />{/if}</span>
          <span class="part__label">{part.label}</span>
          <span class="part__share meta num">
            {weightTotal > 0 ? Math.round((part.weight / weightTotal) * 100) : 0}%
          </span>
          <span class="part__value num">
            <span class="part__meter" aria-hidden="true"><span style:width="{met}%"></span></span>{met}%
          </span>
          <span class="part__brief meta">{part.brief}</span>
        </div>
      {/each}
      {#if absent.length}
        <p class="meta parts__absent">Not in the posting: {absent.map((p) => p.label).join(", ")}</p>
      {/if}
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
    gap: var(--space-2);
    padding: var(--space-3) var(--space-3) var(--space-3) var(--space-2);
  }
  /* icon | name and gist | share | met; the head shares the columns, its label across the icon's. */
  .part {
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr) 36px 64px;
    column-gap: var(--space-2);
    align-items: baseline;
  }
  .parts__head {
    padding-block-end: var(--space-1);
    border-bottom: 1px solid var(--line);
  }
  .parts__head > :first-child {
    grid-column: 1 / 3;
    padding-inline-start: var(--space-1);
  }
  .part__icon {
    grid-row: 1 / 3;
    align-self: start;
    display: flex;
    padding-top: 2px;
    color: var(--ink-3);
  }
  .part__label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: var(--weight-medium);
  }
  .part__share,
  .parts__met {
    text-align: end;
  }
  /* The percent sets the row's baseline; the meter centers on it. */
  .part__value {
    display: flex;
    align-items: baseline;
    justify-content: flex-end;
    gap: var(--space-1-5);
    font-weight: var(--weight-medium);
    color: var(--ink);
  }
  .part__meter {
    align-self: center;
    flex: none;
    width: 24px;
    height: 6px;
    border-radius: var(--radius-full);
    border: 1px solid transparent;
    overflow: hidden;
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .part__meter > span {
    display: block;
    height: 100%;
    border-radius: var(--radius-full);
    background: var(--ink-3);
  }
  .part__brief {
    grid-column: 2 / -1;
    text-wrap: pretty;
  }
  .part--full .part__icon,
  .part--full .part__value {
    color: var(--gain-text);
  }
  .part--full .part__meter > span {
    background: var(--gain);
  }
  .part--none .part__value {
    color: var(--ink-3);
  }
  .parts__absent {
    margin: 0;
    padding-block-start: var(--space-2);
    padding-inline-start: var(--space-1);
    border-top: 1px solid var(--line);
  }
</style>
