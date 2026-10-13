<!--
  The key footer, collapsed: one button at the column's foot, Commands ⌘K, opens the screen's commands in a menu above
  it, each with its keys. A command with `run` is a menu item (click it, or ↑↓ and ⏎); one without (↑↓ Select) is
  listed for its keys only. Esc, Tab or a click outside closes it.
-->
<script module lang="ts">
  export interface KeyCommand {
    keys: string[];
    label: string;
    /** What the keys do, run from the menu; without it the row only shows the keys. */
    run?: () => unknown;
    /** Can't run now (nothing selected, nothing to undo). */
    disabled?: boolean;
  }
</script>

<script lang="ts">
  import { untrack } from "svelte";
  import Keyboard from "@lucide/svelte/icons/keyboard";
  import { bindKeys } from "$lib/keys";
  import Button from "./Button.svelte";
  import Kbd from "./Kbd.svelte";
  import Popover from "./Popover.svelte";

  interface Props {
    keys: KeyCommand[];
  }

  let { keys }: Props = $props();

  const id = $props.id();
  let open = $state(false);
  let button = $state<HTMLButtonElement>();
  let list = $state<HTMLDivElement>();
  let active = $state(-1);

  const runnable = (item: KeyCommand | undefined) => !!item?.run && !item.disabled;

  $effect(() => bindKeys({ "mod+k": () => void (open = !open) }));

  // Opening focuses the list on its first command.
  $effect(() => {
    if (!open) return;
    active = untrack(() => keys.findIndex(runnable));
    // The list lives in the column's DOM (shown in the top layer): focusing it must not scroll the column.
    list?.focus({ preventScroll: true });
  });

  function choose(i: number) {
    const item = keys[i];
    if (!runnable(item)) return;
    open = false;
    button?.focus();
    item.run!();
  }

  /** The next command from `from` in direction `by` (1 or -1), or `from` when there's none. */
  function step(from: number, by: 1 | -1): number {
    for (let i = from + by; i >= 0 && i < keys.length; i += by) if (runnable(keys[i])) return i;
    return from;
  }

  function onListKey(e: KeyboardEvent) {
    const handlers: Record<string, () => void> = {
      ArrowDown: () => (active = step(active, 1)),
      ArrowUp: () => (active = step(active, -1)),
      Home: () => (active = step(-1, 1)),
      End: () => (active = step(keys.length, -1)),
      Enter: () => choose(active),
      " ": () => choose(active),
      Escape: () => {
        open = false;
        button?.focus();
      },
      Tab: () => (open = false),
    };
    const handle = handlers[e.key];
    if (!handle) return;
    if (e.key !== "Tab") e.preventDefault();
    // Keys inside the menu never reach the screen's bindings.
    e.stopPropagation();
    handle();
  }
</script>

<footer class="foot">
  <Button
    bind:element={button}
    variant="ghost"
    size="sm"
    icon={Keyboard}
    kbd="⌘K"
    popovertarget="{id}-menu"
    aria-haspopup="menu"
    aria-expanded={open}
  >
    Commands
  </Button>
</footer>

<Popover bind:open anchor={button} placement="top-start" id="{id}-menu">
  <div
    bind:this={list}
    class="list"
    role="menu"
    tabindex="-1"
    aria-label="Commands"
    aria-activedescendant={active >= 0 ? `${id}-item-${active}` : undefined}
    onkeydown={onListKey}
  >
    {#each keys as item, i (item.label)}
      <!-- The menu handles keys for its items. -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        id="{id}-item-{i}"
        class={["item", i === active && "active", !item.run && "item--keys"]}
        role="menuitem"
        tabindex="-1"
        aria-disabled={!runnable(item)}
        onclick={() => choose(i)}
        onpointermove={() => runnable(item) && (active = i)}
      >
        <span class="label-text">{item.label}</span>
        <span class="kbds">{#each item.keys as key, k (k)}<Kbd>{key}</Kbd>{/each}</span>
      </div>
    {/each}
  </div>
</Popover>

<style>
  .foot {
    position: sticky;
    bottom: 0;
    z-index: 1;
    display: flex;
    padding: var(--space-1-5) var(--space-4);
    background: var(--paper);
    box-shadow: inset 0 1px 0 var(--line);
  }
  /* 6 inner + 5 padding + 1 border = 12 outer (the popover's --radius-lg). */
  .list {
    display: grid;
    gap: 1px;
    min-width: 240px;
    padding: calc(var(--space-1-5) - 1px);
    outline: none;
  }
  .item {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    height: var(--control-md);
    padding-inline: var(--space-2);
    border-radius: var(--radius-sm);
    color: var(--ink);
    cursor: default;
    user-select: none;
    white-space: nowrap;
  }
  .item.active {
    background: var(--hover);
  }
  .list:focus-visible .item.active {
    background: var(--selected);
  }
  .item[aria-disabled="true"] {
    color: var(--ink-4);
  }
  /* Listed for its keys only (↑↓ Select): readable, not an action. */
  .item--keys[aria-disabled="true"] {
    color: var(--ink-3);
  }
  .label-text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .kbds {
    display: inline-flex;
    gap: var(--space-0-5);
  }
</style>
