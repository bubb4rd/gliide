<!--
  The review's cover letter tab, in the column: Write cover letter (one model call from the tailored resume as the
  draft has it now and the posting), then each part of the letter as a row: ⏎ edits it in your words; a paragraph
  can also be rewritten with a note. The toolbar holds Rewrite letter and Export; the desk shows the page.
-->
<script lang="ts">
  import Pencil from "@lucide/svelte/icons/pencil";
  import PenLine from "@lucide/svelte/icons/pen-line";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import type { CoverPart } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import {
    Button,
    Empty,
    GroupHead,
    Input,
    KeyFooter,
    PartRow,
    Textarea,
    Thinking,
    type KeyCommand,
  } from "$lib/ui";
  import type { Session } from "./session.svelte";

  /** The tab's commands the review owns (write, export, back to the resume), listed after the letter's own. */
  let { s, commands }: { s: Session; commands: KeyCommand[] } = $props();

  interface Part {
    key: string;
    label: string;
    text: string;
    part: CoverPart;
  }

  /** The letter of the review on screen; another review's (just replaced) counts as still loading. */
  const view = $derived(s.cover && s.cover.review === s.review?.id ? s.cover : null);
  const letter = $derived(view?.letter ?? null);
  const parts = $derived.by((): Part[] => {
    if (!letter) return [];
    return [
      { key: "greeting", label: "Greeting", text: letter.greeting, part: { kind: "greeting" } },
      ...letter.paragraphs.map(
        (text, index): Part => ({
          key: `p${index}`,
          label: `Paragraph ${index + 1}`,
          text,
          part: { kind: "paragraph", index },
        }),
      ),
      { key: "closing", label: "Closing", text: letter.closing, part: { kind: "closing" } },
    ];
  });
  const writing = $derived(s.drafting === "letter");

  let selectedKey = $state<string | null>(null);
  const selectedIndex = $derived(Math.max(0, parts.findIndex((p) => p.key === selectedKey)));
  const selected = $derived(parts[selectedIndex]);
  let editing = $state<string | null>(null);
  let draft = $state("");
  let noting = $state<number | null>(null);
  let note = $state("");

  // A letter written again starts with nothing open.
  $effect(() => {
    if (writing) {
      editing = null;
      noting = null;
    }
  });

  const focus = (element: HTMLElement) => element.focus();

  function move(by: number) {
    const next = parts[Math.min(Math.max(selectedIndex + by, 0), parts.length - 1)];
    if (next) selectedKey = next.key;
  }

  function startEdit(part: Part | undefined) {
    if (!part || s.drafting !== null) return;
    selectedKey = part.key;
    noting = null;
    editing = part.key;
    draft = part.text;
  }

  function saveEdit(part: Part) {
    const text = draft.trim();
    editing = null;
    if (text && text !== part.text) void s.setCoverPart(part.part, text);
  }

  function startNote(part: Part, index: number) {
    selectedKey = part.key;
    editing = null;
    noting = index;
    note = "";
  }

  function sendNote(index: number) {
    noting = null;
    void s.rewriteParagraph(index, note.trim());
  }

  const KEYS: KeyCommand[] = $derived([
    { keys: ["↑", "↓"], label: "Select" },
    { keys: ["⏎"], label: "Edit", run: () => startEdit(selected), disabled: !selected || s.drafting !== null },
    {
      keys: ["N"],
      label: "Rewrite with a note",
      run: () => selected?.part.kind === "paragraph" && startNote(selected, selected.part.index),
      disabled: selected?.part.kind !== "paragraph" || s.drafting !== null,
    },
    ...commands,
  ]);

  const interactive = (target: EventTarget | null) =>
    target instanceof Element && target.closest("button, a, input, textarea, select") !== null;

  $effect(() =>
    bindKeys({
      ArrowDown: () => move(1),
      ArrowUp: () => move(-1),
      Enter: (e) => {
        if (interactive(e.target)) return false;
        startEdit(selected);
      },
      n: () => {
        if (selected?.part.kind !== "paragraph" || s.drafting !== null) return false;
        startNote(selected, selected.part.index);
      },
    }),
  );
</script>

<!-- The whole letter being written: its stage centered in the ring (`Review`); the one before shows on the desk. -->
{#if writing}
  <div class="center">
    <Thinking center label="Writing the cover letter…" startedAt={s.running?.startedAt} oncancel={() => s.cancel()} />
  </div>
{:else if !view}
  <div class="rows" aria-hidden="true">
    {#each [0, 1, 2] as i (i)}
      <div class="skeleton">
        <span class="bone bone--label"></span>
        <span class="bone"></span>
        <span class="bone bone--short"></span>
      </div>
    {/each}
  </div>
{:else if !letter}
  <Empty
    title="No cover letter yet"
    body="One page from this tailored resume, your toggles and edits included, and the posting. It claims nothing your resume doesn’t."
  >
    {#snippet action()}
      <Button variant="primary" size="lg" icon={PenLine} kbd="W" onclick={() => s.writeCover()}
        >Write cover letter</Button
      >
    {/snippet}
  </Empty>
{:else}
  <div class="letter">
    <div class="rows">
      <GroupHead
        label="Letter"
        meta="{letter.words} words · {letter.paragraphs.length} paragraph{letter.paragraphs.length === 1 ? '' : 's'}"
      />
      {#each parts as part (part.key)}
        {@const index = part.part.kind === "paragraph" ? part.part.index : null}
        {#snippet editor()}
          <Textarea
            size="lg"
            bind:value={draft}
            aria-label="Your wording"
            {@attach focus}
            onkeydown={(e) => {
              if (e.key === "Escape") {
                e.stopPropagation();
                editing = null;
              } else if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                saveEdit(part);
              }
            }}
          />
        {/snippet}
        <PartRow
          label={part.label}
          selected={part.key === selected?.key}
          busy={index !== null && s.drafting === index}
          onselect={() => (selectedKey = part.key)}
          editor={editing === part.key ? editor : undefined}
        >
          {#snippet text()}{part.text}{/snippet}
          {#snippet actions()}
            <Button
              variant="ghost"
              size="sm"
              icon={Pencil}
              kbd="⏎"
              disabled={s.drafting !== null}
              onclick={() => startEdit(part)}>Edit</Button
            >
            {#if index !== null}
              <Button
                variant="ghost"
                size="sm"
                icon={Sparkles}
                kbd="N"
                disabled={s.drafting !== null}
                onclick={() => startNote(part, index)}>Rewrite with a note</Button
              >
            {/if}
          {/snippet}
        </PartRow>
        {#if index !== null && noting === index}
          <form
            class="note"
            onsubmit={(e) => {
              e.preventDefault();
              sendNote(index);
            }}
          >
            <Input
              size="md"
              bind:value={note}
              placeholder="What should change? “Shorter”, “more about …” (optional)"
              aria-label="Note for the rewrite"
              {@attach focus}
              onkeydown={(e) => {
                if (e.key !== "Escape") return;
                e.stopPropagation();
                noting = null;
              }}
            />
            <Button type="submit" variant="primary" size="sm">Rewrite</Button>
          </form>
        {:else if index !== null && s.drafting === index && s.running}
          <div class="note">
            <Thinking
              label="Rewriting the paragraph…"
              startedAt={s.running.startedAt}
              oncancel={() => s.cancel()}
            />
          </div>
        {/if}
      {/each}
    </div>
    <div class="keys-wrap"><KeyFooter keys={KEYS} /></div>
  </div>
{/if}

<style>
  .rows {
    display: grid;
    margin-inline: calc(var(--space-2) * -1);
  }
  .note {
    display: flex;
    gap: var(--space-2);
    align-items: center;
    padding: 0 var(--space-3) var(--space-3);
  }
  .note > :global(.progress) {
    flex: 1;
  }
  /* The stage centered in the room the column has left (the column is a flex column while it's written). */
  .center {
    flex: 1 0 auto;
    display: grid;
    place-items: center;
    padding-block: var(--space-12);
  }
  .letter {
    display: contents;
  }
  .keys-wrap {
    position: sticky;
    bottom: 0;
    margin-inline: calc(var(--space-5) * -1);
  }

  /* Placeholders while the letter loads: static, no shimmer. */
  .skeleton {
    display: grid;
    gap: var(--space-2);
    padding: var(--space-3);
  }
  .bone {
    display: block;
    height: var(--space-3);
    border-radius: var(--radius-xs);
    background: var(--sunken);
  }
  .bone--label {
    width: 20%;
  }
  .bone--short {
    width: 60%;
  }
</style>
