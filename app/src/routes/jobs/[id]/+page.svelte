<!-- /jobs/[id]: one posting's review; a fresh session per posting. -->
<script lang="ts">
  import { page } from "$app/state";
  import Review from "./Review.svelte";

  const job = $derived(page.params.id ?? "");
</script>

{#key job}<Review {job} />{/key}
