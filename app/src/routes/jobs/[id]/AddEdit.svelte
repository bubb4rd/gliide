<!-- The `a` picker: an edit of your own (a skill, a bullet, a line's wording or the summary) on the open review. -->
<script lang="ts">
  import type { CustomEdit, EditTargets } from "$lib/api";
  import { Button, Input, Menu, Segmented } from "$lib/ui";

  interface Props {
    targets: EditTargets;
    /** Resolves true once the edit is in. */
    onadd: (edit: CustomEdit) => Promise<boolean>;
  }

  let { targets, onadd }: Props = $props();

  type Kind = CustomEdit["kind"];
  const KINDS: { value: Kind; label: string }[] = [
    { value: "skill", label: "Skill" },
    { value: "bullet", label: "Bullet" },
    { value: "line", label: "Line" },
    { value: "summary", label: "Summary" },
  ];
  const COPY = {
    skill: { target: "Skills line", placeholder: "The skill, as the posting says it" },
    bullet: { target: "Entry", placeholder: "The new bullet" },
    line: { target: "Line", placeholder: "The line as it should read" },
    summary: { target: "", placeholder: "Your summary" },
  } as const;

  let kind = $state<Kind>("skill");
  let target = $state("");
  let text = $state("");
  let busy = $state(false);

  const list = $derived(
    kind === "skill" ? targets.groups : kind === "bullet" ? targets.entries : kind === "line" ? targets.lines : [],
  );
  const options = $derived(list.map((t) => ({ value: t.id, label: t.label })));

  // Each kind starts on its first target.
  $effect(() => {
    if (!list.some((t) => t.id === target)) pick(list[0]?.id ?? "");
  });

  /** A line starts from its current wording (a note line's label isn't part of it). */
  function pick(id: string) {
    target = id;
    if (kind !== "line") return;
    const label = list.find((t) => t.id === id)?.label ?? "";
    text = id.includes("#") ? label.replace(/^[^:]*:\s*/, "") : label;
  }

  function edit(): CustomEdit {
    switch (kind) {
      case "skill":
        return { kind, group: target, item: text };
      case "bullet":
        return { kind, entry: target, text };
      case "line":
        return { kind, line: target, text };
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
      options={KINDS}
      bind:value={kind}
      label="What to add"
      onchange={() => {
        text = "";
      }}
    />
  </div>
  {#if kind !== "summary"}
    {#if options.length}
      <Menu value={target} {options} label={COPY[kind].target} onchange={pick} />
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
