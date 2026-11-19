<!--
  StackRows, a ranked list of stacked bars: a row each (32px), its maker's 16px mark and name (truncating) in a 200px
  column, then a 10px bar on a shared 0–`max` scale made of its parts end to end, each shaded as `StackedBar`'s (the
  first the accent, then the lighter steps; `segmentStep`) with a 2px slit between, any part above 0 at least 2px; a
  part that stands in for a missing measure is outlined in its step, unfilled. Then the value (tabular, 500 ink on the
  leader) and an aside in meta (what stands in: "needs Coaching"), which moves into the row's tooltip when the list is
  narrower than 560px. Partial rows follow under a "Partial" label, their names in ink 3. Hovering a part opens its own
  tooltip; the row's opens on keyboard focus. One tab stop: ↑↓ Home End move between rows. No motion.
-->
<script module lang="ts">
  import type { TipContent } from "./tip.svelte";

  export interface StackPart {
    label: string;
    value: number;
    /** Stands in for a measure the row hasn't got: outlined, unfilled. */
    placeholder?: boolean;
    tip?: TipContent;
  }

  export interface StackRow {
    key: string;
    icon: string | null;
    name: string;
    /** Up to four, in order. */
    parts: StackPart[];
    /** The value as shown ("78.1"). */
    text: string;
    /** Something in it stands in: listed under Partial. */
    partial?: boolean;
    /** In meta after the value ("needs Coaching"). */
    aside?: string;
    tip: TipContent;
  }
</script>

<script lang="ts">
  import CompanyMark from "./CompanyMark.svelte";
  import { segmentStep } from "./StackedBar.svelte";
  import { rove } from "./rove";
  import { tip } from "./tip.svelte";

  interface Props {
    rows: StackRow[];
    /** Where a bar is full. */
    max?: number;
    /** What it ranks, for assistive tech. */
    label: string;
  }

  let { rows, max = 100, label }: Props = $props();

  const groups = $derived(
    [
      { id: "complete", label: "", rows: rows.filter((r) => !r.partial) },
      { id: "partial", label: "Partial", rows: rows.filter((r) => r.partial) },
    ].filter((g) => g.rows.length),
  );
  const order = $derived(groups.flatMap((g) => g.rows));
  const leader = $derived(rows.find((r) => !r.partial)?.key ?? null);

  /** Each part's start and width, in % of the track. */
  function laid(row: StackRow) {
    let at = 0;
    return row.parts.slice(0, 4).map((p, i) => {
      const w = max > 0 ? (Math.max(0, p.value) / max) * 100 : 0;
      const out = { ...p, step: segmentStep(i), left: at, width: w };
      at += w;
      return out;
    });
  }

  let width = $state(0);
  const narrow = $derived(width > 0 && width < 560);

  let active = $state<number | null>(null);
  const current = $derived(order.length ? Math.min(active ?? 0, order.length - 1) : -1);
  let list: HTMLElement;

  function onkeydown(e: KeyboardEvent) {
    const next = rove(e, current, order.length);
    if (next === null) return;
    active = next;
    list.querySelectorAll<HTMLElement>(".row")[next]?.focus();
  }

  function rowTip(row: StackRow): TipContent {
    if (!narrow || !row.aside) return row.tip;
    if (typeof row.tip === "string") return { title: row.tip, note: row.aside };
    return { ...row.tip, note: [row.tip.note, row.aside].filter(Boolean).join(" · ") };
  }
</script>

<!-- One tab stop; arrows, Home and End move between rows. -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class={["stack-rows", narrow && "stack-rows--narrow"]}
  role="group"
  aria-label={label}
  bind:this={list}
  bind:clientWidth={width}
  {onkeydown}
>
  {#each groups as g (g.id)}
    {#if g.label}<p class="label group">{g.label}</p>{/if}
    {#each g.rows as row (row.key)}
      {@const i = order.indexOf(row)}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div
        class={["row", row.partial && "row--partial", row.key === leader && "row--lead"]}
        role="img"
        aria-label="{row.name}, {row.text}: {row.parts
          .map((p) => `${p.label} ${p.value.toFixed(1)}${p.placeholder ? ' (stand-in)' : ''}`)
          .join(', ')}"
        tabindex={i === current ? 0 : -1}
        onfocus={() => (active = i)}
        {@attach tip(() => rowTip(row), "beside")}
      >
        <span class="who">
          <CompanyMark size={16} src={row.icon} />
          <span class="name">{row.name}</span>
        </span>
        <span class="track">
          {#each laid(row) as p (p.step)}
            {#if p.value > 0}
              <span
                class={["part", p.placeholder && "part--stand-in"]}
                style:--c="var(--accent-step-{p.step})"
                style:left="{p.left}%"
                style:width="max(2px, calc({p.width}% - 2px))"
                {@attach tip(() => p.tip ?? null)}
              ></span>
            {/if}
          {/each}
        </span>
        <span class="value num">{row.text}</span>
        {#if !narrow}<span class="aside meta">{row.aside ?? ""}</span>{/if}
      </div>
    {/each}
  {/each}
</div>

<style>
  .stack-rows {
    --name: 200px;
    --value: 40px;
    --aside: 136px;
    display: grid;
    min-width: 0;
  }
  .group {
    margin: var(--space-3) 0 var(--space-1);
    color: var(--ink-3);
  }
  .row {
    display: grid;
    grid-template-columns: var(--name) minmax(0, 1fr) var(--value) var(--aside);
    align-items: center;
    column-gap: var(--space-3);
    height: var(--space-8);
    border-radius: var(--radius-xs);
    outline-offset: 2px;
  }
  .stack-rows--narrow .row {
    --name: 160px;
    grid-template-columns: var(--name) minmax(0, 1fr) var(--value);
  }
  .row:focus-visible {
    outline: 2px solid var(--accent);
  }
  .who {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }
  .name {
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .row--partial .name {
    color: var(--ink-3);
  }
  .track {
    position: relative;
    height: 10px;
  }
  .part {
    position: absolute;
    top: 0;
    bottom: 0;
    border-radius: calc(var(--radius-xs) / 2);
    background: var(--c);
  }
  .part--stand-in {
    background: none;
    box-shadow: inset 0 0 0 1px var(--c);
  }
  .value {
    text-align: end;
    color: var(--ink-2);
  }
  .row--lead .value {
    color: var(--ink);
    font-weight: var(--weight-medium);
  }
  .aside {
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  @media (forced-colors: active) {
    .part {
      forced-color-adjust: none;
    }
    .row:focus-visible {
      outline-color: Highlight;
    }
  }
</style>
