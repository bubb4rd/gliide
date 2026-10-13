<!--
  Menu (a select): a button showing the current choice, opening a listbox of options with optional meta (a master's
  score). Keys: ↑↓ or Space/⏎ open; inside, ↑↓ Home End move, ⏎/Space choose, Esc closes, Tab closes.
-->
<script lang="ts" generics="T">
  import type { Snippet } from "svelte";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Check from "@lucide/svelte/icons/check";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import Popover from "./Popover.svelte";

  interface Option {
    value: T;
    label: string;
    meta?: string;
  }

  interface Props {
    value: T;
    options: Option[];
    /** What is chosen, for assistive tech ("Master"). */
    label: string;
    disabled?: boolean;
    size?: "sm" | "md";
    variant?: "default" | "ghost";
    placement?: "bottom-start" | "bottom-end";
    onchange?: (value: T) => void;
    /** What the closed menu shows for the current choice (a status `Badge`); its label by default. */
    trigger?: Snippet<[Option | undefined]>;
  }

  let {
    value = $bindable(),
    options,
    label,
    disabled = false,
    size = "md",
    variant = "default",
    placement = "bottom-start",
    onchange,
    trigger,
  }: Props = $props();

  const id = $props.id();
  let open = $state(false);
  let button = $state<HTMLButtonElement>();
  let list = $state<HTMLDivElement>();
  let active = $state(0);

  const current = $derived(options.find((o) => o.value === value));

  $effect(() => {
    if (!open) return;
    active = Math.max(
      0,
      options.findIndex((o) => o.value === value),
    );
    list?.focus();
  });

  function choose(option: Option) {
    open = false;
    button?.focus();
    if (option.value === value) return;
    value = option.value;
    onchange?.(option.value);
  }

  function onTriggerKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      e.stopPropagation();
      open = true;
    }
  }

  function onListKey(e: KeyboardEvent) {
    const last = options.length - 1;
    const keys: Record<string, () => void> = {
      ArrowDown: () => (active = Math.min(active + 1, last)),
      ArrowUp: () => (active = Math.max(active - 1, 0)),
      Home: () => (active = 0),
      End: () => (active = last),
      Enter: () => options[active] && choose(options[active]),
      " ": () => options[active] && choose(options[active]),
      Escape: () => {
        open = false;
        button?.focus();
      },
      Tab: () => (open = false),
    };
    const run = keys[e.key];
    if (!run) return;
    if (e.key !== "Tab") e.preventDefault();
    // Keys inside the menu never reach the screen's bindings.
    e.stopPropagation();
    run();
  }

  $effect(() => {
    if (open) list?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: "nearest" });
  });
</script>

<Button
  bind:element={button}
  {variant}
  {size}
  {disabled}
  iconEnd={ChevronDown}
  popovertarget="{id}-menu"
  aria-haspopup="listbox"
  aria-label="{label}: {current?.label ?? 'None'}"
  onkeydown={onTriggerKey}
>
  {#if trigger}{@render trigger(current)}{:else}<span class="current">{current?.label ?? "Choose…"}</span>{/if}
</Button>

<Popover bind:open anchor={button} {placement} id="{id}-menu" matchWidth>
  <div
    bind:this={list}
    class="list"
    role="listbox"
    tabindex="-1"
    aria-label={label}
    aria-activedescendant="{id}-option-{active}"
    onkeydown={onListKey}
  >
    {#each options as option, i (option.value)}
      <!-- The listbox handles keys for its options. -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        id="{id}-option-{i}"
        data-index={i}
        class="option"
        class:active={i === active}
        role="option"
        aria-selected={option.value === value}
        tabindex="-1"
        onclick={() => choose(option)}
        onpointermove={() => (active = i)}
      >
        <span class="check-slot">
          {#if option.value === value}<Icon icon={Check} size={14} />{/if}
        </span>
        <span class="label-text">{option.label}</span>
        {#if option.meta}<span class="meta num">{option.meta}</span>{/if}
      </div>
    {/each}
  </div>
</Popover>

<style>
  .current {
    max-width: 24ch;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* 6 inner + 5 padding + 1 border = 12 outer (the popover's --radius-lg). */
  .list {
    display: grid;
    gap: 1px;
    padding: calc(var(--space-1-5) - 1px);
    outline: none;
  }
  .option {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: var(--control-md);
    padding-inline: var(--space-1-5) var(--space-2);
    border-radius: var(--radius-sm);
    color: var(--ink);
    cursor: default;
    user-select: none;
    white-space: nowrap;
  }
  .option.active {
    background: var(--hover);
  }
  .list:focus-visible .option.active {
    background: var(--selected);
  }
  .check-slot {
    display: inline-grid;
    place-items: center;
    width: 14px;
    flex: none;
    color: var(--accent-text);
  }
  .label-text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .option .meta {
    margin-inline-start: var(--space-4);
  }
</style>
