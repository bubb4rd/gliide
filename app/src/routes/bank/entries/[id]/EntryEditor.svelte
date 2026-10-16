<!--
  A bank entry's editor (DESIGN.md, Experience): the entry's details (its name, subtitle, place and dates, then the
  lines under it, each edited in place; masters hold none of them, so every master that shows the entry prints the
  new ones), then each bullet as rows, one per wording (its text, then the variants you wrote), with the masters that
  show each. Every edit goes to the bank and the whole view comes back; the desk previews the entry, highlighted, in a
  master that shows it (or alone in the default template when none does). One request at a time: its answer replaces
  the view, a failure leaves it and says why.
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
    type EntryFacts,
    type EntryView,
    type MasterRef,
    type NoteLine,
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
    Input,
    KeyFooter,
    type KeyCommand,
    Menu,
    PartRow,
    Segmented,
    Textarea,
    Toolbar,
    tip,
    toast,
  } from "$lib/ui";

  let { id }: { id: string } = $props();

  const PANES = [
    { value: "entry", label: "Entry" },
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
  let pane = $state<"entry" | "preview">("entry");
  let alive = true;

  const title = $derived(new Map(view?.masters.map((m) => [m.id, m.title]) ?? []));
  const fits = $derived(view !== null && view.pageCount <= view.pageLimit);
  const previewTitle = $derived(view?.preview ? (title.get(view.preview) ?? view.preview) : null);
  const masterOptions = $derived(view?.masters.map((m) => ({ value: m.id, label: m.title })) ?? []);
  /** Every master that shows the entry: where its details print. */
  const showingEntry = $derived(view?.masters.map((m) => m.id) ?? []);

  // ── Rows: the entry's details, then every wording of every bullet, in order ──
  type Field = "name" | "subtitle" | "place" | "dates";
  interface FactRow {
    kind: "fact";
    key: string;
    field: Field;
    label: string;
  }
  interface NoteRow {
    kind: "note";
    key: string;
    index: number;
    note: NoteLine;
  }
  interface WordingRow {
    kind: "wording";
    key: string;
    bullet: BulletView;
    wording: WordingView;
  }
  type Row = FactRow | NoteRow | WordingRow;

  const FACT_ROWS: FactRow[] = (
    [
      ["name", "Name"],
      ["subtitle", "Subtitle"],
      ["place", "Place"],
      ["dates", "Dates"],
    ] as const
  ).map(([field, label]) => ({ kind: "fact", key: `fact:${field}`, field, label }));
  const noteKey = (index: number) => `note:${index}`;
  const keyOf = (bullet: string, variant: number | null) => `${bullet}:${variant ?? "text"}`;
  const noteRows = $derived<NoteRow[]>(
    view?.facts.notes.map((note, index) => ({ kind: "note", key: noteKey(index), index, note })) ?? [],
  );
  const rows = $derived<Row[]>(
    view
      ? [
          ...FACT_ROWS,
          ...noteRows,
          ...view.bullets.flatMap((bullet) =>
            bullet.wordings.map(
              (wording): WordingRow => ({ kind: "wording", key: keyOf(bullet.id, wording.variant), bullet, wording }),
            ),
          ),
        ]
      : [],
  );
  let selectedKey = $state<string | null>(null);
  const selectedIndex = $derived(Math.max(0, rows.findIndex((r) => r.key === selectedKey)));
  const selected = $derived<Row | undefined>(rows[selectedIndex]);
  /** The selected row when it's a bullet's wording: what the bullet keys act on. */
  const selectedWording = $derived(selected?.kind === "wording" ? selected : undefined);

  /** The open editor: a detail, a line under the entry (`index` null: a new one), or a wording of `bullet`'s (a new
   *  variant when `adding`). */
  type Editing =
    | { kind: "fact"; key: string; field: Field }
    | { kind: "note"; key: string; index: number | null }
    | { kind: "wording"; key: string; bullet: string; variant: number | null; adding: boolean };
  let editing = $state<Editing | null>(null);
  let draft = $state("");
  /** The edited line's bold label. */
  let draftLabel = $state("");

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
  const setFacts = (facts: EntryFacts) => edit({ kind: "set-facts", facts });

  /** The masters that show `bullet` (in any wording). */
  const showing = (bullet: BulletView) => (master: MasterRef) => bullet.wordings.some((w) => w.masters.includes(master.id));

  /** Masters that show the row's bullet in another wording, with the key that shows it there (1–9). */
  function elsewhere(row: WordingRow): { master: MasterRef; key: string }[] {
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
    if (!row || !view) return;
    selectedKey = row.key;
    if (row.kind === "fact") {
      editing = { kind: "fact", key: row.key, field: row.field };
      draft = view.facts[row.field];
    } else if (row.kind === "note") {
      editing = { kind: "note", key: row.key, index: row.index };
      draftLabel = row.note.label;
      draft = row.note.text;
    } else {
      editing = { kind: "wording", key: row.key, bullet: row.bullet.id, variant: row.wording.variant, adding: false };
      draft = row.wording.text;
    }
  }

  function startAdd(bullet: BulletView | undefined) {
    if (!bullet) return;
    editing = { kind: "wording", key: `${bullet.id}:new`, bullet: bullet.id, variant: null, adding: true };
    draft = "";
  }

  function startLine() {
    if (!view) return;
    editing = { kind: "note", key: "note:new", index: null };
    draftLabel = "";
    draft = "";
  }

  async function save() {
    const open = editing;
    if (!open || !view) return;
    const text = draft.trim();
    const { facts } = view;
    if (open.kind === "fact") {
      // An entry keeps its name: emptied, the edit is dropped. The other details may be cleared.
      if (text === facts[open.field] || (!text && open.field === "name")) return void (editing = null);
      if (!(await setFacts({ ...facts, [open.field]: text }))) return;
    } else if (open.kind === "note") {
      const line = { label: draftLabel.trim(), text };
      const was = open.index === null ? undefined : facts.notes[open.index];
      if (!text || (was && was.label === line.label && was.text === line.text)) return void (editing = null);
      const notes =
        open.index === null ? [...facts.notes, line] : facts.notes.map((n, i) => (i === open.index ? line : n));
      if (!(await setFacts({ ...facts, notes }))) return;
      selectedKey = noteKey(open.index ?? facts.notes.length);
    } else if (open.adding) {
      if (!text) return void (editing = null);
      // The new variant comes last: after the text and every variant there is now.
      const bullet = view.bullets.find((b) => b.id === open.bullet);
      const index = bullet ? bullet.wordings.length - 1 : 0;
      if (!(await edit({ kind: "add-variant", bullet: open.bullet, text }))) return;
      selectedKey = keyOf(open.bullet, index);
    } else {
      const was = view.bullets.find((b) => b.id === open.bullet)?.wordings.find((w) => w.variant === open.variant)?.text;
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

  async function makeMain(row: WordingRow | undefined) {
    if (!row || row.wording.variant === null) return;
    if (await edit({ kind: "make-main", bullet: row.bullet.id, variant: row.wording.variant }))
      selectedKey = keyOf(row.bullet.id, null);
  }

  /** What ⌫ removes: a line under the entry, or a variant (a bullet's text stays). */
  const removable = (row: Row | undefined) =>
    row?.kind === "note" || (row?.kind === "wording" && row.wording.variant !== null);

  function remove(row: Row | undefined) {
    if (row?.kind === "note") return removeLine(row);
    if (row?.kind === "wording") return removeVariant(row);
  }

  /** Removes a line under the entry; the toast's Undo puts it back where it was. */
  async function removeLine(row: NoteRow) {
    if (!view) return;
    const { facts } = view;
    if (!(await setFacts({ ...facts, notes: facts.notes.filter((_, i) => i !== row.index) }))) return;
    selectedKey = row.index > 0 ? noteKey(row.index - 1) : "fact:dates";
    toast("Removed the line", {
      action: {
        label: "Undo",
        run: async () => {
          if (!view) return;
          const now = view.facts;
          const notes = [...now.notes.slice(0, row.index), row.note, ...now.notes.slice(row.index)];
          if (await setFacts({ ...now, notes })) selectedKey = row.key;
        },
      },
    });
  }

  /** Removes a variant; the toast's Undo adds its words back as a variant and shows them where they were shown. */
  async function removeVariant(row: WordingRow) {
    if (row.wording.variant === null) return;
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

  function show(row: WordingRow | undefined, master: string) {
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

  const KEYS: KeyCommand[] = $derived([
    { keys: ["↑", "↓"], label: "Select" },
    { keys: ["⏎"], label: "Edit", run: () => startEdit(selected), disabled: !selected },
    { keys: ["L"], label: "Add a line", run: startLine, disabled: !view },
    {
      keys: ["V"],
      label: "Add a variant",
      run: () => startAdd(selectedWording?.bullet),
      disabled: !selectedWording,
    },
    {
      keys: ["M"],
      label: "Make it the text",
      run: () => void makeMain(selectedWording),
      disabled: selectedWording?.wording.variant == null,
    },
    ...(view?.masters.length ? [{ keys: [view.masters.length > 1 ? "1–9" : "1"], label: "Show in a master" }] : []),
    {
      keys: ["⌫"],
      label: selected?.kind === "note" ? "Remove line" : "Remove variant",
      run: () => void remove(selected),
      disabled: !removable(selected),
    },
    ...((view?.bullets.length ?? 0) > 1
      ? [
          {
            keys: ["F"],
            label: "Fold into…",
            run: () => selectedWording && openMenu(`fold:${selectedWording.bullet.id}`),
            disabled: !selectedWording,
          },
        ]
      : []),
    ...((view?.masters.length ?? 0) > 1 ? [{ keys: ["P"], label: "Preview in", run: () => openMenu("preview") }] : []),
    { keys: ["Esc"], label: "Cancel" },
  ]);

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
      l: startLine,
      v: () => startAdd(selectedWording?.bullet),
      m: () => void makeMain(selectedWording),
      Backspace: () => void remove(selected),
      Delete: () => void remove(selected),
      f: () => (selectedWording ? openMenu(`fold:${selectedWording.bullet.id}`) : false),
      p: () => openMenu("preview"),
      ...Object.fromEntries(
        DIGITS.map((digit) => [
          digit,
          () => {
            const target = selectedWording && elsewhere(selectedWording).find((t) => t.key === digit);
            if (!target) return false;
            show(selectedWording, target.master.id);
          },
        ]),
      ),
    }),
  );
</script>

<Toolbar>
  {#snippet crumbs()}
    <a href="/bank">Experience</a><Icon icon={ChevronRight} size={14} weight="medium" /><b
      >{view?.facts.name || "Entry"}</b
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
              <Button variant="primary" size="lg" onclick={() => goto("/bank")}>Back to Experience</Button>
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
        <h1 class="title">{view.facts.name}</h1>
      </header>

      {#snippet where()}
        {#if showingEntry.length}<p class="meta hint">Changes it in {list(showingEntry)}</p>{/if}
      {/snippet}
      {#snippet lineFields(name: string, labelFirst: boolean)}
        <div class="line-fields">
          <Input
            size="md"
            bind:value={draftLabel}
            placeholder="Label, like Major (optional)"
            aria-label="{name}’s label"
            {@attach labelFirst && focus}
            onkeydown={onEditorKey}
          />
          <Textarea
            size="lg"
            bind:value={draft}
            placeholder="What it says"
            aria-label={name}
            {@attach !labelFirst && focus}
            onkeydown={onEditorKey}
          />
        </div>
        {@render where()}
      {/snippet}

      <div class="parts">
        <section
          class="group"
          aria-label="Details"
          data-current={selected?.kind === "fact" || selected?.kind === "note" || undefined}
        >
          <GroupHead label="Details" />
          {#each FACT_ROWS as row (row.key)}
            {@const value = view.facts[row.field]}
            {#snippet editor()}
              <Input bind:value={draft} aria-label={row.label} {@attach focus} onkeydown={onEditorKey} />
              {@render where()}
            {/snippet}
            <PartRow
              label={row.label}
              selected={row.key === selected?.key}
              onselect={() => (selectedKey = row.key)}
              editor={editing?.key === row.key ? editor : undefined}
            >
              {#snippet text()}
                {#if value}<p class="words">{value}</p>{:else}<p class="words none">None</p>{/if}
              {/snippet}
              {#snippet actions()}
                <Button variant="ghost" size="sm" icon={Pencil} kbd="⏎" onclick={() => startEdit(row)}>Edit</Button>
              {/snippet}
            </PartRow>
          {/each}
          {#each noteRows as row (row.key)}
            {#snippet editor()}{@render lineFields(`Line ${row.index + 1}`, false)}{/snippet}
            <PartRow
              label="Line {row.index + 1}"
              selected={row.key === selected?.key}
              onselect={() => (selectedKey = row.key)}
              editor={editing?.key === row.key ? editor : undefined}
            >
              {#snippet text()}
                <p class="words">{#if row.note.label}<strong>{row.note.label}:</strong>{" "}{/if}{row.note.text}</p>
              {/snippet}
              {#snippet actions()}
                <Button variant="ghost" size="sm" icon={Pencil} kbd="⏎" onclick={() => startEdit(row)}>Edit</Button>
                <Button
                  variant="danger"
                  size="sm"
                  square
                  icon={Trash2}
                  aria-label="Remove this line"
                  {@attach tip("Remove this line (⌫)")}
                  onclick={() => removeLine(row)}
                />
              {/snippet}
            </PartRow>
          {/each}

          {#if editing?.kind === "note" && editing.index === null}
            <div class="adding">{@render lineFields("New line", true)}</div>
          {/if}

          <div class="foot">
            <Button variant="ghost" size="sm" icon={Plus} kbd="L" onclick={startLine}>Add a line</Button>
          </div>
        </section>

        {#each view.bullets as bullet, b (bullet.id)}
          {@const variants = bullet.wordings.length - 1}
          {@const others = view.bullets.filter((o) => o.id !== bullet.id)}
          <section
            class="group"
            aria-label="Bullet {b + 1}"
            data-current={selectedWording?.bullet.id === bullet.id || undefined}
          >
            <GroupHead
              label="Bullet {b + 1}"
              meta={[variants ? plural(variants, "variant", "variants") : "No variants", ...bullet.skills].join(
                " · ",
              )}
            />
            {#each bullet.wordings as wording (wording.variant ?? "text")}
              {@const row = { kind: "wording" as const, key: keyOf(bullet.id, wording.variant), bullet, wording }}
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
                      {@attach tip("Remove this variant (⌫)")}
                      onclick={() => removeVariant(row)}
                    />
                  {/if}
                {/snippet}
              </PartRow>
            {/each}

            {#if editing?.kind === "wording" && editing.adding && editing.bullet === bullet.id}
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

            <div class="foot">
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

  <div class="desk" hidden={shell.compact && pane === "entry"}>
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
  .parts {
    display: grid;
    margin-inline: calc(var(--space-2) * -1);
  }
  .group {
    display: grid;
  }
  .group:first-child :global(.group-head) {
    padding-top: 0;
  }
  /* The foot above already spaces the groups (its actions hide until you reach the group). */
  .group + .group :global(.group-head) {
    padding-top: var(--space-2);
  }
  .words {
    margin: 0;
  }
  .words strong {
    font-weight: var(--weight-semibold);
  }
  .none {
    color: var(--ink-3);
  }
  .line-fields {
    display: grid;
    gap: var(--space-2);
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
  /* A group's own actions show, like a row's, on hover, focus or when one of its rows is selected. */
  .foot {
    display: flex;
    align-items: center;
    gap: var(--space-0-5);
    min-height: var(--control-sm);
    padding: var(--space-1) var(--space-1-5) 0;
    opacity: 0;
  }
  .group:focus-within .foot,
  .group[data-current] .foot {
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
    .group:hover .foot {
      opacity: 1;
    }
  }
</style>
