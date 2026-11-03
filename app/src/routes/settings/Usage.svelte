<!--
  Settings → Usage: the tokens every model call used and what they'd cost at API prices, over the range you pick
  (7, 30 or 90 days, or all time). Six stats, cost by day, the busiest days (a heatmap of the last 12 weeks, the three
  busiest in the range and the weekday most of it lands on), then by feature, by model and the token mix. Claude Code
  says what each call cost; Codex and Antigravity report tokens only, so theirs is estimated from list prices, and the
  screen says which is which. Export CSV writes every call. Nothing is recorded before this shipped.
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
    Heatmap,
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
  };
  const KINDS = ["Input", "Output", "Cache write", "Cache read"] as const;
  /** Rows By feature shows before "Show n more". */
  const FOLDED = 6;
  /** Past this many days, All time draws a bar a week. */
  const DAILY_MAX = 120;
  const HEAT_WEEKS = 12;

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
  const monthShort = new Intl.DateTimeFormat(undefined, { month: "short" });
  const fullDate = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric", year: "numeric" });

  /** Dollars: two decimals ("<$0.01" for a sliver); `fine` (a call's cost) gives three under ten cents. */
  function money(n: number, fine = false): string {
    if (n > 0 && n < 0.001) return fine ? "<$0.001" : "<$0.01";
    if (n > 0 && n < 0.01 && !fine) return "<$0.01";
    return (fine && n < 0.1 ? money3 : money2).format(n);
  }
  /** A gridline's dollars: whole, else cents. */
  const gridMoney = (n: number) => (Number.isInteger(n) ? `$${n}` : `$${n.toFixed(2)}`);
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

  const busiest = $derived([...byDay].sort((a, b) => busier(a[1], b[1])).slice(0, 3));
  /** What a day went on most. */
  function mostly(day: string): string {
    const top = [...sumBy(rows.filter((r) => r.day === day), (r) => r.feature)].sort((a, b) => busier(a[1], b[1]))[0];
    return top ? FEATURES[top[0]] : "";
  }
  /** "Most of your usage lands on Tuesdays", once the range holds a few weeks of it. */
  const weekday = $derived.by(() => {
    if (spanDays < 14 || activeDays < 5) return null;
    const days = Array.from({ length: 7 }, empty);
    for (const r of rows) add(days[date(r.day).getDay()], r);
    const best = days.map((s, i) => ({ s, i })).sort((a, b) => busier(a.s, b.s))[0];
    return `Most of your usage lands on ${weekdayLong.format(addDays(today, best.i - today.getDay()))}s.`;
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

  // ── The chart: a bar a day, or a week once All time runs long ──
  const weekly = $derived(range === "all" && spanDays > DAILY_MAX);
  function dayTip(title: string, s: Sum): TipData {
    if (!s.calls) return { title, value: money(0), note: "No model calls" };
    return {
      title,
      value: money(s.cost),
      rows: [
        ...KINDS.map((label, i) => ({ label, value: tokens(s.tokens[i]) })),
        { label: "Model calls", value: whole.format(s.calls) },
      ],
      note: costNote(s),
    };
  }
  const bars = $derived.by<ChartBar[]>(() => {
    const out: ChartBar[] = [];
    if (weekly) {
      // Weeks from the Sunday on or before the first day; each month's first week names it.
      let lastMonth = -1;
      for (let start = addDays(from, -from.getDay()); start <= today; start = addDays(start, 7)) {
        const s = empty();
        for (let i = 0; i < 7; i++) {
          const day = byDay.get(dayKey(addDays(start, i)));
          if (day) add(s, day);
        }
        const tick = start.getMonth() !== lastMonth ? monthShort.format(start) : undefined;
        lastMonth = start.getMonth();
        out.push({ key: dayKey(start), value: s.cost, tick, tip: dayTip(`Week of ${shortDay.format(start)}`, s) });
      }
      return out;
    }
    // A week names each day; a month each Monday; longer, every other Monday. Today always.
    const every = spanDays <= 7 ? 1 : spanDays <= 31 ? 1 : 2;
    let mondays = 0;
    for (let i = 0; i < spanDays; i++) {
      const d = addDays(from, i);
      const s = byDay.get(dayKey(d)) ?? empty();
      const last = i === spanDays - 1;
      let tick: string | undefined;
      if (last) tick = "Today";
      else if (spanDays <= 7) tick = weekdayShort.format(d);
      else if (d.getDay() === 1 && mondays++ % every === 0 && spanDays - i > 4) tick = shortDay.format(d);
      out.push({ key: dayKey(d), value: s.cost, tick, tip: dayTip(last ? "Today, so far" : longDay.format(d), s) });
    }
    return out;
  });

  // ── The heatmap: the last 12 weeks by cost (by tokens while nothing has a price) ──
  const heatSums = $derived(sumBy(view?.rows ?? [], (r) => r.day));
  const anyPriced = $derived([...heatSums.values()].some((s) => s.cost > 0));
  const heat = $derived(new Map([...heatSums].map(([d, s]) => [d, anyPriced ? s.cost : total(s)])));
  function describeHeat(d: Date): TipData {
    const s = heatSums.get(dayKey(d));
    if (!s) return { title: longDay.format(d), value: money(0), note: "No model calls" };
    return { title: longDay.format(d), value: money(s.cost), note: `${tokens(total(s))} tokens · ${calls(s.calls)}` };
  }

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

<Card pad as="section" id="usage" tabindex={-1} aria-labelledby="usage-title">
  <div class="card">
    <div class="head">
      <h2 class="name" id="usage-title">Usage</h2>
      {#if view?.rows.length}
        <Segmented options={RANGES} bind:value={range} label="Usage over" />
        <Button size="sm" variant="ghost" icon={Download} disabled={exporting} onclick={exportCsv}>Export CSV</Button>
      {/if}
    </div>
    <p class="lede">
      Tokens your model calls used, and what they’d cost at API prices. On a subscription that’s a measure of what you
      used, not your bill.
    </p>

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
      <dl class="stats">
        <div class="stat">
          <dt>Est. cost</dt>
          <dd class="title num">{money(sum.cost)}</dd>
          <dd class="sub num">
            {#if sum.estimated && sum.reported}
              {money(sum.estimated)} of it estimated
            {:else if sum.estimated}
              Estimated from list prices
            {:else if sum.reported}
              Reported by the agent
            {:else}
              No calls {rangeWords}
            {/if}
          </dd>
        </div>
        <div class="stat">
          <dt>Tokens</dt>
          <dd class="title num">{tokens(total(sum))}</dd>
          <dd class="sub num">{percent(sum.tokens[3], total(sum))} cache reads</dd>
        </div>
        <div class="stat">
          <dt>Model calls</dt>
          <dd class="title num">{whole.format(sum.calls)}</dd>
          <dd class="sub num">on {activeDays} of {spanDays} {spanDays === 1 ? "day" : "days"}</dd>
        </div>
        <div class="stat" {@attach tip("Averaged over the days with any model call")}>
          <dt>Per active day</dt>
          <dd class="title num">{money(activeDays ? sum.cost / activeDays : 0)}</dd>
          <dd class="sub num">
            {tokens(activeDays ? total(sum) / activeDays : 0)} tokens · {calls(
              activeDays ? Math.round(sum.calls / activeDays) : 0,
            )}
          </dd>
        </div>
        <div class="stat">
          <dt>Per call</dt>
          <dd class="title num">{money(sum.calls ? sum.cost / sum.calls : 0, true)}</dd>
          <dd class="sub num">{tokens(sum.calls ? total(sum) / sum.calls : 0)} tokens</dd>
        </div>
        <div class="stat">
          <dt>Busiest day</dt>
          {#if busiest[0]}
            <dd class="title num">{shortDay.format(date(busiest[0][0]))}</dd>
            <dd class="sub num">{money(busiest[0][1].cost)} · {calls(busiest[0][1].calls)}</dd>
          {:else}
            <dd class="title num">—</dd>
            <dd class="sub">No calls {rangeWords}</dd>
          {/if}
        </div>
      </dl>

      <section class="group" aria-labelledby="usage-daily">
        <h3 class="label" id="usage-daily">Est. cost by {weekly ? "week" : "day"}</h3>
        <BarChart {bars} format={gridMoney} label="Estimated cost by {weekly ? 'week' : 'day'}, {rangeWords}" />
      </section>

      <section class="group" aria-labelledby="usage-busiest">
        <h3 class="label" id="usage-busiest">Busiest days</h3>
        <div class="busiest">
          <Heatmap
            counts={heat}
            weeks={HEAT_WEEKS}
            end={today}
            start={since ?? undefined}
            label="Estimated cost by day, last {HEAT_WEEKS} weeks"
            describe={(d) => describeHeat(d)}
          />
          <div class="top">
            {#if busiest.length}
              <ol class="days">
                {#each busiest as [day, s] (day)}
                  <li class="day">
                    <span class="day-name">{longDay.format(date(day))}</span>
                    <span class="num end">{money(s.cost)}</span>
                    <span class="meta">{calls(s.calls)} · mostly {mostly(day)}</span>
                    <span class="meta num end">{tokens(total(s))} tokens</span>
                  </li>
                {/each}
              </ol>
            {:else}
              <p class="hint">No calls {rangeWords}.</p>
            {/if}
            {#if weekday}<p class="pattern">{weekday}</p>{/if}
          </div>
        </div>
      </section>

      {#if features.length}
        <section class="group" aria-labelledby="usage-features">
          <table class="breakdown">
            <thead>
              <tr>
                <th scope="col" class="label" id="usage-features">By feature</th>
                <th scope="col" class="label n">Calls</th>
                <th scope="col" class="label n">Tokens</th>
                <th scope="col" class="label n">Est. cost</th>
                <th scope="col" class="label n last">Share of cost</th>
              </tr>
            </thead>
            <tbody>
              {#each shownFeatures as [feature, s] (feature)}
                <tr>
                  <th scope="row">{FEATURES[feature]}</th>
                  <td class="n num">{whole.format(s.calls)}</td>
                  <td class="n num">{tokens(total(s))}</td>
                  <td class="n num" {@attach tip(costNote(s))}>{money(s.cost)}</td>
                  <td class="n">
                    <span class="share">
                      <Meter value={sum.cost ? s.cost / sum.cost : 0} label="{FEATURES[feature]}, share of cost" />
                      <span class="num">{percent(s.cost, sum.cost)}</span>
                    </span>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
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
          <table class="breakdown">
            <thead>
              <tr>
                <th scope="col" class="label" id="usage-models">By model</th>
                <th scope="col" class="label n">Calls</th>
                <th scope="col" class="label n">Tokens</th>
                <th scope="col" class="label n">Est. cost</th>
                <th scope="col" class="label n last">Cost from</th>
              </tr>
            </thead>
            <tbody>
              {#each models as { key, agent, model, s } (key)}
                <tr>
                  <th scope="row">
                    <span class="model">
                      <CompanyMark size={20} src={makerIcon(model) ?? AGENTS[agent].icon} />
                      <span class="model-name">{app.modelName(model)}</span>
                      <span class="meta">{AGENTS[agent].name}</span>
                    </span>
                  </th>
                  <td class="n num">{whole.format(s.calls)}</td>
                  <td class="n num">{tokens(total(s))}</td>
                  <td class="n num">{money(s.cost)}</td>
                  <td class="n from" {@attach tip(costNote(s, AGENTS[agent].name))}>{costFrom(s)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </section>

        <section class="group" aria-labelledby="usage-mix">
          <h3 class="label" id="usage-mix">Token mix</h3>
          <StackedBar segments={KINDS.map((label, i) => ({ label, value: sum.tokens[i] }))} label="Tokens by kind" />
          <dl class="mix">
            {#each KINDS as kind, i (kind)}
              <div class="kind">
                <dt><StepKey step={segmentStep(i)} />{kind}</dt>
                <dd class="num">
                  <span class="kind-tokens">{tokens(sum.tokens[i])}</span>
                  <span class="meta">{percent(sum.tokens[i], total(sum))}</span>
                </dd>
                {#if priced}
                  <dd class="meta num">{money(sum.byKind[i])} · {percent(sum.byKind[i], priced)} of cost</dd>
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
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    min-height: var(--control-sm);
  }
  .name {
    flex: 1;
    min-width: 0;
    margin: 0;
    font-size: var(--text-lg);
    line-height: var(--leading-lg);
    letter-spacing: var(--tracking-lg);
    font-weight: var(--weight-medium);
  }
  .lede,
  .hint,
  .problem,
  .pattern,
  .none-title {
    margin: 0;
    text-wrap: pretty;
  }
  .lede,
  .pattern {
    color: var(--ink-2);
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
  .foot {
    margin-top: var(--space-5);
    padding-top: var(--space-3);
    box-shadow: inset 0 1px 0 var(--line);
  }

  /* Overview's stats: three to a row, hairlines between the rows. */
  .stats {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--space-3) var(--space-4);
    margin: var(--space-3) 0 0;
    padding-top: var(--space-3);
    box-shadow: inset 0 1px 0 var(--line);
  }
  .stat {
    display: grid;
    align-content: start;
    gap: var(--space-0-5);
    min-width: 0;
  }
  .stat:nth-child(n + 4) {
    padding-top: var(--space-3);
    box-shadow: inset 0 1px 0 var(--line);
  }
  .stat dt,
  .sub {
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
  }
  .stat dt {
    color: var(--ink-2);
  }
  .sub {
    color: var(--ink-3);
  }
  .stat dd {
    min-width: 0;
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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

  .busiest {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    gap: var(--space-6);
    align-items: start;
  }
  .top {
    display: grid;
    gap: var(--space-3);
    min-width: 0;
  }
  .days {
    display: grid;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .day {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    column-gap: var(--space-3);
    padding-block: var(--space-2);
    box-shadow: inset 0 -1px 0 var(--line);
  }
  .day:first-child {
    padding-top: 0;
  }
  .day > span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .end {
    text-align: end;
  }

  /* A breakdown: its label row heads the columns; rows on hairlines, numbers at their trailing edge. */
  .breakdown {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
  }
  .breakdown th,
  .breakdown td {
    overflow: hidden;
    padding: var(--space-2) 0;
    text-align: start;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: var(--weight-regular);
  }
  .breakdown thead th {
    width: 72px;
    padding-top: 0;
  }
  .breakdown thead th:first-child {
    width: auto;
  }
  .breakdown thead th.last {
    width: 112px;
  }
  .breakdown tbody tr {
    box-shadow: inset 0 1px 0 var(--line);
  }
  .breakdown .n {
    padding-inline-start: var(--space-3);
    text-align: end;
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

  /* Narrow: two stats to a row, the heatmap over the busiest days, the mix in two columns. */
  @container (width < 520px) {
    .stats {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .stat:nth-child(n + 3) {
      padding-top: var(--space-3);
      box-shadow: inset 0 1px 0 var(--line);
    }
    .busiest {
      grid-template-columns: minmax(0, 1fr);
    }
    .mix {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
</style>
