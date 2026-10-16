<!--
  /bank/import?file=<path>[&back=<path>]: one resume file's import review; a fresh review per file. `back` (an app path,
  the first-run setup's) is where it goes once the file is added or discarded.
-->
<script lang="ts">
  import { page } from "$app/state";
  import ImportReview from "./ImportReview.svelte";

  const path = $derived(page.url.searchParams.get("file") ?? "");
  const back = $derived.by(() => {
    const to = page.url.searchParams.get("back");
    return to?.startsWith("/") && !to.startsWith("//") ? to : null;
  });
</script>

{#key path}<ImportReview {path} {back} />{/key}
