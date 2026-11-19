<!--
  Settings → Benchmark's overview, above the role-by-role leaderboard: which route does gliide's work best overall, and
  at what cost and speed, as Artificial Analysis lays out its Intelligence Index. Four sections, each a heading and a
  sentence over a card: Overall (three small leaderboards side by side: the gliide Index, cost to run the suite, time
  per posting, then the verdict in a sunken well), gliide Index (each route's Index as its roles' parts end to end),
  Quality against cost and time (two scatters), and Over time (a role's quality per route at each snapshot: the
  reference, then each day this Mac ran it). The app computes every number (`benchmark()`'s `index` and `history`);
  this only lays them out. Routes are named by model with their maker's mark and drawn in their maker's color.
-->
<script lang="ts">
  import type { BenchHistory, BenchmarkView, IndexRoute, Role } from "$lib/api";
  import { AGENTS, makerOf, ROLES } from "$lib/agents";
  import { app } from "$lib/stores/app.svelte";
  import {
    Card,
    CompanyMark,
    LineTrend,
    RankBars,
    type RankItem,
    Scatter,
    type ScatterAxis,
    type ScatterPoint,
    Segmented,
    segmentStep,
    SeriesKey,
    StackRows,
    type StackRow,
    StepKey,
    type TipData,
    type TrendLine,
  } from "$lib/ui";

  interface Props {
    view: BenchmarkView;
  }

  let { view }: Props = $props();

  const ROLE_NAME = Object.fromEntries(ROLES.map((r) => [r.id, r.name])) as Record<Role, string>;
  /** resume-bench's Tailoring quality for an empty suggestion: grounded and fits count as 1, lift as 0. */
  const TAILOR_FLOOR = 200 / 3;

  // ── Formatting ──
  const shortDay = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });
  const one = (x: number) => x.toFixed(1);
  const money = (c: number) => (c >= 0.1 ? `$${c.toFixed(2)}` : `$${c.toFixed(3)}`);
  const secs = (s: number) => `${Math.round(s)} s`;
  /** A tick's value without trailing zeros: $0.1, $1, $10. */
  const tickMoney = (c: number) => `$${Number(c.toPrecision(2))}`;
  function andList(words: string[]) {
    return words.length < 2 ? (words[0] ?? "") : `${words.slice(0, -1).join(", ")} and ${words.at(-1)}`;
  }
  function day(at: string) {
    const ymd = /^(\d{4})-(\d{2})-(\d{2})$/.exec(at);
    const d = ymd ? new Date(+ymd[1], +ymd[2] - 1, +ymd[3]) : new Date(at);
    return Number.isNaN(d.getTime()) ? at : shortDay.format(d);
  }

  // ── Routes ──
  const rid = (r: { agent: string; model: string }) => `${r.agent}:${r.model}`;
  const nameOf = (r: { model: string }) => app.modelName(r.model);
  /** The route's maker: its mark, its series color, its name; the agent's when the model's maker isn't known. */
  function who(r: { agent: IndexRoute["agent"]; model: string }) {
    const m = makerOf(r.model);
    return {
      icon: m?.icon ?? AGENTS[r.agent].icon,
      color: m ? `var(--maker-${m.id})` : "var(--ink-3)",
      maker: m?.name ?? AGENTS[r.agent].name,
    };
  }

  const index = $derived(view.index);
  const routes = $derived(index.routes);
  const complete = $derived(routes.filter((r) => r.complete));
  const target = $derived(index.targetS);
  const counted = $derived(index.roles.map((r) => ROLE_NAME[r]));

  /** What stands in for a route: the roles it hasn't run. */
  const needs = (r: IndexRoute) => r.parts.filter((p) => p.placeholder).map((p) => ROLE_NAME[p.role]);

  function indexTip(r: IndexRoute): TipData {
    return {
      title: nameOf(r),
      value: one(r.index),
      rows: r.parts.map((p, i) => ({
        label: ROLE_NAME[p.role],
        value: `${one(p.value)}${p.placeholder ? " stand-in" : ""}`,
        step: segmentStep(i),
      })),
      note: needs(r).length
        ? `${andList(needs(r))} not run: the median of the routes that ran it stands in.`
        : `${andList(counted)}, a third each.`,
    };
  }

  // ── Overall: three small leaderboards ──
  const byValue = (list: IndexRoute[], f: (r: IndexRoute) => number | null, low: boolean) =>
    [...list]
      .filter((r) => f(r) !== null)
      .sort((a, b) => Number(b.complete) - Number(a.complete) || (low ? 1 : -1) * (f(a)! - f(b)!));

  const qualityItems = $derived<RankItem[]>(
    routes.map((r) => ({
      key: rid(r),
      ...who(r),
      name: nameOf(r),
      value: r.index,
      text: one(r.index),
      partial: !r.complete,
      tip: indexTip(r),
    })),
  );
  const costs = $derived(routes.flatMap((r) => (r.costUsd === null ? [] : [r.costUsd])));
  /** Routes that ran a role without a price: their suite's cost is unknown, so they're left off cost. */
  const unpriced = $derived(routes.filter((r) => r.costUsd === null));
  const costItems = $derived<RankItem[]>(
    byValue(routes, (r) => r.costUsd, true).map((r) => ({
      key: rid(r),
      ...who(r),
      name: nameOf(r),
      value: r.costUsd!,
      text: money(r.costUsd!),
      partial: !r.complete,
      estimated: r.costEstimated,
      tip: {
        title: nameOf(r),
        value: money(r.costUsd!),
        note: r.costEstimated
          ? "One full suite at API prices. A role it hasn’t run is priced at the median of the routes that have, scaled by how its prices compare on the roles it ran."
          : "One full suite at API prices: Reading and Tailoring 36 calls each, Coaching 12.",
      },
    })),
  );
  const times = $derived(routes.flatMap((r) => (r.seconds === null ? [] : [r.seconds])));
  const timeItems = $derived<RankItem[]>(
    byValue(routes, (r) => r.seconds, true).map((r) => ({
      key: rid(r),
      ...who(r),
      name: nameOf(r),
      value: r.seconds!,
      text: secs(r.seconds!),
      partial: !r.complete,
      estimated: r.secondsEstimated,
      tip: {
        title: nameOf(r),
        value: secs(r.seconds!),
        note: `${r.seconds! <= target ? "Inside" : "Over"} the ${secs(target)} target: one posting through ${andList(counted)}, each role’s typical call${r.secondsEstimated ? " (a role it hasn’t run at the median, scaled by its pace on the others)" : ""}.`,
      },
    })),
  );

  /** The verdict, as runs of plain and bold text. */
  const verdict = $derived.by(() => {
    const best = complete[0] ?? routes[0];
    if (!best) return [];
    const out: { text: string; bold?: boolean }[] = [{ text: nameOf(best), bold: true }];
    out.push({
      text: ` leads: the highest Index${best.complete ? " of the routes that ran every role" : ""} (${one(best.index)})`,
    });
    if (best.seconds !== null)
      out.push({
        text:
          best.seconds <= target
            ? `, inside the ${secs(target)} target at ${secs(best.seconds)} a posting`
            : `, at ${secs(best.seconds)} a posting against the ${secs(target)} target`,
      });
    out.push({ text: best.costUsd !== null ? `, for ${money(best.costUsd)} a suite.` : "." });
    const priced = complete.filter((r) => r.costUsd !== null);
    const cheapest = priced.sort((a, b) => a.costUsd! - b.costUsd!)[0];
    if (cheapest && rid(cheapest) !== rid(best)) {
      out.push({ text: " " });
      out.push({ text: nameOf(cheapest), bold: true });
      out.push({
        text: ` costs least, ${money(cheapest.costUsd!)}, at an Index ${one(best.index - cheapest.index)} lower.`,
      });
    }
    return out;
  });

  const makers = $derived(
    [...new Map(routes.map((r) => [who(r).maker, who(r)])).values()].sort((a, b) => a.maker.localeCompare(b.maker)),
  );

  // ── gliide Index ──
  const indexRows = $derived<StackRow[]>(
    routes.map((r) => ({
      key: rid(r),
      icon: who(r).icon,
      name: nameOf(r),
      partial: !r.complete,
      text: one(r.index),
      aside: needs(r).length ? `needs ${andList(needs(r))}` : undefined,
      parts: r.parts.map((p) => ({
        label: ROLE_NAME[p.role],
        value: p.value / r.parts.length,
        placeholder: p.placeholder,
        tip: `${ROLE_NAME[p.role]}: ${one(p.value / r.parts.length)} points, from ${one(p.value)} of 100${p.placeholder ? " (a stand-in: the median of the routes that ran it)" : ""}`,
      })),
      tip: indexTip(r),
    })),
  );
  /** A role every route aces separates none of them: say so. */
  const saturated = $derived.by(() => {
    for (const [i, role] of index.roles.entries()) {
      const vals = routes.flatMap((r) => (r.parts[i] && !r.parts[i].placeholder ? [r.parts[i].value] : []));
      if (vals.length >= 3 && Math.min(...vals) >= 95) {
        const n = vals.length;
        return `${ROLE_NAME[role]} scores ${Math.floor(Math.min(...vals))}–${Math.round(Math.max(...vals))} for every route that ran it, so it adds about ${Math.round(vals.reduce((a, b) => a + b, 0) / n / index.roles.length)} points to each and separates none of them.`;
      }
    }
    return null;
  });

  // ── Quality against cost and time ──
  /** Ticks at 1, 2 and 5 of each decade in [lo, hi]; decades alone when that's three or more. */
  function logTicks(lo: number, hi: number) {
    const out: number[] = [];
    const decades: number[] = [];
    for (let e = Math.floor(Math.log10(lo)); e <= Math.ceil(Math.log10(hi)); e++) {
      for (const m of [1, 2, 5]) {
        const v = m * 10 ** e;
        if (v >= lo && v <= hi) (m === 1 ? decades : out).push(v);
      }
    }
    return decades.length >= 3 ? decades : [...decades, ...out].sort((a, b) => a - b);
  }
  const yAxis = $derived.by((): ScatterAxis => {
    const vals = routes.map((r) => r.index);
    const lo = Math.max(0, Math.floor((Math.min(...vals) - 2) / 10) * 10);
    const hi = Math.min(100, Math.ceil((Math.max(...vals) + 2) / 10) * 10);
    const ticks = [];
    for (let t = lo; t <= hi; t += 10) ticks.push(t);
    return { min: lo, max: hi, ticks, format: (v) => String(Math.round(v)) };
  });
  const median = (v: number[]) => {
    const s = [...v].sort((a, b) => a - b);
    return s.length % 2 ? s[(s.length - 1) / 2] : (s[s.length / 2 - 1] + s[s.length / 2]) / 2;
  };
  /** Quality against `f`: points, the x axis, the quadrant at both medians, the frontier of complete routes. */
  function plot(
    f: (r: IndexRoute) => number | null,
    estimated: (r: IndexRoute) => boolean,
    x: Omit<ScatterAxis, "min" | "max" | "ticks">,
  ) {
    const on = routes.filter((r) => f(r) !== null);
    const xs = on.map((r) => f(r)!);
    const lo = Math.min(...xs) / 1.8;
    const hi = Math.max(...xs) * 1.8;
    const points: ScatterPoint[] = [...on]
      .sort((a, b) => b.index - a.index)
      .map((r) => ({
        key: rid(r),
        x: f(r)!,
        y: r.index,
        label: nameOf(r),
        color: who(r).color,
        hollow: !r.complete || estimated(r),
        tip: {
          title: nameOf(r),
          rows: [
            { label: "Index", value: one(r.index) },
            { label: x.title ?? "", value: x.format(f(r)!) },
          ],
          note: !r.complete
            ? `Partial: ${andList(needs(r))} stand in.`
            : estimated(r)
              ? "A stand-in in its value."
              : undefined,
        },
      }));
    const frontier: string[] = [];
    let top = -1;
    for (const r of [...on.filter((r) => r.complete)].sort((a, b) => f(a)! - f(b)!))
      if (r.index > top) {
        frontier.push(rid(r));
        top = r.index;
      }
    return {
      points,
      x: { ...x, min: lo, max: hi, log: true, ticks: logTicks(lo, hi) } as ScatterAxis,
      quadrant: { x: median(xs), y: median(on.map((r) => r.index)), label: "Most attractive" },
      frontier,
    };
  }
  const costPlot = $derived(
    costs.length
      ? plot(
          (r) => r.costUsd,
          (r) => r.costEstimated,
          { format: tickMoney, title: "Cost to run the suite (log)" },
        )
      : null,
  );
  const timePlot = $derived(
    times.length
      ? plot(
          (r) => r.seconds,
          (r) => r.secondsEstimated,
          { format: (v) => `${Number(v.toPrecision(2))} s`, title: "Time per posting (log)" },
        )
      : null,
  );

  // ── Over time ──
  const histories = $derived(view.history.filter((h) => h.routes.length));
  let trendRole = $state<Role>("tailor");
  const trend = $derived<BenchHistory | null>(histories.find((h) => h.role === trendRole) ?? histories[0] ?? null);
  const snapshotLabels = $derived(
    (trend?.snapshots ?? []).map((s) => `${day(s.at)} · ${s.source === "reference" ? "reference" : "this Mac"}`),
  );
  const trendLines = $derived.by((): TrendLine[] => {
    if (!trend) return [];
    const seen = new Map<string, number>();
    const points = (r: { values: (number | null)[] }) => r.values.filter((v) => v !== null).length;
    // Within a maker, the route with the most snapshots draws solid, the next dashed, then dotted.
    return [...trend.routes]
      .sort((a, b) => points(b) - points(a) || nameOf(a).localeCompare(nameOf(b)))
      .map((r) => {
        const w = who(r);
        const nth = seen.get(w.maker) ?? 0;
        seen.set(w.maker, nth + 1);
        return {
          key: rid(r),
          label: nameOf(r),
          color: w.color,
          dash: (["solid", "dashed", "dotted"] as const)[nth % 3],
          values: r.values,
          tip: (i: number) => `${nameOf(r)} · ${snapshotLabels[i]}: ${one(r.values[i] ?? 0)}`,
        };
      });
  });
  const trendY = $derived.by(() => {
    const vals = (trend?.routes ?? []).flatMap((r) => r.values.filter((v): v is number => v !== null));
    if (trend?.role === "tailor") vals.push(TAILOR_FLOOR);
    const lo = vals.length ? Math.max(0, Math.floor((Math.min(...vals) - 2) / 10) * 10) : 0;
    const hi = vals.length ? Math.min(100, Math.ceil((Math.max(...vals) + 2) / 10) * 10) : 100;
    const ticks = [];
    for (let t = lo; t <= hi; t += 10) ticks.push(t);
    return { min: lo, max: hi, ticks, format: (v: number) => String(Math.round(v)), value: one };
  });
  const trendOptions = $derived(histories.map((h) => ({ value: h.role, label: ROLE_NAME[h.role] })));
</script>

{#if routes.length}
  <section class="section" aria-labelledby="bench-overall">
    <div class="section__head">
      <h3 id="bench-overall">Overall</h3>
      <p>The routes that ran every role first, then the partial ones, whose missing roles stand in at the median.</p>
    </div>
    <Card pad>
      <div class="boards">
        <section class="board" aria-labelledby="bench-board-quality">
          <h4 id="bench-board-quality">Quality</h4>
          <p class="meta">gliide Index · higher is better</p>
          <RankBars items={qualityItems} scale={{ max: 100 }} label="Quality, gliide Index" />
        </section>
        {#if costItems.length}
          <section class="board" aria-labelledby="bench-board-cost">
            <h4 id="bench-board-cost">Cost to run</h4>
            <p class="meta">One suite · lower is better</p>
            <RankBars
              items={costItems}
              scale={{ log: true, min: Math.min(...costs) / 3, max: Math.max(...costs) * 1.2 }}
              label="Cost to run the suite"
            />
            {#if unpriced.length}
              <p class="meta unpriced">No price yet: {andList(unpriced.map(nameOf))}</p>
            {/if}
          </section>
        {/if}
        {#if timeItems.length}
          <section class="board" aria-labelledby="bench-board-time">
            <h4 id="bench-board-time">Time per posting</h4>
            <p class="meta num">Typical, end to end · target {secs(target)}</p>
            <RankBars
              items={timeItems}
              scale={{ max: Math.max(target, ...times) * 1.05 }}
              target={{ value: target, label: `target ${secs(target)}` }}
              label="Time per posting"
            />
          </section>
        {/if}
      </div>
      <p class="verdict">
        {#each verdict as v, i (i)}{#if v.bold}<b>{v.text}</b>{:else}{v.text}{/if}{/each}
      </p>
      <p class="keys meta">
        {#each makers as m (m.maker)}
          <span class="key"><CompanyMark size={16} src={m.icon} /><SeriesKey kind="bar" color={m.color} />{m.maker}</span>
        {/each}
        <span class="key"><SeriesKey kind="outline" />A stand-in in the value</span>
      </p>
    </Card>
  </section>

  <section class="section" aria-labelledby="bench-index">
    <div class="section__head">
      <h3 id="bench-index">gliide Index</h3>
      <p>
        One quality score per route, like Artificial Analysis’s Intelligence Index: {andList(counted)} count
        {counted.length === 3 ? "a third" : "an equal share"} each, a failed run scores 0, and Tailoring is rebased so an
        empty suggestion scores 0.
      </p>
    </div>
    <Card pad>
      <p class="keys meta">
        {#each index.roles as role, i (role)}
          <span class="key"><StepKey step={segmentStep(i)} />{ROLE_NAME[role]}</span>
        {/each}
        <span class="key"><SeriesKey kind="outline" color="var(--accent-step-3)" />Stand-in</span>
      </p>
      <StackRows rows={indexRows} label="gliide Index by route" />
      {#if saturated}<p class="note meta">{saturated} It needs a harder measure.</p>{/if}
    </Card>
  </section>

  {#if costPlot || timePlot}
    <section class="section" aria-labelledby="bench-scatter">
      <div class="section__head">
        <h3 id="bench-scatter">Quality against cost and time</h3>
        <p>
          Up and to the left is better. The shaded corner beats the median on both; the dashed line joins the routes no
          cheaper one beats on quality.
        </p>
      </div>
      <Card pad>
        <p class="keys meta">
          {#each makers as m (m.maker)}
            <span class="key"><CompanyMark size={16} src={m.icon} /><SeriesKey color={m.color} />{m.maker}</span>
          {/each}
          <span class="key"><SeriesKey kind="ring" />Partial, or a stand-in in it</span>
        </p>
        <div class="pair">
          {#if costPlot}
            <Scatter
              points={costPlot.points}
              x={costPlot.x}
              y={yAxis}
              quadrant={costPlot.quadrant}
              frontier={costPlot.frontier}
              label="gliide Index against the cost to run the suite"
            />
          {/if}
          {#if timePlot}
            <Scatter
              points={timePlot.points}
              x={timePlot.x}
              y={yAxis}
              quadrant={timePlot.quadrant}
              target={{ x: target, label: "target" }}
              label="gliide Index against the time per posting"
            />
          {/if}
        </div>
      </Card>
    </section>
  {/if}

  {#if trend}
    <section class="section" aria-labelledby="bench-trend">
      <div class="section__head">
        <h3 id="bench-trend">Over time</h3>
        <p>A role’s quality per route at each snapshot: gliide’s reference, then each day it ran on this Mac.</p>
      </div>
      <Card pad>
        <div class="trend__bar">
          <Segmented options={trendOptions} value={trend.role} label="Role" onchange={(r) => (trendRole = r)} />
        </div>
        {#if trend.snapshots.length > 1}
          <p class="keys trend__keys meta">
            {#each trendLines as l (l.key)}
              <span class="key"
                ><SeriesKey kind={l.dash === "solid" || !l.dash ? "line" : l.dash} color={l.color} />{l.label}</span
              >
            {/each}
          </p>
          <LineTrend
            lines={trendLines}
            snapshots={snapshotLabels}
            y={trendY}
            floor={trend.role === "tailor" ? { value: TAILOR_FLOOR, label: "An empty suggestion scores 66.7" } : undefined}
            label="{ROLE_NAME[trend.role]} quality by snapshot"
          />
        {:else}
          <p class="note meta">
            One snapshot so far, the reference of {day(trend.snapshots[0]?.at ?? view.referenceDate)}. Each day you run
            {ROLE_NAME[trend.role]} on this Mac adds one here.
          </p>
        {/if}
      </Card>
    </section>
  {/if}
{/if}

<style>
  p {
    margin: 0;
  }
  .section {
    display: grid;
    gap: var(--space-3);
  }
  .section__head {
    display: grid;
    gap: var(--space-1);
  }
  .section__head h3 {
    margin: 0;
    font-size: var(--text-lg);
    line-height: var(--leading-lg);
    letter-spacing: var(--tracking-lg);
    font-weight: var(--weight-medium);
  }
  .section__head p {
    color: var(--ink-2);
    text-wrap: pretty;
  }

  /* Three boards side by side, hairlines between; stacked when there's no room for three. */
  .boards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(176px, 1fr));
    gap: var(--space-6) 0;
  }
  .board {
    min-width: 0;
    padding: 0 var(--space-5);
    border-left: 1px solid var(--line);
  }
  .board:first-child {
    padding-left: 0;
    border-left: 0;
  }
  .board:last-child {
    padding-right: 0;
  }
  .board h4 {
    margin: 0;
    font-size: var(--text-base);
    line-height: var(--leading-base);
    font-weight: var(--weight-medium);
  }
  .board .meta {
    margin-bottom: var(--space-3);
  }
  .unpriced {
    margin-top: var(--space-3);
    text-wrap: pretty;
  }
  .verdict {
    margin-top: var(--space-5);
    padding: var(--space-3);
    border-radius: var(--radius-md);
    background: var(--sunken);
    color: var(--ink-2);
    text-wrap: pretty;
  }
  .verdict b {
    color: var(--ink);
    font-weight: var(--weight-medium);
  }

  .keys {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1) var(--space-4);
  }
  .verdict + .keys {
    margin-top: var(--space-3);
  }
  .keys:first-child {
    margin-bottom: var(--space-4);
  }
  .key {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1-5);
  }
  .note {
    margin-top: var(--space-3);
    text-wrap: pretty;
  }
  .pair {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: var(--space-6);
  }
  .trend__bar {
    margin-bottom: var(--space-4);
  }
  .trend__keys {
    margin-bottom: var(--space-4);
  }
</style>
