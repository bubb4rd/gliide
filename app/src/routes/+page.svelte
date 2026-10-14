<!--
  `/` is the Overview: a greeting, your activity (postings added and applications sent: counts over a range, and the
  last 26 weeks by day), what's up next (applications in interviews, then postings not sent yet, best fit first) and
  what's new for you (the feed's fits, best first; hidden while the feed is off).
  V (or ⌘V) pastes a posting, I imports a resume.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import ClipboardPaste from "@lucide/svelte/icons/clipboard-paste";
  import FileUp from "@lucide/svelte/icons/file-up";
  import type { AppStatus, FeedItem } from "$lib/api";
  import { activity as feedActivity, checkedToday, openItem, postedText } from "$lib/feed";
  import { applicationFit, jobFit, scoreFit, type Fit } from "$lib/fit";
  import { chooseResume } from "$lib/import";
  import { bindKeys } from "$lib/keys";
  import { paste } from "$lib/paste.svelte";
  import { app } from "$lib/stores/app.svelte";
  import { feed } from "$lib/stores/feed.svelte";
  import { STATUS } from "$lib/status";
  import {
    Badge,
    Button,
    Card,
    CompanyMark,
    Heatmap,
    Icon,
    Kbd,
    ScoreBar,
    Segmented,
    Toolbar,
    dayKey,
    tip,
    type TipData,
  } from "$lib/ui";

  type Range = "all" | 30 | 7;
  const RANGES: { value: Range; label: string }[] = [
    { value: "all", label: "All" },
    { value: 30, label: "30 days" },
    { value: 7, label: "7 days" },
  ];
  const WEEKS = 26;
  const NEXT = 5;
  const FITS = 8;
  const WEEKDAYS = ["Sundays", "Mondays", "Tuesdays", "Wednesdays", "Thursdays", "Fridays", "Saturdays"];

  let range = $state<Range>("all");

  onMount(() => {
    void Promise.all([app.refreshJobs(), app.refreshApplications(), app.refreshLibrary(), feed.refresh()]);
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

  const longDay = new Intl.DateTimeFormat(undefined, { weekday: "short", month: "short", day: "numeric" });
  /** A day's tooltip: both counts on every day, so it keeps its shape as you move along the grid. */
  function describe(date: Date): TipData {
    const key = dayKey(date);
    return {
      title: longDay.format(date),
      rows: [
        { label: "Postings added", value: added.get(key) ?? 0 },
        { label: "Applications sent", value: sent.get(key) ?? 0 },
      ],
    };
  }

  // ── Up next: interviews first, then postings not sent yet, best fit first ──
  interface Next {
    key: string;
    href: string;
    company: string;
    companyKey: string;
    role: string;
    status: AppStatus | null;
    fit: Fit | null;
    day: string;
  }
  const interviewing = $derived<Next[]>(
    app.applications
      .filter((a) => a.status === "interviewing")
      .map((a) => ({
        key: `application-${a.id}`,
        href: a.job ? `/jobs/${a.job}` : "/applications",
        company: a.company,
        companyKey: a.companyKey,
        role: a.role,
        status: a.status,
        fit: applicationFit(a),
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
        companyKey: j.companyKey,
        role: j.role,
        status: j.status,
        fit: jobFit(j, app.applications),
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

  // ── New for you: the feed's fits, best score first ──
  const fits = $derived(feed.fits.slice(0, FITS));
  const feedStatus = $derived(feed.status?.enabled ? feed.status : null);
  const feedMeta = $derived(
    feedStatus
      ? `${feed.fits.length} ${feed.fits.length === 1 ? "fit" : "fits"} · ${checkedToday(feedStatus)}`
      : "",
  );
  const feedNow = $derived(feedStatus && feedActivity(feedStatus, feed.items));
  /** The job being opened: its posting may need reading first. */
  let opening = $state<string | null>(null);

  async function openFit(item: FeedItem) {
    if (opening) return;
    opening = item.slug;
    await openItem(item);
    opening = null;
  }
</script>

{#snippet fitBar(fit: Fit | null)}
  {#if fit}
    <ScoreBar bar={false} now={fit.now} projected={fit.projected} onFile={fit.onFile} />
  {:else}
    <span class="meta num">—</span>
  {/if}
{/snippet}

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

    <div class="split">
      <section class="section" aria-labelledby="activity">
        <header class="head">
          <h2 class="heading" id="activity">Activity</h2>
          <Segmented options={RANGES} bind:value={range} label="Activity over" />
        </header>
        <Card pad>
          <div class="activity">
            <dl class="stats">
              <div class="stat">
                <dt class="meta">Postings</dt>
                <dd class="title num">{stats.postings}</dd>
              </div>
              <div class="stat">
                <dt class="meta">Applications</dt>
                <dd class="title num">{stats.applications}</dd>
              </div>
              <div class="stat">
                <dt class="meta">Interviews</dt>
                <dd class="title num">{stats.interviews}</dd>
              </div>
              <div class="stat">
                <dt class="meta">Active days</dt>
                <dd class="title num">{stats.days.length}</dd>
              </div>
              <div class="stat" {@attach tip("On file, else projected, averaged over the applications")}>
                <dt class="meta">Average fit</dt>
                <dd class="title num">{percent(stats.fit)}</dd>
              </div>
              <div class="stat" {@attach tip("Projected minus now, averaged over the applications")}>
                <dt class="meta">Points from tailoring</dt>
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
      </section>

      <section class="section" aria-labelledby="next">
        <header class="head">
          <h2 class="heading" id="next">Up next</h2>
          {#if nextMeta}<span class="meta num">{nextMeta}</span>{/if}
        </header>
        <Card>
          <div class="list">
            {#if noBank}
              <button type="button" class="row row--action" onclick={importResume}>
                <span class="lead lead--icon"><Icon icon={FileUp} size={16} weight="medium" /></span>
                <span class="company">Import your resume</span>
                <span class="role meta">Resumes and fit scores start from your experience.</span>
                <span class="status"><Kbd>I</Kbd></span>
              </button>
            {/if}
            {#each next as item (item.key)}
              <a class="row" href={item.href}>
                <span class="lead"><CompanyMark size={28} src={app.logos[item.companyKey]} /></span>
                <span class="company" {@attach tip(item.company)}>{item.company}</span>
                <span class="role meta" {@attach tip(item.role)}>{item.role}</span>
                <span class="status">
                  {#if item.status}<Badge tone={STATUS[item.status].tone}>{STATUS[item.status].label}</Badge>{/if}
                </span>
                <span class="score">{@render fitBar(item.fit)}</span>
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

    {#if feedStatus}
      <section class="section" aria-labelledby="new">
        <header class="head">
          <h2 class="heading" id="new">New for you</h2>
          <span class="meta" aria-live="polite">{feedNow ?? ""}</span>
          <span class="meta num">{feedMeta}</span>
          <Button size="sm" variant="ghost" onclick={() => goto("/feed")}>See all</Button>
        </header>
        <Card>
          <div class="list">
            {#each fits as item (item.slug)}
              <button type="button" class="row row--fit" disabled={opening !== null} onclick={() => openFit(item)}>
                <span class="lead"><CompanyMark size={28} src={app.logos[item.companyKey]} /></span>
                <span class="company" {@attach tip(item.company)}>{item.company}</span>
                <span class="role meta" {@attach tip(item.role)}>{item.role}</span>
                <span class="score">{@render fitBar(scoreFit(item.score))}</span>
                <span class="day meta num">{opening === item.slug ? "Opening…" : `${postedText(item)} · ${item.origin}`}</span>
                {#if item.knockouts.length}
                  <span class="knockouts meta" {@attach tip(item.knockouts.join(" · "))}>{item.knockouts.join(" · ")}</span>
                {/if}
              </button>
            {:else}
              <p class="none">{feedStatus.error ?? "No fits yet: new postings are checked as they come in, and the ones that fit land here."}</p>
            {/each}
          </div>
        </Card>
      </section>
    {/if}
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
    container-type: inline-size;
  }
  /* Activity and Up next side by side; stacked once Activity's three stats would truncate their labels. */
  .split {
    display: grid;
    grid-template-columns: minmax(0, 5fr) minmax(0, 4fr);
    align-items: start;
    gap: var(--space-6);
  }
  @container (width < 760px) {
    .split {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  .section {
    display: grid;
    gap: var(--space-2);
    min-width: 0;
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
    gap: var(--space-4);
    margin: 0;
  }
  .stat {
    display: grid;
    gap: var(--space-0-5);
    min-width: 0;
  }
  /* Hairlines between the rows only: the card's edge already bounds the first. */
  .stat:nth-child(n + 4) {
    padding-top: var(--space-3);
    box-shadow: inset 0 1px 0 var(--line);
  }
  dt {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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

  /* Inside the card's 1px border: 12 - 1, so a row's hover wash follows the corners. */
  .list {
    display: grid;
    overflow: hidden;
    border-radius: calc(var(--radius-lg) - 1px);
  }
  /* Two lines: the company over its role, the status badge centered on both, the fit score over the day. The mark's
     edges meet the company's cap height and the role's baseline (centered, then 1px down). Up next and New for you
     share it. */
  .row {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto auto;
    grid-template-areas:
      "lead company status score"
      "lead role status day";
    align-items: center;
    column-gap: var(--space-2);
    padding: var(--space-2) var(--space-5);
    color: var(--ink);
    text-decoration: none;
    cursor: default;
    -webkit-user-drag: none;
    transition: background-color var(--dur-instant) var(--ease-out);
  }
  .row + .row {
    box-shadow: inset 0 1px 0 var(--line);
  }
  button.row {
    width: 100%;
    border: 0;
    background: none;
    font: inherit;
    text-align: start;
  }
  .row--action {
    grid-template-columns: auto minmax(0, 1fr) auto;
    grid-template-areas:
      "lead company status"
      "lead role status";
  }
  /* A fit: no status (it isn't yours yet); its knockouts, when it has any, a quiet third line under the role. */
  .row--fit {
    grid-template-columns: auto minmax(0, 1fr) auto;
    grid-template-areas:
      "lead company score"
      "lead role day"
      ". knockouts knockouts";
  }
  .lead {
    grid-area: lead;
    display: flex;
    translate: 0 1px;
  }
  /* The icon in a mark-sized box, so the action's text lines up with the companies under it. */
  .lead--icon {
    width: 28px;
    height: 28px;
    place-content: center;
    align-items: center;
    color: var(--accent-text);
  }
  .company {
    grid-area: company;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: var(--weight-medium);
  }
  .role {
    grid-area: role;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink-2);
  }
  .row--action .role {
    white-space: normal;
    text-wrap: pretty;
  }
  .status {
    grid-area: status;
    display: flex;
    margin-inline-start: var(--space-1);
  }
  /* The fit as Applications draws it, in small: the bar's number keeps the column's end. */
  .score {
    grid-area: score;
    justify-self: end;
  }
  .day {
    grid-area: day;
    text-align: end;
  }
  .knockouts {
    grid-area: knockouts;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink-3);
  }
  .none {
    margin: 0;
    padding: var(--space-4) var(--space-5);
    color: var(--ink-3);
    text-wrap: pretty;
  }
  @media (hover: hover) {
    .row:not(:disabled):hover {
      background: var(--hover);
    }
  }
</style>
