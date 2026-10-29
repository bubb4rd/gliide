<!--
  Nudges for the question being practiced (DESIGN.md, Job → Interview → Practice): up to three small suggestions, each
  a direction or a question that jogs your memory, never the answer. One may carry what you could say, with `…` where
  only you know the words (Add to answer puts it at the end of your draft), and the lines of the resume it points to.
-->
<script lang="ts">
  import Lightbulb from "@lucide/svelte/icons/lightbulb";
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  import type { Nudge } from "$lib/api";
  import { Button, Icon, tip } from "$lib/ui";

  interface Props {
    nudges: Nudge[];
    /** A line a nudge points to, by id: its text and the entry it's under. */
    line: (id: string) => { text: string; entry: string | null } | undefined;
    onadd: (example: string) => void;
    ondismiss: () => void;
  }

  let { nudges, line, onadd, ondismiss }: Props = $props();

  const clip = (text: string) => (text.length > 44 ? `${text.slice(0, 43).trimEnd()}…` : text);
</script>

<article class="card" aria-label="Nudges for your answer">
  <header class="head">
    <Icon icon={Lightbulb} size={16} />
    <span class="head__title">Nudges</span>
    <span class="meta head__note">From your words and what you sent</span>
    <Button
      variant="ghost"
      size="sm"
      square
      icon={X}
      aria-label="Dismiss the nudges"
      {@attach tip("Dismiss")}
      onclick={ondismiss}
    />
  </header>
  {#each nudges as n, i (i)}
    <div class="nudge">
      <p class="nudge__text">{n.text}</p>
      {#if n.example}<blockquote class="nudge__example">{n.example}</blockquote>{/if}
      {#if n.example || n.lines.length}
        <p class="acts">
          {#if n.example}
            {@const example = n.example}
            <Button variant="ghost" size="sm" icon={Plus} onclick={() => onadd(example)}>Add to answer</Button>
          {/if}
          {#each n.lines as id (id)}
            {@const at = line(id)}
            {#if at}
              <span class="chip" {@attach tip(at.entry ? `${at.entry}: ${at.text}` : at.text)}>{clip(at.text)}</span>
            {/if}
          {/each}
        </p>
      {/if}
    </div>
  {/each}
</article>

<style>
  .card {
    display: grid;
    border-radius: var(--radius-lg);
    border: 1px solid transparent;
    background: var(--surface);
    box-shadow: var(--shadow-border);
    overflow: hidden;
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-1-5) var(--space-1-5) var(--space-1-5) var(--space-3);
  }
  .head :global(.icon) {
    flex: none;
    color: var(--ink-3);
  }
  .head__title {
    font-weight: var(--weight-medium);
  }
  .head__note {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .nudge {
    display: grid;
    gap: var(--space-1-5);
    padding: var(--space-2-5) var(--space-3);
    box-shadow: inset 0 1px 0 var(--line);
  }
  .nudge p {
    margin: 0;
  }
  .nudge__text {
    font-size: var(--text-md);
    line-height: var(--leading-md);
    text-wrap: pretty;
  }
  .nudge__example {
    margin: 0;
    padding-inline-start: var(--space-3);
    box-shadow: inset 2px 0 0 var(--line-strong);
    font-size: var(--text-md);
    line-height: var(--leading-md);
    color: var(--ink-2);
    text-wrap: pretty;
  }
  .acts {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1-5) var(--space-2);
  }
  .chip {
    display: inline-flex;
    align-items: center;
    height: 22px;
    padding-inline: var(--space-2);
    border-radius: var(--radius-full);
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
    font-size: var(--text-sm);
    color: var(--ink-2);
  }
</style>
