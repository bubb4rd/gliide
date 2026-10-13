<!--
  Applications: every posting, newest first. A posting you sent a resume for (exported here, or imported from ATS
  Fit's applications.csv) shows its scores, file, posting link and status; the status menu saves at once and rewrites
  applications.csv. A posting not sent yet shows "Not sent", its fit score and the day it was added; its trash (shown on
  hover or focus) deletes it, with Undo in the toast. V (or ⌘V) reads the clipboard: a link or the posting's text
  becomes a posting, and its review opens.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { openPath, openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
  import ClipboardPaste from "@lucide/svelte/icons/clipboard-paste";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import FileSpreadsheet from "@lucide/svelte/icons/file-spreadsheet";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import { appError, setStatus, type ApplicationRow, type AppStatus, type JobRow } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import { paste } from "$lib/paste.svelte";
  import { app } from "$lib/stores/app.svelte";
  import { STATUS } from "$lib/status";
  import { Badge, Button, CompanyMark, Empty, Menu, Table, Toolbar, dayKey, toast } from "$lib/ui";

  const STATUSES = (Object.keys(STATUS) as AppStatus[]).map((value) => ({ value, label: STATUS[value].label }));

  const COLUMNS = [
    { key: "date", label: "Date", width: "88px" },
    { key: "job", label: "Job" },
    { key: "now", label: "Now", numeric: true, width: "64px" },
    { key: "projected", label: "Projected", numeric: true, width: "96px" },
    { key: "onFile", label: "On file", numeric: true, width: "80px" },
    { key: "status", label: "Status", width: "148px" },
    { key: "resume", label: "Resume", width: "16%" },
    { key: "link", label: "Link", align: "end" as const, width: "68px" },
    { key: "delete", label: "", align: "end" as const, width: "64px" },
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
  }

  let loaded = $state(false);

  onMount(async () => {
    await Promise.all([app.refreshApplications(), app.refreshJobs(), app.settings ? null : app.loadSettings()]);
    loaded = true;
  });

  $effect(() => bindKeys({ v: paste.run, "mod+v": paste.run }));

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
        }),
      ),
    ].sort((a, b) => b.date.localeCompare(a.date)),
  );
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

  const score = (n: number | null | undefined) => (n == null ? "—" : Math.round(n));
  const fileName = (path: string) => path.split("/").pop() || path;

  async function changeStatus(row: ApplicationRow, status: AppStatus) {
    const before = row.status;
    const i = app.applications.findIndex((a) => a.id === row.id);
    if (i >= 0) app.applications[i] = { ...app.applications[i], status };
    try {
      const saved = await setStatus(row.id, status);
      const j = app.applications.findIndex((a) => a.id === saved.id);
      if (j >= 0) app.applications[j] = saved;
      // The posting's status shows in the sidebar's Recent.
      void app.refreshJobs();
    } catch (e) {
      const j = app.applications.findIndex((a) => a.id === row.id);
      if (j >= 0) app.applications[j] = { ...app.applications[j], status: before };
      toast(appError(e).message, { tone: "danger" });
    }
  }

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
    <Button icon={FileSpreadsheet} title={csvPath} onclick={() => run("open applications.csv", () => openPath(csvPath))}>
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
  <div class="table">
    <Table columns={COLUMNS} {rows} key={(r) => r.key} label="Applications">
      {#snippet cell(row, column)}
        {#if column.key === "date"}
          <span class="num" title={row.date}>{shortDate(row.date)}</span>
        {:else if column.key === "job"}
          <svelte:element
            this={row.href ? "a" : "span"}
            class={["job", row.href && "job--link"]}
            href={row.href}
            title="{row.company} · {row.role}"
          >
            <CompanyMark size={28} src={app.logos[row.companyKey]} />
            <span class="company">{row.company}</span>
            <span class="role meta">{row.role}</span>
          </svelte:element>
        {:else if column.key === "now"}
          {score(row.application?.now)}
        {:else if column.key === "projected"}
          {#if row.posting}
            <span title="Fit score">{score(row.posting.score)}</span>
          {:else}
            {score(row.application?.projected)}
          {/if}
        {:else if column.key === "onFile"}
          {score(row.application?.onFile)}
        {:else if column.key === "status"}
          {#if row.application}
            {@const application = row.application}
            <Menu
              value={application.status}
              options={STATUSES}
              label="Status for {row.company}"
              variant="ghost"
              size="sm"
              onchange={(s) => changeStatus(application, s)}
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
              title="Show {path} in Finder"
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
              title={url}
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
              title="Delete posting"
              onclick={() => app.deleteJob(posting)}
            />
          </span>
        {/if}
      {/snippet}
    </Table>
  </div>
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
  .table {
    display: grid;
    grid-template-rows: minmax(0, 1fr);
    flex: 1;
    min-height: 0;
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
