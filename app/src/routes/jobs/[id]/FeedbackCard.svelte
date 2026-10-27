<!--
  What came back for one practiced answer (DESIGN.md, Job → Interview → Practice; the story editor's Tell it): the
  lines it drew on, which of situation, task, action and result it has, what it says differently from the resume (amber),
  what it says the bank doesn't hold yet (Keep as fact), a tighter telling and the likely follow-up. Never a grade.
  `actions` adds to the Tighter row's (practice's Save as story).
-->
<script lang="ts">
  import type { Snippet } from "svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import Check from "@lucide/svelte/icons/check";
  import CircleDashed from "@lucide/svelte/icons/circle-dashed";
  import Copy from "@lucide/svelte/icons/copy";
  import Plus from "@lucide/svelte/icons/plus";
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import { appError, type Feedback, type NewClaim, type StarPart } from "$lib/api";
  import { Button, Icon, tip, toast } from "$lib/ui";

  interface Props {
    feedback: Feedback;
    /** A line the answer drew on, by id: its text and the entry it's under. */
    line: (id: string) => { text: string; entry: string | null } | undefined;
    /** Claims kept as facts, by text. */
    kept: Set<string>;
    onkeep: (claim: NewClaim) => void;
    /** What the tighter telling was written from ("Only your answer and what you sent"). */
    grounded: string;
    /** What a mismatch is against ("the resume you sent"). */
    against?: string;
    actions?: Snippet;
  }

  let { feedback: f, line, kept, onkeep, grounded, against = "the resume you sent", actions }: Props = $props();

  const STAR: { key: "situation" | "task" | "action" | "result"; label: string }[] = [
    { key: "situation", label: "Situation" },
    { key: "task", label: "Task" },
    { key: "action", label: "Action" },
    { key: "result", label: "Result" },
  ];
  const partWord = (p: StarPart) => (p === "yes" ? "" : p === "thin" ? " thin" : " missing");
  const clip = (text: string) => (text.length > 44 ? `${text.slice(0, 43).trimEnd()}…` : text);

  async function copy(text: string) {
    try {
      await writeText(text);
      toast("Copied");
    } catch (e) {
      toast(`Couldn’t copy: ${appError(e).message}`, { tone: "danger" });
    }
  }
</script>

<article class="card" aria-label="Feedback on your answer">
  {#if f.lines.length}
    <div class="row">
      <span class="row__label">Drew on</span>
      <div class="chips">
        {#each f.lines as id (id)}
          {@const drew = line(id)}
          {#if drew}
            <span class="chip" {@attach tip(drew.entry ? `${drew.entry}: ${drew.text}` : drew.text)}
              >{clip(drew.text)}</span
            >
          {/if}
        {/each}
      </div>
    </div>
  {/if}
  <div class="row">
    <span class="row__label">STAR</span>
    <div>
      <p class="star">
        {#each STAR as part (part.key)}
          <span class={["star__part", f[part.key] !== "yes" && "star__part--short"]}
            ><Icon icon={f[part.key] === "yes" ? Check : CircleDashed} size={14} weight="medium" />{part.label}{partWord(
              f[part.key],
            )}</span
          >
        {/each}
      </p>
      {#if f.starNote}<p class="meta">{f.starNote}</p>{/if}
    </div>
  </div>
  {#each f.mismatches as m (m.said + m.line)}
    <div class="row row--warn">
      <span class="row__label">Consistency</span>
      <p class="warn">
        <Icon icon={TriangleAlert} size={16} weight="medium" /><span
          >You said {m.said}; {against} says {m.resume}.</span
        >
      </p>
    </div>
  {/each}
  {#if f.newClaims.length}
    <div class="row">
      <span class="row__label">New to gliide</span>
      <div class="claims">
        {#each f.newClaims as claim (claim.text)}
          <div class="claim">
            <p>“{claim.text}”</p>
            {#if kept.has(claim.text)}
              <p class="meta kept"><Icon icon={Check} size={14} />Kept as a fact of {claim.entryName}</p>
            {:else}
              <p class="acts">
                <Button size="sm" icon={Plus} onclick={() => onkeep(claim)}>Keep as fact</Button>
                <span class="meta">On {claim.entryName}, your answer as its source</span>
              </p>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  {/if}
  {#if f.tighter}
    {@const tighter = f.tighter}
    <div class="row">
      <span class="row__label">Tighter</span>
      <div>
        <p class="tighter">{tighter}</p>
        <p class="acts">
          <Button variant="ghost" size="sm" icon={Copy} onclick={() => copy(tighter)}>Copy</Button>
          {@render actions?.()}
          <span class="meta">{grounded}</span>
        </p>
      </div>
    </div>
  {:else if actions}
    <div class="row">
      <span class="row__label">Story</span>
      <p class="acts acts--alone">{@render actions()}</p>
    </div>
  {/if}
  {#if f.followUp}
    <div class="row">
      <span class="row__label">Follow-up</span>
      <p class="follow">“{f.followUp}”</p>
    </div>
  {/if}
</article>

<style>
  .card {
    display: grid;
    border-radius: var(--radius-lg);
    border: 1px solid transparent;
    background: var(--surface);
    box-shadow: var(--shadow-border);
    overflow: hidden;
  }
  .row {
    display: grid;
    grid-template-columns: 84px minmax(0, 1fr);
    gap: var(--space-3);
    padding: var(--space-2-5) var(--space-3);
  }
  .row + .row {
    box-shadow: inset 0 1px 0 var(--line);
  }
  .row p {
    margin: 0;
  }
  .row__label {
    padding-top: 2px;
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    color: var(--ink-3);
  }
  .row--warn {
    background: var(--warn-soft);
  }
  .warn {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
    font-weight: var(--weight-medium);
  }
  .warn :global(.icon) {
    flex: none;
    margin-top: 2px;
    color: var(--warn);
  }
  .chips,
  .star {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1-5) var(--space-3);
  }
  .chip {
    display: inline-flex;
    align-items: center;
    height: 22px;
    padding-inline: var(--space-2);
    border-radius: var(--radius-full);
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
    font-size: var(--text-sm);
    color: var(--ink-2);
  }
  .star__part {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
  }
  .star__part :global(.icon) {
    color: var(--gain-text);
  }
  .star__part--short {
    color: var(--ink-3);
  }
  .star__part--short :global(.icon) {
    color: var(--ink-3);
  }
  .claims {
    display: grid;
    gap: var(--space-3);
  }
  .claim {
    display: grid;
    gap: var(--space-1-5);
  }
  .row .acts {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    margin-top: var(--space-1-5);
  }
  .row .acts--alone {
    margin-top: 0;
  }
  .kept {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
  }
  .kept :global(.icon) {
    color: var(--gain-text);
  }
  .tighter,
  .follow {
    font-size: var(--text-md);
    line-height: var(--leading-md);
    text-wrap: pretty;
  }
</style>
