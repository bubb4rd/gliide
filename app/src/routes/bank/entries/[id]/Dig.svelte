<!--
  An entry's Interview tab (DESIGN.md, Experience → an entry → Interview): gliide asks about the entry. It opens with
  the agent's read of the entry and a set of questions asked at once (one model call), each about a bullet or the
  entry, answered in place, skipped, or marked as not applying; `⌘⏎` sends them all (one call). What comes back: the
  facts in your answers, kept at once (your words; Undo on each), additions proposed for you to accept (a variant, a
  bullet, skills, a story; never a rewrite of a bullet's text), and follow-up questions where an answer was vague.
  The box at the foot takes anything else you want it to know. The thread is kept per entry on this Mac. Opened from
  a round's debrief (`debrief`, the round), it asks that debrief's questions about the entry, once per debrief.
-->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import { goto } from "$app/navigation";
  import ArrowUp from "@lucide/svelte/icons/arrow-up";
  import BookOpen from "@lucide/svelte/icons/book-open";
  import Check from "@lucide/svelte/icons/check";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import Undo2 from "@lucide/svelte/icons/undo-2";
  import X from "@lucide/svelte/icons/x";
  import {
    appError,
    applications as loadApplications,
    cancel,
    debrief as loadDebrief,
    digCapture,
    digQuestions,
    editEntry,
    rounds as loadRounds,
    saveStory,
    type DigAnswer,
    type DigCapture,
    type DigOp,
    type DigQuestion,
    type DigQuestions,
    type EntryView,
    type FactLine,
  } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import { modelName } from "$lib/models";
  import { kindLabel } from "$lib/rounds";
  import { app } from "$lib/stores/app.svelte";
  import { themeLabel } from "$lib/stories";
  import { Badge, Button, CompanyMark, Icon, KeyFooter, type KeyCommand, Textarea, Thinking, tip, toast } from "$lib/ui";

  interface Props {
    entry: string;
    view: EntryView;
    /** The entry as an edit left it. */
    onview: (view: EntryView) => void;
    /** The screen's keys, after Dig's own in Commands. */
    keys: KeyCommand[];
    /** A round whose debrief has questions about this entry: they open the thread, once. */
    debrief?: string | null;
  }

  let { entry, view, onview, keys, debrief = null }: Props = $props();

  // ── The thread, kept per entry ──
  interface Answer {
    text: string;
    skipped: boolean;
    notApplicable: boolean;
  }
  type Turn =
    | {
        kind: "ask";
        read: string;
        model: string;
        questions: DigQuestion[];
        answers: Record<string, Answer>;
        sent: boolean;
        /** Asked from a round's debrief: `<round>:<entry>`. */
        from?: string;
      }
    | { kind: "note"; text: string }
    | {
        kind: "capture";
        capture: Omit<DigCapture, "entry">;
        ops: Record<string, "pending" | "accepted" | "rejected">;
        removed: string[];
      };

  // `entry` is fixed for this component's life: the route re-creates the editor per entry.
  // svelte-ignore state_referenced_locally
  const STORE = `gliide-dig:${entry}`;
  let turns = $state<Turn[]>([]);
  let working = $state<{ label: string; at: number } | null>(null);
  let note = $state("");
  let thread = $state<HTMLElement>();

  const save = () => localStorage.setItem(STORE, JSON.stringify(turns));
  const last = $derived(turns.at(-1));
  const open = $derived(last?.kind === "ask" && !last.sent ? last : null);

  let loaded = $state(false);
  onMount(() => {
    try {
      turns = JSON.parse(localStorage.getItem(STORE) ?? "[]");
    } catch {
      turns = [];
    }
    loaded = true;
    if (!turns.length && !debrief) void ask(false);
  });

  // From a debrief: its questions about this entry open the thread (once per debrief), the read naming the round.
  let asked: string | null = null;
  $effect(() => {
    const round = debrief;
    if (!loaded || !round || asked === round) return;
    asked = round;
    void fromDebrief(round);
  });

  async function fromDebrief(round: string) {
    const key = `${round}:${entry}`;
    try {
      const found = await loadDebrief(round);
      const digs = found?.read?.digs.filter((d) => d.entry === entry) ?? [];
      if (!found?.read || !digs.length) {
        if (!turns.length) void ask(false);
        return;
      }
      if (turns.some((t) => t.kind === "ask" && t.from === key)) return;
      const at = await roundOf(round);
      const name = at ? `${at.company} ${at.kind}` : "";
      const topics = new Intl.ListFormat(undefined, { type: "conjunction" }).format(digs.map((d) => d.topic));
      const questions = digs.flatMap((d) => d.questions);
      turns.push({
        kind: "ask",
        read: `From your ${name ? `${name} ` : ""}debrief: you got stuck on ${topics}.`,
        model: found.read.model,
        questions,
        answers: blank(questions),
        sent: false,
        from: key,
      });
      save();
      void follow();
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
      if (!turns.length) void ask(false);
    }
  }

  /** The round's company and kind ("Globex", "technical"), from the application it belongs to. */
  async function roundOf(round: string): Promise<{ company: string; kind: string } | null> {
    const rows = app.applications.length ? app.applications : await loadApplications().catch(() => []);
    const known = rows.find((a) => a.debriefDue?.id === round || a.nextRound?.id === round);
    const lookIn = known ? [known] : rows.filter((a) => a.status !== "saved");
    const found = await Promise.all(
      lookIn.map((a) =>
        loadRounds(a.id)
          .then((rs) => ({ a, r: rs.find((r) => r.id === round) }))
          .catch(() => ({ a, r: undefined })),
      ),
    );
    const hit = found.find((f) => f.r);
    if (!hit?.r) return null;
    return { company: hit.a.company, kind: hit.r.kind === "other" ? "interview" : kindLabel(hit.r.kind).toLowerCase() };
  }

  async function follow() {
    await tick();
    thread?.scrollTo({ top: thread.scrollHeight });
  }

  async function run<T>(label: string, call: () => Promise<T>): Promise<T | null> {
    if (working) return null;
    working = { label, at: performance.now() };
    try {
      return await call();
    } catch (e) {
      const error = appError(e);
      if (error.kind !== "cancelled") toast(error.message, { tone: "danger" });
      return null;
    } finally {
      working = null;
    }
  }

  async function ask(again: boolean) {
    const q = await run<DigQuestions>("Reading the entry…", () => digQuestions(entry, again));
    if (!q) return;
    turns.push({ kind: "ask", read: q.read, model: q.model, questions: q.questions, answers: blank(q.questions), sent: false });
    save();
    void follow();
  }

  const blank = (questions: DigQuestion[]) =>
    Object.fromEntries(questions.map((q) => [q.id, { text: "", skipped: false, notApplicable: false }]));

  /** Sends the open questions' answers (and the note, if any), one call. */
  async function send(withNote = "") {
    const set = open;
    const answers: DigAnswer[] = set
      ? set.questions.map((question) => {
          const a = set.answers[question.id];
          const text = a.text.trim();
          return { question, answer: a.skipped || a.notApplicable || !text ? null : text, notApplicable: a.notApplicable };
        })
      : [];
    if (!answers.some((a) => a.answer) && !withNote.trim()) return;
    const result = await run<DigCapture>("Reading your answers…", () => digCapture(entry, answers, withNote.trim()));
    if (!result) return;
    if (set) set.sent = true;
    if (withNote.trim()) turns.push({ kind: "note", text: withNote.trim() });
    const { entry: entryView, ...capture } = result;
    turns.push({
      kind: "capture",
      capture,
      ops: Object.fromEntries(capture.ops.map((op) => [op.key, "pending" as const])),
      removed: [],
    });
    if (capture.followUps.length) {
      turns.push({
        kind: "ask",
        read: "",
        model: capture.model,
        questions: capture.followUps,
        answers: blank(capture.followUps),
        sent: false,
      });
    }
    note = "";
    save();
    onview(entryView);
    void follow();
  }

  // ── Facts kept, and Undo ──
  async function unkeep(turn: Extract<Turn, { kind: "capture" }>, fact: FactLine) {
    try {
      const facts = view.facts.filter((f) => f.id !== fact.id);
      onview(await editEntry(entry, { kind: "set-facts", facts }, view.preview));
      turn.removed.push(fact.id);
      save();
      toast("Removed the fact", {
        action: {
          label: "Undo",
          run: async () => {
            onview(await editEntry(entry, { kind: "set-facts", facts: [...view.facts, fact] }, view.preview));
            turn.removed = turn.removed.filter((id) => id !== fact.id);
            save();
          },
        },
      });
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
    }
  }

  // ── Proposed additions ──
  async function accept(turn: Extract<Turn, { kind: "capture" }>, op: DigOp) {
    try {
      if (op.kind === "add-story") {
        const story = await saveStory(op.story);
        toast("Saved as a story", { action: { label: "Open", run: () => void goto(`/bank/stories/${story.id}`) } });
      } else if (op.kind === "add-variant") {
        onview(await editEntry(entry, { kind: "add-variant", bullet: op.bullet, text: op.text }, view.preview));
      } else if (op.kind === "add-bullet") {
        onview(await editEntry(entry, { kind: "add-bullet", text: op.text }, view.preview));
      } else {
        const now = view.bullets.find((b) => b.id === op.bullet)?.skills ?? [];
        const skills = [...now, ...op.skills.filter((s) => !now.some((x) => x.toLowerCase() === s.toLowerCase()))];
        onview(await editEntry(entry, { kind: "set-skills", bullet: op.bullet, skills }, view.preview));
      }
      turn.ops[op.key] = "accepted";
      save();
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
    }
  }

  function reject(turn: Extract<Turn, { kind: "capture" }>, op: DigOp, state: "rejected" | "pending") {
    turn.ops[op.key] = state;
    save();
  }

  const OP = { "add-variant": "Add a variant", "add-bullet": "Add a bullet", "tag-skills": "Tag skills", "add-story": "Save a story" } as const;
  /** "Acme Widgets Co. › Bullet 2", "› new bullet", "› a story". */
  function where(op: DigOp): string {
    const name = view.details.name;
    if (op.kind === "add-variant" || op.kind === "tag-skills") {
      const at = view.bullets.findIndex((b) => b.id === op.bullet);
      return `${name} › ${at >= 0 ? `bullet ${at + 1}` : op.bullet}`;
    }
    return `${name} › ${op.kind === "add-bullet" ? "new bullet" : "a story"}`;
  }
  const textOf = (bullet: string) => view.bullets.find((b) => b.id === bullet)?.wordings[0]?.text ?? "";
  const day = (at: string) => new Date(at).toLocaleDateString(undefined, { month: "short", day: "numeric" });

  // ── Keys ──
  const counts = $derived.by(() => {
    if (!open) return null;
    const all = Object.values(open.answers);
    return {
      answered: all.filter((a) => a.text.trim() && !a.skipped && !a.notApplicable).length,
      skipped: all.filter((a) => a.skipped || a.notApplicable).length,
    };
  });

  const KEYS: KeyCommand[] = $derived([
    { keys: ["⌘⏎"], label: "Send answers", run: () => void send(note), disabled: !counts?.answered || working !== null },
    { keys: [], label: "Ask new questions", run: () => void ask(true), disabled: working !== null },
    ...keys,
  ]);

  $effect(() =>
    bindKeys({
      "mod+Enter": () => void send(note),
      Escape: () => (working ? void cancel(`entry:${entry}`) : false),
    }),
  );

  function onNoteKey(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey && !e.metaKey) {
      e.preventDefault();
      void send(note);
    }
  }
</script>

<div class="thread" bind:this={thread}>
  {#each turns as turn, t (t)}
    {#if turn.kind === "ask"}
      <article class="agent">
        {#if turn.read}
          <p class="who"><CompanyMark size={20} src="/agents/claude.png" /><span class="who__name">{modelName(turn.model)}</span></p>
          <p class="read">{turn.read}</p>
        {/if}
        <section class="card" aria-label="Questions">
          <header class="card__head">
            <span class="card__title">{turn.questions.length} questions</span>
            {#if turn.sent}<span class="meta">Sent</span>{:else if counts && turn === open}<span class="meta num"
                >{counts.answered} answered · {counts.skipped} skipped</span
              >{/if}
          </header>
          {#each turn.questions as q, i (q.id)}
            {@const a = turn.answers[q.id]}
            <div class="q">
              <span class="q__n num">{i + 1}</span>
              <div class="q__body">
                <p class="q__text"><span class="q__about">{q.aboutLabel} ·</span> {q.text}</p>
                {#if turn.sent}
                  {#if a.notApplicable}<p class="meta">Doesn’t apply</p>
                  {:else if a.skipped || !a.text.trim()}<p class="meta">Skipped</p>
                  {:else}<p class="q__answer">{a.text}</p>{/if}
                {:else if a.skipped || a.notApplicable}
                  <p class="q__line">
                    <span class="meta">{a.notApplicable ? "Doesn’t apply" : "Skipped"}</span>
                    <Button
                      variant="ghost"
                      size="sm"
                      icon={Undo2}
                      onclick={() => {
                        a.skipped = false;
                        a.notApplicable = false;
                        save();
                      }}>Undo</Button
                    >
                  </p>
                {:else}
                  <p class="q__line">
                    <span class="meta">{q.why}</span>
                    <Button
                      variant="ghost"
                      size="sm"
                      onclick={() => {
                        a.skipped = true;
                        save();
                      }}>Skip</Button
                    >
                    <Button
                      variant="ghost"
                      size="sm"
                      onclick={() => {
                        a.notApplicable = true;
                        save();
                      }}>Doesn’t apply</Button
                    >
                  </p>
                  <Textarea
                    size="lg"
                    rows={2}
                    bind:value={a.text}
                    placeholder="Your answer…"
                    aria-label="Answer to question {i + 1}"
                    oninput={save}
                  />
                {/if}
              </div>
            </div>
          {/each}
          {#if turn === open}
            <footer class="card__foot">
              <span class="meta">Your words, never a rewrite of a bullet</span>
              <Button
                variant="primary"
                size="sm"
                icon={ArrowUp}
                kbd="⌘⏎"
                disabled={!counts?.answered || working !== null}
                onclick={() => void send(note)}>Send answers</Button
              >
            </footer>
          {/if}
        </section>
      </article>
    {:else if turn.kind === "note"}
      <p class="bubble">{turn.text}</p>
    {:else}
      {@const c = turn.capture}
      <article class="agent">
        <p class="who"><CompanyMark size={20} src="/agents/claude.png" /><span class="who__name">{modelName(c.model)}</span></p>
        <p class="read">{c.reply}</p>

        {#if c.facts.length}
          <section class="card" aria-label="Facts kept">
            <header class="card__head">
              <span class="card__title">Facts kept <span class="meta num">{c.facts.length}</span></span>
              <span class="meta">Never printed</span>
            </header>
            {#each c.facts as fact (fact.id)}
              {@const gone = turn.removed.includes(fact.id)}
              <div class={["fact", gone && "fact--gone"]}>
                <Icon icon={gone ? X : Check} size={16} weight="medium" />
                <div>
                  <p class="fact__text">{fact.text}</p>
                  <p class="meta">
                    {gone ? "Removed" : `You · ${day(fact.at)} · Dig`}{fact.skills.length ? ` · ${fact.skills.join(", ")}` : ""}
                  </p>
                </div>
                {#if !gone}
                  <Button variant="ghost" size="sm" icon={Undo2} onclick={() => unkeep(turn, fact)}>Undo</Button>
                {/if}
              </div>
            {/each}
          </section>
        {/if}

        {#each c.ops as op, i (op.key)}
          {@const state = turn.ops[op.key]}
          <section class={["card", "op", state !== "pending" && "op--done"]} aria-label={OP[op.kind]}>
            <header class="card__head">
              <span class="op__n num">{i + 1}</span>
              <span class="op__where">{where(op)}</span>
              <Badge>{OP[op.kind]}</Badge>
            </header>
            <div class="op__body">
              {#if op.kind === "add-variant"}
                <p class="op__line op__line--same"><span aria-hidden="true">=</span>{textOf(op.bullet)}</p>
                <p class="op__line"><span aria-hidden="true">+</span>{op.text}</p>
              {:else if op.kind === "add-bullet"}
                <p class="op__line"><span aria-hidden="true">+</span>{op.text}</p>
              {:else if op.kind === "tag-skills"}
                <p class="op__line op__line--same"><span aria-hidden="true">=</span>{textOf(op.bullet)}</p>
                <p class="op__skills">
                  {#each op.skills as s (s)}<Badge tone="gain">{s}</Badge>{/each}
                </p>
              {:else}
                <p class="op__story-title"><Icon icon={BookOpen} size={14} />{op.story.title}</p>
                <dl class="star">
                  <dt>Situation</dt><dd>{op.story.situation}</dd>
                  <dt>Task</dt><dd>{op.story.task}</dd>
                  <dt>Action</dt><dd>{op.story.action}</dd>
                  <dt>Result</dt><dd>{op.story.result}</dd>
                </dl>
                {#if op.story.themes.length}<p class="meta">{op.story.themes.map(themeLabel).join(" · ")}</p>{/if}
              {/if}
              <p class="meta why">{op.why}</p>
            </div>
            <footer class="card__foot card__foot--start">
              {#if state === "pending"}
                <Button size="sm" icon={Check} onclick={() => accept(turn, op)}>Accept</Button>
                <Button variant="ghost" size="sm" icon={X} onclick={() => reject(turn, op, "rejected")}>Reject</Button>
                <span class="meta grow">Waiting on you</span>
              {:else if state === "accepted"}
                <span class="accepted"><Icon icon={Check} size={14} weight="medium" />Accepted</span>
              {:else}
                <span class="meta">Rejected</span>
                <Button variant="ghost" size="sm" icon={RotateCcw} onclick={() => reject(turn, op, "pending")}>Restore</Button>
              {/if}
            </footer>
          </section>
        {/each}

        {#each c.skipped as skip (skip.what)}
          <div class="left-out">
            <Icon icon={TriangleAlert} weight="medium" />
            <div>
              <p class="left-out__what">{skip.what} <span>· left out</span></p>
              <p class="left-out__why">{skip.why}</p>
            </div>
          </div>
        {/each}
      </article>
    {/if}
  {/each}

  {#if working}
    <div class="working">
      <Thinking label={working.label} startedAt={working.at} oncancel={() => void cancel(`entry:${entry}`)} />
    </div>
  {/if}
</div>

<div class="foot">
  <div class="composer">
    <Textarea
      bind:value={note}
      rows={2}
      placeholder="Anything else about {view.details.name}, in your words…"
      aria-label="Anything else"
      disabled={working !== null}
      onkeydown={onNoteKey}
    />
    <div class="composer__bar">
      <KeyFooter keys={KEYS} bare />
      <Button
        variant="ghost"
        size="sm"
        icon={RotateCcw}
        disabled={working !== null}
        {@attach tip("Ask new questions about this entry")}
        onclick={() => void ask(true)}>New questions</Button
      >
      <Button
        variant="primary"
        size="sm"
        square
        icon={ArrowUp}
        aria-label="Send"
        {@attach tip("Send (⏎)")}
        disabled={!note.trim() || working !== null}
        onclick={() => void send(note)}
      />
    </div>
  </div>
</div>

<style>
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
  .agent {
    display: grid;
    gap: var(--space-3);
  }
  .agent > p {
    margin: 0;
  }
  .who {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .who__name {
    font-weight: var(--weight-medium);
  }
  .read {
    font-size: var(--text-md);
    line-height: var(--leading-md);
    text-wrap: pretty;
  }
  .card {
    display: grid;
    border-radius: var(--radius-lg);
    border: 1px solid transparent;
    background: var(--surface);
    box-shadow: var(--shadow-border);
    overflow: hidden;
  }
  .card__head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: 36px;
    padding: var(--space-1-5) var(--space-3);
    box-shadow: inset 0 -1px 0 var(--line);
  }
  .card__title {
    flex: 1;
    font-weight: var(--weight-medium);
  }
  .card__foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    box-shadow: inset 0 1px 0 var(--line);
  }
  .card__foot--start {
    justify-content: flex-start;
  }
  .grow {
    flex: 1;
    text-align: end;
  }
  .q {
    display: flex;
    gap: var(--space-2);
    padding: var(--space-3);
  }
  .q + .q {
    box-shadow: inset 0 1px 0 var(--line);
  }
  .q__n {
    flex: none;
    width: 16px;
    color: var(--ink-3);
    line-height: var(--leading-md);
  }
  .q__body {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: var(--space-1-5);
  }
  .q__body p {
    margin: 0;
  }
  .q__text {
    font-weight: var(--weight-medium);
    text-wrap: pretty;
  }
  .q__about {
    font-weight: var(--weight-regular);
    color: var(--ink-3);
  }
  .q__line {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }
  .q__line .meta {
    flex: 1;
    min-width: 0;
  }
  .q__answer {
    font-size: var(--text-md);
    line-height: var(--leading-md);
    color: var(--ink-2);
    white-space: pre-wrap;
  }
  .fact {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: start;
    gap: var(--space-2);
    padding: var(--space-2-5) var(--space-3);
  }
  .fact + .fact {
    box-shadow: inset 0 1px 0 var(--line);
  }
  .fact p {
    margin: 0;
  }
  .fact > :global(.icon) {
    margin-top: 3px;
    color: var(--gain-text);
  }
  .fact--gone > :global(.icon),
  .fact--gone .fact__text {
    color: var(--ink-3);
  }
  .fact__text {
    font-size: var(--text-md);
    line-height: var(--leading-md);
    text-wrap: pretty;
  }
  .op__n {
    display: inline-grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: var(--radius-xs);
    background: var(--accent);
    color: var(--on-accent);
    font-size: var(--text-sm);
  }
  .op--done .op__n {
    background: var(--sunken);
    color: var(--ink-3);
  }
  .op__where {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .op__body {
    display: grid;
    gap: var(--space-1-5);
    padding: var(--space-2-5) var(--space-3);
  }
  .op__body p,
  .star {
    margin: 0;
  }
  .op__line {
    display: grid;
    grid-template-columns: 14px minmax(0, 1fr);
    gap: var(--space-1-5);
    font-size: var(--text-md);
    line-height: var(--leading-md);
    text-wrap: pretty;
  }
  .op__line span {
    color: var(--ink-3);
  }
  .op__line--same {
    color: var(--ink-3);
  }
  .op__skills {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
  }
  .op__story-title {
    display: flex;
    align-items: center;
    gap: var(--space-1-5);
    font-weight: var(--weight-medium);
  }
  .star {
    display: grid;
    grid-template-columns: 72px minmax(0, 1fr);
    gap: var(--space-1) var(--space-3);
  }
  .star dt {
    color: var(--ink-3);
    font-size: var(--text-sm);
  }
  .star dd {
    margin: 0;
    text-wrap: pretty;
  }
  .why {
    padding-top: var(--space-0-5);
  }
  .accepted {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    color: var(--gain-text);
    font-weight: var(--weight-medium);
  }
  .left-out {
    display: flex;
    gap: var(--space-2);
    padding: var(--space-2-5) var(--space-3);
    border-radius: var(--radius-lg);
    background: var(--warn-soft);
  }
  .left-out > :global(.icon) {
    margin-top: 2px;
    color: var(--warn);
  }
  .left-out p {
    margin: 0;
  }
  .left-out__what {
    font-weight: var(--weight-medium);
  }
  .left-out__what span {
    color: var(--warn);
  }
  .left-out__why {
    color: var(--ink-2);
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
    white-space: pre-wrap;
  }
  .working {
    padding-inline: var(--space-1);
  }
  .foot {
    flex: none;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    padding: var(--space-2) var(--space-5) var(--space-4);
    box-shadow: inset 0 1px 0 var(--line);
    background: var(--paper);
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
    gap: var(--space-2);
  }
  .composer__bar > :global(:first-child) {
    margin-inline-end: auto;
  }
</style>
