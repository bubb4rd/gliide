<!--
  Experience (/bank): everything you've done. Your name, then each section in a card: an entry is its name and
  dates, its role, place and length, its first bullet, and which masters show it; a skills line, its items. Import
  picks a resume file (PDF, Word, Markdown, text, or ATS Fit's master.json) and opens its review (/bank/import). Each
  entry (not a skills line) opens its editor (/bank/entries/<id>).
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import FileUp from "@lucide/svelte/icons/file-up";
  import type { EntryRow } from "$lib/api";
  import { chooseResume } from "$lib/import";
  import { bindKeys } from "$lib/keys";
  import { app } from "$lib/stores/app.svelte";
  import { Badge, Button, Card, Empty, Toolbar } from "$lib/ui";

  let loaded = $state(false);
  /** Back from adding an import: the screen enters once (a result arriving), not on every visit. */
  const arrived = page.url.searchParams.has("added");

  onMount(async () => {
    await Promise.all([app.refreshLibrary(), app.settings ? null : app.loadSettings()]);
    loaded = true;
  });

  const lib = $derived(app.library);
  const bank = $derived(lib?.bank ?? null);
  const masterTitle = $derived(new Map(lib?.masters.map((m) => [m.id, m.title]) ?? []));

  /** The toolbar's and the empty state's import, and `I`. */
  const pick = () => chooseResume(app.settings?.exportDir);

  $effect(() => bindKeys({ i: () => void pick() }));

  function updated(rfc3339: string): string {
    const d = new Date(rfc3339);
    if (Number.isNaN(d.getTime())) return "";
    return d.toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });
  }

  const plural = (n: number, one: string, many: string) => `${n.toLocaleString()} ${n === 1 ? one : many}`;
  const sum = (rows: EntryRow[], count: (e: EntryRow) => number) => rows.reduce((n, e) => n + count(e), 0);
  const href = (e: EntryRow) => `/bank/entries/${encodeURIComponent(e.id)}`;

  const MONTH = /^(\d{4})-(0[1-9]|1[0-2])$/;
  /** `2026-06` → months since year 0; `present` → this month; `null` when unreadable. */
  function month(value: string, now: Date): number | null {
    if (value === "present") return now.getFullYear() * 12 + now.getMonth();
    const m = value.match(MONTH);
    return m ? Number(m[1]) * 12 + Number(m[2]) - 1 : null;
  }
  /** How long an entry lasts, both ends counted (June to September is 4 months): `4 mo`, `1 yr`, `3 yr 10 mo`; `""`
   *  when its dates couldn't be read. */
  function duration(e: EntryRow, now = new Date()): string {
    const s = e.start === null ? null : month(e.start, now);
    const end = e.end === null ? null : month(e.end, now);
    if (s === null || end === null) return "";
    const n = Math.max(1, end - s + 1);
    const years = Math.floor(n / 12);
    const rest = n % 12;
    return [years ? `${years} yr` : "", rest ? `${rest} mo` : ""].filter(Boolean).join(" ");
  }

  /** What an entry holds: "3 bullets · 2 variants". */
  const holds = (e: EntryRow) =>
    [plural(e.items, "bullet", "bullets"), e.variants ? plural(e.variants, "variant", "variants") : ""]
      .filter(Boolean)
      .join(" · ");
</script>

<!-- Which masters show it. With one master, only an entry it leaves out says so: a badge on every row would say nothing. -->
{#snippet masters(entry: EntryRow)}
  {#if !entry.masters.length}
    <span class="masters meta">In no master</span>
  {:else if masterTitle.size > 1}
    <span class="masters">
      {#each entry.masters as id (id)}<Badge tone="accent">{masterTitle.get(id) ?? id}</Badge>{/each}
    </span>
  {/if}
{/snippet}

<Toolbar>
  {#snippet crumbs()}<b>Experience</b>{/snippet}
  {#if bank}
    <Button icon={FileUp} kbd="I" onclick={pick}>Import resume</Button>
  {/if}
</Toolbar>

{#if bank && lib}
  <div class="scroll">
    <div class="column">
      <header class={["head", arrived && "enter"]} style:--i="0">
        <h1 class="headline">{bank.name}</h1>
        <p class="meta num">{[updated(bank.updatedAt) && `Updated ${updated(bank.updatedAt)}`, `rev ${bank.rev}`].filter(Boolean).join(" · ")}</p>
      </header>

      {#each lib.sections as section, s (section.id)}
        <section
          class={["section", arrived && "enter"]}
          style:--i={Math.min(s + 1, 6)}
          aria-labelledby="section-{section.id}"
        >
          <header class="section-head">
            <h2 class="heading" id="section-{section.id}">{section.title}</h2>
            <span class="meta num">
              {#if section.kind === "skills"}
                {plural(section.entries.length, "line", "lines")} · {plural(sum(section.entries, (e) => e.items), "item", "items")}
              {:else}
                {plural(section.entries.length, "entry", "entries")} · {plural(sum(section.entries, (e) => e.items), "bullet", "bullets")}
              {/if}
            </span>
          </header>
          <Card>
            <ul class="list">
              {#each section.entries as entry (entry.id)}
                <li>
                  {#if section.kind === "skills"}
                    <div class="line">
                      <span class="name">{entry.name}</span>
                      {@render masters(entry)}
                      <span class="items">
                        {#each entry.skills as skill, i (i)}<Badge>{skill}</Badge>{/each}
                      </span>
                    </div>
                  {:else}
                    {@const length = duration(entry)}
                    <a class="entry" href={href(entry)}>
                      <span class="name">{entry.name}</span>
                      {#if entry.dates}<span class="dates meta num">{entry.dates}</span>{/if}
                      {#if entry.subtitle || entry.place}
                        <span class="sub">{[entry.subtitle, entry.place].filter(Boolean).join(" · ")}</span>
                      {/if}
                      {#if length}<span class="length meta num">{length}</span>{/if}
                      {#if entry.lead}<span class="lead">{entry.lead}</span>{/if}
                      <span class="facts">
                        <span class="meta num">{holds(entry)}</span>
                        {#each entry.tags as tag (tag)}<Badge>{tag}</Badge>{/each}
                        {@render masters(entry)}
                      </span>
                    </a>
                  {/if}
                </li>
              {/each}
            </ul>
          </Card>
        </section>
      {/each}
    </div>
  </div>
{:else if loaded}
  <div class="center">
    <Empty
      title="No experience yet"
      body="Import your resume as a PDF, Word file or ATS Fit's master.json: its entries become your bank, and the resume your first master."
    >
      {#snippet action()}
        <Button variant="primary" size="lg" icon={FileUp} kbd="I" onclick={pick}>Import resume</Button>
      {/snippet}
    </Empty>
  </div>
{/if}

<style>
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    overscroll-behavior: contain;
  }
  .column {
    display: grid;
    gap: var(--space-6);
    max-width: 880px;
    margin-inline: auto;
    padding: var(--space-10) var(--space-6) var(--space-12);
  }
  .head {
    display: grid;
    gap: var(--space-1);
  }
  .head p {
    margin: 0;
  }
  .section {
    display: grid;
    gap: var(--space-2);
    min-width: 0;
  }
  .section-head {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
  }
  .heading {
    flex: 1;
    min-width: 0;
    margin: 0;
    font-size: var(--text-lg);
    line-height: var(--leading-lg);
    letter-spacing: var(--tracking-lg);
    font-weight: var(--weight-medium);
  }

  /* Inside the card's 1px border: 12 - 1, so a row's hover wash follows the corners. */
  .list {
    display: grid;
    margin: 0;
    padding: 0;
    overflow: hidden;
    list-style: none;
    border-radius: calc(var(--radius-lg) - 1px);
  }
  .list > li + li {
    box-shadow: inset 0 1px 0 var(--line);
  }

  /* An entry: its name over its role and place, its dates over its length, then its first bullet and what it holds.
     The whole row opens its editor. */
  .entry {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    grid-template-areas:
      "name dates"
      "sub length"
      "lead lead"
      "facts facts";
    column-gap: var(--space-4);
    align-items: baseline;
    padding: var(--space-4) var(--space-5);
    color: var(--ink);
    text-decoration: none;
    cursor: default;
    -webkit-user-drag: none;
    transition: background-color var(--dur-instant) var(--ease-out);
  }
  .entry:focus-visible {
    outline-offset: -2px;
  }
  .name {
    grid-area: name;
    min-width: 0;
    font-size: var(--text-md);
    line-height: var(--leading-md);
    letter-spacing: var(--tracking-md);
    font-weight: var(--weight-medium);
    text-wrap: balance;
  }
  .dates {
    grid-area: dates;
    white-space: nowrap;
    color: var(--ink-2);
    text-align: end;
  }
  .sub {
    grid-area: sub;
    min-width: 0;
    color: var(--ink-2);
    text-wrap: pretty;
  }
  .length {
    grid-area: length;
    white-space: nowrap;
    text-align: end;
  }
  /* The first bullet, as the page reads it, in two lines at most. */
  .lead {
    grid-area: lead;
    display: -webkit-box;
    margin-top: var(--space-2);
    overflow: hidden;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    font-size: var(--text-md);
    line-height: var(--leading-md);
    color: var(--ink-2);
    text-wrap: pretty;
  }
  .facts {
    grid-area: facts;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1-5);
    margin-top: var(--space-3);
  }
  .masters {
    display: inline-flex;
    flex-wrap: wrap;
    gap: var(--space-1);
    margin-inline-start: auto;
  }

  /* A skills line: its label and masters, then its items. */
  .line {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--space-2) var(--space-4);
    padding: var(--space-4) var(--space-5);
  }
  .line .name {
    grid-area: auto;
  }
  .items {
    grid-column: 1 / -1;
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
  }

  .center {
    display: grid;
    flex: 1;
    place-items: center;
    min-height: 0;
  }
  @media (hover: hover) {
    .entry:hover {
      background: var(--hover);
    }
  }
</style>
