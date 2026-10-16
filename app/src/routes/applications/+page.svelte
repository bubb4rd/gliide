<!--
  Applications: every posting, newest first. A posting you sent a resume for (exported here, or imported from ATS
  Fit's applications.csv) shows its scores, file, posting link and status; the status menu saves at once and rewrites
  applications.csv. A posting not sent yet shows "Not sent", its fit score and the day it was added; its trash (shown on
  hover or focus) deletes it, with Undo in the toast. V (or ⌘V) reads the clipboard: a link or the posting's text
  becomes a posting, and its review opens.

  Above the table: a filter on company or role (/ or ⌘F focuses it, Esc clears it) and a status filter. A column
  header sorts by its column; a second click reverses it. Filters and sort stay as you left them when you come back
  from a job.
-->
<script lang="ts" module>
  import type { AppStatus } from "$lib/api";
  import type { TableSort } from "$lib/ui";

  type Show = "all" | "unsent" | AppStatus;

  const view = $state<{ query: string; show: Show; sort: TableSort }>({
    query: "",
    show: "all",
    sort: { key: "date", dir: "desc" },
  });
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import { openPath, openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
  import ClipboardPaste from "@lucide/svelte/icons/clipboard-paste";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import FileSpreadsheet from "@lucide/svelte/icons/file-spreadsheet";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import { appError, type ApplicationRow, type JobRow, type SalaryRange } from "$lib/api";
  import { applicationFit, scoreFit, type Fit } from "$lib/fit";
  import { bindKeys } from "$lib/keys";
  import { paste } from "$lib/paste.svelte";
  import { app } from "$lib/stores/app.svelte";
  import { perYear, salaryText } from "$lib/salary";
  import { STATUS, STATUS_OPTIONS, STATUS_ORDER } from "$lib/status";
  import {
    Badge,
    Button,
    CompanyMark,
    Empty,
    Input,
    Menu,
    ScoreBar,
    Segmented,
    Table,
    Toolbar,
    dayKey,
    tip,
    toast,
    type TableColumn,
  } from "$lib/ui";

  const SHOW: { value: Show; label: string }[] = [
    { value: "all", label: "All" },
    { value: "unsent", label: "Not sent" },
    ...STATUS_OPTIONS,
  ];

  const COLUMNS: TableColumn<Row>[] = [
    { key: "date", label: "Date", width: "88px", sortBy: (r) => r.date, sortFirst: "desc" },
    { key: "job", label: "Job", sortBy: (r) => `${r.company} ${r.role}` },
    // By the top of the range a year (hourly at 2,080 hours).
    {
      key: "salary",
      label: "Salary",
      numeric: true,
      width: "128px",
      sortBy: (r) => (r.salary ? perYear(r.salary) : null),
    },
    // The score that counts: on file once exported, else projected (a posting not sent yet: its fit score).
    {
      key: "fit",
      label: "Fit",
      numeric: true,
      width: "176px",
      sortBy: (r) => (r.fit ? (r.fit.onFile ?? r.fit.projected) : null),
    },
    // Along the pipeline: not sent, then each status in order.
    {
      key: "status",
      label: "Status",
      width: "148px",
      sortBy: (r) => (r.application ? STATUS_ORDER.indexOf(r.application.status) + 1 : 0),
    },
    {
      key: "resume",
      label: "Resume",
      width: "16%",
      sortBy: (r) => (r.application?.resume ? fileName(r.application.resume) : null),
    },
    { key: "link", label: "Link", align: "end", width: "68px" },
    { key: "delete", label: "", align: "end", width: "64px" },
  ];

  /** One row: an application, or a posting without one yet (`posting`). */
  interface Row {
    key: string;
    /** `YYYY-MM-DD`: the day it was sent, or added when it wasn't. */
    date: string;
    company: string;
    companyKey: string;
    role: string;
    /** The job's review; none for an application imported without its posting. */
    href: string | null;
    url: string | null;
    application: ApplicationRow | null;
    posting: JobRow | null;
    /** The posting's salary range (an application's through its posting). */
    salary: SalaryRange | null;
    /** The scores the Fit bar draws; none before a posting's requirements are read. */
    fit: Fit | null;
  }

  let loaded = $state(false);
  let filterInput = $state<ReturnType<typeof Input>>();

  onMount(async () => {
    await Promise.all([app.refreshApplications(), app.refreshJobs(), app.settings ? null : app.loadSettings()]);
    loaded = true;
  });

  function focusFilter() {
    filterInput?.focus();
    filterInput?.select();
  }

  function clearFilters() {
    view.query = "";
    view.show = "all";
  }

  function onFilterKey(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    e.preventDefault();
    if (view.query) view.query = "";
    else (e.currentTarget as HTMLInputElement).blur();
  }

  $effect(() =>
    bindKeys({
      v: paste.run,
      // In the filter, ⌘V pastes into it.
      "mod+v": (e) => (e.target instanceof HTMLInputElement ? false : paste.run()),
      "/": focusFilter,
      "mod+f": focusFilter,
      Escape: () => (filtered ? clearFilters() : false),
    }),
  );

  const jobsById = $derived(new Map(app.jobs.map((j) => [j.id, j])));
  const rows = $derived<Row[]>(
    [
      ...app.applications.map(
        (a): Row => ({
          key: `application-${a.id}`,
          date: a.date,
          company: a.company,
          companyKey: a.companyKey,
          role: a.role,
          href: a.job ? `/jobs/${a.job}` : null,
          url: a.url,
          application: a,
          posting: null,
          salary: (a.job && jobsById.get(a.job)?.salary) || null,
          fit: applicationFit(a),
        }),
      ),
      ...app.unsent.map(
        (j): Row => ({
          key: `posting-${j.id}`,
          date: dayKey(new Date(j.addedAt)),
          company: j.company,
          companyKey: j.companyKey,
          role: j.role,
          href: `/jobs/${j.id}`,
          url: j.url,
          application: null,
          posting: j,
          salary: j.salary,
          fit: scoreFit(j.score),
        }),
      ),
    ].sort((a, b) => b.date.localeCompare(a.date)),
  );
  const terms = $derived(view.query.toLocaleLowerCase().split(/\s+/).filter(Boolean));
  const filtered = $derived(terms.length > 0 || view.show !== "all");
  const shown = $derived(
    rows.filter((r) => {
      if (view.show === "unsent" ? r.application : view.show !== "all" && r.application?.status !== view.show)
        return false;
      const text = `${r.company} ${r.role}`.toLocaleLowerCase();
      return terms.every((t) => text.includes(t));
    }),
  );
  const showing = $derived(
    filtered ? `${shown.length} of ${rows.length}` : `${rows.length} ${rows.length === 1 ? "posting" : "postings"}`,
  );
  const noMatch = $derived.by(() => {
    const query = view.query.trim();
    const where = view.show === "all" ? "" : ` in ${SHOW.find((s) => s.value === view.show)?.label}`;
    return query ? `Nothing${where} matches “${query}”.` : `Nothing${where} yet.`;
  });
  const csvPath = $derived(app.settings ? `${app.settings.exportDir.replace(/\/+$/, "")}/applications.csv` : null);

  const THIS_YEAR = new Date().getFullYear();
  /** `2026-10-12` → `Oct 12` (with the year when it isn't this one). */
  function shortDate(iso: string): string {
    const [y, m, d] = iso.split("-").map(Number);
    if (!y || !m || !d) return iso;
    return new Date(y, m - 1, d).toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      year: y === THIS_YEAR ? undefined : "numeric",
    });
  }

  const fileName = (path: string) => path.split("/").pop() || path;

  async function run(what: string, call: () => Promise<void>) {
    try {
      await call();
    } catch (e) {
      toast(`Couldn’t ${what}: ${appError(e).message}`, { tone: "danger" });
    }
  }
</script>

<Toolbar>
  {#snippet crumbs()}<b>Applications</b>{/snippet}
  {#if csvPath}
    <Button icon={FileSpreadsheet} {@attach tip(csvPath)} onclick={() => run("open applications.csv", () => openPath(csvPath))}>
      Open applications.csv
    </Button>
  {/if}
  {#if rows.length}
    <Button variant="primary" icon={ClipboardPaste} kbd="V" disabled={paste.reading} onclick={paste.run}>
      {paste.reading ? "Reading…" : "Paste from clipboard"}
    </Button>
  {/if}
</Toolbar>

{#if rows.length}
  <div class="filters">
    <div class="query">
      <Input
        bind:this={filterInput}
        bind:value={view.query}
        size="md"
        placeholder="Filter by company or role"
        aria-label="Filter by company or role"
        aria-keyshortcuts="/"
        onkeydown={onFilterKey}
      />
    </div>
    <Segmented options={SHOW} bind:value={view.show} label="Show" />
    <span class="meta num count" aria-live="polite">{showing}</span>
  </div>
  {#if shown.length}
    <div class="table">
      <Table columns={COLUMNS} rows={shown} bind:sort={view.sort} key={(r) => r.key} label="Applications">
        {#snippet cell(row, column)}
          {#if column.key === "date"}
            <span class="num" {@attach tip(row.date)}>{shortDate(row.date)}</span>
          {:else if column.key === "job"}
            <svelte:element
              this={row.href ? "a" : "span"}
              class={["job", row.href && "job--link"]}
              href={row.href}
              {@attach tip(`${row.company} · ${row.role}`)}
            >
              <CompanyMark size={28} src={app.logos[row.companyKey]} />
              <span class="company">{row.company}</span>
              <span class="role meta">{row.role}</span>
            </svelte:element>
          {:else if column.key === "salary"}
            {#if row.salary}
              <span {@attach tip(salaryText(row.salary))}>{salaryText(row.salary, true)}</span>
            {:else}
              —
            {/if}
          {:else if column.key === "fit"}
            {#if row.fit}
              <ScoreBar now={row.fit.now} projected={row.fit.projected} onFile={row.fit.onFile} />
            {:else}
              —
            {/if}
          {:else if column.key === "status"}
            {#if row.application}
              {@const application = row.application}
              <Menu
                value={application.status}
                options={STATUS_OPTIONS}
                label="Status for {row.company}"
                variant="ghost"
                size="sm"
                onchange={(s) => app.setStatus(application, s)}
              >
                {#snippet trigger(current)}<Badge tone={STATUS[application.status].tone}>{current?.label}</Badge>{/snippet}
              </Menu>
            {:else}
              <span class="unsent"><Badge>Not sent</Badge></span>
            {/if}
          {:else if column.key === "resume"}
            {#if row.application?.resume}
              {@const path = row.application.resume}
              <button
                type="button"
                class="file"
                {@attach tip(`Show ${path} in Finder`)}
                onclick={() => run(`show ${fileName(path)} in Finder`, () => revealItemInDir(path))}
              >
                {fileName(path)}
              </button>
            {:else}
              <span class="none">—</span>
            {/if}
          {:else if column.key === "link"}
            {#if row.url}
              {@const url = row.url}
              <Button
                variant="ghost"
                size="sm"
                square
                icon={ExternalLink}
                aria-label="Open the {row.company} posting"
                {@attach tip(url)}
                onclick={() => run("open the posting", () => openUrl(url))}
              />
            {:else}
              <span class="none">—</span>
            {/if}
          {:else if row.posting}
            {@const posting = row.posting}
            <span class="delete">
              <Button
                variant="ghost"
                size="sm"
                square
                icon={Trash2}
                aria-label="Delete the {row.company || row.role} posting"
                {@attach tip("Delete posting")}
                onclick={() => app.deleteJob(posting)}
              />
            </span>
          {/if}
        {/snippet}
      </Table>
    </div>
  {:else}
    <div class="center surface">
      <Empty title="No matches" body={noMatch}>
        {#snippet action()}
          <Button size="lg" kbd="Esc" onclick={clearFilters}>Clear filters</Button>
        {/snippet}
      </Empty>
    </div>
  {/if}
{:else if loaded && app.jobsLoaded}
  <div class="center">
    <Empty title="No postings yet" body="Paste a job link or the posting’s text, and the score shows up in a few seconds.">
      {#snippet action()}
        <Button variant="primary" size="lg" icon={ClipboardPaste} kbd="V" disabled={paste.reading} onclick={paste.run}>
          {paste.reading ? "Reading…" : "Paste from clipboard"}
        </Button>
      {/snippet}
    </Empty>
  </div>
{/if}

<style>
  /* One surface with the table under it; the 24px inset lines the field up with the first column. */
  .filters {
    display: flex;
    flex: none;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2) var(--space-3);
    padding: var(--space-3) var(--space-6) var(--space-1);
    background: var(--surface);
  }
  .query {
    flex: 0 1 280px;
    min-width: 180px;
  }
  .count {
    margin-inline-start: auto;
    white-space: nowrap;
  }
  .table {
    display: grid;
    grid-template-rows: minmax(0, 1fr);
    flex: 1;
    min-height: 0;
    background: var(--surface);
  }
  .surface {
    background: var(--surface);
  }
  /* As the sidebar's Recent: the 28px mark spans both lines, its edges on the company's cap height and the role's
     baseline (centered, then 1px down); each line truncates. */
  .job {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    column-gap: var(--space-2);
    align-items: center;
    color: var(--ink);
    text-decoration: none;
    cursor: default;
    -webkit-user-drag: none;
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
  /* Lines up with the status menus' badges (their ghost button's padding). */
  .unsent {
    display: inline-flex;
    padding-inline: var(--space-2);
  }
  .none {
    color: var(--ink-4);
  }
  .file {
    display: block;
    max-width: 100%;
    padding: 0;
    border: 0;
    border-radius: var(--radius-xs);
    background: none;
    color: var(--accent-text);
    text-align: start;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: default;
  }
  .center {
    display: grid;
    flex: 1;
    place-items: center;
    min-height: 0;
  }
  @media (hover: hover) {
    .job--link:hover .company {
      color: var(--accent-text);
    }
    .file:hover {
      text-decoration: underline;
      text-underline-offset: 2px;
    }
    /* The trash shows on the row's hover or focus; touch screens always show it. */
    .delete {
      opacity: 0;
      transition: opacity var(--dur-instant) var(--ease-out);
    }
    :global(tr:hover) .delete,
    :global(tr:focus-within) .delete {
      opacity: 1;
    }
  }
</style>
