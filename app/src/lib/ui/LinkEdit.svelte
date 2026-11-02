<!--
  Where you add or change a posting's link, inside a `Popover`: one field, opened with the link shown (focused and
  selected, so typing replaces it; `autofocus={false}` where it isn't the panel just opened). A bare acme.com/jobs/1 is
  taken as https; anything but a web link says so under the field and can't be saved. `removable`: the link is yours,
  so Remove shows. `onsave` resolves true once it's saved (`null`: removed); a failure leaves the panel as it was.
-->
<script lang="ts">
  import { untrack } from "svelte";
  import Button from "./Button.svelte";
  import Input from "./Input.svelte";

  interface Props {
    url: string | null;
    removable?: boolean;
    autofocus?: boolean;
    onsave: (url: string | null) => Promise<boolean>;
  }

  let { url, removable = false, autofocus = true, onsave }: Props = $props();

  // The field starts from the link shown when the panel opens.
  const initial = untrack(() => url) ?? "";
  let text = $state(initial);
  let busy = $state(false);

  /** A web page to open: `http` or `https` to a named host, nothing before it (the app checks again on save). */
  function isLink(s: string) {
    try {
      const u = new URL(s.includes("://") ? s : `https://${s}`);
      return (u.protocol === "https:" || u.protocol === "http:") && u.hostname.includes(".") && !u.username;
    } catch {
      return false;
    }
  }

  const typed = $derived(text.trim());
  const problem = $derived(typed && !isLink(typed) ? "Use a web link, like https://acme.com/jobs/123." : null);
  const unchanged = $derived(typed === initial.trim());

  async function run(next: string | null) {
    if (busy) return;
    busy = true;
    try {
      await onsave(next);
    } finally {
      busy = false;
    }
  }

  function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!typed || problem || unchanged) return;
    void run(typed);
  }
</script>

<!-- `novalidate`: the field's own check takes a bare acme.com/jobs/1, which the browser's `type=url` check refuses. -->
<form class="panel" novalidate onsubmit={submit}>
  <p class="label">Posting link</p>
  <Input
    size="md"
    type="url"
    bind:value={text}
    placeholder="https://"
    spellcheck={false}
    autocomplete="off"
    autocapitalize="off"
    aria-label="Posting link"
    aria-invalid={problem ? true : undefined}
    {@attach (input: HTMLInputElement) => {
      if (autofocus)
        requestAnimationFrame(() => {
          input.focus();
          input.select();
        });
    }}
  />
  {#if problem}<p class="meta problem" role="status">{problem}</p>{/if}
  <div class="actions">
    {#if removable}
      <Button variant="danger" size="sm" disabled={busy} onclick={() => void run(null)}>Remove</Button>
    {/if}
    <Button type="submit" variant="primary" size="sm" kbd="⏎" disabled={busy || !typed || !!problem || unchanged}
      >Save</Button
    >
  </div>
</form>

<style>
  .panel {
    display: grid;
    gap: var(--space-3);
    width: var(--column-min);
    max-width: 100%;
    padding: var(--space-3);
  }
  .panel p {
    margin: 0;
  }
  .problem {
    color: var(--danger);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
  }
</style>
