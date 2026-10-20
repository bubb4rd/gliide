<!--
  One edit the agent proposed in a chat turn, as a diff card read live from the review. The head: its number, where
  it sits (entry › bullet), its points, a triangle whose tooltip says anything to check, then its state. Pending: the
  line it replaces (−, struck) over the line as it will read (+, keywords marked), why, then Accept, Reject, Tweak
  (edit the line in place) and "Waiting on you". Accepted folds to the head (Diff, Take back) over the new line;
  rejected to the head alone (Restore); gone (undone) is the head, dimmed. The card takes focus: A X T ⏎ on a pending
  one, U on an accepted one, ↑↓ to the next card, Esc back to the composer; the buttons show their keys while it's
  focused or hovered.
-->
<script lang="ts">
  import Check from "@lucide/svelte/icons/check";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Info from "@lucide/svelte/icons/info";
  import Pencil from "@lucide/svelte/icons/pencil";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import Undo2 from "@lucide/svelte/icons/undo-2";
  import X from "@lucide/svelte/icons/x";
  import type { ReviewView } from "$lib/api";
  import { Button, Icon, Marked, Textarea, tip } from "$lib/ui";
  import type { Place } from "./place";
  import { signed } from "./place";
  import { find, proposalState, type Session } from "./session.svelte";
  import type { TurnProposal } from "./thread";

  interface Props {
    s: Session;
    review: ReviewView;
    p: TurnProposal;
    /** Its number in the thread, from 1. */
    n: number;
    place: Place;
    /** Commands can run (the review isn't being replaced). */
    ready: boolean;
    /** ↑↓ on the card: focus the card before (-1) or after (1) it. */
    onmove: (from: HTMLElement, by: 1 | -1) => void;
    /** Esc: back to the composer. */
    onescape: () => void;
  }

  let { s, review, p, n, place, ready, onmove, onescape }: Props = $props();

  const standing = $derived(proposalState(review, p));
  const change = $derived(find(review, p.key));
  const text = $derived(change?.text ?? (p.edit.kind === "skill" ? p.edit.item : p.edit.text));
  const was = $derived(p.was ?? change?.was ?? null);
  const notes = $derived([
    ...(change?.state === "left-out" && change.reason ? [`Left out: ${change.reason}`] : []),
    ...(change?.warning ? [change.warning] : []),
    ...p.flags,
  ]);
  const points = $derived(change?.points == null ? null : Math.round(change.points));

  let editing = $state(false);
  let draft = $state("");
  let diff = $state(false);
  /** Focused or hovered: the buttons show their keys. */
  let focused = $state(false);
  let hovered = $state(false);
  const lit = $derived(focused || hovered);
  /**
   * The card itself has focus from the keyboard (↑↓, Tab, Esc from the field): its ring shows. A click focuses it too
   * (WebKit focuses the card, not the button pressed), and that never rings it: the last input was the pointer.
   */
  let ringed = $state(false);
  let keyed = false;
  let card = $state<HTMLElement>();

  const STATES = { pending: "waiting on you", accepted: "accepted", rejected: "rejected", gone: "undone" } as const;

  function startTweak() {
    if (!change?.editable || !ready) return;
    draft = change.text;
    editing = true;
  }

  function stopTweak() {
    editing = false;
    card?.focus();
  }

  function save() {
    const next = draft.trim();
    stopTweak();
    if (next && next !== change?.text) void s.word(p.key, next);
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.isComposing || e.metaKey || e.ctrlKey || e.altKey) return;
    // Tab moves focus as usual (the screen would take it for the letter tab); Esc from anywhere in the card.
    if (e.key === "Tab") return e.stopPropagation();
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      return onescape();
    }
    // The rest act on the card itself, not on a button inside it.
    if (e.target !== e.currentTarget) return;
    const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
    const pending = standing === "pending" && ready;
    const handlers: Record<string, (() => unknown) | false> = {
      ArrowDown: () => onmove(card!, 1),
      ArrowUp: () => onmove(card!, -1),
      a: pending && (() => s.accept([p.key])),
      x: pending && (() => s.reject([p.key])),
      t: pending && !!change?.editable && startTweak,
      Enter: pending && !!change?.editable && startTweak,
      u: standing === "accepted" && ready && (() => s.takeBack(p)),
    };
    const handle = handlers[key];
    if (!handle) return;
    e.preventDefault();
    e.stopPropagation();
    void handle();
  }
</script>

<svelte:window onkeydowncapture={() => (keyed = true)} onpointerdowncapture={() => (keyed = false)} />

<!-- A focusable group: its keys act on the edit (listed in Commands), its buttons do the same with a click. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<section
  bind:this={card}
  class={["card", `card--${standing}`, ringed && "card--ring"]}
  data-card
  tabindex="0"
  aria-label="Edit {n}, {place.main}{place.sub ? `, ${place.sub}` : ''}, {STATES[standing]}"
  {onkeydown}
  onfocusin={(e) => {
    focused = true;
    ringed = e.target === card && keyed;
  }}
  onfocusout={(e) => {
    focused = card?.contains(e.relatedTarget as Node) ?? false;
    ringed = false;
  }}
  onpointerenter={(e) => e.pointerType === "mouse" && (hovered = true)}
  onpointerleave={() => (hovered = false)}
>
  <header class="head">
    <span class="n num" aria-hidden="true">{n}</span>
    <span class="place">
      <span class="place__main">{place.main}</span>
      {#if place.sub}<Icon icon={ChevronRight} size={12} /><span class="place__sub">{place.sub}</span>{/if}
    </span>
    {#if points !== null && standing !== "gone"}
      <span class={["pts", "num", (points <= 0 || standing === "rejected") && "pts--quiet"]}>{signed(points)}</span>
    {/if}
    {#if notes.length && standing === "pending"}
      <span class="warning" {@attach tip(notes.length === 1 ? notes[0] : { rows: notes.map((label) => ({ label })) })}
        ><Icon icon={TriangleAlert} size={14} weight="medium" /><span class="visually-hidden">{notes.join(". ")}</span
        ></span
      >
    {/if}
    {#if standing === "accepted"}
      <span class="state state--ok"><Icon icon={Check} size={14} weight="medium" />Accepted</span>
      {#if was}
        <Button variant="ghost" size="sm" aria-expanded={diff} onclick={() => (diff = !diff)}
          >{diff ? "Hide diff" : "Diff"}</Button
        >
      {/if}
      <Button
        variant="ghost"
        size="sm"
        icon={Undo2}
        kbd={lit ? "U" : undefined}
        disabled={!ready}
        onclick={() => s.takeBack(p)}>Take back</Button
      >
    {:else if standing === "rejected"}
      <span class="state">Rejected</span>
      <Button variant="ghost" size="sm" icon={RotateCcw} disabled={!ready} onclick={() => s.restore(p)}
        >Restore</Button
      >
    {:else if standing === "gone"}
      <span class="state">Undone</span>
    {/if}
  </header>

  {#if standing === "pending" || standing === "accepted"}
    <div class="body">
      {#if was && (standing === "pending" || diff) && !editing}
        <p class="line line--old"><span class="sign" aria-hidden="true">−</span><span class="text">{was}</span></p>
      {/if}
      <div class={["line", "line--new", standing === "accepted" && !diff && "line--clamp"]}>
        <span class="sign" aria-hidden="true">+</span>
        {#if editing}
          <Textarea
            size="lg"
            bind:value={draft}
            aria-label="Your wording for edit {n}"
            {@attach (field: HTMLTextAreaElement) => field.focus()}
            onkeydown={(e) => {
              e.stopPropagation();
              if (e.key === "Escape") stopTweak();
              else if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
                e.preventDefault();
                save();
              }
            }}
          />
        {:else}
          <span class="text"><Marked {text} marks={change?.marks ?? []} /></span>
        {/if}
      </div>
      {#if standing === "pending"}
        {#if p.why}
          <p class="why meta"><Icon icon={Info} size={14} /><span>{p.why}</span></p>
        {/if}
      {/if}
    </div>
  {/if}

  {#if standing === "pending"}
    <footer class="foot">
      {#if editing}
        <Button size="sm" icon={Check} kbd="⏎" onclick={save}>Save</Button>
        <Button variant="ghost" size="sm" kbd="Esc" onclick={stopTweak}>Cancel</Button>
      {:else}
        <Button size="sm" icon={Check} kbd={lit ? "A" : undefined} disabled={!ready} onclick={() => s.accept([p.key])}
          >Accept</Button
        >
        <Button
          variant="ghost"
          size="sm"
          icon={X}
          kbd={lit ? "X" : undefined}
          disabled={!ready}
          onclick={() => s.reject([p.key])}>Reject</Button
        >
        {#if change?.editable}
          <Button variant="ghost" size="sm" icon={Pencil} kbd={lit ? "T" : undefined} disabled={!ready} onclick={startTweak}
            >Tweak</Button
          >
        {/if}
      {/if}
      <span class="meta waiting">Waiting on you</span>
    </footer>
  {/if}
</section>

<style>
  /* As a Card (surface, ring and lift), its focus ring the app's. */
  .card {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    min-width: 0;
    border-radius: var(--radius-lg);
    border: 1px solid transparent;
    background: var(--surface);
    box-shadow: var(--shadow-border);
    outline: none;
  }
  .card--ring {
    box-shadow: var(--shadow-border), var(--focus-ring);
  }

  /* The head: one 36px line. */
  .head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: 36px;
    padding: var(--space-1) var(--space-2) var(--space-1) var(--space-3);
  }
  .n {
    display: inline-grid;
    flex: none;
    place-items: center;
    min-width: 18px;
    height: 18px;
    padding-inline: var(--space-1);
    border-radius: var(--radius-xs);
    border: 1px solid transparent;
    font: var(--weight-semibold) var(--text-xs) / 1 var(--font-ui);
    color: var(--on-accent);
    background: var(--accent);
  }
  .card--accepted .n {
    border-color: var(--line-strong);
    color: var(--ink-2);
    background: var(--sunken);
  }
  .card--rejected .n,
  .card--gone .n {
    border-color: var(--line-strong);
    color: var(--ink-3);
    background: transparent;
  }
  .place {
    display: flex;
    flex: 1;
    align-items: center;
    gap: var(--space-1);
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    font-weight: var(--weight-medium);
    color: var(--ink-2);
  }
  .place__main {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .place :global(.icon) {
    flex: none;
    color: var(--ink-4);
  }
  .place__sub {
    flex: none;
    font-weight: var(--weight-regular);
    color: var(--ink-3);
  }
  .card--rejected .place,
  .card--gone .place {
    color: var(--ink-3);
  }
  .pts {
    flex: none;
    font-size: var(--text-sm);
    font-weight: var(--weight-medium);
    color: var(--gain-text);
  }
  .pts--quiet {
    color: var(--ink-3);
  }
  .state {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: var(--space-1);
    padding-inline-end: var(--space-1);
    font-size: var(--text-sm);
    font-weight: var(--weight-medium);
    color: var(--ink-3);
  }
  .state--ok {
    color: var(--gain-text);
  }

  /* The lines: a sign in the gutter, the text at the reading size. */
  .body {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--space-1-5);
    padding: 0 var(--space-3) var(--space-3);
  }
  .line {
    display: grid;
    grid-template-columns: 14px minmax(0, 1fr);
    gap: var(--space-1-5);
    margin: 0;
    font-size: var(--text-md);
    line-height: var(--leading-md);
    letter-spacing: var(--tracking-md);
    text-wrap: pretty;
    overflow-wrap: anywhere;
    color: var(--ink);
  }
  .sign {
    font-weight: var(--weight-medium);
    text-align: center;
    color: var(--ink-4);
  }
  .card--accepted .line--new .sign {
    color: var(--gain-text);
  }
  .line--old {
    color: var(--ink-3);
  }
  .line--old .text {
    text-decoration-line: line-through;
    text-decoration-color: var(--ink-4);
  }
  .line--clamp .text {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .why {
    display: flex;
    align-items: flex-start;
    gap: var(--space-1-5);
    margin: 0;
    padding-top: var(--space-0-5);
    text-wrap: pretty;
  }
  .why :global(.icon) {
    flex: none;
    margin-top: 1px;
    color: var(--ink-4);
  }
  /* A warning is its triangle alone; what to check is in its tooltip (and read out). */
  .warning {
    display: inline-flex;
    flex: none;
    color: var(--warn);
  }

  /* Accept, Reject, Tweak at the start; "Waiting on you" at the end. */
  .foot {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1);
    padding: var(--space-2) var(--space-2) var(--space-2) var(--space-3);
    border-top: 1px solid var(--line);
  }
  .waiting {
    margin-inline-start: auto;
    padding-inline-end: var(--space-1);
  }
</style>
