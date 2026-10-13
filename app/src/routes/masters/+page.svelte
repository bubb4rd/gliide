<!--
  Resumes (/masters): one card per master (a track's resume over the bank) with its template and page limit, saved as
  you choose them, and the pages it takes now.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { appError, setMasterOptions, type MasterRow } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import { app } from "$lib/stores/app.svelte";
  import { Badge, Button, Card, Empty, Menu, Segmented, Toolbar, toast } from "$lib/ui";

  const PAGE_LIMITS = [
    { value: 1, label: "1 page" },
    { value: 2, label: "2 pages" },
  ];

  let loaded = $state(false);
  /** Masters with a save in flight. */
  let saving = $state<Record<string, boolean>>({});
  /** Bumped when a save fails, so the controls redraw from what's stored. */
  let resets = $state(0);

  onMount(async () => {
    await app.refreshLibrary();
    loaded = true;
  });

  const lib = $derived(app.library);
  const masters = $derived(lib?.masters ?? []);
  const templates = $derived(
    (lib?.templates ?? []).map((t) => ({ value: t.id, label: t.name, meta: t.atsSafe ? "ATS-safe" : undefined })),
  );

  $effect(() => {
    if (masters.length || !loaded) return;
    return bindKeys({ e: () => goto("/bank") });
  });

  async function save(master: MasterRow, template: string, pageLimit: number) {
    saving[master.id] = true;
    try {
      app.library = await setMasterOptions(master.id, template, pageLimit);
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
      await app.refreshLibrary();
      resets++;
    } finally {
      saving[master.id] = false;
    }
  }

  function updated(rfc3339: string): string {
    const d = new Date(rfc3339);
    if (Number.isNaN(d.getTime())) return "";
    return d.toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });
  }

  const pagesWord = (n: number) => `${n} ${n === 1 ? "page" : "pages"}`;
</script>

<Toolbar>
  {#snippet crumbs()}<b>Resumes</b>{/snippet}
</Toolbar>

{#if masters.length}
  <div class="scroll">
    <div class="column">
      {#each masters as master (master.id)}
        {@const over = master.pages - master.pageLimit}
        <Card pad as="section" aria-labelledby="master-{master.id}">
          <div class="card">
            <header class="head">
              <h2 class="name" id="master-{master.id}">{master.title}</h2>
              {#if over > 0}
                <Badge tone="warn" dot>{pagesWord(master.pages)}: over by {over}</Badge>
              {:else}
                <Badge tone="gain" dot>Fits on {pagesWord(master.pages)}</Badge>
              {/if}
            </header>
            <p class="meta num">
              {master.entries}
              {master.entries === 1 ? "entry" : "entries"}
              {#if updated(master.updatedAt)}· changed {updated(master.updatedAt)}{/if}
            </p>
            {#key resets}
              <dl class="fields">
                <div class="field">
                  <dt>Template</dt>
                  <dd>
                    <Menu
                      value={master.template}
                      options={templates}
                      label="Template for {master.title}"
                      disabled={saving[master.id]}
                      onchange={(t) => save(master, t, master.pageLimit)}
                    />
                  </dd>
                </div>
                <div class="field">
                  <dt>Page limit</dt>
                  <dd>
                    <Segmented
                      options={PAGE_LIMITS}
                      value={master.pageLimit}
                      label="Page limit for {master.title}"
                      disabled={saving[master.id]}
                      onchange={(n) => save(master, master.template, n)}
                    />
                  </dd>
                </div>
              </dl>
            {/key}
          </div>
        </Card>
      {/each}
    </div>
  </div>
{:else if loaded}
  <div class="center">
    <Empty
      title="No resumes yet"
      body="Import your ATS Fit resume in Experience; it becomes your first master."
    >
      {#snippet action()}<Button variant="primary" size="lg" kbd="E" onclick={() => goto("/bank")}>Go to Experience</Button>{/snippet}
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
    gap: var(--space-4);
    max-width: 640px;
    margin-inline: auto;
    padding: var(--space-8) var(--space-6) var(--space-12);
  }
  .card {
    display: grid;
    gap: var(--space-1);
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
  .name {
    flex: 1;
    min-width: 0;
    margin: 0;
    font-size: var(--text-lg);
    line-height: var(--leading-lg);
    letter-spacing: var(--tracking-lg);
    font-weight: var(--weight-medium);
  }
  .card > .meta {
    margin: 0;
  }
  .fields {
    display: grid;
    margin: var(--space-3) 0 0;
  }
  .field {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    min-height: var(--control-lg);
    padding-block: var(--space-2);
    box-shadow: inset 0 1px 0 var(--line);
  }
  .field:last-child {
    padding-bottom: 0;
  }
  dt {
    color: var(--ink-2);
  }
  dd {
    margin: 0;
  }
  .center {
    display: grid;
    flex: 1;
    place-items: center;
    min-height: 0;
  }
</style>
