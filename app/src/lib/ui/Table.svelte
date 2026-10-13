<!--
  A full-width table on --surface (Applications): sticky header, hairline rows, tabular numbers, a hover wash, no
  zebra stripes. Columns name their alignment; `numeric` columns are tabular and end-aligned. Cells come from the
  `cell` snippet, so a column can hold a badge, a menu or a link.

  A column with `sortBy` is sortable: its header is a button that sorts by it, a second click reverses the order. The
  sorted column's header is ink with an arrow (on the label's outer side, so end-aligned labels stay on the numbers'
  edge); others show the arrow faintly on hover. Empty values (`null`) sort last in both directions; ties keep the
  order `rows` came in.
-->
<script lang="ts" module>
  export type SortDir = "asc" | "desc";

  export interface TableSort {
    key: string;
    dir: SortDir;
  }

  export interface TableColumn<Row> {
    key: string;
    label: string;
    numeric?: boolean;
    align?: "start" | "end";
    /** CSS width (`120px`, `30%`); omitted, the column takes what's left. */
    width?: string;
    /** Makes the column sortable: what a row sorts by. `null` or `undefined` sorts last either way. */
    sortBy?: (row: Row) => string | number | null | undefined;
    /** The direction a first click sorts in: `desc` for numeric columns, `asc` otherwise. */
    sortFirst?: SortDir;
  }

  const collator = new Intl.Collator(undefined, { sensitivity: "base", numeric: true });
</script>

<script lang="ts" generics="Row">
  import type { Snippet } from "svelte";
  import ArrowDown from "@lucide/svelte/icons/arrow-down";
  import ArrowUp from "@lucide/svelte/icons/arrow-up";
  import Icon from "./Icon.svelte";

  interface Props {
    columns: TableColumn<Row>[];
    rows: Row[];
    /** A stable id per row. */
    key: (row: Row) => string;
    cell: Snippet<[Row, TableColumn<Row>]>;
    /** What the table lists, for assistive tech. */
    label: string;
    /** The column the rows are sorted by (bindable); none, they show in the order given. */
    sort?: TableSort;
  }

  let { columns, rows, key, cell, label, sort = $bindable() }: Props = $props();

  const sorted = $derived.by(() => {
    const by = sort && columns.find((c) => c.key === sort!.key)?.sortBy;
    if (!sort || !by) return rows;
    const sign = sort.dir === "asc" ? 1 : -1;
    const values = new Map(rows.map((row) => [row, by(row)]));
    return [...rows].sort((a, b) => {
      const x = values.get(a);
      const y = values.get(b);
      if (x == null || y == null) return x == null ? (y == null ? 0 : 1) : -1;
      const order = typeof x === "number" && typeof y === "number" ? x - y : collator.compare(String(x), String(y));
      return sign * order;
    });
  });

  function sortOn(column: TableColumn<Row>) {
    const first = column.sortFirst ?? (column.numeric ? "desc" : "asc");
    sort = { key: column.key, dir: sort?.key === column.key ? (sort.dir === "asc" ? "desc" : "asc") : first };
  }

  const ariaSort = (column: TableColumn<Row>) =>
    !column.sortBy ? undefined : sort?.key !== column.key ? "none" : sort.dir === "asc" ? "ascending" : "descending";
</script>

<div class="wrap">
  <table aria-label={label}>
    <colgroup>
      {#each columns as column (column.key)}<col style:width={column.width} />{/each}
    </colgroup>
    <thead>
      <tr>
        {#each columns as column (column.key)}
          {@const end = column.numeric || column.align === "end"}
          <th scope="col" class={[end && "end"]} aria-sort={ariaSort(column)}>
            {#if column.sortBy}
              {@const on = sort?.key === column.key}
              <button type="button" class={["sort", end && "sort--end", on && "sort--on"]} onclick={() => sortOn(column)}>
                {column.label}
                <Icon icon={on && sort?.dir === "asc" ? ArrowUp : ArrowDown} size={12} weight="medium" class="arrow" />
              </button>
            {:else}
              {column.label}
            {/if}
          </th>
        {/each}
      </tr>
    </thead>
    <tbody>
      {#each sorted as row (key(row))}
        <tr>
          {#each columns as column (column.key)}
            <td class={[column.numeric && "num", (column.numeric || column.align === "end") && "end"]}>
              {@render cell(row, column)}
            </td>
          {/each}
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .wrap {
    min-width: 0;
    overflow: auto;
    overscroll-behavior: contain;
    background: var(--surface);
  }
  table {
    width: 100%;
    border-collapse: separate;
    border-spacing: 0;
    table-layout: fixed;
  }
  th {
    position: sticky;
    top: 0;
    z-index: 1;
    height: var(--control-lg);
    padding: 0 var(--space-3);
    text-align: start;
    white-space: nowrap;
    font-size: var(--text-xs);
    line-height: var(--leading-xs);
    font-weight: var(--weight-medium);
    color: var(--ink-3);
    background: var(--surface);
    box-shadow: inset 0 -1px 0 var(--line-strong);
  }
  td {
    height: var(--control-lg);
    padding: var(--space-1-5) var(--space-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    border-bottom: 1px solid var(--line);
    transition: background-color var(--dur-instant) var(--ease-out);
  }
  th:first-child,
  td:first-child {
    padding-inline-start: var(--space-6);
  }
  th:last-child,
  td:last-child {
    padding-inline-end: var(--space-6);
  }
  tbody tr:last-child td {
    border-bottom: 0;
  }
  .end {
    text-align: end;
  }
  /* The label keeps its place: 4px of hit area each side is pulled back into the cell's padding. */
  .sort {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    height: var(--control-sm);
    max-width: calc(100% + 2 * var(--space-1));
    margin-inline: calc(-1 * var(--space-1));
    padding: 0 var(--space-1);
    border: 0;
    border-radius: var(--radius-xs);
    background: none;
    vertical-align: middle;
    cursor: default;
    user-select: none;
    transition: color var(--dur-instant) var(--ease-out);
  }
  .sort--end {
    flex-direction: row-reverse;
  }
  .sort :global(.arrow) {
    visibility: hidden;
    color: var(--ink-4);
  }
  .sort--on {
    color: var(--ink);
  }
  .sort--on :global(.arrow) {
    visibility: visible;
    color: inherit;
  }
  .sort:focus-visible :global(.arrow) {
    visibility: visible;
  }
  @media (hover: hover) {
    tbody tr:hover td {
      background: var(--hover);
    }
    .sort:hover {
      color: var(--ink-2);
    }
    .sort--on:hover {
      color: var(--ink);
    }
    .sort:hover :global(.arrow) {
      visibility: visible;
    }
  }
</style>
