<!--
  `/` is the Overview: a greeting, your activity (postings added and applications sent: counts over a range, and the
  last 26 weeks by day) and what's up next (applications in interviews, then postings not sent yet, best fit first).
  V (or ⌘V) pastes a posting, I imports a resume.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import ClipboardPaste from "@lucide/svelte/icons/clipboard-paste";
  import FileUp from "@lucide/svelte/icons/file-up";
  import type { AppStatus } from "$lib/api";
  import { chooseResume } from "$lib/import";
  import { bindKeys } from "$lib/keys";
  import { paste } from "$lib/paste.svelte";
  import { app } from "$lib/stores/app.svelte";
  import { STATUS } from "$lib/status";
  import { Badge, Button, Card, CompanyMark, Heatmap, Icon, Kbd, Segmented, Toolbar, dayKey } from "$lib/ui";

  type Range = "all" | 30 | 7;
  const RANGES: { value: Range; label: string }[] = [
    { value: "all", label: "All" },
    { value: 30, label: "30 days" },
    { value: 7, label: "7 days" },
  ];
  const WEEKS = 26;
  const NEXT = 5;
  const WEEKDAYS = ["Sundays", "Mondays", "Tuesdays", "Wednesdays", "Thursdays", "Fridays", "Saturdays"];

  let range = $state<Range>("all");

  onMount(() => {
    void Promise.all([app.refreshJobs(), app.refreshApplications(), app.refreshLibrary()]);
  });

  const importResume = () => chooseResume(app.settings?.exportDir);

  $effect(() => bindKeys({ v: paste.run, "mod+v": paste.run, i: () => void importResume() }));

  /** "Jane (JJ) Doe" → "Bo"; else the first name. */
  function firstName(full: string): string {
    const nick = full.match(/\(([^)]+)\)/)?.[1]?.trim();
    return nick || full.trim().split(/\s+/)[0] || "";
  }
  const name = $derived(app.library?.bank ? firstName(app.library.bank.name) : "");

  // ── Activity: postings by the day they were added, applications by the day they were sent ──
  const today = new Date();
  const jobDay = (addedAt: string) => dayKey(new Date(addedAt));
  const tally = (days: string[]) => {
    const counts = new Map<string, number>();
    for (const day of days) counts.set(day, (counts.get(day) ?? 0) + 1);
    return counts;
  };
  const added = $derived(tally(app.jobs.map((j) => jobDay(j.addedAt))));
  const sent = $derived(tally(app.applications.map((a) => a.date)));
  const activity = $derived.by(() => {
    const counts = new Map(added);
    for (const [day, n] of sent) counts.set(day, (counts.get(day) ?? 0) + n);
    return counts;
  });

  /** The first day in the range; "" (before every day) for All. */
  const since = $derived(
    range === "all" ? "" : dayKey(new Date(today.getFullYear(), today.getMonth(), today.getDate() - range + 1)),
  );
  const mean = (ns: number[]) => (ns.length ? ns.reduce((a, b) => a + b, 0) / ns.length : null);

  const stats = $derived.by(() => {
    const jobs = app.jobs.filter((j) => jobDay(j.addedAt) >= since);
    const apps = app.applications.filter((a) => a.date >= since);
    const days = [...activity.keys()].filter((d) => d >= since);
    const fits = apps.flatMap((a) => {
      const fit = a.onFile ?? a.projected;
      return fit === null ? [] : [fit];
    });
    const lifts = apps.flatMap((a) => (a.now === null || a.projected === null ? [] : [a.projected - a.now]));
    return {
      postings: jobs.length,
      applications: apps.length,
      interviews: apps.filter((a) => a.status === "interviewing" || a.status === "offer").length,
      days,
      fit: mean(fits),
      lift: mean(lifts),
    };
  });

  const note = $derived.by(() => {
    if (!activity.size) return "Paste a job link or a posting’s text to start.";
    if (!stats.days.length) return `Nothing in the last ${range} days.`;
    if (stats.days.length < 3) return null;
    const byWeekday = Array<number>(7).fill(0);
    for (const day of stats.days) {
      const [y, m, d] = day.split("-").map(Number);
      byWeekday[new Date(y, m - 1, d).getDay()] += activity.get(day) ?? 0;
    }
    return `Most of your activity lands on ${WEEKDAYS[byWeekday.indexOf(Math.max(...byWeekday))]}.`;
  });

  const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
  const longDay = new Intl.DateTimeFormat(undefined, { weekday: "short", month: "short", day: "numeric" });
  function describe(date: Date, count: number): string {
    if (!count) return `${longDay.format(date)}: nothing`;
    const key = dayKey(date);
    const parts = [];
    const p = added.get(key);
    const a = sent.get(key);
    if (p) parts.push(plural(p, "posting", "postings"));
    if (a) parts.push(plural(a, "application", "applications"));
    return `${longDay.format(date)}: ${parts.join(", ")}`;
  }

  // ── Up next: interviews first, then postings not sent yet, best fit first ──
  interface Next {
    key: string;
    href: string;
    company: string;
    role: string;
    status: AppStatus | null;
    score: number | null;
    day: string;
  }
  const interviewing = $derived<Next[]>(
    app.applications
      .filter((a) => a.status === "interviewing")
      .map((a) => ({
        key: `application-${a.id}`,
        href: a.job ? `/jobs/${a.job}` : "/applications",
        company: a.company,
        role: a.role,
        status: a.status,
        score: a.onFile ?? a.projected,
        day: a.date,
      })),
  );
  const waiting = $derived<Next[]>(
    app.jobs
      .filter((j) => j.status === null || j.status === "saved")
      .sort((a, b) => (b.score ?? -1) - (a.score ?? -1))
      .map((j) => ({
        key: `job-${j.id}`,
        href: `/jobs/${j.id}`,
        company: j.company,
        role: j.role,
        status: j.status,
        score: j.score,
        day: jobDay(j.addedAt),
      })),
  );
  const next = $derived([...interviewing, ...waiting].slice(0, NEXT));
  const noBank = $derived(app.library !== null && app.library.bank === null);
  const nextMeta = $derived(
    [
      interviewing.length ? `${interviewing.length} interviewing` : "",
      waiting.length ? `${waiting.length} not sent` : "",
    ]
      .filter(Boolean)
      .join(" · "),
  );

  const THIS_YEAR = today.getFullYear();
  function shortDay(key: string): string {
    const [y, m, d] = key.split("-").map(Number);
    return new Date(y, m - 1, d).toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      year: y === THIS_YEAR ? undefined : "numeric",
    });
  }
  const percent = (n: number | null) => (n === null ? "—" : `${Math.round(n)}%`);
</script>

<Toolbar>
  {#snippet crumbs()}<b>Overview</b>{/snippet}
  <Button icon={FileUp} kbd="I" onclick={importResume}>Import resume</Button>
  <Button variant="primary" icon={ClipboardPaste} kbd="V" disabled={paste.reading} onclick={paste.run}>
    {paste.reading ? "Reading…" : "Paste from clipboard"}
  </Button>
</Toolbar>

<div class="scroll">
  <div class="column">
    <h1 class="headline">What’s up next{name ? `, ${name}` : ""}?</h1>

    <Card pad as="section" aria-labelledby="activity">
      <div class="activity">
        <header class="head">
          <h2 class="heading" id="activity">Activity</h2>
          <Segmented options={RANGES} bind:value={range} label="Activity over" />
        </header>
        <dl class="stats">
          <div class="stat">
            <dt>Postings</dt>
            <dd class="title num">{stats.postings}</dd>
          </div>
          <div class="stat">
            <dt>Applications</dt>
            <dd class="title num">{stats.applications}</dd>
          </div>
          <div class="stat">
            <dt>Interviews</dt>
            <dd class="title num">{stats.interviews}</dd>
          </div>
          <div class="stat">
            <dt>Active days</dt>
            <dd class="title num">{stats.days.length}</dd>
          </div>
          <div class="stat">
            <dt title="On file, else projected">Average fit</dt>
            <dd class="title num">{percent(stats.fit)}</dd>
          </div>
          <div class="stat">
            <dt title="Projected minus now, per application">Points from tailoring</dt>
            <dd class={["title", "num", stats.lift !== null && stats.lift > 0 && "gain"]}>
              {#if stats.lift === null}—{:else}{stats.lift > 0 ? "+" : ""}{Math.round(stats.lift)}{/if}
            </dd>
          </div>
        </dl>
        <Heatmap counts={activity} weeks={WEEKS} end={today} label="Postings added and applications sent, last {WEEKS} weeks" {describe}>
          {#snippet caption()}{#if note}<p class="note">{note}</p>{/if}{/snippet}
        </Heatmap>
      </div>
    </Card>

    <section class="next" aria-labelledby="next">
      <header class="head">
        <h2 class="heading" id="next">Up next</h2>
        {#if nextMeta}<span class="meta num">{nextMeta}</span>{/if}
      </header>
      <Card>
        <div class="list">
          {#if noBank}
            <button type="button" class="row row--action" onclick={importResume}>
              <span class="lead"><Icon icon={FileUp} size={16} weight="medium" /></span>
              <span class="company">Import your resume</span>
              <span class="role">Resumes and fit scores start from your experience.</span>
              <Kbd>I</Kbd>
            </button>
          {/if}
          {#each next as item (item.key)}
            <a class="row" href={item.href}>
              <span class="company"><CompanyMark><span title={item.company}>{item.company}</span></CompanyMark></span>
              <span class="role" title={item.role}>{item.role}</span>
              <span class="status">
                {#if item.status}<Badge tone={STATUS[item.status].tone}>{STATUS[item.status].label}</Badge>{/if}
              </span>
              <span class="score num" title="Fit score">{percent(item.score)}</span>
              <span class="day meta num">{shortDay(item.day)}</span>
            </a>
          {:else}
            {#if !noBank}
              <p class="none">Nothing waiting. Paste a job link or a posting’s text and it lands here.</p>
            {/if}
          {/each}
        </div>
      </Card>
    </section>
  </div>
</div>

<style>
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    overscroll-behavior: contain;
  }
  .column {
    display: grid;
    gap: var(--space-6);
    max-width: 880px;
    margin-inline: auto;
    padding: var(--space-10) var(--space-6) var(--space-12);
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: var(--control-md);
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

  .activity {
    display: grid;
    gap: var(--space-5);
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--space-4) var(--space-6);
    margin: 0;
  }
  .stat {
    display: grid;
    gap: var(--space-0-5);
    min-width: 0;
    padding-top: var(--space-3);
    box-shadow: inset 0 1px 0 var(--line);
  }
  dt {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink-3);
  }
  dd {
    margin: 0;
  }
  .gain {
    color: var(--gain-text);
  }
  .note {
    margin: 0;
  }

  .next {
    display: grid;
    gap: var(--space-2);
  }
  /* Inside the card's 1px border: 12 - 1, so a row's hover wash follows the corners. */
  .list {
    display: grid;
    overflow: hidden;
    border-radius: calc(var(--radius-lg) - 1px);
  }
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.6fr) 112px 48px 64px;
    align-items: center;
    gap: var(--space-3);
    min-height: 44px;
    padding: var(--space-1-5) var(--space-5);
    color: var(--ink);
    text-decoration: none;
    cursor: default;
    -webkit-user-drag: none;
    transition: background-color var(--dur-instant) var(--ease-out);
  }
  .row + .row {
    box-shadow: inset 0 1px 0 var(--line);
  }
  .row--action {
    grid-template-columns: 20px auto minmax(0, 1fr) auto;
    width: 100%;
    border: 0;
    background: none;
    font: inherit;
    text-align: start;
  }
  .lead {
    display: grid;
    place-items: center;
    color: var(--accent-text);
  }
  .company {
    min-width: 0;
    font-weight: var(--weight-medium);
  }
  .role {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink-2);
  }
  .status {
    display: flex;
  }
  .score {
    text-align: end;
    color: var(--ink-2);
  }
  .day {
    text-align: end;
  }
  .none {
    margin: 0;
    padding: var(--space-4) var(--space-5);
    color: var(--ink-3);
    text-wrap: pretty;
  }
  @media (hover: hover) {
    .row:hover {
      background: var(--hover);
    }
  }
</style>
