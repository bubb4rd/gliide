<!--
  One job: its requirements (Extract), then the review (Suggest, then the fit check on every edit). On the resume tab
  the column is the chat with the tailoring agent (`Chat`); on the cover letter tab, the job header over the letter.
  The desk shows the pages live, under the job's map when the posting names a place.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
  import Banknote from "@lucide/svelte/icons/banknote";
  import Check from "@lucide/svelte/icons/check";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Download from "@lucide/svelte/icons/download";
  import ImagePlus from "@lucide/svelte/icons/image-plus";
  import PenLine from "@lucide/svelte/icons/pen-line";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import { appError, places as findPlaces, setSalary, type Place, type SalaryRange } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import { logo } from "$lib/logo.svelte";
  import { salaryText } from "$lib/salary";
  import { STATUS, STATUS_OPTIONS } from "$lib/status";
  import { app } from "$lib/stores/app.svelte";
  import { shell } from "$lib/stores/shell.svelte";
  import {
    Badge,
    Button,
    Chip,
    CompanyMark,
    Desk,
    GroupHead,
    Icon,
    type KeyCommand,
    LogoDrop,
    Menu,
    PlaceMap,
    Popover,
    Segmented,
    Thinking,
    Toolbar,
    tip,
    toast,
    WorkingRing,
  } from "$lib/ui";
  import Chat from "./Chat.svelte";
  import SalaryEdit from "./SalaryEdit.svelte";
  import Letter from "./Letter.svelte";
  import { sessionFor } from "./session.svelte";

  let { job }: { job: string } = $props();

  // `job` is fixed for this component's life: the route re-creates it per posting.
  // svelte-ignore state_referenced_locally
  const s = sessionFor(job);
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
      s.leave();
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
    instructing: "Working on your request…",
  } as const;
  /** A finished stage in the status's trail. */
  const DONE = {
    extracting: "Read the requirements",
    suggesting: "Suggested edits",
    rewriting: "Rewrote the line",
    writing: "Wrote the cover letter",
    revising: "Rewrote the paragraph",
    importing: "Copied your resume",
    instructing: "Worked on your request",
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
  /** The requirements while the review loads: the chips, as Extract read them. */
  const skills = $derived(s.view?.skills ?? []);
  const fits = $derived(review ? review.pageCount <= review.pageLimit : true);
  let zoom = $state(100);
  let pane = $state<"changes" | "preview">("changes");

  // ── Resume | Cover letter ──
  type Tab = "resume" | "cover";
  let tab = $state<Tab>("resume");
  const onCover = $derived(tab === "cover" && review !== null);
  /** The column is the chat: on the resume tab, once there's a review to talk about. */
  const chatting = $derived(!onCover && review !== null && (s.phase === "ready" || s.phase === "suggesting"));
  const letter = $derived(s.cover && s.cover.review === review?.id ? s.cover : null);
  const letterFits = $derived(letter ? letter.pageCount <= 1 : true);
  /**
   * The first load: Extract, then Suggest, before there's a review (the moment between them included, Extract's step
   * already in the trail). The column wears the ring, its stage centered, no placeholders.
   */
  const loading = $derived(
    review === null &&
      (s.phase === "opening" || s.phase === "suggesting") &&
      (s.running !== null || s.steps.length > 0),
  );
  /** Between Extract and Suggest (Suggest not reported yet), the stage is Suggest's, from that moment. */
  const loadStage = $derived(s.running?.stage ?? "suggesting");
  const loadSince = $derived(s.running?.startedAt ?? performance.now());
  /** A model call writing the column: the first load, or the whole cover letter (`Letter` centers its stage). */
  const working = $derived(loading || (onCover && ready && s.drafting === "letter"));
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

  /** Exported and sent: the application goes from Saved to Applied. */
  function markApplied() {
    if (application?.status === "saved") void app.setStatus([application], "applied");
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

  // ── Where it is: the map over the desk ──
  const where = $derived(head?.location ?? "");
  let places = $state<Place[]>([]);
  /** The place the map is on (null: all of them). */
  let mapAt = $state<number | null>(null);
  $effect(() => {
    const at = where;
    places = [];
    mapAt = null;
    if (!at) return;
    let live = true;
    // Quiet when it fails (offline, Nominatim busy): the desk shows the page alone, as for a remote job.
    findPlaces(at)
      .then((found) => live && (places = found))
      .catch((e) => console.warn("places: couldn't map", at, e));
    return () => {
      live = false;
    };
  });
  /** Opens the place the map is on, else the first, in Maps (Apple Maps on the web where the Maps app isn't). */
  function openMap() {
    const place = places[mapAt ?? 0] ?? places[0];
    if (!place) return false;
    const url = `https://maps.apple.com/?q=${encodeURIComponent(place.name)}&ll=${place.lat},${place.lon}`;
    openUrl(url).catch((e) => toast(appError(e).message, { tone: "danger" }));
  }

  /** The chat's screen commands. Letters type into the composer there, so these run from its menu (or the toolbar). */
  const CHAT_KEYS: KeyCommand[] = $derived([
    { keys: [], label: "Export", run: () => void exportNow(), disabled: !ready },
    { keys: [], label: "Mark applied", run: markApplied, disabled: application?.status !== "saved" },
    { keys: [], label: "Re-run", run: rerun, disabled: !ready },
    { keys: ["⌘Z"], label: "Undo", run: () => void s.undo(), disabled: !review?.canUndo },
    { keys: ["⇧⌘Z"], label: "Redo", run: () => void s.redo(), disabled: !review?.canRedo },
    { keys: [], label: "Salary range", run: openSalary, disabled: !row },
    { keys: [], label: "Open in Maps", run: openMap, disabled: !places.length },
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
    { keys: ["$"], label: "Salary range", run: openSalary, disabled: !row },
    { keys: [], label: "Mark applied", run: markApplied, disabled: application?.status !== "saved" },
    { keys: ["M"], label: "Open in Maps", run: openMap, disabled: !places.length },
    { keys: ["Tab"], label: "Resume", run: switchTab },
  ]);

  // ── Keys ──
  const interactive = (target: EventTarget | null) =>
    target instanceof HTMLElement &&
    (target.isContentEditable || target.closest("button, a, input, textarea, select") !== null);

  // In the chat, letters are typing (the composer takes them): only the letter tab binds them.
  $effect(() =>
    bindKeys({
      Tab: (e) => {
        if (interactive(e.target) || !review) return false;
        switchTab();
      },
      e: () => (onCover ? void exportLetter() : false),
      w: () => (onCover ? writeLetter() : false),
      $: () => (onCover ? openSalary() : false),
      m: () => (onCover ? openMap() : false),
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
        onchange={(status) => app.setStatus([current], status)}
      >
        {#snippet trigger(option)}<Badge tone={STATUS[current.status].tone}>{option?.label}</Badge>{/snippet}
      </Menu>
      {#if current.status === "saved"}
        <Button variant="ghost" size="sm" icon={Check} {@attach tip("Set the status to Applied")} onclick={markApplied}>
          Mark applied
        </Button>
      {/if}
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
    <Button variant="primary" icon={Download} disabled={!ready} onclick={exportNow}>Export</Button>
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
  <div
    class={["column", chatting && "column--chat", working && "column--working"]}
    hidden={shell.compact && pane === "preview"}
  >
    {#if shell.compact}
      <div class="tabs">
        <Segmented
          options={[
            { value: "changes", label: "Chat" },
            { value: "preview", label: "Preview" },
          ]}
          bind:value={pane}
          label="Show"
        />
      </div>
    {/if}

    {#if head && !chatting}
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
    {:else if s.phase === "opening" && !loading}
      <div class="job" aria-hidden="true">
        <span class="bone bone--meta"></span>
        <span class="bone bone--headline"></span>
      </div>
    {/if}
    {#if head}
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
    {/if}

    <!-- The chat shows its own progress, the first load and the letter centered; this one is for the rest. -->
    {#if s.running && !loading && !chatting && !s.instructing && s.drafting === null}
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
    {:else if chatting}
      <Chat
        {s}
        {company}
        {logoSrc}
        {salary}
        keys={CHAT_KEYS}
        bind:salaryAnchor
        bind:logoMark
        onsalary={openSalary}
        canSalary={row !== undefined}
      />
    {:else if s.phase === "opening" || s.phase === "suggesting" || s.phase === "stopped"}
      {#if skills.length}
        <div class="skills">
          <GroupHead label="Skills and keywords" meta="{skills.length} the posting asks for" />
          <div class="row chips">
            {#each skills as chip (chip.term)}
              <Chip state={chip.state} p1={chip.priority === 1} disabled>{chip.term}</Chip>
            {/each}
          </div>
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
      {#if loading}
        <div class="center">
          <Thinking
            center
            steps={s.steps.map((step) => ({ label: DONE[step.stage], ms: step.ms }))}
            label={STAGES[loadStage]}
            startedAt={loadSince}
            oncancel={() => s.cancel()}
          />
        </div>
      {:else if s.phase !== "stopped"}
        <div class="skeletons" aria-hidden="true">
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
  <div class="ring" hidden={shell.compact && pane === "preview"}><WorkingRing on={working} /></div>

  <div class="desk" hidden={shell.compact && pane === "changes"}>
    {#if shell.compact}
      <div class="tabs">
        <Segmented
          options={[
            { value: "changes", label: "Chat" },
            { value: "preview", label: "Preview" },
          ]}
          bind:value={pane}
          label="Show"
        />
      </div>
    {/if}
    <Desk
      pages={onCover ? (letter?.pages ?? []) : (review?.pages ?? [])}
      bind:zoom
      above={places.length ? placeMap : undefined}
    >
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

{#snippet placeMap()}
  <PlaceMap {places} bind:current={mapAt} onopen={openMap} />
{/snippet}

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
  /*
   * The chat scrolls its own thread over the composer. Its hairline is a border, not the inset shadow: the strip and
   * the composer are opaque and would paint over a shadow.
   */
  .column--chat {
    display: flex;
    flex-direction: column;
    padding: 0;
    overflow: hidden;
    border-inline-end: 1px solid var(--line);
    box-shadow: none;
  }
  .column--chat > .tabs {
    padding: var(--space-3) var(--space-5) 0;
  }
  /* The working ring lies over the column's cell, still while the column scrolls under it. */
  .column,
  .ring {
    grid-area: 1 / 1;
  }
  .ring {
    position: relative;
    pointer-events: none;
  }
  /* Written by a model call: the stage centered in the room the column has left. */
  .column--working {
    display: flex;
    flex-direction: column;
  }
  .center {
    flex: 1 0 auto;
    display: grid;
    place-items: center;
    padding-block: var(--space-12);
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
  /* Heads and rows 4px inside the column's text edge. */
  .skills,
  .skeletons {
    display: grid;
    margin-inline: calc(var(--space-2) * -1);
  }
  .chips {
    padding: var(--space-2) var(--space-3) 0;
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
  /* The progress stays in view while the column scrolls. */
  .running {
    position: sticky;
    top: 0;
    z-index: 1;
    margin: calc(var(--space-3) * -1) calc(var(--space-5) * -1);
    padding: var(--space-3) var(--space-5);
    background: var(--paper);
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
