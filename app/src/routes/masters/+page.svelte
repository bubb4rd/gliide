<!--
  Resumes (/masters): the masters (a track's resume over the bank), each with its template and page limit, saved as
  you choose them, and the pages it takes now; then the PDFs in the export folder, newest first, filtered to resumes
  or cover letters (the filter stays as you left it when you come back). The desk shows the selected PDF, named in
  its bar (the full path on hover). The bar names what the desk shows: while the next PDF opens, the last one stays.
-->
<script lang="ts" module>
  import type { FileKind } from "$lib/api";

  type Show = "all" | FileKind;

  const view = $state<{ show: Show }>({ show: "all" });
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import {
    appError,
    folder,
    folderPdf,
    setMasterOptions,
    type FolderFile,
    type FolderPdf,
    type FolderView,
    type MasterRow,
  } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import { app } from "$lib/stores/app.svelte";
  import { shell } from "$lib/stores/shell.svelte";
  import {
    Badge,
    Button,
    Card,
    Desk,
    Empty,
    GroupHead,
    KeyFooter,
    type KeyCommand,
    Menu,
    PartRow,
    Segmented,
    Toolbar,
    tip,
    toast,
  } from "$lib/ui";

  const PAGE_LIMITS = [
    { value: 1, label: "1 page" },
    { value: 2, label: "2 pages" },
  ];
  const PANES = [
    { value: "resumes", label: "Resumes" },
    { value: "preview", label: "Preview" },
  ] as const;

  let loaded = $state(false);
  /** Masters with a save in flight. */
  let saving = $state<Record<string, boolean>>({});
  /** Bumped when a save fails, so the controls redraw from what's stored. */
  let resets = $state(0);
  let zoom = $state(100);
  let pane = $state<"resumes" | "preview">("resumes");

  let files = $state<FolderView | null>(null);
  let filesFailed = $state<string | null>(null);
  let selectedName = $state<string | null>(null);
  /** What the desk shows: the selected PDF once it has opened, or why it couldn't. */
  let shown = $state<{ file: FolderFile; pdf: FolderPdf | null; failed: string | null } | null>(null);
  /** Opened PDFs by name and modification time: going back to one is instant. */
  const opened = new Map<string, FolderPdf>();

  onMount(() => {
    void Promise.all([app.refreshLibrary(), loadFolder()]).then(() => (loaded = true));
  });

  async function loadFolder() {
    filesFailed = null;
    try {
      files = await folder();
    } catch (e) {
      filesFailed = appError(e).message;
    }
  }

  const pagesWord = (n: number) => `${n} ${n === 1 ? "page" : "pages"}`;
  const pdfsWord = (n: number) => `${n} ${n === 1 ? "PDF" : "PDFs"}`;

  const lib = $derived(app.library);
  const masters = $derived(lib?.masters ?? []);
  const templates = $derived(
    (lib?.templates ?? []).map((t) => ({ value: t.id, label: t.name, meta: t.atsSafe ? "ATS-safe" : undefined })),
  );
  const pdfs = $derived(files?.files ?? []);
  const others = $derived(pdfs.some((f) => f.kind === "other"));
  const SHOW = $derived<{ value: Show; label: string }[]>([
    { value: "all", label: "All" },
    { value: "resume", label: "Resumes" },
    { value: "cover-letter", label: "Cover letters" },
    ...(others ? [{ value: "other" as const, label: "Other" }] : []),
  ]);
  /** The filter in force: "Other" once no other PDF is left shows them all. */
  const show = $derived<Show>(view.show === "other" && !others ? "all" : view.show);
  const listed = $derived(show === "all" ? pdfs : pdfs.filter((f) => f.kind === show));
  const selectedIndex = $derived(Math.max(0, listed.findIndex((f) => f.name === selectedName)));
  const selected = $derived<FolderFile | undefined>(listed[selectedIndex]);
  const counted = $derived(show === "all" ? pdfsWord(pdfs.length) : `${listed.length} of ${pdfs.length}`);
  /** Nothing to show at all: no masters and no PDFs. */
  const empty = $derived(loaded && !masters.length && !pdfs.length && !filesFailed);

  // Opens the selected PDF; an answer for one no longer selected is kept for later but not shown.
  $effect(() => {
    const file = selected;
    if (!file) {
      shown = null;
      return;
    }
    const key = `${file.name}\n${file.modifiedAt}`;
    const cached = opened.get(key);
    if (cached) {
      shown = { file, pdf: cached, failed: null };
      return;
    }
    let current = true;
    folderPdf(file.name).then(
      (pdf) => {
        opened.set(key, pdf);
        if (current) shown = { file, pdf, failed: null };
      },
      (e) => {
        const error = appError(e);
        if (error.kind !== "missing") {
          if (current) shown = { file, pdf: null, failed: error.message };
          return;
        }
        // Gone since the folder was listed: say so and list it again.
        toast(error.message, { tone: "danger" });
        void loadFolder();
      },
    );
    return () => {
      current = false;
    };
  });

  function move(by: number) {
    if (!listed.length) return false;
    const next = Math.min(listed.length - 1, Math.max(0, selectedIndex + by));
    selectedName = listed[next].name;
  }

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

  function saved(rfc3339: string): string {
    const d = new Date(rfc3339);
    if (Number.isNaN(d.getTime())) return "";
    return d.toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      year: "numeric",
      hour: "numeric",
      minute: "2-digit",
    });
  }

  const KEYS: KeyCommand[] = $derived([
    { keys: ["↑", "↓"], label: "Select a PDF" },
    { keys: ["Esc"], label: "Show all PDFs", run: () => (view.show = "all"), disabled: show === "all" },
    ...(masters.length ? [] : [{ keys: ["E"], label: "Go to Experience", run: () => goto("/bank") }]),
  ]);

  $effect(() =>
    bindKeys({
      ArrowDown: () => move(1),
      ArrowUp: () => move(-1),
      Escape: () => (show === "all" ? false : void (view.show = "all")),
      ...(loaded && !masters.length ? { e: () => goto("/bank") } : {}),
    }),
  );
</script>

<Toolbar>
  {#snippet crumbs()}<b>Resumes</b>{/snippet}
</Toolbar>

{#if empty}
  <div class="center">
    <Empty
      title="No resumes yet"
      body="Import your resume in Experience; it becomes your first master."
    >
      {#snippet action()}<Button variant="primary" size="lg" kbd="E" onclick={() => goto("/bank")}>Go to Experience</Button>{/snippet}
    </Empty>
  </div>
{:else if loaded || masters.length}
  <div class={["split", shell.compact && "split--compact"]}>
    <div class="column" hidden={shell.compact && pane === "preview"}>
      {#if shell.compact}
        <div class="tabs"><Segmented options={[...PANES]} bind:value={pane} label="Show" /></div>
      {/if}

      <section class="group" aria-label="Masters">
        <GroupHead label="Masters" meta={masters.length ? String(masters.length) : undefined} />
        {#if masters.length}
          <div class="cards">
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
        {:else}
          <div class="note">
            <p class="meta">No masters yet: import your resume in Experience and it becomes the first.</p>
            <Button size="sm" kbd="E" onclick={() => goto("/bank")}>Go to Experience</Button>
          </div>
        {/if}
      </section>

      <section class="group" aria-label="Export folder">
        <GroupHead label="Export folder" meta={files ? counted : undefined} />
        {#if filesFailed}
          <div class="note">
            <p class="meta">{filesFailed}</p>
            <Button size="sm" onclick={loadFolder}>Try again</Button>
          </div>
        {:else if files}
          <p class="meta path" {@attach tip(files.dir)}>{files.dir}</p>
          {#if pdfs.length}
            <div class="filter">
              <Segmented options={SHOW} value={show} label="Show" onchange={(s) => (view.show = s)} />
            </div>
          {/if}
          {#each listed as file (file.name)}
            <PartRow
              label={saved(file.modifiedAt)}
              selected={file.name === selected?.name}
              onselect={() => (selectedName = file.name)}
            >
              {#snippet text()}<p class="file" {@attach tip(file.path)}>{file.name}</p>{/snippet}
            </PartRow>
          {:else}
            {#if pdfs.length}
              <div class="note">
                <p class="meta">No {show === "resume" ? "resumes" : "cover letters"} in this folder yet.</p>
                <Button size="sm" kbd="Esc" onclick={() => (view.show = "all")}>Show all PDFs</Button>
              </div>
            {:else}
              <p class="meta note">No PDFs here yet: exported resumes and cover letters land in this folder.</p>
            {/if}
          {/each}
        {/if}
      </section>

      <div class="keys-wrap"><KeyFooter keys={KEYS} /></div>
    </div>

    <div class="desk" hidden={shell.compact && pane === "resumes"}>
      {#if shell.compact}
        <div class="tabs"><Segmented options={[...PANES]} bind:value={pane} label="Show" /></div>
      {/if}
      <Desk pages={shown?.pdf?.pages ?? []} bind:zoom>
        {#snippet status()}
          {#if !shown}
            <Badge>No PDF to show</Badge>
          {:else if shown.failed}
            <Badge tone="danger">Can’t show it</Badge>
          {:else if shown.pdf}
            <Badge>{pagesWord(shown.pdf.pages.length)}</Badge>
          {/if}
        {/snippet}
        {#if shown}
          <span {@attach tip(shown.failed ?? shown.file.path)}>{shown.failed ?? shown.file.name}</span>
        {/if}
      </Desk>
    </div>
  </div>
{/if}

<style>
  .split {
    flex: 1;
    display: grid;
    grid-template-columns: minmax(var(--column-min), var(--column-max)) minmax(0, 1fr);
    min-height: 0;
  }
  .split--compact {
    grid-template-columns: minmax(0, 1fr);
  }
  .column {
    display: grid;
    align-content: start;
    gap: var(--space-6);
    min-height: 0;
    overflow: auto;
    overscroll-behavior: contain;
    scroll-padding-block-end: var(--space-16);
    padding: var(--space-6) var(--space-5) 0;
    box-shadow: inset -1px 0 0 var(--line);
  }
  .desk {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    min-width: 0;
    min-height: 0;
  }
  .desk > :global(:only-child) {
    grid-row: 1 / -1;
  }
  [hidden] {
    display: none;
  }
  .tabs {
    display: flex;
    justify-content: center;
    padding: var(--space-2);
  }
  .column .tabs {
    padding: 0;
  }
  /* Rows and heads carry 12px of their own padding: the group's edge moves out so their text lines up at 4px. */
  .group {
    display: grid;
    margin-inline: calc(var(--space-2) * -1);
  }
  .group:first-of-type :global(.group-head) {
    padding-top: 0;
  }
  .cards {
    display: grid;
    gap: var(--space-4);
    padding: var(--space-2) var(--space-3) 0;
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
  .note {
    display: grid;
    justify-items: start;
    gap: var(--space-2);
    margin: 0;
    padding: var(--space-1) var(--space-3) var(--space-2);
  }
  .note p {
    margin: 0;
  }
  .filter {
    display: flex;
    padding: 0 var(--space-3) var(--space-2);
  }
  .path {
    margin: 0;
    padding: var(--space-1) var(--space-3) var(--space-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .file {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .keys-wrap {
    position: sticky;
    bottom: 0;
    margin-inline: calc(var(--space-5) * -1);
  }
  .center {
    display: grid;
    flex: 1;
    place-items: center;
    min-height: 0;
  }
</style>
