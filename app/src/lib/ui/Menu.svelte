<!--
  Menu (a select): a button showing the current choice, opening a listbox of options with optional meta (a master's
  score). `lead` draws something before each option's label and the button's (an agent's logo). Options that name a
  `group` are listed under it, in the order they come, each group headed by its name in a label (Settings → Agents'
  route menu: an agent's models under the agent). Keys: ↑↓ or Space/⏎ open; inside, ↑↓ Home End move (across groups),
  ⏎/Space choose, Esc closes, Tab closes.
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
    /** The group it's listed under; consecutive options with the same group share its heading. */
    group?: string;
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
    /** Drawn before each option's label, and the current one's in the button (unless `trigger` replaces it). */
    lead?: Snippet<[Option]>;
    /** Whether the list is open (bindable): a screen key can open it. */
    open?: boolean;
    /** The screen key that opens it, as a hint inside the button. */
    kbd?: string;
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
    lead,
    open = $bindable(false),
    kbd,
  }: Props = $props();

  const id = $props.id();
  let button = $state<HTMLButtonElement>();
  let list = $state<HTMLDivElement>();
  let active = $state(0);

  const current = $derived(options.find((o) => o.value === value));
  /** Runs of options sharing a group, each with its options' places in the whole list (the keys move across them). */
  const sections = $derived.by(() => {
    const out: { group: string | undefined; items: { option: Option; i: number }[] }[] = [];
    options.forEach((option, i) => {
      const last = out.at(-1);
      if (last && last.group === option.group) last.items.push({ option, i });
      else out.push({ group: option.group, items: [{ option, i }] });
    });
    return out;
  });

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
  {kbd}
  popovertarget="{id}-menu"
  aria-haspopup="listbox"
  aria-label="{label}: {current?.label ?? 'None'}"
  onkeydown={onTriggerKey}
>
  {#if trigger}{@render trigger(current)}{:else}{#if lead && current}{@render lead(current)}{/if}<span class="current"
      >{current?.label ?? "Choose…"}</span
    >{/if}
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
    {#each sections as section, s (s)}
      {#if section.group === undefined}
        {#each section.items as { option, i } (option.value)}{@render row(option, i)}{/each}
      {:else}
        <div class="group" role="group" aria-labelledby="{id}-group-{s}">
          <p class="group-label label" id="{id}-group-{s}">{section.group}</p>
          {#each section.items as { option, i } (option.value)}{@render row(option, i)}{/each}
        </div>
      {/if}
    {/each}
  </div>
</Popover>

{#snippet row(option: Option, i: number)}
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
    {#if lead}{@render lead(option)}{/if}
    <span class="label-text">{option.label}</span>
    {#if option.meta}<span class="meta num">{option.meta}</span>{/if}
  </div>
{/snippet}

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
  /* A group: its name as a label, inset to the options' text, a hairline over every group but the first. */
  .group {
    display: grid;
    gap: 1px;
  }
  .group + .group,
  .option + .group {
    margin-top: var(--space-1);
    padding-top: var(--space-1);
    box-shadow: inset 0 1px 0 var(--line);
  }
  .group-label {
    margin: 0;
    padding: var(--space-1) var(--space-2) var(--space-0-5) calc(var(--space-1-5) + 14px + var(--space-2));
    color: var(--ink-3);
    white-space: nowrap;
  }
</style>
