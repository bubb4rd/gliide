<!--
  The Chat layout's column: the review as a conversation with the tailoring agent, beside the live page (the desk,
  Review's). The thread opens with the job (folding to a strip once it scrolls away) and the agent's plan for the
  master (its suggested edits, read live from the review); then each ask you sent, right-aligned, and what came back:
  the reply, a diff card per proposed edit (accept, reject, tweak, take back), what was left out and why (a skill your
  resume doesn't show asks whether you have it: Yes sends the ask again with it confirmed), and where the page stands.
  The composer at the foot sends the next ask. Every message has its actions on hover or focus: your ask Edit (in
  place, then sent again from there) and Copy, an answer Copy and Retry (asked again from its ask), the plan Suggest
  again; asking again from an ask takes it and every turn after it back, their edits off the page in one undo step,
  first asking you when that takes back more than the answer. The thread follows its foot as turns arrive or change,
  unless you've scrolled up; then a button over the composer goes back to it (↓ in an empty field too), saying "New
  answer" once one arrives out of view. While another master or a re-run is suggesting, everything but the keys is
  inert, dimmed under the progress.
-->
<script lang="ts">
  import { untrack } from "svelte";
  import ArrowDown from "@lucide/svelte/icons/arrow-down";
  import Check from "@lucide/svelte/icons/check";
  import CheckCheck from "@lucide/svelte/icons/check-check";
  import Copy from "@lucide/svelte/icons/copy";
  import FileCheck from "@lucide/svelte/icons/file-check";
  import FileWarning from "@lucide/svelte/icons/file-warning";
  import Pencil from "@lucide/svelte/icons/pencil";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Tag from "@lucide/svelte/icons/tag";
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import X from "@lucide/svelte/icons/x";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { appError, editTargets, type EditTargets, type SalaryRange } from "$lib/api";
  import { modelName } from "$lib/models";
  import { app } from "$lib/stores/app.svelte";
  import { Button, CompanyMark, Icon, type KeyCommand, Thinking, tip, toast } from "$lib/ui";
  import ChatJob from "./ChatJob.svelte";
  import Composer from "./Composer.svelte";
  import DiffCard from "./DiffCard.svelte";
  import MentionChip from "./MentionChip.svelte";
  import MentionField from "./MentionField.svelte";
  import { keywordLabel, keywordsOf, keywordTip } from "./keywords";
  import PlanCard from "./PlanCard.svelte";
  import { editTarget, placeOf } from "./place";
  import { find, proposalState, type Session } from "./session.svelte";
  import { pieces, trimmed, type Mention, type Turn } from "./thread";

  interface Props {
    s: Session;
    company: string;
    logoSrc: string | null;
    salary: SalaryRange | null;
    keys: KeyCommand[];
    /** The salary range's panel opens over this (Review owns the panel). */
    salaryAnchor?: HTMLButtonElement;
    /** The company photo's panel opens over this (Review owns the panel). */
    logoMark?: HTMLElement;
    onsalary: () => unknown;
    /** The posting has a row, so its salary range can be set. */
    canSalary: boolean;
  }

  let {
    s,
    company,
    logoSrc,
    salary,
    keys,
    salaryAnchor = $bindable(),
    logoMark = $bindable(),
    onsalary,
    canSalary,
  }: Props = $props();

  // Review renders the chat only once there's a review; it never goes back to none.
  const review = $derived(s.review!);
  const ready = $derived(s.phase === "ready");
  const busy = $derived(s.instructing !== null);
  const hasCards = $derived(s.turns.some((t) => t.proposals.length));
  /** The thread's foot is out of view: the button back to it shows. */
  let away = $state(false);
  /** An answer arrived while the foot was out of view, and you haven't been back since. */
  let unseen = $state(false);

  /** The chat's own keys, listed (the composer's act in the field, a card's on the focused card) before the screen's. */
  const CHAT_KEYS: KeyCommand[] = $derived([
    { keys: ["⏎"], label: "Send" },
    { keys: ["⇧⏎"], label: "New line" },
    { keys: ["@"], label: "Point at a part" },
    { keys: ['"'], label: "Quote a line" },
    { keys: ["/"], label: "Quick asks" },
    { keys: ["Esc"], label: "Stop", run: () => s.cancel(), disabled: !busy },
    { keys: ["↑"], label: "To the newest edit", run: () => void last(), disabled: !hasCards },
    { keys: ["↓"], label: "To the latest", run: () => void latest(), disabled: !away },
    { keys: ["↑", "↓"], label: "Between edits" },
    { keys: ["A"], label: "Accept the edit" },
    { keys: ["X"], label: "Reject the edit" },
    { keys: ["T", "⏎"], label: "Tweak the edit" },
    { keys: ["U"], label: "Take the edit back" },
    { keys: ["Esc"], label: "Back to the field" },
  ]);

  // ── The parts of the page, once per review: the composer's `@` menu and the cards' places ──
  let targets = $state<EditTargets | null>(null);
  let targetsFor: string | null = null;
  $effect(() => {
    const at = review.id;
    if (targetsFor === at) return;
    targetsFor = at;
    targets = null;
    editTargets(at)
      .then((t) => targetsFor === at && (targets = t))
      .catch((e) => s.onerror(appError(e)));
  });

  // ── The plan: the master's suggestions, every change that isn't one of the turns' proposals ──
  const proposed = $derived(new Set(s.turns.flatMap((t) => t.proposals.map((p) => p.key))));
  const plan = $derived(
    review.groups.flatMap((group) =>
      group.changes
        .filter((c) => !proposed.has(c.key))
        .map((change) => ({ change, place: placeOf(change.target, group.label, targets) })),
    ),
  );

  // ── Turns ──
  /** Each turn's first card number: they count on from the plan's across the thread. */
  const firsts = $derived(
    s.turns.reduce<number[]>(
      (acc, t, i) => [...acc, i ? acc[i - 1] + s.turns[i - 1].proposals.length : plan.length + 1],
      [],
    ),
  );
  /** The newest answered turn: its foot says where the page stands now. */
  const newest = $derived(s.turns.findLastIndex((t) => t.status === "done"));
  const pendingOf = (t: Turn) => t.proposals.filter((p) => proposalState(review, p) === "pending").map((p) => p.key);
  const groupOf = (key: string) => review.groups.find((g) => g.changes.some((c) => c.key === key))?.label;
  const KINDS = { bullet: "New bullet", skill: "Skill", line: "Line", summary: "Summary" } as const;
  const time = (ms: number) => new Date(ms).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  /** Who answered: the model the plan was made with, a turn's own (older turns didn't keep it: the review's). */
  const planModel = $derived(app.modelName(review.model));
  const turnModel = (t: Turn) => (t.model ? modelName(t.model) : planModel);

  // ── Where the page stands (the newest turn's foot) ──
  const keywords = $derived(keywordsOf(review));
  const covered = $derived(keywords.have.length + keywords.add.length);
  const fits = $derived(review.pageCount <= review.pageLimit);
  const pageNote = $derived(
    fits
      ? `Fits on ${review.pageLimit} page${review.pageLimit === 1 ? "" : "s"}`
      : `${review.pageCount} pages: over by ${review.pageCount - review.pageLimit}`,
  );

  // Only what arrives while the chat is open enters; a thread already there (or loaded with a review) just shows.
  const openedAt = Date.now();
  const firstArrival = untrack(() => s.arrival);

  // ── Asking again: an answer retried, an ask edited (both from where that ask was), copying ──
  /** The turn whose Retry waits on your say-so: it would take back later asks or accepted edits. */
  let confirming = $state<string | null>(null);
  /** The ask being edited in place: its words and parts. */
  let editing = $state<{ id: string; text: string; scope: Mention[] } | null>(null);

  /** What asking again from `t` takes back beyond `t` itself, as a sentence; null when nothing. */
  function rewindNote(t: Turn): string | null {
    const { later, accepted } = s.rewinding(t);
    const parts = [
      later > 0 && (later === 1 ? "the ask after it leaves the thread" : `the ${later} asks after it leave the thread`),
      accepted > 0 &&
        (accepted === 1
          ? "1 accepted edit leaves the page (⌘Z brings it back)"
          : `${accepted} accepted edits leave the page (⌘Z brings them back)`),
    ].filter((p) => p !== false);
    if (!parts.length) return null;
    const sentence = parts.join(", and ");
    return `${sentence[0].toUpperCase()}${sentence.slice(1)}.`;
  }

  /** Asks `t` again from where it was; first asks you when that takes back more than `t`'s own pending edits. */
  function retry(t: Turn) {
    if (confirming !== t.id && rewindNote(t)) {
      confirming = t.id;
      return;
    }
    confirming = null;
    follow();
    void s.resend(t, t.ask, t.scope);
  }

  function startEdit(t: Turn) {
    confirming = null;
    editing = { id: t.id, text: t.ask, scope: [...t.scope] };
  }

  function sendEdit(t: Turn) {
    const e = editing && trimmed(editing);
    if (!e?.text || !ready || busy) return;
    editing = null;
    follow();
    void s.resend(t, e.text, e.scope);
  }

  async function copy(text: string) {
    try {
      await writeText(text);
      toast("Copied", { tone: "neutral" });
    } catch (e) {
      toast(`Couldn’t copy: ${appError(e).message}`, { tone: "danger" });
    }
  }

  /** Yes to a skill the agent left out asks again with it confirmed; the thread follows the new turn. */
  function answer(t: Turn, i: number, yes: boolean) {
    if (yes) follow();
    void s.answer(t, i, yes);
  }

  // ── Moving between cards ──
  let composer = $state<Composer>();
  const cards = () => [...(flow?.querySelectorAll<HTMLElement>("[data-card]") ?? [])];

  function move(from: HTMLElement, by: 1 | -1) {
    const all = cards();
    const next = all[all.indexOf(from) + by];
    if (next) {
      next.focus({ preventScroll: true });
      next.scrollIntoView({ block: "nearest" });
    } else if (by === 1) composer?.focus();
  }

  /** Focuses the newest card; false when there's none. */
  function last(): boolean {
    const card = cards().at(-1);
    if (!card) return false;
    card.focus({ preventScroll: true });
    card.scrollIntoView({ block: "nearest" });
    return true;
  }

  // ── Following the thread's foot ──
  let scroller = $state<HTMLElement>();
  let flow = $state<HTMLElement>();
  /** The view follows the foot as the thread grows; scrolling up lets go, coming back near the foot takes hold. */
  let pinned = true;
  /** `scrollTop` as of the last scroll event (or our own scroll), to tell scrolling up from content growing. */
  let lastTop = 0;
  /** Within this of the foot counts as at it. */
  const NEAR = 48;

  function stick() {
    const el = scroller;
    if (!pinned || !el) return;
    el.scrollTop = el.scrollHeight;
    lastTop = el.scrollTop;
  }

  function measure() {
    const el = scroller;
    if (!el) return;
    away = el.scrollHeight - el.scrollTop - el.clientHeight >= NEAR;
    if (!away) unseen = false;
  }

  /** Brings the foot into view and holds it there: a sent ask, a retry. */
  function follow() {
    pinned = true;
    stick();
  }

  /** Jumps to the foot and holds it there; false when it's already in view. */
  function latest(): boolean {
    if (!away) return false;
    follow();
    measure();
    return true;
  }

  // The newest turn finishing out of view marks it unseen; one already there when the chat opens doesn't.
  const signOf = (t?: Turn) => (t ? `${t.id} ${t.status}` : "");
  let heard = untrack(() => signOf(s.turns.at(-1)));
  $effect(() => {
    const t = s.turns.at(-1);
    const sign = signOf(t);
    if (sign === heard) return;
    heard = sign;
    if (t?.status === "done" && untrack(() => away)) unseen = true;
  });

  /** Working in the thread (a click, a focused card) lets go of the foot: what you open or fold stays put. */
  function letGo(el: HTMLElement) {
    const off = () => (pinned = false);
    el.addEventListener("pointerdown", off);
    el.addEventListener("focusin", off);
    return () => {
      el.removeEventListener("pointerdown", off);
      el.removeEventListener("focusin", off);
    };
  }

  // Only scrolling up lets go of the foot. A scroll event can arrive after the thread grew under it (the ask just
  // sent lands before the event for the scroll that `follow` made), so distance from the foot alone would let go of
  // it there; scrollTop never goes down by the thread growing.
  function onscroll() {
    const el = scroller;
    if (!el) return;
    if (el.scrollHeight - el.scrollTop - el.clientHeight < NEAR) pinned = true;
    else if (el.scrollTop < lastTop) pinned = false;
    lastTop = el.scrollTop;
    measure();
  }

  // Anything that changes the thread's height (a turn, an answer, a card's state, the composer growing) keeps the
  // foot in view while it's held.
  $effect(() => {
    if (!scroller || !flow) return;
    const observer = new ResizeObserver(() => {
      stick();
      measure();
    });
    observer.observe(flow);
    observer.observe(scroller);
    stick();
    measure();
    return () => observer.disconnect();
  });
</script>

<div class="chat">
  {#if !ready && s.phase === "suggesting" && !s.running}
    <div class="running">
      <Thinking label="Suggesting edits…" oncancel={() => s.cancel()} />
    </div>
  {/if}

  <div class="thread" bind:this={scroller} {onscroll}>
    <!--
      The thread still scrolls while it's inert. Working in it (a click, a card focused) lets go of the foot, so what
      you opened or folded stays where it is; sending an ask, or scrolling back to the foot, takes hold again.
    -->
    <div class={["flow", !ready && "stale"]} inert={!ready} bind:this={flow} {@attach letGo}>
      <ChatJob
        {review}
        {company}
        {logoSrc}
        {salary}
        root={scroller}
        bind:salaryAnchor
        bind:logoMark
        {onsalary}
        {canSalary}
      />

      <div class="turns">
        {#key s.arrival}
          <article class={["agent", s.arrival !== firstArrival && "enter"]} aria-label={planModel}>
            {@render identity(planModel)}
            {#if review.headline}<p class="prose">{review.headline}</p>{/if}
            {#if plan.length}
              <PlanCard {s} {review} items={plan} {ready} />
            {:else}
              <p class="prose">No suggested edits for your {review.master.title} master. Ask for a change below.</p>
            {/if}
          </article>
        {/key}

        {#each s.turns as turn, ti (turn.id)}
          {@const live = turn.at > openedAt}
          {@const pending = pendingOf(turn)}
          {#if editing?.id === turn.id}
            <div class="editor">
              <MentionField
                class="editor__field"
                label="Your ask"
                initial={{ text: editing.text, scope: editing.scope }}
                autofocus
                oninput={(draft) => {
                  if (editing) ({ text: editing.text, scope: editing.scope } = draft);
                }}
                onkeydown={(e: KeyboardEvent) => {
                  if (e.isComposing) return;
                  if (e.key === "Escape") {
                    e.preventDefault();
                    e.stopPropagation();
                    editing = null;
                  } else if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    sendEdit(turn);
                  }
                }}
              />
              <div class="editor__foot">
                <span class="meta editor__note">{rewindNote(turn) ?? "Its answer is replaced by the new one."}</span>
                <Button variant="ghost" size="sm" kbd="Esc" onclick={() => (editing = null)}>Cancel</Button>
                <Button
                  variant="primary"
                  size="sm"
                  kbd="⏎"
                  disabled={!ready || busy || !editing.text.trim()}
                  onclick={() => sendEdit(turn)}>Send</Button
                >
              </div>
            </div>
          {:else}
            <div class={["you", live && "enter"]}>
              <p class="bubble">
                {#each pieces(turn.ask, turn.scope) as piece, i (i)}{#if typeof piece === "string"}{piece}{:else}<MentionChip
                      label={piece.label}
                      quote={piece.quote}
                    />{/if}{/each}
                {#if turn.confirmed?.length}<span class="bubble__confirmed"
                    ><Icon icon={Check} size={14} weight="medium" />You have {turn.confirmed.join(", ")} experience</span
                  >{/if}
              </p>
              <span class="acts acts--you">
                <Button
                  variant="ghost"
                  size="sm"
                  square
                  icon={Pencil}
                  aria-label="Edit"
                  {@attach tip("Edit and send again from here")}
                  disabled={!ready || busy}
                  onclick={() => startEdit(turn)}
                />
                <Button
                  variant="ghost"
                  size="sm"
                  square
                  icon={Copy}
                  aria-label="Copy"
                  {@attach tip("Copy")}
                  onclick={() => copy(turn.ask)}
                />
              </span>
            </div>
          {/if}

          {#if turn.status !== "running"}
            <article class={["agent", live && "enter"]} aria-label={turnModel(turn)}>
              {@render identity(turnModel(turn), time(turn.at + turn.ms), turn)}
              {#if confirming === turn.id}
                {@const note = rewindNote(turn)}
                <div class="confirm" role="group" aria-label="Retry from here">
                  <p class="confirm__text">Retry from this ask? {note}</p>
                  <Button size="sm" icon={RotateCcw} disabled={!ready || busy} onclick={() => retry(turn)}
                    >Retry</Button
                  >
                  <Button variant="ghost" size="sm" onclick={() => (confirming = null)}>Cancel</Button>
                </div>
              {/if}
              {#if turn.status === "failed"}
                <p class="error" role="alert">{turn.error ?? "The request failed."}</p>
                <div class="row">
                  <Button
                    size="sm"
                    icon={RotateCcw}
                    disabled={!ready || busy}
                    onclick={() => retry(turn)}>Try again</Button
                  >
                </div>
              {:else if turn.status === "stopped"}
                <p class="meta">Stopped. Nothing was changed.</p>
              {:else}
                {#if turn.reply}
                  <p class="prose">{turn.reply}</p>
                {:else if !turn.proposals.length && !turn.skipped.length}
                  <p class="prose">Nothing to change for that.</p>
                {/if}

                {#if turn.proposals.length || turn.skipped.length}
                  <div class="cards">
                    {#each turn.proposals as p, pi (pi)}
                      {@const change = find(review, p.key)}
                      <DiffCard
                        {s}
                        {review}
                        {p}
                        n={firsts[ti] + pi}
                        place={placeOf(
                          change?.target ?? editTarget(p.edit),
                          groupOf(p.key) ?? KINDS[p.edit.kind],
                          targets,
                        )}
                        {ready}
                        onmove={move}
                        onescape={() => composer?.focus()}
                      />
                    {/each}
                    {#each turn.skipped as skip, i (i)}
                      <div class="left-out">
                        <Icon icon={TriangleAlert} weight="medium" />
                        <div class="left-out__text">
                          {#if skip.term && !skip.answer}
                            <p class="left-out__what">{skip.what}</p>
                            {#if skip.why}<p class="left-out__why">{skip.why}</p>{/if}
                            <div class="left-out__ask">
                              <span class="left-out__question">Do you have {skip.term} experience?</span>
                              <Button size="sm" icon={Check} disabled={!ready || busy} onclick={() => answer(turn, i, true)}
                                >Yes</Button
                              >
                              <Button variant="ghost" size="sm" disabled={!ready} onclick={() => answer(turn, i, false)}
                                >No</Button
                              >
                            </div>
                          {:else}
                            <p class="left-out__what">{skip.what} <span>· left out</span></p>
                            {#if skip.answer === "yes"}
                              <p class="left-out__why">You have {skip.term} experience: asked again with it below</p>
                            {:else}
                              {#if skip.why}<p class="left-out__why">{skip.why}</p>{/if}
                              {#if skip.answer === "no"}<p class="left-out__why">You don’t have {skip.term} experience</p>{/if}
                            {/if}
                          {/if}
                        </div>
                      </div>
                    {/each}
                  </div>
                {/if}

                {#if pending.length >= 2}
                  <div class="row">
                    <Button size="sm" icon={CheckCheck} disabled={!ready} onclick={() => s.accept(pending)}
                      >Accept all</Button
                    >
                    <Button variant="ghost" size="sm" icon={X} disabled={!ready} onclick={() => s.reject(pending)}
                      >Reject all</Button
                    >
                    <span class="meta row__end num">{pending.length} waiting on you</span>
                  </div>
                {/if}

                <div class={["checks", ti !== newest && "checks--quiet"]}>
                  {#if ti === newest}
                    {#if keywords.of}
                      <span
                        class="check"
                        role="img"
                        aria-label={keywordLabel(keywords)}
                        {@attach tip(() => keywordTip(keywords))}
                      >
                        <Icon icon={Tag} size={14} weight="medium" />
                        <span class="check__name">Keywords</span>
                        <span class="num check__value">{covered} of {keywords.of}</span>
                        <span class="meter">
                          {#each { length: keywords.of }, i (i)}
                            <span
                              class={[
                                "cell",
                                i < keywords.have.length && "cell--have",
                                i >= keywords.have.length && i < covered && "cell--add",
                              ]}
                            ></span>
                          {/each}
                        </span>
                      </span>
                    {/if}
                    <span class={["check", fits ? "check--ok" : "check--over"]}>
                      <Icon icon={fits ? FileCheck : FileWarning} size={14} />
                      <span class="check__name">{pageNote}</span>
                    </span>
                  {/if}
                  <span class="meta num check__took">{Math.max(1, Math.round(turn.ms / 1000))} s</span>
                </div>
              {/if}
            </article>
          {/if}
        {/each}
      </div>
    </div>
  </div>

  {#if away}
    <div class="latest">
      <span class="latest__lift">
        <Button
          icon={ArrowDown}
          square={!unseen}
          aria-label={unseen ? undefined : "To the latest"}
          {@attach tip("To the latest (↓)")}
          onclick={() => {
            latest();
            composer?.focus();
          }}>{#if unseen}New answer{/if}</Button
        >
      </span>
    </div>
  {/if}

  <div class={["foot", !ready && "stale"]} inert={!ready}>
    <Composer
      bind:this={composer}
      {s}
      {review}
      {ready}
      {targets}
      keys={[...CHAT_KEYS, ...keys]}
      onsend={follow}
      onlast={last}
      onlatest={latest}
    />
  </div>
</div>

{#snippet identity(model: string, at?: string, turn?: Turn)}
  <div class="who">
    <CompanyMark size={20} src="/agents/claude.png" /><span class="who__name">{model}</span>{#if at}<span
        class="meta num">{at}</span
      >{/if}
    <span class="acts">
      {#if turn}
        {#if turn.reply}
          <Button
            variant="ghost"
            size="sm"
            square
            icon={Copy}
            aria-label="Copy the answer"
            {@attach tip("Copy")}
            onclick={() => copy(turn.reply)}
          />
        {/if}
        <Button
          variant="ghost"
          size="sm"
          square
          icon={RotateCcw}
          aria-label="Retry"
          {@attach tip("Retry: ask again from here")}
          disabled={!ready || busy}
          onclick={() => retry(turn)}
        />
      {:else}
        <Button
          variant="ghost"
          size="sm"
          square
          icon={RotateCcw}
          aria-label="Suggest again"
          {@attach tip("Suggest again (your edits stay)")}
          disabled={!ready || busy}
          onclick={() => void s.suggest(s.master, s.mode, true)}
        />
      {/if}
    </span>
  </div>
{/snippet}

<style>
  .chat {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .stale {
    opacity: 0.5;
  }
  .running {
    flex: none;
    padding: var(--space-3) var(--space-6);
    background: var(--paper);
    box-shadow: inset 0 -1px 0 var(--line);
  }

  /*
   * The thread never grows past the column: every grid here is one minmax(0, 1fr) column, so a long line (a chip, a
   * card's head) truncates or wraps inside it instead of widening the track.
   */
  .thread {
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden auto;
    overscroll-behavior: contain;
    /* The strip lies over the top 48px: a card scrolled into view clears it. */
    scroll-padding-top: 48px;
  }
  /* While the thread's foot is out of view, the button back to it: no height of its own, it rises over the thread. */
  .latest {
    position: relative;
    z-index: 3;
    flex: none;
    display: flex;
    justify-content: center;
    align-items: flex-end;
    height: 0;
  }
  /* Lifted off the thread as a popover is; fades in, goes at once. */
  .latest__lift {
    display: flex;
    margin-bottom: var(--space-3);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-popover);
    transition: opacity var(--dur-instant) var(--ease-out);
  }
  @starting-style {
    .latest__lift {
      opacity: 0;
    }
  }

  .flow,
  .turns,
  .agent,
  .cards {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    min-width: 0;
  }
  .turns {
    gap: var(--space-5);
    padding: var(--space-6) var(--space-6) var(--space-8);
  }
  .agent {
    gap: var(--space-3);
  }
  .who {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: var(--control-sm);
    margin: 0;
  }
  .who__name {
    font-weight: var(--weight-medium);
    color: var(--ink);
  }
  /*
   * A message's actions (copy, retry, edit): shown on hover or focus where there's hover, always on touch; hidden,
   * they keep their place, so nothing moves when they show.
   */
  .acts {
    display: flex;
    flex: none;
    gap: var(--space-0-5);
    margin-inline-start: auto;
    transition: opacity var(--dur-instant) var(--ease-out);
  }
  @media (hover: hover) {
    .acts {
      opacity: 0;
    }
    .agent:hover .acts,
    .agent:focus-within .acts,
    .you:hover .acts,
    .you:focus-within .acts {
      opacity: 1;
    }
  }
  /* Retry when it takes back more than the answer: what goes, then Retry and Cancel. */
  .confirm {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2) var(--space-1);
    padding: var(--space-2-5) var(--space-3);
    border-radius: var(--radius-lg);
    border: 1px solid transparent;
    background: var(--surface);
    box-shadow: var(--shadow-border);
  }
  .confirm__text {
    flex: 1 1 100%;
    margin: 0;
    color: var(--ink-2);
    text-wrap: pretty;
  }
  /* What the agent says, at the reading size. */
  .prose {
    margin: 0;
    font-size: var(--text-md);
    line-height: var(--leading-md);
    letter-spacing: var(--tracking-md);
    color: var(--ink);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    text-wrap: pretty;
  }
  .cards {
    gap: var(--space-2);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1);
  }
  .row__end {
    margin-inline-start: auto;
  }
  .error {
    margin: 0;
    color: var(--danger);
    text-wrap: pretty;
  }

  /* Your ask: on the trailing side, in a well, its mentions inline before the text; its actions before it. */
  .you {
    display: flex;
    justify-content: flex-end;
    align-items: flex-end;
    gap: var(--space-1);
    padding-inline-start: var(--space-6);
    min-width: 0;
  }
  .acts--you {
    order: -1;
    margin-inline-start: 0;
  }
  /* Editing an ask in place: the field (its parts in its words), then what sending again takes back, Cancel and Send. */
  .editor {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--space-2);
    padding-inline-start: var(--space-12);
  }
  /* As `Textarea size="lg"` at two lines: an input's edge, its focus ring. */
  .editor :global(.editor__field) {
    min-height: calc(2 * var(--leading-md) + 2 * var(--space-2) + 2px);
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-sm);
    border: 1px solid var(--input-border);
    background: var(--surface);
    transition-property: border-color, box-shadow;
    transition-duration: var(--dur-fast);
    transition-timing-function: var(--ease-out);
  }
  .editor :global(.editor__field:focus-visible) {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px oklch(from var(--accent) l c h / 0.18);
  }
  .editor__foot {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }
  .editor__note {
    flex: 1;
    min-width: 0;
    text-wrap: pretty;
  }
  .bubble {
    min-width: 0;
    margin: 0;
    padding: var(--space-2-5) var(--space-3);
    border-radius: var(--radius-lg);
    border: 1px solid transparent;
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
    font-size: var(--text-md);
    line-height: var(--leading-md);
    letter-spacing: var(--tracking-md);
    color: var(--ink);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    text-wrap: pretty;
  }
  /* The skills you said you have, under the ask they were sent with. */
  .bubble__confirmed {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    margin-top: var(--space-1);
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    letter-spacing: normal;
    color: var(--ink-2);
  }
  .bubble__confirmed :global(.icon) {
    flex: none;
    color: var(--gain-text);
  }

  /* Left out: amber's wash, the triangle, what and why; a skill your resume doesn't show asks whether you have it. */
  .left-out {
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr);
    gap: var(--space-2);
    padding: var(--space-2-5) var(--space-3);
    border-radius: var(--radius-lg);
    background: var(--warn-soft);
  }
  .left-out > :global(.icon) {
    margin-top: 2px;
    color: var(--warn);
  }
  .left-out__text p {
    margin: 0;
    text-wrap: pretty;
  }
  .left-out__what {
    font-weight: var(--weight-medium);
    color: var(--ink);
  }
  .left-out__what span {
    color: var(--warn);
  }
  .left-out__why {
    margin-top: var(--space-0-5);
    color: var(--ink-2);
  }
  .left-out__ask {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1);
    margin-top: var(--space-2);
  }
  .left-out__question {
    margin-inline-end: var(--space-1);
    font-weight: var(--weight-medium);
    color: var(--ink);
  }

  /* Where the page stands: keywords with their meter, the page fit, the seconds the turn took. */
  .checks {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1) var(--space-4);
    min-height: 40px;
    padding: var(--space-1) var(--space-3);
    border-radius: var(--radius-lg);
    border: 1px solid transparent;
    background: var(--surface);
    box-shadow: var(--shadow-border);
    white-space: nowrap;
  }
  .checks--quiet {
    min-height: 0;
    padding: 0;
    background: none;
    box-shadow: none;
  }
  .check {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }
  .check :global(.icon) {
    color: var(--ink-3);
  }
  .check__name {
    font-weight: var(--weight-medium);
  }
  .check__value {
    font-size: var(--text-sm);
    font-weight: var(--weight-medium);
    color: var(--ink-2);
  }
  .check--ok :global(.icon) {
    color: var(--gain-text);
  }
  .check--over :global(.icon),
  .check--over .check__name {
    color: var(--warn);
  }
  .check__took {
    margin-inline-start: auto;
  }
  .meter {
    display: flex;
    gap: 2px;
  }
  /* A cell a keyword: an edge (empty), filled (yours), gain's wash in a gain edge (added by an edit). */
  .cell {
    width: 6px;
    height: 12px;
    border-radius: 2px;
    border: 1px solid var(--line-strong);
    background: var(--sunken);
  }
  .cell--have {
    border-color: transparent;
    background: var(--gain);
  }
  .cell--add {
    border-color: var(--gain);
    background: var(--gain-soft);
  }

  .foot {
    flex: none;
  }
</style>
