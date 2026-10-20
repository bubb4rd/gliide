<!--
  The chat's composer: one box holding the field, where the parts the ask is limited to sit in its words as chips
  ("work web performance into @elovatesr.com"), growing from 1 to 6 lines, and at its foot a toolbar: `@`, `"` and
  `/` (the same menus as typing them), the model you're talking to, then Commands ⌘K and Send (Stop while the agent
  works). `@` opens a menu of the page's parts (the summary, every entry and skills line) to point at; `"` opens the
  page's lines (bullets and the summary) matched against the words you type after it, quoted or paraphrased; the one
  you pick takes the place of what you typed for it, as a chip. `/` at the start opens quick asks that fill the field
  with a sentence you can edit. ⏎ sends, ⇧⏎ breaks the line, ↑ in an empty field goes to the newest card, ↓ to the
  thread's foot. While the agent works, a Thinking line sits over the box with Stop (Esc). With nothing asked yet, two
  or three asks built from the posting's gaps sit over the box and send at once.
-->
<script lang="ts">
  import type { Component } from "svelte";
  import type { LucideProps } from "@lucide/svelte";
  import AlignLeft from "@lucide/svelte/icons/align-left";
  import ArrowUp from "@lucide/svelte/icons/arrow-up";
  import AtSign from "@lucide/svelte/icons/at-sign";
  import Briefcase from "@lucide/svelte/icons/briefcase";
  import Quote from "@lucide/svelte/icons/quote";
  import Scissors from "@lucide/svelte/icons/scissors";
  import Square from "@lucide/svelte/icons/square";
  import SquareSlash from "@lucide/svelte/icons/square-slash";
  import Tag from "@lucide/svelte/icons/tag";
  import Wrench from "@lucide/svelte/icons/wrench";
  import type { EditTargets, ReviewView } from "$lib/api";
  import { app } from "$lib/stores/app.svelte";
  import { Button, CompanyMark, Icon, Kbd, KeyFooter, type KeyCommand, Thinking, tip } from "$lib/ui";
  import MentionField from "./MentionField.svelte";
  import { placeOf } from "./place";
  import { match } from "./quote";
  import type { Session } from "./session.svelte";
  import { token, trimmed, type Draft, type Mention } from "./thread";

  interface Props {
    s: Session;
    review: ReviewView;
    /** Commands can run (the review isn't being replaced). */
    ready: boolean;
    /** The parts you can point at; null while they load. */
    targets: EditTargets | null;
    /** The Commands menu's list. */
    keys: KeyCommand[];
    /** Called as an ask goes out, so the thread follows it. */
    onsend: () => void;
    /** ↑ in an empty field: focus the newest card; false when there's none. */
    onlast: () => boolean;
    /** ↓ in an empty field: bring the thread's foot into view; false when it's already there. */
    onlatest: () => boolean;
  }

  let { s, review, ready, targets, keys, onsend, onlast, onlatest }: Props = $props();

  const id = $props.id();
  const SUMMARY: Mention = { id: "summary", label: "Summary" };

  let text = $state("");
  let scope = $state<Mention[]>([]);
  let field = $state<MentionField>();
  let list = $state<HTMLElement>();

  const busy = $derived(s.instructing !== null);
  const canSend = $derived(ready && !busy && text.trim() !== "");
  const hasSummary = $derived(
    review.lines.some((l) => l.line === null) || review.groups.some((g) => g.changes.some((c) => c.op === "summary")),
  );

  /** Puts the caret in the field (Esc from a card, a picked option). */
  export function focus() {
    field?.focus();
  }

  /** The model an ask goes to, by name; the agent's problem, if any, in its tooltip (its state is the sidebar's). */
  const model = $derived(app.modelName(app.settings?.suggestModel ?? review.model));

  // ── The menu: `@` parts, `"` lines, or `/` quick asks ──
  interface Option {
    key: string;
    label: string;
    meta: string;
    icon: Component<LucideProps>;
    /** A part or line of the page (its label is the long part), not a quick ask. */
    part: boolean;
    pick: () => void;
  }
  const COMMANDS = $derived([
    { name: "tighten", icon: Scissors, ask: "Tighten these lines: shorter, same facts, the posting's words" },
    { name: "keywords", icon: Tag, ask: "Work in the posting's missing keywords where my experience backs them" },
    {
      name: "summary",
      icon: AlignLeft,
      ask: hasSummary ? "Rewrite my summary for this posting" : "Write a summary for this posting",
      // Pointing at the summary it rewrites.
      draft: hasSummary
        ? {
            text: `Rewrite ${token(SUMMARY)} for this posting`,
            scope: [{ ...SUMMARY, at: "Rewrite ".length }],
          }
        : undefined,
    },
  ]);

  /** What's typed since the `@` or `"` before the caret, and where that character is. */
  let trigger = $state<{ kind: "@" | '"'; at: number; query: string } | null>(null);
  /** The `/command` filling the whole field. */
  let command = $state<string | null>(null);
  /** The `@` (or `/`, at 0) the menu was dismissed for with Esc: it stays shut until you type another. */
  let dismissed = $state<number | null>(null);
  let active = $state(0);

  const parts = $derived<Mention[]>([
    ...(hasSummary ? [SUMMARY] : []),
    ...(targets?.entries ?? []),
    ...(targets?.groups ?? []),
  ]);
  const kinds = $derived(
    new Map<string, { label: string; icon: Component<LucideProps> }>([
      ...(targets?.entries.map((t) => [t.id, { label: "Entry", icon: Briefcase }] as const) ?? []),
      ...(targets?.groups.map((t) => [t.id, { label: "Skills line", icon: Wrench }] as const) ?? []),
      [SUMMARY.id, { label: "Summary", icon: AlignLeft }],
    ]),
  );

  /**
   * The lines you can quote, as the page reads them now: each bullet and the summary, once (an edit that's on reads
   * as its new wording, one that's off as the line it would replace). Note lines can't be asked about alone.
   */
  const lines = $derived.by(() => {
    const quotable = (id: string | null): id is string => id === SUMMARY.id || /^.+\.\d+$/.test(id ?? "");
    const byId = new Map<string, { id: string; text: string; place: string; on: boolean }>();
    for (const g of review.groups)
      for (const c of g.changes) {
        const on = c.state === "in" || c.state === "forced";
        const text = on ? c.text : c.was;
        if (!c.rewritable || !quotable(c.target) || text === null || byId.get(c.target)?.on) continue;
        byId.set(c.target, { id: c.target, text, place: placeOf(c.target, g.label, targets).main, on });
      }
    for (const l of review.lines) {
      const lineId = l.line ?? SUMMARY.id;
      if (!l.rewritable || !quotable(lineId) || byId.has(lineId)) continue;
      byId.set(lineId, { id: lineId, text: l.text, place: placeOf(lineId, l.place, targets).main, on: false });
    }
    return [...byId.values()];
  });

  const options = $derived.by<Option[]>(() => {
    const free = (id: string) => !scope.some((c) => c.id === id);
    if (trigger?.kind === "@") {
      const q = trigger.query.toLowerCase();
      return parts
        .filter((m) => free(m.id) && m.label.toLowerCase().includes(q))
        .map((m) => ({
          key: m.id,
          label: m.label,
          // The summary's name says its kind.
          meta: m.id === SUMMARY.id ? "" : (kinds.get(m.id)?.label ?? ""),
          icon: kinds.get(m.id)?.icon ?? Briefcase,
          part: true,
          pick: () => void pick(m),
        }));
    }
    if (trigger?.kind === '"') {
      return match(
        trigger.query,
        lines.filter((l) => free(l.id)),
        (l) => l.text,
      ).map((l) => ({
        key: l.id,
        label: l.text,
        meta: l.place,
        icon: Quote,
        part: true,
        pick: () => void pick({ id: l.id, label: l.text, quote: true }),
      }));
    }
    if (command !== null) {
      const q = command.toLowerCase();
      return COMMANDS.filter((c) => c.name.startsWith(q)).map((c) => ({
        key: c.name,
        label: `/${c.name}`,
        meta: c.ask,
        icon: c.icon,
        part: false,
        pick: () => void pickCommand(c.draft ?? { text: c.ask, scope: [] }),
      }));
    }
    return [];
  });
  let focused = $state(false);
  const menuOpen = $derived(focused && (trigger !== null || command !== null));
  const MENUS = { "@": "Parts of your resume", '"': "Lines on the page" } as const;
  const menuTitle = $derived(trigger ? MENUS[trigger.kind] : "Quick asks");

  /** Reads the words before the caret: an `@query` or `"words` just before it, or a `/command` filling the field. */
  function read() {
    const before = field?.before() ?? null;
    if (before === null) {
      trigger = null;
      command = null;
      return;
    }
    const at = /(?:^|\s)@([^\s@]*)$/.exec(before);
    // A quote runs to the caret, spaces and all, until it's closed.
    const opened = /(?:^|\s)["“]([^"“”\n]*)$/.exec(before);
    const slash = !scope.length && before === text ? /^\/(\S*)$/.exec(text) : null;
    const found = [
      at && { kind: "@" as const, at: before.length - at[1].length - 1, query: at[1] },
      opened && { kind: '"' as const, at: before.length - opened[1].length - 1, query: opened[1] },
    ].filter((t) => t !== null);
    // Whichever opened last: an `@` inside a quote is a part, a quote after an `@word` a quote.
    const next = found.sort((a, b) => b.at - a.at)[0] ?? null;
    const nextCommand = slash ? slash[1] : null;
    const opening = next?.at ?? (nextCommand !== null ? 0 : null);
    if (opening === null) dismissed = null;
    if (opening !== null && opening === dismissed) {
      trigger = null;
      command = null;
      return;
    }
    // A new or narrowed list starts on its first option.
    if (next?.at !== trigger?.at || next?.query !== trigger?.query || nextCommand !== command) active = 0;
    trigger = next;
    command = next ? null : nextCommand;
  }

  function close() {
    dismissed = trigger?.at ?? (command !== null ? 0 : null);
    trigger = null;
    command = null;
  }

  /** The toolbar's `@`, `"` and `/`: types the character at the caret (`/` only into an empty field), opening its menu. */
  function insert(char: "@" | '"' | "/") {
    if (!field || (char === "/" && (text !== "" || scope.length))) return;
    field.focus();
    const before = field.before() ?? "";
    const lead = char !== "/" && before !== "" && !/\s$/.test(before) ? " " : "";
    dismissed = null;
    field.put(lead + char);
  }

  /** Puts `m` in the words as a chip, in place of the `@query` or `"words` that found it. */
  function pick(m: Mention) {
    const from = trigger;
    if (!from || !field) return;
    trigger = null;
    field.put(m, 1 + from.query.length);
  }

  function pickCommand(draft: Draft) {
    command = null;
    field?.set(draft);
    field?.focus(true);
  }

  // Keeps the active option in view as ↑↓ move it.
  $effect(() => {
    void active;
    list?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: "nearest" });
  });

  function onkeydown(e: KeyboardEvent) {
    if (e.isComposing) return;
    if (menuOpen) {
      const count = options.length;
      // A quote nothing matches is just a quote: ⏎ sends it.
      if (!count && trigger?.kind === '"' && e.key === "Enter") {
        close();
      } else {
        const handlers: Record<string, () => void> = {
          ArrowDown: () => count && (active = (active + 1) % count),
          ArrowUp: () => count && (active = (active - 1 + count) % count),
          // With nothing to pick, ⏎ doesn't send the half-typed `@`: Esc first.
          Enter: () => options[active]?.pick(),
          Escape: close,
          ...(count ? { Tab: () => options[active]?.pick() } : {}),
        };
        const handle = handlers[e.key];
        if (!handle || (e.key === "Enter" && e.shiftKey)) return;
        e.preventDefault();
        e.stopPropagation();
        handle();
        return;
      }
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void sendBox();
    } else if (e.key === "Escape") {
      e.stopPropagation();
      if (busy) s.cancel();
      else if (e.currentTarget instanceof HTMLElement) e.currentTarget.blur();
    } else if (e.key === "ArrowUp" && text === "" && !scope.length && !e.shiftKey && onlast()) {
      e.preventDefault();
    } else if (e.key === "ArrowDown" && text === "" && !scope.length && !e.shiftKey && onlatest()) {
      e.preventDefault();
    }
  }

  /**
   * Typing anywhere in the chat goes to the box, as in any chat: a printable key outside a field, menu or card
   * focuses it first (capture, before the screen's key bindings, which let letters through in the chat). A focused
   * card keeps its letters (A, X, T, U).
   */
  function claim(e: KeyboardEvent) {
    if (e.defaultPrevented || e.isComposing || e.metaKey || e.ctrlKey || e.altKey) return;
    if (e.key.length !== 1 || e.key === " " || !ready || !field) return;
    const at = e.target;
    if (
      at instanceof HTMLElement &&
      (at.isContentEditable || at.closest("input, textarea, select, [role=menu], [data-card]"))
    )
      return;
    field.focus();
  }

  /** Sends `ask` within `parts` (the thread follows it); resolves false when it couldn't start. */
  async function send(ask: string, parts: Mention[]): Promise<boolean> {
    const draft = trimmed({ text: ask, scope: parts });
    if (!draft.text || !ready || busy) return false;
    onsend();
    return (await s.instruct(draft.text, draft.scope)) !== null;
  }

  /** Sends what's in the field; it clears at once and comes back if the ask couldn't start. */
  async function sendBox() {
    if (!canSend) return;
    const kept = { text, scope };
    field?.clear();
    dismissed = null;
    if (!(await send(kept.text, kept.scope)) && !text && !scope.length) field?.set(kept);
  }

  // ── Asks to start from, while nothing has been asked ──
  const quote = (term: string) => `“${term}”`;
  const starters = $derived.by(() => {
    const terms = [
      ...review.skills
        .filter((c) => c.state === "missing")
        .sort((a, b) => a.priority - b.priority)
        .map((c) => c.term),
      ...review.gaps.map((g) => g.skill),
    ];
    const unique = [...new Map(terms.map((t) => [t.toLowerCase(), t])).values()];
    const asks = unique.slice(0, 2).map((t) => `Work in ${quote(t)} where it’s true`);
    if (asks.length < 3) asks.push("Tighten my bullets: shorter, same facts");
    if (asks.length < 3 && hasSummary) asks.push("Rewrite my summary for this posting");
    return asks.slice(0, 3);
  });
</script>

<svelte:window onkeydowncapture={claim} />

<div class="composer">
  {#if busy}
    <div class="thinking">
      <CompanyMark size={16} src="/agents/claude.png" />
      <div class="thinking__stage">
        <Thinking
          label="Working on your request…"
          startedAt={s.running?.stage === "instructing" ? s.running.startedAt : undefined}
        />
      </div>
      <Button variant="ghost" size="sm" kbd="Esc" onclick={() => s.cancel()}>Stop</Button>
    </div>
  {:else if !s.turns.length && starters.length}
    <div class="starters" role="group" aria-label="Asks to start with">
      {#each starters as ask (ask)}
        <Button size="sm" disabled={!ready} onclick={() => send(ask, [])}>{ask}</Button>
      {/each}
    </div>
  {/if}

  <div class="box">
    <MentionField
      bind:this={field}
      class="composer__field"
      label="Ask {model} for a change"
      placeholder={'Ask for a change, @ a part, " a line, / for quick asks'}
      aria-controls={menuOpen ? `${id}-menu` : undefined}
      aria-activedescendant={menuOpen && options[active] ? `${id}-option-${active}` : undefined}
      disabled={!ready}
      {onkeydown}
      oninput={(draft) => {
        ({ text, scope } = draft);
        read();
      }}
      onclick={read}
      onkeyup={(e: KeyboardEvent) => {
        if (e.key === "ArrowLeft" || e.key === "ArrowRight" || e.key === "Home" || e.key === "End") read();
      }}
      onfocus={() => {
        focused = true;
        read();
      }}
      onblur={() => (focused = false)}
    />

    <div class="tools">
      <Button
        variant="ghost"
        size="sm"
        square
        icon={AtSign}
        aria-label="Point at a part"
        {@attach tip("Point at a part (@)")}
        disabled={!ready}
        onmousedown={(e: MouseEvent) => e.preventDefault()}
        onclick={() => insert("@")}
      />
      <Button
        variant="ghost"
        size="sm"
        square
        icon={Quote}
        aria-label="Quote a line"
        {@attach tip('Quote a line (")')}
        disabled={!ready}
        onmousedown={(e: MouseEvent) => e.preventDefault()}
        onclick={() => insert('"')}
      />
      <Button
        variant="ghost"
        size="sm"
        square
        icon={SquareSlash}
        aria-label="Quick asks"
        {@attach tip(text ? "Quick asks (/), from an empty field" : "Quick asks (/)")}
        disabled={!ready || text !== ""}
        onmousedown={(e: MouseEvent) => e.preventDefault()}
        onclick={() => insert("/")}
      />
      <span class="agent" {@attach tip(app.agent?.problem)}>
        <CompanyMark size={16} src="/agents/claude.png" />
        <span class="agent__name">{model}</span>
      </span>
      <span class="tools__end">
        <KeyFooter bare {keys} />
        {#if busy}
          <Button
            size="sm"
            square
            icon={Square}
            aria-label="Stop"
            {@attach tip("Stop (Esc)")}
            onclick={() => s.cancel()}
          />
        {:else}
          <Button
            variant="primary"
            size="sm"
            square
            icon={ArrowUp}
            aria-label="Send"
            {@attach tip("Send (⏎)")}
            disabled={!canSend}
            onclick={sendBox}
          />
        {/if}
      </span>
    </div>
    {#if menuOpen}
      <!--
        A menu over the box, held by its bottom edge so it stays on the box as the list narrows. Mousedown keeps the
        focus in the field, so the menu stays open and the caret where it was; the field handles its keys.
      -->
      <div
        id="{id}-menu"
        class="menu"
        role="listbox"
        tabindex="-1"
        aria-label={menuTitle}
        onmousedown={(e) => e.preventDefault()}
      >
        <p class="label menu__title">{menuTitle}</p>
        <div class="menu__list" bind:this={list}>
          {#each options as option, i (option.key)}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <div
              id="{id}-option-{i}"
              class={["option", i === active && "active", option.part && "option--part"]}
              {@attach tip(option.part ? option.label : option.meta)}
              role="option"
              tabindex="-1"
              aria-selected={i === active}
              onpointermove={() => (active = i)}
              onclick={() => option.pick()}
            >
              <Icon icon={option.icon} weight="medium" />
              <span class="option__label">{option.label}</span>
              <span class="option__meta">{option.meta}</span>
              <span class="option__key">{#if i === active}<Kbd>⏎</Kbd>{/if}</span>
            </div>
          {:else}
            <p class="meta none">
              {#if trigger?.kind === "@" && !targets}Loading the page’s parts…{:else if trigger?.kind === "@"}No part
                matches “{trigger.query}”{:else if trigger}No line reads like “{trigger.query}”{:else}No quick ask
                starts with “/{command}”{/if}
            </p>
          {/each}
        </div>
        <p class="meta menu__foot"><span>↑↓ to choose</span><span>⏎ to insert</span><span>Esc to close</span></p>
      </div>
    {/if}
  </div>

</div>

<style>
  .composer {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--space-2);
    padding: var(--space-3) var(--space-4);
    background: var(--paper);
    box-shadow: inset 0 1px 0 var(--line);
  }

  /* The agent at work, docked over the box: its icon, the stage and its seconds, Stop. */
  .thinking {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: var(--control-md);
    padding-inline: var(--space-2) var(--space-1);
  }
  .thinking__stage {
    flex: 1;
    min-width: 0;
  }
  .starters {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1-5);
  }
  /* A long term never widens the column: the ask clips at the column's edge. */
  .starters :global(.btn) {
    min-width: 0;
    max-width: 100%;
    overflow: hidden;
  }

  /* One box: the field and the toolbar inside an input's edge, with its focus ring. */
  .box {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    border-radius: var(--radius-lg);
    border: 1px solid var(--input-border);
    background: var(--surface);
    transition-property: border-color, box-shadow;
    transition-duration: var(--dur-fast);
    transition-timing-function: var(--ease-out);
  }
  .box:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px oklch(from var(--accent) l c h / 0.18);
  }
  /* The field is the box's inside: no edge or ring of its own. 1 line to 6, then it scrolls. */
  .box :global(.composer__field) {
    max-height: calc(6 * var(--leading-md) + var(--space-2-5) + var(--space-1));
    padding: var(--space-2-5) var(--space-3) var(--space-1);
    overflow-y: auto;
    overscroll-behavior: contain;
  }
  .tools {
    display: flex;
    align-items: center;
    gap: var(--space-0-5);
    padding: var(--space-1) var(--space-1-5) var(--space-1-5);
  }
  .agent {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1-5);
    min-width: 0;
    margin-inline-start: var(--space-1-5);
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    white-space: nowrap;
  }
  .agent__name {
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: var(--weight-medium);
    color: var(--ink-2);
  }
  .tools__end {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-1);
    margin-inline-start: auto;
  }

  /*
   * As a popover (surface, ring and lift, opacity in over 100ms, closing at once), held 4px over the box by its
   * bottom edge, as wide as it. 6 inner + 5 padding + 1 border = 12 outer (--radius-lg).
   */
  .menu {
    position: absolute;
    inset-inline: -1px;
    bottom: calc(100% + var(--space-1));
    z-index: 3;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    padding: calc(var(--space-1-5) - 1px);
    border-radius: var(--radius-lg);
    border: 1px solid transparent;
    background: var(--surface);
    box-shadow: var(--shadow-popover);
    outline: none;
    transition: opacity var(--dur-instant) var(--ease-out);
  }
  @starting-style {
    .menu {
      opacity: 0;
    }
  }
  .menu__title {
    padding: var(--space-1-5) var(--space-2) var(--space-1);
  }
  .menu__list {
    display: grid;
    gap: 1px;
    max-height: 256px;
    overflow-y: auto;
    overscroll-behavior: contain;
  }
  .option {
    display: grid;
    /* A quick ask's name in a column of its own, so the sentences start on one line. */
    grid-template-columns: 16px 88px minmax(0, 1fr) 20px;
    align-items: center;
    gap: var(--space-2) var(--space-3);
    height: 32px;
    padding-inline: var(--space-2);
    border-radius: var(--radius-sm);
    color: var(--ink);
    cursor: default;
    user-select: none;
  }
  .option :global(.icon) {
    color: var(--ink-3);
  }
  .option.active {
    background: var(--selected);
  }
  .option.active :global(.icon) {
    color: var(--accent-text);
  }
  .option__label,
  .option__meta {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .option__label {
    font-weight: var(--weight-medium);
  }
  .option__meta {
    color: var(--ink-3);
  }
  /* A part's label takes the room (its kind stays whole, at the end); a quick ask's sentence gives way to its name. */
  .option--part {
    grid-template-columns: 16px minmax(0, 1fr) max-content 20px;
  }
  .option__key {
    display: flex;
    justify-content: flex-end;
  }
  .none {
    margin: 0;
    padding: var(--space-1-5) var(--space-2);
  }
  .menu__foot {
    display: flex;
    gap: var(--space-3);
    margin: var(--space-1) 0 0;
    padding: var(--space-1-5) var(--space-2) var(--space-1);
    border-top: 1px solid var(--line);
  }
</style>
