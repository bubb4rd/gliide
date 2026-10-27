<!--
  The window: a 232px sidebar (native vibrancy behind a translucent --sidebar; the traffic lights sit in its top 52px)
  and the main area, where each screen draws its own Toolbar. Under 960px the sidebar becomes a sheet. A new install
  opens on the first-run setup (`/welcome`), which has no sidebar, nor has the import review it opens; until settings
  and the library are in (and with them whether the setup is due), the window is paper.
-->
<script lang="ts">
  import "../app.css";
  import { onMount, type Snippet } from "svelte";
  import { dev } from "$app/environment";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import Archive from "@lucide/svelte/icons/archive";
  import CircleArrowDown from "@lucide/svelte/icons/circle-arrow-down";
  import FileText from "@lucide/svelte/icons/file-text";
  import ListChecks from "@lucide/svelte/icons/list-checks";
  import LayoutDashboard from "@lucide/svelte/icons/layout-dashboard";
  import Palette from "@lucide/svelte/icons/palette";
  import Rss from "@lucide/svelte/icons/rss";
  import SlidersHorizontal from "@lucide/svelte/icons/sliders-horizontal";
  import type { Stage } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import { app } from "$lib/stores/app.svelte";
  import { feed } from "$lib/stores/feed.svelte";
  import { shell } from "$lib/stores/shell.svelte";
  import { jobFit } from "$lib/fit";
  import { STATUS } from "$lib/status";
  import { when } from "$lib/rounds";
  import { AgentMark, CompanyMark, Nav, NavItem, ScoreBar, Toaster, Tooltips, tip } from "$lib/ui";

  let { children }: { children: Snippet } = $props();

  const RECENT = 8;
  /** A job's model call in flight, as its row in Recent says it. */
  const BUSY: Record<Stage, string> = {
    extracting: "Reading…",
    suggesting: "Suggesting…",
    rewriting: "Rewriting…",
    writing: "Writing…",
    revising: "Rewriting…",
    importing: "Importing…",
    instructing: "Working…",
    inviting: "Reading…",
  };

  /**
   * Each job's next round, as its row in Recent says it, by job id: "Tue 2 PM" within the week, else the day alone
   * ("Nov 5"), so the status keeps to the line.
   */
  const nextRounds = $derived.by(() => {
    const now = new Date();
    const today = new Date(now).setHours(0, 0, 0, 0);
    const by = new Map<string, string>();
    for (const a of app.applications) {
      const startsAt = a.nextRound?.startsAt;
      if (!a.job || !startsAt) continue;
      const start = new Date(startsAt);
      const days = Math.round((new Date(start).setHours(0, 0, 0, 0) - today) / 86_400_000);
      by.set(
        a.job,
        days < 7 ? when(startsAt, now, true) : start.toLocaleDateString(undefined, { month: "short", day: "numeric" }),
      );
    }
    return by;
  });

  /** Settings and the library are in: the first screen can draw. */
  let ready = $state(false);

  onMount(() => {
    void app
      .load()
      .then(() => (app.setupDue && path !== SETUP ? goto(SETUP, { replaceState: true }) : undefined))
      .finally(() => (ready = true));
    void feed.refresh();
    const listening = Promise.all([feed.listen(), app.listen()]);
    return () => void listening.then((stops) => stops.forEach((stop) => stop()));
  });

  // Leaving a screen closes the compact sheet.
  $effect(() => {
    void page.url.pathname;
    shell.sidebarOpen = false;
  });

  $effect(() =>
    bindKeys({
      "mod+,": () => goto("/settings"),
      Escape: () => {
        if (!shell.sidebarOpen) return false;
        shell.sidebarOpen = false;
      },
    }),
  );

  const SETUP = "/welcome";
  const path = $derived<string>(page.url.pathname);
  const within = (href: string) => path === href || path.startsWith(`${href}/`);
  const count = (n: number | undefined) => (n ? n : null);
  const bare = $derived(
    path === SETUP || (path === "/bank/import" && !!page.url.searchParams.get("back")?.startsWith(SETUP)),
  );
  $effect(() => {
    shell.bare = bare;
  });

  /** Claude Code as the account you're signed in to: its icon with a presence dot, its state (and plan) under it. */
  const agentLine = $derived.by(() => {
    const a = app.agent;
    if (!a) return { tone: "neutral", sub: "Not checked" } as const;
    if (!a.found) return { tone: "danger", sub: "Not installed" } as const;
    if (a.signedIn === false) return { tone: "warn", sub: "Signed out" } as const;
    if (a.signedIn === null) return { tone: "neutral", sub: "Couldn't check sign-in" } as const;
    return { tone: "gain", sub: ["Signed in", a.plan].filter(Boolean).join(" · ") } as const;
  });

  /** A newer release: the sidebar's foot says so, over the agent, until it's installed. */
  const updateLine = $derived.by(() => {
    const u = app.update;
    if (!u?.available) return null;
    if (u.state === "installing")
      return `Downloading${u.progress === null ? "…" : ` ${Math.round(u.progress * 100)}%`}`;
    return `gliide ${u.available}`;
  });
</script>

<div class={["window", (shell.compact || bare || !ready) && "window--compact"]}>
  {#if ready && !bare}
    {#if shell.compact && shell.sidebarOpen}
      <button class="scrim" type="button" aria-label="Hide sidebar" onclick={() => (shell.sidebarOpen = false)}></button>
    {/if}
    <aside class={["sidebar", shell.sidebarOpen && "sidebar--open"]} aria-label="Sidebar">
      <div class="sidebar__top" data-tauri-drag-region></div>

      <Nav label="Main">
        <NavItem href="/" icon={LayoutDashboard} current={path === "/"}>Overview</NavItem>
        <NavItem
          href="/applications"
          icon={ListChecks}
          current={within("/applications")}
          meta={count(app.applications.length + app.unsent.length)}
        >
          Applications
        </NavItem>
        <NavItem href="/feed" icon={Rss} current={within("/feed")} meta={count(feed.fits.length)} metaTitle="Fits">
          Feed
        </NavItem>
        <NavItem href="/bank" icon={Archive} current={within("/bank")}>Experience</NavItem>
        <NavItem href="/masters" icon={FileText} current={within("/masters")}>Resumes</NavItem>
      </Nav>

      {#if app.jobs.length}
        <section class="recent" aria-labelledby="recent-label">
          <h2 class="label" id="recent-label">Recent</h2>
          <Nav label="Recent jobs">
            {#each app.jobs.slice(0, RECENT) as job (job.id)}
              {@const round = nextRounds.get(job.id)}
              {@const status = job.status
                ? round
                  ? { ...STATUS[job.status], label: `${STATUS[job.status].label} · ${round}` }
                  : STATUS[job.status]
                : null}
              {@const fit = jobFit(job, app.applications)}
              {@const call = app.calls.find((c) => c.job === job.id)}
              {@const busy = call ? BUSY[call.stage] : null}
              {@const word = busy ?? status?.label}
              <!-- A status with a time leaves the role too little room at 232px: it takes the line alone. -->
              <NavItem
                href="/jobs/{job.id}"
                quiet
                current={path === `/jobs/${job.id}`}
                sub={round && !busy ? "" : job.role}
                {status}
                {busy}
                {@attach tip(`${job.company} · ${job.role}${word ? ` · ${word}` : ""}`)}
              >
                {#snippet lead()}<CompanyMark size={28} src={app.logos[job.companyKey]} />{/snippet}
                {#snippet trailing()}
                  {#if fit}<ScoreBar bar={false} now={fit.now} projected={fit.projected} onFile={fit.onFile} />{/if}
                {/snippet}
                {job.company}
              </NavItem>
            {/each}
          </Nav>
        </section>
      {:else}
        <div class="recent"></div>
      {/if}

      <div class="foot">
        <Nav label="App">
          {#if dev}<NavItem href="/specimen" icon={Palette} current={within("/specimen")}>Specimen</NavItem>{/if}
          <NavItem href="/settings" icon={SlidersHorizontal} current={within("/settings")}>Settings</NavItem>
        </Nav>
        {#if updateLine}
          <NavItem href="/settings#about" icon={CircleArrowDown} sub={updateLine}>Update available</NavItem>
        {/if}
        <NavItem href="/settings" sub={agentLine.sub} {@attach tip(app.agent?.problem)}>
          {#snippet lead()}<AgentMark src="/agents/claude.png" tone={agentLine.tone} />{/snippet}
          Claude Code
        </NavItem>
      </div>
    </aside>
  {/if}

  <main class="main">
    {#if ready}{@render children()}{/if}
    <Toaster />
    <Tooltips />
  </main>
</div>

<style>
  .window {
    display: grid;
    grid-template-columns: var(--sidebar-width) minmax(0, 1fr);
    height: 100vh;
    overflow: hidden;
  }
  .window--compact {
    grid-template-columns: minmax(0, 1fr);
  }

  .sidebar {
    display: grid;
    grid-template-rows: var(--toolbar-height) auto minmax(0, 1fr) auto;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--space-4);
    min-width: 0;
    padding: 0 var(--space-3) var(--space-3);
    background: var(--sidebar);
    box-shadow: inset -1px 0 0 var(--line);
    user-select: none;
  }
  /* Under 960px: a sheet over the content, opaque, shown from the toolbar's toggle. */
  .window--compact .sidebar {
    display: none;
  }
  .window--compact .sidebar--open {
    display: grid;
    position: fixed;
    inset-block: 0;
    inset-inline-start: 0;
    z-index: 20;
    width: var(--sidebar-width);
    border-inline-end: 1px solid transparent;
    background: var(--paper);
    box-shadow: var(--shadow-popover);
  }
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 19;
    border: 0;
    padding: 0;
    background: transparent;
  }

  .recent {
    display: grid;
    align-content: start;
    gap: var(--space-1);
    min-height: 0;
    overflow: auto;
    overscroll-behavior: contain;
  }
  .recent .label {
    padding: var(--space-2) var(--space-2) 0;
  }

  .foot {
    display: grid;
    gap: var(--space-1);
  }

  .main {
    position: relative;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    background: var(--paper);
  }
</style>
