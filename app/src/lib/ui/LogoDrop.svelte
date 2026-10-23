<!--
  Where you set a company's logo: drop an image file on it, paste one (⌘V) or choose one. Sits in a `Popover` anchored
  to the job header's `CompanyMark`. `over`: files are dragged over it; `busy`: a logo is being set; `set`: the company
  has one, so Remove shows.
-->
<script lang="ts">
  import type { HTMLAttributes } from "svelte/elements";
  import ClipboardPaste from "@lucide/svelte/icons/clipboard-paste";
  import ImageUp from "@lucide/svelte/icons/image-up";
  import Button from "./Button.svelte";
  import FileDrop from "./FileDrop.svelte";

  interface Props extends HTMLAttributes<HTMLDivElement> {
    over?: boolean;
    busy?: boolean;
    set?: boolean;
    onpaste: () => void;
    onchoose: () => void;
    onremove: () => void;
  }

  let { over = false, busy = false, set = false, onpaste, onchoose, onremove, class: className, ...rest }: Props =
    $props();
</script>

<div class={["drop", className]} {...rest}>
  <p class="label">Company photo</p>
  <FileDrop
    size="sm"
    icon={ImageUp}
    {over}
    {busy}
    text={busy ? "Setting the photo…" : over ? "Drop to set the photo" : "Drop an image here"}
  />
  <div class="actions">
    <Button size="sm" icon={ClipboardPaste} kbd="⌘V" disabled={busy} onclick={onpaste}>Paste</Button>
    <Button size="sm" disabled={busy} onclick={onchoose}>Choose file…</Button>
    {#if set}<Button size="sm" variant="ghost" class="remove" disabled={busy} onclick={onremove}>Remove</Button>{/if}
  </div>
</div>

<style>
  .drop {
    display: grid;
    gap: var(--space-2);
    width: 280px;
    padding: var(--space-2);
  }
  .label {
    margin: 0;
    padding: var(--space-1) var(--space-1) 0;
  }
  .actions {
    display: flex;
    gap: var(--space-1);
  }
  .actions :global(.remove) {
    margin-inline-start: auto;
  }
</style>
