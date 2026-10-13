<!-- `/` is the Jobs screen. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";

  onMount(() => {
    goto("/jobs", { replaceState: true });
  });
</script>
