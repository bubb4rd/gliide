<!--
  The window: a 232px sidebar (native vibrancy behind a translucent --sidebar; the traffic lights sit in its top 52px)
  and the main area, where each screen draws its own Toolbar. Under 960px the sidebar becomes a sheet. A new install
  opens on the first-run setup (`/welcome`), which has no sidebar, nor has the import review it opens; until settings
  and the library are in (and with them whether the setup is due), the window is paper. When gliide couldn't open its
  data at launch, the window shows `Startup` alone, without the sidebar. Settings is a modal sheet over whichever
  screen is showing (`shell.settings`), opened by ⌘, and the sidebar's Settings, update and agent rows. The first
  launch on a new version shows that release's `WhatsNew` in the main area's lower right, on the Overview (elsewhere
  it would cover a composer), until Got it, × or Esc.
-->
<script lang="ts">
  import "../app.css";
  import { onMount, type Snippet } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
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
  import {
    dismissWhatsNew,
    onNotification,
    startupProblem,
    whatsNew,
    type AgentStatus,
    type Stage,
    type StartupProblem,
    type WhatsNew as Release,
  } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import { app } from "$lib/stores/app.svelte";
  import { benchmark } from "$lib/stores/benchmark.svelte";
  import { feed } from "$lib/stores/feed.svelte";
  import { shell } from "$lib/stores/shell.svelte";
  import { jobFit } from "$lib/fit";
  import { STATUS } from "$lib/status";
  import { when } from "$lib/rounds";
  import { AgentMark, CompanyMark, Nav, NavItem, ScoreBar, Toaster, Tooltips, WhatsNew, tip } from "$lib/ui";
  import Startup from "./Startup.svelte";
  import Settings from "./settings/Settings.svelte";

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
    prepping: "Prepping…",
    practicing: "Reading…",
    nudging: "Nudging…",
    digging: "Reading…",
    debriefing: "Reading…",
    interviewing: "Reading…",
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
  /** Jobs whose application has a round awaiting its debrief: Recent says "Debrief" in the status's place. */
  const debriefsDue = $derived(new Set(app.applications.filter((a) => a.job && a.debriefDue).map((a) => a.job!)));

  /** Settings and the library are in: the first screen can draw. */
  let ready = $state(false);
  /** The data couldn't be opened at launch: only Startup shows, and nothing else asks the app for anything. */
  let problem = $state<StartupProblem | null>(null);

  onMount(() => {
    let stops: Promise<(() => void)[]> | null = null;
    void startupProblem()
      .catch(() => null)
      .then((found) => {
        problem = found;
        if (found) return void (ready = true);
        void app
          .load()
          .then(() => (app.setupDue && path !== SETUP ? goto(SETUP, { replaceState: true }) : undefined))
          // Also during setup: a fresh install records its version there, so its next launch shows nothing.
          .then(() => void showWhatsNew())
          .finally(() => (ready = true));
        void feed.refresh();
        // A round's banner clicked: its job (or its debrief). A benchmark run's status: the rail's count.
        stops = Promise.all([
          feed.listen(),
          app.listen(),
          benchmark.listen(),
          onNotification((to) => void goto(to)),
        ]);
      });
    return () => {
      void stops?.then((all) => all.forEach((stop) => stop()));
      if (release?.picture) URL.revokeObjectURL(release.picture);
    };
  });

  /** The running version's release, while its notes are due; `picture` as a blob URL. */
  let release = $state<(Omit<Release, "picture"> & { picture: string | null }) | null>(null);
  async function showWhatsNew() {
    const r = await whatsNew().catch(() => null);
    if (!r) return;
    const picture = r.picture
      ? URL.createObjectURL(new Blob([new Uint8Array(r.picture.bytes)], { type: r.picture.mediaType }))
      : null;
    release = { ...r, picture };
  }
  function dismissRelease() {
    if (!release) return;
    void dismissWhatsNew(release.version);
    const picture = release.picture;
    release = null;
    // After its exit, so the picture shows until the card is gone.
    if (picture) setTimeout(() => URL.revokeObjectURL(picture), 1000);
  }

  // Leaving a screen closes the compact sheet, and Settings (a banner's click goes to a job).
  $effect(() => {
    void page.url.pathname;
    shell.sidebarOpen = false;
    shell.closeSettings();
  });

  $effect(() =>
    bindKeys({
      "mod+,": () => shell.openSettings(),
      Escape: () => {
        if (!shell.sidebarOpen) return false;
        shell.sidebarOpen = false;
      },
    }),
  );

  const SETUP = "/welcome";
  const path = $derived<string>(page.url.pathname);
  const releaseShown = $derived(release !== null && path === "/");
  // Esc closes it, after anything newer that takes Esc first (a field, a menu, the Settings sheet).
  $effect(() => (releaseShown ? bindKeys({ Escape: dismissRelease }) : undefined));
  const within = (href: string) => path === href || path.startsWith(`${href}/`);
  const count = (n: number | undefined) => (n ? n : null);
  const bare = $derived(
    problem !== null ||
      path === SETUP ||
      (path === "/bank/import" && !!page.url.searchParams.get("back")?.startsWith(SETUP)),
  );
  $effect(() => {
    shell.bare = bare;
  });

  /** One agent's state, as its dot and words say it: alone ("Signed out"), and named among others ("Codex signed
   *  out"); `rank` orders them worst first. */
  function stateOf(a: AgentStatus | null, name: string) {
    if (!a) return { tone: "neutral", sub: "Not checked", among: "Not checked", rank: 1 } as const;
    if (!a.found) return { tone: "danger", sub: "Not installed", among: `${name} not installed`, rank: 3 } as const;
    if (a.signedIn === false) return { tone: "warn", sub: "Signed out", among: `${name} signed out`, rank: 2 } as const;
    if (a.signedIn === null)
      return { tone: "neutral", sub: "Couldn't check sign-in", among: `Couldn't check ${name}'s sign-in`, rank: 1 } as const;
    const sub = ["Signed in", a.plan].filter(Boolean).join(" · ");
    return { tone: "gain", sub, among: "Signed in", rank: 0 } as const;
  }

  /** The agents model calls run on (`app.routed`), as the accounts you're signed in to: the one in the worst state
   *  leads (its icon with a presence dot; Tailoring's on a tie), every one's name, and that state under them. */
  const agentLine = $derived.by(() => {
    const states = app.routed.map((info) => {
      const status = app.status(info.id);
      return { info, status, ...stateOf(status, info.name) };
    });
    const worst = states.reduce((w, s) => (s.rank > w.rank ? s : w));
    return {
      icon: worst.info.icon,
      tone: worst.tone,
      name: states.map((s) => s.info.name).join(" + "),
      sub: states.length > 1 ? worst.among : worst.sub,
      problem: worst.status?.problem ?? null,
    };
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
              {@const due = debriefsDue.has(job.id)}
              {@const round = due ? undefined : nextRounds.get(job.id)}
              {@const status = due
                ? { label: "Debrief", tone: "accent" as const }
                : job.status
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
                  {#if fit}
                    <ScoreBar
                      bar={false}
                      now={fit.now}
                      projected={fit.projected}
                      onFile={fit.onFile}
                      rescoring={call?.stage === "extracting" || call?.stage === "suggesting"}
                    />
                  {/if}
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
          <NavItem
            icon={SlidersHorizontal}
            current={shell.settings !== null}
            aria-haspopup="dialog"
            onclick={() => shell.openSettings()}>Settings</NavItem
          >
        </Nav>
        {#if updateLine}
          <NavItem
            icon={CircleArrowDown}
            sub={updateLine}
            aria-haspopup="dialog"
            onclick={() => shell.openSettings("about")}>Update available</NavItem
          >
        {/if}
        <NavItem
          sub={agentLine.sub}
          aria-haspopup="dialog"
          onclick={() => shell.openSettings("agents")}
          {@attach tip(agentLine.problem)}
        >
          {#snippet lead()}<AgentMark src={agentLine.icon} tone={agentLine.tone} />{/snippet}
          {agentLine.name}
        </NavItem>
      </div>
    </aside>
  {/if}

  <main class="main">
    {#if problem}<Startup {problem} />{:else if ready}{@render children()}{/if}
    {#if ready && !problem && shell.settings}<Settings />{/if}
    <Toaster />
    <Tooltips />
    {#if release && releaseShown}
      <div class="whats-new">
        <WhatsNew
          version={release.version}
          date={release.date}
          notes={release.notes}
          picture={release.picture}
          onrelease={() => void openUrl(release!.url)}
          ondismiss={dismissRelease}
        />
      </div>
    {/if}
  </main>
</div>

<style>
  /* What's new: 12px in from the main area's lower right, growing up to 12px under the toolbar at most. */
  .whats-new {
    position: absolute;
    z-index: 20;
    inset-inline-end: var(--space-3);
    inset-block-end: var(--space-3);
    --wn-room: calc(100vh - var(--toolbar-height) - var(--space-3) * 2);
  }
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
