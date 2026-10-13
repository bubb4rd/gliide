<!--
  /jobs: every posting, newest first, or the empty state. V (or ⌘V) anywhere here reads the clipboard: a link or the
  posting's text becomes a posting, and its review opens.
-->
<script lang="ts">
  import { goto } from "$app/navigation";
  import { readText } from "@tauri-apps/plugin-clipboard-manager";
  import ClipboardPaste from "@lucide/svelte/icons/clipboard-paste";
  import { addJob, appError } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import { app } from "$lib/stores/app.svelte";
  import { STATUS } from "$lib/status";
  import { Badge, Button, CompanyMark, Empty, Table, Toolbar, toast } from "$lib/ui";

  let reading = $state(false);

  async function paste() {
    if (reading) return;
    reading = true;
    try {
      const text = (await readText().catch(() => ""))?.trim();
      if (!text) {
        toast("The clipboard has no text. Copy a job link or the posting first.", { tone: "neutral" });
        return;
      }
      const job = await addJob(text);
      void app.refreshJobs();
      await goto(`/jobs/${job.id}`);
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
    } finally {
      reading = false;
    }
  }

  $effect(() => bindKeys({ v: paste, "mod+v": paste }));

  const COLUMNS = [
    { key: "company", label: "Company", width: "24%" },
    { key: "role", label: "Role" },
    { key: "status", label: "Status", width: "120px" },
    { key: "score", label: "Score", numeric: true, width: "80px" },
    { key: "added", label: "Added", align: "end" as const, width: "112px" },
  ];
  const day = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });
</script>

<Toolbar>
  {#snippet crumbs()}<b>Jobs</b>{/snippet}
  {#if app.jobs.length}
    <Button variant="primary" icon={ClipboardPaste} kbd="V" disabled={reading} onclick={paste}>
      {reading ? "Reading…" : "Paste from clipboard"}
    </Button>
  {/if}
</Toolbar>

{#if app.jobs.length}
  <div class="table">
    <Table columns={COLUMNS} rows={app.jobs} key={(j) => j.id} label="Postings">
      {#snippet cell(job, column)}
        {#if column.key === "company"}
          <CompanyMark><span title={job.company}>{job.company}</span></CompanyMark>
        {:else if column.key === "role"}
          <a class="open" href="/jobs/{job.id}" title={job.role}>{job.role}</a>
        {:else if column.key === "status"}
          {#if job.status}<Badge tone={STATUS[job.status].tone}>{STATUS[job.status].label}</Badge>{/if}
        {:else if column.key === "score"}
          {job.score === null ? "—" : `${Math.round(job.score)}%`}
        {:else}
          <span class="meta num">{day.format(new Date(job.addedAt))}</span>
        {/if}
      {/snippet}
    </Table>
  </div>
{:else if app.jobsLoaded}
  <div class="empty">
    <Empty title="No postings yet" body="Paste a job link or the posting’s text, and the score shows up in a few seconds.">
      {#snippet action()}
        <Button variant="primary" size="lg" icon={ClipboardPaste} kbd="V" disabled={reading} onclick={paste}>
          {reading ? "Reading…" : "Paste from clipboard"}
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
  .open {
    color: var(--ink);
    font-weight: var(--weight-medium);
    text-decoration: none;
  }
  @media (hover: hover) {
    .open:hover {
      color: var(--accent-text);
    }
  }
  .empty {
    flex: 1;
    display: grid;
    place-items: center;
    min-height: 0;
  }
</style>
