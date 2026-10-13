<!--
  One job: its requirements (Extract), then the review (Suggest, then the fit check on every edit). The column holds
  the job header, the score, the skill chips and the change groups; the desk shows the tailored pages live.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Download from "@lucide/svelte/icons/download";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Plus from "@lucide/svelte/icons/plus";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import X from "@lucide/svelte/icons/x";
  import { appError, editTargets, type ChangeView, type CustomEdit, type EditTargets, type Mode } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import { app } from "$lib/stores/app.svelte";
  import { shell } from "$lib/stores/shell.svelte";
  import {
    Badge,
    Button,
    ChangeRow,
    Chip,
    Desk,
    GroupHead,
    Icon,
    Input,
    KeyFooter,
    Marked,
    Menu,
    Popover,
    Progress,
    ScoreHeader,
    Segmented,
    Textarea,
    Toolbar,
    toast,
  } from "$lib/ui";
  import AddEdit from "./AddEdit.svelte";
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
    return () => {
      s.dispose();
      void app.refreshJobs();
    };
  });

  const MODES: { value: Mode; label: string }[] = [
    { value: "master-only", label: "Master only" },
    { value: "bank-aware", label: "Bank-aware" },
  ];
  const STAGES = {
    extracting: "Reading the requirements…",
    suggesting: "Suggesting edits…",
    rewriting: "Rewriting the line…",
  } as const;
  const VERDICTS = {
    apply: { label: "Apply as is", tone: "accent" },
    tailor: { label: "Tailor first", tone: "accent" },
    stretch: { label: "Stretch", tone: "warn" },
    skip: { label: "Skip", tone: "neutral" },
  } as const;
  const KEYS = [
    { keys: ["↑", "↓"], label: "Select" },
    { keys: ["Space"], label: "Toggle" },
    { keys: ["⏎"], label: "Edit" },
    { keys: ["A"], label: "Add your own" },
    { keys: ["E"], label: "Export" },
    { keys: ["R"], label: "Re-run" },
    { keys: ["⌘Z"], label: "Undo" },
    { keys: ["⇧⌘Z"], label: "Redo" },
  ];

  const review = $derived(s.review);
  const ready = $derived(s.phase === "ready" && review !== null);
  const head = $derived(review?.head ?? s.view?.head ?? null);
  const row = $derived(app.jobs.find((j) => j.id === job));
  const company = $derived(head?.company || row?.company || "Posting");
  const metaLine = $derived(
    head ? [head.company, head.location, head.url ? null : head.source].filter(Boolean).join(" · ") : "",
  );
  const masters = $derived(
    (review?.masters ?? s.view?.masters ?? []).map((m) => ({
      value: m.id,
      label: m.title,
      meta: `${Math.round(m.score)}%`,
    })),
  );
  const skills = $derived(review?.skills ?? s.view?.skills ?? []);
  const rows = $derived(review?.groups.flatMap((g) => g.changes) ?? []);
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

  // ── Selection and editing ──
  let selectedKey = $state<string | null>(null);
  const selectedIndex = $derived(Math.max(0, rows.findIndex((c) => c.key === selectedKey)));
  const selected = $derived(rows[selectedIndex]);
  let editing = $state<string | null>(null);
  let draft = $state("");
  let noting = $state<string | null>(null);
  let note = $state("");
  let zoom = $state(100);
  let pane = $state<"changes" | "preview">("changes");

  // A new review (another master, mode, or a re-run) starts with nothing open.
  $effect(() => {
    void s.arrival;
    editing = null;
    noting = null;
  });

  const focus = (element: HTMLElement) => element.focus();

  function move(by: number) {
    const next = rows[Math.min(Math.max(selectedIndex + by, 0), rows.length - 1)];
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
    if (!done) return;
    if (done.flags.length) toast(`Check the new wording: ${done.flags.join("; ")}`, { tone: "warn" });
    else toast("Rewrote the line from your note");
  }

  function checkboxLabel(change: ChangeView) {
    if (change.custom) return "Include your edit";
    if (change.op.startsWith("skill_")) return "Include this skill change";
    if (["include", "exclude", "swap_entry"].includes(change.op)) return "Include this entry change";
    if (change.editable) return "Include this rewrite";
    return "Include this change";
  }

  // ── Toolbar ──
  function chooseMode(mode: Mode) {
    s.mode = mode;
    void s.suggest(s.master, mode);
  }

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
    const ok = await s.add(edit);
    if (ok) pickerOpen = false;
    return ok;
  }

  // ── Keys ──
  const interactive = (target: EventTarget | null) =>
    target instanceof Element && target.closest("button, a, input, textarea, select") !== null;

  $effect(() =>
    bindKeys({
      ArrowDown: () => move(1),
      ArrowUp: () => move(-1),
      " ": (e) => {
        if (interactive(e.target)) return false;
        if (selected) void s.toggle(selected.key);
      },
      Enter: (e) => {
        if (interactive(e.target)) return false;
        startEdit(selected);
      },
      a: () => void openPicker(),
      e: () => void exportNow(),
      r: rerun,
      "mod+z": (e) => {
        if (interactive(e.target)) return false;
        void s.undo();
      },
      "shift+mod+z": (e) => {
        if (interactive(e.target)) return false;
        void s.redo();
      },
    }),
  );
</script>

<Toolbar>
  {#snippet crumbs()}
    <a href="/jobs">Jobs</a><Icon icon={ChevronRight} size={14} weight="medium" /><b>{company}</b>
  {/snippet}
  <Segmented
    options={MODES}
    value={s.mode ?? app.settings?.defaultMode ?? "bank-aware"}
    label="Suggestions from"
    disabled={!ready}
    onchange={chooseMode}
  />
  <hr />
  {#if masters.length}
    <Menu
      value={s.master ?? ""}
      options={masters}
      label="Master"
      placement="bottom-end"
      disabled={!ready}
      onchange={chooseMaster}
    />
  {/if}
  <Button variant="primary" icon={Download} kbd="E" disabled={!ready} onclick={exportNow}>Export</Button>
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
            {metaLine}{#if head.url}{metaLine ? " · " : ""}<a
                class="source"
                href={head.url}
                onclick={(e) => {
                  e.preventDefault();
                  if (head.url) void openUrl(head.url);
                }}>{head.source}</a
              >{/if}
          </span>
        </div>
        <h1 class="headline">{head.role}</h1>
        {#if review?.headline}<p class="lede">{review.headline}</p>{/if}
      </div>
    {:else if s.phase === "opening"}
      <div class="job" aria-hidden="true">
        <span class="bone bone--meta"></span>
        <span class="bone bone--headline"></span>
      </div>
    {/if}

    {#if s.running && !s.rewriting}
      <div class="running">
        <Progress
          label={STAGES[s.running.stage]}
          expectedMs={s.running.expectedMs}
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
            <Button variant="primary" onclick={() => goto("/bank")}>Open Experience bank</Button>
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

    {#if review && (s.phase === "ready" || s.phase === "suggesting")}
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

          {#if skills.length}
            <div class="skills enter" style:--i="1">
              <p class="label">Skills the posting asks for</p>
              <div class="row">
                {#each skills as chip (chip.term)}
                  <Chip
                    state={chip.state}
                    p1={chip.priority === 1}
                    title={chip.locked ?? undefined}
                    onclick={() => (chip.locked ? toast(chip.locked, { tone: "neutral" }) : s.skill(chip.term))}
                    >{chip.term}</Chip
                  >
                {/each}
              </div>
            </div>
          {/if}

          <div class="changes enter" style:--i="2">
            {#each review.groups as group (group.id)}
              {@const noun = group.id === "bank" ? "suggestion" : "change"}
              <GroupHead
                label={group.label}
                meta="{group.changes.length} {noun}{group.changes.length === 1 ? '' : 's'} · {signed(Math.round(group.points))}"
              />
              {#each group.changes as change (change.key)}
                {#snippet editor()}
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
                        saveEdit(change);
                      }
                    }}
                  />
                {/snippet}
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
                  {#snippet text()}<span title={change.why || undefined}
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
                        disabled={s.rewriting !== null}
                        onclick={() => startNote(change)}>Rewrite with a note</Button
                      >
                    {/if}
                    {#if change.yours}
                      <Button
                        variant="ghost"
                        size="sm"
                        square
                        icon={change.custom ? X : RotateCcw}
                        aria-label={change.custom ? "Remove your edit" : "Back to the suggested wording"}
                        title={change.custom ? "Remove your edit" : "Back to the suggested wording"}
                        onclick={() => s.remove(change)}
                      />
                    {/if}
                  {/snippet}
                </ChangeRow>
                {#if noting === change.key}
                  <form
                    class="note"
                    onsubmit={(e) => {
                      e.preventDefault();
                      void sendNote(change);
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
                {:else if s.rewriting === change.key && s.running}
                  <div class="note">
                    <Progress
                      label={STAGES.rewriting}
                      expectedMs={s.running.expectedMs}
                      startedAt={s.running.startedAt}
                      oncancel={() => s.cancel()}
                    />
                  </div>
                {/if}
              {:else}
                <p class="meta quiet-row">No changes in this group.</p>
              {/each}
            {:else}
              <p class="meta quiet-row">No suggested edits for this master. Add your own with A.</p>
            {/each}
          </div>

          {#if review.gaps.length || review.knockouts.length}
            <div class="quiet enter" style:--i="3">
              {#if review.gaps.length}
                <section>
                  <p class="label">Gaps your resume can’t back</p>
                  <ul>
                    {#each review.gaps as gap (gap.skill)}
                      <li><span class="term">{gap.skill}</span> <span class="meta">{gap.fix}</span></li>
                    {/each}
                  </ul>
                </section>
              {/if}
              {#if review.knockouts.length}
                <section>
                  <p class="label">Knockouts</p>
                  <ul>
                    {#each review.knockouts as knockout, i (i)}<li class="meta">{knockout}</li>{/each}
                  </ul>
                </section>
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
          {#if targets}<AddEdit {targets} onadd={addOwn} />{/if}
        </Popover>

        <div class="keys-wrap"><KeyFooter keys={KEYS} /></div>
      </div>
    {:else if s.phase === "opening" || s.phase === "suggesting" || s.phase === "stopped"}
      {#if skills.length}
        <div class="skills">
          <p class="label">Skills the posting asks for</p>
          <div class="row">
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
    <Desk pages={review?.pages ?? []} bind:zoom>
      {#snippet status()}
        {#if !review}
          <Badge>No preview yet</Badge>
        {:else if fits}
          <Badge tone="gain" dot>Fits on {review.pageLimit} page{review.pageLimit === 1 ? "" : "s"}</Badge>
        {:else}
          <Badge tone="warn" dot>{review.pageCount} pages: over by {review.pageCount - review.pageLimit}</Badge>
        {/if}
      {/snippet}
      {#if review}Highlights show what changed{/if}
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
  .job .meta {
    margin: 0;
  }
  .source {
    color: var(--accent-text);
    text-decoration: none;
  }
  @media (hover: hover) {
    .source:hover {
      text-decoration: underline;
    }
  }
  .lede {
    margin: 0;
    font-size: var(--text-md);
    color: var(--ink-2);
    text-wrap: pretty;
  }
  .skills {
    display: grid;
    gap: var(--space-3);
    padding-inline: var(--space-1);
  }
  .changes {
    display: grid;
    margin-inline: calc(var(--space-2) * -1);
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
    display: grid;
    gap: var(--space-4);
    padding: 0 var(--space-1) var(--space-2);
    color: var(--ink-2);
  }
  .quiet section {
    display: grid;
    gap: var(--space-2);
  }
  .quiet ul {
    display: grid;
    gap: var(--space-1-5);
    margin: 0;
    padding: 0;
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
