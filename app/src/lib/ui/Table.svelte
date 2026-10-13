<!--
  A full-width table on --surface (Applications): sticky header, hairline rows, tabular numbers, a hover wash, no
  zebra stripes. Columns name their alignment; `numeric` columns are tabular and end-aligned. Cells come from the
  `cell` snippet, so a column can hold a badge, a menu or a link.
-->
<script lang="ts" generics="Row">
  import type { Snippet } from "svelte";

  interface Column {
    key: string;
    label: string;
    numeric?: boolean;
    align?: "start" | "end";
    /** CSS width (`120px`, `30%`); omitted, the column takes what's left. */
    width?: string;
  }

  interface Props {
    columns: Column[];
    rows: Row[];
    /** A stable id per row. */
    key: (row: Row) => string;
    cell: Snippet<[Row, Column]>;
    /** What the table lists, for assistive tech. */
    label: string;
  }

  let { columns, rows, key, cell, label }: Props = $props();
</script>

<div class="wrap">
  <table aria-label={label}>
    <colgroup>
      {#each columns as column (column.key)}<col style:width={column.width} />{/each}
    </colgroup>
    <thead>
      <tr>
        {#each columns as column (column.key)}
          <th scope="col" class={[(column.numeric || column.align === "end") && "end"]}>{column.label}</th>
        {/each}
      </tr>
    </thead>
    <tbody>
      {#each rows as row (key(row))}
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
    letter-spacing: 0.04em;
    text-transform: uppercase;
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
  @media (hover: hover) {
    tbody tr:hover td {
      background: var(--hover);
    }
  }
</style>
