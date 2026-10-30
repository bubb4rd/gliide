<!--
  Experience (/bank): everything you've done. Your name, then each section in a card: an entry is its name and
  dates, its role, place and length, its first bullet, and which masters show it; a skills line, its items. Then your
  stories: each its title, the entry it happened in and its themes, how long it takes to tell and when you last
  practiced it, under the themes no story covers. Import picks a resume file (PDF, Word, Markdown, text, or a gliide
  v1 JSON) and opens its review (/bank/import). Each entry (not a skills line) opens its editor
  (/bank/entries/<id>), each story its own (/bank/stories/<id>); New story `N` opens a blank one. A section of
  entries adds one from its heading: named in place (`⏎`), it joins the bank in no master and its editor opens.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import FileUp from "@lucide/svelte/icons/file-up";
  import Plus from "@lucide/svelte/icons/plus";
  import {
    addEntry,
    appError,
    stories as loadStories,
    type EntryRow,
    type SectionRow,
    type StoriesView,
    type StoryView,
  } from "$lib/api";
  import { chooseResume } from "$lib/import";
  import { bindKeys } from "$lib/keys";
  import { app } from "$lib/stores/app.svelte";
  import { themeLabel, tellTime } from "$lib/stories";
  import { Badge, Button, Card, Empty, Input, Toolbar, toast } from "$lib/ui";

  let loaded = $state(false);
  let stories = $state<StoriesView | null>(null);
  /** Back from adding an import: the screen enters once (a result arriving), not on every visit. */
  const arrived = page.url.searchParams.has("added");

  onMount(async () => {
    const told = loadStories().then(
      (v) => (stories = v),
      (e) => toast(`Couldn’t load your stories: ${appError(e).message}`, { tone: "danger" }),
    );
    await Promise.all([app.refreshLibrary(), app.settings ? null : app.loadSettings(), told]);
    loaded = true;
  });

  const lib = $derived(app.library);
  const bank = $derived(lib?.bank ?? null);
  const masterTitle = $derived(new Map(lib?.masters.map((m) => [m.id, m.title]) ?? []));

  /** The toolbar's and the empty state's import, and `I`. */
  const pick = () => chooseResume(app.settings?.exportDir);
  const newStory = () => goto("/bank/stories/new");

  $effect(() => bindKeys({ i: () => void pick(), n: () => void newStory() }));

  // ── A new entry, named in place at the end of its section ──
  /** What adding to a section of each kind says, and what its name asks for (an entry's name is where or what). */
  const ADDS: Record<string, { action: string; placeholder: string }> = {
    experience: { action: "Add a role", placeholder: "Where, like Acme Widgets Co." },
    projects: { action: "Add a project", placeholder: "Its name, like Tidepool" },
    education: { action: "Add a school", placeholder: "Its name, like State University" },
    certifications: { action: "Add a certification", placeholder: "Its name, like AWS Certified Developer" },
    custom: { action: "Add an entry", placeholder: "Its name" },
  };
  /** The section a new entry is being named in. */
  let adding = $state<string | null>(null);
  let name = $state("");
  let creating = $state(false);
  const bankAware = $derived(app.settings?.suggestionsFrom !== "master-only");

  function startAdd(section: SectionRow) {
    adding = section.id;
    name = "";
  }

  /** Adds the named entry and opens its editor; with no name, the field closes. */
  async function create() {
    const section = adding;
    const text = name.trim();
    if (!section || creating) return;
    if (!text) return void (adding = null);
    creating = true;
    try {
      const id = await addEntry(section, text);
      await goto(`/bank/entries/${encodeURIComponent(id)}`);
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
    } finally {
      creating = false;
    }
  }

  function onNameKey(e: KeyboardEvent) {
    if (e.isComposing) return;
    if (e.key === "Escape") {
      e.stopPropagation();
      adding = null;
    } else if (e.key === "Enter") {
      e.preventDefault();
      void create();
    }
  }

  const focus = (element: HTMLElement) => element.focus();

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

  /** "5 stories · No story for conflict or failure": the themes no story covers, three named at most. */
  function storiesMeta(v: StoriesView): string {
    const names = v.uncovered.map((t) => themeLabel(t).toLowerCase());
    const shown = names.length > 3 ? [...names.slice(0, 3), `${names.length - 3} more`] : names;
    const last = shown.pop();
    const gap = last ? `No story for ${shown.length ? `${shown.join(", ")} ${names.length > 3 ? "and" : "or"} ` : ""}${last}` : "";
    return [plural(v.stories.length, "story", "stories"), gap].filter(Boolean).join(" · ");
  }

  /** "Practiced Oct 30" from its newest use, else "Not practiced". */
  function practicedOn(story: StoryView): string {
    const newest = story.used.reduce<string | null>((at, u) => (at && at > u.at ? at : u.at), null);
    const d = newest ? new Date(newest) : null;
    if (!d || Number.isNaN(d.getTime())) return "Not practiced";
    return `Practiced ${d.toLocaleDateString(undefined, { month: "short", day: "numeric" })}`;
  }
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
    <Button icon={Plus} kbd="N" onclick={newStory}>New story</Button>
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
            {#if ADDS[section.kind]}
              <Button variant="ghost" size="sm" icon={Plus} onclick={() => startAdd(section)}
                >{ADDS[section.kind].action}</Button
              >
            {/if}
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
              {#if adding === section.id && ADDS[section.kind]}
                <li class="adding">
                  <Input
                    size="md"
                    bind:value={name}
                    placeholder={ADDS[section.kind].placeholder}
                    aria-label="{ADDS[section.kind].action}: its name"
                    disabled={creating}
                    {@attach focus}
                    onkeydown={onNameKey}
                    onblur={() => {
                      if (!name.trim() && !creating) adding = null;
                    }}
                  />
                  <p class="meta hint">
                    {#if bankAware}
                      In no master: a review brings it in where it fits a posting better than what the master shows.
                    {:else}
                      In no master: suggestions come from the master only (Settings, Tailoring).
                    {/if}
                  </p>
                </li>
              {/if}
            </ul>
          </Card>
        </section>
      {/each}

      {#if stories}
        <section
          class={["section", arrived && "enter"]}
          style:--i={Math.min(lib.sections.length + 1, 6)}
          aria-labelledby="section-stories"
        >
          <header class="section-head">
            <h2 class="heading" id="section-stories">Stories</h2>
            {#if stories.stories.length}<span class="meta num">{storiesMeta(stories)}</span>{/if}
          </header>
          {#if stories.stories.length}
            <Card>
              <ul class="list">
                {#each stories.stories as story (story.id)}
                  <li>
                    <a class="story" href="/bank/stories/{encodeURIComponent(story.id)}">
                      <span class="name">{story.title || "Untitled story"}</span>
                      <span class="tell meta num">{tellTime(story.seconds)} to tell</span>
                      <span class="facts">
                        <span class={["where", !story.entryName && "where--off"]}
                          >{story.entryName ?? (story.entry ? "Entry removed" : "Off the resume")}</span
                        >
                        {#each story.themes as theme (theme)}<Badge>{themeLabel(theme)}</Badge>{/each}
                      </span>
                      <span class="when meta num">{practicedOn(story)}</span>
                    </a>
                  </li>
                {/each}
              </ul>
            </Card>
          {:else}
            <p class="meta none">
              No stories yet. Dig into an entry, save an answer from practice as one, or start one with New story.
            </p>
          {/if}
        </section>
      {/if}
    </div>
  </div>
{:else if loaded}
  <div class="center">
    <Empty
      title="No experience yet"
      body="Import your resume as a PDF, Word file or gliide v1 JSON: its entries become your bank, and the resume your first master."
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
  .entry:focus-visible,
  .story:focus-visible {
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

  /* A story: its title over where it happened and its themes; how long it takes to tell over when you last did. */
  .story {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    grid-template-areas:
      "name tell"
      "facts when";
    column-gap: var(--space-4);
    row-gap: var(--space-1-5);
    align-items: center;
    padding: var(--space-3) var(--space-5);
    color: var(--ink);
    text-decoration: none;
    cursor: default;
    -webkit-user-drag: none;
    transition: background-color var(--dur-instant) var(--ease-out);
  }
  .story .name {
    text-wrap: pretty;
  }
  .tell {
    grid-area: tell;
    white-space: nowrap;
    text-align: end;
  }
  .story .facts {
    margin-top: 0;
  }
  .where {
    color: var(--ink-2);
  }
  .where--off {
    color: var(--ink-3);
  }
  .when {
    grid-area: when;
    white-space: nowrap;
    text-align: end;
  }
  .none {
    margin: 0;
    text-wrap: pretty;
  }
  .items {
    grid-column: 1 / -1;
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
  }
  /* Naming a new entry: the row it will be, its field over what happens to it. */
  .adding {
    padding: var(--space-3) var(--space-5);
  }
  .hint {
    margin: var(--space-1-5) 0 0;
    text-wrap: pretty;
  }

  .center {
    display: grid;
    flex: 1;
    place-items: center;
    min-height: 0;
  }
  @media (hover: hover) {
    .entry:hover,
    .story:hover {
      background: var(--hover);
    }
  }
</style>
