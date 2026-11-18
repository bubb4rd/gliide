<!--
  Settings → Benchmark: how each route (an agent and model) does each role's work, scored by code against
  hand-checked answers (ARCHITECTURE.md §12, Benchmark). The scores come from the app (`benchmark()`); this pane only
  rounds and lays them out. Its actions lead the pane (the sheet's head holds only its title and Close): Use the best
  per role, and Run benchmark. Then the lede naming the best route per role; the run going on this Mac (its stage, how
  far it is, Stop), or a stopped one (Resume); the run's setup (length, roles, routes, the estimate; ⏎ starts, Esc
  closes), which leads the pane until something has run; then the leaderboard: a role at a time, its winner as the
  caption (with Use for …), every route as a row (score, quality, reliability, speed, typical time, cost), each
  opening onto its three sources side by side. Tooltips say what every number is made of. Writing has no quality
  check, so it's timed and never ranked.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import Check from "@lucide/svelte/icons/check";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Play from "@lucide/svelte/icons/play";
  import {
    type Agent,
    appError,
    type BenchEstimate,
    benchmarkEstimate,
    type BenchPlan,
    type BenchResult,
    type BenchRoute,
    type BenchRunStatus,
    type BenchScore,
    type BenchSource,
    type BenchStep,
    type Role,
  } from "$lib/api";
  import { AGENT_LIST, AGENTS, ROLES } from "$lib/agents";
  import type { KeyBindings } from "$lib/keys";
  import { app } from "$lib/stores/app.svelte";
  import { benchmark } from "$lib/stores/benchmark.svelte";
  import {
    Button,
    Card,
    Checkbox,
    CompanyMark,
    Icon,
    ScorePill,
    Segmented,
    Table,
    type TableColumn,
    type TableSort,
    Thinking,
    TimeTrack,
    type TipData,
    type TipRow,
    tip,
  } from "$lib/ui";

  interface Props {
    /** The pane's keys, for the sheet to bind while it's shown. */
    keys?: KeyBindings;
  }

  let { keys = $bindable({}) }: Props = $props();

  /** Below this many decided changes, a route's kept share still sits near your average. */
  const KEPT_FEW = 30;
  const ROLE_NAME = Object.fromEntries(ROLES.map((r) => [r.id, r.name])) as Record<Role, string>;
  const ROLE_OPTIONS = ROLES.map((r) => ({ value: r.id, label: r.name }));
  const LENGTHS: { value: "quick" | "full"; label: string }[] = [
    { value: "quick", label: "Quick" },
    { value: "full", label: "Full" },
  ];
  /** The shares (0–1) each role's quality is the mean of, by resume-bench's part ids. Writing has none. */
  const SHARES: Record<Role, { id: string; label: string }[]> = {
    read: [
      { id: "recall", label: "Term recall" },
      { id: "precision", label: "Term precision" },
      { id: "must_recall", label: "Must-have recall" },
    ],
    tailor: [
      { id: "grounded", label: "Grounded" },
      { id: "fits", label: "Fits the page" },
      { id: "lift", label: "Lift, of 20 points" },
    ],
    write: [],
    coach: [{ id: "grounded", label: "Grounded questions" }],
  };
  /** What quality is, per role. */
  const QUALITY: Record<Role, string> = {
    read: "The mean of term recall, term precision and must-have recall, against the hand-checked requirements.",
    tailor:
      "The code checks (the mean of grounded, fits the page and lift out of 20 points), scaled by how the share of its suggested changes you kept compares with your average: ×1.00 at your average.",
    write: "Not scored yet: nothing checks a cover letter’s quality.",
    coach: "The share of prep questions the checks keep as grounded in your resume.",
  };
  const KEPT_NONE = "Kept counts once you’ve decided on some suggested changes and exported";

  onMount(() => void benchmark.refresh());

  const view = $derived(benchmark.view);
  const run = $derived(benchmark.run);
  const running = $derived(run?.state === "running");
  /** A run that's going or stopped: its routes not reached yet say Queued. */
  const active = $derived(run?.state === "running" || run?.state === "stopped");

  let role = $state<Role>(benchmark.run?.now?.role ?? "read");
  let sort = $state<TableSort>({ key: "score", dir: "desc" });
  /** The routes whose sources are open, by id. */
  let open = $state<string[]>([]);

  const board = $derived(view?.roles.find((r) => r.role === role) ?? null);
  const target = $derived(board?.targetS ?? 0);
  const routes = $derived(board?.routes ?? []);
  /** The routes with a score, best first (the app's order: by the unrounded score). */
  const ranked = $derived(routes.filter((r) => r.score?.composite != null));

  function chooseRole(next: Role) {
    role = next;
    open = [];
  }

  // ── Formatting: digits, sentence case ──
  const money2 = new Intl.NumberFormat(undefined, { style: "currency", currency: "USD", minimumFractionDigits: 2 });
  const money3 = new Intl.NumberFormat(undefined, { style: "currency", currency: "USD", minimumFractionDigits: 3 });
  const whole = new Intl.NumberFormat();
  const shortDay = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });
  const int = (x: number) => Math.round(x);
  const one = (x: number) => x.toFixed(1);
  const dec = (x: number) => x.toFixed(2);
  const sec = (s: number) => `${s.toFixed(1)} s`;
  const pct = (x: number) => `${Math.round(x * 100)}%`;
  const xf = (m: number) => `×${m.toFixed(2)}`;
  /** A call's cost: three decimals under ten cents. */
  const usd = (c: number) => (c > 0 && c < 0.001 ? "<$0.001" : (c < 0.1 ? money3 : money2).format(c));
  const plural = (n: number, word: string) => `${whole.format(n)} ${word}${n === 1 ? "" : "s"}`;
  const lower = (r: Role) => ROLE_NAME[r].toLowerCase();
  function andList(words: string[]) {
    return words.length < 2 ? (words[0] ?? "") : `${words.slice(0, -1).join(", ")} and ${words.at(-1)}`;
  }
  const capital = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
  /** "2026-11-16" (a day, read as local) or a timestamp → "Nov 16". */
  function day(at: string) {
    const ymd = /^(\d{4})-(\d{2})-(\d{2})$/.exec(at);
    const d = ymd ? new Date(+ymd[1], +ymd[2] - 1, +ymd[3]) : new Date(at);
    return Number.isNaN(d.getTime()) ? at : shortDay.format(d);
  }
  /** "40 s", "12 min", "1 h 23 min". */
  function span(s: number) {
    if (s < 60) return `${Math.max(0, Math.round(s))} s`;
    const m = Math.round(s / 60);
    if (m < 60) return `${m} min`;
    return m % 60 ? `${Math.floor(m / 60)} h ${m % 60} min` : `${m / 60} h`;
  }
  /** An estimate: to 5 minutes, at least 5. */
  const about = (s: number) => span(Math.max(5, Math.round(s / 300) * 5) * 60);

  // ── Routes ──
  const rid = (r: { agent: Agent; model: string }) => `${r.agent}:${r.model}`;
  const nameOf = (r: { model: string }) => app.modelName(r.model);
  const resultOf = (r: BenchRoute, from: BenchSource | null): BenchResult | null =>
    from === "thisMac" ? r.thisMac : from === "reference" ? r.reference : null;
  /** Where reliability, speed, time and cost come from: this Mac's runs where there are any, else the reference. */
  const timed = (r: BenchRoute) => resultOf(r, r.timingFrom);
  /** Where quality comes from: the source with more runs (this Mac on a tie). */
  const judged = (r: BenchRoute) => resultOf(r, r.qualityFrom);
  /** The role runs on this route now (Agents → Who does what, live). */
  const isCurrent = (of: Role, r: { agent: Agent; model: string }) =>
    app.routes[of].info.id === r.agent && app.routes[of].model === r.model;
  /** Its agent is on and signed in and lists the model: Who does what can take it. */
  const canUse = (r: { agent: Agent; model: string }) => app.offers(r.agent) && app.lists(r.agent, r.model);
  const part = (d: BenchResult | null, id: string) => d?.parts.find((p) => p.id === id)?.value ?? null;
  const winnerOf = (of: Role) =>
    view?.roles.find((r) => r.role === of)?.routes.find((r) => r.score?.composite != null) ?? null;

  /** Where a result came from, in a phrase: "This Mac’s run, Nov 16 · 36 runs". */
  function sourceNote(r: BenchRoute, from: BenchSource | null) {
    const d = resultOf(r, from);
    if (!d) return "";
    if (from === "thisMac")
      return active && run?.plan.roles.includes(role)
        ? `This Mac’s run so far · ${plural(d.runs, "run")}`
        : `This Mac’s run, ${day(d.at)} · ${plural(d.runs, "run")}`;
    return `gliide’s reference, ${day(view?.referenceDate ?? d.at)} · ${plural(d.runs, "run")}`;
  }
  /** The quality and the timing come from different sources. */
  const split = (r: BenchRoute) => !!r.qualityFrom && !!r.timingFrom && r.qualityFrom !== r.timingFrom;
  /** Where a whole score came from, as a sentence. */
  const scoreFrom = (r: BenchRoute) =>
    split(r)
      ? `Quality from ${sourceNote(r, r.qualityFrom)}; reliability and speed from ${sourceNote(r, r.timingFrom)}.`
      : `From ${sourceNote(r, r.timingFrom ?? r.qualityFrom)}.`;
  /** A source, with how many runs it holds: "the reference (36 runs)", "this Mac’s quick run (4 runs)". */
  function sourcePhrase(r: BenchRoute, from: BenchSource | null) {
    const d = resultOf(r, from);
    if (!d) return "";
    if (from === "reference") return `the reference (${plural(d.runs, "run")})`;
    const latest =
      !!run && run.plan.roles.includes(role) && run.plan.routes.some((x) => x.agent === r.agent && x.model === r.model);
    return `this Mac’s ${latest && run ? (run.plan.quick ? "quick " : "full ") : ""}run (${plural(d.runs, "run")})`;
  }

  // ── Kept (Tailoring): your decisions against your average ──
  const average = $derived(view?.keptAverage ?? null);
  const averageShare = $derived(average && average.decided ? average.kept / average.decided : null);
  /** What the code checks are multiplied by: 0.75 + 0.25 × (kept counted ÷ your average). */
  const keptMultiplier = (s: BenchScore | null) => (s?.keptFactor == null ? null : 0.75 + 0.25 * s.keptFactor);
  /** The score on the code checks alone (the app's; equal to the score outside Tailoring and before any decision). */
  const withoutKept = (s: BenchScore) => s.compositeWithoutKept ?? s.composite ?? 0;
  const keptWhy = $derived(
    average && averageShare !== null
      ? `Kept scales the code checks by how your decisions on this route compare with your average (${pct(averageShare)} over ${plural(average.decided, "change")}): at your average it changes nothing. A route with few decisions sits near your average.`
      : "",
  );
  /** Under a Tailoring route's name, when its kept share leans on your average; short, so it keeps to the line. */
  function keptNote(r: BenchRoute) {
    if (role !== "tailor" || !average || r.score?.quality == null) return null;
    const decided = r.yours?.kept?.decided ?? 0;
    if (decided === 0) return { short: "Kept: your average", long: "No decisions of its own: kept counts your average" };
    return decided < KEPT_FEW
      ? { short: `Kept: ${decided} · near average`, long: `Kept: ${plural(decided, "decision")}, so it sits near your average` }
      : null;
  }

  // ── Ties: equal at the shown number, ranked before rounding ──
  function tieNote(r: BenchRoute) {
    const c = r.score?.composite;
    if (c == null) return "";
    return ranked
      .filter((o) => o !== r && int(o.score!.composite!) === int(c))
      .map((o) => {
        const oc = o.score!.composite!;
        const head = `Tied at ${int(c)} with ${nameOf(o)}`;
        if (one(oc) === one(c)) return `${head}.`;
        return `${head}; ${c > oc ? "ahead" : "behind"} before rounding, ${one(c)} to ${one(oc)}.`;
      })
      .join(" ");
  }

  // ── Tooltips ──
  function scoreTip(r: BenchRoute): TipData | null {
    const s = r.score;
    if (!s || s.composite == null || s.quality == null) return null;
    return {
      title: `${nameOf(r)} for ${lower(role)}`,
      value: int(s.composite),
      rows: [
        { label: "Quality × 60%", value: `${int(s.quality)} → ${one(0.6 * s.quality)}` },
        { label: "Reliability × 20%", value: `${int(s.reliability)} → ${one(0.2 * s.reliability)}` },
        { label: "Speed × 20%", value: `${int(s.speed)} → ${one(0.2 * s.speed)}` },
      ],
      note: [tieNote(r), scoreFrom(r), r.timingFrom === "reference" ? "A run on this Mac would time it here." : ""]
        .filter(Boolean)
        .join(" "),
    };
  }

  function partRows(d: BenchResult | null): TipRow[] {
    return SHARES[role].flatMap(({ id, label }) => {
      const v = part(d, id);
      return v === null ? [] : [{ label, value: dec(v) }];
    });
  }

  function qualityTip(r: BenchRoute): TipData | string | null {
    const s = r.score;
    const d = judged(r);
    if (!s || s.quality == null) return "Quality not scored yet: nothing checks a cover letter’s quality.";
    if (role !== "tailor" || s.codeChecks == null)
      return { title: "Quality", value: int(s.quality), rows: partRows(d), note: `The mean of these, from ${sourceNote(r, r.qualityFrom)}.` };
    const m = keptMultiplier(s);
    const kept = r.yours?.kept;
    const rows: TipRow[] = [{ label: "Code checks", value: int(s.codeChecks) }];
    if (m !== null && averageShare !== null && average) {
      const counted = r.yours?.keptCounted ?? averageShare;
      rows.push({
        label: "Kept vs your average",
        value: `${pct(counted)} ÷ ${pct(averageShare)} → ${xf(m)}`,
        sub: `${kept?.decided ? `${pct(kept.kept / kept.decided)} kept of ${kept.decided} decided` : "None decided"} · your average ${pct(averageShare)} of ${average.decided}`,
      });
    } else rows.push({ label: "Kept", value: "Not counted yet" });
    const parts = partRows(d);
    if (parts.length) parts[0] = { ...parts[0], rule: true };
    rows.push(...parts);
    return {
      title: "Quality",
      value: int(s.quality),
      rows,
      note:
        m !== null
          ? `${int(s.codeChecks)} ${xf(m)} = ${int(s.quality)}. ${keptWhy} Code checks from ${sourceNote(r, r.qualityFrom)}.`
          : `${KEPT_NONE}. Until then, the code checks alone, from ${sourceNote(r, r.qualityFrom)}.`,
    };
  }

  function reliabilityTip(r: BenchRoute): TipData | null {
    const d = timed(r);
    if (!d || !r.score) return null;
    const rows: TipRow[] = [{ label: "Valid first try", value: `${d.valid} of ${d.runs}` }];
    if (d.repaired) rows.push({ label: "Repaired, counts half", value: d.repaired });
    if (d.failed) rows.push({ label: "Failed", value: d.failed, tone: "danger" });
    return { title: "Reliability", value: int(r.score.reliability), rows, note: d.errors[0] ?? sourceNote(r, r.timingFrom) };
  }

  function speedTip(r: BenchRoute): TipData | null {
    const d = timed(r);
    if (!d || !r.score || d.p50S == null) return null;
    return {
      title: "Speed",
      value: int(r.score.speed),
      rows: [
        { label: "Typical", value: sec(d.p50S) },
        { label: "Target", value: `${target} s` },
        { label: "Scores 0 at", value: `${target * 4} s` },
      ],
      note: sourceNote(r, r.timingFrom),
    };
  }

  function timeTip(r: BenchRoute): TipData | null {
    const d = timed(r);
    if (!d || d.p50S == null) return null;
    const rows: TipRow[] = [{ label: "Median", value: sec(d.p50S) }];
    if (d.p90S != null) rows.push({ label: "Slowest 1 in 10", value: sec(d.p90S) });
    rows.push({ label: "Target", value: `${target} s` });
    if (r.yours?.p50S != null) {
      rows.push({ label: "Your calls, median", value: sec(r.yours.p50S), rule: true });
      rows.push({ label: "Your calls timed", value: whole.format(r.yours.calls) });
    }
    return { title: "Typical time", rows, note: `${sourceNote(r, r.timingFrom)}.` };
  }

  function costTip(r: BenchRoute): TipData | string | null {
    const d = timed(r);
    if (!d) return null;
    const agent = AGENTS[r.agent].name;
    if (d.costUsd == null) return `${agent} reports tokens only, and ${nameOf(r)} has no list price yet.`;
    return {
      title: "Cost a call",
      value: usd(d.costUsd),
      note: `At API prices, ${r.agent === "claude" ? "as Claude Code reported it" : `estimated from ${agent}’s token counts at list prices`}. On a subscription it’s a measure, not a bill.`,
    };
  }

  // ── The leaderboard ──
  /**
   * Every column's width holds its widest value with the cells' 12px a side (the last 24 at its end), so no number is
   * ever cut: Score a 24px pill and "100", Typical time a 24px track and "112.4 s" with "Yours 41.2 s" under it, Cost
   * "No price". The route needs 240px: the longest model name ("Gemini 3.8 Flash (Medium)", 167px) with the mark, the
   * chevron and the cell's padding; at the sheet's widest (709px of table) every column fits beside it. Narrower, the
   * columns the score's tooltip and a route's details also show go first (Reliability, Speed, Quality), then Cost,
   * until the route has that room again; past that, a long name truncates, its full name in its tooltip.
   */
  const ROUTE_ROOM = 240;
  /** The order columns leave a narrow table in. */
  const DROP = ["reliability", "speed", "quality", "cost"];
  /** The table's width (the board's), as laid out. */
  let boardWidth = $state(0);
  const allColumns = $derived<TableColumn<BenchRoute>[]>([
    { key: "route", label: "Route" },
    {
      key: "score",
      label: "Score",
      numeric: true,
      width: "82px",
      sortBy: (r) => r.score?.composite ?? null,
      tip: "60% quality, 20% reliability, 20% speed. Ranked before rounding, so equal numbers can differ in the tenths.",
    },
    {
      key: "quality",
      label: "Quality",
      numeric: true,
      width: "60px",
      sortBy: (r) => r.score?.quality ?? null,
      tip: role === "write" ? QUALITY.write : `60% of the score. ${QUALITY[role]}`,
    },
    {
      key: "reliability",
      label: "Reliability",
      numeric: true,
      width: "72px",
      sortBy: (r) => r.score?.reliability ?? null,
      tip: "20% of the score. Valid on the first try: a repaired answer counts half, a failed call nothing.",
    },
    {
      key: "speed",
      label: "Speed",
      numeric: true,
      width: "56px",
      sortBy: (r) => r.score?.speed ?? null,
      tip: `20% of the score. 100 at ${target} s typical or under, falling to 0 at ${target * 4} s.`,
    },
    {
      key: "time",
      label: "Typical time",
      numeric: true,
      width: "104px",
      sortBy: (r) => {
        const p = timed(r)?.p50S;
        return p == null ? null : -p;
      },
      tip: `The median call, the slowest 1 in 10 fainter behind it. The tick is ${ROLE_NAME[role]}’s ${target} s target; the track ends where speed scores 0.`,
    },
    {
      key: "cost",
      label: "Cost a call",
      numeric: true,
      width: "89px",
      sortBy: (r) => {
        const c = timed(r)?.costUsd;
        return c == null ? null : -c;
      },
      tip: "At API prices. Shown, never scored: on a subscription it’s a measure of what you used, not a bill.",
    },
  ]);
  const px = (c: TableColumn<BenchRoute>) => parseInt(c.width ?? "0", 10);
  const columns = $derived.by(() => {
    let shown = allColumns;
    for (const key of DROP) {
      if (!boardWidth || shown.reduce((sum, c) => sum + px(c), 0) + ROUTE_ROOM <= boardWidth) break;
      shown = shown.filter((c) => c.key !== key);
    }
    // The last column ends at the table's 24px inset, not 12.
    const last = shown.at(-1)!;
    return last.key === "cost" ? shown : shown.map((c) => (c === last ? { ...c, width: `${px(c) + 12}px` } : c));
  });

  function toggle(r: BenchRoute) {
    const id = rid(r);
    open = open.includes(id) ? open.filter((x) => x !== id) : [...open, id];
  }

  /** A route name's tooltip, only while it's cut short (a narrow sheet). */
  const fullWhenCut = (full: string) => (el: HTMLElement) =>
    tip(() => {
      const text = el.querySelector(".disclose__text");
      return text && text.scrollWidth > text.clientWidth ? full : null;
    })(el);

  /** The run's step on this route and role now. */
  const busyOn = (r: BenchRoute) =>
    running && !!run?.now && run.now.role === role && run.now.agent === r.agent && run.now.model === r.model;
  /** In the run's plan for this role, not run here yet. */
  const queued = (r: BenchRoute) =>
    active &&
    !busyOn(r) &&
    !r.thisMac &&
    !!run?.plan.roles.includes(role) &&
    run.plan.routes.some((x) => x.agent === r.agent && x.model === r.model);

  /** The line under a route's name: where its times are from (or its kept note), In use, Queued; while the run is on
   *  it, the source gives way to Running, so the line keeps to its width. Its tooltip says where quality is from when
   *  that's the other source. */
  function subLine(r: BenchRoute) {
    const d = timed(r);
    const source = d ? (r.timingFrom === "thisMac" ? "This Mac" : "Reference") : AGENTS[r.agent].name;
    const kept = keptNote(r);
    const busy = busyOn(r);
    const bits = busy ? [] : [kept?.short ?? source];
    if (isCurrent(role, r)) bits.push("In use");
    if (queued(r)) bits.push("Queued");
    const quality = split(r) ? `Quality from ${r.qualityFrom === "thisMac" ? "this Mac" : "the reference"}, which has more runs` : null;
    const tipText = [kept ? `${source} · ${kept.long}` : busy ? `${source} · Running` : null, quality].filter(Boolean);
    return { text: bits.join(" · "), busy, tip: tipText.length ? tipText.join(". ") : null };
  }

  const countTip = $derived.by(() => {
    const none = routes.filter((r) => !r.reference && !r.thisMac).map(nameOf);
    const unscored = routes.filter((r) => (r.reference || r.thisMac) && r.score?.composite == null).map(nameOf);
    const parts = [
      none.length && `${andList(none)}: not in the reference, and not run on this Mac.`,
      unscored.length && `${andList(unscored)}: timed, but quality isn’t scored for ${lower(role)}.`,
    ].filter(Boolean);
    return parts.join(" ") || "Every route has a score.";
  });

  // ── The caption: the role's winner ──
  const winner = $derived(ranked[0] ?? null);
  const second = $derived(ranked[1] ?? null);
  const bestLine = $derived.by(() => {
    if (!winner?.score?.composite) return "";
    const w = winner.score.composite;
    const bits = [`Best for ${ROLE_NAME[role]} of ${plural(ranked.length, "route")}`];
    if (second?.score?.composite != null) {
      const s = second.score.composite;
      const ahead = int(w) - int(s);
      bits.push(
        ahead === 0
          ? `tied with ${nameOf(second)}${one(w) === one(s) ? "" : ", ahead before rounding"}`
          : `${plural(ahead, "point")} ahead of ${nameOf(second)}`,
      );
    }
    const here = winner.thisMac?.p50S ?? winner.yours?.p50S ?? null;
    const where = winner.thisMac?.p50S != null ? "here" : "in your use";
    const ref = winner.reference?.p50S ?? null;
    bits.push(
      here !== null && ref !== null
        ? `${sec(here)} typical ${where}, ${sec(ref)} in the reference`
        : here !== null
          ? `${sec(here)} typical ${where}; not in the reference`
          : ref !== null
            ? `${sec(ref)} typical in the reference; not timed on this Mac yet`
            : "",
    );
    return bits.filter(Boolean).join(" · ");
  });

  /** What kept does to the ranking, in one line, and every route without kept → with it. */
  const kept = $derived.by(() => {
    if (role !== "tailor" || !winner) return null;
    if (!average) return { text: `${KEPT_NONE}.`, tip: null };
    const plain = [...ranked].sort((a, b) => withoutKept(b.score!) - withoutKept(a.score!));
    const pw = plain[0];
    const ahead = second ? int(winner.score!.composite!) - int(second.score!.composite!) : null;
    const text =
      pw === winner
        ? `Your kept edits don’t change the winner: ${nameOf(winner)} first either way`
        : `With your kept edits: ${nameOf(winner)} first${ahead === null ? "" : ahead === 0 ? ", tied" : `, ${plural(ahead, "point")} ahead`} · without them ${nameOf(pw)}`;
    const tipData: TipData = {
      title: "Without kept → with it",
      rows: ranked.map((r) => {
        const m = keptMultiplier(r.score);
        return {
          label: nameOf(r),
          value: `${int(withoutKept(r.score!))} → ${int(r.score!.composite!)}${m === null ? "" : ` · ${xf(m)}`}`,
        };
      }),
      note: keptWhy,
    };
    return { text, tip: tipData };
  });

  // ── Lede: the answer, in a sentence ──
  const lede = $derived.by(() => {
    if (!view) return null;
    const by = new Map<string, { name: string; roles: string[] }>();
    const unscored: Role[] = [];
    for (const { id } of ROLES) {
      const w = winnerOf(id);
      if (!w) {
        // Writing has no quality check: never ranked, so never named.
        if (id !== "write") unscored.push(id);
        continue;
      }
      const at = by.get(rid(w)) ?? { name: nameOf(w), roles: [] };
      at.roles.push(lower(id));
      by.set(rid(w), at);
    }
    const later = active ? "until this run reaches it" : "until you run it on this Mac";
    return {
      best: [...by.values()],
      after: unscored.length
        ? ` ${andList(unscored.map((r) => ROLE_NAME[r]))} ${unscored.length > 1 ? "aren’t" : "isn’t"} scored ${unscored.length > 1 ? later.replace(" it", " them") : later}.`
        : "",
    };
  });

  // ── Use the best per role, and Use for … ──
  const changes = $derived(
    ROLES.flatMap(({ id }) => {
      const w = winnerOf(id);
      return w && !isCurrent(id, w) && canUse(w) ? [{ role: id, route: w }] : [];
    }),
  );
  const bestTip = $derived<TipData>({
    title: changes.length ? "Sets Who does what" : "Every role is on its best route here",
    rows: ROLES.map(({ id, name }) => {
      const w = winnerOf(id);
      const now = app.modelName(app.routes[id].model);
      const value = !w
        ? `${now}, not scored`
        : isCurrent(id, w)
          ? `${now}, already`
          : canUse(w)
            ? `${now} → ${nameOf(w)}`
            : `${now}; ${nameOf(w)} isn’t on here`;
      return { label: name, value };
    }),
    note: "Each role on its top route. Agents → Who does what changes with it; nothing runs. A route is offered once its agent is on, signed in and lists the model.",
  });

  async function useRoutes(list: { role: Role; route: BenchRoute }[]) {
    await app.applyRoutes(Object.fromEntries(list.map((c) => [c.role, { agent: c.route.agent, model: c.route.model }])));
  }

  // ── The run going on this Mac ──
  /** Ticks the elapsed time between events. */
  let clock = $state(Date.now());
  $effect(() => {
    if (!running) return;
    const timer = setInterval(() => (clock = Date.now()), 1000);
    return () => clearInterval(timer);
  });

  const stage = (step: BenchStep) =>
    `${ROLE_NAME[step.role]} · ${nameOf(step)} · ${step.company} · run ${step.n} of ${step.runs}`;
  /** The step's start on `performance.now()`'s clock, as `Thinking` counts. */
  const stepStart = (step: BenchStep) => performance.now() - Math.max(0, Date.now() - Date.parse(step.startedAt));
  const progress = $derived.by(() => {
    if (!run) return "";
    const elapsed = run.elapsedS + (running ? Math.max(0, clock - benchmark.runAt) / 1000 : 0);
    const left = run.done ? (elapsed / run.done) * (run.total - run.done) : null;
    return [
      `${whole.format(run.done)} of ${plural(run.total, "call")}`,
      `${span(elapsed)} elapsed`,
      running && left !== null && `about ${span(left)} left`,
    ]
      .filter(Boolean)
      .join(" · ");
  });

  /** "Full run: Reading, Tailoring and Writing on Sonnet, Opus and GPT-6 Luna." */
  function planLine(plan: BenchPlan) {
    const roleNames = ROLES.filter((r) => plan.roles.includes(r.id)).map((r) => r.name);
    return `${plan.quick ? "Quick" : "Full"} run: ${andList(roleNames)} on ${andList(plan.routes.map(nameOf))}.`;
  }

  function macLine(r: BenchRunStatus | null) {
    if (!r) return "nothing run yet";
    if (r.state === "running" || r.state === "stopped") return "the run going now; scores fill in as calls land";
    if (r.state === "failed") return `the run of ${day(r.startedAt)} stopped after ${r.done} of ${plural(r.total, "call")}`;
    return `your ${r.plan.quick ? "quick" : "full"} run of ${day(r.startedAt)}, ${plural(r.total, "call")} over ${span(r.elapsedS)}`;
  }

  // ── Run on this Mac: the setup ──
  /** Shown: asked for, or, until something has run, the pane's lead. Never while a run is going. */
  let setupOpen = $state<boolean | null>(null);
  const showSetup = $derived(!running && (setupOpen ?? (view !== null && run === null)));
  let length = $state<"quick" | "full">("full");
  let pickRoles = $state<Role[]>(["read", "tailor", "write"]);
  let pickRoutes = $state<string[]>([]);
  /** The picks were set (the routes in use, or what an Add to a run asked for). */
  let seeded = false;
  let estimate = $state<BenchEstimate | null>(null);

  /** Every listed model of every agent that's on and signed in. */
  const runnable = $derived(
    AGENT_LIST.filter((a) => app.offers(a.id)).flatMap((a) =>
      (app.status(a.id)?.models ?? []).map((m) => ({ agent: a.id, model: m.id, name: m.name, id: `${a.id}:${m.id}` })),
    ),
  );
  /** The routes the roles run on now, that can run. */
  const inUse = () =>
    [...new Set(ROLES.map((r) => rid({ agent: app.routes[r.id].info.id, model: app.routes[r.id].model })))].filter((id) =>
      runnable.some((x) => x.id === id),
    );

  /** Routes with something to compare (the reference, a run here, your own calls) or that a role runs on now: the
   *  setup lists these; every other listed model waits behind More models. */
  const known = $derived(
    new Set([
      ...(view?.roles.flatMap((r) => r.routes.filter((x) => x.reference || x.thisMac || x.yours).map(rid)) ?? []),
      ...ROLES.map((r) => rid({ agent: app.routes[r.id].info.id, model: app.routes[r.id].model })),
    ]),
  );
  const shownRoutes = $derived(runnable.filter((x) => known.has(x.id)));
  const moreRoutes = $derived(runnable.filter((x) => !known.has(x.id)));
  let moreOpen = $state(false);
  const moreChecked = $derived(moreRoutes.filter((x) => pickRoutes.includes(x.id)).length);

  // Until something has run, the setup leads with the routes in use, once the agents are known.
  $effect(() => {
    if (!showSetup || seeded) return;
    const ids = inUse();
    if (!ids.length) return;
    pickRoutes = ids;
    seeded = true;
  });

  const plan = $derived<BenchPlan>({
    quick: length === "quick",
    roles: ROLES.map((r) => r.id).filter((r) => pickRoles.includes(r)),
    routes: runnable.filter((x) => pickRoutes.includes(x.id)).map(({ agent, model }) => ({ agent, model })),
  });
  const ready = $derived(plan.roles.length > 0 && plan.routes.length > 0);

  $effect(() => {
    if (!showSetup || !ready) {
      estimate = null;
      return;
    }
    let live = true;
    benchmarkEstimate(plan).then(
      (e) => live && (estimate = e),
      (e) => console.error(`benchmark estimate: ${appError(e).message}`),
    );
    return () => void (live = false);
  });

  const estimateLine = $derived.by(() => {
    if (!ready) return "Choose at least one role and one route";
    const roleNames = ROLES.filter((r) => plan.roles.includes(r.id)).map((r) => r.name);
    const head = `${andList(roleNames)} on ${plural(plan.routes.length, "route")}`;
    return estimate ? `${head} · ${plural(estimate.calls, "call")} · about ${about(estimate.seconds)}` : head;
  });
  const quotaLine = $derived(
    estimate && ready
      ? `${andList(estimate.perAgent.map(([a, n]) => `${whole.format(n)} on ${AGENTS[a].name}`))}, one call at a time. Stop whenever: it picks up where it stopped.`
      : "",
  );

  function openSetup(roles?: Role[], routeIds?: string[]) {
    if (roles) pickRoles = roles;
    pickRoutes = routeIds ?? inUse();
    seeded = true;
    setupOpen = true;
  }

  function closeSetup() {
    setupOpen = false;
  }

  function pick<T>(list: T[], item: T, on: boolean) {
    return on ? [...list.filter((x) => x !== item), item] : list.filter((x) => x !== item);
  }

  async function startRun() {
    if (!ready || benchmark.pending) return;
    const first = plan.roles[0];
    if (await benchmark.start(plan)) {
      setupOpen = false;
      chooseRole(first);
    }
  }

  // ⏎ starts the run (a focused button still presses itself); Esc closes the setup before the sheet.
  $effect(() => {
    keys = showSetup
      ? {
          Enter: (e) => {
            if (e.target instanceof HTMLButtonElement) return false;
            void startRun();
          },
          Escape: () => closeSetup(),
        }
      : {};
  });
</script>

<div class="actions">
  <Button disabled={!changes.length} onclick={() => useRoutes(changes)} {@attach tip(bestTip)}>Use the best per role</Button>
  {#if !showSetup}
    <Button
      variant="primary"
      icon={Play}
      disabled={running || benchmark.pending}
      onclick={() => openSetup()}
      {@attach tip(running ? "A run is going. Stop it to start another." : null)}>Run benchmark</Button
    >
  {/if}
</div>

<p class="lede">
  How each agent and model does gliide’s work, scored by code against hand-checked answers.{#if lede?.best.length}{" "}Best
    now: {#each lede.best as b, i (i)}{i === 0 ? "" : i === lede.best.length - 1 ? " and " : ", "}<b>{b.name}</b
      >{" "}for {andList(b.roles)}{/each}.{/if}{lede?.after ?? ""}
</p>

<div class="stack">
  {#if run && (run.state === "running" || run.state === "stopped" || run.state === "failed")}
    <Card pad>
      <div class="run">
        {#if run.state === "running"}
          <div class="run__now">
            {#if run.now}
              <div class="run__stage"><Thinking label={stage(run.now)} startedAt={stepStart(run.now)} /></div>
            {:else}
              <p class="run__stage run__words sweep">Starting…</p>
            {/if}
            <Button variant="ghost" size="sm" disabled={benchmark.pending} onclick={() => benchmark.stop()}>Stop</Button>
          </div>
          <p class="meta num run__sub">{progress}</p>
          <p class="meta run__note">
            {planLine(run.plan)} One call at a time on your own plans. Close Settings and it keeps going; quit gliide and
            Resume picks it up where it stopped.
          </p>
        {:else if run.state === "stopped"}
          <p class="run__words num">Stopped at {run.done} of {plural(run.total, "call")}</p>
          <p class="meta run__note">
            Resume picks up {run.now ? `at ${stage(run.now)}` : "where it stopped"}. What’s done is kept, and scored
            below.
          </p>
          <div class="run__actions">
            <Button variant="primary" size="sm" disabled={benchmark.pending} onclick={() => benchmark.resume()}>Resume</Button>
          </div>
        {:else}
          <p class="problem" role="alert">
            The run stopped after {run.done} of {plural(run.total, "call")}: {run.error ?? "something went wrong"}.
          </p>
          <p class="meta run__note">What’s done is kept, and scored below. Run benchmark starts again.</p>
        {/if}
      </div>
    </Card>
  {/if}

  {#if showSetup}
    <Card pad as="section" aria-labelledby="bench-setup">
      <h3 class="setup__head" id="bench-setup">Run on this Mac</h3>
      <p class="setup__lede">
        The bundled suite: 12 sample postings against the sample resume, Jane Doe, each with hand-checked answers. It
        runs on your own agents, so it spends your plans’ quota.
      </p>
      <dl class="fields">
        <div class="field">
          <dt>
            Length<span class="meta"
              >{length === "full"
                ? "12 postings; Reading and Tailoring 3 runs each, so scores settle"
                : "4 postings, 1 run each, for a first look"}</span
            >
          </dt>
          <dd>
            <Segmented options={LENGTHS} bind:value={length} label="Length" />
          </dd>
        </div>
        <div class="field">
          <dt>Roles<span class="meta">Writing is timed, not scored</span></dt>
          <dd>
            <div class="opts">
              {#each ROLES as r (r.id)}
                <label class="opt">
                  <Checkbox
                    checked={pickRoles.includes(r.id)}
                    label={r.name}
                    onchange={(on) => (pickRoles = pick(pickRoles, r.id, on))}
                  />
                  <span>{r.name}</span>
                </label>
              {/each}
            </div>
          </dd>
        </div>
        <div class="field">
          <dt>Routes<span class="meta">Agents that are on and signed in</span></dt>
          <dd>
            {#if runnable.length}
              {#if shownRoutes.length}
                <div class="opts opts--routes">
                  {#each shownRoutes as x (x.id)}{@render routeOption(x)}{/each}
                </div>
              {/if}
              {#if moreRoutes.length}
                <Button
                  variant="ghost"
                  size="sm"
                  class="more"
                  iconEnd={moreOpen ? ChevronDown : ChevronRight}
                  aria-expanded={moreOpen}
                  aria-controls="bench-more-routes"
                  onclick={() => (moreOpen = !moreOpen)}
                  >More models ({moreRoutes.length}){moreChecked ? ` · ${moreChecked} checked` : ""}</Button
                >
                {#if moreOpen}
                  <div class="opts opts--routes" id="bench-more-routes">
                    {#each moreRoutes as x (x.id)}{@render routeOption(x)}{/each}
                  </div>
                {/if}
              {/if}
            {:else}
              <p class="meta">No agent is on and signed in. Turn one on in Agents.</p>
            {/if}
          </dd>
        </div>
        <div class="field">
          <dt>Estimate<span class="meta">From your own times, else the reference’s</span></dt>
          <dd>
            <p
              class="estimate num"
              {@attach tip(
                estimate
                  ? "Each call timed as your own calls run once 5 are timed, else as the reference ran it, else at the role’s target."
                  : null,
              )}
            >
              {estimateLine}
            </p>
            {#if quotaLine}<p class="meta num">{quotaLine}</p>{/if}
          </dd>
        </div>
      </dl>
      <div class="setup__actions">
        <Button variant="ghost" onclick={closeSetup}>Cancel</Button>
        <Button variant="primary" kbd="⏎" disabled={!ready || benchmark.pending} onclick={startRun}>Start run</Button>
      </div>
    </Card>
  {/if}

  {#if benchmark.failed && !view}
    <p class="problem" role="alert">Couldn’t read the benchmark: {benchmark.failed}</p>
  {:else if view && board}
    <Card as="section" aria-label="Leaderboard">
      <div class="board" bind:clientWidth={boardWidth}>
        <div class="board__top">
          <div class="board__bar">
            <Segmented options={ROLE_OPTIONS} value={role} label="Role" onchange={chooseRole} />
            <span class="meta num" {@attach tip(countTip)}
              >{ranked.length} of {plural(routes.length, "route")} scored</span
            >
          </div>
          {@render caption()}
        </div>
        <Table
          {columns}
          rows={routes}
          bind:sort
          key={rid}
          label="{ROLE_NAME[role]} leaderboard"
          expanded={(r) => open.includes(rid(r))}
        >
          {#snippet cell(r, column)}
            {@render routeCell(r, column.key)}
          {/snippet}
          {#snippet detail(r)}
            {@render sources(r)}
          {/snippet}
        </Table>
        <div class="legend meta">
          <p>
            Score: 60% quality, 20% valid on the first try, 20% speed (full marks at {target} s typical, none at
            {target * 4} s). Cost is shown, never scored.
          </p>
          {#if role === "tailor"}
            <p>
              Tailoring’s quality: the code checks (grounded, fits, lift), scaled by kept, the share of suggested changes
              you kept when you exported, against your average: ×1.00 at your average, more above it, less below. A
              route with few decisions sits near your average. Kept always comes from your use: the reference and runs
              here have no one deciding.
            </p>
          {:else if role === "write"}
            <p>Writing is timed and checked for a valid answer; nothing scores a cover letter’s quality yet.</p>
          {/if}
          <p>
            Quality comes from the source with more runs (this Mac on a tie); reliability and speed from this Mac once it
            has runs, else from gliide’s reference. Hover a number for what it’s made of; open a route for its three
            sources side by side.
          </p>
        </div>
      </div>
    </Card>
    <p class="meta footnote">
      Reference: gliide’s own run of {day(view.referenceDate)}, the 12 sample postings against the sample resume, shipped
      with this release; it spent none of your quota. This Mac: {macLine(run)}. A route’s quality comes from whichever of
      the two has more runs, this Mac on a tie, so a quick run here doesn’t outweigh the reference’s full one; its
      reliability and speed come from this Mac as soon as it has runs, since that’s how fast it answers for you. Your use:
      the calls you’ve made, timed on this Mac. Kept: what you did with each route’s suggested changes when you exported
      (kept on or reworded counts as kept). Claude Code reports cost; Codex and Antigravity costs are estimated from list
      prices. Nothing leaves this Mac.
    </p>
  {/if}
</div>

{#snippet routeOption(x: { agent: Agent; model: string; name: string; id: string })}
  <label class="opt">
    <Checkbox
      checked={pickRoutes.includes(x.id)}
      label="{AGENTS[x.agent].name} · {x.name}"
      onchange={(on) => (pickRoutes = pick(pickRoutes, x.id, on))}
    />
    <span class="opt__mark" {@attach tip(AGENTS[x.agent].name)}><CompanyMark size={16} src={AGENTS[x.agent].icon} /></span>
    <span class="opt__name">{x.name}</span>
    {#if ROLES.some((r) => isCurrent(r.id, x))}<span class="meta">In use</span>{/if}
  </label>
{/snippet}

{#snippet caption()}
  {#if winner && winner.score?.composite != null}
    {@const w = winner}
    {@const info = AGENTS[w.agent]}
    <div class="lead">
      <span class="title num lead__score" {@attach tip(() => scoreTip(w))}>{int(w.score!.composite!)}</span>
      <div class="lead__text">
        <p class="lead__who">
          <CompanyMark size={20} src={info.icon} />
          <span class="lead__name">{nameOf(w)}</span>
          <span class="meta">{info.name}</span>
        </p>
        <p class="meta num lead__line">{bestLine}</p>
        {#if kept}<p class="meta num lead__line" {@attach tip(kept.tip)}>{kept.text}</p>{/if}
      </div>
      {#if isCurrent(role, w)}
        <span class="lead__end meta" {@attach tip(`${ROLE_NAME[role]} runs on ${nameOf(w)} now (Agents → Who does what).`)}
          ><Icon icon={Check} size={14} weight="medium" />In use</span
        >
      {:else if canUse(w)}
        <Button
          size="sm"
          onclick={() => useRoutes([{ role, route: w }])}
          {@attach tip({
            title: `Run ${lower(role)} on ${nameOf(w)}`,
            rows: [
              { label: "Now", value: app.modelName(app.routes[role].model) },
              { label: "After", value: nameOf(w) },
            ],
            note: `Sets ${ROLE_NAME[role]} in Agents → Who does what, with ${info.name}’s ${nameOf(w)}.`,
          })}>Use for {ROLE_NAME[role]}</Button
        >
      {/if}
    </div>
  {:else}
    <div class="lead lead--none">
      <span class="title num lead__score">–</span>
      <div class="lead__text">
        {#if role === "write"}
          <p class="lead__name">Quality not scored yet</p>
          <p class="meta lead__line">
            Cover letters are timed and checked for a valid answer; nothing checks their quality yet, so Writing isn’t
            ranked.
          </p>
        {:else}
          <p class="lead__name">No score for {lower(role)} yet</p>
          <p class="meta lead__line">
            {active && run?.plan.roles.includes(role)
              ? "This run scores it when it gets there."
              : "Neither the reference nor a run on this Mac has it. A run here scores it."}
          </p>
        {/if}
      </div>
      {#if role !== "write" && !running}
        <Button size="sm" onclick={() => openSetup([role])}>Run {ROLE_NAME[role]}</Button>
      {/if}
    </div>
  {/if}
{/snippet}

{#snippet routeCell(r: BenchRoute, key: string)}
  {@const d = timed(r)}
  {@const s = r.score}
  {#if key === "route"}
    {@const id = rid(r)}
    {@const isOpen = open.includes(id)}
    {@const sub = subLine(r)}
    {@const info = AGENTS[r.agent]}
    <div class={["route", !d && "route--none"]}>
      <span class="route__mark" {@attach tip(`${info.name} · ${nameOf(r)}`)}
        ><CompanyMark size={20} src={info.icon} /></span
      >
      {#if d}
        <button
          type="button"
          class="disclose"
          aria-expanded={isOpen}
          aria-controls="bench-{role}-{id}"
          onclick={() => toggle(r)}
          {@attach fullWhenCut(nameOf(r))}
        >
          <span class="disclose__text">{nameOf(r)}</span>
          <Icon icon={isOpen ? ChevronDown : ChevronRight} size={12} weight="medium" class="chevron" />
        </button>
      {:else}
        <span class="disclose" {@attach fullWhenCut(nameOf(r))}><span class="disclose__text">{nameOf(r)}</span></span>
      {/if}
      <span class="route__sub meta" {@attach tip(sub.tip)}
        >{sub.text}{#if sub.busy}{sub.text ? " · " : ""}<span class="sweep">Running</span>{/if}</span
      >
    </div>
  {:else if !d}
    {#if key === "score"}
      <span
        class="none"
        {@attach tip(`${nameOf(r)} isn’t in the reference for ${lower(role)}, and hasn’t run on this Mac.`)}>Not run</span
      >
    {/if}
  {:else if key === "score"}
    {#if s?.composite != null}
      <ScorePill value={s.composite} label="Score {int(s.composite)}" {@attach tip(() => scoreTip(r))} />
    {:else}
      <span
        class="none"
        {@attach tip(
          role === "write"
            ? "Writing’s quality has no check yet, so it’s timed but not ranked."
            : "No quality check ran for this route, so it isn’t ranked.",
        )}>Not scored</span
      >
    {/if}
  {:else if key === "quality"}
    {#if s?.quality != null}
      <span class="part" {@attach tip(() => qualityTip(r))}>{int(s.quality)}</span>
    {:else}
      <span class="none" {@attach tip("Quality not scored yet.")}>–</span>
    {/if}
  {:else if key === "reliability"}
    {#if s}<span class="part" {@attach tip(() => reliabilityTip(r))}>{int(s.reliability)}</span>{/if}
  {:else if key === "speed"}
    {#if s}<span class="part" {@attach tip(() => speedTip(r))}>{int(s.speed)}</span>{/if}
  {:else if key === "time"}
    {#if d.p50S != null}
      <TimeTrack seconds={d.p50S} slow={d.p90S} {target} yours={r.yours?.p50S ?? null} {@attach tip(() => timeTip(r))} />
    {/if}
  {:else if key === "cost"}
    {#if d.costUsd != null}
      <span class="part" {@attach tip(() => costTip(r))}>{usd(d.costUsd)}</span>
    {:else}
      <span class="none" {@attach tip(() => costTip(r))}>No price</span>
    {/if}
  {/if}
{/snippet}

{#snippet sources(r: BenchRoute)}
  {@const s = r.score}
  {@const m = keptMultiplier(s)}
  {@const both = s && !split(r) ? (r.timingFrom ?? r.qualityFrom) : null}
  {@const from = both === "thisMac" ? "this Mac’s run" : "the reference"}
  {@const keptScored = role === "tailor" && m !== null && !!r.yours?.kept?.decided}
  {@const cols = [
    { id: "reference", label: "Reference", d: r.reference },
    { id: "thisMac", label: "This Mac", d: r.thisMac },
  ] as const}
  <div class="well" id="bench-{role}-{rid(r)}">
    <p class="sum">
      {#if s?.composite != null && s.quality != null}
        Score <b class="num">{int(s.composite)}</b> = 60% of quality <b class="num">{int(s.quality)}</b> + 20% of
        reliability <b class="num">{int(s.reliability)}</b> + 20% of speed <b class="num">{int(s.speed)}</b>{#if split(r)}:
          quality <b class="num">{int(s.quality)}</b> from {sourcePhrase(r, r.qualityFrom)}, reliability
          <b class="num">{int(s.reliability)}</b> and speed <b class="num">{int(s.speed)}</b> from
          {sourcePhrase(r, r.timingFrom)}.{:else}, from {from}.{/if}{#if role === "tailor" && s.codeChecks != null}{" "}{#if m !== null && averageShare !== null}Quality =
            code checks <b class="num">{int(s.codeChecks)}</b>
            {xf(m)} (kept {pct(r.yours?.keptCounted ?? averageShare)} vs your average {pct(averageShare)}). {keptWhy}
          {:else}Quality is the code checks alone. {KEPT_NONE}.{/if}
        {/if}
      {:else if s}
        Not scored: {role === "write" ? "nothing checks a cover letter’s quality yet" : "no quality check ran"}.
        Reliability <b class="num">{int(s.reliability)}</b> and speed <b class="num">{int(s.speed)}</b> from
        {sourcePhrase(r, r.timingFrom)}.
      {/if}
    </p>
    <div class="cmp">
      <span></span>
      {#each cols as c (c.id)}
        {@const marks = [r.qualityFrom === c.id && "quality", r.timingFrom === c.id && "times"].filter(Boolean)}
        <div class={["cmp__h", marks.length && "cmp__h--used"]}>
          <p class="label">{c.label}{marks.length === 2 ? " · scored" : marks.length ? ` · ${marks[0]}` : ""}</p>
          <p class="meta num">
            {#if c.d}
              {c.id === "reference" ? day(view?.referenceDate ?? c.d.at) : day(c.d.at)} · {plural(c.d.runs, "run")}
              over {plural(c.d.items, "posting")}
            {:else}
              {c.id === "reference" ? "Not in the reference" : "Not run here"}
            {/if}
          </p>
          {#if c.id === "thisMac" && !c.d && canUse(r) && !running}
            <Button
              variant="ghost"
              size="sm"
              class="cmp__add"
              onclick={() => openSetup([role], [rid(r)])}>Add to a run</Button
            >
          {/if}
        </div>
      {/each}
      <div class={["cmp__h", keptScored && "cmp__h--kept"]}>
        <p class="label">Your use{keptScored ? " · kept scored" : ""}</p>
        <p class="meta num">
          {#if !r.yours || !r.yours.calls}
            Not used yet
          {:else if r.yours.p50S == null}
            {plural(r.yours.calls, "call")}; times show from 5
          {:else}
            {plural(r.yours.calls, "call")}
          {/if}
        </p>
      </div>

      {#if role === "tailor"}
        {@render metric(r, "Grounded", (d) => {
          const g = part(d, "grounded");
          const flagged = part(d, "flagged");
          const changes = part(d, "changes");
          return g === null
            ? null
            : [{ text: dec(g) }, ...(flagged !== null && changes !== null ? [{ text: `${int(flagged)} of ${int(changes)} left out`, tone: "warn" }] : [])];
        })}
        {@render metric(r, "Fits the page", (d) => {
          const f = part(d, "fits");
          const left = part(d, "left_fit");
          return f === null ? null : [{ text: dec(f) }, ...(left !== null ? [{ text: `${int(left)} left out`, tone: "warn" }] : [])];
        })}
        {@render metric(r, "Lift, of 20 points", (d) => {
          const l = part(d, "lift");
          const pts = part(d, "lift_pts");
          return l === null ? null : [{ text: dec(l) }, ...(pts !== null ? [{ text: `+${one(pts)} pts`, tone: "gain" }] : [])];
        })}
        {@render metric(r, "Code checks", (d) => (d.quality === null ? null : [{ text: String(int(d.quality)) }]))}
        <span class="cmp__k">Kept</span>
        <span class="cmp__v none">{r.reference ? "Not reviewed" : ""}</span>
        <span class="cmp__v none">{r.thisMac ? "Not reviewed" : ""}</span>
        <span class="cmp__v">
          {#if !average}
            <span class="none">None decided yet</span>
          {:else if r.yours?.kept?.decided}
            {@const k = r.yours.kept}
            <span class="cmp__two"
              ><span class="cmp__in num">{pct(k.kept / k.decided)}</span> of {plural(k.decided, "change")}<br /><span
                class="cmp__ink2 num"
                >counted as {pct(r.yours.keptCounted ?? k.kept / k.decided)}{m !== null ? ` · ${xf(m)}` : ""}</span
              ></span
            >
          {:else}
            <span class="cmp__two">Kept: your average{#if m !== null}<br /><span class="cmp__ink2 num">{xf(m)}</span>{/if}</span>
          {/if}
        </span>
      {:else if role === "read"}
        {#each SHARES.read as p (p.id)}
          {@render metric(r, p.label, (d) => {
            const v = part(d, p.id);
            return v === null ? null : [{ text: dec(v) }];
          })}
        {/each}
      {:else if role === "coach"}
        {@render metric(r, SHARES.coach[0].label, (d) => {
          const v = part(d, "grounded");
          const kept = part(d, "kept_items");
          const dropped = part(d, "dropped_items");
          return v === null
            ? null
            : [
                { text: dec(v) },
                ...(kept !== null && dropped !== null
                  ? [{ text: `${int(dropped)} of ${int(kept + dropped)} dropped`, tone: "none" }]
                  : []),
              ];
        })}
      {/if}

      <span class="cmp__k cmp__k--strong">Quality</span>
      {#each cols as c (c.id)}
        <span class={["cmp__v cmp__v--strong num", r.qualityFrom === c.id && "cmp__v--used"]}>
          {#if !c.d}
            {""}
          {:else if role === "write"}
            <span class="none">Not scored yet</span>
          {:else if role === "tailor"}
            {r.qualityFrom === c.id && s?.quality != null ? int(s.quality) : ""}
          {:else}
            {c.d.quality !== null ? int(c.d.quality) : "–"}
          {/if}
        </span>
      {/each}
      <span></span>

      <span class="cmp__rule"></span>

      {@render metric(
        r,
        "Valid first try",
        (d) => [
          { text: `${d.valid} of ${d.runs}` },
          ...(d.repaired ? [{ text: `${d.repaired} repaired`, tone: "none" }] : []),
          ...(d.failed ? [{ text: `${d.failed} failed`, tone: "danger" }] : []),
        ],
        "timing",
      )}
      {@render metric(
        r,
        "Typical",
        (d) => (d.p50S === null ? null : [{ text: sec(d.p50S) }]),
        "timing",
        r.yours?.p50S != null ? sec(r.yours.p50S) : "",
      )}
      {@render metric(
        r,
        "Slowest 1 in 10",
        (d) => (d.p90S === null ? null : [{ text: sec(d.p90S) }]),
        "timing",
        r.yours?.p90S != null && r.yours.p50S != null ? sec(r.yours.p90S) : "",
      )}
      {@render metric(
        r,
        "Cost a call",
        (d) => (d.costUsd === null ? [{ text: "No price", tone: "none" }] : [{ text: usd(d.costUsd) }]),
        "timing",
      )}
    </div>
    {#each cols as c (c.id)}
      {#each c.d?.errors ?? [] as e (e)}
        <p class="meta error">{c.label}, failed call: {e.replace(/\.$/, "")}.</p>
      {/each}
    {/each}
  </div>
{/snippet}

<!-- A metric's row: its value in each source, the one its group is scored from (quality, or timing) in ink. -->
{#snippet metric(
  r: BenchRoute,
  label: string,
  of: (d: BenchResult) => { text: string; tone?: string }[] | null,
  group: "quality" | "timing" = "quality",
  yours: string = "",
)}
  {@const cells = [r.reference, r.thisMac].map((d) => (d ? of(d) : null))}
  {@const from = group === "quality" ? r.qualityFrom : r.timingFrom}
  {#if cells.some((c) => c !== null) || yours}
    <span class="cmp__k">{label}</span>
    {#each cells as c, i (i)}
      <span class={["cmp__v num", from === (i === 0 ? "reference" : "thisMac") && "cmp__v--used"]}>
        {#each c ?? [] as bit, j (j)}{j ? " " : ""}<span class={bit.tone && `tone--${bit.tone}`}>{bit.text}</span>{/each}
      </span>
    {/each}
    <span class="cmp__v num">{yours}</span>
  {/if}
{/snippet}

<style>
  /* The pane's actions lead it, at the trailing edge (the sheet's head holds its title and Close alone). */
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    margin-bottom: var(--space-4);
  }
  .lede {
    margin: 0 0 var(--space-4);
    color: var(--ink-2);
    text-wrap: pretty;
  }
  .lede b {
    font-weight: var(--weight-medium);
    color: var(--ink);
  }
  .stack {
    display: grid;
    gap: var(--space-4);
  }
  p {
    margin: 0;
  }
  .problem {
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-sm);
    color: var(--danger);
    background: var(--danger-soft);
    text-wrap: pretty;
  }
  .none {
    color: var(--ink-3);
  }
  .part {
    color: var(--ink-2);
  }
  .tone--warn {
    color: var(--warn);
  }
  .tone--gain {
    color: var(--gain-text);
  }
  .tone--danger,
  .error {
    color: var(--danger);
  }
  .tone--none {
    color: var(--ink-3);
  }

  /* ── The run in progress: Thinking with Stop at its end; stopped: Resume ── */
  .run {
    display: grid;
    gap: var(--space-1);
  }
  .run__now {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: var(--control-sm);
  }
  .run__stage {
    flex: 1;
    min-width: 0;
  }
  .run__words {
    font-weight: var(--weight-medium);
  }
  .run__sub {
    color: var(--ink-2);
  }
  .run__note {
    text-wrap: pretty;
  }
  .run__actions {
    display: flex;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }

  /* ── Run on this Mac: the setup, as Settings' fields ── */
  .setup__head {
    margin: 0;
    font-size: var(--text-lg);
    line-height: var(--leading-lg);
    letter-spacing: var(--tracking-lg);
    font-weight: var(--weight-medium);
  }
  .setup__lede {
    margin-top: var(--space-1);
    color: var(--ink-2);
    text-wrap: pretty;
  }
  .fields {
    display: grid;
    margin: var(--space-3) 0 0;
  }
  .field {
    display: grid;
    grid-template-columns: 168px minmax(0, 1fr);
    column-gap: var(--space-4);
    align-items: start;
    padding-block: var(--space-3);
    box-shadow: inset 0 1px 0 var(--line);
  }
  .field > dt {
    padding-top: 2px;
    color: var(--ink-2);
  }
  .field > dt .meta {
    display: block;
    margin-top: var(--space-0-5);
    text-wrap: pretty;
  }
  .field > dd {
    display: grid;
    justify-items: start;
    gap: var(--space-0-5);
    min-width: 0;
    margin: 0;
  }
  .opts {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1) var(--space-5);
  }
  /* Two columns at the sheet's widest, one where a column would be under 200px. */
  .opts--routes {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(200px, 100%), 1fr));
    gap: var(--space-1) var(--space-4);
    width: 100%;
  }
  /* More models: a ghost disclosure, its label on the checkboxes' edge; the rest open under it, in place. */
  .field > dd :global(.more) {
    margin-inline-start: calc(-1 * var(--space-2));
  }
  .opt {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
    min-height: var(--control-sm);
    color: var(--ink);
    white-space: nowrap;
  }
  .opt__mark {
    display: inline-flex;
    flex: none;
  }
  .opt__name,
  .opt .meta {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .estimate {
    font-weight: var(--weight-medium);
  }
  .setup__actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    padding-top: var(--space-3);
    box-shadow: inset 0 1px 0 var(--line);
  }

  /* ── The leaderboard card: the role, the caption, the table (inset 24 as Table's cells), the legend ── */
  .board {
    overflow: clip;
    border-radius: calc(var(--radius-lg) - 1px);
  }
  .board__top {
    display: grid;
    gap: var(--space-4);
    padding: var(--space-5) var(--space-6) var(--space-4);
  }
  .board__bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
  }

  /* The caption: the winner's score (serif, as Usage's caption), its route, a meta line; the role's one action. */
  .lead {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    column-gap: var(--space-4);
  }
  .lead__score {
    min-width: 2ch;
    text-align: end;
  }
  .lead--none .lead__score {
    color: var(--ink-3);
  }
  .lead__text {
    display: grid;
    gap: var(--space-0-5);
    min-width: 0;
  }
  .lead__who {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }
  .lead__name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: var(--weight-medium);
  }
  .lead__line {
    color: var(--ink-2);
    text-wrap: pretty;
  }
  .lead__end {
    display: flex;
    align-items: center;
    gap: var(--space-1-5);
  }

  /* A route: the agent's 20px icon, the model (a disclosure, GroupHead's chevron after it), the line under it. */
  .route {
    display: grid;
    grid-template-columns: 20px minmax(0, 1fr);
    column-gap: var(--space-2);
    align-items: center;
  }
  .route__mark {
    display: inline-flex;
    grid-row: span 2;
  }
  .disclose {
    display: inline-flex;
    align-items: center;
    justify-self: start;
    gap: var(--space-1);
    max-width: 100%;
    min-width: 0;
    padding: 0;
    border: 0;
    border-radius: var(--radius-xs);
    background: none;
    font: inherit;
    font-weight: var(--weight-medium);
    color: var(--ink);
    cursor: default;
  }
  .disclose__text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .disclose :global(.chevron) {
    flex: none;
    color: var(--ink-3);
    transition: color var(--dur-instant) var(--ease-out);
  }
  @media (hover: hover) {
    .disclose:hover :global(.chevron) {
      color: var(--ink-2);
    }
  }
  .route--none .disclose {
    color: var(--ink-2);
  }
  .route__sub {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* The sources: a sunken well under the row, indented to the name; the score's sum, then the three side by side. */
  .well {
    margin-inline-start: calc(20px + var(--space-2));
    padding: var(--space-3) var(--space-4) var(--space-4);
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .sum {
    color: var(--ink-2);
    text-wrap: pretty;
  }
  .sum b {
    font-weight: var(--weight-medium);
    color: var(--ink);
  }
  .cmp {
    display: grid;
    grid-template-columns: minmax(88px, 148px) repeat(3, minmax(0, 1fr));
    column-gap: var(--space-4);
    row-gap: var(--space-0-5);
    margin-top: var(--space-3);
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
  }
  .cmp__h {
    display: grid;
    align-content: start;
    gap: var(--space-0-5);
    padding-block: var(--space-2) var(--space-1-5);
    box-shadow: inset 0 1px 0 var(--line-strong);
  }
  /* The source the score uses: a heavier rule, its label in ink. Your use, when its kept is scored, likewise. */
  .cmp__h--used {
    box-shadow: inset 0 2px 0 var(--ink-3);
  }
  .cmp__h--used .label,
  .cmp__h--kept .label {
    color: var(--ink);
  }
  .cmp__h :global(.cmp__add) {
    justify-self: start;
    margin-top: var(--space-1);
    margin-inline-start: calc(-1 * var(--space-2));
  }
  /* A narrow sheet wraps a value rather than cutting it. */
  .cmp__k,
  .cmp__v {
    min-width: 0;
    overflow-wrap: anywhere;
    color: var(--ink-2);
  }
  .cmp__k--strong,
  .cmp__v--used {
    color: var(--ink);
  }
  .cmp__k--strong,
  .cmp__v--strong {
    font-weight: var(--weight-medium);
  }
  .cmp__two {
    display: block;
    white-space: normal;
  }
  .cmp__in {
    font-weight: var(--weight-medium);
    color: var(--ink);
  }
  .cmp__ink2 {
    color: var(--ink-2);
  }
  .cmp__rule {
    grid-column: 1 / -1;
    height: 1px;
    margin-block: var(--space-1);
    background: var(--line);
  }
  .error {
    margin-top: var(--space-2);
    text-wrap: pretty;
  }

  .legend {
    display: grid;
    gap: var(--space-1);
    padding: var(--space-3) var(--space-6) var(--space-4);
    box-shadow: inset 0 1px 0 var(--line);
    text-wrap: pretty;
  }
  .footnote {
    padding-top: var(--space-3);
    box-shadow: inset 0 1px 0 var(--line);
    text-wrap: pretty;
  }
</style>
