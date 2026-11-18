<!--
  Startup (DESIGN.md): what the window shows, alone, when gliide couldn't open its data at launch, laid out as a
  sign-in page: the form on the left, the meadow as a `Showcase` on the right. Data from a newer gliide: it looks for
  the newest release as it opens and says which this is and which is out; Update installs it in one step (the updater
  needs no data), Download from GitHub is the way round a failed one, and the showcase's card holds the release's
  notes once a check found them. Data it can't read (damaged, locked, no space): Try again (relaunches). Show in
  Finder reveals the file either way. Its settings are in the data, so the theme is the last one this Mac used. Under
  960px the showcase steps aside and the form has the window.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
  import CircleArrowDown from "@lucide/svelte/icons/circle-arrow-down";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import {
    appError,
    checkUpdate,
    installUpdate,
    onUpdate,
    relaunch,
    updateStatus,
    type StartupProblem,
    type UpdateStatus,
  } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import { shell } from "$lib/stores/shell.svelte";
  import { Badge, Button, Card, ReleaseNotes, Showcase, Toolbar } from "$lib/ui";

  let { problem }: { problem: StartupProblem } = $props();

  /** Where every release is, for a download by hand. */
  const RELEASES = "https://github.com/bubb4rd/gliide/releases/latest";

  const newer = $derived(problem.kind === "newer");
  let update = $state<UpdateStatus | null>(null);
  /** Why the last step failed, or that there's nothing newer to install. */
  let note = $state<string | null>(null);

  onMount(() => {
    const stop = onUpdate((s) => (update = s));
    void updateStatus().then((s) => {
      update = s;
      // Updating is the only fix for newer data, so look for it now rather than on a press.
      if (newer && s.state === "idle" && !s.available && !s.checkedAt)
        void checkUpdate().then((found) => {
          if (found.error) note = found.error;
        });
    });
    return () => void stop.then((f) => f());
  });

  const checking = $derived(update?.state === "checking");
  const working = $derived(checking || update?.state === "installing");
  const label = $derived.by(() => {
    if (checking) return "Looking for an update…";
    if (update?.state === "installing")
      return update.progress === null ? "Downloading…" : `Downloading ${Math.round(update.progress * 100)}%`;
    return update?.available ? `Update to gliide ${update.available}` : "Update gliide";
  });
  /** The newest release, as the version card says it. */
  const latest = $derived.by(() => {
    if (!update || checking) return "Looking…";
    if (update.available) return `gliide ${update.available}`;
    if (update.error || !update.checkedAt) return "Couldn’t check";
    return "None newer yet";
  });

  /** Looks for the newest release and installs it (it relaunches into the new one), in one step. */
  async function updateNow() {
    note = null;
    try {
      const found = update?.available ? update : await checkUpdate();
      update = found;
      if (found.error) return void (note = found.error);
      if (!found.available) return void (note = `No newer release to install; this is gliide ${found.current}.`);
      await installUpdate();
    } catch (e) {
      note = appError(e).message;
    }
  }

  async function tryAgain() {
    note = null;
    try {
      await relaunch();
    } catch (e) {
      note = appError(e).message;
    }
  }

  const reveal = () => revealItemInDir(problem.path).catch((e) => (note = appError(e).message));
  const download = () => openUrl(RELEASES).catch((e) => (note = appError(e).message));

  $effect(() => bindKeys({ Enter: () => void (working ? undefined : newer ? updateNow() : tryAgain()) }));
</script>

<Toolbar over clear>
  {#snippet crumbs()}{/snippet}
</Toolbar>

<div class={["startup", shell.compact && "startup--compact"]}>
  <div class="side">
    <div class="form">
      <header class="head">
        <img class="mark enter" style:--i="0" src="/icon.png" alt="gliide" width="48" height="48" draggable="false" />
        <h1 class="title enter" style:--i="1">
          {newer ? "Update gliide to open your data" : "gliide can’t open your data"}
        </h1>
        <p class="lede enter" style:--i="2">
          {newer
            ? "Your data was saved by a newer gliide than this one. Nothing was changed: update, and it opens as you left it."
            : "It couldn’t read the file that holds your applications, experience and settings. Nothing was changed."}
        </p>
      </header>

      <div class="body enter" style:--i="3">
        {#if newer}
          <Card>
            <dl class="versions">
              <div class="version">
                <dt>This Mac</dt>
                <dd class="num">gliide {update?.current ?? ""}</dd>
              </div>
              <div class="version">
                <dt>Newest</dt>
                <dd class={["num", !update?.available && "quiet"]} aria-live="polite">
                  {latest}
                  {#if update?.available}<Badge tone="accent">New</Badge>{/if}
                </dd>
              </div>
            </dl>
          </Card>
          <Button
            variant="primary"
            size="lg"
            class="wide"
            icon={CircleArrowDown}
            kbd="⏎"
            disabled={working}
            onclick={updateNow}>{label}</Button
          >
          <p class="or meta" aria-hidden="true">or</p>
          <Button size="lg" class="wide" iconEnd={ExternalLink} onclick={download}>Download from GitHub</Button>
        {:else}
          <Button variant="primary" size="lg" class="wide" icon={RotateCcw} kbd="⏎" onclick={tryAgain}
            >Try again</Button
          >
        {/if}
      </div>

      <footer class="foot enter" style:--i="4">
        <Button variant="ghost" size="sm" icon={FolderOpen} onclick={reveal}>Show in Finder</Button>
        <p class="meta detail">{problem.detail}</p>
        {#if note ?? update?.error}<p class="meta note" role="status">{note ?? update?.error}</p>{/if}
      </footer>
    </div>
  </div>

  {#if !shell.compact}
    <div class="panel">
      <Showcase name="meadow" children={newer && update?.available && update.notes ? whatsNew : undefined} />
    </div>
  {/if}
</div>

{#snippet whatsNew()}
  <p class="label whats-new">What’s new</p>
  <ReleaseNotes notes={update?.notes ?? ""} version={update?.available} date={update?.released} />
{/snippet}

<style>
  /* Form and showcase side by side, 5:6; the showcase inset 12px from the window's edges. */
  .startup {
    flex: 1;
    display: grid;
    grid-template-columns: minmax(0, 5fr) minmax(0, 6fr);
    min-height: 0;
  }
  .startup--compact {
    grid-template-columns: minmax(0, 1fr);
  }
  .side {
    display: grid;
    place-items: center;
    min-height: 0;
    overflow: auto;
    padding: var(--toolbar-height) var(--space-6) var(--space-6);
  }
  .form {
    display: grid;
    gap: var(--space-6);
    width: 100%;
    max-width: 360px;
  }
  .head {
    display: grid;
    justify-items: center;
    gap: var(--space-2);
    text-align: center;
  }
  .mark {
    display: block;
    width: var(--space-12);
    height: var(--space-12);
    margin-bottom: var(--space-2);
    user-select: none;
  }
  .title {
    margin: 0;
    text-wrap: balance;
  }
  .lede {
    margin: 0;
    color: var(--ink-2);
    text-wrap: pretty;
  }

  .body {
    display: grid;
    gap: var(--space-3);
  }
  .body :global(.wide) {
    width: 100%;
    justify-content: center;
  }
  .versions {
    margin: 0;
  }
  .version {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    min-height: var(--control-lg);
    padding: var(--space-2) var(--space-4);
  }
  .version + .version {
    box-shadow: inset 0 1px 0 var(--line);
  }
  dt {
    color: var(--ink-3);
  }
  dd {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: 0;
    font-weight: var(--weight-medium);
  }
  dd.quiet {
    color: var(--ink-3);
    font-weight: var(--weight-regular);
  }
  /* "or" between two hairlines. */
  .or {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin: 0;
  }
  .or::before,
  .or::after {
    content: "";
    flex: 1;
    height: 1px;
    background: var(--line);
  }

  .foot {
    display: grid;
    justify-items: center;
    gap: var(--space-2);
    text-align: center;
  }
  .detail,
  .note {
    max-width: 52ch;
    margin: 0;
    text-wrap: pretty;
    user-select: text;
  }
  .note {
    color: var(--ink-2);
  }

  .panel {
    min-height: 0;
    padding: var(--space-3);
  }
  .whats-new {
    margin: 0 0 var(--space-1);
  }
</style>
