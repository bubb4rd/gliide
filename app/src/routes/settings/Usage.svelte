<!--
  Settings → Usage (its pane's card; the pane says what it is): the tokens every model call used and what they'd cost at API prices, over the range you pick
  (7, 30 or 90 days, or all time). The range's cost captions the ledger of cost by day, stacked by token kind, with a
  sentence on where it went; then by feature, by model and the token mix. Claude Code says what each call cost; Codex
  and Antigravity report tokens only, so theirs is estimated from list prices, and the screen says which is which.
  Export CSV writes every call. Nothing is recorded before this shipped.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import ChevronUp from "@lucide/svelte/icons/chevron-up";
  import Download from "@lucide/svelte/icons/download";
  import { appError, exportUsage, type Feature, type UsageDay, type UsageView, usage } from "$lib/api";
  import { AGENTS, makerIcon } from "$lib/agents";
  import { app } from "$lib/stores/app.svelte";
  import {
    BarChart,
    Button,
    Card,
    type ChartBar,
    CompanyMark,
    dayKey,
    Meter,
    Segmented,
    StackedBar,
    StepKey,
    segmentStep,
    type TipData,
    tip,
    toast,
  } from "$lib/ui";

  type Range = "7" | "30" | "90" | "all";
  type Agent = keyof typeof AGENTS;
  const RANGES: { value: Range; label: string }[] = [
    { value: "7", label: "7 days" },
    { value: "30", label: "30 days" },
    { value: "90", label: "90 days" },
    { value: "all", label: "All time" },
  ];
  const FEATURES: Record<Feature, string> = {
    requirements: "Reading the requirements",
    "feed-check": "Feed checks",
    suggestions: "Suggesting edits",
    rewrite: "Rewriting lines",
    chat: "Chat",
    "cover-letter": "Cover letter",
    import: "Importing resumes",
    invite: "Reading invites",
    prep: "Interview prep",
    practice: "Practice",
    nudges: "Nudges",
    dig: "Dig",
    debrief: "Reading debriefs",
    live: "Live interview",
  };
  /** The token kinds, output first (the dearest per token), as the ledger stacks them from the baseline up and the
   *  mix lays them out: each with its place in a day's four (input, output, cache write, cache read). */
  const KINDS = [
    { label: "Output", at: 1 },
    { label: "Input", at: 0 },
    { label: "Cache write", at: 2 },
    { label: "Cache read", at: 3 },
  ] as const;
  /** Rows By feature shows before "Show n more". */
  const FOLDED = 6;
  /** Past this many days, All time draws a bar a week. */
  const DAILY_MAX = 120;

  let view = $state<UsageView | null>(null);
  let failed = $state<string | null>(null);
  let range = $state<Range>("30");
  let allFeatures = $state(false);
  let exporting = $state(false);

  onMount(() => {
    usage().then(
      (v) => (view = v),
      (e) => (failed = appError(e).message),
    );
  });

  // ── Formatting ──
  const money2 = new Intl.NumberFormat(undefined, { style: "currency", currency: "USD", minimumFractionDigits: 2 });
  const money3 = new Intl.NumberFormat(undefined, { style: "currency", currency: "USD", minimumFractionDigits: 3 });
  const compact = new Intl.NumberFormat(undefined, { notation: "compact", maximumFractionDigits: 1 });
  const whole = new Intl.NumberFormat();
  const longDay = new Intl.DateTimeFormat(undefined, { weekday: "short", month: "short", day: "numeric" });
  const shortDay = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });
  const weekdayShort = new Intl.DateTimeFormat(undefined, { weekday: "short" });
  const weekdayLong = new Intl.DateTimeFormat(undefined, { weekday: "long" });
  const fullDate = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric", year: "numeric" });

  /** Dollars: two decimals ("<$0.01" for a sliver); `fine` (a call's cost) gives three under ten cents. */
  function money(n: number, fine = false): string {
    if (n > 0 && n < 0.001) return fine ? "<$0.001" : "<$0.01";
    if (n > 0 && n < 0.01 && !fine) return "<$0.01";
    return (fine && n < 0.1 ? money3 : money2).format(n);
  }
  const tokens = (n: number) => compact.format(n);
  const calls = (n: number) => `${whole.format(n)} ${n === 1 ? "call" : "calls"}`;
  function percent(part: number, of: number): string {
    if (!of || !part) return "0%";
    const p = (part / of) * 100;
    return p < 1 ? "<1%" : `${Math.round(p)}%`;
  }
  /** A `YYYY-MM-DD` day as a local date. */
  function date(key: string): Date {
    const [y, m, d] = key.split("-").map(Number);
    return new Date(y, m - 1, d);
  }
  const addDays = (d: Date, n: number) => new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);

  // ── Sums ──
  type Four = [number, number, number, number];
  interface Sum {
    calls: number;
    tokens: Four;
    cost: number;
    reported: number;
    estimated: number;
    estimatedCalls: number;
    unpriced: number;
    byKind: Four;
  }
  const empty = (): Sum => ({
    calls: 0,
    tokens: [0, 0, 0, 0],
    cost: 0,
    reported: 0,
    estimated: 0,
    estimatedCalls: 0,
    unpriced: 0,
    byKind: [0, 0, 0, 0],
  });
  /** `s` with another sum or a day's row added. */
  function add(s: Sum, r: UsageDay | Sum): Sum {
    const row = "day" in r;
    s.calls += r.calls;
    const t: Four = row ? [r.inputTokens, r.outputTokens, r.cacheWriteTokens, r.cacheReadTokens] : r.tokens;
    t.forEach((n, i) => (s.tokens[i] += n));
    s.reported += row ? r.reportedCost : r.reported;
    s.estimated += row ? r.estimatedCost : r.estimated;
    s.cost = s.reported + s.estimated;
    s.estimatedCalls += r.estimatedCalls;
    s.unpriced += row ? r.unpricedCalls : r.unpriced;
    (row ? r.costByKind : r.byKind).forEach((c, i) => (s.byKind[i] += c));
    return s;
  }
  const total = (s: Sum) => s.tokens[0] + s.tokens[1] + s.tokens[2] + s.tokens[3];
  function sumBy<K>(rows: UsageDay[], key: (r: UsageDay) => K): Map<K, Sum> {
    const m = new Map<K, Sum>();
    for (const r of rows) {
      const k = key(r);
      add(m.get(k) ?? m.set(k, empty()).get(k)!, r);
    }
    return m;
  }
  /** Busier first: by cost, then tokens. */
  const busier = (a: Sum, b: Sum) => b.cost - a.cost || total(b) - total(a);

  const today = $derived(view ? date(view.today) : new Date());
  const since = $derived(view?.since ? date(view.since) : null);
  /** The range's first day, never before usage began. */
  const from = $derived.by(() => {
    const start = range === "all" ? (since ?? today) : addDays(today, 1 - Number(range));
    return since && since > start ? since : start;
  });
  const spanDays = $derived(Math.round((today.getTime() - from.getTime()) / 86_400_000) + 1);
  const rows = $derived(view ? view.rows.filter((r) => r.day >= dayKey(from)) : []);
  const sum = $derived(rows.reduce(add, empty()));
  const byDay = $derived(sumBy(rows, (r) => r.day));
  const activeDays = $derived(byDay.size);
  /** The cost split by token kind, over the calls with a price. */
  const priced = $derived(sum.byKind.reduce((a, b) => a + b, 0));
  const rangeWords = $derived(range === "all" ? "since usage began" : `in the last ${range} days`);
  const rangeDates = $derived(spanDays === 1 ? "today" : `${shortDay.format(from)} – today`);

  const busiest = $derived([...byDay].sort((a, b) => busier(a[1], b[1]))[0]);
  /** "Most of your usage lands on Tuesdays.", once the range holds a few weeks of it. */
  const weekday = $derived.by(() => {
    if (spanDays < 14 || activeDays < 5) return null;
    const days = Array.from({ length: 7 }, empty);
    for (const r of rows) add(days[date(r.day).getDay()], r);
    const best = days.map((s, i) => ({ s, i })).sort((a, b) => busier(a.s, b.s))[0];
    return `Most of your usage lands on ${weekdayLong.format(addDays(today, best.i - today.getDay()))}s.`;
  });
  /** The range's averages and busiest day, behind its cost. */
  const totalTip = $derived<TipData>({
    rows: [
      { label: "Per active day", value: money(activeDays ? sum.cost / activeDays : 0) },
      { label: "Per call", value: money(sum.calls ? sum.cost / sum.calls : 0, true) },
      ...(busiest ? [{ label: "Busiest day", value: `${shortDay.format(date(busiest[0]))} · ${money(busiest[1].cost)}` }] : []),
    ],
    note: "Per active day: averaged over the days with any model call",
  });
  /** Under the sentence: the calls, where the cost came from, the weekday it lands on. */
  const facts = $derived.by(() => {
    const parts = [`${calls(sum.calls)} on ${activeDays} of ${spanDays} ${spanDays === 1 ? "day" : "days"}`];
    if (sum.estimated && sum.reported) parts.push(`${money(sum.estimated)} of it estimated`);
    else if (sum.estimated) parts.push("estimated from list prices");
    else if (sum.reported) parts.push("reported by the agent");
    if (sum.unpriced) parts.push(`${calls(sum.unpriced)} without a price`);
    return `${parts.join(" · ")}.${weekday ? ` ${weekday}` : ""}`;
  });

  const features = $derived([...sumBy(rows, (r) => r.feature)].sort((a, b) => busier(a[1], b[1])));
  const shownFeatures = $derived(allFeatures ? features : features.slice(0, FOLDED));
  const models = $derived(
    [...sumBy(rows, (r) => `${r.agent}\u001f${r.model}`)]
      .sort((a, b) => busier(a[1], b[1]))
      .map(([key, s]) => {
        const [agent, model] = key.split("\u001f") as [Agent, string];
        return { key, agent, model, s };
      }),
  );

  /** Where a sum's cost came from, in a word or two. */
  function costFrom(s: Sum): string {
    const unsaid = s.estimatedCalls + s.unpriced;
    if (s.unpriced === s.calls) return "No price";
    if (!unsaid) return "Reported";
    return unsaid === s.calls && !s.unpriced ? "Estimated" : "Partly estimated";
  }
  /** Where a sum's cost came from, in a sentence. */
  function costNote(s: Sum, agent?: string): string {
    const parts: string[] = [];
    if (s.reported && s.estimated) parts.push(`${money(s.reported)} reported, ${money(s.estimated)} estimated`);
    else if (s.reported) parts.push(`Reported by ${agent ?? "the agent"}`);
    else if (s.estimated) parts.push("Estimated from list prices");
    if (s.unpriced) parts.push(`${calls(s.unpriced)} without a price`);
    return parts.join(" · ");
  }

  // ── The ledger: a bar a day (a week once All time runs long), its cost stacked by kind ──
  const weekly = $derived(range === "all" && spanDays > DAILY_MAX);
  function dayTip(title: string, s: Sum): TipData {
    if (!s.calls) return { title, value: money(0), note: "No model calls" };
    // The bar is the cost by kind; a day with no price shows its tokens by kind instead.
    const priced = s.byKind.some((c) => c > 0);
    return {
      title,
      value: money(s.cost),
      rows: [
        ...KINDS.map((k, n) => ({
          label: k.label,
          value: priced ? money(s.byKind[k.at]) : tokens(s.tokens[k.at]),
          step: segmentStep(n),
        })),
        { label: "Model calls", value: whole.format(s.calls) },
        { label: "Tokens", value: tokens(total(s)) },
      ],
      note: costNote(s),
    };
  }
  const bar = (key: string, s: Sum, title: string): ChartBar => ({
    key,
    value: s.cost,
    parts: KINDS.map((k) => s.byKind[k.at]),
    tip: dayTip(title, s),
  });
  const bars = $derived.by<ChartBar[]>(() => {
    const out: ChartBar[] = [];
    if (weekly) {
      // Weeks from the Sunday on or before the first day.
      for (let start = addDays(from, -from.getDay()); start <= today; start = addDays(start, 7)) {
        const s = empty();
        for (let i = 0; i < 7; i++) {
          const day = byDay.get(dayKey(addDays(start, i)));
          if (day) add(s, day);
        }
        out.push(bar(dayKey(start), s, `Week of ${shortDay.format(start)}`));
      }
    } else {
      for (let i = 0; i < spanDays; i++) {
        const d = addDays(from, i);
        const last = i === spanDays - 1;
        out.push(bar(dayKey(d), byDay.get(dayKey(d)) ?? empty(), last ? "Today, so far" : longDay.format(d)));
      }
    }
    // A week names each day; longer, the first, the middle and the last ("Today", "This week").
    const last = out.length - 1;
    const name = (i: number) => {
      const d = date(out[i].key);
      if (i === last) return weekly ? "This week" : "Today";
      return out.length <= 7 ? weekdayShort.format(d) : shortDay.format(d);
    };
    const ticked = out.length <= 7 ? out.map((_, i) => i) : out.length > 2 ? [0, Math.floor(last / 2), last] : [last];
    for (const i of ticked) out[i].tick = name(i);
    return out;
  });

  async function exportCsv() {
    const path = await save({
      defaultPath: `gliide-usage-${view?.today ?? dayKey(new Date())}.csv`,
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (!path) return;
    exporting = true;
    try {
      await exportUsage(path);
      const name = path.split("/").pop() ?? path;
      toast(`Saved ${name}`, { action: { label: "Show in Finder", run: () => void revealItemInDir(path) } });
    } catch (e) {
      toast(`Couldn’t save the usage: ${appError(e).message}`, { tone: "danger" });
    } finally {
      exporting = false;
    }
  }
</script>

<Card pad>
  <div class="card">
    {#if view?.rows.length}
      <!-- The range's cost captions the ledger; the range and Export CSV at the trailing edge. -->
      <div class="head">
        <p class="total">
          <span class="title num" {@attach tip(totalTip)}>{money(sum.cost)}</span>
          <span class="meta num">{tokens(total(sum))} tokens · {rangeDates}</span>
        </p>
        <div class="controls">
          <Segmented options={RANGES} bind:value={range} label="Usage over" />
          <Button size="sm" variant="ghost" icon={Download} disabled={exporting} onclick={exportCsv}>Export CSV</Button>
        </div>
      </div>
    {/if}

    {#if failed}
      <p class="problem" role="alert">Couldn’t read the usage: {failed}</p>
    {:else if view && !view.rows.length}
      <div class="none">
        <p class="none-title">No usage yet</p>
        <p class="hint">
          Each model call’s tokens and cost show here from now on: reading postings, suggesting edits, chat and the rest.
        </p>
      </div>
    {:else if view}
      <section class="ledger" aria-label="Estimated cost by {weekly ? 'week' : 'day'}">
        <BarChart {bars} format={(n) => money(n)} label="Estimated cost by {weekly ? 'week' : 'day'}, {rangeWords}" />
        {#if sum.calls}
          <div class="summary">
            <p class="sentence">
              {#if priced}
                <strong class="num">{money(sum.byKind[1])}</strong> on output
              {:else}
                <strong class="num">{tokens(sum.tokens[1])}</strong> output tokens
              {/if}
              · <strong class="num">{percent(sum.tokens[3], total(sum))}</strong> of tokens read from cache
            </p>
            <ul class="legend" aria-label="Bars stacked by">
              {#each KINDS as kind, n (kind.label)}
                <li><StepKey step={segmentStep(n)} />{kind.label}</li>
              {/each}
            </ul>
          </div>
          <p class="hint num">{facts}</p>
        {:else}
          <p class="hint">No model calls {rangeWords}.</p>
        {/if}
      </section>

      {#if features.length}
        <section class="group" aria-labelledby="usage-features">
          <h3 class="label" id="usage-features">By feature</h3>
          <ul class="rows">
            {#each shownFeatures as [feature, s] (feature)}
              <li class="row">
                <span class="name">{FEATURES[feature]}</span>
                <span class="n calls num">{calls(s.calls)}</span>
                <span class="n num">{tokens(total(s))} tokens</span>
                <span class="n cost num" {@attach tip(costNote(s))}>{money(s.cost)}</span>
                <span class="n share">
                  <Meter value={sum.cost ? s.cost / sum.cost : 0} label="{FEATURES[feature]}, share of cost" />
                  <span class="num">{percent(s.cost, sum.cost)}</span>
                </span>
              </li>
            {/each}
          </ul>
          {#if features.length > FOLDED}
            <div>
              <Button
                size="sm"
                variant="ghost"
                iconEnd={allFeatures ? ChevronUp : ChevronDown}
                aria-expanded={allFeatures}
                onclick={() => (allFeatures = !allFeatures)}
              >
                {allFeatures ? "Show fewer" : `Show ${features.length - FOLDED} more`}
              </Button>
            </div>
          {/if}
        </section>

        <section class="group" aria-labelledby="usage-models">
          <h3 class="label" id="usage-models">By model</h3>
          <ul class="rows">
            {#each models as { key, agent, model, s } (key)}
              <li class="row">
                <span class="name model">
                  <CompanyMark size={20} src={makerIcon(model) ?? AGENTS[agent].icon} />
                  <span class="model-name">{app.modelName(model)}</span>
                  <span class="meta">{AGENTS[agent].name}</span>
                </span>
                <span class="n calls num">{calls(s.calls)}</span>
                <span class="n num">{tokens(total(s))} tokens</span>
                <span class="n cost num">{money(s.cost)}</span>
                <span class="n from" {@attach tip(costNote(s, AGENTS[agent].name))}>{costFrom(s)}</span>
              </li>
            {/each}
          </ul>
        </section>

        <section class="group" aria-labelledby="usage-mix">
          <h3 class="label" id="usage-mix">Token mix</h3>
          <StackedBar segments={KINDS.map((k) => ({ label: k.label, value: sum.tokens[k.at] }))} label="Tokens by kind" />
          <dl class="mix">
            {#each KINDS as kind, n (kind.label)}
              <div class="kind">
                <dt><StepKey step={segmentStep(n)} />{kind.label}</dt>
                <dd class="num">
                  <span class="kind-tokens">{tokens(sum.tokens[kind.at])}</span>
                  <span class="meta">{percent(sum.tokens[kind.at], total(sum))}</span>
                </dd>
                {#if priced}
                  <dd class="meta num">{money(sum.byKind[kind.at])} · {percent(sum.byKind[kind.at], priced)} of cost</dd>
                {/if}
              </div>
            {/each}
          </dl>
          {#if sum.tokens[3] && priced}
            <p class="hint">
              Cache reads are the instructions sent with every call, read back from the agent’s cache at a tenth of the
              input price: {percent(sum.tokens[3], total(sum))} of the tokens, {percent(sum.byKind[3], priced)} of the
              cost.
            </p>
          {/if}
        </section>
      {/if}
    {/if}

    {#if view}
      <p class="hint foot">
        Claude Code says what each call would cost at API prices; Codex and Antigravity report tokens only, so theirs is
        estimated from the labs’ list prices as of {fullDate.format(date(view.pricesAsOf))}.
        {#if since}Kept since {fullDate.format(since)}, on this Mac only.{:else}Kept on this Mac only.{/if}
      </p>
    {/if}
  </div>
</Card>

<style>
  .card {
    display: grid;
    gap: var(--space-1);
    container-type: inline-size;
  }
  /* The range's cost and what it holds, then the range and Export CSV at the trailing edge. */
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2) var(--space-4);
    min-height: var(--control-sm);
  }
  .total {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--space-2);
    min-width: 0;
    margin: 0;
  }
  .total .meta {
    color: var(--ink-3);
  }
  .controls {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .hint,
  .problem,
  .sentence,
  .none-title {
    margin: 0;
    text-wrap: pretty;
  }
  .hint {
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    color: var(--ink-3);
  }
  .problem {
    margin-top: var(--space-3);
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-sm);
    color: var(--danger);
    background: var(--danger-soft);
  }
  .none {
    display: grid;
    gap: var(--space-0-5);
    margin-top: var(--space-3);
    padding-top: var(--space-3);
    box-shadow: inset 0 1px 0 var(--line);
  }
  .none-title {
    font-weight: var(--weight-medium);
  }
  /* Nothing yet, or a failure, opens the card: no hairline or gap over it. */
  .card > .none:first-child {
    margin-top: 0;
    padding-top: 0;
    box-shadow: none;
  }
  .card > .problem:first-child {
    margin-top: 0;
  }
  .foot {
    margin-top: var(--space-5);
    padding-top: var(--space-3);
    box-shadow: inset 0 1px 0 var(--line);
  }

  /* The ledger under its cost: the sentence on where it went beside the parts' key, the facts under them. */
  .ledger {
    display: grid;
    gap: var(--space-2);
    margin-top: var(--space-4);
  }
  .summary {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-1) var(--space-4);
    margin-top: var(--space-2);
  }
  .sentence {
    color: var(--ink-2);
  }
  .sentence strong {
    font-weight: var(--weight-medium);
    color: var(--ink);
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1) var(--space-3);
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    color: var(--ink-2);
  }
  .legend li {
    display: flex;
    align-items: center;
    gap: var(--space-1-5);
  }

  .group {
    display: grid;
    gap: var(--space-2);
    margin-top: var(--space-6);
  }
  .label {
    margin: 0;
    color: var(--ink-3);
  }

  /* A breakdown: a row each on hairlines, no header (each number says what it counts), the columns shared down the
     list so the numbers line up at their trailing edges. */
  .rows {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 88px 104px 80px 112px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .row {
    display: grid;
    grid-template-columns: subgrid;
    grid-column: 1 / -1;
    align-items: center;
    padding-block: var(--space-2);
    box-shadow: inset 0 1px 0 var(--line);
  }
  .row > span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .n {
    padding-inline-start: var(--space-3);
    text-align: end;
    color: var(--ink-2);
  }
  .n.cost {
    color: var(--ink);
  }
  .from {
    color: var(--ink-2);
  }
  .share {
    display: inline-flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--space-2);
    width: 100%;
  }
  .share .num {
    min-width: 4ch;
  }
  .model {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }
  .model-name {
    flex: none;
  }
  .model .meta {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--ink-3);
  }

  .mix {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: var(--space-4);
    margin: var(--space-1) 0 0;
  }
  .kind {
    display: grid;
    gap: var(--space-0-5);
    min-width: 0;
  }
  .kind dt {
    display: flex;
    align-items: center;
    gap: var(--space-1-5);
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    color: var(--ink-2);
  }
  .kind dd {
    margin: 0;
  }
  .kind-tokens {
    font-weight: var(--weight-medium);
  }

  /* Narrow: the rows without their calls, the mix in two columns. */
  @container (width < 520px) {
    .rows {
      grid-template-columns: minmax(0, 1fr) 104px 80px 112px;
    }
    .calls {
      display: none;
    }
    .mix {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
</style>
