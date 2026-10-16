<!--
  The screen's toolbar (52px): crumbs at the leading edge, the screen's controls trailing with the primary action
  last (only a delete follows it). Drags the window from any empty part. An `<hr>` among the controls draws a short
  divider. When the window is compact the sidebar becomes a sheet, and the toolbar makes room for the traffic lights
  and its toggle; in a window without a sidebar (the first-run setup) it makes room for the lights alone. `over`
  lays it over the top of the screen's scroll area (which starts under it) in paper with its hairline; `clear` drops
  both, so a `Backdrop` shows through it until the content scrolls under it.
-->
<script lang="ts">
  import type { Snippet } from "svelte";
  import PanelLeft from "@lucide/svelte/icons/panel-left";
  import { shell } from "$lib/stores/shell.svelte";
  import Button from "./Button.svelte";

  interface Props {
    crumbs: Snippet;
    children?: Snippet;
    over?: boolean;
    clear?: boolean;
  }

  let { crumbs, children, over = false, clear = false }: Props = $props();
</script>

<header
  class={[
    "toolbar",
    (shell.compact || shell.bare) && "toolbar--compact",
    over && "toolbar--over",
    clear && "toolbar--clear",
  ]}
  data-tauri-drag-region="deep"
>
  {#if shell.compact && !shell.bare}
    <Button
      variant="ghost"
      square
      icon={PanelLeft}
      aria-label="Show sidebar"
      aria-expanded={shell.sidebarOpen}
      onclick={() => (shell.sidebarOpen = !shell.sidebarOpen)}
    />
  {/if}
  <div class="crumbs">{@render crumbs()}</div>
  <div class="trailing">{@render children?.()}</div>
</header>

<style>
  .toolbar {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-2);
    height: var(--toolbar-height);
    padding-inline: var(--space-4) var(--space-3);
    box-shadow: inset 0 -1px 0 var(--line);
    user-select: none;
  }
  /* Traffic lights: 20px inset + three 12px lights 8px apart, then the column padding. */
  .toolbar--compact {
    padding-inline-start: calc(var(--space-5) + 3 * 12px + 2 * var(--space-2) + var(--space-4));
  }
  .toolbar--over {
    position: absolute;
    inset-block-start: 0;
    inset-inline: 0;
    z-index: 1;
    background: var(--paper);
  }
  .toolbar--clear {
    background: none;
    box-shadow: none;
  }
  /* Room for a focus ring (2px + 2px offset) inside the clip, without moving anything. */
  .crumbs {
    display: flex;
    flex: 1;
    align-items: center;
    gap: var(--space-1-5);
    min-width: 0;
    margin: calc(-1 * var(--space-1));
    padding: var(--space-1);
    overflow: hidden;
    white-space: nowrap;
    color: var(--ink-3);
    font-weight: var(--weight-medium);
  }
  .crumbs :global(a) {
    color: inherit;
    text-decoration: none;
    border-radius: var(--radius-xs);
    cursor: default;
    -webkit-user-drag: none;
  }
  .crumbs :global(b),
  .crumbs :global(strong) {
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--ink);
    font-weight: var(--weight-medium);
  }
  .crumbs > :global(.icon) {
    color: var(--ink-4);
  }
  .trailing {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-2);
  }
  .trailing :global(hr) {
    width: 1px;
    height: 16px;
    margin: 0 var(--space-1);
    border: 0;
    background: var(--line-strong);
  }
  @media (hover: hover) {
    .crumbs :global(a:hover) {
      color: var(--ink-2);
    }
  }
</style>
