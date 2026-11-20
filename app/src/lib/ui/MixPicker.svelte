<!--
  MixPicker: whole setups as named positions (Settings → Agents' Your mix): a `Segmented` grown into cards, side by
  side on a sunken track. Each card: its name (500), "Yours" (`Badge` accent) on the one the roles are set as; its
  model time a month ("43 min", 16/24 500, tabular, "of model time a month" in meta after it) over a 4px bar against the
  longest card's (ink 3 on the track; none without a total, which says why instead); Tailoring's quality, a `Meter`
  and "Tailoring quality 82" in meta; then its foot in meta: what choosing it changes ("27 min less, quality 6 lower"),
  "Where you are", or why it can't be chosen. A card that can be chosen is a toggle button (`aria-pressed`, the one on
  raised on the surface with `--shadow-border`); one that can't is `aria-disabled` with its reason as its foot; `here`
  is where you are when no named one is (Your own), raised, not a button. Hovering or focusing a choosable card calls
  `onpreview` with it (null on leaving); choosing calls `onchoose`. Washes only, ≤100ms.
-->
<script module lang="ts">
  export interface MixCard {
    key: string;
    name: string;
    /** Model time a month, in minutes; null without a total. */
    minutes: number | null;
    /** Why there's no total ("Writing isn’t timed yet"). */
    noTotal?: string;
    /** Tailoring's quality, 0–100; null when it wasn't judged. */
    quality: number | null;
    /** The roles are set as this one. */
    on?: boolean;
    /** Where you are, when no named card is: shown raised, not a choice. */
    here?: boolean;
    /** Why it can't be chosen. */
    disabled?: string | null;
    /** Under it: what choosing it changes, or where it stands. */
    foot?: string | null;
    /** The foot is a problem (a role that can't run). */
    footTone?: "warn" | null;
    /** In place of its time and quality, a sentence (Your own, while a named card is on). */
    note?: string;
  }
</script>

<script lang="ts">
  import Badge from "./Badge.svelte";
  import Meter from "./Meter.svelte";

  interface Props {
    cards: MixCard[];
    /** What they are, for assistive tech ("Your mix"). */
    label: string;
    onchoose?: (key: string) => void;
    onpreview?: (key: string | null) => void;
  }

  let { cards, label, onchoose, onpreview }: Props = $props();

  const id = $props.id();
  const scale = $derived(Math.max(1, ...cards.map((c) => c.minutes ?? 0)));
  const mins = (m: number) => `${Math.round(m)} min`;

  function choose(c: MixCard) {
    if (c.on || c.disabled) return;
    onchoose?.(c.key);
  }
</script>

<div class="mix" role="group" aria-label={label} style:--cols={cards.length}>
  {#each cards as c (c.key)}
    {#snippet body()}
      <span class="top">
        <span class="name">{c.name}</span>
        {#if c.on || c.here}<Badge tone="accent">Yours</Badge>{/if}
      </span>
      {#if c.note}
        <span class="meta none">{c.note}</span>
      {:else if c.minutes !== null}
        <span class="time"><span class="min num">{mins(c.minutes)}</span><span class="meta">of model time a month</span></span>
        <span class="bar" aria-hidden="true"><span style:--at={c.minutes / scale}></span></span>
      {:else}
        <span class="meta none">{c.noTotal ?? "No total yet"}</span>
      {/if}
      {#if !c.note}
        <span class="quality meta">
          {#if c.quality !== null}
            <Meter value={c.quality / 100} label="Tailoring quality" />
            <span>Tailoring quality <span class="num">{Math.round(c.quality)}</span></span>
          {:else}
            Tailoring quality not benchmarked
          {/if}
        </span>
      {/if}
      {#if c.foot}<span class={["foot meta", c.footTone === "warn" && "foot--warn"]} id="{id}-{c.key}-foot">{c.foot}</span
        >{/if}
    {/snippet}
    {#if c.here}
      <div class="card card--on" role="group" aria-label="{c.name}, where you are">{@render body()}</div>
    {:else}
      <button
        type="button"
        class={["card", c.on && "card--on"]}
        aria-pressed={!!c.on}
        aria-disabled={!!c.disabled}
        aria-describedby={c.foot ? `${id}-${c.key}-foot` : undefined}
        onclick={() => choose(c)}
        onpointerenter={() => !c.on && !c.disabled && onpreview?.(c.key)}
        onpointerleave={() => onpreview?.(null)}
        onfocus={() => !c.on && !c.disabled && onpreview?.(c.key)}
        onblur={() => onpreview?.(null)}>{@render body()}</button
      >
    {/if}
  {/each}
</div>

<style>
  /* 8 inner + 3 padding + 1 border = 12 outer (a card's --radius-lg). */
  .mix {
    display: grid;
    grid-template-columns: repeat(var(--cols), minmax(0, 1fr));
    gap: var(--space-0-5);
    padding: calc(var(--space-1) - 1px);
    border: 1px solid transparent;
    border-radius: var(--radius-lg);
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .card {
    display: grid;
    align-content: start;
    gap: var(--space-1);
    min-width: 0;
    padding: var(--space-3);
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--ink-2);
    font: inherit;
    text-align: start;
    cursor: default;
    -webkit-tap-highlight-color: transparent;
    transition-property: background-color, box-shadow;
    transition-duration: var(--dur-instant);
    transition-timing-function: var(--ease-out);
  }
  button.card:focus-visible {
    outline: none;
    box-shadow: var(--focus-ring);
  }
  .card--on {
    background: var(--surface);
    box-shadow: var(--shadow-border);
    color: var(--ink);
  }
  button.card--on:focus-visible {
    box-shadow: var(--shadow-border), var(--focus-ring);
  }
  @media (hover: hover) {
    button.card[aria-pressed="false"]:not([aria-disabled="true"]):hover {
      background: var(--hover);
    }
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
    min-height: var(--space-5);
  }
  .name {
    font-weight: var(--weight-medium);
    color: var(--ink);
  }
  .card[aria-disabled="true"] .name {
    color: var(--ink-2);
  }
  .time {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    column-gap: var(--space-1-5);
    margin-top: var(--space-1);
  }
  .min {
    font-size: var(--text-lg);
    line-height: var(--leading-lg);
    font-weight: var(--weight-medium);
    color: var(--ink);
  }
  .card[aria-disabled="true"] .min {
    color: var(--ink-2);
  }
  .none {
    margin-top: var(--space-1);
    color: var(--ink-3);
    text-wrap: pretty;
  }
  /* The time against the longest card's: a 4px bar, as TimeTrack's track. */
  .bar {
    position: relative;
    display: block;
    height: var(--space-1);
    margin-top: var(--space-0-5);
    border: 1px solid transparent;
    border-radius: var(--radius-full);
    background: var(--surface);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .card--on .bar {
    background: var(--sunken);
  }
  .bar > span {
    position: absolute;
    inset-block: -1px;
    inset-inline-start: -1px;
    width: calc(var(--at) * (100% + 2px));
    min-width: 2px;
    border-radius: var(--radius-full);
    background: var(--ink-3);
  }
  .quality {
    display: flex;
    align-items: center;
    gap: var(--space-1-5);
    margin-top: var(--space-1);
    color: var(--ink-3);
  }
  .foot {
    margin-top: var(--space-1);
    color: var(--ink-2);
    text-wrap: pretty;
  }
  .foot--warn {
    color: var(--warn);
  }

  @media (forced-colors: active) {
    .bar > span {
      forced-color-adjust: none;
      background: CanvasText;
    }
    .card--on {
      border-color: Highlight;
    }
  }
</style>
