<!--
  One job: its requirements (Extract), then the review (Suggest, then the fit check on every edit). The column holds
  the job header, the score, the skill chips and the change groups; the desk shows the tailored pages live. Every
  section folds from its head (remembered across jobs, `$lib/stores/folds`); Collapse all / Expand all (C) folds or
  opens them together. ↑↓ skips what's folded.
-->
<script lang="ts">
  import { onMount, type Snippet } from "svelte";
  import { goto } from "$app/navigation";
  import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
  import Banknote from "@lucide/svelte/icons/banknote";
  import BookmarkPlus from "@lucide/svelte/icons/bookmark-plus";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import ChevronsDownUp from "@lucide/svelte/icons/chevrons-down-up";
  import ChevronsUpDown from "@lucide/svelte/icons/chevrons-up-down";
  import Download from "@lucide/svelte/icons/download";
  import ImagePlus from "@lucide/svelte/icons/image-plus";
  import Pencil from "@lucide/svelte/icons/pencil";
  import PenLine from "@lucide/svelte/icons/pen-line";
  import Plus from "@lucide/svelte/icons/plus";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";
  import {
    appError,
    editTargets,
    setSalary,
    type ChangeView,
    type CustomEdit,
    type EditTargets,
    type LineView,
    type SalaryRange,
  } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import { logo } from "$lib/logo.svelte";
  import { salaryText } from "$lib/salary";
  import { STATUS, STATUS_OPTIONS } from "$lib/status";
  import { app } from "$lib/stores/app.svelte";
  import { folds } from "$lib/stores/folds.svelte";
  import { shell } from "$lib/stores/shell.svelte";
  import {
    Badge,
    Button,
    ChangeRow,
    Chip,
    CompanyMark,
    Desk,
    GroupHead,
    Icon,
    Input,
    KeyFooter,
    type KeyCommand,
    LogoDrop,
    Marked,
    Menu,
    PartRow,
    Popover,
    ScoreHeader,
    Segmented,
    Textarea,
    Thinking,
    Toolbar,
    tip,
    toast,
  } from "$lib/ui";
  import AddEdit from "./AddEdit.svelte";
  import SalaryEdit from "./SalaryEdit.svelte";
  import Letter from "./Letter.svelte";
  import { Session } from "./session.svelte";

  let { job }: { job: string } = $props();

  // `job` is fixed for this component's life: the route re-creates it per posting.
  // svelte-ignore state_referenced_locally
  const s = new Session(job);
  s.onerror = (e) =>
    toast(e.message, {
      tone: "danger",
      action: e.kind === "agent" ? { label: "Open Settings", run: () => goto("/settings") } : undefined,
    });

  onMount(() => {
    void s.start();
    // The posting's application, for the status menu in the crumbs.
    void app.refreshApplications();
    return () => {
      s.dispose();
      void app.refreshJobs();
    };
  });

  const STAGES = {
    extracting: "Reading the requirements…",
    suggesting: "Suggesting edits…",
    rewriting: "Rewriting the line…",
    writing: "Writing the cover letter…",
    revising: "Rewriting the paragraph…",
    importing: "Copying your resume…",
  } as const;
  /** A finished stage in the status's trail. */
  const DONE = {
    extracting: "Read the requirements",
    suggesting: "Suggested edits",
    rewriting: "Rewrote the line",
    writing: "Wrote the cover letter",
    revising: "Rewrote the paragraph",
    importing: "Copied your resume",
  } as const;
  const TABS: { value: Tab; label: string }[] = [
    { value: "resume", label: "Resume" },
    { value: "cover", label: "Cover letter" },
  ];
  const VERDICTS = {
    apply: { label: "Apply as is", tone: "accent" },
    tailor: { label: "Tailor first", tone: "accent" },
    stretch: { label: "Stretch", tone: "warn" },
    skip: { label: "Skip", tone: "neutral" },
  } as const;

  const review = $derived(s.review);
  const ready = $derived(s.phase === "ready" && review !== null);
  const head = $derived(review?.head ?? s.view?.head ?? null);
  const row = $derived(app.jobs.find((j) => j.id === job));
  const company = $derived(head?.company || row?.company || "Posting");
  /** Its application once exported (a posting has at most one): its status is set from the crumbs. */
  const application = $derived(app.applications.find((a) => a.job === job));
  /** Where and from where: the company has its own line, above the title. */
  const metaLine = $derived(head ? [head.location, head.url ? null : head.source].filter(Boolean).join(" · ") : "");
  const masters = $derived(
    (review?.masters ?? s.view?.masters ?? []).map((m) => ({
      value: m.id,
      label: m.title,
      meta: `${Math.round(m.score)}%`,
    })),
  );
  const skills = $derived(review?.skills ?? s.view?.skills ?? []);
  const skillsMeta = $derived(
    `${skills.filter((c) => c.state === "have" || c.state === "add").length} of ${skills.length} on the page`,
  );
  const rows = $derived(review?.groups.flatMap((g) => g.changes) ?? []);
  /** Lines no change touches ("Other lines"): edited or rewritten, each becomes your edit. */
  const others = $derived(review?.lines ?? []);
  const lineKey = (line: LineView) => `line:${line.line ?? "summary"}`;
  const counted = (c: ChangeView) => c.state === "in" || c.state === "forced";
  const changesIn = $derived(rows.filter(counted).length);
  const gain = $derived(review ? Math.round(review.projected.total) - Math.round(review.now.total) : 0);
  /** Points as the score line and group heads say them: `+7`, `−3`, `0`. */
  const signed = (n: number) => (n > 0 ? `+${n}` : n < 0 ? `−${-n}` : "0");
  const fits = $derived(review ? review.pageCount <= review.pageLimit : true);
  const pageNote = $derived.by(() => {
    if (!review) return "";
    const limit = review.pageLimit;
    if (fits) return `fits on ${limit} page${limit === 1 ? "" : "s"}`;
    return `${review.pageCount} pages: over by ${review.pageCount - limit}`;
  });

  // ── Folding ──
  /** Every section the review shows, by its `folds` id, top to bottom. */
  const sections = $derived([
    ...(skills.length ? ["skills"] : []),
    ...(review?.groups.map((g) => `group:${g.id}`) ?? []),
    ...(others.length ? ["lines"] : []),
    ...(review?.gaps.length ? ["gaps"] : []),
    ...(review?.knockouts.length ? ["knockouts"] : []),
  ]);
  const allFolded = $derived(sections.length > 0 && sections.every((id) => folds.has(id)));
  /** Collapses every section, or opens them all once every one is folded. */
  const foldAll = () => folds.set(sections, !allFolded);
  /** Opens the group a change landed in, so you see it. */
  function reveal(key: string) {
    const group = review?.groups.find((g) => g.changes.some((c) => c.key === key));
    if (group) folds.set([`group:${group.id}`], false);
  }

  // ── Selection and editing ──
  /** What ↑↓ moves through: every change in an open group, then every other line while they're open. */
  type Item = { key: string; change: ChangeView; line?: never } | { key: string; line: LineView; change?: never };
  const items = $derived<Item[]>([
    ...(review?.groups ?? [])
      .filter((g) => !folds.has(`group:${g.id}`))
      .flatMap((g) => g.changes.map((change) => ({ key: change.key, change }))),
    ...(folds.has("lines") ? [] : others.map((line) => ({ key: lineKey(line), line }))),
  ]);
  let selectedKey = $state<string | null>(null);
  const selectedIndex = $derived(Math.max(0, items.findIndex((i) => i.key === selectedKey)));
  const selectedItem = $derived<Item | undefined>(items[selectedIndex]);
  const selected = $derived(selectedItem?.change);
  const selectedLine = $derived(selectedItem?.line);
  let editing = $state<string | null>(null);
  let draft = $state("");
  let noting = $state<string | null>(null);
  let note = $state("");
  let zoom = $state(100);
  let pane = $state<"changes" | "preview">("changes");

  // ── Resume | Cover letter ──
  type Tab = "resume" | "cover";
  let tab = $state<Tab>("resume");
  const onCover = $derived(tab === "cover" && review !== null);
  const letter = $derived(s.cover && s.cover.review === review?.id ? s.cover : null);
  const letterFits = $derived(letter ? letter.pageCount <= 1 : true);
  // The letter loads once per review, when its tab first shows.
  let letterFor: string | null = null;
  $effect(() => {
    if (!onCover || !ready || !review || letterFor === review.id) return;
    letterFor = review.id;
    void s.loadCover();
  });

  function switchTab() {
    if (review) tab = tab === "resume" ? "cover" : "resume";
  }

  function writeLetter() {
    if (ready && s.drafting === null) void s.writeCover();
  }

  async function exportLetter() {
    if (!ready || !letter?.letter) return;
    const done = await s.exportCover();
    if (!done) return;
    toast(`Saved ${done.fileName}`, {
      action: { label: "Show in Finder", run: () => revealItemInDir(done.path) },
    });
    void app.refreshApplications();
  }

  // A new review (another master, mode, or a re-run) starts with nothing open.
  $effect(() => {
    void s.arrival;
    editing = null;
    noting = null;
  });

  const focus = (element: HTMLElement) => element.focus();

  function move(by: number) {
    const next = items[Math.min(Math.max(selectedIndex + by, 0), items.length - 1)];
    if (next) selectedKey = next.key;
  }

  function startEdit(change: ChangeView | undefined) {
    if (!ready || !change?.editable) return;
    selectedKey = change.key;
    noting = null;
    editing = change.key;
    draft = change.text;
  }

  function saveEdit(change: ChangeView) {
    const text = draft.trim();
    editing = null;
    if (text && text !== change.text) void s.word(change.key, text);
  }

  async function keepWording(change: ChangeView) {
    selectedKey = change.key;
    if (await s.saveVariant(change.key))
      toast("Saved as a variant of this bullet: every master and later review can use it");
  }

  function startNote(change: ChangeView) {
    selectedKey = change.key;
    editing = null;
    noting = change.key;
    note = "";
  }

  async function sendNote(change: ChangeView) {
    const text = note.trim();
    if (!text) return;
    noting = null;
    const done = await s.rewrite(change.key, text);
    if (done) rewrote(done.flags);
  }

  function rewrote(flags: string[]) {
    if (flags.length) toast(`Check the new wording: ${flags.join("; ")}`, { tone: "warn" });
    else toast("Rewrote the line from your note");
  }

  /** Rewrite with a note on the selected row (a change or an other line), when it applies. */
  function noteSelected(): unknown {
    if (selected?.rewritable) return startNote(selected);
    if (selectedLine?.rewritable) return startLineNote(selectedLine);
    return false;
  }

  // ── Other lines: editing or rewriting one makes it your edit, selected where it lands in its section ──
  function startLineEdit(line: LineView | undefined) {
    if (!ready || !line) return;
    selectedKey = lineKey(line);
    noting = null;
    editing = lineKey(line);
    draft = line.text;
  }

  function saveLineEdit(line: LineView) {
    const text = draft.trim();
    editing = null;
    if (!text || text === line.text) return;
    const edit: CustomEdit = line.line === null ? { kind: "summary", text } : { kind: "line", line: line.line, text };
    void landing(() => s.add(edit));
  }

  function startLineNote(line: LineView) {
    if (!line.rewritable) return;
    selectedKey = lineKey(line);
    editing = null;
    noting = lineKey(line);
    note = "";
  }

  async function sendLineNote(line: LineView) {
    const text = note.trim();
    if (!text) return;
    noting = null;
    const done = await landing(() => s.rewriteLine(line.line, lineKey(line), text));
    if (done) rewrote(done.flags);
  }

  /** Runs `call`, then selects the change it added: the line's new row, in its section (opened if folded). */
  async function landing<T>(call: () => Promise<T>): Promise<T> {
    const before = new Set(rows.map((c) => c.key));
    const done = await call();
    const fresh = rows.find((c) => !before.has(c.key));
    if (fresh) {
      selectedKey = fresh.key;
      reveal(fresh.key);
    }
    return done;
  }

  function checkboxLabel(change: ChangeView) {
    if (change.custom) return "Include your edit";
    if (change.op.startsWith("skill_")) return "Include this skill change";
    if (["include", "exclude", "swap_entry"].includes(change.op)) return "Include this entry change";
    if (change.editable) return "Include this rewrite";
    return "Include this change";
  }

  // ── Toolbar ──
  function chooseMaster(master: string) {
    s.master = master;
    void s.suggest(master, s.mode);
  }

  async function exportNow() {
    if (!ready) return;
    const done = await s.export();
    if (!done) return;
    const onFile = done.onFile === null ? "" : ` · ${Math.round(done.onFile)}% on file`;
    toast(`Saved ${done.fileName}${onFile}`, {
      action: { label: "Show in Finder", run: () => revealItemInDir(done.path) },
    });
    void app.refreshJobs();
    void app.refreshApplications();
  }

  function rerun() {
    if (ready) void s.suggest(s.master, s.mode, true);
  }

  // The sidebar's Recent shows this job's names (from Extract) and projected score: refresh them once they settle.
  $effect(() => {
    void s.view;
    void review?.projected.total;
    const timer = setTimeout(() => void app.refreshJobs(), 600);
    return () => clearTimeout(timer);
  });

  // ── Add your own ──
  let pickerOpen = $state(false);
  let addButton = $state<HTMLButtonElement>();
  let targets = $state<EditTargets | null>(null);

  async function openPicker() {
    if (!ready || !review) return;
    try {
      targets = await editTargets(review.id);
      // The picker opens above its button, which sits below the groups: bring it into view first.
      addButton?.scrollIntoView({ block: "nearest" });
      pickerOpen = true;
    } catch (e) {
      s.onerror(appError(e));
    }
  }

  async function addOwn(edit: CustomEdit) {
    const before = new Set(rows.map((c) => c.key));
    const ok = await s.add(edit);
    if (ok) pickerOpen = false;
    // Your edit lands in its section's group: open it if it's folded.
    for (const c of rows) if (!before.has(c.key)) reveal(c.key);
    return ok;
  }

  // ── Company photo (the logo) ──
  let logoOpen = $state(false);
  let logoMark = $state<HTMLElement>();
  const logoSrc = $derived(head?.companyKey ? (app.logos[head.companyKey] ?? null) : null);

  /** Closes the panel once `setting` set the logo; a failure leaves it open, with the toast saying why. */
  async function photo(setting: Promise<boolean>) {
    if (await setting) logoOpen = false;
  }

  $effect(() => logo.watch(() => (logoOpen = false)));
  $effect(() => {
    const company = head?.company;
    if (!logoOpen || !company) return;
    const paste = () => void photo(logo.set(company, { kind: "clipboard" }));
    return bindKeys({ v: paste, "mod+v": paste });
  });

  // ── Salary range ──
  let salaryOpen = $state(false);
  let salaryAnchor = $state<HTMLButtonElement>();
  const salary = $derived(row?.salary ?? null);

  function openSalary() {
    if (!row) return false;
    salaryOpen = true;
  }

  /** Saves it as yours; the row comes back with the range it now shows. A failure leaves the panel open. */
  async function saveSalary(next: SalaryRange | null): Promise<boolean> {
    try {
      const saved = await setSalary(job, next);
      const i = app.jobs.findIndex((j) => j.id === saved.id);
      if (i >= 0) app.jobs[i] = saved;
      salaryOpen = false;
      return true;
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
      return false;
    }
  }

  const KEYS: KeyCommand[] = $derived([
    { keys: ["↑", "↓"], label: "Select" },
    { keys: ["Space"], label: "Toggle", run: () => selected && void s.toggle(selected.key), disabled: !ready || !selected },
    {
      keys: ["⏎"],
      label: "Edit",
      run: () => (selectedLine ? startLineEdit(selectedLine) : startEdit(selected)),
      disabled: !ready || !(selectedLine || selected?.editable),
    },
    {
      keys: ["N"],
      label: "Rewrite with a note",
      run: noteSelected,
      disabled: !ready || s.rewriting !== null || !(selected?.rewritable || selectedLine?.rewritable),
    },
    {
      keys: ["S"],
      label: "Save as variant",
      run: () => selected && void keepWording(selected),
      disabled: !ready || !selected?.savable,
    },
    { keys: ["A"], label: "Add your own", run: () => void openPicker(), disabled: !ready },
    { keys: ["C"], label: allFolded ? "Expand all" : "Collapse all", run: foldAll, disabled: !review },
    { keys: ["E"], label: "Export", run: () => void exportNow(), disabled: !ready },
    { keys: ["R"], label: "Re-run", run: rerun, disabled: !ready },
    { keys: ["⌘Z"], label: "Undo", run: () => void s.undo(), disabled: !review?.canUndo },
    { keys: ["⇧⌘Z"], label: "Redo", run: () => void s.redo(), disabled: !review?.canRedo },
    { keys: ["$"], label: "Salary range", run: openSalary, disabled: !row },
    { keys: ["⌘⌫"], label: "Delete posting", run: () => void removeJob(), disabled: !row },
    { keys: ["Tab"], label: "Cover letter", run: switchTab, disabled: !review },
  ]);
  /** The letter tab's commands that the review owns; the letter lists its own rows' commands before them. */
  const LETTER_KEYS: KeyCommand[] = $derived([
    {
      keys: ["W"],
      label: letter?.letter ? "Rewrite letter" : "Write letter",
      run: writeLetter,
      disabled: !ready || s.drafting !== null,
    },
    {
      keys: ["E"],
      label: "Export",
      run: () => void exportLetter(),
      disabled: !ready || !letter?.letter || s.drafting !== null,
    },
    { keys: ["Tab"], label: "Resume", run: switchTab },
  ]);

  // ── Keys ──
  const interactive = (target: EventTarget | null) =>
    target instanceof Element && target.closest("button, a, input, textarea, select") !== null;

  $effect(() =>
    bindKeys({
      // On the letter tab, ↑↓ and ⏎ belong to the letter's own bindings.
      ArrowDown: () => move(1),
      ArrowUp: () => move(-1),
      " ": (e) => {
        if (interactive(e.target) || onCover || !selected) return false;
        void s.toggle(selected.key);
      },
      Enter: (e) => {
        if (interactive(e.target)) return false;
        if (selectedLine) startLineEdit(selectedLine);
        else startEdit(selected);
      },
      Tab: (e) => {
        if (interactive(e.target) || !review) return false;
        switchTab();
      },
      a: () => (onCover ? false : void openPicker()),
      c: () => (onCover || !review ? false : foldAll()),
      n: () => (onCover || !ready || s.rewriting !== null ? false : noteSelected()),
      e: () => void (onCover ? exportLetter() : exportNow()),
      r: () => (onCover ? false : rerun()),
      w: () => (onCover ? writeLetter() : false),
      $: openSalary,
      s: () => (onCover || !selected?.savable ? false : void keepWording(selected)),
      "mod+z": (e) => {
        if (interactive(e.target) || onCover) return false;
        void s.undo();
      },
      "shift+mod+z": (e) => {
        if (interactive(e.target) || onCover) return false;
        void s.redo();
      },
      // In a field, ⌘⌫ deletes the line instead.
      "mod+Backspace": (e) => {
        if (e.target instanceof Element && e.target.closest("input, textarea, [contenteditable]")) return false;
        void removeJob();
      },
    }),
  );

  /** Back to Applications, then the posting is deleted, with Undo in the toast. */
  async function removeJob() {
    const current = row;
    if (!current) return;
    await goto("/applications");
    app.deleteJob(current);
  }
</script>

<!-- A section's head folds it; its body stays, empty, as the target of the head's aria-controls. -->
{#snippet foldable(id: string, label: string, meta: string, body: Snippet)}
  <GroupHead {label} {meta} collapsed={folds.has(id)} controls="fold-{id}" ontoggle={(c) => folds.set([id], c)} />
  <div class="fold" id="fold-{id}">{#if !folds.has(id)}{@render body()}{/if}</div>
{/snippet}

<Toolbar>
  {#snippet crumbs()}
    <a href="/applications">Applications</a><Icon icon={ChevronRight} size={14} weight="medium" /><b>{company}</b>
    {#if application}
      {@const current = application}
      <Menu
        value={current.status}
        options={STATUS_OPTIONS}
        label="Status"
        variant="ghost"
        size="sm"
        onchange={(status) => app.setStatus(current, status)}
      >
        {#snippet trigger(option)}<Badge tone={STATUS[current.status].tone}>{option?.label}</Badge>{/snippet}
      </Menu>
    {/if}
  {/snippet}
  <!-- What you're working on (the document, and its master or a rewrite), then a divider, then export and delete. -->
  {#if review}
    <Segmented options={TABS} bind:value={tab} label="Document" />
  {/if}
  {#if onCover}
    {#if letter?.letter}
      <Button icon={PenLine} kbd="W" disabled={!ready || s.drafting !== null} onclick={writeLetter}
        >Rewrite letter</Button
      >
    {/if}
  {:else if masters.length}
    <Menu
      value={s.master ?? ""}
      options={masters}
      label="Master"
      placement="bottom-end"
      disabled={!ready}
      onchange={chooseMaster}
    />
  {/if}
  {#if review || masters.length}<hr />{/if}
  {#if onCover}
    <Button
      variant="primary"
      icon={Download}
      kbd="E"
      disabled={!ready || !letter?.letter || s.drafting !== null}
      onclick={exportLetter}>Export letter</Button
    >
  {:else}
    <Button variant="primary" icon={Download} kbd="E" disabled={!ready} onclick={exportNow}>Export</Button>
  {/if}
  <Button
    variant="ghost"
    square
    icon={Trash2}
    aria-label="Delete posting"
    {@attach tip("Delete posting (⌘⌫)")}
    disabled={!row}
    onclick={removeJob}
  />
</Toolbar>

<div class={["split", shell.compact && "split--compact"]}>
  <div class="column" hidden={shell.compact && pane === "preview"}>
    {#if shell.compact}
      <div class="tabs">
        <Segmented
          options={[
            { value: "changes", label: "Changes" },
            { value: "preview", label: "Preview" },
          ]}
          bind:value={pane}
          label="Show"
        />
      </div>
    {/if}

    {#if head}
      <div class="job">
        <div class="row">
          {#if review}<Badge tone={VERDICTS[review.verdict].tone}>{VERDICTS[review.verdict].label}</Badge>{/if}
          <span class="meta">
            {metaLine}{#if salary}{metaLine ? " · " : ""}<button
                type="button"
                class="salary"
                {@attach tip("Change the salary range ($)")}
                bind:this={salaryAnchor}
                onclick={openSalary}>{salaryText(salary)}</button
              >{/if}{#if head.url}{metaLine || salary ? " · " : ""}<a
                class="source"
                href={head.url}
                onclick={(e) => {
                  e.preventDefault();
                  if (head.url) void openUrl(head.url);
                }}>{head.source}</a
              >{/if}
          </span>
          {#if (row && !salary) || (head.companyKey && !logoSrc)}
            <span class="tools">
              {#if row && !salary}
                <Button
                  bind:element={salaryAnchor}
                  variant="ghost"
                  size="sm"
                  icon={Banknote}
                  kbd="$"
                  onclick={openSalary}>Add salary</Button
                >
              {/if}
              {#if head.companyKey && !logoSrc}
                <Button variant="ghost" size="sm" icon={ImagePlus} popovertarget="logo-panel">Upload company photo</Button>
              {/if}
            </span>
          {/if}
        </div>
        <div class={["named", !head.company && "named--solo"]}>
          <span class="lead">
            {#if head.companyKey}
              <CompanyMark
                size={48}
                src={logoSrc}
                label={logoSrc ? `Change ${head.company}'s photo` : "Upload company photo"}
                over={logo.over === head.company}
                bind:element={logoMark}
                popovertarget="logo-panel"
                data-logo-drop={head.company}
              />
            {:else}
              <CompanyMark size={48} />
            {/if}
          </span>
          {#if head.company}<p class="company" {@attach tip(head.company)}>{head.company}</p>{/if}
          <h1 class="headline">{head.role}</h1>
        </div>
        {#if review?.headline}<p class="lede">{review.headline}</p>{/if}
      </div>
      <Popover bind:open={salaryOpen} anchor={salaryAnchor} aria-label="Salary range">
        {#if salaryOpen}<SalaryEdit {salary} onsave={saveSalary} />{/if}
      </Popover>
      {#if head.companyKey}
        {@const company = head.company}
        <Popover bind:open={logoOpen} anchor={logoMark} id="logo-panel" aria-label="Company photo">
          <LogoDrop
            data-logo-drop={company}
            over={logo.over === company}
            busy={logo.busy}
            set={logoSrc !== null}
            onpaste={() => photo(logo.set(company, { kind: "clipboard" }))}
            onchoose={() => photo(logo.choose(company))}
            onremove={() => photo(logo.remove(company, head.companyKey).then(() => true))}
          />
        </Popover>
      {/if}
    {:else if s.phase === "opening"}
      <div class="job" aria-hidden="true">
        <span class="bone bone--meta"></span>
        <span class="bone bone--headline"></span>
      </div>
    {/if}

    {#if s.running && !s.rewriting && s.drafting === null}
      <div class="running">
        <Thinking
          steps={s.steps.map((step) => ({ label: DONE[step.stage], ms: step.ms }))}
          label={STAGES[s.running.stage]}
          startedAt={s.running.startedAt}
          oncancel={() => s.cancel()}
        />
      </div>
    {/if}

    {#if s.phase === "failed" && s.error}
      <div class="problem" role="alert">
        <p>{s.error.message}</p>
        <div class="row">
          {#if s.error.kind === "agent"}
            <Button variant="primary" onclick={() => goto("/settings")}>Open Settings</Button>
          {:else if !app.library?.bank}
            <Button variant="primary" onclick={() => goto("/bank")}>Open Experience</Button>
          {/if}
          <Button icon={RotateCcw} onclick={() => (s.view ? s.suggest(null, null) : s.open())}>Try again</Button>
        </div>
      </div>
    {:else if s.phase === "stopped"}
      <div class="problem">
        <p class="meta">Stopped. Nothing was changed.</p>
        <div class="row">
          <Button icon={Sparkles} onclick={() => (s.view ? s.suggest(null, null) : s.open())}>
            {s.view ? "Suggest edits" : "Read the requirements"}
          </Button>
        </div>
      </div>
    {/if}

    {#if onCover && ready}
      <Letter {s} commands={LETTER_KEYS} />
    {:else if review && (s.phase === "ready" || s.phase === "suggesting")}
      <!-- While another master, mode or a re-run is suggesting, the current review stays, inert, under the progress. -->
      <div class={["review", !ready && "review--stale"]} inert={!ready}>
        {#key s.arrival}
          <div class="enter" style:--i="0">
            <ScoreHeader
              now={review.now.total}
              projected={review.projected.total}
              ceiling={review.ceiling.total}
              components={review.projected.components}
              nowTitle="Your {review.master.title} master as it is"
            >
              <strong>{signed(gain)} points</strong> from {changesIn}
              {changesIn === 1 ? "change" : "changes"} · {pageNote}
            </ScoreHeader>
          </div>

          <div class="fold-all enter" style:--i="1">
            <Button
              variant="ghost"
              size="sm"
              icon={allFolded ? ChevronsUpDown : ChevronsDownUp}
              kbd="C"
              onclick={foldAll}>{allFolded ? "Expand all" : "Collapse all"}</Button
            >
          </div>

          {#if skills.length}
            <div class="skills enter" style:--i="1">
              {#snippet chips()}
                <div class="row chips">
                  {#each skills as chip (chip.term)}
                    <Chip
                      state={chip.state}
                      p1={chip.priority === 1}
                      {@attach tip(chip.locked)}
                      onclick={() => (chip.locked ? toast(chip.locked, { tone: "neutral" }) : s.skill(chip.term))}
                      >{chip.term}</Chip
                    >
                  {/each}
                </div>
              {/snippet}
              {@render foldable("skills", "Skills and keywords", skillsMeta, chips)}
            </div>
          {/if}

          <div class="changes enter" style:--i="2">
            <!-- Shared by change rows and other lines: the wording field, the note for a rewrite, its progress. -->
            {#snippet wording(save: () => void)}
              <Textarea
                size="lg"
                bind:value={draft}
                aria-label="Your wording"
                {@attach focus}
                onkeydown={(e) => {
                  if (e.key === "Escape") {
                    e.stopPropagation();
                    editing = null;
                  } else if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    save();
                  }
                }}
              />
            {/snippet}
            {#snippet noteFor(key: string, send: () => void)}
              {#if noting === key}
                <form
                  class="note"
                  onsubmit={(e) => {
                    e.preventDefault();
                    send();
                  }}
                >
                  <Input
                    size="md"
                    bind:value={note}
                    placeholder="What should change? “Shorter”, “mention Kafka”…"
                    aria-label="Note for the rewrite"
                    {@attach focus}
                    onkeydown={(e) => {
                      if (e.key !== "Escape") return;
                      e.stopPropagation();
                      noting = null;
                    }}
                  />
                  <Button type="submit" variant="primary" size="sm" disabled={!note.trim()}>Rewrite</Button>
                </form>
              {:else if s.rewriting === key && s.running}
                <div class="note">
                  <Thinking label={STAGES.rewriting} startedAt={s.running.startedAt} oncancel={() => s.cancel()} />
                </div>
              {/if}
            {/snippet}
            {#each review.groups as group (group.id)}
              {@const noun = group.id === "bank" ? "suggestion" : "change"}
              {#snippet groupRows()}
                {#each group.changes as change (change.key)}
                  {#snippet editor()}{@render wording(() => saveEdit(change))}{/snippet}
                  <ChangeRow
                    state={change.state}
                    checked={counted(change)}
                    selected={change.key === selected?.key}
                    points={change.points}
                    label={checkboxLabel(change)}
                    ontoggle={() => s.toggle(change.key)}
                    onselect={() => (selectedKey = change.key)}
                    was={change.was}
                    reason={change.reason}
                    warning={change.warning}
                    yours={change.yours}
                    meta={change.meta || undefined}
                    editor={editing === change.key ? editor : undefined}
                  >
                    {#snippet text()}<span {@attach tip(change.why)}
                        ><Marked text={change.text} marks={change.marks} /></span
                      >{/snippet}
                    {#snippet actions()}
                      {#if change.editable}
                        <Button variant="ghost" size="sm" icon={Pencil} kbd="⏎" onclick={() => startEdit(change)}
                          >Edit</Button
                        >
                      {/if}
                      {#if change.rewritable}
                        <Button
                          variant="ghost"
                          size="sm"
                          icon={Sparkles}
                          kbd="N"
                          disabled={s.rewriting !== null}
                          onclick={() => startNote(change)}>Rewrite with a note</Button
                        >
                      {/if}
                      {#if change.savable}
                        <Button
                          variant="ghost"
                          size="sm"
                          icon={BookmarkPlus}
                          kbd="S"
                          {@attach tip("Keep this wording in your bank, as a variant of the bullet")}
                          onclick={() => keepWording(change)}>Save as variant</Button
                        >
                      {/if}
                      {#if change.yours}
                        <Button
                          variant="ghost"
                          size="sm"
                          square
                          icon={change.custom ? X : RotateCcw}
                          aria-label={change.custom ? "Remove your edit" : "Back to the suggested wording"}
                          {@attach tip(change.custom ? "Remove your edit" : "Back to the suggested wording")}
                          onclick={() => s.remove(change)}
                        />
                      {/if}
                    {/snippet}
                  </ChangeRow>
                  {@render noteFor(change.key, () => void sendNote(change))}
                {:else}
                  <p class="meta quiet-row">No changes in this group.</p>
                {/each}
              {/snippet}
              {@render foldable(
                `group:${group.id}`,
                group.label,
                `${group.changes.length} ${noun}${group.changes.length === 1 ? "" : "s"} · ${signed(Math.round(group.points))}`,
                groupRows,
              )}
            {:else}
              <p class="meta quiet-row">
                No suggested edits for this master. {others.length
                  ? "Edit any of its lines below, or add your own with A."
                  : "Add your own with A."}
              </p>
            {/each}

            {#if others.length}
              {#snippet otherLines()}
                {#each others as line (lineKey(line))}
                  {@const key = lineKey(line)}
                  {#snippet lineEditor()}{@render wording(() => saveLineEdit(line))}{/snippet}
                  <PartRow
                    label={line.place}
                    selected={key === selectedItem?.key}
                    busy={s.rewriting === key}
                    onselect={() => (selectedKey = key)}
                    editor={editing === key ? lineEditor : undefined}
                  >
                    {#snippet text()}{line.text}{/snippet}
                    {#snippet actions()}
                      <Button variant="ghost" size="sm" icon={Pencil} kbd="⏎" onclick={() => startLineEdit(line)}
                        >Edit</Button
                      >
                      {#if line.rewritable}
                        <Button
                          variant="ghost"
                          size="sm"
                          icon={Sparkles}
                          kbd="N"
                          disabled={s.rewriting !== null}
                          onclick={() => startLineNote(line)}>Rewrite with a note</Button
                        >
                      {/if}
                    {/snippet}
                  </PartRow>
                  {@render noteFor(key, () => void sendLineNote(line))}
                {/each}
              {/snippet}
              {@render foldable(
                "lines",
                "Other lines",
                `${others.length} line${others.length === 1 ? "" : "s"} as on the master`,
                otherLines,
              )}
            {/if}
          </div>

          {#if review.gaps.length || review.knockouts.length}
            <div class="quiet enter" style:--i="3">
              {#if review.gaps.length}
                {#snippet gaps()}
                  <ul>
                    {#each review.gaps as gap (gap.skill)}
                      <li><span class="term">{gap.skill}</span> <span class="meta">{gap.fix}</span></li>
                    {/each}
                  </ul>
                {/snippet}
                {@render foldable(
                  "gaps",
                  "Gaps",
                  `${review.gaps.length} skill${review.gaps.length === 1 ? "" : "s"}`,
                  gaps,
                )}
              {/if}
              {#if review.knockouts.length}
                {#snippet knockouts()}
                  <ul>
                    {#each review.knockouts as knockout, i (i)}<li class="meta">{knockout}</li>{/each}
                  </ul>
                {/snippet}
                {@render foldable("knockouts", "Knockouts", `${review.knockouts.length}`, knockouts)}
              {/if}
            </div>
          {/if}
        {/key}

        <div class="picker-anchor">
          <Button bind:element={addButton} variant="ghost" size="sm" icon={Plus} kbd="A" onclick={openPicker}
            >Add your own</Button
          >
        </div>
        <Popover bind:open={pickerOpen} anchor={addButton} placement="top-start" aria-label="Add your own">
          {#if targets}
            <AddEdit
              {targets}
              hasSummary={others.some((l) => l.line === null) || rows.some((c) => c.op === "summary")}
              onadd={addOwn}
            />
          {/if}
        </Popover>

        <div class="keys-wrap"><KeyFooter keys={KEYS} /></div>
      </div>
    {:else if s.phase === "opening" || s.phase === "suggesting" || s.phase === "stopped"}
      {#if skills.length}
        <div class="skills">
          {#snippet chips()}
            <div class="row chips">
              {#each skills as chip (chip.term)}
                <Chip state={chip.state} p1={chip.priority === 1} disabled>{chip.term}</Chip>
              {/each}
            </div>
          {/snippet}
          {@render foldable("skills", "Skills and keywords", skillsMeta, chips)}
        </div>
      {/if}
      {#if s.view?.masters.length}
        <div class="ranked">
          <p class="label">Your masters, best first</p>
          {#each s.view.masters as master (master.id)}
            <div class="ranked__row">
              <span>{master.title}</span><span class="meta num">{Math.round(master.score)}%</span>
            </div>
          {/each}
        </div>
      {/if}
      {#if s.phase !== "stopped"}
        <div class="changes" aria-hidden="true">
          {#each [0, 1, 2, 3] as i (i)}
            <div class="skeleton">
              <span class="bone bone--box"></span>
              <span class="bone bone--line"></span>
              <span class="bone bone--line bone--short"></span>
            </div>
          {/each}
        </div>
      {/if}
    {/if}
  </div>

  <div class="desk" hidden={shell.compact && pane === "changes"}>
    {#if shell.compact}
      <div class="tabs">
        <Segmented
          options={[
            { value: "changes", label: "Changes" },
            { value: "preview", label: "Preview" },
          ]}
          bind:value={pane}
          label="Show"
        />
      </div>
    {/if}
    <Desk pages={onCover ? (letter?.pages ?? []) : (review?.pages ?? [])} bind:zoom>
      {#snippet status()}
        {#if onCover}
          {#if !letter?.letter}
            <Badge>No letter yet</Badge>
          {:else if letterFits}
            <Badge tone="gain" dot>Fits on 1 page</Badge>
          {:else}
            <Badge tone="warn" dot>{letter.pageCount} pages: over by {letter.pageCount - 1}</Badge>
          {/if}
        {:else if !review}
          <Badge>No preview yet</Badge>
        {:else if fits}
          <Badge tone="gain" dot>Fits on {review.pageLimit} page{review.pageLimit === 1 ? "" : "s"}</Badge>
        {:else}
          <Badge tone="warn" dot>{review.pageCount} pages: over by {review.pageCount - review.pageLimit}</Badge>
        {/if}
      {/snippet}
      {#if onCover}{#if letter?.letter}Dated today{/if}{:else if review}Highlights show what changed{/if}
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
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }
  .job {
    display: grid;
    gap: var(--space-2);
    padding-inline: var(--space-1);
  }
  /*
   * As the sidebar's Recent: the 48px mark spans the company and the title's first line, its edges on the company's
   * cap height (6px into its 24px line) and the title's baseline (2px below the company line, 28.5px into its own).
   * Without a company it centers on the title's first line.
   */
  .named {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    column-gap: var(--space-3);
    row-gap: var(--space-0-5);
    align-items: start;
  }
  .lead {
    display: flex;
    grid-row: span 2;
    margin-top: 6px;
  }
  .named--solo .lead {
    grid-row: auto;
    margin-top: calc((var(--leading-headline) - 48px) / 2);
  }
  .company {
    min-width: 0;
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--text-lg);
    line-height: var(--leading-lg);
    letter-spacing: var(--tracking-lg);
    font-weight: var(--weight-medium);
    color: var(--ink-2);
  }
  .job .meta {
    margin: 0;
  }
  /* At the meta line's end, its text centered on the line's. */
  .tools {
    display: flex;
    gap: var(--space-1);
    margin-block: calc((var(--leading-sm) - var(--control-sm)) / 2);
    margin-inline-start: auto;
  }
  .source {
    color: var(--accent-text);
    text-decoration: none;
  }
  /* Reads as part of the line; a click opens the salary panel. */
  .salary {
    padding: 0;
    border: 0;
    border-radius: var(--radius-xs);
    background: none;
    font: inherit;
    color: var(--ink-2);
    font-variant-numeric: tabular-nums;
    cursor: default;
  }
  @media (hover: hover) {
    .source:hover {
      text-decoration: underline;
    }
    .salary:hover {
      color: var(--ink);
      text-decoration: underline;
    }
  }
  .lede {
    margin: 0;
    font-size: var(--text-md);
    color: var(--ink-2);
    text-wrap: pretty;
  }
  /* Sections share the change groups' inset: heads and rows 4px inside the column's text edge. */
  .skills,
  .changes,
  .quiet {
    display: grid;
    margin-inline: calc(var(--space-2) * -1);
  }
  .fold {
    display: grid;
  }
  .chips {
    padding: var(--space-2) var(--space-3) 0;
  }
  .fold-all {
    display: flex;
    justify-content: flex-end;
    margin-block: calc(var(--space-3) * -1) calc(var(--space-4) * -1);
  }
  .note {
    display: flex;
    gap: var(--space-2);
    align-items: center;
    padding: 0 var(--space-3) var(--space-3) calc(var(--space-3) + 16px + var(--space-3));
  }
  .note > :global(.progress) {
    flex: 1;
  }
  .quiet-row {
    margin: 0;
    padding: var(--space-2) var(--space-3) var(--space-3);
  }
  .problem {
    display: grid;
    gap: var(--space-3);
    padding-inline: var(--space-1);
  }
  .problem p {
    margin: 0;
    color: var(--ink-2);
    text-wrap: pretty;
  }
  .problem p.meta {
    color: var(--ink-3);
  }
  .ranked {
    display: grid;
    gap: var(--space-1);
    padding-inline: var(--space-1);
  }
  .ranked .label {
    padding-bottom: var(--space-1);
  }
  .ranked__row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-4);
    color: var(--ink-2);
  }
  .quiet {
    padding-bottom: var(--space-2);
    color: var(--ink-2);
  }
  .quiet ul {
    display: grid;
    gap: var(--space-1-5);
    margin: 0;
    padding: var(--space-1) var(--space-3) 0;
    list-style: none;
  }
  .quiet .term {
    font-weight: var(--weight-medium);
  }
  .picker-anchor {
    margin-top: calc(var(--space-4) * -1);
  }
  /* The progress stays in view while the stale review under it scrolls. */
  .running {
    position: sticky;
    top: 0;
    z-index: 1;
    margin: calc(var(--space-3) * -1) calc(var(--space-5) * -1);
    padding: var(--space-3) var(--space-5);
    background: var(--paper);
  }
  .review {
    display: contents;
  }
  .review--stale > :global(:not(.keys-wrap)) {
    opacity: 0.5;
  }
  .keys-wrap {
    position: sticky;
    bottom: 0;
    margin-inline: calc(var(--space-5) * -1);
  }

  /* Placeholders while the requirements and suggestions load: static, no shimmer. */
  .bone {
    display: block;
    border-radius: var(--radius-xs);
    background: var(--sunken);
  }
  .bone--meta {
    width: 40%;
    height: var(--space-3);
  }
  .bone--headline {
    width: 70%;
    height: var(--space-8);
  }
  .skeleton {
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr);
    gap: var(--space-2) var(--space-3);
    padding: var(--space-3);
  }
  .bone--box {
    width: 16px;
    height: 16px;
    grid-row: span 2;
  }
  .bone--line {
    height: var(--space-3);
    margin-top: var(--space-1);
  }
  .bone--short {
    width: 60%;
  }
</style>
