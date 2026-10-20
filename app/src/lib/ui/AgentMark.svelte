<!--
  An agent as an account: its icon (a 28px `CompanyMark`) with a presence dot on its bottom-right corner, in the
  dot's tone: gain signed in, warn signed out, danger not installed, neutral not checked or unknown. The icon is cut
  away 2px around the dot, so the gap shows whatever is behind it (vibrancy, a hover wash) rather than a painted ring.
  The dot is decoration: whatever names the agent beside it says its state in words too.
-->
<script lang="ts">
  import CompanyMark from "./CompanyMark.svelte";

  interface Props {
    /** The agent's icon (`/agents/claude.png`). */
    src: string;
    tone: "gain" | "warn" | "danger" | "neutral";
  }

  let { src, tone }: Props = $props();
</script>

<span class="agent-mark">
  <CompanyMark size={28} {src} />
  <span class="presence presence--{tone}" aria-hidden="true"></span>
</span>

<style>
  .agent-mark {
    position: relative;
    display: inline-flex;
    flex: none;
  }
  /* The dot's 4px radius plus a 2px gap, centered where the dot is (2px past each edge less its radius). */
  .agent-mark :global(.mark) {
    mask-image: radial-gradient(
      circle at calc(100% - 2px) calc(100% - 2px),
      transparent 6px,
      var(--ink) 6.5px
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
