<!--
  Experience bank: everything you've done, by section, with which masters show each entry. Before the first import,
  an empty state that imports ATS Fit's master.json. Read-only in M4 (the editors are M5).
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import FileUp from "@lucide/svelte/icons/file-up";
  import { appError, importResume } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import { app } from "$lib/stores/app.svelte";
  import { Badge, Button, Empty, GroupHead, Toolbar, toast } from "$lib/ui";

  let loaded = $state(false);
  let importing = $state(false);
  /** Set by an import in this visit, so the list enters once (a first result), not on every visit. */
  let arrived = $state(false);

  onMount(async () => {
    await Promise.all([app.refreshLibrary(), app.settings ? null : app.loadSettings()]);
    loaded = true;
  });

  const lib = $derived(app.library);
  const bank = $derived(lib?.bank ?? null);
  const masterTitle = $derived(new Map(lib?.masters.map((m) => [m.id, m.title]) ?? []));

  $effect(() => bindKeys({ i: () => void pick() }));

  async function pick() {
    if (importing) return;
    const path = await open({
      title: "Import an ATS Fit resume",
      multiple: false,
      directory: false,
      defaultPath: app.settings?.exportDir,
      filters: [{ name: "ATS Fit master.json", extensions: ["json"] }],
    }).catch((e) => {
      toast(appError(e).message, { tone: "danger" });
      return null;
    });
    if (!path) return;
    importing = true;
    try {
      const first = !app.library?.bank;
      const next = await importResume(path);
      app.library = next;
      arrived = first;
      const entries = next.bank?.entries ?? 0;
      toast(`Imported ${entries} ${entries === 1 ? "entry" : "entries"} from ${path.split("/").pop()}`);
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
    } finally {
      importing = false;
    }
  }

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
    <Button icon={FileUp} kbd="I" disabled={importing} onclick={pick}>
      {importing ? "Importing…" : "Import ATS Fit resume"}
    </Button>
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
              <li class="entry">
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
      body="Import your ATS Fit master.json: its entries become your bank, and its resume your first master."
    >
      {#snippet action()}
        <Button variant="primary" size="lg" icon={FileUp} kbd="I" disabled={importing} onclick={pick}>
          {importing ? "Importing…" : "Import ATS Fit resume"}
        </Button>
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
</style>
