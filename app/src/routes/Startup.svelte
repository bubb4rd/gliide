<!--
  Startup (DESIGN.md): what the window shows, alone, when gliide couldn't open its data at launch. Why, that nothing
  was changed, and what fixes it: data from a newer gliide → Update gliide (the updater needs no data); data it can't
  read (damaged, locked, no space) → Try again (relaunches). Show in Finder reveals the file either way. Its settings
  are in the data, so the theme is the last one this Mac used.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import CircleArrowDown from "@lucide/svelte/icons/circle-arrow-down";
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
  import { Button, Empty, Toolbar } from "$lib/ui";

  let { problem }: { problem: StartupProblem } = $props();

  const newer = $derived(problem.kind === "newer");
  let update = $state<UpdateStatus | null>(null);
  /** Why the last step failed, or that there's nothing newer to install. */
  let note = $state<string | null>(null);

  onMount(() => {
    void updateStatus().then((s) => (update = s));
    const stop = onUpdate((s) => (update = s));
    return () => void stop.then((f) => f());
  });

  const working = $derived(update?.state === "checking" || update?.state === "installing");
  const label = $derived.by(() => {
    if (update?.state === "checking") return "Looking for an update…";
    if (update?.state === "installing")
      return update.progress === null ? "Downloading…" : `Downloading ${Math.round(update.progress * 100)}%`;
    return update?.available ? `Install gliide ${update.available}` : "Update gliide";
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

  $effect(() => bindKeys({ Enter: () => void (working ? undefined : newer ? updateNow() : tryAgain()) }));
</script>

<Toolbar>
  {#snippet crumbs()}<b>gliide</b>{/snippet}
</Toolbar>

<div class="center">
  <Empty
    title={newer ? "This gliide is older than your data" : "gliide can’t open your data"}
    body={newer
      ? "Your data was saved by a newer gliide, so this one can’t read it. Nothing was changed: update gliide to open it."
      : "It couldn’t read the file that holds your applications, experience and settings. Nothing was changed."}
  >
    {#snippet action()}
      <div class="actions">
        {#if newer}
          <Button variant="primary" size="lg" icon={CircleArrowDown} kbd="⏎" disabled={working} onclick={updateNow}
            >{label}</Button
          >
        {:else}
          <Button variant="primary" size="lg" icon={RotateCcw} kbd="⏎" onclick={tryAgain}>Try again</Button>
        {/if}
        <Button size="lg" icon={FolderOpen} onclick={reveal}>Show in Finder</Button>
      </div>
      <p class="meta detail">{problem.detail}</p>
      {#if note}<p class="meta note" role="status">{note}</p>{/if}
    {/snippet}
  </Empty>
</div>

<style>
  .center {
    flex: 1;
    display: grid;
    place-items: center;
    min-height: 0;
    overflow: auto;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: var(--space-2);
  }
  .detail,
  .note {
    max-width: 52ch;
    margin: var(--space-4) auto 0;
    text-wrap: pretty;
    user-select: text;
  }
  .note {
    margin-top: var(--space-2);
    color: var(--ink-2);
  }
</style>
