<!--
  A bank entry's editor (DESIGN.md, Experience bank): the entry's facts, then each bullet as rows, one per wording
  (its text, then the variants you wrote), with the masters that show each. Every edit goes to the bank and the whole
  view comes back; the desk previews the entry, highlighted, in a master that shows it (or alone in the default
  template when none does). One request at a time: its answer replaces the view, a failure leaves it and says why.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import ArrowUpToLine from "@lucide/svelte/icons/arrow-up-to-line";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Eye from "@lucide/svelte/icons/eye";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import {
    appError,
    editEntry,
    entry,
    type BulletView,
    type EntryEdit,
    type EntryView,
    type MasterRef,
    type WordingView,
  } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import { shell } from "$lib/stores/shell.svelte";
  import {
    Badge,
    Button,
    Desk,
    Empty,
    GroupHead,
    Icon,
    KeyFooter,
    Menu,
    PartRow,
    Segmented,
    Textarea,
    Toolbar,
    toast,
  } from "$lib/ui";

  let { id }: { id: string } = $props();

  const PANES = [
    { value: "bullets", label: "Bullets" },
    { value: "preview", label: "Preview" },
  ] as const;
  /** Fold options show this much of a bullet's text: the menu's widest line. */
  const FOLD_CHARS = 48;

  let view = $state<EntryView | null>(null);
  let failed = $state<string | null>(null);
  let busy = $state(false);
  /** Bumped after every answer, so the menus show the view's choice again after one that failed. */
  let round = $state(0);
  let zoom = $state(100);
  let pane = $state<"bullets" | "preview">("bullets");
  let alive = true;

  const title = $derived(new Map(view?.masters.map((m) => [m.id, m.title]) ?? []));
  const fits = $derived(view !== null && view.pageCount <= view.pageLimit);
  const previewTitle = $derived(view?.preview ? (title.get(view.preview) ?? view.preview) : null);
  const masterOptions = $derived(view?.masters.map((m) => ({ value: m.id, label: m.title })) ?? []);
  const KEYS = $derived([
    { keys: ["↑", "↓"], label: "Select" },
    { keys: ["⏎"], label: "Edit" },
    { keys: ["V"], label: "Add a variant" },
    { keys: ["M"], label: "Make it the text" },
    ...(view?.masters.length ? [{ keys: [view.masters.length > 1 ? "1–9" : "1"], label: "Show in a master" }] : []),
    { keys: ["⌫"], label: "Remove variant" },
    ...((view?.bullets.length ?? 0) > 1 ? [{ keys: ["F"], label: "Fold into…" }] : []),
    ...((view?.masters.length ?? 0) > 1 ? [{ keys: ["P"], label: "Preview in" }] : []),
    { keys: ["Esc"], label: "Cancel" },
  ]);

  // ── Rows: every wording of every bullet, in order ──
  interface Row {
    key: string;
    bullet: BulletView;
    wording: WordingView;
  }
  const keyOf = (bullet: string, variant: number | null) => `${bullet}:${variant ?? "text"}`;
  const rows = $derived<Row[]>(
    view?.bullets.flatMap((bullet) =>
      bullet.wordings.map((wording) => ({ key: keyOf(bullet.id, wording.variant), bullet, wording })),
    ) ?? [],
  );
  let selectedKey = $state<string | null>(null);
  const selectedIndex = $derived(Math.max(0, rows.findIndex((r) => r.key === selectedKey)));
  const selected = $derived<Row | undefined>(rows[selectedIndex]);

  /** The open editor: one wording's words, or a new variant of `bullet`. */
  let editing = $state<{ key: string; bullet: string; variant: number | null; adding: boolean } | null>(null);
  let draft = $state("");

  onMount(() => {
    void open();
    return () => {
      alive = false;
    };
  });

  async function open() {
    failed = null;
    try {
      const v = await entry(id);
      if (alive) view = v;
    } catch (e) {
      if (alive) failed = appError(e).message;
    }
  }

  /** Sends one request (none while another runs); its answer becomes the view. */
  async function request(run: () => Promise<EntryView>): Promise<boolean> {
    if (busy) return false;
    busy = true;
    try {
      const v = await run();
      if (alive) view = v;
      return true;
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
      return false;
    } finally {
      busy = false;
      round++;
    }
  }

  const edit = (change: EntryEdit) => request(() => editEntry(id, change, view?.preview ?? null));

  /** The masters that show `bullet` (in any wording). */
  const showing = (bullet: BulletView) => (master: MasterRef) => bullet.wordings.some((w) => w.masters.includes(master.id));

  /** Masters that show the row's bullet in another wording, with the key that shows it there (1–9). */
  function elsewhere(row: Row): { master: MasterRef; key: string }[] {
    if (!view) return [];
    const shows = showing(row.bullet);
    return view.masters.flatMap((master, i) =>
      shows(master) && !row.wording.masters.includes(master.id) ? [{ master, key: String(i + 1) }] : [],
    );
  }

  function wordingLabel(wording: WordingView): string {
    return wording.variant === null ? "Text" : `Variant ${wording.variant + 1}`;
  }

  function list(ids: string[]): string {
    const names = ids.map((m) => title.get(m) ?? m);
    const last = names.pop() ?? "";
    return names.length ? `${names.join(", ")} and ${last}` : last;
  }

  function clip(text: string): string {
    return text.length > FOLD_CHARS ? `${text.slice(0, FOLD_CHARS - 1).trimEnd()}…` : text;
  }

  const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

  // ── Edits ──
  const focus = (element: HTMLElement) => element.focus();

  function move(by: number) {
    const next = rows[Math.min(Math.max(selectedIndex + by, 0), rows.length - 1)];
    if (next) selectedKey = next.key;
  }

  function startEdit(row: Row | undefined) {
    if (!row) return;
    selectedKey = row.key;
    editing = { key: row.key, bullet: row.bullet.id, variant: row.wording.variant, adding: false };
    draft = row.wording.text;
  }

  function startAdd(bullet: BulletView | undefined) {
    if (!bullet) return;
    editing = { key: `${bullet.id}:new`, bullet: bullet.id, variant: null, adding: true };
    draft = "";
  }

  async function save() {
    const open = editing;
    if (!open) return;
    const text = draft.trim();
    const bullet = view?.bullets.find((b) => b.id === open.bullet);
    if (open.adding) {
      if (!text) return void (editing = null);
      // The new variant comes last: after the text and every variant there is now.
      const index = bullet ? bullet.wordings.length - 1 : 0;
      if (!(await edit({ kind: "add-variant", bullet: open.bullet, text }))) return;
      selectedKey = keyOf(open.bullet, index);
    } else {
      const was = bullet?.wordings.find((w) => w.variant === open.variant)?.text;
      if (!text || text === was) return void (editing = null);
      if (!(await edit({ kind: "set-wording", bullet: open.bullet, variant: open.variant, text }))) return;
    }
    if (editing === open) editing = null;
  }

  function onEditorKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      editing = null;
    } else if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void save();
    }
  }

  async function makeMain(row: Row | undefined) {
    if (!row || row.wording.variant === null) return;
    if (await edit({ kind: "make-main", bullet: row.bullet.id, variant: row.wording.variant }))
      selectedKey = keyOf(row.bullet.id, null);
  }

  /** Removes a variant; the toast's Undo adds its words back as a variant and shows them where they were shown. */
  async function remove(row: Row | undefined) {
    if (!row || row.wording.variant === null) return;
    const { bullet } = row;
    const { text, masters } = row.wording;
    if (!(await edit({ kind: "remove-variant", bullet: bullet.id, variant: row.wording.variant }))) return;
    selectedKey = keyOf(bullet.id, null);
    toast("Removed the variant", {
      action: {
        label: "Undo",
        run: async () => {
          if (!(await edit({ kind: "add-variant", bullet: bullet.id, text }))) return;
          const wordings = view?.bullets.find((b) => b.id === bullet.id)?.wordings ?? [];
          const variant = wordings.length - 2;
          for (const master of masters) await edit({ kind: "show", master, bullet: bullet.id, variant });
          selectedKey = keyOf(bullet.id, variant);
        },
      },
    });
  }

  function show(row: Row | undefined, master: string) {
    if (!row) return;
    selectedKey = row.key;
    void edit({ kind: "show", master, bullet: row.bullet.id, variant: row.wording.variant });
  }

  async function fold(from: string, into: string) {
    if (await edit({ kind: "fold", from, into })) selectedKey = keyOf(into, null);
  }

  /** Opens a menu from its key: clicks the trigger inside `[data-menu=name]`. */
  function openMenu(name: string): unknown {
    const trigger = document.querySelector<HTMLButtonElement>(`[data-menu="${CSS.escape(name)}"] button`);
    if (!trigger || trigger.disabled) return false;
    trigger.scrollIntoView({ block: "nearest" });
    trigger.click();
  }

  // ── Keys ──
  const interactive = (target: EventTarget | null) =>
    target instanceof Element && target.closest("button, a, input, textarea, select") !== null;

  const DIGITS = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];

  $effect(() =>
    bindKeys({
      ArrowDown: () => move(1),
      ArrowUp: () => move(-1),
      Enter: (e) => {
        if (interactive(e.target)) return false;
        startEdit(selected);
      },
      v: () => startAdd(selected?.bullet),
      m: () => void makeMain(selected),
      Backspace: () => void remove(selected),
      Delete: () => void remove(selected),
      f: () => (selected ? openMenu(`fold:${selected.bullet.id}`) : false),
      p: () => openMenu("preview"),
      ...Object.fromEntries(
        DIGITS.map((digit) => [
          digit,
          () => {
            const target = selected && elsewhere(selected).find((t) => t.key === digit);
            if (!target) return false;
            show(selected, target.master.id);
          },
        ]),
      ),
    }),
  );
</script>

<Toolbar>
  {#snippet crumbs()}
    <a href="/bank">Experience bank</a><Icon icon={ChevronRight} size={14} weight="medium" /><b
      >{view?.name || "Entry"}</b
    >
  {/snippet}
</Toolbar>

<div class={["split", shell.compact && "split--compact"]}>
  <div class="column" hidden={shell.compact && pane === "preview"} aria-busy={busy || undefined}>
    {#if shell.compact}
      <div class="tabs"><Segmented options={[...PANES]} bind:value={pane} label="Show" /></div>
    {/if}

    {#if failed}
      <div class="center">
        <Empty title="Couldn’t open this entry" body={failed}>
          {#snippet action()}
            <div class="actions">
              <Button variant="primary" size="lg" onclick={() => goto("/bank")}>Back to Experience bank</Button>
              <Button size="lg" onclick={open}>Try again</Button>
            </div>
          {/snippet}
        </Empty>
      </div>
    {:else if !view}
      <div class="head" aria-hidden="true">
        <span class="bone bone--meta"></span>
        <span class="bone bone--title"></span>
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
      <header class="head">
        <p class="meta">{view.section}</p>
        <h1 class="title">{view.name}</h1>
        {#if view.subtitle || view.place || view.dates}
          <div class="facts">
            {#if view.subtitle || view.place}
              <span class="subtitle">{[view.subtitle, view.place].filter(Boolean).join(" · ")}</span>
            {/if}
            {#if view.dates}<span class="dates meta num">{view.dates}</span>{/if}
          </div>
        {/if}
        {#each view.notes as note, i (i)}<p class="meta">{note}</p>{/each}
      </header>

      <div class="bullets">
        {#each view.bullets as bullet, b (bullet.id)}
          {@const variants = bullet.wordings.length - 1}
          {@const others = view.bullets.filter((o) => o.id !== bullet.id)}
          <section
            class="bullet"
            aria-label="Bullet {b + 1}"
            data-current={selected?.bullet.id === bullet.id || undefined}
          >
            <GroupHead
              label="Bullet {b + 1}"
              meta={[variants ? plural(variants, "variant", "variants") : "No variants", ...bullet.skills].join(
                " · ",
              )}
            />
            {#each bullet.wordings as wording (wording.variant ?? "text")}
              {@const row = { key: keyOf(bullet.id, wording.variant), bullet, wording }}
              {#snippet editor()}
                <Textarea
                  size="lg"
                  bind:value={draft}
                  aria-label={wording.variant === null
                    ? "The bullet’s text"
                    : `Variant ${wording.variant + 1}`}
                  {@attach focus}
                  onkeydown={onEditorKey}
                />
                {#if wording.masters.length}
                  <p class="meta hint">Changes it in {list(wording.masters)}</p>
                {/if}
              {/snippet}
              <PartRow
                label={wordingLabel(wording)}
                selected={row.key === selected?.key}
                onselect={() => (selectedKey = row.key)}
                editor={editing?.key === row.key ? editor : undefined}
              >
                {#snippet text()}
                  <p class="words">{wording.text}</p>
                  <div class="shown">
                    {#each wording.masters as master (master)}
                      <Badge tone="accent">{title.get(master) ?? master}</Badge>
                    {:else}
                      <span class="meta">In no master</span>
                    {/each}
                  </div>
                {/snippet}
                {#snippet actions()}
                  <Button variant="ghost" size="sm" icon={Pencil} kbd="⏎" onclick={() => startEdit(row)}>Edit</Button>
                  {#if wording.variant !== null}
                    <Button variant="ghost" size="sm" icon={ArrowUpToLine} kbd="M" onclick={() => makeMain(row)}
                      >Make it the text</Button
                    >
                  {/if}
                  {#each elsewhere(row) as target (target.master.id)}
                    <Button
                      variant="ghost"
                      size="sm"
                      icon={Eye}
                      kbd={target.key}
                      onclick={() => show(row, target.master.id)}>Show in {target.master.title}</Button
                    >
                  {/each}
                  {#if wording.variant !== null}
                    <Button
                      variant="danger"
                      size="sm"
                      square
                      icon={Trash2}
                      aria-label="Remove this variant"
                      title="Remove this variant (⌫)"
                      onclick={() => remove(row)}
                    />
                  {/if}
                {/snippet}
              </PartRow>
            {/each}

            {#if editing?.adding && editing.bullet === bullet.id}
              <div class="adding">
                <Textarea
                  size="lg"
                  bind:value={draft}
                  placeholder="Another way to say it"
                  aria-label="New variant of bullet {b + 1}"
                  {@attach focus}
                  onkeydown={onEditorKey}
                />
                <p class="meta hint">Masters keep the words they show; show it in one from its row.</p>
              </div>
            {/if}

            <div class="bullet__foot">
              <Button variant="ghost" size="sm" icon={Plus} kbd="V" onclick={() => startAdd(bullet)}
                >Add a variant</Button
              >
              {#if others.length}
                {#key round}
                  <span class="menu" data-menu="fold:{bullet.id}">
                    <Menu
                      value=""
                      options={others.map((o) => ({ value: o.id, label: clip(o.wordings[0]?.text ?? o.id) }))}
                      label="Fold bullet {b + 1} into"
                      size="sm"
                      variant="ghost"
                      disabled={busy}
                      onchange={(into) => fold(bullet.id, into)}
                    >
                      {#snippet trigger()}Fold into…{/snippet}
                    </Menu>
                  </span>
                {/key}
              {/if}
            </div>
          </section>
        {/each}
      </div>

      <div class="keys-wrap"><KeyFooter keys={KEYS} /></div>
    {/if}
  </div>

  <div class="desk" hidden={shell.compact && pane === "bullets"}>
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
        {#if view && view.masters.length > 1}
          {#key round}
            <span class="menu" data-menu="preview">
              <Menu
                value={view.preview ?? ""}
                options={masterOptions}
                label="Preview in"
                size="sm"
                disabled={busy}
                onchange={(master) => request(() => entry(id, master))}
              />
            </span>
          {/key}
        {/if}
      {/snippet}
      {#if view}
        {#if !previewTitle}Entry alone, default template
        {:else if view.masters.length > 1}Entry highlighted
        {:else}{previewTitle} · entry highlighted{/if}
      {/if}
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
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }
  .head {
    display: grid;
    gap: var(--space-1);
    padding-inline: var(--space-1);
  }
  .head p {
    margin: 0;
  }
  .facts {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    min-width: 0;
  }
  .subtitle {
    min-width: 0;
    color: var(--ink-2);
  }
  .dates {
    flex: none;
    margin-inline-start: auto;
    white-space: nowrap;
  }
  .bullets {
    display: grid;
    margin-inline: calc(var(--space-2) * -1);
  }
  .bullet {
    display: grid;
  }
  .bullet:first-child :global(.group-head) {
    padding-top: 0;
  }
  /* The foot above already spaces the groups (its actions hide until you reach the bullet). */
  .bullet + .bullet :global(.group-head) {
    padding-top: var(--space-2);
  }
  .words {
    margin: 0;
  }
  .shown {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1);
    margin-top: var(--space-1-5);
  }
  .hint {
    margin: var(--space-1-5) 0 0;
  }
  .adding {
    padding: var(--space-2) var(--space-3);
  }
  /* The bullet's own actions show, like a row's, on hover, focus or when one of its rows is selected. */
  .bullet__foot {
    display: flex;
    align-items: center;
    gap: var(--space-0-5);
    min-height: var(--control-sm);
    padding: var(--space-1) var(--space-1-5) 0;
    opacity: 0;
  }
  .bullet:focus-within .bullet__foot,
  .bullet[data-current] .bullet__foot {
    opacity: 1;
  }
  .menu {
    display: contents;
  }
  .keys-wrap {
    position: sticky;
    bottom: 0;
    margin-inline: calc(var(--space-5) * -1);
  }

  /* Placeholders while the entry loads: static, no shimmer. */
  .bone {
    display: block;
    border-radius: var(--radius-xs);
    background: var(--sunken);
  }
  .bone--meta {
    width: 30%;
    height: var(--space-3);
  }
  .bone--title {
    width: 60%;
    height: var(--space-6);
  }
  .skeleton {
    display: grid;
    gap: var(--space-2);
    padding: var(--space-3);
  }
  .bone--line {
    height: var(--space-3);
  }
  .bone--short {
    width: 60%;
  }

  @media (hover: hover) {
    .bullet:hover .bullet__foot {
      opacity: 1;
    }
  }
</style>
