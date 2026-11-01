<!--
  An agent as an account: its icon (a `CompanyMark`) with a presence dot on its bottom-right corner, in the dot's
  tone: gain signed in, warn signed out, danger not installed, neutral not checked or unknown. The icon is cut away 2px
  around the dot, so the gap shows whatever is behind it (vibrancy, a hover wash) rather than a painted ring. 28 (the
  sidebar's foot, Settings' agents): an 8px dot 2px past each edge; 16 (the agent picker): a 6px dot 1px past each
  edge, cut 1.5px around. The dot is decoration: whatever names the agent beside it says its state in words too.
-->
<script lang="ts">
  import CompanyMark from "./CompanyMark.svelte";

  interface Props {
    /** The agent's icon (`app.agentInfo.icon`: `/agents/claude.png`, `openai.png` or `antigravity.png`). */
    src: string;
    tone: "gain" | "warn" | "danger" | "neutral";
    size?: 16 | 28;
  }

  let { src, tone, size = 28 }: Props = $props();
</script>

<span class="agent-mark agent-mark--{size}">
  <CompanyMark {size} {src} />
  <span class="presence presence--{tone}" aria-hidden="true"></span>
</span>

<style>
  .agent-mark {
    position: relative;
    display: inline-flex;
    flex: none;
  }
  /* The dot's radius plus its gap, centered where the dot is (past each edge less its radius). */
  .agent-mark--28 :global(.mark) {
    mask-image: radial-gradient(
      circle at calc(100% - 2px) calc(100% - 2px),
      transparent 6px,
      var(--ink) 6.5px
    );
  }
  .agent-mark--16 :global(.mark) {
    mask-image: radial-gradient(
      circle at calc(100% - 2px) calc(100% - 2px),
      transparent 4.5px,
      var(--ink) 5px
    );
  }
  .presence {
    position: absolute;
    right: -2px;
    bottom: -2px;
    width: 8px;
    height: 8px;
    border-radius: var(--radius-full);
    background: var(--ink-4);
  }
  .agent-mark--16 .presence {
    right: -1px;
    bottom: -1px;
    width: 6px;
    height: 6px;
  }
  .presence--gain {
    background: var(--gain);
  }
  .presence--warn {
    background: var(--warn);
  }
  .presence--danger {
    background: var(--danger);
  }
</style>
