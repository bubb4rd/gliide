<!--
  The `$` panel: the posting's salary range, yours to set, change or remove. Either end may stay empty ("up to",
  "from"); amounts as you'd type them (80,000, 80k, 45.50). Saving makes it yours: a re-read of the posting leaves it.
-->
<script lang="ts">
  import { untrack } from "svelte";
  import type { SalaryPeriod, SalaryRange } from "$lib/api";
  import { amountField, parseAmount } from "$lib/salary";
  import { Button, Input, Segmented } from "$lib/ui";

  interface Props {
    salary: SalaryRange | null;
    /** Resolves true once it's saved (`null`: removed). */
    onsave: (salary: SalaryRange | null) => Promise<boolean>;
  }

  let { salary, onsave }: Props = $props();

  const PERIODS: { value: SalaryPeriod; label: string }[] = [
    { value: "year", label: "Per year" },
    { value: "hour", label: "Per hour" },
  ];

  // The fields start from the range shown when the panel opens.
  const initial = untrack(() => salary);
  let min = $state(amountField(initial?.minCents ?? null));
  let max = $state(amountField(initial?.maxCents ?? null));
  let currency = $state(initial?.currency ?? "USD");
  let period = $state<SalaryPeriod>(initial?.period ?? "year");
  let busy = $state(false);

  const lo = $derived(parseAmount(min));
  const hi = $derived(parseAmount(max));
  const code = $derived(currency.trim().toUpperCase());
  const problem = $derived.by(() => {
    if (lo === undefined || hi === undefined) return "Type amounts like 80,000, 80k or 45.50.";
    if (lo !== null && hi !== null && lo > hi) return "The low end is above the high end.";
    if (!/^[A-Z]{3}$/.test(code)) return "The currency is a 3-letter code, like USD.";
    return null;
  });
  const empty = $derived(lo === null && hi === null);

  async function run(next: SalaryRange | null) {
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
    if (problem || empty || lo === undefined || hi === undefined) return;
    void run({ minCents: lo, maxCents: hi, currency: code, period });
  }
</script>

<form class="panel" onsubmit={submit}>
  <p class="label">Salary range</p>
  <div class="range">
    <Input
      size="md"
      bind:value={min}
      inputmode="decimal"
      placeholder="From"
      aria-label="Low end"
      {@attach (input: HTMLInputElement) => void requestAnimationFrame(() => input.focus())}
    />
    <span class="dash" aria-hidden="true">–</span>
    <Input size="md" bind:value={max} inputmode="decimal" placeholder="To" aria-label="High end" />
    <Input
      size="md"
      bind:value={currency}
      maxlength={3}
      autocapitalize="characters"
      spellcheck={false}
      aria-label="Currency"
    />
  </div>
  <div class="period">
    <Segmented options={PERIODS} bind:value={period} label="Per" />
  </div>
  {#if problem && !empty}<p class="meta problem" role="status">{problem}</p>{/if}
  <div class="actions">
    {#if salary}
      <Button variant="danger" size="sm" disabled={busy} onclick={() => void run(null)}>Remove</Button>
    {/if}
    <Button type="submit" variant="primary" size="sm" kbd="⏎" disabled={busy || empty || !!problem}>Save</Button>
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
  .range {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr) 64px;
    align-items: center;
    gap: var(--space-2);
  }
  .dash {
    color: var(--ink-3);
  }
  .period {
    display: flex;
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
