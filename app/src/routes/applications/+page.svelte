<!--
  Applications: every resume you sent (exported here, or imported from ATS Fit's applications.csv), newest first,
  with its scores, file, posting link and status. The status menu saves at once and rewrites applications.csv.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { openPath, openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import FileSpreadsheet from "@lucide/svelte/icons/file-spreadsheet";
  import { appError, setStatus, type ApplicationRow, type AppStatus } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import { app } from "$lib/stores/app.svelte";
  import { Badge, Button, Empty, Menu, Table, Toolbar, toast } from "$lib/ui";

  const STATUSES: { value: AppStatus; label: string }[] = [
    { value: "saved", label: "Saved" },
    { value: "applied", label: "Applied" },
    { value: "interviewing", label: "Interviewing" },
    { value: "offer", label: "Offer" },
    { value: "rejected", label: "Rejected" },
  ];
  // Amber means "left out" only (DESIGN.md), so no status is warn.
  const TONE = {
    saved: "neutral",
    applied: "accent",
    interviewing: "accent",
    offer: "gain",
    rejected: "danger",
  } as const satisfies Record<AppStatus, string>;

  const COLUMNS = [
    { key: "date", label: "Date", width: "104px" },
    { key: "company", label: "Company", width: "15%" },
    { key: "role", label: "Role" },
    { key: "now", label: "Now", numeric: true, width: "64px" },
    { key: "projected", label: "Projected", numeric: true, width: "96px" },
    { key: "onFile", label: "On file", numeric: true, width: "80px" },
    { key: "status", label: "Status", width: "148px" },
    { key: "resume", label: "Resume", width: "16%" },
    { key: "link", label: "Link", align: "end" as const, width: "68px" },
  ];

  let loaded = $state(false);

  onMount(async () => {
    await Promise.all([app.refreshApplications(), app.settings ? null : app.loadSettings()]);
    loaded = true;
  });

  const rows = $derived(app.applications);
  const csvPath = $derived(app.settings ? `${app.settings.exportDir.replace(/\/+$/, "")}/applications.csv` : null);

  $effect(() => {
    if (rows.length || !loaded) return;
    return bindKeys({ j: () => goto("/jobs") });
  });

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

  const score = (n: number | null) => (n === null ? "—" : Math.round(n));
  const fileName = (path: string) => path.split("/").pop() || path;

  async function changeStatus(row: ApplicationRow, status: AppStatus) {
    const before = row.status;
    const i = app.applications.findIndex((a) => a.id === row.id);
    if (i >= 0) app.applications[i] = { ...app.applications[i], status };
    try {
      const saved = await setStatus(row.id, status);
      const j = app.applications.findIndex((a) => a.id === saved.id);
      if (j >= 0) app.applications[j] = saved;
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
</Toolbar>

{#if rows.length}
  <div class="table">
    <Table columns={COLUMNS} {rows} key={(r) => r.id} label="Applications">
      {#snippet cell(row, column)}
        {#if column.key === "date"}
          <span class="num" title={row.date}>{shortDate(row.date)}</span>
        {:else if column.key === "company"}
          <span title={row.company}>{row.company}</span>
        {:else if column.key === "role"}
          <span class="role" title={row.role}>{row.role}</span>
        {:else if column.key === "now"}
          {score(row.now)}
        {:else if column.key === "projected"}
          {score(row.projected)}
        {:else if column.key === "onFile"}
          {score(row.onFile)}
        {:else if column.key === "status"}
          <Menu
            value={row.status}
            options={STATUSES}
            label="Status for {row.company}"
            variant="ghost"
            size="sm"
            onchange={(s) => changeStatus(row, s)}
          >
            {#snippet trigger(current)}<Badge tone={TONE[row.status]}>{current?.label}</Badge>{/snippet}
          </Menu>
        {:else if column.key === "resume"}
          {#if row.resume}
            {@const path = row.resume}
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
        {/if}
      {/snippet}
    </Table>
  </div>
{:else if loaded}
  <div class="center">
    <Empty
      title="No applications yet"
      body="Export a tailored resume from a job and it lands here with its scores, its file and a status."
    >
      {#snippet action()}<Button variant="primary" size="lg" kbd="J" onclick={() => goto("/jobs")}>Go to Jobs</Button>{/snippet}
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
  .role {
    color: var(--ink-2);
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
    .file:hover {
      text-decoration: underline;
      text-underline-offset: 2px;
    }
  }
</style>
