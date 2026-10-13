<!--
  Experience bank: everything you've done, by section, with which masters show each entry. Import picks a resume file
  (PDF, Word, Markdown, text, or ATS Fit's master.json) and opens its review (/bank/import). Each entry (not a skills
  line) opens its editor (/bank/entries/<id>): its bullets and their variants.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import FileUp from "@lucide/svelte/icons/file-up";
  import { chooseResume } from "$lib/import";
  import { bindKeys } from "$lib/keys";
  import { app } from "$lib/stores/app.svelte";
  import { Badge, Button, Empty, GroupHead, Toolbar } from "$lib/ui";

  let loaded = $state(false);
  /** Back from adding an import: the list enters once (a result arriving), not on every visit. */
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

  const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
</script>

<Toolbar>
  {#snippet crumbs()}<b>Experience bank</b>{/snippet}
  {#if bank}
    <Button icon={FileUp} kbd="I" onclick={pick}>Import resume</Button>
  {/if}
</Toolbar>

{#if bank && lib}
  <div class="scroll">
    <div class="column">
      <header class={["head", arrived && "enter"]} style:--i="0">
        <h1 class="title">{bank.name}</h1>
        <p class="meta num">
          {plural(bank.entries, "entry", "entries")} · {plural(bank.bullets, "bullet", "bullets")}
          {#if updated(bank.updatedAt)}· updated {updated(bank.updatedAt)}{/if} · rev {bank.rev}
        </p>
      </header>

      {#each lib.sections as section, s (section.id)}
        <section
          class={["section", arrived && "enter"]}
          style:--i={Math.min(s + 1, 6)}
          aria-label={section.title}
        >
          <GroupHead
            label={section.title}
            meta={section.kind === "skills"
              ? plural(section.entries.length, "line", "lines")
              : plural(section.entries.length, "entry", "entries")}
          />
          <ul class="entries">
            {#each section.entries as entry (entry.id)}
              {#snippet body()}
                <div class="line">
                  <span class="name">{entry.name}</span>
                  {#if entry.subtitle}<span class="subtitle">{entry.subtitle}</span>{/if}
                  {#if entry.dates}<span class="dates meta num">{entry.dates}</span>{/if}
                </div>
                <div class="facts">
                  <span class="meta num">
                    {section.kind === "skills"
                      ? plural(entry.items, "item", "items")
                      : plural(entry.items, "bullet", "bullets")}
                  </span>
                  {#each entry.tags as tag (tag)}<Badge>{tag}</Badge>{/each}
                  {#if entry.masters.length}
                    <span class="masters">
                      {#each entry.masters as id (id)}<Badge tone="accent">{masterTitle.get(id) ?? id}</Badge>{/each}
                    </span>
                  {:else}
                    <span class="meta">In no master</span>
                  {/if}
                </div>
              {/snippet}
              <li>
                {#if section.kind === "skills"}
                  <div class="entry">{@render body()}</div>
                {:else}
                  <a class="entry entry--link" href="/bank/entries/{encodeURIComponent(entry.id)}">{@render body()}</a>
                {/if}
              </li>
            {/each}
          </ul>
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
    padding: var(--space-8) var(--space-6) var(--space-12);
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
  }
  .section :global(.group-head) {
    padding-top: 0;
  }
  .entries {
    display: grid;
    margin: 0;
    padding: 0;
    list-style: none;
    box-shadow: inset 0 1px 0 var(--line);
  }
  .entry {
    display: grid;
    gap: var(--space-1);
    padding: var(--space-3);
    box-shadow: inset 0 -1px 0 var(--line);
  }
  /* An entry that opens its editor: the whole row is the link. */
  .entry--link {
    color: inherit;
    text-decoration: none;
    cursor: default;
    transition: background-color var(--dur-instant) var(--ease-out);
  }
  .entry--link:focus-visible {
    outline-offset: -2px;
  }
  .line {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    min-width: 0;
  }
  .name {
    flex: none;
    max-width: 60%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: var(--weight-medium);
  }
  .subtitle {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink-2);
  }
  .dates {
    flex: none;
    margin-inline-start: auto;
    white-space: nowrap;
  }
  .facts {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1-5);
  }
  .facts > .meta:first-child {
    margin-inline-end: var(--space-1);
  }
  .masters {
    display: inline-flex;
    gap: var(--space-1);
    margin-inline-start: auto;
  }
  .facts > .meta:last-child:not(:first-child) {
    margin-inline-start: auto;
  }
  .center {
    display: grid;
    flex: 1;
    place-items: center;
    min-height: 0;
  }
  @media (hover: hover) {
    .entry--link:hover {
      background: var(--hover);
    }
  }
</style>
