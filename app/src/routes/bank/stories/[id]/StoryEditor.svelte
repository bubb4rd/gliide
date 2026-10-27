<!--
  A story's editor (DESIGN.md, Story editor), the entry editor's shape: the title as the serif heading, edited in place;
  Situation, Task, Action and Result as rows, each saying where its words came from; the entry it happened in (a menu,
  or off the resume) with the bullets and facts it draws on; its themes as toggles; where it was used. Every edit
  saves at once and the story comes back; a new one (`new`) is saved on its first edit. The desk shows the entry in a
  master that shows it, highlighted. Tell it (`P`) turns the column into a small practice thread for this story
  alone: one question from its themes, your telling, the feedback card. Delete (`⌘⌫`) goes back to Experience with
  Undo.
-->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import { goto, replaceState } from "$app/navigation";
  import Archive from "@lucide/svelte/icons/archive";
  import ArrowRight from "@lucide/svelte/icons/arrow-right";
  import ArrowUp from "@lucide/svelte/icons/arrow-up";
  import Check from "@lucide/svelte/icons/check";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import FileText from "@lucide/svelte/icons/file-text";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Play from "@lucide/svelte/icons/play";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import UserRound from "@lucide/svelte/icons/user-round";
  import {
    appError,
    cancel,
    entry as loadEntry,
    keepClaim,
    removeStory,
    saveStory,
    story as loadStory,
    storyTurns,
    tellStory,
    type EntryView,
    type NewClaim,
    type PracticeTurn,
    type StoryDraft,
    type StoryTheme,
    type StoryView,
  } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import { app } from "$lib/stores/app.svelte";
  import { shell } from "$lib/stores/shell.svelte";
  import { askFor, tellTime, THEMES } from "$lib/stories";
  import {
    Badge,
    Button,
    Card,
    Chip,
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
    Thinking,
    Toolbar,
    tip,
    toast,
  } from "$lib/ui";
  import FeedbackCard from "../../../jobs/[id]/FeedbackCard.svelte";

  /** `id` "new": a blank story, happening in `entry` when given. */
  let { id, entry: startEntry = null }: { id: string; entry?: string | null } = $props();

  const PANES = [
    { value: "story", label: "Story" },
    { value: "preview", label: "Preview" },
  ] as const;
  /** The entry menu's choice for a story off the resume. */
  const OFF = "";

  let view = $state<StoryView | null>(null);
  let failed = $state<string | null>(null);
  let busy = $state(false);
  /** Bumped after every answer, so the menus show the view's choice again after one that failed. */
  let round = $state(0);
  let zoom = $state(100);
  let pane = $state<"story" | "preview">("story");
  /** The story's entry as the desk shows it; null off the resume or while it loads. */
  let desk = $state<EntryView | null>(null);
  let alive = true;

  const saved = $derived(view !== null && view.id !== "");
  const href = (story: string) => `/bank/stories/${encodeURIComponent(story)}`;

  /** The bank's entries (not skills lines), for the entry menu and the entry's role. */
  const entries = $derived(
    app.library?.sections.flatMap((s) =>
      s.kind === "skills" ? [] : s.entries.map((e) => ({ ...e, section: s.title })),
    ) ?? [],
  );
  const entryOptions = $derived([
    ...entries.map((e) => ({ value: e.id, label: e.name, meta: e.section })),
    { value: OFF, label: "Off the resume" },
  ]);
  const entryRow = $derived(view?.entry ? entries.find((e) => e.id === view?.entry) : undefined);

  /** A blank story, as `new` opens it. */
  function blank(entry: string | null): StoryView {
    return {
      id: "",
      title: "",
      entry,
      entryName: entry ? (entries.find((e) => e.id === entry)?.name ?? null) : null,
      situation: "",
      task: "",
      action: "",
      result: "",
      skills: [],
      themes: [],
      links: [],
      source: "edit",
      updatedAt: "",
      seconds: 0,
      used: [],
    };
  }

  onMount(() => {
    void open();
    return () => {
      alive = false;
    };
  });

  async function open() {
    failed = null;
    try {
      if (!app.library) await app.refreshLibrary();
      const v = id === "new" ? blank(startEntry) : await loadStory(id);
      if (!alive) return;
      view = v;
      void showDesk(v.entry, null);
      if (v.id) void loadTurns(v.id);
    } catch (e) {
      if (alive) failed = appError(e).message;
    }
  }

  /** The desk: the entry in `preview` (null: the first master that shows it). */
  async function showDesk(entry: string | null, preview: string | null) {
    if (!entry) return void (desk = null);
    try {
      const v = await loadEntry(entry, preview);
      if (alive && view?.entry === entry) desk = v;
    } catch {
      // A removed entry: the story stays usable, its desk empty.
      if (alive && view?.entry === entry) desk = null;
    }
  }

  /** The story as `saveStory` takes it: its links as bullets and facts, its source kept; typed words are yours. */
  function draftOf(v: StoryView): StoryDraft {
    return {
      id: v.id || null,
      title: v.title,
      entry: v.entry,
      bullets: v.links.filter((l) => !l.fact).map((l) => l.id),
      facts: v.links.filter((l) => l.fact).map((l) => l.id),
      situation: v.situation,
      task: v.task,
      action: v.action,
      result: v.result,
      skills: v.skills,
      themes: v.themes,
      source: v.id ? v.source : null,
      from: null,
    };
  }

  /** Saves the story with `change` (one request at a time); its answer becomes the view. A new story's first save
   *  gives it its address. */
  async function save(change: Partial<StoryDraft>): Promise<boolean> {
    if (busy || !view) return false;
    busy = true;
    try {
      const was = view;
      const v = await saveStory({ ...draftOf(was), ...change });
      if (!alive) return true;
      view = v;
      if (!was.id) replaceState(href(v.id), {});
      if (v.entry !== desk?.id) void showDesk(v.entry, null);
      return true;
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
      return false;
    } finally {
      busy = false;
      round++;
    }
  }

  // ── Rows: the title, then situation, task, action and result ──
  type Part = "situation" | "task" | "action" | "result";
  type Field = "title" | Part;
  const PARTS: { key: Part; label: string; placeholder: string }[] = [
    { key: "situation", label: "Situation", placeholder: "Where you were and what was going on" },
    { key: "task", label: "Task", placeholder: "What you had to do" },
    { key: "action", label: "Action", placeholder: "What you did, in your words" },
    { key: "result", label: "Result", placeholder: "What changed, with a number if you have one" },
  ];
  const ROWS: Field[] = ["title", ...PARTS.map((p) => p.key)];
  let selectedKey = $state<Field>("title");
  let editing = $state<Field | null>(null);
  let draft = $state("");

  /** Where a part's words came from, after its name. */
  function from(v: StoryView, part: Part): string {
    if (!v.id || !v[part]) return "";
    if (v.source === "practice") return "From your answer · practice";
    if (v.source === "dig") return "From Dig";
    return "Typed by you";
  }

  const focus = (element: HTMLElement) => element.focus();

  function move(by: number) {
    const at = ROWS.indexOf(selectedKey);
    selectedKey = ROWS[Math.min(Math.max(at + by, 0), ROWS.length - 1)];
  }

  function startEdit(field: Field) {
    if (!view) return;
    selectedKey = field;
    editing = field;
    draft = view[field];
  }

  async function commit() {
    const field = editing;
    if (!field || !view) return;
    const text = draft.trim();
    if (text === view[field]) return void (editing = null);
    if (!(await save({ [field]: text }))) return;
    if (editing === field) editing = null;
  }

  function onEditorKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      editing = null;
    } else if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void commit();
    }
  }

  // ── Links, themes ──
  const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
  const bulletLinks = $derived(view?.links.filter((l) => !l.fact) ?? []);
  const factLinks = $derived(view?.links.filter((l) => l.fact) ?? []);
  const linksMeta = $derived(
    [
      view?.entry ? "1 entry" : "Off the resume",
      bulletLinks.length ? plural(bulletLinks.length, "bullet", "bullets") : "",
      factLinks.length ? plural(factLinks.length, "fact", "facts") : "",
    ]
      .filter(Boolean)
      .join(" · "),
  );
  /** "Bullet 2": where a linked bullet sits in its entry, as the desk reads it. */
  function bulletName(link: string): string {
    const at = desk?.bullets.findIndex((b) => b.id === link) ?? -1;
    return at >= 0 ? `Bullet ${at + 1}` : "Bullet";
  }

  /** Moves the story to another entry (or off the resume). Its bullets and facts were the old entry's, so they go;
   *  the toast's Undo brings the story back as it was. */
  async function setEntry(entry: string) {
    if (!view) return;
    const before = view;
    const next = entry === OFF ? null : entry;
    if (next === before.entry) return;
    const dropped = before.links.length > 0;
    if (!saved) {
      // Nothing saved yet: the entry is part of the blank story until its first edit.
      view = { ...before, entry: next, entryName: entries.find((e) => e.id === next)?.name ?? null, links: [] };
      void showDesk(next, null);
      return;
    }
    if (!(await save({ entry: next, bullets: [], facts: [] }))) return;
    const name = next ? (entries.find((e) => e.id === next)?.name ?? "the entry") : null;
    toast(name ? `Moved the story to ${name}` : "The story is off the resume now", {
      ...(dropped
        ? {
            action: {
              label: "Undo",
              run: async () => {
                await save(draftOf(before));
              },
            },
          }
        : {}),
    });
  }

  function toggleTheme(theme: StoryTheme) {
    if (!view) return;
    const themes = view.themes.includes(theme)
      ? view.themes.filter((t) => t !== theme)
      : THEMES.map((t) => t.value).filter((t) => t === theme || view?.themes.includes(t));
    if (!saved) return void (view = { ...view, themes });
    void save({ themes });
  }

  function usedOn(at: string): string {
    const d = new Date(at);
    return Number.isNaN(d.getTime()) ? "" : d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
  }

  // ── Delete ──
  /** Back to Experience, the story deleted; the toast's Undo saves it again under its id and opens it. */
  async function remove() {
    const was = view;
    if (!was) return;
    if (!was.id) return void goto("/bank");
    try {
      await removeStory(was.id);
    } catch (e) {
      return void toast(appError(e).message, { tone: "danger" });
    }
    await goto("/bank");
    toast("Deleted the story", {
      action: {
        label: "Undo",
        run: async () => {
          try {
            const back = await saveStory(draftOf(was));
            await goto(href(back.id));
          } catch (e) {
            toast(appError(e).message, { tone: "danger" });
          }
        },
      },
    });
  }

  // ── Tell it ──
  let telling = $state(false);
  let turns = $state<PracticeTurn[]>([]);
  let answer = $state("");
  let reading = $state<number | null>(null);
  /** You asked again: the question waits for a new telling though the last turn answered it. */
  let again = $state(false);
  let kept = $state(new Set<string>());
  let field = $state<Textarea>();
  let thread = $state<HTMLElement>();
  const asked = $derived(view ? askFor(view) : "");
  const answered = $derived(turns.length > 0 && !again);
  const canTell = $derived(saved && !!view && [view.situation, view.task, view.action, view.result].some(Boolean));

  async function loadTurns(story: string) {
    try {
      const t = await storyTurns(story);
      if (alive) turns = t;
    } catch {
      // No tellings to show: Tell it starts fresh.
    }
  }

  async function startTelling() {
    if (!canTell) return;
    editing = null;
    telling = true;
    pane = "story";
    await tick();
    thread?.scrollTo({ top: thread.scrollHeight });
    field?.focus();
  }

  async function tellAgain() {
    if (reading !== null) return;
    again = true;
    answer = "";
    await tick();
    thread?.scrollTo({ top: thread.scrollHeight });
    field?.focus();
  }

  async function send() {
    const text = answer.trim();
    if (!text || !view?.id || reading !== null) return;
    const story = view.id;
    reading = performance.now();
    try {
      const turn = await tellStory(story, asked, text);
      if (!alive) return;
      turns = [...turns, turn];
      answer = "";
      again = false;
      (document.activeElement as HTMLElement | null)?.blur();
      await tick();
      thread?.scrollTo({ top: thread.scrollHeight });
      // Telling it counts as using it.
      const v = await loadStory(story);
      if (alive) view = v;
    } catch (e) {
      const error = appError(e);
      if (error.kind !== "cancelled") toast(error.message, { tone: "danger" });
    } finally {
      reading = null;
    }
  }

  async function keep(claim: NewClaim) {
    try {
      await keepClaim(null, claim.entry, claim.text);
      kept = new Set([...kept, claim.text]);
      toast(`Kept as a fact of ${claim.entryName}`);
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
    }
  }

  /** A line a telling drew on: a bullet of the story's entry, else one of its links. */
  function line(lineId: string): { text: string; entry: string | null } | undefined {
    const bullet = desk?.bullets.find((b) => b.id === lineId);
    if (bullet) return { text: bullet.wordings[0]?.text ?? "", entry: desk?.details.name ?? null };
    const link = view?.links.find((l) => l.id === lineId && !l.gone);
    return link ? { text: link.text, entry: view?.entryName ?? null } : undefined;
  }

  function onFieldKey(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void send();
    } else if (e.key === "Escape") {
      e.stopPropagation();
      (e.currentTarget as HTMLElement).blur();
    }
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

  const TELL_KEYS: KeyCommand[] = $derived([
    { keys: ["⏎"], label: "Send your telling", run: () => void send(), disabled: !answer.trim() || reading !== null },
    { keys: ["R"], label: "Tell it again", run: () => void tellAgain(), disabled: reading !== null },
    { keys: ["Esc"], label: "Back to the story", run: () => (telling = false) },
    { keys: ["⌘⌫"], label: "Delete story", run: () => void remove() },
  ]);
  const KEYS: KeyCommand[] = $derived([
    { keys: ["↑", "↓"], label: "Select" },
    { keys: ["⏎"], label: "Edit", run: () => startEdit(selectedKey), disabled: !view },
    { keys: ["P"], label: "Tell it", run: () => void startTelling(), disabled: !canTell },
    { keys: ["E"], label: "Happened in…", run: () => openMenu("entry"), disabled: !view },
    { keys: ["⌘⌫"], label: "Delete story", run: () => void remove(), disabled: !view },
    { keys: ["Esc"], label: "Cancel" },
  ]);

  $effect(() =>
    bindKeys(
      telling
        ? {
            r: (e) => (interactive(e.target) ? false : void tellAgain()),
            Escape: (e) => {
              if (reading !== null && view) return void cancel(`story:${view.id}`);
              if (interactive(e.target)) return false;
              telling = false;
            },
            "mod+Backspace": () => void remove(),
          }
        : {
            ArrowDown: () => move(1),
            ArrowUp: () => move(-1),
            Enter: (e) => {
              if (interactive(e.target)) return false;
              startEdit(selectedKey);
            },
            p: () => void startTelling(),
            e: () => openMenu("entry"),
            "mod+Backspace": (e) => {
              if (e.target instanceof Element && e.target.closest("input, textarea")) return false;
              void remove();
            },
          },
    ),
  );
</script>

<Toolbar>
  {#snippet crumbs()}
    <a href="/bank">Experience</a><Icon icon={ChevronRight} size={14} weight="medium" /><a href="/bank#section-stories"
      >Stories</a
    ><Icon icon={ChevronRight} size={14} weight="medium" /><b>{view?.title || (saved ? "Untitled story" : "New story")}</b>
  {/snippet}
  {#if telling}
    <Button variant="ghost" kbd="Esc" onclick={() => (telling = false)}>Back to the story</Button>
  {:else}
    <Button
      variant="primary"
      icon={Play}
      kbd="P"
      disabled={!canTell}
      {@attach tip(canTell ? "" : "Write what happened first")}
      onclick={startTelling}>Tell it</Button
    >
  {/if}
  <hr />
  <Button
    variant="ghost"
    square
    icon={Trash2}
    aria-label="Delete story"
    {@attach tip("Delete story (⌘⌫)")}
    disabled={!view}
    onclick={remove}
  />
</Toolbar>

<div class={["split", shell.compact && "split--compact"]}>
  <div
    class={["column", telling && "column--tell"]}
    hidden={shell.compact && pane === "preview"}
    aria-busy={busy || undefined}
  >
    {#if shell.compact}
      <div class="tabs"><Segmented options={[...PANES]} bind:value={pane} label="Show" /></div>
    {/if}

    {#if failed}
      <div class="center">
        <Empty title="Couldn’t open this story" body={failed}>
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
    {:else if telling}
      {@const story = view}
      <div class="thread" bind:this={thread}>
        {#each turns as turn (turn.id)}
          <article class="turn">
            <p class="who"><Icon icon={UserRound} size={16} /><span class="who__name">Interviewer</span></p>
            <p class="asked">{turn.asked}</p>
          </article>
          <p class="bubble">{turn.answer}</p>
          <FeedbackCard
            feedback={turn.feedback}
            {line}
            {kept}
            onkeep={keep}
            grounded="Only your telling, this story and its entry"
            against="its entry"
          />
        {/each}
        {#if !answered}
          <article class="turn">
            <p class="who">
              <Icon icon={UserRound} size={16} /><span class="who__name">Interviewer</span><span class="meta"
                >Telling “{story.title || "this story"}” · {tellTime(story.seconds)}</span
              >
            </p>
            <p class="asked">{asked}</p>
          </article>
        {/if}
        {#if reading !== null}
          <div class="reading">
            <Thinking
              label="Reading your telling…"
              startedAt={reading}
              oncancel={() => void cancel(`story:${story.id}`)}
            />
          </div>
        {/if}
      </div>
      <div class="foot">
        <div class="foot__row">
          <span class="meta next">{answered ? "Tell it again with R" : "Tell it as you would out loud"}</span>
          <Button
            variant={answered ? "primary" : "ghost"}
            size="sm"
            icon={RotateCcw}
            kbd="R"
            disabled={reading !== null}
            onclick={tellAgain}>Again</Button
          >
        </div>
        <div class="composer">
          <Textarea
            bind:this={field}
            bind:value={answer}
            placeholder={answered ? "Tell it again, or go back with Esc" : "Your telling…"}
            rows={3}
            aria-label="Your telling"
            disabled={reading !== null}
            onkeydown={onFieldKey}
          />
          <div class="composer__bar">
            <KeyFooter keys={TELL_KEYS} bare />
            <Button
              variant="primary"
              size="sm"
              square
              icon={ArrowUp}
              aria-label="Send your telling"
              {@attach tip("Send your telling (⏎)")}
              disabled={!answer.trim() || reading !== null}
              onclick={() => void send()}
            />
          </div>
        </div>
      </div>
    {:else}
      {@const story = view}
      <div class="parts">
        {#snippet titleEditor()}
          <Input
            bind:value={draft}
            placeholder="What it’s about, like “Caught 14 regressions before release”"
            aria-label="Title"
            {@attach focus}
            onkeydown={onEditorKey}
          />
        {/snippet}
        <div class="title-row">
          <PartRow
            label={["Story", story.entryName ?? "Off the resume", story.seconds ? `${tellTime(story.seconds)} to tell` : ""]
              .filter(Boolean)
              .join(" · ")}
            selected={selectedKey === "title"}
            onselect={() => (selectedKey = "title")}
            editor={editing === "title" ? titleEditor : undefined}
          >
            {#snippet text()}
              {#if story.title}<h1 class="title">{story.title}</h1>{:else}<h1 class="title none">Untitled story</h1>{/if}
            {/snippet}
            {#snippet actions()}
              <Button variant="ghost" size="sm" icon={Pencil} kbd="⏎" onclick={() => startEdit("title")}>Edit</Button>
            {/snippet}
          </PartRow>
        </div>

        <section class="group" aria-label="Story">
          <GroupHead label="Story" meta="Never printed" />
          {#each PARTS as part (part.key)}
            {@const where = from(story, part.key)}
            {#snippet editor()}
              <Textarea
                size="lg"
                bind:value={draft}
                placeholder={part.placeholder}
                aria-label={part.label}
                {@attach focus}
                onkeydown={onEditorKey}
              />
            {/snippet}
            <PartRow
              label={where ? `${part.label} · ${where}` : part.label}
              selected={selectedKey === part.key}
              onselect={() => (selectedKey = part.key)}
              editor={editing === part.key ? editor : undefined}
            >
              {#snippet text()}
                {#if story[part.key]}<p class="words">{story[part.key]}</p>{:else}<p class="words none">
                    {part.placeholder}
                  </p>{/if}
              {/snippet}
              {#snippet actions()}
                <Button variant="ghost" size="sm" icon={Pencil} kbd="⏎" onclick={() => startEdit(part.key)}>Edit</Button>
              {/snippet}
            </PartRow>
          {/each}
        </section>

        <section class="group" aria-label="Links">
          <GroupHead label="Links" meta={linksMeta} />
          <div class="block">
            {#if story.entry || story.links.length}
              <Card>
                <ul class="list">
                  {#if story.entry}
                    <li>
                      <a class="link link--open" href="/bank/entries/{encodeURIComponent(story.entry)}">
                        <Icon icon={Archive} size={16} />
                        <span class="link__text">
                          {#if story.entryName}<b>{story.entryName}</b>{#if entryRow?.subtitle}<span class="meta"
                                >{" · "}{entryRow.subtitle}</span
                              >{/if}{:else}<span class="gone">Entry removed</span>{/if}
                        </span>
                        <Icon icon={ArrowRight} size={14} />
                      </a>
                    </li>
                  {/if}
                  {#each bulletLinks as link (link.id)}
                    <li class="link">
                      <Icon icon={FileText} size={16} />
                      <span class="link__text" {@attach tip(link.gone ? "" : link.text)}>
                        {#if link.gone}<span class="gone">Bullet removed</span>{:else}<b>{bulletName(link.id)}</b
                          ><span class="meta">{" · "}{link.text}</span>{/if}
                      </span>
                    </li>
                  {/each}
                  {#each factLinks as link (link.id)}
                    <li class="link">
                      <Icon icon={Check} size={16} />
                      <span class="link__text" {@attach tip(link.gone ? "" : link.text)}>
                        {#if link.gone}<span class="gone">Fact removed</span>{:else}<b>Fact</b><span class="meta"
                            >{" · "}{link.text}</span
                          >{/if}
                      </span>
                    </li>
                  {/each}
                </ul>
              </Card>
            {/if}
            <div class="happened">
              <span class="meta">Happened in</span>
              {#key round}
                <span class="menu" data-menu="entry">
                  <Menu
                    value={story.entry ?? OFF}
                    options={entryOptions}
                    label="Happened in"
                    size="sm"
                    variant="ghost"
                    kbd="E"
                    disabled={busy}
                    onchange={setEntry}
                  />
                </span>
              {/key}
            </div>
          </div>
        </section>

        <section class="group" aria-label="Themes">
          <GroupHead label="Themes" meta={story.themes.length ? "" : "What questions it answers"} />
          <div class="block chips">
            {#each THEMES as theme (theme.value)}
              <Chip
                state={story.themes.includes(theme.value) ? "on" : "off"}
                disabled={busy}
                {@attach tip(theme.ask)}
                onclick={() => toggleTheme(theme.value)}>{theme.label}</Chip
              >
            {/each}
          </div>
        </section>

        <section class="group" aria-label="Used">
          <GroupHead label="Used" meta={story.used.length ? String(story.used.length) : ""} />
          <div class="block">
            {#if story.used.length}
              <Card>
                <ul class="list">
                  {#each story.used as use, i (i)}
                    <li class="link">
                      <Icon icon={Play} size={16} />
                      <span class="link__text num"
                        ><b>{use.label}</b>{#if usedOn(use.at)}<span class="meta">{" · "}{usedOn(use.at)}</span>{/if}</span
                      >
                    </li>
                  {/each}
                </ul>
              </Card>
            {:else}
              <p class="meta empty">Not practiced yet. Tell it, or practice a round whose prep calls for it.</p>
            {/if}
          </div>
        </section>
      </div>

      <div class="keys-wrap"><KeyFooter keys={KEYS} /></div>
    {/if}
  </div>

  <div class="desk" hidden={shell.compact && pane === "story"}>
    {#if shell.compact}
      <div class="tabs"><Segmented options={[...PANES]} bind:value={pane} label="Show" /></div>
    {/if}
    <Desk pages={desk?.pages ?? []} bind:zoom>
      {#snippet status()}
        {#if !view?.entry}
          <Badge>Off the resume</Badge>
        {:else if !desk}
          <Badge>No preview yet</Badge>
        {:else if desk.masters.length > 1}
          {@const current = desk}
          <span class="menu">
            <Menu
              value={current.preview ?? ""}
              options={current.masters.map((m) => ({ value: m.id, label: m.title }))}
              label="Preview in"
              size="sm"
              onchange={(master) => showDesk(current.id, master)}
            />
          </span>
        {:else}
          <Badge>{desk.masters[0]?.title ?? "Default template"}</Badge>
        {/if}
      {/snippet}
      {#if view?.entry && desk}
        {desk.preview ? "Entry highlighted" : "Entry alone, default template"}
      {:else if view && !view.entry}
        Nothing on a page: the story happened off the resume
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
    grid-template-columns: minmax(0, 1fr);
    align-content: start;
    gap: var(--space-6);
    min-height: 0;
    overflow: auto;
    overscroll-behavior: contain;
    scroll-padding-block-end: var(--space-16);
    padding: var(--space-6) var(--space-5) 0;
    box-shadow: inset -1px 0 0 var(--line);
  }
  /* Telling it: the thread scrolls, the composer stays at the foot. */
  .column--tell {
    display: flex;
    flex-direction: column;
    gap: 0;
    overflow: hidden;
    padding: 0;
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
  .parts {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--space-4);
    margin-inline: calc(var(--space-2) * -1);
  }
  .group {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
  }
  /* The title row's label is the story's meta; its text, the serif heading. */
  .title-row .title {
    margin: 0;
  }
  .words {
    margin: 0;
    white-space: pre-wrap;
  }
  .none {
    color: var(--ink-3);
  }
  /* What isn't a row (the links, the themes, the uses) sits on the rows' text edge. */
  .block {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--space-2);
    padding: var(--space-1) var(--space-3) 0;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1-5);
  }
  .list {
    display: grid;
    margin: 0;
    padding: 0;
    overflow: hidden;
    list-style: none;
    border-radius: calc(var(--radius-lg) - 1px);
  }
  .list > li + li {
    box-shadow: inset 0 1px 0 var(--line);
  }
  .link {
    display: flex;
    align-items: center;
    gap: var(--space-2-5);
    min-height: 40px;
    padding: var(--space-2) var(--space-4);
    color: var(--ink);
    text-decoration: none;
  }
  .link :global(.icon) {
    flex: none;
    color: var(--ink-3);
  }
  .link--open {
    cursor: default;
    -webkit-user-drag: none;
    transition: background-color var(--dur-instant) var(--ease-out);
  }
  .link--open:focus-visible {
    outline-offset: -2px;
  }
  .link__text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .link__text b {
    font-weight: var(--weight-medium);
  }
  .gone {
    color: var(--ink-3);
  }
  .happened {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }
  .menu {
    display: contents;
  }
  .empty {
    margin: 0;
    text-wrap: pretty;
  }
  .keys-wrap {
    position: sticky;
    bottom: 0;
    margin-inline: calc(var(--space-5) * -1);
  }

  /* Tell it: Practice's thread, for one story. */
  .thread {
    flex: 1;
    min-height: 0;
    overflow: auto;
    overscroll-behavior: contain;
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    padding: var(--space-5) var(--space-6);
  }
  .turn p {
    margin: 0;
  }
  .turn .who {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-bottom: var(--space-1-5);
  }
  .who :global(.icon) {
    color: var(--ink-3);
  }
  .who__name {
    font-weight: var(--weight-medium);
  }
  .asked {
    font-size: var(--text-md);
    line-height: var(--leading-md);
    font-weight: var(--weight-medium);
    text-wrap: pretty;
  }
  .bubble {
    align-self: flex-end;
    max-width: 88%;
    margin: 0;
    padding: var(--space-2-5) var(--space-3);
    border-radius: var(--radius-lg);
    border: 1px solid transparent;
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
    font-size: var(--text-md);
    line-height: var(--leading-md);
    white-space: pre-wrap;
    text-wrap: pretty;
  }
  .reading {
    padding-inline: var(--space-1);
  }
  .foot {
    flex: none;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--space-2);
    padding: var(--space-2) var(--space-5) var(--space-4);
    box-shadow: inset 0 1px 0 var(--line);
    background: var(--paper);
  }
  .foot__row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .next {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .composer {
    display: grid;
    gap: var(--space-1);
    padding: var(--space-1);
    border-radius: var(--radius-lg);
    border: 1px solid var(--input-border);
    background: var(--surface);
  }
  .composer :global(textarea) {
    border: 0;
    box-shadow: none;
    background: none;
  }
  .composer:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px oklch(from var(--accent) l c h / 0.18);
  }
  .composer__bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
  }

  /* Placeholders while the story loads: static, no shimmer. */
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
    .link--open:hover {
      background: var(--hover);
    }
  }
</style>
