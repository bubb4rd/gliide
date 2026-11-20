<!--
  The score: now → projected in the serif, the ceiling, and a bar (now | gain | headroom). Interactive updates (toggling
  a change) are instant: no CSS width transition. During a posting's first load, `now`, `projected` and `ceiling` can be
  null: stage 1 shows the empty bar and static skeleton bones; stage 2 fills the baseline (`0%` → `now%`) once Extract
  scores your master; stage 3 (or mounting with `reveal` right after loading) sweeps the gain (`0%` → `gain%`) and rolls
  the projected score up from `now` over `--dur-roll` `--ease-emphasized` (reduced motion fades only). Hover or focus
  the numbers for the parts: one line each (icon, name, share of the score, how much is met), its gist under it; met in
  full turns green. Parts the posting doesn't ask for are named in one line at the foot.
-->
<script lang="ts">
  import { untrack, type Component, type Snippet } from "svelte";
  import type { LucideProps } from "@lucide/svelte";
  import Briefcase from "@lucide/svelte/icons/briefcase";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import FileCheck from "@lucide/svelte/icons/file-check";
  import GraduationCap from "@lucide/svelte/icons/graduation-cap";
  import IdCard from "@lucide/svelte/icons/id-card";
  import MessagesSquare from "@lucide/svelte/icons/messages-square";
  import Wrench from "@lucide/svelte/icons/wrench";
  import Icon from "./Icon.svelte";
  import { fillBar, ROLL_CELLS, roll, rollDigits, token, type RollDigit } from "./motion";
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
    /** 0–100, or null while Extract is still scoring the master. */
    now?: number | null;
    /** 0–100, or null while Suggest is still estimating the tailored score. */
    projected?: number | null;
    /** 0–100, or null while Suggest is still estimating the ceiling. */
    ceiling?: number | null;
    components?: Part[];
    /** What "now" is ("Your SWE master as it is"). */
    nowTitle?: string;
    /**
     * Animate the score arriving from loading:
     * - Mounted with `now` and `projected == null`: `.bar__now` fills from 0% to `now%`.
     * - Mounted with `now` and `projected` (right after loading): `.bar__now` starts at `now%`, `.bar__gain` fills
     *   from 0% to `gain%`, the projected figure rolls up (on a rise) or enters, and ceiling/below fade in.
     */
    reveal?: boolean;
    /** The line under the bar ("+25 points from 4 changes · fits on 1 page"). */
    children?: Snippet;
  }

  let {
    now = null,
    projected = null,
    ceiling = null,
    components = [],
    nowTitle,
    reveal = false,
    children,
  }: Props = $props();

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
  const nowPct = $derived(now === null ? null : Math.round(clamp(now)));
  const projectedPct = $derived(projected === null ? null : Math.round(clamp(projected)));
  const ceilingPct = $derived(ceiling === null ? null : Math.round(clamp(ceiling)));
  const gain = $derived(nowPct !== null && projectedPct !== null ? Math.max(0, projectedPct - nowPct) : 0);
  const loading = $derived(nowPct === null || projectedPct === null);
  const counted = $derived(components.filter((c) => c.present));
  const absent = $derived(components.filter((c) => !c.present));
  const weightTotal = $derived(counted.reduce((s, c) => s + c.weight, 0));

  const groupLabel = $derived(
    nowPct === null
      ? "Calculating fit score"
      : projectedPct === null
        ? `Now ${nowPct}%, estimating projected score`
        : ceilingPct === null
          ? `Now ${nowPct}%, projected ${projectedPct}%`
          : `Now ${nowPct}%, projected ${projectedPct}%, ceiling ${ceilingPct}%`,
  );

  // ── Staged loading arrival (stage 1 → 2 baseline fill, stage 2 → 3 projected roll & gain fill) ──
  let rolling = $state<RollDigit[] | null>(null);
  let nowEntered = $state(false);
  let projEntered = $state(false);
  /** Non-zero generation triggers a one-shot fill attachment on the bar segment; 0 cancels any in-flight fill. */
  let nowAnim = $state<{ gen: number; target: number }>({ gen: 0, target: 0 });
  let gainAnim = $state<{ gen: number; target: number }>({ gen: 0, target: 0 });
  let gen = 0;
  let rollTimer: ReturnType<typeof setTimeout> | null = null;

  function barSweep(req: { gen: number; target: number }) {
    return (node: HTMLElement) => {
      if (!req.gen || req.target <= 0) return;
      const anim = fillBar(node, req.target);
      return () => anim.cancel();
    };
  }

  function stopNow() {
    if (untrack(() => nowAnim.gen)) nowAnim = { gen: 0, target: 0 };
  }

  function stopProj() {
    if (untrack(() => gainAnim.gen)) gainAnim = { gen: 0, target: 0 };
    if (rollTimer !== null) {
      clearTimeout(rollTimer);
      rollTimer = null;
    }
    rolling = null;
  }

  function animateNow(target: number) {
    nowEntered = true;
    nowAnim = target > 0 ? { gen: ++gen, target } : { gen: 0, target: 0 };
  }

  function animateProj(from: number | null, to: number, delta: number) {
    stopProj();
    projEntered = true;
    if (delta > 0) gainAnim = { gen: ++gen, target: delta };
    if (from !== null && to > from && !matchMedia("(prefers-reduced-motion: reduce)").matches) {
      rolling = rollDigits(from, to);
      rollTimer = setTimeout(() => {
        rolling = null;
        rollTimer = null;
      }, token("--dur-roll"));
    }
  }

  let mounted = false;
  let lastNow: number | null = null;
  let lastProj: number | null = null;

  $effect(() => {
    const n = nowPct;
    const p = projectedPct;
    const g = gain;

    if (!mounted) {
      mounted = true;
      lastNow = n;
      lastProj = p;
      const shouldReveal = untrack(() => reveal);
      if (shouldReveal) {
        if (n !== null && p === null) animateNow(n);
        else if (p !== null) animateProj(n, p, g);
      }
      return;
    }

    const prevNow = lastNow;
    const prevProj = lastProj;
    lastNow = n;
    lastProj = p;

    if (prevNow === null && n !== null) {
      animateNow(n);
    } else if (prevNow !== n) {
      stopNow();
      if (n === null) nowEntered = false;
    }

    if (prevProj === null && p !== null) {
      animateProj(n, p, g);
    } else if (prevProj !== p) {
      stopProj();
      if (p === null) projEntered = false;
    }
  });

  $effect(() => () => {
    if (rollTimer !== null) {
      clearTimeout(rollTimer);
      rollTimer = null;
    }
  });
</script>

{#snippet figures()}
  {#if nowPct !== null}
    <span class={["value value--now num", nowEntered && "enter-fade"]} {@attach tip(nowTitle)}
      >{nowPct}<span class="unit">%</span></span
    >
  {:else}
    <span class="value value--now value--bone" aria-hidden="true"><span class="bone bone--now"></span></span>
  {/if}
  <Icon icon={ChevronRight} class="arrow" />
  {#if projectedPct !== null}
    <span class={["value value--projected num", projEntered && !rolling && "enter"]}>
      {#if rolling}<span class="roll"
          >{#each rolling as digit, i (i)}<span class="digit"
              ><span class="strip" {@attach roll(digit.from, digit.to)}
                >{#each ROLL_CELLS as cell, j (j)}<span>{cell}</span>{/each}</span
              ></span
            >{/each}</span
        >{:else}{projectedPct}{/if}<span class="unit">%</span>
    </span>
  {:else}
    <span class="value value--projected value--bone" aria-hidden="true"
      ><span class="bone bone--projected"></span></span
    >
  {/if}
  <span class="ceiling">
    <span class="meta">Ceiling</span>
    {#if ceilingPct !== null}
      <strong class={["num", projEntered && "enter-fade"]}>{ceilingPct}%</strong>
    {:else}
      <strong class="ceiling__bone" aria-hidden="true"><span class="bone bone--ceiling"></span></strong>
    {/if}
  </span>
{/snippet}

<div class="score" aria-busy={loading || undefined}>
  {#if !loading && components.length}
    <button
      bind:this={numbers}
      type="button"
      class="numbers"
      aria-label={groupLabel}
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
    <div class="numbers" role="group" aria-label={groupLabel}>
      {@render figures()}
    </div>
  {/if}
  <div class="bar" aria-hidden="true">
    {#if nowPct !== null && nowPct > 0}
      <span class="bar__now" style:width="{nowPct}%" {@attach barSweep(nowAnim)}></span>
    {/if}
    {#if gain > 0}
      <span class="bar__gain" style:width="{gain}%" {@attach barSweep(gainAnim)}></span>
    {/if}
  </div>
  {#if loading}
    <div class="meta below below--bone" aria-hidden="true"><span class="bone bone--below"></span></div>
  {:else if children}
    <div class={["meta below", projEntered && "enter-fade"]}>{@render children()}</div>
  {/if}
</div>

{#if !loading && components.length}
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
  /* Zero-width strut keeps every skeleton bone on the exact typographic baseline and line height of its number. */
  .value--bone::before,
  .ceiling__bone::before,
  .below--bone::before {
    content: "\200b";
  }
  .bone {
    display: inline-block;
    vertical-align: baseline;
    border: 1px solid transparent;
    border-radius: var(--radius-xs);
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .bone--now {
    width: 44px;
    height: 0.7em;
  }
  .bone--projected {
    width: 76px;
    height: 0.68em;
  }
  .bone--ceiling {
    width: 34px;
    height: 0.75em;
  }
  .bone--below {
    width: 184px;
    height: 10px;
    vertical-align: middle;
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

  /* Rolling digits for the arriving projected score (same odometer window as ScoreBar). */
  .roll {
    display: inline-flex;
    vertical-align: top;
  }
  .digit {
    box-sizing: content-box;
    height: 1lh;
    padding-block: var(--space-1);
    margin-block: calc(-1 * var(--space-1));
    overflow: clip;
    mask-image: linear-gradient(
      transparent,
      currentColor var(--space-1),
      currentColor calc(100% - var(--space-1)),
      transparent
    );
  }
  .strip {
    display: flex;
    flex-direction: column;
  }

  .enter-fade {
    animation: fade var(--dur-base) var(--ease-out) both;
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  /* now | gain | headroom; interactive toggles update instantly, initial load fills via Web Animations. */
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
    flex: none;
    border-radius: var(--radius-full);
    background: var(--ink-3);
  }
  .bar__gain {
    flex: none;
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

  @media (forced-colors: active) {
    .bar__now {
      forced-color-adjust: none;
      background: CanvasText;
    }
    .bar__gain {
      forced-color-adjust: none;
      background: Highlight;
    }
  }
</style>
