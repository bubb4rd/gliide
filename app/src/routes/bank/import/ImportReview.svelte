<!--
  Import review: the resume file at `path` copied into the bank's shape, before anything is added. The column lists
  each section of the file and where it goes (a bank section, or a new one), each entry new or the same as one of
  yours (merged into it unless you keep it separate), each skills line's new items, and contact items you may add;
  the desk shows the master it makes, with what's new to your bank highlighted. ⌘⏎ adds it, Esc discards it.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import Check from "@lucide/svelte/icons/check";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import {
    IMPORT_JOB,
    appError,
    cancel,
    discardImport,
    finishImport,
    importView,
    onProgress,
    readImport,
    type ImportChoices,
    type ImportView,
    type SameEntry,
  } from "$lib/api";
  import { chooseResume } from "$lib/import";
  import { bindKeys } from "$lib/keys";
  import { app } from "$lib/stores/app.svelte";
  import { shell } from "$lib/stores/shell.svelte";
  import {
    Badge,
    Button,
    Checkbox,
    Desk,
    Empty,
    GroupHead,
    Icon,
    Input,
    KeyFooter,
    Menu,
    Segmented,
    Thinking,
    Toolbar,
    toast,
  } from "$lib/ui";

  /** The master menu's value for a new master. */
  const NEW = "";
  const KEYS = [
    { keys: ["⌘", "⏎"], label: "Add to bank" },
    { keys: ["Esc"], label: "Discard" },
  ];
  const PANES = [
    { value: "import", label: "Import" },
    { value: "preview", label: "Preview" },
  ] as const;

  let { path }: { path: string } = $props();
  const file = $derived(path.split("/").pop() || path);

  let view = $state<ImportView | null>(null);
  /** The model call copying the file, while it runs. */
  let running = $state<{ startedAt: number } | null>(null);
  let failed = $state<string | null>(null);
  let adding = $state(false);
  let separate = $state<string[]>([]);
  let contact = $state<number[]>([]);
  let replace = $state(NEW);
  let title = $state("");
  let zoom = $state(100);
  let pane = $state<"import" | "preview">("import");
  let titleInput = $state<ReturnType<typeof Input>>();
  let alive = true;
  let added = false;
  /** Numbers view requests: only the newest one's answer is shown. */
  let ticket = 0;

  const choices = $derived<ImportChoices | null>(
    view && { import: view.id, separate, contact, replace: replace || null, title: title.trim() },
  );
  const masterOptions = $derived([
    { value: NEW, label: "New master" },
    ...(app.library?.masters ?? []).map((m) => ({ value: m.id, label: `Replace ${m.title}` })),
  ]);
  const named = $derived(replace !== NEW || title.trim() !== "");
  const fits = $derived(view !== null && view.pageCount <= view.pageLimit);

  onMount(() => {
    if (!path) {
      void goto("/bank");
      return;
    }
    let unlisten: (() => void) | undefined;
    void (async () => {
      unlisten = await onProgress((p) => {
        if (p.job === IMPORT_JOB && alive) running = { startedAt: performance.now() };
      });
      if (!alive) return unlisten();
      await Promise.all([app.library ? null : app.refreshLibrary(), read()]);
    })();
    return () => {
      alive = false;
      unlisten?.();
      if (running) void cancel(IMPORT_JOB).catch(() => {});
      if (!added) void discardImport().catch(() => {});
    };
  });

  $effect(() => bindKeys({ "mod+Enter": () => void add(), Escape: () => void goto("/bank") }));

  async function read() {
    failed = null;
    try {
      const v = await readImport(path);
      if (alive) view = v;
    } catch (e) {
      const err = appError(e);
      if (!alive) return;
      if (err.kind === "cancelled") void goto("/bank");
      else failed = err.message;
    } finally {
      running = null;
    }
  }

  /** Shows the import as the choices now say (the desk re-renders). */
  async function refresh() {
    if (!choices) return;
    const n = ++ticket;
    try {
      const v = await importView(choices);
      if (alive && n === ticket) view = v;
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
    }
  }

  function keepSeparate(id: string, on: boolean) {
    separate = on ? [...separate, id] : separate.filter((x) => x !== id);
    void refresh();
  }

  function addContact(i: number, on: boolean) {
    contact = on ? [...contact, i] : contact.filter((x) => x !== i);
    void refresh();
  }

  function chooseMaster(value: string) {
    replace = value;
    void refresh();
  }

  async function add() {
    if (!choices || adding) return;
    if (!named) {
      titleInput?.focus();
      return;
    }
    adding = true;
    const { newEntries, sameEntries, newBullets } = view!;
    const master = replace
      ? (app.library?.masters.find((m) => m.id === replace)?.title ?? "master")
      : title.trim();
    try {
      app.library = await finishImport(choices);
      added = true;
      toast(
        `Added ${plural(newEntries, "entry", "entries")} and ${plural(newBullets, "bullet", "bullets")}` +
          (sameEntries ? `, merged ${plural(sameEntries, "entry", "entries")}` : "") +
          ` · ${replace ? "Updated" : "New"} master: ${master}`,
      );
      await goto("/bank?added");
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
    } finally {
      adding = false;
    }
  }

  /** What merging keeps of yours where the file prints something else: `subtitle “…”, dates “…” and link`. */
  function kept(same: SameEntry): string {
    const parts = same.differs.map((f) =>
      f === "subtitle" && same.subtitle
        ? `subtitle “${same.subtitle}”`
        : f === "dates" && same.dates
          ? `dates “${same.dates}”`
          : f,
    );
    const last = parts.pop() ?? "";
    return parts.length ? `${parts.join(", ")} and ${last}` : last;
  }

  const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
</script>

<Toolbar>
  {#snippet crumbs()}
    <a href="/bank">Experience bank</a><Icon icon={ChevronRight} size={14} weight="medium" /><b>{file}</b>
  {/snippet}
  <Button variant="ghost" kbd="Esc" onclick={() => goto("/bank")}>Discard</Button>
  <Button variant="primary" icon={Check} kbd="⌘⏎" disabled={!view || adding} onclick={add}>Add to bank</Button>
</Toolbar>

<div class={["split", shell.compact && "split--compact"]}>
  <div class="column" hidden={shell.compact && pane === "preview"}>
    {#if shell.compact}
      <div class="tabs"><Segmented options={[...PANES]} bind:value={pane} label="Show" /></div>
    {/if}

    {#if failed}
      <div class="center">
        <Empty title="Couldn’t import {file}" body={failed}>
          {#snippet action()}
            <div class="row">
              <Button variant="primary" size="lg" onclick={() => chooseResume(app.settings?.exportDir)}>
                Choose another file
              </Button>
              <Button size="lg" onclick={read}>Try again</Button>
            </div>
          {/snippet}
        </Empty>
      </div>
    {:else if !view}
      <div class="running">
        <Thinking
          label="Copying your resume…"
          startedAt={running?.startedAt}
          oncancel={() => void cancel(IMPORT_JOB).catch(() => {})}
        />
      </div>
      <div aria-hidden="true">
        {#each [0, 1, 2, 3] as i (i)}
          <div class="skeleton">
            <span class="bone bone--line"></span>
            <span class="bone bone--line bone--short"></span>
          </div>
        {/each}
      </div>
    {:else}
      <header class="head enter" style:--i="0">
        <h1 class="title">{view.name || file}</h1>
        <p class="meta num">
          {plural(view.newEntries, "new entry", "new entries")} · {view.sameEntries} merged into yours ·
          {plural(view.newBullets, "new bullet", "new bullets")} · {plural(view.newItems, "new skill", "new skills")}
        </p>
      </header>

      <section class="master enter" style:--i="1" aria-label="Save as">
        <p class="label">Save as</p>
        <div class="row">
          <Menu value={replace} options={masterOptions} label="Master" onchange={chooseMaster} />
          {#if replace === NEW}
            <Input
              bind:this={titleInput}
              bind:value={title}
              size="md"
              class="title-input"
              placeholder="Title, like Graphic Design"
              aria-label="The new master's title"
              onkeydown={(e) => {
                if (e.key === "Enter") void add();
              }}
            />
          {/if}
        </div>
        <p class="meta">
          {replace === NEW
            ? "A new master that shows the resume as the file does."
            : "It shows the resume as the file does; its template and page limit stay."}
        </p>
      </section>

      {#if view.warnings.length}
        <ul class="warnings enter" style:--i="2">
          {#each view.warnings as warning (warning)}<li class="meta">{warning}</li>{/each}
        </ul>
      {/if}

      {#if view.contact.length}
        <section class="enter" style:--i="2" aria-label="Contact">
          <GroupHead label="Contact" meta="Adds to every resume" />
          <ul class="rows">
            {#each view.contact as item, i (i)}
              <li class="item">
                <label class="option">
                  <Checkbox
                    checked={item.chosen}
                    label={`Add ${item.text} to your contact line`}
                    onchange={(on) => addContact(i, on)}
                  />
                  <span class="name">{item.text}</span>
                </label>
              </li>
            {/each}
          </ul>
        </section>
      {/if}

      {#each view.sections as section, s (s)}
        <section class="enter" style:--i={Math.min(s + 3, 6)} aria-label={section.title}>
          <GroupHead
            label={section.title}
            meta={section.into === null
              ? "New section"
              : section.into === section.title
                ? "In your bank"
                : `Into ${section.into}`}
          />
          <ul class="rows">
            {#each section.entries as entry (entry.id)}
              <li class="item">
                <div class="line">
                  <span class="name">{entry.name || "Untitled"}</span>
                  {#if entry.subtitle}<span class="subtitle">{entry.subtitle}</span>{/if}
                  {#if entry.dates}<span class="dates meta num">{entry.dates}</span>{/if}
                </div>
                <div class="facts">
                  {#if entry.same && !entry.separate}
                    <Badge>Yours</Badge>
                    <span class="meta num">
                      {entry.same.matched} of {plural(entry.bullets, "bullet", "bullets")} already in it{entry.same.new
                        ? ` · ${entry.same.new} new`
                        : ""}
                    </span>
                  {:else}
                    <Badge tone="accent">New</Badge>
                    <span class="meta num">{plural(entry.bullets, "bullet", "bullets")}</span>
                  {/if}
                  {#if entry.same}
                    <label class="option meta">
                      <Checkbox
                        checked={entry.separate}
                        label={`Keep ${entry.name} separate from yours`}
                        onchange={(on) => keepSeparate(entry.id, on)}
                      />
                      Keep separate
                    </label>
                  {/if}
                </div>
                {#if entry.same && !entry.separate && entry.same.differs.length}
                  <p class="meta">Keeps your {kept(entry.same)}</p>
                {:else if entry.same && entry.separate}
                  <p class="meta">Added beside your {entry.same.name}{entry.same.dates ? `, ${entry.same.dates}` : ""}</p>
                {/if}
              </li>
            {/each}
            {#each section.groups as group, g (g)}
              <li class="item">
                <div class="line">
                  <span class="name">{group.label || "Skills"}</span>
                  <span class="dates meta num">{plural(group.items, "item", "items")}</span>
                </div>
                <div class="facts">
                  {#if group.into === null}
                    <Badge tone="accent">New</Badge>
                  {:else}
                    <Badge>Yours</Badge>
                    <span class="meta">
                      {group.newItems.length ? `Adds ${group.newItems.join(", ")}` : "Nothing new"}
                    </span>
                  {/if}
                </div>
              </li>
            {/each}
          </ul>
        </section>
      {/each}

      <div class="keys-wrap"><KeyFooter keys={KEYS} /></div>
    {/if}
  </div>

  <div class="desk" hidden={shell.compact && pane === "import"}>
    {#if shell.compact}
      <div class="tabs"><Segmented options={[...PANES]} bind:value={pane} label="Show" /></div>
    {/if}
    <Desk pages={view?.pages ?? []} bind:zoom>
      {#snippet status()}
        {#if !view}
          <Badge>No preview yet</Badge>
        {:else if fits}
          <Badge tone="gain" dot>Fits on {plural(view.pageLimit, "page", "pages")}</Badge>
        {:else}
          <Badge tone="warn" dot>{view.pageCount} pages: over by {view.pageCount - view.pageLimit}</Badge>
        {/if}
      {/snippet}
      {#if view && !view.first}Highlights show what’s new to your bank{/if}
    </Desk>
  </div>
</div>

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
  .center {
    display: grid;
    place-items: center;
    padding-block: var(--space-16);
  }
  .running {
    position: sticky;
    top: 0;
    z-index: 1;
    margin: calc(var(--space-3) * -1) calc(var(--space-5) * -1);
    padding: var(--space-3) var(--space-5);
    background: var(--paper);
  }
  .head {
    display: grid;
    gap: var(--space-1);
  }
  .head h1,
  .head p,
  .master p {
    margin: 0;
  }
  .master {
    display: grid;
    gap: var(--space-2);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }
  .row :global(.title-input) {
    flex: 1;
    min-width: 0;
  }
  .warnings {
    display: grid;
    gap: var(--space-1);
    margin: 0;
    padding: var(--space-3);
    list-style: none;
    border-radius: var(--radius-md);
    background: var(--warn-soft);
  }
  section :global(.group-head) {
    padding-top: 0;
  }
  .rows {
    display: grid;
    margin: 0;
    padding: 0;
    list-style: none;
    box-shadow: inset 0 1px 0 var(--line);
  }
  .item {
    display: grid;
    gap: var(--space-1);
    padding: var(--space-3);
    box-shadow: inset 0 -1px 0 var(--line);
  }
  .item p {
    margin: 0;
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
  .option {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
    cursor: default;
  }
  .facts .option {
    margin-inline-start: auto;
  }
  .keys-wrap {
    position: sticky;
    bottom: 0;
    margin-inline: calc(var(--space-5) * -1);
  }
  .skeleton {
    display: grid;
    gap: var(--space-2);
    padding: var(--space-3);
  }
  .bone {
    display: block;
    border-radius: var(--radius-xs);
    background: var(--sunken);
  }
  .bone--line {
    height: var(--space-3);
  }
  .bone--short {
    width: 60%;
  }
</style>
