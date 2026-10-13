<!--
  The `a` picker: something new of your own on the open review: a skill, a bullet, or a summary when the master has
  none. Existing lines are edited in place (the review's Other lines).
-->
<script lang="ts">
  import type { CustomEdit, EditTargets } from "$lib/api";
  import { Button, Input, Menu, Segmented } from "$lib/ui";

  interface Props {
    targets: EditTargets;
    /** The master has a summary (edited in place), so the picker doesn't offer one. */
    hasSummary: boolean;
    /** Resolves true once the edit is in. */
    onadd: (edit: CustomEdit) => Promise<boolean>;
  }

  let { targets, hasSummary, onadd }: Props = $props();

  type Kind = "skill" | "bullet" | "summary";
  const kinds = $derived<{ value: Kind; label: string }[]>([
    { value: "skill", label: "Skill" },
    { value: "bullet", label: "Bullet" },
    ...(hasSummary ? [] : [{ value: "summary" as const, label: "Summary" }]),
  ]);
  const COPY = {
    skill: { target: "Skills line", placeholder: "The skill, as the posting says it" },
    bullet: { target: "Entry", placeholder: "The new bullet" },
    summary: { target: "", placeholder: "Your summary" },
  } as const;

  let kind = $state<Kind>("skill");
  let target = $state("");
  let text = $state("");
  let busy = $state(false);

  const list = $derived(kind === "skill" ? targets.groups : kind === "bullet" ? targets.entries : []);
  const options = $derived(list.map((t) => ({ value: t.id, label: t.label })));

  // Each kind starts on its first target.
  $effect(() => {
    if (!list.some((t) => t.id === target)) target = list[0]?.id ?? "";
  });

  function edit(): CustomEdit {
    switch (kind) {
      case "skill":
        return { kind, group: target, item: text };
      case "bullet":
        return { kind, entry: target, text };
      case "summary":
        return { kind, text };
    }
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!text.trim() || busy) return;
    busy = true;
    try {
      if (await onadd(edit())) text = "";
    } finally {
      busy = false;
    }
  }
</script>

<form class="picker" onsubmit={submit}>
  <p class="label">Add your own</p>
  <div class="kinds">
    <Segmented
      options={kinds}
      bind:value={kind}
      label="What to add"
      onchange={() => {
        text = "";
      }}
    />
  </div>
  {#if kind !== "summary"}
    {#if options.length}
      <Menu value={target} {options} label={COPY[kind].target} onchange={(id) => (target = id)} />
    {:else}
      <p class="meta">Nothing on this master to add it to.</p>
    {/if}
  {/if}
  <!-- Focused once the popover has opened (it mounts just before it shows). -->
  <Input
    size="md"
    bind:value={text}
    placeholder={COPY[kind].placeholder}
    aria-label={COPY[kind].placeholder}
    {@attach (input: HTMLInputElement) => void requestAnimationFrame(() => input.focus())}
  />
  <div class="actions">
    <Button
      type="submit"
      variant="primary"
      size="sm"
      disabled={busy || !text.trim() || (kind !== "summary" && !target)}>Add</Button
    >
  </div>
</form>

<style>
  .picker {
    display: grid;
    gap: var(--space-3);
    width: var(--column-min);
    max-width: 100%;
    padding: var(--space-3);
  }
  .picker .meta {
    margin: 0;
  }
  .kinds {
    display: flex;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
  }
</style>
