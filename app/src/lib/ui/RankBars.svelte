<!--
  RankBars, a small leaderboard: an entry a row, its maker's 16px mark, its name (truncates) and its value at the
  trailing edge (tabular), over a 6px bar in its series color running from the start, sized on a shared scale (linear,
  or log for costs that span decades), any value at least 2px. The entries come ranked; the first complete one leads
  (name and value 500, ink). Partial entries (something in them stands in) follow under a "Partial" label, in ink 3
  with their bars at half strength. An estimated value (a stand-in in it) is an outlined bar. A `target` is a 1×12 tick
  across each bar at its value. One tab stop: ↑↓ Home End move between rows, each opening its `tip`. No motion.
-->
<script module lang="ts">
  import type { TipContent } from "./tip.svelte";

  export interface RankItem {
    key: string;
    /** The maker's icon; null for the blank tile. */
    icon: string | null;
    name: string;
    value: number;
    /** The value as shown ("$2.04"). */
    text: string;
    /** The series color, a token (`var(--maker-anthropic)`). */
    color: string;
    /** Something in it stands in: listed under Partial. */
    partial?: boolean;
    /** Its value has a stand-in in it: an outlined bar. */
    estimated?: boolean;
    tip: TipContent;
  }
</script>

<script lang="ts">
  import CompanyMark from "./CompanyMark.svelte";
  import { rove } from "./rove";
  import { tip } from "./tip.svelte";

  interface Props {
    items: RankItem[];
    /** Where a bar is full; `log` from `min` (a value at or under it is the 2px stub). */
    scale: { max: number; min?: number; log?: boolean };
    /** A tick across each bar at its value; `label` names it for assistive tech ("target 58 s"). */
    target?: { value: number; label: string };
    /** What it ranks, for assistive tech ("Quality, gliide Index"). */
    label: string;
  }

  let { items, scale, target, label }: Props = $props();

  /** A value's share of the track, 0–1. */
  function share(v: number) {
    if (scale.log) {
      const lo = Math.log10(Math.max(scale.min ?? 0.01, 1e-9));
      const hi = Math.log10(Math.max(scale.max, 1e-9));
      return hi > lo ? Math.min(1, Math.max(0, (Math.log10(Math.max(v, 1e-9)) - lo) / (hi - lo))) : 0;
    }
    return scale.max > 0 ? Math.min(1, Math.max(0, v / scale.max)) : 0;
  }

  const groups = $derived(
    [
      { id: "complete", label: "", items: items.filter((i) => !i.partial) },
      { id: "partial", label: "Partial", items: items.filter((i) => i.partial) },
    ].filter((g) => g.items.length),
  );
  const order = $derived(groups.flatMap((g) => g.items));
  const leader = $derived(items.find((i) => !i.partial)?.key ?? null);

  let active = $state<number | null>(null);
  const current = $derived(order.length ? Math.min(active ?? 0, order.length - 1) : -1);
  let list: HTMLElement;

  function onkeydown(e: KeyboardEvent) {
    const next = rove(e, current, order.length);
    if (next === null) return;
    active = next;
    list.querySelectorAll<HTMLElement>(".row")[next]?.focus();
  }
</script>

<!-- One tab stop; arrows, Home and End move between rows. -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="rank-bars" role="group" aria-label={label} bind:this={list} {onkeydown}>
  {#each groups as g (g.id)}
    {#if g.label}<p class="label group">{g.label}</p>{/if}
    <div class="rows">
      {#each g.items as item (item.key)}
        {@const i = order.indexOf(item)}
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <div
          class={["row", item.partial && "row--partial", item.key === leader && "row--lead"]}
          role="img"
          aria-label="{item.name}, {item.text}{target ? `, ${target.label}` : ''}"
          tabindex={i === current ? 0 : -1}
          onfocus={() => (active = i)}
          {@attach tip(() => item.tip)}
        >
          <span class="head">
            <CompanyMark size={16} src={item.icon} />
            <span class="name">{item.name}</span>
            <span class="value num">{item.text}</span>
          </span>
          <span class="track">
            <span
              class={["bar", item.estimated && "bar--estimated"]}
              style:--c={item.color}
              style:width="max(2px, {share(item.value) * 100}%)"
            ></span>
            {#if target}<span class="target" style:left="{share(target.value) * 100}%"></span>{/if}
          </span>
        </div>
      {/each}
    </div>
  {/each}
</div>

<style>
  .rank-bars {
    display: grid;
    min-width: 0;
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
  }
  .group {
    margin: var(--space-3) 0 var(--space-1);
    color: var(--ink-3);
  }
  .rows {
    position: relative;
    display: grid;
    gap: var(--space-2);
  }
  .row {
    display: grid;
    gap: var(--space-1);
    min-width: 0;
    border-radius: var(--radius-xs);
    outline-offset: 2px;
  }
  .row:focus-visible {
    outline: 2px solid var(--accent);
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    color: var(--ink-2);
  }
  .value {
    flex: none;
    color: var(--ink-2);
  }
  .row--lead .name,
  .row--lead .value {
    color: var(--ink);
    font-weight: var(--weight-medium);
  }
  .row--partial .name,
  .row--partial .value {
    color: var(--ink-3);
  }
  .track {
    position: relative;
    display: block;
    height: 6px;
  }
  .bar {
    display: block;
    height: 100%;
    border-radius: var(--radius-full);
    background: var(--c);
  }
  .row--partial .bar {
    opacity: 0.5;
  }
  .bar--estimated,
  .row--partial .bar--estimated {
    background: none;
    box-shadow: inset 0 0 0 1px var(--c);
    opacity: 1;
  }
  /* The target: a 1×12 tick in ink 2 across each bar, as TimeTrack's. */
  .target {
    position: absolute;
    top: -3px;
    width: 1px;
    height: 12px;
    margin-left: -0.5px;
    background: var(--ink-2);
  }

  @media (forced-colors: active) {
    .bar {
      forced-color-adjust: none;
    }
    .target {
      background: CanvasText;
    }
    .row:focus-visible {
      outline-color: Highlight;
    }
  }
</style>
