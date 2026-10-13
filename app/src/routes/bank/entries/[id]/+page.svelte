<!-- /bank/entries/[id]: one bank entry's editor; a fresh one per entry. -->
<script lang="ts">
  import { page } from "$app/state";
  import EntryEditor from "./EntryEditor.svelte";

  const id = $derived(page.params.id ?? "");
</script>

{#key id}<EntryEditor {id} />{/key}
