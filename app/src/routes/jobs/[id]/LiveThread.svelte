<!--
  A live interview's thread (DESIGN.md, Job → Interview: live interview): the session as it goes, and its readback's
  Transcript. Each prep question as she asked it (the who-line names the question and its group), your answer as
  Practice's sunken bubble (how long you spoke under it), and the follow-ups it drew hanging off it on one 2px rule
  ("Follow-up 1 · on that answer"), each with the phrase of your answer it picked up quoted over it (↳ “…”). In your
  bubble that phrase is underlined (dotted); hovering either lights the other, and while you answer that follow-up the
  phrase holds the selection wash and its quote's arrow the accent. A turn passed over with Next question says "Skipped".
  `tail` goes at the thread's end (her turn being read, or the call that failed).
-->
<script lang="ts">
  import type { Snippet } from "svelte";
  import UserRound from "@lucide/svelte/icons/user-round";
  import CornerDownRight from "@lucide/svelte/icons/corner-down-right";
  import type { LiveTurn, PrepView } from "$lib/api";
  import { spokenLine } from "$lib/speech.svelte";
  import { Icon } from "$lib/ui";
  import { GROUPS } from "./Prep.svelte";
  import { currentTurn } from "./live";

  interface Props {
    turns: LiveTurn[];
    prep: PrepView;
    /** The prep's question count when the run started. */
    total: number;
    /** Who asks ("Priya, as Globex"). */
    asker: string;
    tail?: Snippet;
  }

  let { turns, prep, total, asker, tail }: Props = $props();

  const current = $derived(currentTurn(turns));
  /** Each question's turns in order: its ask, then its follow-ups. */
  const groups = $derived.by(() => {
    const out: { question: string; ask: LiveTurn | undefined; follows: LiveTurn[] }[] = [];
    for (const t of turns) {
      let g = out.at(-1);
      if (!g || g.question !== t.question || t.kind === "ask") {
        g = { question: t.question, ask: undefined, follows: [] };
        out.push(g);
      }
      if (t.kind === "ask") g.ask = t;
      else g.follows.push(t);
    }
    return out;
  });
  /** The follow-up whose phrase the pointer is on (its quote or its mark), by `seq`. */
  let lit = $state<number | null>(null);

  const question = (id: string) => prep.questions.find((q) => q.id === id);
  const number = (id: string) => prep.questions.findIndex((q) => q.id === id) + 1;
  const groupLabel = (id: string) => GROUPS.find((g) => g.value === question(id)?.group)?.label;

  /** The follow-up that picks up a phrase from this turn's answer: the next turn, on the same question. */
  const followOf = (t: LiveTurn) => {
    const next = turns.find((x) => x.seq === t.seq + 1);
    return next?.kind === "follow" && next.question === t.question && next.picked ? next : undefined;
  };

  /** The answer split round the phrase a follow-up picked (spaces and case as written may differ). */
  function split(text: string, picked: string): [string, string, string] | null {
    const words = picked.trim().split(/\s+/).filter(Boolean);
    if (!words.length) return null;
    const re = new RegExp(words.map((w) => w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("\\s+"), "i");
    const m = re.exec(text);
    return m ? [text.slice(0, m.index), m[0], text.slice(m.index + m[0].length)] : null;
  }

  const hover = (seq: number) => (e: PointerEvent) => e.pointerType === "mouse" && (lit = seq);
  const unhover = () => (lit = null);
</script>

{#snippet who(meta: string)}
  <p class="who">
    <Icon icon={UserRound} size={16} /><span class="who__name">{asker}</span><span class="meta">{meta}</span>
  </p>
{/snippet}

{#snippet said(turn: LiveTurn)}
  {#if turn.answer !== null}
    {@const follow = followOf(turn)}
    {@const parts = follow?.picked ? split(turn.answer, follow.picked) : null}
    <p class="bubble">{#if parts && follow}{parts[0]}<!-- svelte-ignore a11y_no_static_element_interactions --><mark
          class={["picked", follow === current && "picked--now", lit === follow.seq && "picked--lit"]}
          onpointerenter={hover(follow.seq)}
          onpointerleave={unhover}>{parts[1]}</mark
        >{parts[2]}{:else}{turn.answer}{/if}</p>
    {#if turn.spoken}<p class="meta said num">{spokenLine(turn.spoken)}</p>{/if}
  {:else if turn.skipped}
    <p class="meta skipped">Skipped</p>
  {/if}
{/snippet}

<div class="turns">
  {#each groups as g, gi (gi)}
    {#if g.ask}
      <article class="turn">
        {@render who(`Question ${number(g.question) || gi + 1} of ${total}${groupLabel(g.question) ? ` · ${groupLabel(g.question)}` : ""}`)}
        <p class="asked">{g.ask.asked}</p>
      </article>
    {/if}
    {#if g.follows.length}
      <div class="branch">
        {#if g.ask}{@render said(g.ask)}{/if}
        {#each g.follows as f, fi (f.seq)}
          <article class="turn">
            {@render who(`Follow-up ${fi + 1} · on that answer`)}
            {#if f.picked}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <p
                class={["on", f === current && "on--now", lit === f.seq && "on--lit"]}
                onpointerenter={hover(f.seq)}
                onpointerleave={unhover}
              >
                <Icon icon={CornerDownRight} size={14} /><span>“{f.picked}”</span>
              </p>
            {/if}
            <p class="asked">{f.asked}</p>
          </article>
          {@render said(f)}
        {/each}
      </div>
    {:else if g.ask}
      {@render said(g.ask)}
    {/if}
  {/each}
  {@render tail?.()}
</div>

<style>
  .turns,
  .branch {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
  }
  .turns > :global(*),
  .branch > :global(*) {
    flex: none;
  }
  /* A follow-up hangs off the answer it follows: one rule from that answer down through the follow-ups and your
     replies, so the chain reads as one and the next question starts back at the margin. */
  .branch {
    padding-inline-start: var(--space-4);
    box-shadow: inset 2px 0 0 var(--line-strong);
  }
  .turn p {
    margin: 0;
  }
  .who {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-bottom: var(--space-1-5);
  }
  .who :global(.icon) {
    flex: none;
    color: var(--ink-3);
  }
  .who__name {
    flex: none;
    font-weight: var(--weight-medium);
  }
  .who .meta {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .asked {
    font-size: var(--text-md);
    line-height: var(--leading-md);
    font-weight: var(--weight-medium);
    text-wrap: pretty;
  }
  /* The phrase she picked up, quoted over her follow-up: ↳ and the words, in meta ink 2. Hovering it lights the
     phrase in your answer (and the other way round); while it's the follow-up you answer, its arrow is the accent. */
  .turn .on {
    display: flex;
    align-items: flex-start;
    gap: var(--space-1-5);
    width: fit-content;
    max-width: 100%;
    margin: 0 0 var(--space-2);
    padding-inline: var(--space-0-5) var(--space-1);
    border-radius: var(--radius-xs);
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    color: var(--ink-2);
    transition: background-color var(--dur-instant) var(--ease-out);
  }
  .on :global(.icon) {
    flex: none;
    margin-top: 1px;
    color: var(--ink-3);
  }
  .on--now :global(.icon) {
    color: var(--accent-text);
  }
  .on--lit {
    background: var(--selected);
  }
  .bubble {
    align-self: flex-end;
    max-width: 88%;
    margin: 0;
    padding: var(--space-2-5) var(--space-3);
    border-radius: var(--radius-lg);
    border: 1px solid transparent;
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
    font-size: var(--text-md);
    line-height: var(--leading-md);
    white-space: pre-wrap;
    text-wrap: pretty;
  }
  .picked {
    color: inherit;
    background: none;
    text-decoration: underline dotted var(--ink-3);
    text-underline-offset: 3px;
    border-radius: 2px;
    transition: background-color var(--dur-instant) var(--ease-out);
  }
  .picked--now {
    background: var(--selected);
    text-decoration-color: var(--accent-text);
  }
  .picked--lit {
    background: var(--selected);
  }
  /* Said out loud: how long it took, under the bubble at its edge. */
  .said {
    align-self: flex-end;
    margin: calc(var(--space-3) * -1) 0 0;
    color: var(--ink-3);
  }
  .skipped {
    align-self: flex-end;
    margin: 0;
    color: var(--ink-3);
  }
</style>
