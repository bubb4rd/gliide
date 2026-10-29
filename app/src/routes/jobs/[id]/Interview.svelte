<!--
  The job's Interview tab (DESIGN.md, Job → Interview), shown once the posting is applied to: the job, with its next
  round where the review shows the score (folding to a strip once it scrolls away, or when you fold it), then its
  rounds, then the prep for the round in focus (`Prep`). A round is added in a form in place (`A`), filled from an
  invite you paste (`⌘V`, one model call) or a calendar file (`.ics`) you drop on the window; edited in place (`⏎`)
  and removed (`⌫`, Undo in the toast). The first upcoming round of a Saved or Applied posting moves it to
  Interviewing, with Undo; adding a round while reminders were never offered offers them once, inline. Practice (`P`)
  turns the column into the round's practice (`Practice`). A round that has ended awaits its debrief (`D`, or its
  Debrief badge): the debrief (`Debrief`) takes the prep's place; the tab opens on it when the latest round awaits
  one, or for `?debrief=<round>`. The desk (Review's) shows the resume you sent, the lines the selected question is
  about marked (in a debrief, the checked questions').
-->
<script lang="ts">
  import { untrack } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { readText } from "@tauri-apps/plugin-clipboard-manager";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { page } from "$app/state";
  import Bell from "@lucide/svelte/icons/bell";
  import CalendarPlus from "@lucide/svelte/icons/calendar-plus";
  import Check from "@lucide/svelte/icons/check";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import ChevronUp from "@lucide/svelte/icons/chevron-up";
  import ClipboardPaste from "@lucide/svelte/icons/clipboard-paste";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import {
    appError,
    cancel,
    openNotificationSettings,
    practiceTurns,
    prep as loadPrep,
    readInvite,
    readInviteFile,
    removeRound,
    requestNotifications,
    rounds as loadRounds,
    saveRound,
    type ApplicationRow,
    type NotificationPermission,
    type PracticeTurn,
    type PrepQuestion,
    type PrepView,
    type Round,
    type RoundDraft,
    type RoundKind,
    type RoundsView,
    type SentLine,
  } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import { awaitingDebrief, endsAt, kindLabel, ROUND_KINDS, roundLine, when } from "$lib/rounds";
  import { app } from "$lib/stores/app.svelte";
  import {
    Badge,
    Button,
    CompanyMark,
    GroupHead,
    Icon,
    Input,
    KeyFooter,
    type KeyCommand,
    Menu,
    PartRow,
    Textarea,
    Thinking,
    tip,
    toast,
  } from "$lib/ui";
  import Debrief from "./Debrief.svelte";
  import Practice from "./Practice.svelte";
  import Prep from "./Prep.svelte";

  interface Props {
    /** The posting. */
    job: string;
    application: ApplicationRow;
    company: string;
    role: string;
    logoSrc: string | null;
    /** The screen's keys, listed after the tab's own in Commands. */
    keys: KeyCommand[];
    /** Opens the add form once (the status just moved to Interviewing, or `?add=round`); set back to false. */
    adding?: boolean;
    /** The lines of the resume you sent (the desk's), to name what a question is about. */
    lines: SentLine[];
    /** The lines the desk marks (bindable): the selected question's. */
    marked?: string[];
    /** Practicing the round in focus (bindable; the toolbar's Practice and Back to prep). */
    practicing?: boolean;
    /** There's a prep to practice (bindable; the toolbar shows Practice). */
    canPractice?: boolean;
    /** The round whose debrief is open (bindable; the toolbar's Save debrief). */
    debriefing?: string | null;
    /** The open debrief has something to save (bindable; the toolbar's Save debrief). */
    canSaveDebrief?: boolean;
    /** The round in focus has ended and awaits its debrief (bindable; the toolbar shows Debrief). */
    debriefDue?: string | null;
  }

  let {
    job,
    application,
    company,
    role,
    logoSrc,
    keys,
    adding = $bindable(false),
    lines,
    marked = $bindable([]),
    practicing = $bindable(false),
    canPractice = $bindable(false),
    debriefing = $bindable(null),
    canSaveDebrief = $bindable(false),
    debriefDue = $bindable(null),
  }: Props = $props();

  const LENGTHS: { value: number | null; label: string }[] = [
    { value: 15, label: "15 min" },
    { value: 30, label: "30 min" },
    { value: 45, label: "45 min" },
    { value: 60, label: "1 hour" },
    { value: 90, label: "1.5 hours" },
    { value: 120, label: "2 hours" },
    { value: 180, label: "3 hours" },
    { value: 240, label: "4 hours" },
    { value: null, label: "Not sure" },
  ];
  /** The strip's height: the card counts as out of view once it's gone under it. */
  const STRIP = 48;

  let list = $state<Round[] | null>(null);
  let failed = $state<string | null>(null);
  let busy = $state(false);
  let now = $state(new Date());

  const next = $derived(application.nextRound);
  /** Upcoming rounds whose end has passed. */
  const over = (r: Round) => r.state === "upcoming" && (endsAt(r) ?? Infinity) < now.getTime();

  // ── Loading ──
  const applicationId = $derived(application.id);
  $effect(() => {
    const id = applicationId;
    untrack(() => void load(id));
  });

  async function load(id: string) {
    failed = null;
    try {
      const rows = await loadRounds(id);
      if (id !== application.id) return;
      list = rows;
      if (!opened) {
        opened = true;
        openFirst(rows);
      }
    } catch (e) {
      failed = appError(e).message;
    }
  }

  // ── Debriefs ──
  let opened = false;
  let debriefPane = $state<ReturnType<typeof Debrief>>();
  const debriefRound = $derived(list?.find((r) => r.id === debriefing && r.state === "upcoming") ?? null);
  $effect(() => {
    debriefDue = !debriefing && focus && awaitingDebrief(focus, now) ? focus.id : null;
  });

  /** Opens on a debrief: the round `?debrief=` names, else the latest round when it awaits one. */
  function openFirst(rows: Round[]) {
    const named = rows.find((r) => r.id === (debriefing ?? page.url.searchParams.get("debrief")));
    const latest = rows
      .filter((r) => r.state === "upcoming" && r.startsAt)
      .sort((a, b) => (endsAt(b) ?? 0) - (endsAt(a) ?? 0))[0];
    const round = named ?? (latest && awaitingDebrief(latest, now) ? latest : undefined);
    if (round) openDebrief(round);
  }

  function openDebrief(r: Round) {
    practicing = false;
    form = null;
    selected = r.id;
    question = null;
    debriefing = r.id;
  }

  function leaveDebrief() {
    debriefing = null;
    marked = [];
  }

  /** The toolbar's Save debrief (`⌘⏎` is the debrief's own). */
  export function saveDebrief() {
    void debriefPane?.save();
  }

  /** Saved: the round is debriefed, and the application's debrief due and last activity have moved. */
  function debriefSaved() {
    void load(application.id);
    void app.refreshApplications();
    now = new Date();
  }

  // ── Reminders, offered once under the first round you add (the round you added) ──
  let offering = $state<Round | null>(null);
  const offerable = $derived(
    app.settings !== null && !app.settings.notifications.offered && !app.settings.notifications.interviews,
  );

  async function turnOn() {
    let permission: NotificationPermission = "unknown";
    try {
      permission = await requestNotifications();
    } catch {
      // Settings still records it on; Settings → Notifications says what macOS allows.
    }
    const settings = app.settings;
    if (!settings) return;
    const live = list?.filter((r) => r.state === "upcoming") ?? [];
    const added = offering;
    const what = live.length <= 1 || !added ? "first round" : kindLabel(added.kind).toLowerCase();
    try {
      await app.saveSettings({
        ...settings,
        notifications: {
          ...settings.notifications,
          interviews: true,
          dayBefore: true,
          hourBefore: true,
          debrief: false,
          turnedOn: new Date().toISOString(),
          turnedOnWith: `${company}’s ${what}`,
          offered: true,
        },
      });
      offering = null;
      if (permission === "denied") {
        toast("Reminders are on, but macOS doesn’t allow gliide’s banners", {
          action: { label: "Open macOS settings", run: () => void openNotificationSettings() },
        });
      } else {
        toast("Reminders on: the day before and an hour before");
      }
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
    }
  }

  async function notNow() {
    const settings = app.settings;
    offering = null;
    if (!settings) return;
    try {
      await app.saveSettings({ ...settings, notifications: { ...settings.notifications, offered: true } });
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
    }
  }

  /** A saved or removed round's answer: the rounds, and the application as it stands (its status may have moved). */
  function show(view: RoundsView) {
    list = view.rounds;
    app.putApplications([view.application]);
    now = new Date();
  }

  // ── The round in focus, its prep and its practice ──
  /** The selected round when it's to come, else the next one, else the first still upcoming. */
  const focus = $derived(
    list?.find((r) => r.id === selected && r.state === "upcoming") ??
      list?.find((r) => r.id === next?.id) ??
      list?.find((r) => r.state === "upcoming" && !over(r)),
  );
  const focusId = $derived(focus?.id ?? null);
  let prep = $state<PrepView | null>(null);
  let turns = $state<PracticeTurn[]>([]);
  let writing = $state<number | null>(null);
  let prepFailed = $state<string | null>(null);
  let question = $state<string | null>(null);
  const practiced = $derived(new Set(turns.map((t) => t.question)));

  $effect(() => {
    const id = focusId;
    untrack(() => void loadFor(id, false));
  });
  $effect(() => {
    canPractice = prep !== null && prep.questions.length > 0 && writing === null;
  });

  async function loadFor(round: string | null, again: boolean) {
    prepFailed = null;
    if (!again) {
      prep = null;
      turns = [];
      question = null;
      marked = [];
    }
    if (!round) return;
    practiceTurns(round)
      .then((t) => focusId === round && (turns = t))
      .catch(() => {});
    writing = performance.now();
    try {
      const p = await loadPrep(round, again);
      if (focusId === round) prep = p;
    } catch (e) {
      const error = appError(e);
      if (focusId === round && error.kind !== "cancelled") prepFailed = error.message;
    } finally {
      if (focusId === round) writing = null;
    }
  }

  function choose(q: PrepQuestion) {
    question = q.id;
    marked = q.lines;
  }

  function startPractice(q?: PrepQuestion) {
    if (!prep?.questions.length) return;
    const first = q ?? prep.questions.find((x) => x.id === question) ?? prep.questions.find((x) => !practiced.has(x.id));
    question = (first ?? prep.questions[0]).id;
    practicing = true;
  }

  // Practice starts on the question you chose, else the first you haven't practiced; the desk marks what it's about.
  $effect(() => {
    if (!practicing || !prep?.questions.length) return;
    if (!question) question = (prep.questions.find((q) => !practiced.has(q.id)) ?? prep.questions[0]).id;
    marked = prep.questions.find((q) => q.id === question)?.lines ?? [];
  });

  // ── The form: a new round (`key` "new") or one being edited ──
  interface Form {
    key: string;
    id: string | null;
    kind: RoundKind;
    date: string;
    time: string;
    minutes: number | null;
    with: string;
    place: string;
    notes: string;
    /** Filled from an invite you pasted or dropped: its fields are for you to check. */
    from: "pasted" | "file" | null;
  }
  let form = $state<Form | null>(null);
  let problem = $state<string | null>(null);

  const pad = (n: number) => String(n).padStart(2, "0");

  /** A round's start as the date and time fields hold it, in this Mac's time. */
  function fields(startsAt: string | null): { date: string; time: string } {
    if (!startsAt) return { date: "", time: "" };
    const d = new Date(startsAt);
    return {
      date: `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`,
      time: `${pad(d.getHours())}:${pad(d.getMinutes())}`,
    };
  }

  /** The fields' date and time as RFC 3339 in this Mac's offset: "2026-11-03T14:00:00-06:00". */
  function startOf(date: string, time: string): string {
    const d = new Date(`${date}T${time}`);
    const offset = -d.getTimezoneOffset();
    const sign = offset >= 0 ? "+" : "-";
    return `${date}T${time}:00${sign}${pad(Math.floor(Math.abs(offset) / 60))}:${pad(Math.abs(offset) % 60)}`;
  }

  function open(draft: RoundDraft | null, key = "new", from: Form["from"] = null) {
    form = {
      key,
      id: draft?.id ?? null,
      kind: draft?.kind ?? "other",
      ...fields(draft?.startsAt ?? null),
      minutes: draft?.minutes ?? null,
      with: draft?.with ?? "",
      place: draft?.place ?? "",
      notes: draft?.notes ?? "",
      from,
    };
    problem = null;
  }

  const startAdd = () => open(null);
  const startEdit = (r: Round) => {
    selected = r.id;
    open(r, r.id);
  };

  $effect(() => {
    if (adding) {
      adding = false;
      untrack(startAdd);
    }
  });

  async function save() {
    const f = form;
    if (!f || busy) return;
    if (f.date && !f.time) return void (problem = "Add the time, or clear the date");
    if (f.time && !f.date) return void (problem = "Add the date, or clear the time");
    const draft: RoundDraft = {
      id: f.id,
      kind: f.kind,
      startsAt: f.date ? startOf(f.date, f.time) : null,
      minutes: f.minutes,
      with: f.with.trim(),
      place: f.place.trim(),
      notes: f.notes.trim(),
    };
    busy = true;
    const had = new Set(list?.map((r) => r.id));
    const was = application;
    try {
      const view = await saveRound(application.id, draft);
      show(view);
      form = null;
      const saved = view.rounds.find((r) => (draft.id ? r.id === draft.id : !had.has(r.id)));
      if (saved) selected = saved.id;
      if (saved && !draft.id && offerable) offering = saved;
      if (view.movedFrom) {
        const from = view.movedFrom;
        toast(`Moved to Interviewing${saved ? ` · ${roundLine(saved)}` : ""}`, {
          action: { label: "Undo", run: () => void app.setStatus([{ ...was, status: "interviewing" }], from) },
        });
      }
    } catch (e) {
      problem = appError(e).message;
    } finally {
      busy = false;
    }
  }

  async function remove(r: Round) {
    if (busy) return;
    busy = true;
    try {
      show(await removeRound(r.id));
      toast("Removed the round", {
        action: {
          label: "Undo",
          run: async () => {
            try {
              const { id, kind, startsAt, minutes, with: who, place, notes } = r;
              show(await saveRound(application.id, { id, kind, startsAt, minutes, with: who, place, notes }));
              selected = r.id;
            } catch (e) {
              toast(appError(e).message, { tone: "danger" });
            }
          },
        },
      });
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
    } finally {
      busy = false;
    }
  }

  function onFieldKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      form = null;
    } else if (e.key === "Enter" && (e.metaKey || !(e.target instanceof HTMLTextAreaElement))) {
      e.preventDefault();
      void save();
    }
  }

  // ── Invites: pasted (one model call) or dropped as a calendar file ──
  let reading = $state<number | null>(null);

  async function pasteInvite() {
    if (reading !== null || busy) return;
    const text = (await readText().catch(() => ""))?.trim();
    if (!text) return void toast("The clipboard has no text. Copy the invite first.", { tone: "neutral" });
    reading = performance.now();
    try {
      open(await readInvite(application.id, text), "new", "pasted");
    } catch (e) {
      const error = appError(e);
      if (error.kind !== "cancelled") toast(error.message, { tone: "danger" });
    } finally {
      reading = null;
    }
  }

  $effect(() => {
    let stopped = false;
    let unlisten: (() => void) | undefined;
    void getCurrentWebview()
      .onDragDropEvent(({ payload }) => {
        if (payload.type !== "drop") return;
        const path = payload.paths.find((p) => p.toLowerCase().endsWith(".ics"));
        if (!path) return;
        readInviteFile(path)
          .then((draft) => open(draft, "new", "file"))
          .catch((e) => toast(appError(e).message, { tone: "danger" }));
      })
      .then((stop) => {
        if (stopped) stop();
        else unlisten = stop;
      });
    return () => {
      stopped = true;
      unlisten?.();
    };
  });

  // ── Card or strip (as the review's job card) ──
  let scroller = $state<HTMLElement>();
  let card = $state<HTMLElement>();
  let gone = $state(false);
  let folded = $state(false);
  const strip = $derived(folded || gone);

  $effect(() => {
    if (!card || !scroller) return;
    const observer = new IntersectionObserver(([entry]) => (gone = !entry.isIntersecting), {
      root: scroller,
      rootMargin: `-${STRIP}px 0px 0px 0px`,
    });
    observer.observe(card);
    return () => observer.disconnect();
  });

  function unfold() {
    folded = false;
    if (scroller) scroller.scrollTop = 0;
  }

  // ── Selection and keys ──
  let selected = $state<string | null>(null);
  const selectedRound = $derived(list?.find((r) => r.id === selected));

  const interactive = (target: EventTarget | null) =>
    target instanceof HTMLElement && target.closest("button, a, input, textarea, select, [contenteditable]") !== null;

  /** ↑↓ go through the rounds, then the prep's questions (the rounds alone in a debrief). */
  function step(by: number) {
    const items = [
      ...(list ?? []).map((r) => ({ round: r.id, q: null as PrepQuestion | null })),
      ...(debriefRound ? [] : (prep?.questions ?? [])).map((q) => ({ round: null, q })),
    ];
    if (!items.length) return;
    const at = question ? items.findIndex((i) => i.q?.id === question) : items.findIndex((i) => i.round === selected);
    const to = items[Math.min(Math.max(at + by, 0), items.length - 1)];
    if (to.q) choose(to.q);
    else selectRound(to.round!);
  }

  /** Selects a round; choosing another leaves the debrief for later. */
  function selectRound(id: string) {
    question = null;
    marked = [];
    selected = id;
    if (debriefing && id !== debriefing) leaveDebrief();
  }

  const chosen = $derived(prep?.questions.find((q) => q.id === question));

  const KEYS: KeyCommand[] = $derived([
    ...(debriefRound
      ? [
          { keys: ["⌘⏎"], label: "Save debrief", run: saveDebrief, disabled: !canSaveDebrief },
          { keys: ["Esc"], label: "Leave it for later", run: leaveDebrief },
        ]
      : [
          ...(debriefDue
            ? [{ keys: ["D"], label: "Debrief", run: () => focus && openDebrief(focus) }]
            : []),
          { keys: ["P"], label: "Practice", run: () => startPractice(chosen), disabled: !canPractice },
        ]),
    { keys: ["A"], label: "Add interview", run: startAdd, disabled: busy },
    { keys: ["⌘V"], label: "Paste an invite", run: () => void pasteInvite(), disabled: reading !== null },
    { keys: ["↑", "↓"], label: debriefRound ? "Select a round" : "Select a round or question" },
    {
      keys: ["⏎"],
      label: chosen ? "Practice this question" : "Edit round",
      run: () => (chosen ? startPractice(chosen) : selectedRound && startEdit(selectedRound)),
      disabled: !chosen && !selectedRound,
    },
    {
      keys: ["⌫"],
      label: "Remove round",
      run: () => void (selectedRound && remove(selectedRound)),
      disabled: !selectedRound || chosen !== undefined,
    },
    ...keys,
  ]);

  /** The toolbar's Debrief: the round in focus. */
  export function debriefNow() {
    if (debriefDue && focus) openDebrief(focus);
  }

  $effect(() => {
    if (practicing) return;
    return bindKeys({
      d: (e) => (interactive(e.target) || !debriefDue || !focus ? false : openDebrief(focus)),
      p: (e) => (interactive(e.target) || !canPractice || debriefRound ? false : startPractice(chosen)),
      a: (e) => (interactive(e.target) ? false : startAdd()),
      "mod+v": (e) => (interactive(e.target) ? false : void pasteInvite()),
      ArrowDown: (e) => (interactive(e.target) ? false : step(1)),
      ArrowUp: (e) => (interactive(e.target) ? false : step(-1)),
      Enter: (e) => {
        if (interactive(e.target)) return false;
        if (chosen) return startPractice(chosen);
        if (!selectedRound) return false;
        startEdit(selectedRound);
      },
      Backspace: (e) =>
        interactive(e.target) || !selectedRound || chosen ? false : void remove(selectedRound),
    });
  });

  // ── Words ──
  /** "Tue, Nov 3 · 2:00 PM · 1 hour", or "Not scheduled". */
  function whenLine(r: Round): string {
    if (!r.startsAt) return "Not scheduled";
    const d = new Date(r.startsAt);
    const day = d.toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric" });
    const time = d.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
    const length = LENGTHS.find((l) => l.value === r.minutes && l.value !== null)?.label ?? (r.minutes ? `${r.minutes} min` : null);
    return [day, time, length].filter(Boolean).join(" · ");
  }

  /** "Tomorrow", "In 3 days", "Today": how soon the next round is. */
  function soon(r: Round): string {
    const d = new Date(r.startsAt ?? 0);
    const days = Math.round(
      (new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime() -
        new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime()) /
        86_400_000,
    );
    return days <= 0 ? "Today" : days === 1 ? "Tomorrow" : `In ${days} days`;
  }

  const isLink = (place: string) => /^https?:\/\//i.test(place);
  const host = (place: string) => {
    try {
      return new URL(place).host.replace(/^www\./, "");
    } catch {
      return place;
    }
  };
</script>

{#if practicing && prep && focus}
  <Practice
    {job}
    {company}
    round={focus}
    {prep}
    {lines}
    bind:turns
    bind:question={() => question ?? prep?.questions[0]?.id ?? "", (q) => (question = q)}
    {keys}
    onback={() => (practicing = false)}
  />
{:else}
<div class="scroller" bind:this={scroller}>
  <div class={["pin", folded && "pin--folded"]}>
    {#if strip}
      <div class="strip">
        <CompanyMark size={20} src={logoSrc} />
        <span class="strip__company">{company}</span>
        <span class="strip__role" {@attach tip(role)}>{role}</span>
        {#if next}<span class="strip__next num">{roundLine(next, now)}</span>{/if}
        <Button
          variant="ghost"
          size="sm"
          square
          icon={ChevronDown}
          aria-label="Show the job"
          {@attach tip("Show the job")}
          onclick={unfold}
        />
      </div>
    {/if}
  </div>

  {#if !folded}
    <header class="card" bind:this={card}>
      <div class="named">
        <span class="lead"><CompanyMark size={48} src={logoSrc} /></span>
        <p class="company">{company}</p>
        <h1 class="headline">{role}</h1>
        <span class="fold">
          <Button
            variant="ghost"
            size="sm"
            square
            icon={ChevronUp}
            aria-label="Fold the job"
            {@attach tip("Fold the job")}
            onclick={() => (folded = true)}
          />
        </span>
      </div>
      {#if next?.startsAt}
        <div class="next">
          <p class="title">{kindLabel(next.kind)}, {when(next.startsAt, now)}</p>
          <p class="meta next__meta">
            {[
              new Date(next.startsAt).toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric" }),
              LENGTHS.find((l) => l.value === next.minutes && l.value !== null)?.label,
              next.with,
            ]
              .filter(Boolean)
              .join(" · ")}{#if next.place}{" · "}{#if isLink(next.place)}<a
                  class="link"
                  href={next.place}
                  onclick={(e) => {
                    e.preventDefault();
                    void openUrl(next.place);
                  }}>Join on {host(next.place)}</a
                >{:else}{next.place}{/if}{/if}
          </p>
        </div>
      {:else}
        <p class="meta">
          {application.status === "interviewing" ? "Interviewing" : "Applied"} · sent {new Date(
            `${application.date}T12:00`,
          ).toLocaleDateString(undefined, { month: "short", day: "numeric" })}{application.onFile === null
            ? ""
            : ` · ${Math.round(application.onFile)}% on file`}
        </p>
      {/if}
    </header>
  {/if}

  <section class="rounds" aria-label="Rounds">
    <GroupHead label="Rounds" meta={list?.length ? String(list.length) : ""} />

    {#if reading !== null}
      <div class="reading">
        <Thinking label="Reading the invite…" startedAt={reading} oncancel={() => void cancel(job)} />
      </div>
    {/if}

    {#snippet editor()}
      {#if form}
        {@const f = form}
        <div class="form" role="group" aria-label={f.id ? "Edit the round" : "Add the interview"}>
          {#if f.from}
            <p class="meta note">
              {f.from === "pasted" ? "Filled from the invite you pasted" : "Filled from the calendar file you dropped"}.
              Check each field; nothing is saved until you {f.id ? "save" : "add"} it.
            </p>
          {/if}
          <span class="field-label">Kind</span>
          <div class="kind">
            <Menu value={f.kind} options={ROUND_KINDS} label="Kind of round" onchange={(k) => (f.kind = k)} />
          </div>
          <span class="field-label">When</span>
          <div class="when">
            <Input
              size="md"
              type="date"
              bind:value={f.date}
              aria-label="Date"
              onkeydown={onFieldKey}
              {@attach (element) => {
                if (f.key === "new") element.focus();
              }}
            />
            <Input size="md" type="time" bind:value={f.time} aria-label="Time" onkeydown={onFieldKey} />
            <Menu value={f.minutes} options={LENGTHS} label="Length" onchange={(m) => (f.minutes = m)} />
          </div>
          <span class="field-label">With</span>
          <Input size="md" bind:value={f.with} placeholder="Who you meet, like Priya Shah, Senior Engineer" onkeydown={onFieldKey} />
          <span class="field-label">Where</span>
          <Input size="md" bind:value={f.place} placeholder="Meeting link or address" onkeydown={onFieldKey} />
          <span class="field-label field-label--top">Notes</span>
          <Textarea bind:value={f.notes} placeholder="What to prepare, who else joins" onkeydown={onFieldKey} />
          {#if problem}<p class="meta problem" role="status">{problem}</p>{/if}
          <div class="form__foot">
            {#if !f.id}<span class="meta">Or paste an invite (⌘V), or drop its .ics on the window</span>{/if}
            <Button variant="ghost" size="sm" kbd="Esc" onclick={() => (form = null)}>Cancel</Button>
            <Button variant="primary" size="sm" kbd="⏎" disabled={busy} onclick={() => void save()}
              >{f.id ? "Save" : "Add interview"}</Button
            >
          </div>
        </div>
      {/if}
    {/snippet}

    {#if failed}
      <p class="meta problem" role="alert">{failed}</p>
    {:else if list}
      {#each list as round (round.id)}
        <PartRow
          label={kindLabel(round.kind)}
          selected={round.id === selected && question === null}
          onselect={() => selectRound(round.id)}
          editor={form?.key === round.id ? editor : undefined}
        >
          {#snippet text()}
            <div class="round">
              <div class="round__body">
                <p class={["words", round.state === "cancelled" && "struck"]}>{whenLine(round)}</p>
                {#if round.with || round.place}
                  <p class="meta">
                    {round.with}{#if round.with && round.place}{" · "}{/if}{#if round.place}{#if isLink(round.place)}<a
                          class="link"
                          href={round.place}
                          onclick={(e) => {
                            e.preventDefault();
                            void openUrl(round.place);
                          }}>{host(round.place)}</a
                        >{:else}{round.place}{/if}{/if}
                  </p>
                {/if}
                {#if round.notes}<p class="meta notes">{round.notes}</p>{/if}
              </div>
              {#if round.state === "cancelled"}
                <Badge>Cancelled</Badge>
              {:else if awaitingDebrief(round, now)}
                <button
                  type="button"
                  class="state-button"
                  {@attach tip("Debrief this round (D)")}
                  onclick={(e) => {
                    e.stopPropagation();
                    openDebrief(round);
                  }}><Badge tone="accent">Debrief</Badge></button
                >
              {:else if round.debriefedAt && over(round)}
                <span class="meta debriefed"><Icon icon={Check} size={14} />Debriefed</span>
              {:else if next?.id === round.id}
                <Badge tone="accent">{soon(round)}</Badge>
              {/if}
            </div>
          {/snippet}
          {#snippet actions()}
            {#if round.debriefedAt && round.state === "upcoming" && round.id !== debriefRound?.id}
              <Button variant="ghost" size="sm" onclick={() => openDebrief(round)}>Open debrief</Button>
            {/if}
            <Button variant="ghost" size="sm" icon={Pencil} kbd="⏎" onclick={() => startEdit(round)}>Edit</Button>
            <Button
              variant="danger"
              size="sm"
              square
              icon={Trash2}
              aria-label="Remove this round"
              {@attach tip("Remove this round (⌫)")}
              onclick={() => remove(round)}
            />
          {/snippet}
        </PartRow>
      {/each}

      {#if form?.key === "new"}
        <div class="adding">{@render editor()}</div>
      {:else if !list.length}
        <div class="empty">
          <p class="empty__title">No interview yet</p>
          <p class="meta">
            When they reach out, add it here: paste the invite (⌘V) or drop its calendar file, and gliide fills in the
            round.
          </p>
          <div class="empty__actions">
            <Button icon={CalendarPlus} kbd="A" onclick={startAdd}>Add interview</Button>
            <Button variant="ghost" icon={ClipboardPaste} kbd="⌘V" onclick={() => void pasteInvite()}
              >Paste an invite</Button
            >
          </div>
        </div>
      {:else}
        <div class="foot">
          <Button variant="ghost" size="sm" icon={CalendarPlus} kbd="A" disabled={busy} onclick={startAdd}
            >Add interview</Button
          >
        </div>
      {/if}
      {#if offering && offerable}
        <div class="offer" role="group" aria-label="Reminders">
          <Icon icon={Bell} size={16} />
          <div class="offer__text">
            <p class="offer__title">Remind you the day before and an hour before?</p>
            <p class="meta">macOS asks once. Settings can change it later.</p>
          </div>
          <div class="offer__actions">
            <Button variant="ghost" size="sm" onclick={() => void notNow()}>Not now</Button>
            <Button size="sm" onclick={() => void turnOn()}>Turn on</Button>
          </div>
        </div>
      {/if}
    {/if}
  </section>

  {#if debriefRound}
    {#key debriefRound.id}
      <Debrief
        bind:this={debriefPane}
        {job}
        round={debriefRound}
        status={application.status}
        {prep}
        {writing}
        {practiced}
        bind:marked
        bind:canSave={canSaveDebrief}
        onsaved={debriefSaved}
        onleave={leaveDebrief}
      />
    {/key}
  {:else if focus}
    <Prep
      name={focus.kind === "other" ? "the interview" : `the ${kindLabel(focus.kind).toLowerCase()}`}
      {prep}
      {writing}
      failed={prepFailed}
      {practiced}
      {lines}
      selected={question}
      onselect={choose}
      onpractice={startPractice}
      onagain={() => focus && void loadFor(focus.id, true)}
      oncancel={() => void cancel(job)}
    />
  {/if}
</div>
<div class="keys-wrap"><KeyFooter keys={KEYS} /></div>
{/if}

<style>
  .scroller {
    flex: 1;
    min-height: 0;
    overflow: auto;
    overscroll-behavior: contain;
    scroll-padding-block-start: 48px;
  }
  .foot {
    padding: var(--space-1) var(--space-1-5) 0;
  }
  /* Sticks to the column's top; takes no room while the card is there, so nothing moves when the strip shows. */
  .pin {
    position: sticky;
    top: 0;
    z-index: 2;
    height: 0;
  }
  .pin--folded {
    height: 48px;
  }
  .strip {
    display: flex;
    align-items: center;
    gap: var(--space-2-5);
    height: 48px;
    padding-inline: var(--space-6) var(--space-3);
    background: var(--paper);
    box-shadow: inset 0 -1px 0 var(--line);
  }
  .strip__company {
    flex: none;
    font-weight: var(--weight-medium);
  }
  .strip__role {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink-2);
  }
  .strip__next {
    flex: none;
    color: var(--ink-2);
  }

  /* The card: the column's first block, on paper, over a hairline. */
  .card {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--space-4);
    padding: var(--space-5) var(--space-6);
    border-bottom: 1px solid var(--line);
  }
  .named {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    column-gap: var(--space-3);
    row-gap: var(--space-0-5);
    align-items: start;
  }
  .lead {
    display: flex;
    grid-row: span 2;
    margin-top: 6px;
  }
  .fold {
    grid-column: 3;
    grid-row: 1;
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
  .headline {
    grid-column: 2 / -1;
    margin: 0;
  }
  .next {
    display: grid;
    gap: var(--space-1);
  }
  .next p,
  .card > p {
    margin: 0;
  }
  .next__meta {
    color: var(--ink-2);
  }
  .link {
    color: var(--accent-text);
    text-decoration: none;
  }

  .rounds {
    display: grid;
    padding: var(--space-4) var(--space-3) var(--space-6);
  }
  .round {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
  }
  .round__body {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: var(--space-0-5);
  }
  .round__body p {
    margin: 0;
  }
  .words {
    color: var(--ink);
  }
  .struck {
    color: var(--ink-3);
    text-decoration: line-through;
  }
  .notes {
    color: var(--ink-3);
    text-wrap: pretty;
  }
  /* A round's Debrief badge is the button that opens its debrief. */
  .state-button {
    display: inline-flex;
    flex: none;
    padding: 0;
    border: 0;
    border-radius: var(--radius-full);
    background: none;
    cursor: default;
  }
  .state-button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .debriefed {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: var(--space-1);
    color: var(--ink-2);
  }
  /* The one-time reminders offer: a sunken well under the rounds. */
  .offer {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2-5);
    margin: var(--space-2) var(--space-3) 0;
    padding: var(--space-3);
    border-radius: var(--radius-md);
    background: var(--sunken);
  }
  .offer > :global(.icon) {
    flex: none;
    margin-top: 3px;
    color: var(--ink-3);
  }
  .offer__text {
    flex: 1;
    min-width: 0;
  }
  .offer__text p {
    margin: 0;
    text-wrap: pretty;
  }
  .offer__title {
    font-weight: var(--weight-medium);
  }
  .offer__actions {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-2);
  }
  .reading {
    padding: var(--space-2) var(--space-3);
  }
  .adding {
    padding: var(--space-2) var(--space-3);
  }
  .form {
    display: grid;
    grid-template-columns: 64px minmax(0, 1fr);
    gap: var(--space-2) var(--space-3);
    align-items: center;
  }
  .note,
  .problem,
  .form__foot {
    grid-column: 1 / -1;
    margin: 0;
  }
  .problem {
    color: var(--danger);
  }
  .field-label {
    color: var(--ink-2);
  }
  .field-label--top {
    align-self: start;
    padding-top: var(--space-1-5);
  }
  .kind {
    display: flex;
  }
  .when {
    display: flex;
    gap: var(--space-2);
  }
  .form__foot {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--space-2);
    padding-top: var(--space-1);
  }
  .form__foot .meta {
    flex: 1;
    min-width: 0;
  }
  .empty {
    display: grid;
    gap: var(--space-2);
    padding: var(--space-3);
  }
  .empty p {
    margin: 0;
    text-wrap: pretty;
  }
  .empty__title {
    font-weight: var(--weight-medium);
  }
  .empty__actions {
    display: flex;
    gap: var(--space-2);
    padding-top: var(--space-1);
  }
  .keys-wrap {
    flex: none;
  }
  @media (hover: hover) {
    .link:hover {
      text-decoration: underline;
    }
  }
</style>
