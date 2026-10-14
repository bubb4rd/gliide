<!--
  Feed: new postings from freehire, LinkedIn, your watchlist's careers boards and Fantastic.jobs, pulled several
  times a day and checked (Extract + score) up to the daily cap. Each row says which source found it. Fits are at or
  over your threshold, best first; Checked adds the rest; Waiting holds what's pulled and not checked yet, in the
  order they'll be checked; Search asks freehire (and LinkedIn, when the feed pulls it) with the feed's filters plus
  your words, no model call, under a strip of what it searches, and Check checks a result now. A row opens its job
  (it then joins Recent and Applications).

  As Applications: a filter on company or role over the tab's rows (/ or ⌘F focuses it, Esc clears it; on Search, /
  focuses the search), and a header sorts by its column, a second click reverses it. ↑↓ select along the order shown,
  ⏎ opens, O opens the employer's page (a LinkedIn job's posting on LinkedIn: its guest page hides the employer's
  link), D dismisses, C checks, P pulls now. The tab, filter, each tab's sort and the last search stay as you left
  them.
-->
<script lang="ts" module>
  import type { FeedItem, SearchCursor } from "$lib/api";
  import type { TableSort } from "$lib/ui";

  type Show = "fits" | "checked" | "waiting" | "search";

  const view = $state<{
    show: Show;
    /** The filter over the listed tabs. */
    filter: string;
    /** Each tab's sort; none keeps the feed's order (Waiting: the check queue's; Search: freehire's, then LinkedIn's). */
    sorts: Record<Show, TableSort | undefined>;
    query: string;
    /**
     * The last search run: its words, its results so far, how many there are (freehire's matches and LinkedIn's found
     * so far), where the next page starts (null: no more) and what couldn't be searched ("LinkedIn: …").
     */
    searched: string | null;
    results: FeedItem[];
    total: number;
    next: SearchCursor | null;
    error: string | null;
  }>({
    show: "fits",
    filter: "",
    sorts: {
      fits: { key: "score", dir: "desc" },
      checked: { key: "score", dir: "desc" },
      waiting: undefined,
      search: undefined,
    },
    query: "",
    searched: null,
    results: [],
    total: 0,
    next: null,
    error: null,
  });
</script>

<script lang="ts">
  import { goto } from "$app/navigation";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import X from "@lucide/svelte/icons/x";
  import { appError, checkFeed, dismissFeed, pullFeed, searchFeed, type FeedItemState } from "$lib/api";
  import { scoreFit } from "$lib/fit";
  import { activity, checkedToday, openItem, postedText, SEARCH_SOURCES_NOTE, searchSources } from "$lib/feed";
  import { bindKeys } from "$lib/keys";
  import { app } from "$lib/stores/app.svelte";
  import { feed } from "$lib/stores/feed.svelte";
  import {
    Badge,
    Button,
    CompanyMark,
    Empty,
    Icon,
    Input,
    KeyFooter,
    ScoreBar,
    Segmented,
    Sources,
    Table,
    Toolbar,
    sortRows,
    tip,
    toast,
    type KeyCommand,
    type TableColumn,
  } from "$lib/ui";

  const SHOW: { value: Show; label: string }[] = [
    { value: "fits", label: "Fits" },
    { value: "checked", label: "Checked" },
    { value: "waiting", label: "Waiting" },
    { value: "search", label: "Search" },
  ];
  /** Which rows each tab lists, by state. */
  const STATES: Record<Exclude<Show, "search">, FeedItemState[]> = {
    fits: ["fit"],
    checked: ["fit", "not-fit"],
    waiting: ["checking", "waiting", "new"],
  };
  /** States Check runs on: not checked yet. */
  const CHECKABLE: FeedItemState[] = ["new", "waiting"];

  /** The feed's order of states, for the State column's sort. */
  const STATE_ORDER: FeedItemState[] = ["fit", "not-fit", "checking", "waiting", "new", "skipped", "dismissed"];

  const COLUMNS: TableColumn<FeedItem>[] = [
    { key: "job", label: "Job", sortBy: (i) => `${i.company} ${i.role}` },
    { key: "score", label: "Fit", numeric: true, width: "176px", sortBy: (i) => i.score },
    // The employer's date, else the day its source found it; newest first. The source under it.
    { key: "posted", label: "Posted", width: "128px", sortBy: (i) => i.postedAt ?? i.foundAt, sortFirst: "desc" },
    { key: "master", label: "Master", width: "16%", sortBy: (i) => i.master },
    { key: "state", label: "State", width: "16%", sortBy: (i) => STATE_ORDER.indexOf(i.state) },
    // Check, the employer's page and Dismiss, inside the 24px edge.
    { key: "actions", label: "", align: "end", width: "168px" },
  ];

  let selectedSlug = $state<string | null>(null);
  /** The job being opened, and the jobs whose Check is in flight (until the feed says they're checking). */
  let opening = $state<string | null>(null);
  let checking = $state<Set<string>>(new Set());
  let pulling = $state(false);
  let searching = $state(false);
  let searchInput = $state<ReturnType<typeof Input>>();
  let filterInput = $state<ReturnType<typeof Input>>();

  const status = $derived(feed.status);
  const off = $derived(status !== null && !status.enabled);
  const bySlug = $derived(new Map(feed.items.map((i) => [i.slug, i])));
  /** Search results as the feed has them now (a result you checked shows its score). */
  const results = $derived(view.results.map((r) => bySlug.get(r.slug) ?? r));
  const listed = $derived(
    view.show === "search" ? results : feed.items.filter((i) => STATES[view.show as Exclude<Show, "search">].includes(i.state)),
  );
  const terms = $derived(view.show === "search" ? [] : view.filter.toLocaleLowerCase().split(/\s+/).filter(Boolean));
  const rows = $derived(
    terms.length
      ? listed.filter((i) => terms.every((t) => `${i.company} ${i.role}`.toLocaleLowerCase().includes(t)))
      : listed,
  );
  /** The rows as the table shows them: ↑↓ walk this order. */
  const ordered = $derived(sortRows(rows, COLUMNS, view.sorts[view.show]));
  const selectedIndex = $derived(Math.max(0, ordered.findIndex((r) => r.slug === selectedSlug)));
  const selected = $derived<FeedItem | undefined>(ordered[selectedIndex]);
  const now = $derived(status && activity(status, feed.items));
  const busyPulling = $derived(pulling || !!status?.pulling);

  const clock = new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit" });
  const time = (iso: string | null) => {
    const d = iso ? new Date(iso) : null;
    return d && !Number.isNaN(d.getTime()) ? clock.format(d) : null;
  };
  /** "Pulled 2:14 PM · next 5:14 PM · 12 of 20 checked today", or what it's doing now. */
  const statusLine = $derived.by(() => {
    if (!status) return "";
    if (now) return now;
    const last = time(status.lastPull);
    const next = time(status.nextPull);
    return [last ? `Pulled ${last}` : "Not pulled yet", next && `next ${next}`, checkedToday(status)]
      .filter(Boolean)
      .join(" · ");
  });
  const count = $derived.by(() => {
    if (view.show !== "search") {
      const all = `${listed.length} ${listed.length === 1 ? "job" : "jobs"}`;
      return terms.length ? `${rows.length} of ${listed.length}` : all;
    }
    if (view.searched === null) return "";
    return `${results.length} of ${view.total.toLocaleString()}`;
  });

  const isChecking = (item: FeedItem) => item.state === "checking" || checking.has(item.slug);
  const canCheck = (item: FeedItem | undefined) => !!item && CHECKABLE.includes(item.state) && !isChecking(item);
  async function run(what: string, call: () => Promise<void>) {
    try {
      await call();
    } catch (e) {
      toast(`Couldn’t ${what}: ${appError(e).message}`, { tone: "danger" });
    }
  }

  async function pull() {
    if (busyPulling) return;
    pulling = true;
    await run("pull new postings", async () => feed.set(await pullFeed()));
    pulling = false;
  }

  async function open(item: FeedItem | undefined) {
    if (!item || opening) return;
    opening = item.slug;
    await openItem(item);
    opening = null;
  }

  /** A LinkedIn job's link is its posting on LinkedIn: LinkedIn's guest page hides the employer's. */
  const onLinkedIn = (item: FeedItem | undefined) => item?.origin === "LinkedIn";

  function employer(item: FeedItem | undefined) {
    if (item) {
      void run(onLinkedIn(item) ? "open the posting on LinkedIn" : "open the employer’s page", () => openUrl(item.url));
    }
  }

  async function check(item: FeedItem | undefined) {
    if (!item || !canCheck(item)) return;
    checking = new Set(checking).add(item.slug);
    await run(`check the ${item.company || item.role} posting`, async () => feed.set(await checkFeed([item.slug])));
    const rest = new Set(checking);
    rest.delete(item.slug);
    checking = rest;
  }

  async function dismiss(item: FeedItem | undefined) {
    if (!item) return;
    const index = selectedIndex;
    await run(`dismiss the ${item.company || item.role} posting`, async () => {
      feed.set(await dismissFeed(item.slug));
      // Out of the search too: the feed won't offer it again.
      view.results = view.results.filter((r) => r.slug !== item.slug);
      // The selection stays in place: the next row moves up into it.
      selectedSlug = ordered[Math.min(index, ordered.length - 1)]?.slug ?? null;
    });
  }

  async function search(more = false) {
    const query = more ? (view.searched ?? "") : view.query.trim();
    if (!query || searching || (more && !view.next)) return;
    searching = true;
    await run("search jobs", async () => {
      const page = await searchFeed(query, more ? view.next : null);
      view.results = more ? [...view.results, ...page.items] : page.items;
      view.total = page.total;
      view.next = page.next;
      view.error = page.error;
      view.searched = query;
    });
    searching = false;
  }

  /** `/` and ⌘F: the tab's field (the filter, or Search's search). */
  function focusField() {
    const input = view.show === "search" ? searchInput : filterInput;
    input?.focus();
    input?.select();
  }

  function onFilterKey(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    e.preventDefault();
    if (view.filter) view.filter = "";
    else (e.currentTarget as HTMLInputElement).blur();
  }

  function onSearchKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      void search();
    } else if (e.key === "Escape") {
      e.preventDefault();
      if (view.query) view.query = "";
      else (e.currentTarget as HTMLInputElement).blur();
    }
  }

  function move(by: number) {
    if (!ordered.length) return false;
    selectedSlug = ordered[Math.min(ordered.length - 1, Math.max(0, selectedIndex + by))].slug;
  }

  const KEYS: KeyCommand[] = $derived([
    { keys: ["↑", "↓"], label: "Select a job" },
    { keys: ["⏎"], label: "Open", run: () => open(selected), disabled: !selected || opening !== null },
    {
      keys: ["O"],
      label: onLinkedIn(selected) ? "Open the posting on LinkedIn" : "Open the employer’s page",
      run: () => employer(selected),
      disabled: !selected,
    },
    { keys: ["C"], label: "Check now", run: () => check(selected), disabled: !canCheck(selected) },
    { keys: ["D"], label: "Dismiss", run: () => dismiss(selected), disabled: !selected },
    { keys: ["P"], label: "Pull now", run: pull, disabled: busyPulling || off },
    { keys: ["/"], label: view.show === "search" ? "Search jobs" : "Filter", run: focusField, disabled: off },
  ]);

  $effect(() =>
    bindKeys({
      ArrowDown: () => move(1),
      ArrowUp: () => move(-1),
      // A focused button or link takes ⏎ itself.
      Enter: (e) => (e.target instanceof Element && e.target.closest("button, a") ? false : void open(selected)),
      o: () => employer(selected),
      c: () => void check(selected),
      d: () => void dismiss(selected),
      p: () => void pull(),
      "/": focusField,
      "mod+f": focusField,
      Escape: () => (view.show !== "search" && view.filter ? void (view.filter = "") : false),
    }),
  );

  const STATE_LABEL: Record<FeedItemState, string> = {
    new: "New",
    skipped: "Skipped",
    waiting: "Waiting",
    checking: "Checking…",
    fit: "Fit",
    "not-fit": "Not a fit",
    dismissed: "Dismissed",
  };
</script>

<Toolbar>
  {#snippet crumbs()}<b>Feed</b>{/snippet}
  <Button variant="primary" icon={RefreshCw} kbd="P" disabled={busyPulling || off} onclick={pull}>
    {busyPulling ? "Pulling…" : "Pull now"}
  </Button>
</Toolbar>

{#if off}
  <div class="center">
    <Empty
      title="The feed is off"
      body="Turn it on in Settings to pull new postings from freehire and your other sources several times a day."
    >
      {#snippet action()}
        <Button variant="primary" size="lg" kbd="⌘," onclick={() => goto("/settings")}>Open Settings</Button>
      {/snippet}
    </Empty>
  </div>
{:else}
  <div class="filters">
    <Segmented options={SHOW} bind:value={view.show} label="Show" />
    {#if view.show === "search"}
      <div class="query">
        <Input
          bind:this={searchInput}
          bind:value={view.query}
          size="md"
          placeholder="Search jobs: a title, skill or company"
          aria-label="Search jobs"
          aria-keyshortcuts="/"
          onkeydown={onSearchKey}
        />
      </div>
      <Button size="md" disabled={!view.query.trim() || searching} onclick={() => search()}>
        {searching && !view.results.length ? "Searching…" : "Search"}
      </Button>
    {:else}
      <div class="query">
        <Input
          bind:this={filterInput}
          bind:value={view.filter}
          size="md"
          placeholder="Filter by company or role"
          aria-label="Filter by company or role"
          aria-keyshortcuts="/"
          onkeydown={onFilterKey}
        />
      </div>
    {/if}
    <span class="meta num count" aria-live="polite">{count}</span>
  </div>
  {#if view.show === "search"}
    <!-- What Search asks, and, under the count, what it couldn't this time (results from the rest still list). -->
    <div class="status status--search">
      <Sources label="Searches" sources={searchSources(!!status?.linkedin)} note={SEARCH_SOURCES_NOTE} />
      <p class="search-error" aria-live="polite">
        {#if view.error}<Icon icon={TriangleAlert} size={14} /><span>{view.error}</span>{/if}
      </p>
    </div>
  {:else}
    <p class={["meta", "status", status?.error && "status--error"]} aria-live="polite">
      {status?.error ?? [statusLine, status?.paused].filter(Boolean).join(" · ")}
    </p>
  {/if}

  {#if rows.length}
    <div class="table">
      <Table
        columns={COLUMNS}
        {rows}
        bind:sort={view.sorts[view.show]}
        key={(r) => r.slug}
        label="Feed"
        selected={selected?.slug ?? null}
        onselect={(r) => (selectedSlug = r.slug)}
      >
        {#snippet cell(item, column)}
          {#if column.key === "job"}
            <button
              type="button"
              class="job"
              disabled={opening !== null}
              {@attach tip(`${item.company} · ${item.role}${item.location ? ` · ${item.location}` : ""}`)}
              onclick={() => open(item)}
            >
              <CompanyMark size={28} src={app.logos[item.companyKey]} />
              <span class="company">{item.company}</span>
              <span class="role meta">{opening === item.slug ? "Opening…" : item.role}</span>
            </button>
          {:else if column.key === "score"}
            {@const fit = scoreFit(item.score)}
            {#if fit}<ScoreBar projected={fit.projected} />{:else}<span class="none">—</span>{/if}
          {:else if column.key === "posted"}
            <span class="posted">
              <span>{postedText(item)}</span>
              <span class="meta">{item.origin}</span>
            </span>
          {:else if column.key === "master"}
            {#if item.master}<span {@attach tip(item.master)}>{item.master}</span>{:else}<span class="none">—</span>{/if}
          {:else if column.key === "state"}
            {@const note = item.knockouts.length ? item.knockouts.join(" · ") : item.reason}
            <span class="state" {@attach tip(note)}>
              {#if isChecking(item)}
                <span class="checking">Checking…</span>
              {:else}
                <Badge tone={item.state === "fit" ? "accent" : "neutral"}>{STATE_LABEL[item.state]}</Badge>
              {/if}
              {#if note}<span class="meta note">{note}</span>{/if}
            </span>
          {:else}
            <span class="actions">
              {#if canCheck(item)}
                <Button size="sm" onclick={() => check(item)}>Check</Button>
              {/if}
              <Button
                variant="ghost"
                size="sm"
                square
                icon={ExternalLink}
                aria-label={onLinkedIn(item)
                  ? `Open the ${item.company} posting on LinkedIn`
                  : `Open the ${item.company} employer page`}
                {@attach tip(onLinkedIn(item) ? "Posting on LinkedIn" : "Employer’s page")}
                onclick={() => employer(item)}
              />
              <Button
                variant="ghost"
                size="sm"
                square
                icon={X}
                aria-label="Dismiss the {item.company} posting"
                {@attach tip("Dismiss")}
                onclick={() => dismiss(item)}
              />
            </span>
          {/if}
        {/snippet}
      </Table>
      {#if view.show === "search" && view.next}
        <div class="more">
          <Button disabled={searching} onclick={() => search(true)}>{searching ? "Searching…" : "Show more"}</Button>
        </div>
      {/if}
    </div>
  {:else if view.show === "search"}
    <div class="center surface">
      {#if view.searched === null}
        <Empty title="Search jobs" body="Type a title, skill or company and press ⏎; results use your feed’s filters." />
      {:else}
        <Empty title="No results" body="Nothing matches “{view.searched}” with your feed’s filters." />
      {/if}
    </div>
  {:else if terms.length}
    <div class="center surface">
      <Empty title="No matches" body="Nothing here matches “{view.filter.trim()}”.">
        {#snippet action()}
          <Button size="lg" kbd="Esc" onclick={() => (view.filter = "")}>Clear filter</Button>
        {/snippet}
      </Empty>
    </div>
  {:else if feed.view}
    <div class="center surface">
      {#if view.show === "fits"}
        <Empty
          title="No fits yet"
          body="New postings are checked as they come in; the ones at or over {status?.threshold ?? 40}% land here."
        />
      {:else if view.show === "checked"}
        <Empty title="Nothing checked yet" body="Postings are checked after each pull, up to your daily checks." />
      {:else}
        <Empty title="Nothing waiting" body="Every posting pulled so far has been checked." />
      {/if}
    </div>
  {:else}
    <div class="center surface"></div>
  {/if}
  <KeyFooter keys={KEYS} />
{/if}

<style>
  /* One surface with the table under it; the 24px inset lines the tabs up with the first column. */
  .filters {
    display: flex;
    flex: none;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2) var(--space-3);
    padding: var(--space-3) var(--space-6) 0;
    background: var(--surface);
  }
  .query {
    flex: 0 1 320px;
    min-width: 180px;
  }
  .count {
    margin-inline-start: auto;
    white-space: nowrap;
  }
  .status {
    flex: none;
    margin: 0;
    padding: var(--space-1-5) var(--space-6) var(--space-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    background: var(--surface);
  }
  /* Several sources can fail at once, a sentence each: the error wraps rather than cut off. */
  .status--error {
    white-space: normal;
    overflow-wrap: anywhere;
    text-wrap: pretty;
    color: var(--danger);
  }
  /* On Search: the sources at the leading edge, a page's error at the trailing edge under the count. */
  .status--search {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--space-1) var(--space-4);
    white-space: normal;
  }
  /* As a change row's reason: amber is what was left out, and why. */
  .search-error {
    display: inline-flex;
    align-items: flex-start;
    gap: var(--space-1);
    min-width: 0;
    margin: 0;
    margin-inline-start: auto;
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    color: var(--warn);
    text-wrap: pretty;
  }
  .search-error:empty {
    display: none;
  }
  .search-error :global(.icon) {
    margin-top: 1px;
  }
  /* The posting's age over the source that found it, as the Job cell's two lines. */
  .posted {
    display: grid;
    min-width: 0;
  }
  .posted > * {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .table {
    display: grid;
    grid-template-rows: minmax(0, 1fr) auto;
    flex: 1;
    min-height: 0;
    background: var(--surface);
  }
  .surface {
    background: var(--surface);
  }
  .center {
    display: grid;
    flex: 1;
    place-items: center;
    min-height: 0;
  }
  .more {
    display: flex;
    justify-content: center;
    padding: var(--space-3);
    box-shadow: inset 0 1px 0 var(--line);
  }
  /* As Applications' Job cell: the 28px mark spans both lines, its edges on the company's cap height and the role's
     baseline (centered, then 1px down); each line truncates. A button: it opens the job. */
  .job {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    column-gap: var(--space-2);
    align-items: center;
    width: 100%;
    padding: 0;
    border: 0;
    border-radius: var(--radius-xs);
    background: none;
    color: var(--ink);
    font: inherit;
    text-align: start;
    cursor: default;
  }
  .job > :global(:first-child) {
    grid-row: span 2;
    translate: 0 1px;
  }
  .company,
  .role {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .company {
    font-weight: var(--weight-medium);
  }
  .role {
    color: var(--ink-2);
  }
  .none {
    color: var(--ink-4);
  }
  .state {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }
  .checking {
    color: var(--accent-text);
  }
  .note {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .actions {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
  }
  @media (hover: hover) {
    .job:not(:disabled):hover .company {
      color: var(--accent-text);
    }
    /* Row actions show on the row's hover, focus or selection; touch screens always show them. */
    .actions {
      opacity: 0;
      transition: opacity var(--dur-instant) var(--ease-out);
    }
    :global(tr:hover) .actions,
    :global(tr:focus-within) .actions,
    :global(tr[aria-current="true"]) .actions {
      opacity: 1;
    }
  }
</style>
