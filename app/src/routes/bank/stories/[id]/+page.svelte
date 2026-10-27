<!-- /bank/stories/[id]: one story's editor; `new` (with `?entry=` for the entry it happened in) for a blank one. -->
<script lang="ts">
  import { page } from "$app/state";
  import StoryEditor from "./StoryEditor.svelte";

  const id = $derived(page.params.id ?? "");
  const entry = $derived(page.url.searchParams.get("entry"));
</script>

{#key id}<StoryEditor {id} {entry} />{/key}
