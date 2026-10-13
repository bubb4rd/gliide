<!--
  The window: a 232px sidebar (native vibrancy behind a translucent --sidebar; the traffic lights sit in its top 52px)
  and the main area, where each screen draws its own Toolbar. Under 960px the sidebar becomes a sheet.
-->
<script lang="ts">
  import "../app.css";
  import { onMount, type Snippet } from "svelte";
  import { dev } from "$app/environment";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import Archive from "@lucide/svelte/icons/archive";
  import Briefcase from "@lucide/svelte/icons/briefcase";
  import FileText from "@lucide/svelte/icons/file-text";
  import ListChecks from "@lucide/svelte/icons/list-checks";
  import Palette from "@lucide/svelte/icons/palette";
  import SlidersHorizontal from "@lucide/svelte/icons/sliders-horizontal";
  import { bindKeys } from "$lib/keys";
  import { app } from "$lib/stores/app.svelte";
  import { shell } from "$lib/stores/shell.svelte";
  import { STATUS } from "$lib/status";
  import { CompanyMark, Nav, NavItem, Toaster } from "$lib/ui";

  let { children }: { children: Snippet } = $props();

  const RECENT = 8;

  onMount(() => {
    app.load();
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

  const path = $derived<string>(page.url.pathname);
  const within = (href: string) => path === href || path.startsWith(`${href}/`);
  const count = (n: number | undefined) => (n ? n : null);

  const agentLine = $derived.by(() => {
    const a = app.agent;
    if (!a) return { tone: "unknown", text: "Claude Code · not checked" };
    if (!a.found) return { tone: "danger", text: "Claude Code · not found" };
    if (a.signedIn === false) return { tone: "warn", text: "Claude Code · signed out" };
    if (a.signedIn === null) return { tone: "unknown", text: "Claude Code · couldn't check sign-in" };
    return { tone: "gain", text: "Claude Code · signed in" };
  });
</script>

<div class={["window", shell.compact && "window--compact"]}>
  {#if shell.compact && shell.sidebarOpen}
    <button class="scrim" type="button" aria-label="Hide sidebar" onclick={() => (shell.sidebarOpen = false)}></button>
  {/if}
  <aside class={["sidebar", shell.sidebarOpen && "sidebar--open"]} aria-label="Sidebar">
    <div class="sidebar__top" data-tauri-drag-region></div>

    <Nav label="Main">
      <NavItem href="/jobs" icon={Briefcase} current={path === "/jobs"} meta={count(app.jobs.length)}>Jobs</NavItem>
      <NavItem
        href="/applications"
        icon={ListChecks}
        current={within("/applications")}
        meta={count(app.applications.length)}
      >
        Applications
      </NavItem>
      <NavItem href="/bank" icon={Archive} current={within("/bank")}>Experience bank</NavItem>
      <NavItem href="/masters" icon={FileText} current={within("/masters")}>Masters</NavItem>
    </Nav>

    {#if app.jobs.length}
      <section class="recent" aria-labelledby="recent-label">
        <h2 class="label" id="recent-label">Recent</h2>
        <Nav label="Recent jobs">
          {#each app.jobs.slice(0, RECENT) as job (job.id)}
            {@const status = job.status ? STATUS[job.status] : null}
            <NavItem
              href="/jobs/{job.id}"
              quiet
              current={path === `/jobs/${job.id}`}
              meta={job.score === null ? null : `${Math.round(job.score)}%`}
              metaTitle="Fit score"
              sub={job.role}
              {status}
              title="{job.company} · {job.role}{status ? ` · ${status.label}` : ''}"
            >
              {#snippet lead()}<CompanyMark size={28} />{/snippet}
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
      <a class="agent" href="/settings" title={app.agent?.problem ?? undefined}>
        <span class="agent__dot agent__dot--{agentLine.tone}" aria-hidden="true"></span>
        <span class="agent__text">{agentLine.text}</span>
      </a>
    </div>
  </aside>

  <main class="main">
    {@render children()}
    <Toaster />
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
  .agent {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
    padding: var(--space-1-5) var(--space-2);
    border-radius: var(--radius-sm);
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    color: var(--ink-3);
    text-decoration: none;
    cursor: default;
    -webkit-user-drag: none;
    transition: background-color var(--dur-instant) var(--ease-out);
  }
  .agent__text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .agent__dot {
    flex: none;
    width: 6px;
    height: 6px;
    border-radius: var(--radius-full);
    background: var(--ink-4);
  }
  .agent__dot--gain {
    background: var(--gain);
  }
  .agent__dot--warn {
    background: var(--warn);
  }
  .agent__dot--danger {
    background: var(--danger);
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
  @media (hover: hover) {
    .agent:hover {
      background: var(--hover);
    }
  }
</style>
