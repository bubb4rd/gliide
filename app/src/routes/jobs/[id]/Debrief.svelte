<!--
  A round's debrief on the Interview tab (DESIGN.md, Job → Interview: debrief), shown once the round has ended: the
  questions they asked (the round's prep questions to check, first five then the rest on request, and any you add),
  what you got stuck on (chips), and notes. Saving (`⌘⏎`, the toolbar's Save debrief) keeps it as written (no model
  call), then reads it (one model call): what to dig into, per entry (each opens Dig on the entry with those
  questions), and notes the next round's prep reads. If reading fails the debrief stays saved, with Try again.
  Reopening a debriefed round shows it filled in; Save debrief waits for a change, and saving one reads it again.
  `Esc` leaves it for later (what you wrote waits here till you're back). The desk marks the lines of the prep
  questions you checked.
-->
<script module lang="ts">
  import type { Debrief as DebriefData } from "$lib/api";

  /** What you wrote and didn't save, by round, while the app runs: `Esc` leaves it for later. */
  const drafts = new Map<string, DebriefData>();
</script>

<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { goto } from "$app/navigation";
  import ArrowRight from "@lucide/svelte/icons/arrow-right";
  import CalendarClock from "@lucide/svelte/icons/calendar-clock";
  import CircleDashed from "@lucide/svelte/icons/circle-dashed";
  import ListTodo from "@lucide/svelte/icons/list-todo";
  import Plus from "@lucide/svelte/icons/plus";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Shovel from "@lucide/svelte/icons/shovel";
  import X from "@lucide/svelte/icons/x";
  import {
    appError,
    cancel,
    debrief as loadDebrief,
    readDebrief,
    saveDebrief,
    type AppStatus,
    type Asked,
    type DebriefView,
    type PrepView,
    type Round,
  } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import { endedAgo, kindLabel } from "$lib/rounds";
  import { STATUS } from "$lib/status";
  import { Button, Card, Checkbox, GroupHead, Icon, Input, Textarea, Thinking, tip } from "$lib/ui";

  interface Props {
    /** The posting: what Cancel stops. */
    job: string;
    round: Round;
    status: AppStatus;
    /** The round's prep, once loaded: its questions are the ones to check. */
    prep: PrepView | null;
    /** The prep is being written (`performance.now()` when it started). */
    writing: number | null;
    /** Prep question ids you've practiced. */
    practiced: Set<string>;
    /** The lines the desk marks (bindable): the checked prep questions'. */
    marked?: string[];
    /** There's something to save, and nothing saving or reading (bindable; the toolbar's Save debrief). */
    canSave?: boolean;
    /** Saved (and maybe read): the rounds and the application have moved on. */
    onsaved: (view: DebriefView) => void;
    /** `Esc`: leave it for later. */
    onleave: () => void;
  }

  let {
    job,
    round,
    status,
    prep,
    writing,
    practiced,
    marked = $bindable([]),
    canSave = $bindable(false),
    onsaved,
    onleave,
  }: Props = $props();

  /** Prep questions show five at first. */
  const FIRST = 5;

  let view = $state<DebriefView | null>(null);
  let loaded = $state(false);
  let checked = $state<string[]>([]);
  /** Prep questions checked in a saved debrief that today's prep no longer has. */
  let kept = $state<Asked[]>([]);
  let yours = $state<string[]>([]);
  let stuck = $state<string[]>([]);
  let notes = $state("");
  let question = $state("");
  let adding = $state<string | null>(null);
  let more = $state(false);
  let saving = $state(false);
  let reading = $state<number | null>(null);
  let failed = $state<string | null>(null);

  function fill(d: DebriefData) {
    checked = d.asked.filter((a) => a.prep !== null).map((a) => a.prep!);
    kept = d.asked.filter((a) => a.prep !== null);
    yours = d.asked.filter((a) => a.prep === null).map((a) => a.text);
    stuck = [...d.stuckOn];
    notes = d.notes;
  }

  onMount(() => {
    const id = round.id;
    loadDebrief(id)
      .then((v) => {
        view = v;
        const draft = drafts.get(id);
        if (draft) fill(draft);
        else if (v) fill(v.debrief);
      })
      .catch((e) => (failed = appError(e).message))
      .finally(() => (loaded = true));
  });

  /** The prep's questions, then any checked before that it no longer has. */
  const rows = $derived.by(() => {
    const questions = (prep?.questions ?? []).map((q) => ({ id: q.id, text: q.text, lines: q.lines }));
    const ids = new Set(questions.map((q) => q.id));
    return [...questions, ...kept.filter((k) => !ids.has(k.prep!)).map((k) => ({ id: k.prep!, text: k.text, lines: [] }))];
  });
  const shown = $derived(more ? rows : rows.filter((r, i) => i < FIRST || checked.includes(r.id)));
  const hidden = $derived(rows.length - shown.length);

  /** The debrief as it stands on screen. */
  const current = $derived<DebriefData>({
    asked: [
      ...rows.filter((r) => checked.includes(r.id)).map((r) => ({ text: r.text, prep: r.id })),
      ...yours.map((text) => ({ text, prep: null })),
    ],
    stuckOn: stuck,
    notes: notes.trim(),
  });
  /** A debrief's content, whatever its order and case (saving dedupes and may reorder). */
  const sig = (d: DebriefData) =>
    JSON.stringify([
      d.asked.map((a) => `${a.prep ?? ""}|${a.text.trim().toLowerCase()}`).sort(),
      d.stuckOn.map((s) => s.trim().toLowerCase()).sort(),
      d.notes.trim(),
    ]);
  const dirty = $derived(!view || sig(current) !== sig(view.debrief));
  const fromPrep = $derived(current.asked.filter((a) => a.prep !== null).length);
  /** A change from the saved debrief, or a question or topic still in its field (saving adds those). */
  const pending = $derived(dirty || question.trim() !== "" || (adding?.trim() ?? "") !== "");

  $effect(() => {
    canSave = loaded && pending && !saving && reading === null;
  });

  // What you wrote waits while you're away; saving clears it.
  $effect(() => {
    const d = current;
    const id = round.id;
    if (!loaded) return;
    untrack(() => {
      if (dirty) drafts.set(id, $state.snapshot(d));
      else drafts.delete(id);
    });
  });

  // The desk marks the lines of the prep questions you checked.
  $effect(() => {
    marked = [...new Set(rows.filter((r) => checked.includes(r.id)).flatMap((r) => r.lines))];
  });

  function toggle(id: string, on: boolean) {
    checked = on ? [...checked, id] : checked.filter((c) => c !== id);
  }

  function addQuestion() {
    const text = question.trim();
    if (!text) return;
    if (!yours.some((y) => y.toLowerCase() === text.toLowerCase())) yours = [...yours, text];
    question = "";
  }

  function addStuck() {
    const text = adding?.trim();
    if (text && !stuck.some((s) => s.toLowerCase() === text.toLowerCase())) stuck = [...stuck, text];
    adding = null;
  }

  /** Saves the debrief as written, then reads it (unless there's nothing in it to read). */
  export async function save() {
    if (!canSave) return;
    if (adding !== null) addStuck();
    if (question.trim()) addQuestion();
    saving = true;
    failed = null;
    try {
      view = await saveDebrief(round.id, $state.snapshot(current));
      fill(view.debrief);
      drafts.delete(round.id);
      onsaved(view);
    } catch (e) {
      failed = appError(e).message;
      return;
    } finally {
      saving = false;
    }
    if (!view.read && readable) await read();
  }

  async function read() {
    if (reading !== null) return;
    reading = performance.now();
    failed = null;
    try {
      view = await readDebrief(round.id);
      onsaved(view);
    } catch (e) {
      const error = appError(e);
      failed = error.kind === "cancelled" ? null : error.message;
    } finally {
      reading = null;
    }
  }

  // A field keeps its own `Esc` (it leaves the field); outside one, `Esc` leaves the debrief, or stops the reading.
  $effect(() =>
    bindKeys({
      "mod+Enter": () => void save(),
      Escape: () => (reading !== null ? void cancel(job) : onleave()),
    }),
  );

  /** In a field, `Esc` leaves the field; the next leaves the debrief. */
  function blurOnEscape(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    e.stopPropagation();
    (e.currentTarget as HTMLElement).blur();
  }

  // ── What saving does, then what reading found ──
  const name = $derived(round.kind === "other" ? "interview" : kindLabel(round.kind).toLowerCase());
  const noWord = $derived(
    new Date(new Date(view && !dirty ? view.at : Date.now()).getTime() + 7 * 86_400_000).toLocaleDateString(
      undefined,
      { month: "short", day: "numeric" },
    ),
  );
  const found = $derived(view?.read && !dirty ? view.read : null);
  /** The saved debrief has something to read. */
  const readable = $derived(
    view !== null && (view.debrief.asked.length > 0 || view.debrief.stuckOn.length > 0 || view.debrief.notes !== ""),
  );
  /** Stuck-on topics a dig took up (a dig's topic is one of them, verbatim). */
  const covered = (topic: string) => found?.digs.some((d) => d.topic === topic) ?? false;
  const digHref = (entry: string) => `/bank/entries/${encodeURIComponent(entry)}?debrief=${encodeURIComponent(round.id)}`;
  const count = (n: number, one: string) => `${n} ${one}${n === 1 ? "" : "s"}`;
</script>

<section class="debrief" aria-label="Debrief">
  <header class="head">
    <h2 class="heading">How did the {name} go?</h2>
    <span class="meta num">{endedAgo(round)}</span>
  </header>

  {#if !loaded}
    <p class="meta quiet">Opening the debrief…</p>
  {:else}
    <div class="group">
      <div class="group__head">
        <GroupHead
          label="Questions they asked"
          meta={current.asked.length ? `${current.asked.length} · ${fromPrep} from your prep` : ""}
        />
        {#if hidden > 0}
          <Button variant="ghost" size="sm" onclick={() => (more = true)}>Show {hidden} more from prep</Button>
        {:else if more && rows.length > FIRST}
          <Button variant="ghost" size="sm" onclick={() => (more = false)}>Show fewer</Button>
        {/if}
      </div>
      {#if writing !== null && !prep}
        <div class="writing">
          <Thinking label="Writing prep from the posting and the resume you sent…" startedAt={writing} />
        </div>
      {/if}
      <ul class="asked">
        {#each shown as row (row.id)}
          <li>
            <label class="asked__row">
              <Checkbox
                checked={checked.includes(row.id)}
                label="They asked this"
                onchange={(on) => toggle(row.id, on)}
              />
              <span class="asked__text">{row.text}</span>
              <span class="meta asked__src">{practiced.has(row.id) ? "Prep · practiced" : "Prep"}</span>
            </label>
          </li>
        {/each}
        {#each yours as text, i (text)}
          <li class="asked__row">
            <span class="yours"><Icon icon={Plus} size={14} weight="medium" /></span>
            <span class="asked__text">{text}</span>
            <span class="meta asked__src">Yours</span>
            <Button
              variant="ghost"
              size="sm"
              square
              icon={X}
              aria-label="Remove this question"
              {@attach tip("Remove this question")}
              onclick={() => (yours = yours.filter((_, j) => j !== i))}
            />
          </li>
        {/each}
      </ul>
      <div class="add">
        <Input
          size="md"
          bind:value={question}
          placeholder="Add a question they asked"
          aria-label="Add a question they asked"
          onkeydown={(e) => {
            if (e.key === "Enter" && !e.metaKey) {
              e.preventDefault();
              addQuestion();
            } else blurOnEscape(e);
          }}
        />
      </div>
    </div>

    <div class="group">
      <GroupHead label="What you got stuck on" />
      <div class="chips">
        {#each stuck as topic, i (topic)}
          <span class="stuck">
            <span class="stuck__text">{topic}</span>
            <button
              type="button"
              class="stuck__x"
              aria-label="Remove {topic}"
              {@attach tip("Remove")}
              onclick={() => (stuck = stuck.filter((_, j) => j !== i))}><Icon icon={X} size={12} weight="medium" /></button
            >
          </span>
        {/each}
        {#if adding !== null}
          <span class="stuck__field">
            <Input
              size="sm"
              bind:value={adding}
              placeholder="Kafka consumer groups"
              aria-label="What you got stuck on"
              onkeydown={(e) => {
                if (e.key === "Enter" && !e.metaKey) {
                  e.preventDefault();
                  addStuck();
                  adding = "";
                } else if (e.key === "Escape") {
                  e.stopPropagation();
                  adding = null;
                }
              }}
              onblur={addStuck}
              {@attach (el) => el.focus()}
            />
          </span>
        {:else}
          <button type="button" class="stuck stuck--add" onclick={() => (adding = "")}
            ><Icon icon={Plus} size={12} weight="medium" />Add</button
          >
        {/if}
      </div>
    </div>

    <div class="group">
      <GroupHead label="Notes" />
      <div class="inset">
        <Textarea
          bind:value={notes}
          placeholder="What they cared about, who you met, when they’ll write"
          aria-label="Notes"
          onkeydown={blurOnEscape}
        />
      </div>
    </div>

    <div class="group">
      <GroupHead label={found ? "What happens next" : "When you save"} />
      {#if reading !== null}
        <div class="writing">
          <Thinking label="Reading your debrief…" startedAt={reading} oncancel={() => void cancel(job)} />
        </div>
      {:else if view && !dirty && !view.read && (failed || readable)}
        <div class="failed">
          {#if failed}
            <p class="meta problem" role="alert">Saved, but reading it failed: {failed}</p>
          {:else}
            <p class="meta">Saved; not read yet.</p>
          {/if}
          <Button size="sm" icon={RotateCcw} onclick={() => void read()}>{failed ? "Try again" : "Read it"}</Button>
        </div>
      {:else if failed}
        <p class="meta problem" role="alert">{failed}</p>
      {/if}
      {#if reading === null}
        <div class="inset"><Card>
          <ul class="nexts">
            {#if found}
              {#each found.digs as dig (dig.entry + dig.topic)}
                <li>
                  <Icon icon={Shovel} size={16} />
                  <p class="nexts__text">
                    <b>Dig into {dig.topic} at {dig.entryName}</b><span class="meta"
                      >{` · ${count(dig.questions.length, "question")}`}</span
                    >
                  </p>
                  <Button
                    variant="ghost"
                    size="sm"
                    square
                    icon={ArrowRight}
                    aria-label="Open {dig.entryName}"
                    {@attach tip(`Open ${dig.entryName} on its Interview tab`)}
                    onclick={() => void goto(digHref(dig.entry))}
                  />
                </li>
              {/each}
              {#each view?.debrief.stuckOn.filter((s) => !covered(s)) ?? [] as topic (topic)}
                <li>
                  <Icon icon={CircleDashed} size={16} />
                  <p class="nexts__text meta">Nothing in your experience touches {topic} yet</p>
                </li>
              {/each}
              <li>
                <Icon icon={ListTodo} size={16} />
                <div class="nexts__text">
                  <p><b>The next round’s prep will use this</b></p>
                  {#each found.prepNotes as note (note)}<p class="meta">{note}</p>{/each}
                </div>
              </li>
            {:else}
              {#if current.stuckOn.length}
                <li>
                  <Icon icon={Shovel} size={16} />
                  <p class="nexts__text">
                    <b>Questions to dig into what you got stuck on</b><span class="meta"
                      >{" · in the experience it touches"}</span
                    >
                  </p>
                </li>
              {/if}
              <li>
                <Icon icon={ListTodo} size={16} />
                <p class="nexts__text"><b>The next round’s prep will use this</b></p>
              </li>
            {/if}
            <li>
              <Icon icon={CalendarClock} size={16} />
              <p class="nexts__text">
                <b>Status stays {STATUS[status].label}</b><span class="meta">{` · “No word” from ${noWord}`}</span>
              </p>
            </li>
          </ul>
        </Card></div>
      {/if}
    </div>
    <p class="meta foot">⌘⏎ saves · Esc leaves it for later</p>
  {/if}
</section>

<style>
  .debrief {
    display: grid;
    gap: var(--space-4);
    padding: var(--space-2) var(--space-3) var(--space-6);
  }
  .head {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    padding-inline: var(--space-3);
  }
  .heading {
    flex: 1;
    min-width: 0;
    margin: 0;
    font-size: var(--text-lg);
    line-height: var(--leading-lg);
    letter-spacing: var(--tracking-lg);
    font-weight: var(--weight-medium);
  }
  .quiet,
  .foot {
    margin: 0;
    padding-inline: var(--space-3);
    color: var(--ink-3);
  }
  .group {
    display: grid;
    gap: var(--space-1);
  }
  /* Fields, chips and the card sit on the rows' text, past their 12px wash. */
  .inset,
  .add,
  .writing,
  .failed,
  .chips,
  .group > .problem {
    padding-inline: var(--space-3);
  }
  .group__head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .group__head > :global(:first-child) {
    flex: 1;
    min-width: 0;
  }
  .asked {
    display: grid;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .asked__row {
    display: flex;
    align-items: center;
    gap: var(--space-2-5);
    min-height: 36px;
    padding: var(--space-1) var(--space-3);
    border-radius: var(--radius-md);
    cursor: default;
    transition: background-color var(--dur-instant) var(--ease-out);
  }
  .asked__text {
    flex: 1;
    min-width: 0;
    font-size: var(--text-md);
    line-height: var(--leading-md);
    text-wrap: pretty;
  }
  .asked__src {
    flex: none;
    color: var(--ink-3);
  }
  /* Yours: a plus where the checkbox sits, in the same 16px. */
  .yours {
    display: grid;
    place-items: center;
    flex: none;
    width: 16px;
    height: 16px;
    color: var(--ink-3);
  }
  .add {
    padding-top: var(--space-1);
  }
  .writing,
  .failed {
    display: grid;
    justify-items: start;
    gap: var(--space-2);
    padding-block: var(--space-1);
  }
  .problem {
    margin: 0;
    color: var(--danger);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1-5);
    padding-top: var(--space-1);
  }
  /* What you got stuck on: a neutral chip (as `Chip`'s) with its remove at the end. */
  .stuck {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    max-width: 100%;
    height: var(--control-sm);
    padding-inline: var(--space-2) var(--space-1);
    border: 0;
    border-radius: var(--radius-full);
    font: var(--weight-medium) var(--text-sm) / 1 var(--font-ui);
    color: var(--ink-2);
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .stuck__text {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .stuck__x {
    display: grid;
    place-items: center;
    flex: none;
    width: 16px;
    height: 16px;
    padding: 0;
    border: 0;
    border-radius: var(--radius-full);
    background: none;
    color: var(--ink-3);
    transition-property: background-color, color;
    transition-duration: var(--dur-instant);
    transition-timing-function: var(--ease-out);
  }
  .stuck__x:focus-visible,
  .stuck--add:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .stuck--add {
    padding-inline: calc(var(--space-2) - 2px) var(--space-2);
    background: transparent;
    color: var(--ink-3);
  }
  .stuck__field {
    width: 200px;
  }
  .nexts {
    display: grid;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .nexts li {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2-5);
    min-height: 40px;
    padding: var(--space-2-5) var(--space-3) var(--space-2-5) var(--space-4);
  }
  .nexts li + li {
    box-shadow: inset 0 1px 0 var(--line);
  }
  .nexts li > :global(.icon) {
    flex: none;
    margin-top: 3px;
    color: var(--ink-3);
  }
  .nexts li > :global(button) {
    margin-block: -2px;
  }
  .nexts__text {
    flex: 1;
    min-width: 0;
    margin: 0;
    text-wrap: pretty;
  }
  .nexts__text p {
    margin: 0;
  }
  .nexts b {
    font-weight: var(--weight-medium);
  }
  @media (hover: hover) {
    .asked__row:hover {
      background: var(--hover);
    }
    .stuck__x:hover {
      background: var(--hover);
      color: var(--ink);
    }
    .stuck--add:hover {
      color: var(--ink-2);
      background: var(--hover);
    }
  }
</style>
