<!--
  ReleaseNotes: a release's notes as a changelog. The version (16/24 500) and the day it was published (meta, ink 3)
  over its changes as a dotted list: the notes' lines when there are several (a leading -, * or • dropped), else their
  sentences. A change that opens with a short lead before a colon ("See what your model calls cost: Settings → Usage
  shows…") is a title (500, ink) over the rest (ink 2); any other is a line in ink 2. Without a version, the list
  alone (Settings → About, under where updates stand). `limit` folds it to its first changes (the rest hidden, for a
  toggle elsewhere that names the list by `id`: `WhatsNew`).
-->
<script module lang="ts">
  interface ReleaseChange {
    /** The short lead before a colon, as the change's title. */
    lead: string | null;
    text: string;
  }

  /** A lead longer than this is a sentence with a colon in it, not a title. */
  const LEAD_MAX = 48;

  /** The changes in a release's notes: each line when there are several, else each sentence. */
  export function releaseChanges(notes: string): ReleaseChange[] {
    const lines = notes
      .split(/\n+/)
      .map((l) => l.replace(/^\s*(?:[-*•]|\d+[.)])\s+/, "").trim())
      .filter(Boolean);
    // No lookbehind: WebKit before Safari 16.4 rejects it, and with it the whole chunk this sits in.
    const sentences = (l: string) => l.replace(/([.!?])\s+(?=[A-Z0-9“"(])/g, "$1\n").split("\n");
    const items = lines.length > 1 ? lines : lines.flatMap(sentences);
    return items.map((item) => {
      const at = item.indexOf(": ");
      if (at <= 0 || at > LEAD_MAX) return { lead: null, text: item };
      const rest = item.slice(at + 2);
      return { lead: item.slice(0, at), text: rest.charAt(0).toUpperCase() + rest.slice(1) };
    });
  }
</script>

<script lang="ts">
  interface Props {
    notes: string;
    /** The release's version; without it, the list alone. */
    version?: string | null;
    /** When it was published (RFC 3339). */
    date?: string | null;
    /** Show only the first this many changes; the rest stay in the list, hidden. */
    limit?: number | null;
    /** The list's id, for the toggle that unfolds it (`aria-controls`). */
    id?: string;
  }

  let { notes, version = null, date = null, limit = null, id }: Props = $props();

  const changes = $derived(releaseChanges(notes));
  const day = $derived(
    date ? new Date(date).toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" }) : null,
  );
</script>

<div class="release">
  {#if version}
    <header class="head">
      <h3 class="version num">gliide {version}</h3>
      {#if day && date}<time class="meta num" datetime={date}>{day}</time>{/if}
    </header>
  {/if}
  <ul class="changes" {id}>
    {#each changes as change, i (i)}
      <li hidden={limit !== null && i >= limit}>
        {#if change.lead}<span class="lead">{change.lead}</span>{/if}
        <span class="text">{change.text}</span>
      </li>
    {/each}
  </ul>
</div>

<style>
  .release {
    display: grid;
    gap: var(--space-3);
  }
  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-3);
  }
  .version {
    margin: 0;
    font-size: var(--text-lg);
    line-height: var(--leading-lg);
    letter-spacing: var(--tracking-lg);
    font-weight: var(--weight-medium);
  }
  time {
    flex: none;
    color: var(--ink-3);
  }
  .changes {
    display: grid;
    gap: var(--space-2-5);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  /* A 4px dot in ink 4, centered on the first line. */
  li {
    position: relative;
    display: grid;
    gap: var(--space-0-5);
    padding-inline-start: var(--space-4);
    font-size: var(--text-base);
    line-height: var(--leading-base);
    text-wrap: pretty;
  }
  li[hidden] {
    display: none;
  }
  li::before {
    content: "";
    position: absolute;
    inset-block-start: calc((var(--leading-base) - var(--space-1)) / 2);
    inset-inline-start: var(--space-1);
    width: var(--space-1);
    height: var(--space-1);
    border-radius: var(--radius-full);
    background: var(--ink-4);
  }
  .lead {
    color: var(--ink);
    font-weight: var(--weight-medium);
  }
  .text {
    color: var(--ink-2);
  }
  @media (forced-colors: active) {
    li::before {
      background: CanvasText;
    }
  }
</style>
