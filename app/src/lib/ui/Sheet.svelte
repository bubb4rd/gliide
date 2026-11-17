<!--
  Sheet: a modal panel over the window (Settings), open while it's mounted. A native `<dialog>` opened with
  `showModal`: the screen under it is inert, Tab stays inside, Esc, Close or a click on the scrim closes it (`onclose`:
  the owner unmounts it), and focus goes back where it was. A `rail` on the leading side, on `--sidebar` as the
  window's sidebar (a search, the panes, a foot), then the pane on paper: its title (serif) and Close, over a body that
  scrolls on its own and starts at the top whenever the title changes. In with its scrim, opacity only, over
  `--dur-fast` (@starting-style); closes instantly. While it's open only its `keys` are bound (`bindKeys`, modal):
  nothing reaches the screen under it. Toasts show over its pane, since the window's can't be reached past the modal.
-->
<script lang="ts">
  import { untrack, type Snippet } from "svelte";
  import X from "@lucide/svelte/icons/x";
  import { bindKeys, type KeyBindings } from "$lib/keys";
  import Button from "./Button.svelte";
  import Toaster from "./Toaster.svelte";
  import { tip } from "./tip.svelte";
  import { toastHost } from "./toast.svelte";

  interface Props {
    /** Names the dialog ("Settings"). */
    label: string;
    /** The pane's heading ("Agents"). */
    title: string;
    /** The leading column: a search, the panes, a foot. */
    rail?: Snippet;
    children: Snippet;
    /** Esc, Close or the scrim: the owner unmounts the sheet. */
    onclose: () => void;
    /** The sheet's own keys while it's open (Esc is the dialog's). */
    keys?: KeyBindings;
  }

  let { label, title, rail, children, onclose, keys = {} }: Props = $props();

  const id = $props.id();
  let dialog = $state<HTMLDialogElement>();
  let body = $state<HTMLElement>();
  /** A press that started on the scrim: only then does its click close the sheet (not a drag out of a field). */
  let fromScrim = false;

  $effect(() => {
    if (dialog && !dialog.open) dialog.showModal();
  });

  $effect(() => {
    untrack(() => toastHost.sheets++);
    return () => untrack(() => toastHost.sheets--);
  });

  $effect(() => bindKeys(keys, { modal: true }));

  // A new pane starts at its top.
  $effect(() => {
    void title;
    if (body) body.scrollTop = 0;
  });

  /** Closes the dialog (focus goes back where it was); its `close` event tells the owner. */
  function close() {
    dialog?.close();
  }

  function onpointerdown(e: PointerEvent) {
    fromScrim = e.target === dialog;
  }

  function onclick(e: MouseEvent) {
    if (e.target === dialog && fromScrim) close();
    fromScrim = false;
  }
</script>

<!-- The scrim is the dialog's ::backdrop: a click there lands on the dialog itself. Esc closes it natively. -->
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<dialog bind:this={dialog} class="dialog" aria-label={label} {onpointerdown} {onclick} onclose={() => onclose()}>
  {#if rail}<div class="rail">{@render rail()}</div>{/if}
  <section class="pane" aria-labelledby="{id}-title">
    <header class="head">
      <h2 class="title" id="{id}-title">{title}</h2>
      <Button variant="ghost" size="sm" square icon={X} aria-label="Close {label}" {@attach tip("Close")} onclick={close} />
    </header>
    <div class="body" bind:this={body}>{@render children()}</div>
    <Toaster layer="sheet" />
  </section>
</dialog>

<style>
  /* 16px of radius (a sheet), 32px clear of the window's edges at least; the popover shadow, not the page's. */
  .dialog {
    width: min(960px, 100vw - 2 * var(--space-8));
    max-width: none;
    height: min(720px, 100vh - 2 * var(--space-8));
    max-height: none;
    margin: auto;
    padding: 0;
    overflow: hidden;
    border: 1px solid transparent;
    border-radius: var(--radius-xl);
    color: var(--ink);
    background: var(--paper);
    box-shadow: var(--shadow-popover);
  }
  .dialog[open] {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    opacity: 1;
    transition: opacity var(--dur-fast) var(--ease-out);
  }
  .dialog::backdrop {
    background: var(--scrim);
  }
  .dialog[open]::backdrop {
    opacity: 1;
    transition: opacity var(--dur-fast) var(--ease-out);
  }
  @starting-style {
    .dialog[open],
    .dialog[open]::backdrop {
      opacity: 0;
    }
  }

  /* As the window's sidebar: --sidebar over paper, a hairline at its trailing edge. */
  .rail {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    width: 200px;
    min-height: 0;
    padding: var(--space-3);
    overflow: auto;
    overscroll-behavior: contain;
    background: var(--sidebar);
    box-shadow: inset -1px 0 0 var(--line);
  }

  .pane {
    position: relative;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    min-width: 0;
    min-height: 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-5) var(--space-4) var(--space-4) var(--space-6);
  }
  .head .title {
    flex: 1;
    min-width: 0;
  }
  .body {
    min-height: 0;
    padding: 0 var(--space-6) var(--space-6);
    overflow: auto;
    overscroll-behavior: contain;
  }
</style>
