<!-- /bank/import?file=<path>: one resume file's import review; a fresh review per file. -->
<script lang="ts">
  import { page } from "$app/state";
  import ImportReview from "./ImportReview.svelte";

  const path = $derived(page.url.searchParams.get("file") ?? "");
</script>

{#key path}<ImportReview {path} />{/key}
