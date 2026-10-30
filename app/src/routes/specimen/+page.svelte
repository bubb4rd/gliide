<!--
  The design system's specimen: the composed Review screen, then every primitive in every state, in the theme
  chosen at the top (Light, Dark, System; a preview that isn't saved). Content is synthetic (Jane Q. Doe).
  Open /specimen?theme=dark to start dark.
-->
<script lang="ts">
  import { untrack } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { page } from "$app/state";
  import Archive from "@lucide/svelte/icons/archive";
  import Check from "@lucide/svelte/icons/check";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import ChevronsDownUp from "@lucide/svelte/icons/chevrons-down-up";
  import ChevronsUpDown from "@lucide/svelte/icons/chevrons-up-down";
  import ClipboardPaste from "@lucide/svelte/icons/clipboard-paste";
  import Download from "@lucide/svelte/icons/download";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import FileUp from "@lucide/svelte/icons/file-up";
  import FileText from "@lucide/svelte/icons/file-text";
  import ListChecks from "@lucide/svelte/icons/list-checks";
  import LayoutDashboard from "@lucide/svelte/icons/layout-dashboard";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Plus from "@lucide/svelte/icons/plus";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Search from "@lucide/svelte/icons/search";
  import SlidersHorizontal from "@lucide/svelte/icons/sliders-horizontal";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import X from "@lucide/svelte/icons/x";
  import { AGENT_LIST, AGENTS, makerIcon } from "$lib/agents";
  import type { Theme } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import { modelName } from "$lib/models";
  import { app } from "$lib/stores/app.svelte";
  import { theme } from "$lib/stores/theme.svelte";
  import {
    AgentMark,
    Backdrop,
    Badge,
    Button,
    Card,
    ChangeRow,
    Checkbox,
    Chip,
    CompanyMark,
    dayKey,
    Desk,
    Empty,
    FileDrop,
    GroupHead,
    Heatmap,
    Icon,
    Input,
    Kbd,
    KeyFooter,
    type KeyCommand,
    LogoDrop,
    Marked,
    Menu,
    Nav,
    NavItem,
    PartRow,
    Popover,
    PlaceMap,
    ScoreBar,
    ScoreHeader,
    SectionNav,
    type NavSection,
    Segmented,
    Sources,
    Steps,
    Table,
    Textarea,
    Thinking,
    Toolbar,
    Tooltip,
    tip,
    type TipContent,
    toast,
    exit,
    WorkingRing,
    type TableColumn,
    type TableSort,
  } from "$lib/ui";
  import { SEARCH_SOURCES_NOTE, searchSources } from "$lib/feed";
  import { RESUME_FORMATS } from "$lib/import";
  import { FITS, LOGO, MCLEAN, OVER, PLANO, RIVERWOODS, WIDE_LOGO } from "./sample";

  // ── Company photo: the job header's mark opens the panel; Paste sets the square logo, Choose file the wide one ──
  let logoOpen = $state(false);
  let logoMark = $state<HTMLElement>();
  let demoLogo = $state<string | null>(null);

  // ── Theme preview ──
  const THEMES: { value: Theme; label: string }[] = [
    { value: "light", label: "Light" },
    { value: "dark", label: "Dark" },
    { value: "system", label: "System" },
  ];
  // `?theme=` previews a theme; it applies again once settings load (they set the saved theme).
  const asked = page.url.searchParams.get("theme");
  $effect(() => {
    void app.settings;
    if (asked === "light" || asked === "dark" || asked === "system") untrack(() => theme.preview(asked));
  });
  let replay = $state(0);
  /** The backdrop sample has scrolled under its toolbar. */
  let backdropScrolled = $state(false);

  // ── Setup steps: the bar in every state, and a sample that turns as the setup's steps do ──
  const SETUP = ["Welcome", "Experience", "What it says", "Agents", "Appearance", "Ready"];
  const TURNS = [
    { title: "Your experience, fitted to every job", body: "The first step enters from the right, its parts 100ms apart." },
    { title: "Bring in your resume", body: "Next: this one leaves to the left and the new one comes in from the right." },
    { title: "What your resume says", body: "Back: the reverse, and the bar’s fill sweeps back out." },
    { title: "You’re set", body: "Reduced motion: the steps only fade, the fill only fades." },
  ];
  let turnAt = $state(0);
  let turnForward = $state(true);
  let turned = $state(false);
  function turnTo(index: number) {
    if (index < 0 || index >= TURNS.length || index === turnAt) return;
    turnForward = index > turnAt;
    turned = true;
    turnAt = index;
  }

  // ── Section nav: Settings' links to its cards, over a sample scroll in each orientation ──
  const JUMPS: (NavSection & { body: string; tall?: boolean })[] = [
    { id: "agent", label: "Agents", body: "Short: a jump lands it at the top." },
    { id: "tailoring", label: "Tailoring", body: "Scroll: each section turns current as its top reaches the top." },
    { id: "feed", label: "Feed", body: "Tall, as Settings’ Feed card.", tall: true },
    { id: "sources", label: "Sources", body: "A part of the one before: indented, regular weight.", sub: true },
    { id: "long", label: "A section with a long name that truncates", body: "Its link truncates in the column." },
    { id: "appearance", label: "Appearance", body: "Last and short: current at the end, or once jumped to." },
  ];
  const jumpsIn = (prefix: string): NavSection[] =>
    JUMPS.map(({ id, label, sub }) => ({ id: `${prefix}-${id}`, label, sub }));
  const JUMPS_COLUMN = jumpsIn("spec-jump-column");
  const JUMPS_ROW = jumpsIn("spec-jump-row");
  let jumpColumn = $state<HTMLElement>();
  let jumpRow = $state<HTMLElement>();

  // ── The composed Review screen: toggles update the score instantly ──
  type State = "in" | "off" | "left-out" | "forced";
  interface DemoChange {
    key: string;
    group: string;
    state: State;
    points: number | null;
    text: string;
    marks: string[];
    label: string;
    was?: string;
    reason?: string;
    warning?: string;
    yours?: boolean;
    meta?: string;
    editable?: boolean;
  }

  const NOW = 53;
  const CEILING = 85;
  let changes = $state<DemoChange[]>([
    {
      key: "acme.1",
      group: "Work · Acme Widgets Co.",
      state: "in",
      points: 9,
      text: "Built REST APIs in a Rust service that batches widget telemetry into Postgres, cutting ingest latency from 900 ms to 120 ms",
      marks: ["REST APIs"],
      was: "Built a Rust service that batches widget telemetry into Postgres, cutting ingest latency from 900 ms to 120 ms across 40 production lines",
      meta: "Rewrite · same 2 lines",
      label: "Include this rewrite",
      editable: true,
    },
    {
      key: "acme.2",
      group: "Work · Acme Widgets Co.",
      state: "left-out",
      points: 3,
      text: "Cut order API response time 40% by caching widget lookups with Redis",
      marks: ["Redis"],
      reason: "“40%” isn’t in your resume",
      label: "Include this rewrite",
      editable: true,
    },
    {
      key: "own.1",
      group: "Work · Acme Widgets Co.",
      state: "in",
      points: 3,
      text: "Add Unit testing to Tools & Technologies",
      marks: ["Unit testing"],
      yours: true,
      label: "Include your edit",
    },
    {
      key: "skills.1",
      group: "Skills",
      state: "in",
      points: 8,
      text: "Add CI/CD to Tools & Technologies",
      marks: ["CI/CD"],
      label: "Include this skill",
    },
    {
      key: "skills.2",
      group: "Skills",
      state: "in",
      points: 5,
      text: "Reword “Code reviews” as Code review",
      marks: ["Code review"],
      warning: "Runs 2 lines, not 1",
      label: "Include this skill",
    },
    {
      key: "bank.1",
      group: "From your bank",
      state: "off",
      points: 4,
      text: "Include Springfield Public Library, IT Assistant, Summer 2023",
      marks: [],
      meta: "Adds Troubleshooting and Ticketing · takes 3 lines",
      label: "Include this entry",
    },
  ]);

  const counted = (c: DemoChange) => c.state === "in" || c.state === "forced";
  const gain = $derived(changes.filter(counted).reduce((s, c) => s + (c.points ?? 0), 0));
  const groups = $derived(
    [...new Set(changes.map((c) => c.group))].map((label) => {
      const rows = changes.filter((c) => c.group === label);
      const pts = rows.filter(counted).reduce((s, c) => s + (c.points ?? 0), 0);
      const noun = label === "From your bank" ? "suggestion" : "change";
      return { label, rows, meta: `${rows.length} ${noun}${rows.length === 1 ? "" : "s"} · +${pts}` };
    }),
  );

  // Folded sections, kept by the specimen itself.
  const folded = new SvelteSet<string>();
  const sections = $derived(["skills", ...groups.map((g) => g.label)]);
  const allFolded = $derived(sections.every((id) => folded.has(id)));
  function fold(ids: string[], on: boolean) {
    for (const id of ids) {
      if (on) folded.add(id);
      else folded.delete(id);
    }
  }
  const foldAll = () => fold(sections, !allFolded);
  // GroupHead states, on their own.
  let headFolded = $state(true);
  let headOpen = $state(false);

  let selected = $state(0);
  let editing = $state<string | null>(null);
  let draft = $state("");
  let noting = $state<string | null>(null);
  let note = $state("");
  let doc = $state<"resume" | "cover">("resume");
  let master = $state("swe");
  let zoom = $state(100);
  let pickerOpen = $state(false);
  let addButton = $state<HTMLButtonElement>();

  const focus = (element: HTMLElement) => element.focus();

  function toggle(change: DemoChange, on: boolean) {
    if (change.state === "left-out" && on) change.state = "forced";
    else if (change.state === "forced" && !on) change.state = "left-out";
    else change.state = on ? "in" : "off";
  }

  function startEdit(change: DemoChange) {
    if (!change.editable) return;
    editing = change.key;
    draft = change.text;
  }

  function saveEdit(change: DemoChange) {
    if (draft.trim() && draft !== change.text) {
      change.was ??= change.text;
      change.text = draft.trim();
      change.yours = true;
    }
    editing = null;
  }

  function addOwn(target: string) {
    pickerOpen = false;
    changes.splice(changes.findIndex((c) => c.group === "Skills"), 0, {
      key: `own.${changes.length}`,
      group: "Work · Acme Widgets Co.",
      state: "in",
      points: 2,
      text: `Add ${target} to Tools & Technologies`,
      marks: [target],
      yours: true,
      label: "Include your edit",
    });
  }

  function exportDemo() {
    toast("Saved globex_jr_sde_doe_resume.pdf · 77% on file", {
      action: { label: "Show in Finder", run: () => {} },
    });
  }

  $effect(() =>
    bindKeys({
      ArrowDown: () => (selected = Math.min(selected + 1, changes.length - 1)),
      ArrowUp: () => (selected = Math.max(selected - 1, 0)),
      " ": () => toggle(changes[selected], !counted(changes[selected])),
      Enter: () => startEdit(changes[selected]),
      a: () => (pickerOpen = true),
      e: exportDemo,
      c: foldAll,
    }),
  );

  const masters = [
    { value: "swe", label: "Software Engineering", meta: "53%" },
    { value: "it", label: "IT Support", meta: "41%" },
    { value: "design", label: "Product Design", meta: "22%" },
  ];
  // Every popover state: met in full (green), partial, none met, and one the posting doesn't ask for.
  const parts = [
    { id: "hard_tool", label: "Hard skills and tools", value: 0.72, weight: 45, present: true, brief: "13 of 18 · missing Kafka, Docker +3" },
    { id: "domain_soft", label: "Domain and soft", value: 0.6, weight: 10, present: true, brief: "3 of 5 · missing collaborate, agile" },
    { id: "title", label: "Job title", value: 1, weight: 10, present: true, brief: "“Software Engineer” stated" },
    { id: "experience", label: "Experience", value: 0, weight: 15, present: true, brief: "0 of 2 years" },
    { id: "education", label: "Education", value: 1, weight: 10, present: true, brief: "Bachelor's in Computer Science" },
    { id: "format", label: "Format", value: 0, weight: 10, present: false, brief: "Not in the posting" },
  ];
  // Every command state: keys only (Select), can't run now (Toggle, Edit), runnable (Add your own, Export).
  const reviewKeys: KeyCommand[] = $derived([
    { keys: ["↑", "↓"], label: "Select" },
    { keys: ["Space"], label: "Toggle", run: () => {}, disabled: true },
    { keys: ["⏎"], label: "Edit", run: () => {}, disabled: true },
    { keys: ["A"], label: "Add your own", run: () => (pickerOpen = true) },
    { keys: ["C"], label: allFolded ? "Expand all" : "Collapse all", run: foldAll },
    { keys: ["E"], label: "Export", run: exportDemo },
  ]);

  // ── States gallery ──
  let seg = $state("light");
  let check = $state({ off: false, on: true });
  let text = $state("");
  let filled = $state("Globex · Junior Software Engineer");
  let longText = $state("");
  let menuValue = $state("applied");
  let popoverOpen = $state(false);
  let popoverAnchor = $state<HTMLButtonElement>();
  let progressRun = $state(0);
  // The working ring: on, then off once "finished"; the stage turns from Extract to Suggest.
  let ringOn = $state(true);
  let ringStage = $state(0);

  const statuses = [
    { value: "saved", label: "Saved" },
    { value: "applied", label: "Applied" },
    { value: "interviewing", label: "Interviewing" },
    { value: "offer", label: "Offer" },
    { value: "rejected", label: "Rejected" },
  ];
  // Amber means "left out" only (DESIGN.md), so no status is warn.
  const statusTone = {
    saved: "neutral",
    applied: "accent",
    interviewing: "accent",
    offer: "gain",
    rejected: "danger",
  } as const;

  interface AppRow {
    id: string;
    date: string;
    company: string;
    role: string;
    now: number;
    projected: number;
    onFile: number | null;
    status: keyof typeof statusTone;
    resume: string;
  }
  const appRows: AppRow[] = [
    { id: "1", date: "2026-10-12", company: "Globex", role: "Junior Software Engineer", now: 53, projected: 78, onFile: 77, status: "applied", resume: "globex_jr_sde_doe_resume.pdf" },
    { id: "2", date: "2026-10-10", company: "Northwind Traders", role: "Support Engineer", now: 61, projected: 71, onFile: 70, status: "interviewing", resume: "northwind_support_engineer_doe_resume.pdf" },
    { id: "3", date: "2026-10-07", company: "Contoso", role: "IT Analyst", now: 48, projected: 59, onFile: null, status: "saved", resume: "contoso_it_analyst_doe_resume.pdf" },
    { id: "4", date: "2026-10-01", company: "Fabrikam", role: "Product Designer", now: 40, projected: 64, onFile: 63, status: "rejected", resume: "fabrikam_product_designer_doe_resume.pdf" },
  ];
  // Every column but Role sorts; Date starts sorted, newest first.
  const appColumns: TableColumn<AppRow>[] = [
    { key: "date", label: "Date", width: "112px", sortBy: (r) => r.date, sortFirst: "desc" },
    { key: "company", label: "Company", width: "18%", sortBy: (r) => r.company },
    { key: "role", label: "Role" },
    { key: "now", label: "Now", numeric: true, width: "72px", sortBy: (r) => r.now },
    { key: "projected", label: "Projected", numeric: true, width: "96px", sortBy: (r) => r.projected },
    { key: "onFile", label: "On file", numeric: true, width: "80px", sortBy: (r) => r.onFile },
    { key: "status", label: "Status", width: "128px", sortBy: (r) => statuses.findIndex((s) => s.value === r.status) },
    { key: "resume", label: "Resume", width: "22%", sortBy: (r) => r.resume },
  ];
  let appSort = $state<TableSort>({ key: "date", dir: "desc" });
  /** The selected row (the Feed's keys act on it): Northwind to start; a click selects another. */
  let appSelected = $state<string | null>("2");
  /** Checked rows (Applications' multi-select): Contoso to start, so the header is mixed; Fabrikam has no checkbox. */
  let appChecked = $state(new Set(["3"]));
  let bulkStatus = $state<string | null>(null);

  // ── Score bar: Applications' Fit column ──
  interface BarSample {
    state: string;
    now?: number | null;
    projected: number;
    onFile?: number | null;
    ceiling?: number | null;
  }
  const barSamples: BarSample[] = [
    { state: "Tailored, under its ceiling: 69 → 88, ceiling 92", now: 69, projected: 88, onFile: 88, ceiling: 92 },
    { state: "Small gain: 80 → 82", now: 80, projected: 82, onFile: 82, ceiling: 84 },
    { state: "At its ceiling: 64 → 85", now: 64, projected: 85, ceiling: 85 },
    { state: "Gain, then lost in the PDF: 60 → 80, reads 70", now: 60, projected: 80, onFile: 70, ceiling: 86 },
    { state: "Lost in the PDF, imported (no before): 76, reads 47", projected: 76, onFile: 47 },
    { state: "No edits in: 72 → 72, ceiling 79", now: 72, projected: 72, ceiling: 79 },
    { state: "Not sent: its fit only", projected: 60 },
    { state: "Edges: 0", projected: 0 },
    { state: "Edges: 90 → 100", now: 90, projected: 100, onFile: 100, ceiling: 100 },
  ];
  interface BarRow extends Omit<BarSample, "state"> {
    id: string;
    company: string;
    role: string;
  }
  const barRows: BarRow[] = [
    { id: "1", company: "Initech", role: "Software Engineer, New Grad", now: 80, projected: 82, onFile: 82, ceiling: 84 },
    { id: "2", company: "OneImaging", role: "Full Stack Associate Software Engineer", now: 69, projected: 88, onFile: 88 },
    { id: "3", company: "Amazon", role: "Front-End Engineer", now: 69, projected: 72, onFile: 72 },
    { id: "4", company: "Whatnot", role: "Software Engineer, 2027 New Grad", projected: 60 },
    { id: "5", company: "Together AI", role: "Software Engineer, New Grad", projected: 76, onFile: 47 },
    { id: "6", company: "BAE Systems", role: "Entry Level Software Engineer", projected: 88, onFile: 73 },
    { id: "7", company: "ID.me", role: "Software Development Engineer II", projected: 90, onFile: 90 },
  ];
  const barColumns: TableColumn<BarRow>[] = [
    { key: "company", label: "Company", width: "28%", sortBy: (r) => r.company },
    { key: "role", label: "Role" },
    { key: "fit", label: "Fit", numeric: true, width: "176px", sortBy: (r) => r.onFile ?? r.projected },
  ];
  let barSort = $state<TableSort>({ key: "fit", dir: "desc" });

  // ── Heatmap: nine weeks of made-up activity ending on a fixed day, so the specimen never shifts ──
  const heatEnd = new Date(2026, 9, 13);
  const heatBusy = new Map<string, number>();
  for (let i = 0, seed = 7; i < 63; i++) {
    seed = (seed * 48271) % 2147483647;
    const n = (seed % 7) - 2;
    if (n > 0) heatBusy.set(dayKey(new Date(2026, 9, 13 - i)), n);
  }
  const heatNone = new Map<string, number>();

  // ── Tooltips: every kind, from a label to a chart's legend ──
  const tipKinds: { name: string; content: TipContent }[] = [
    { name: "Label", content: "Delete posting" },
    { name: "Sentence", content: "Kept on by you: the fit check would leave it out, as “40%” isn’t in your resume" },
    { name: "Value", content: { value: "78%" } },
    { name: "Title and value", content: { title: "Fit", value: 78 } },
    {
      name: "Rows",
      content: {
        title: "Tue, Oct 13",
        rows: [
          { label: "Postings added", value: 2 },
          { label: "Applications sent", value: 1 },
        ],
      },
    },
    {
      name: "Rows with legend",
      content: {
        title: "Fit",
        value: 88,
        rows: [
          { key: "base", label: "Before your edits", value: 69 },
          { key: "gain", label: "From your edits", value: "+19", tone: "gain" },
          { key: "tick", label: "Ceiling", value: 90 },
        ],
        note: "Read back from the exported PDF",
      },
    },
    {
      name: "Lost in the PDF",
      content: {
        title: "Fit",
        value: 73,
        tone: "warn",
        rows: [{ key: "lost", label: "Lost in the PDF", value: "−15", tone: "warn" }],
        note: "Read back from the exported PDF",
      },
    },
    {
      name: "Named lists (keyword meter)",
      content: {
        title: "Keywords",
        value: "8 of 14",
        groups: [
          { label: "On your resume", key: "have", items: ["TypeScript", "React", "Node.js", "PostgreSQL", "Git", "REST APIs"] },
          { label: "Added by your edits", key: "add", items: ["CI/CD", "SQL"] },
          { label: "Missing", key: "missing", tone: "warn", items: ["Objective-C", "scripting language", "AWS", "GCP", "Bash", "Kubernetes"] },
        ],
      },
    },
  ];

  const swatches = [
    ["Surfaces", [["Paper", "--paper"], ["Surface", "--surface"], ["Sunken", "--sunken"], ["Desk", "--desk"], ["Sidebar", "--sidebar"]]],
    ["Ink", [["Ink · 16.6:1", "--ink"], ["Ink 2 · 7.4:1", "--ink-2"], ["Ink 3 · ≥4.6:1", "--ink-3"], ["Ink 4 · disabled", "--ink-4"]]],
    ["Meaning", [["Accent · act, focus", "--accent"], ["Gain · points, have", "--gain"], ["Warn · left out", "--warn"], ["Danger · cut, error", "--danger"], ["Mark · inserted words", "--mark"]]],
  ] as const;
</script>

<Toolbar>
  {#snippet crumbs()}<b>Specimen</b><span>· Paper &amp; Ink</span>{/snippet}
  <Button variant="ghost" icon={RotateCcw} onclick={() => replay++}>Replay entrances</Button>
  <hr />
  <Segmented options={THEMES} value={theme.current} label="Theme" onchange={(t) => theme.preview(t)} />
</Toolbar>

<div class="scroll">
  <div class="spec">
    <header class="spec__head">
      <p class="label">gliide design system</p>
      <h1 class="headline">Paper &amp; Ink</h1>
      <p class="meta">
        Warm paper, graphite ink, one cobalt accent. A serif for the numbers and titles you read; Inter for everything
        you operate. The resume page is the only object with depth. Every component below is the one screens use, from
        <code>$lib/ui</code>.
      </p>
    </header>

    <!-- ═══════════ Composed: the Review screen ═══════════ -->
    <section class="section">
      <h2 class="title">Review</h2>
      <p class="meta lede">
        Toggle a checkbox (or ↑↓ and Space): the projected score and bar update instantly, without animation. ⏎ edits
        the selected line, A adds your own, E exports.
      </p>

      <div class="frame">
        <aside class="frame__sidebar">
          <div class="lights" aria-hidden="true"><i></i><i></i><i></i></div>
          <Nav label="Main (specimen)">
            <NavItem href="#review" icon={LayoutDashboard}>Overview</NavItem>
            <NavItem href="#review" icon={ListChecks} meta={53}>Applications</NavItem>
            <NavItem href="#review" icon={Archive}>Experience</NavItem>
            <NavItem href="#review" icon={FileText}>Resumes</NavItem>
          </Nav>
          <div class="frame__recent">
            <p class="label">Recent</p>
            <Nav label="Recent (specimen)">
              <NavItem
                href="#review"
                quiet
                current
                sub="Junior Software Engineer"
                status={{ label: "Applied", tone: "accent" }}
              >
                {#snippet lead()}<CompanyMark size={28} />{/snippet}
                {#snippet trailing()}<ScoreBar bar={false} now={53} projected={78} onFile={78} />{/snippet}
                Globex
              </NavItem>
              <NavItem href="#review" quiet sub="Support Engineer" status={{ label: "Interviewing", tone: "accent" }}>
                {#snippet lead()}<CompanyMark size={28} />{/snippet}
                {#snippet trailing()}<ScoreBar bar={false} now={64} projected={74} onFile={71} />{/snippet}
                Northwind Traders
              </NavItem>
              <NavItem href="#review" quiet sub="Product Designer" status={{ label: "Saved", tone: "neutral" }}>
                {#snippet lead()}<CompanyMark size={28} />{/snippet}
                {#snippet trailing()}<ScoreBar bar={false} projected={64} />{/snippet}
                Fabrikam
              </NavItem>
              <NavItem href="#review" quiet sub="IT Analyst">
                {#snippet lead()}<CompanyMark size={28} />{/snippet}
                {#snippet trailing()}<ScoreBar bar={false} projected={59} />{/snippet}
                Contoso
              </NavItem>
              <NavItem
                href="#review"
                quiet
                sub="Software Engineer, New Grads"
                status={{ label: "Saved", tone: "neutral" }}
                busy="Suggesting…"
              >
                {#snippet lead()}<CompanyMark size={28} />{/snippet}
                {#snippet trailing()}<ScoreBar bar={false} projected={61} />{/snippet}
                Litware
              </NavItem>
            </Nav>
          </div>
          <div>
            <Nav label="App (specimen)">
              <NavItem href="#review" icon={SlidersHorizontal}>Settings</NavItem>
            </Nav>
            <NavItem href="#review" sub="Signed in · Max">
              {#snippet lead()}<AgentMark src="/agents/claude.png" tone="gain" />{/snippet}
              Claude Code
            </NavItem>
          </div>
        </aside>

        <div class="frame__main">
          <Toolbar>
            {#snippet crumbs()}
              <a href="#review">Applications</a><Icon icon={ChevronRight} size={14} weight="medium" /><b>Globex</b>
              <Menu bind:value={menuValue} options={statuses} label="Status" variant="ghost" size="sm">
                {#snippet trigger(current)}
                  <Badge tone={statusTone[menuValue as keyof typeof statusTone]}>{current?.label}</Badge>
                {/snippet}
              </Menu>
              {#if menuValue === "saved"}
                <Button variant="ghost" size="sm" icon={Check} onclick={() => (menuValue = "applied")}>
                  Mark applied
                </Button>
              {/if}
            {/snippet}
            <Segmented
              options={[
                { value: "resume", label: "Resume" },
                { value: "cover", label: "Cover letter" },
              ]}
              bind:value={doc}
              label="Document"
            />
            <Menu bind:value={master} options={masters} label="Master" placement="bottom-end" />
            <hr />
            <Button variant="primary" icon={Download} kbd="E" onclick={exportDemo}>Export</Button>
          </Toolbar>

          <div class="split">
            <div class="column">
              {#key replay}
                <div class="job enter" style:--i="0">
                  <div class="row">
                    <Badge tone="accent">Tailor first</Badge>
                    <span class="meta">Remote, US · read from globex.com</span>
                  </div>
                  <div class="named">
                    <span class="lead"><CompanyMark size={48} /></span>
                    <p class="company">Globex</p>
                    <h1 class="headline">Junior Software Engineer</h1>
                  </div>
                </div>

                <div class="enter" style:--i="1">
                  <ScoreHeader
                    now={NOW}
                    projected={NOW + gain}
                    ceiling={CEILING}
                    components={parts}
                    nowTitle="Your Software Engineering master as it is"
                  >
                    <strong>+{gain} points</strong> from {changes.filter(counted).length} changes · fits on 1 page
                  </ScoreHeader>
                </div>

                <div class="fold-all enter" style:--i="2">
                  <Button
                    variant="ghost"
                    size="sm"
                    icon={allFolded ? ChevronsUpDown : ChevronsDownUp}
                    kbd="C"
                    onclick={foldAll}>{allFolded ? "Expand all" : "Collapse all"}</Button
                  >
                </div>

                <div class="skills enter" style:--i="2">
                  <GroupHead
                    label="Skills and keywords"
                    meta="7 of 10 on the page"
                    collapsed={folded.has("skills")}
                    controls="spec-fold-skills"
                    ontoggle={(c) => fold(["skills"], c)}
                  />
                  <div class="fold" id="spec-fold-skills">
                    {#if !folded.has("skills")}
                      <div class="row chips">
                        <Chip state="have" p1>React</Chip>
                        <Chip state="have" p1>TypeScript</Chip>
                        <Chip state="add" p1>REST APIs</Chip>
                        <Chip state="add">CI/CD</Chip>
                        <Chip state="have">Postgres</Chip>
                        <Chip state="missing" p1>SQL Server</Chip>
                        <Chip state="add">Code review</Chip>
                        <Chip state="missing">Kafka</Chip>
                        <Chip state="have">Git</Chip>
                        <Chip state="not-mine">.NET</Chip>
                        <Chip state="on">Impact</Chip>
                        <Chip state="off">Conflict</Chip>
                      </div>
                    {/if}
                  </div>
                </div>

                <div class="changes enter" style:--i="3">
                  {#each groups as group, g (group.label)}
                    <GroupHead
                      label={group.label}
                      meta={group.meta}
                      collapsed={folded.has(group.label)}
                      controls="spec-fold-{g}"
                      ontoggle={(c) => fold([group.label], c)}
                    />
                    <div class="fold" id="spec-fold-{g}">
                      {#if !folded.has(group.label)}
                        {#each group.rows as change (change.key)}
                          {@const index = changes.indexOf(change)}
                          {#snippet editor()}
                            <Textarea
                              size="lg"
                              bind:value={draft}
                              aria-label="Your wording"
                              {@attach focus}
                              onkeydown={(e) => {
                                if (e.key === "Escape") editing = null;
                                if (e.key === "Enter" && e.metaKey) saveEdit(change);
                              }}
                            />
                          {/snippet}
                          <ChangeRow
                            state={change.state}
                            checked={counted(change)}
                            selected={index === selected}
                            points={change.points}
                            label={change.label}
                            ontoggle={(on) => toggle(change, on)}
                            onselect={() => (selected = index)}
                            was={change.was}
                            reason={change.reason}
                            warning={change.warning}
                            yours={change.yours}
                            meta={change.meta}
                            editor={editing === change.key ? editor : undefined}
                          >
                            {#snippet text()}<Marked text={change.text} marks={change.marks} />{/snippet}
                            {#snippet actions()}
                              {#if change.editable}
                                <Button variant="ghost" size="sm" icon={Pencil} kbd="⏎" onclick={() => startEdit(change)}
                                  >Edit</Button
                                >
                                <Button
                                  variant="ghost"
                                  size="sm"
                                  icon={Sparkles}
                                  onclick={() => ((noting = change.key), (note = ""))}>Rewrite with a note</Button
                                >
                              {/if}
                              {#if change.yours}
                                <Button
                                  variant="ghost"
                                  size="sm"
                                  square
                                  icon={X}
                                  aria-label="Remove your edit"
                                  onclick={() => changes.splice(changes.indexOf(change), 1)}
                                />
                              {/if}
                            {/snippet}
                          </ChangeRow>
                          {#if noting === change.key}
                            <form
                              class="note"
                              onsubmit={(e) => {
                                e.preventDefault();
                                noting = null;
                                toast("Rewrote the line from your note", { tone: "gain" });
                              }}
                            >
                              <Input
                                size="md"
                                bind:value={note}
                                placeholder="What should change? “Shorter”, “mention Kafka”…"
                                aria-label="Note for the rewrite"
                                {@attach focus}
                                onkeydown={(e) => e.key === "Escape" && (noting = null)}
                              />
                              <Button type="submit" variant="primary" size="sm" disabled={!note.trim()}>Rewrite</Button>
                            </form>
                          {/if}
                        {/each}
                      {/if}
                    </div>
                  {/each}
                </div>
              {/key}

              <div class="picker-anchor">
                <Button bind:element={addButton} variant="ghost" size="sm" icon={Plus} kbd="A" onclick={() => (pickerOpen = true)}
                  >Add your own</Button
                >
              </div>
              <Popover bind:open={pickerOpen} anchor={addButton} aria-label="Add your own">
                <div class="picker">
                  <p class="label">Add a skill to Tools &amp; Technologies</p>
                  {#each ["SQL Server", "Kafka", "Docker"] as target (target)}
                    <Button variant="ghost" onclick={() => addOwn(target)}>{target}</Button>
                  {/each}
                </div>
              </Popover>

              <div class="keys-wrap"><KeyFooter keys={reviewKeys} /></div>
            </div>

            <Desk pages={FITS} bind:zoom>
              {#snippet status()}<Badge tone="gain" dot>Fits on 1 page</Badge>{/snippet}
              Highlights show what changed
            </Desk>
          </div>
        </div>
      </div>
    </section>

    <!-- ═══════════ Tokens ═══════════ -->
    <section class="section">
      <h2 class="title">Color</h2>
      <p class="meta lede">
        OKLCH, one warm graphite hue, one cobalt accent. Contrast measured against paper, surface and sunken; ink 4 is
        never readable text.
      </p>
      <div class="grid cols-3">
        {#each swatches as [group, items] (group)}
          <Card pad>
            <div class="tile">
              <p class="label">{group}</p>
              {#each items as [name, token] (token)}
                <div class="swatch">
                  <i style:background="var({token})"></i><b>{name}</b><code>{token}</code>
                </div>
              {/each}
            </div>
          </Card>
        {/each}
      </div>
    </section>

    <section class="section">
      <h2 class="title">Type</h2>
      <p class="meta lede">
        Source Serif 4 for what you read at a glance: titles, the score. Inter for what you operate. Numbers are
        tabular everywhere they change.
      </p>
      <Card pad>
        <div class="ramp">
          <div><code>--text-score · serif</code><span class="score-sample num">78<small>%</small></span></div>
          <div><code>--text-headline · serif</code><span class="headline">Junior Software Engineer</span></div>
          <div><code>--text-title · serif</code><span class="title">Experience</span></div>
          <div><code>--text-lg · 16/24</code><span class="lg">Section heading inside a screen</span></div>
          <div><code>--text-md · 14/22</code><span class="md">Resume lines you read and edit: bullets, rewrites, notes.</span></div>
          <div><code>--text-base · 13/20</code><span>Controls, labels, menus, everything you operate.</span></div>
          <div><code>--text-sm · 12/16</code><span class="meta">Metadata, reasons, counts.</span></div>
          <div><code>label · 11 caps</code><span class="label">Group label</span></div>
        </div>
      </Card>
    </section>

    <!-- ═══════════ Controls ═══════════ -->
    <section class="section">
      <h2 class="title">Controls</h2>
      <div class="grid cols-2">
        <Card pad>
          <div class="tile">
            <p class="label">Button · variants</p>
            <div class="row">
              <Button variant="primary" icon={Download} kbd="E">Export</Button>
              <Button>Rebuild</Button>
              <Button iconEnd={ChevronRight}>Software Engineering</Button>
              <Button variant="ghost">Cancel</Button>
              <Button variant="danger">Remove</Button>
            </div>
            <p class="label">Disabled</p>
            <div class="row">
              <Button variant="primary" kbd="E" disabled>Export</Button>
              <Button disabled>Rebuild</Button>
              <Button variant="ghost" disabled>Cancel</Button>
              <Button variant="danger" disabled>Remove</Button>
            </div>
            <p class="label">Sizes · square · static</p>
            <div class="row">
              <Button variant="primary" size="lg" kbd="V">Paste from clipboard</Button>
              <Button size="sm" icon={Plus}>Small</Button>
              <Button square icon={Search} aria-label="Search" />
              <Button size="sm" variant="ghost" square icon={X} aria-label="Close" />
              <Button size="lg" square icon={Search} aria-label="Search" />
              <Button static>No press scale</Button>
            </div>
          </div>
        </Card>

        <Card pad>
          <div class="tile">
            <p class="label">Segmented</p>
            <div class="row">
              <Segmented
                options={[
                  { value: "light", label: "Light" },
                  { value: "dark", label: "Dark" },
                  { value: "system", label: "System" },
                ]}
                bind:value={seg}
                label="Example choice"
              />
              <Segmented options={[{ value: 1, label: "1 page" }, { value: 2, label: "2 pages" }]} value={1} label="Page limit" disabled />
            </div>
            <p class="label">Badge · tones, dot</p>
            <div class="row">
              <Badge tone="accent">Tailor first</Badge>
              <Badge tone="gain" dot>Apply</Badge>
              <Badge tone="warn" dot>Stretch</Badge>
              <Badge tone="danger" dot>Rejected</Badge>
              <Badge>Skip</Badge>
              <Badge dot>Saved</Badge>
            </div>
            <p class="label">Kbd</p>
            <div class="row"><Kbd>⌘</Kbd><Kbd>K</Kbd><Kbd>Space</Kbd><Kbd>⏎</Kbd><Kbd>↑</Kbd></div>
          </div>
        </Card>

        <Card pad>
          <div class="tile">
            <p class="label">Chip · have · add · missing · not yours</p>
            <div class="row">
              <Chip state="have">React</Chip>
              <Chip state="add">CI/CD</Chip>
              <Chip state="missing">Kafka</Chip>
              <Chip state="not-mine">.NET</Chip>
            </div>
            <p class="label">Priority 1 · disabled (only stated in bullets)</p>
            <div class="row">
              <Chip state="have" p1>TypeScript</Chip>
              <Chip state="add" p1>REST APIs</Chip>
              <Chip state="missing" p1>SQL Server</Chip>
              <Chip state="have" disabled {@attach tip("Only stated in your bullets")}>Agile</Chip>
            </div>
            <p class="label">Checkbox · off · on · mixed · disabled</p>
            <div class="row">
              <Checkbox bind:checked={check.off} label="Off example" />
              <Checkbox bind:checked={check.on} label="On example" />
              <Checkbox checked={false} mixed label="Mixed example" />
              <Checkbox checked={false} label="Disabled off" disabled />
              <Checkbox checked label="Disabled on" disabled />
            </div>
          </div>
        </Card>

        <Card pad>
          <div class="tile">
            <p class="label">Input · lg · md · sm · disabled</p>
            <Input bind:value={text} placeholder="Paste a job link or the posting’s text" />
            <div class="row nowrap">
              <Input size="md" bind:value={filled} aria-label="Filled" />
              <Input size="sm" placeholder="Small" aria-label="Small" />
            </div>
            <Input value="Can’t change while exporting" disabled aria-label="Disabled" />
            <p class="label">Textarea · md · lg (reading size) · disabled</p>
            <Textarea bind:value={longText} placeholder="What should change? “Shorter”, “mention Kafka”…" />
            <Textarea size="lg" value="Built REST APIs in a Rust service that batches widget telemetry into Postgres" aria-label="Reading size" />
            <Textarea value="Disabled" disabled aria-label="Disabled textarea" />
          </div>
        </Card>
      </div>
    </section>

    <!-- ═══════════ Change rows ═══════════ -->
    <section class="section">
      <h2 class="title">Change rows</h2>
      <p class="meta lede">
        Every state of one suggestion. Actions show on hover, focus or selection; reasons say why it was left out;
        your own edits say so.
      </p>
      <Card>
        <div class="rows">
          <GroupHead label="Work · Acme Widgets Co." meta="7 changes · +21" />
          {#snippet line()}<Marked text="Built REST APIs and webhooks for the order API" marks={["REST APIs"]} />{/snippet}
          {#snippet acts()}
            <Button variant="ghost" size="sm" icon={Pencil} kbd="⏎">Edit</Button>
            <Button variant="ghost" size="sm" icon={Sparkles}>Rewrite with a note</Button>
          {/snippet}
          <ChangeRow state="in" checked points={9} label="In" ontoggle={() => {}} text={line} meta="In · Rewrite · same 2 lines" was="Built order API integrations" actions={acts} />
          <ChangeRow state="in" checked selected points={9} label="Selected" ontoggle={() => {}} text={line} meta="Selected: actions show" actions={acts} />
          <ChangeRow state="off" checked={false} points={4} label="Off" ontoggle={() => {}} text={line} meta="Off: you turned it off" />
          <ChangeRow state="left-out" checked={false} points={3} label="Left out" ontoggle={() => {}} text={line} reason="“40%” isn’t in your resume" />
          <ChangeRow state="forced" checked points={3} label="Forced" ontoggle={() => {}} text={line} reason="“40%” isn’t in your resume" warning="Runs 3 lines, not 2" />
          <ChangeRow state="in" checked points={2} label="Yours" ontoggle={() => {}} text={line} yours meta="Your wording" />
          <ChangeRow state="in" checked points={null} label="No points alone" ontoggle={() => {}} text={line} meta="Moves with the reorder: no points alone" />
          {#snippet editingSnippet()}
            <Textarea size="lg" value="Built REST APIs and webhooks for the order API" aria-label="Your wording" />
          {/snippet}
          <ChangeRow state="in" checked selected points={9} label="Editing" ontoggle={() => {}} text={line} editor={editingSnippet} meta="Editing: ⌘⏎ saves, Esc cancels" />
        </div>
      </Card>
    </section>

    <!-- ═══════════ Group heads ═══════════ -->
    <section class="section">
      <h2 class="title">Group heads</h2>
      <p class="meta lede">
        Plain (a letter's parts), or a toggle that folds its section: the whole head is the button, the chevron after
        the label points down while open and right while folded, the meta stays. Hover lifts the label to ink 2.
      </p>
      <Card>
        <div class="rows">
          <GroupHead label="Letter" meta="312 words · 4 paragraphs" />
          <GroupHead label="Work · Acme Widgets Co." meta="3 changes · +12" bind:collapsed={headOpen} controls="spec-head-open" />
          <div id="spec-head-open">{#if !headOpen}<p class="meta demo-body">The group's rows.</p>{/if}</div>
          <GroupHead label="Other lines" meta="6 lines as on the master" bind:collapsed={headFolded} controls="spec-head-folded" />
          <div id="spec-head-folded">{#if !headFolded}<p class="meta demo-body">The group's rows.</p>{/if}</div>
        </div>
      </Card>
    </section>

    <!-- ═══════════ Part rows ═══════════ -->
    <section class="section">
      <h2 class="title">Part rows</h2>
      <p class="meta lede">
        The parts of a cover letter, edited in place. Like change rows without the checkbox: actions show on hover,
        focus or selection; a part being rewritten stays, dimmed, under its progress.
      </p>
      <Card>
        <div class="rows">
          <GroupHead label="Letter" meta="312 words · 4 paragraphs" />
          {#snippet greeting()}Dear Acme Hiring Team,{/snippet}
          {#snippet para()}At Acme Widgets I built the REST APIs and webhooks behind the order API, and cut its p95
            latency from 900 ms to 240 ms with a Redis cache.{/snippet}
          {#snippet partActs()}
            <Button variant="ghost" size="sm" icon={Pencil} kbd="⏎">Edit</Button>
          {/snippet}
          {#snippet paraActs()}
            <Button variant="ghost" size="sm" icon={Pencil} kbd="⏎">Edit</Button>
            <Button variant="ghost" size="sm" icon={Sparkles} kbd="N">Rewrite with a note</Button>
          {/snippet}
          <PartRow label="Greeting" text={greeting} actions={partActs} />
          <PartRow label="Paragraph 2 · selected: actions show" selected text={para} actions={paraActs} />
          <PartRow label="Paragraph 3 · being rewritten" busy text={para} />
          <div class="part-progress">
            <Thinking label="Rewriting the paragraph…" oncancel={() => {}} />
          </div>
          {#snippet partEditor()}
            <Textarea
              size="lg"
              value="At Acme Widgets I built the REST APIs and webhooks behind the order API."
              aria-label="Your wording"
            />
          {/snippet}
          <PartRow label="Paragraph 4 · editing: ⏎ saves, Esc cancels" selected text={para} editor={partEditor} />
        </div>
      </Card>
    </section>

    <!-- ═══════════ Score ═══════════ -->
    <section class="section">
      <h2 class="title">Score</h2>
      <div class="grid cols-2">
        <Card pad>
          <ScoreHeader now={53} projected={78} ceiling={85} components={parts} nowTitle="Your master as it is">
            <strong>+25 points</strong> from 4 changes · hover or focus the numbers for the parts
          </ScoreHeader>
        </Card>
        <Card pad>
          <ScoreHeader now={64} projected={64} ceiling={70}>No changes in yet: no gain, no popover</ScoreHeader>
        </Card>
      </div>
    </section>

    <!-- ═══════════ Score bar ═══════════ -->
    <section class="section">
      <h2 class="title">Score bar</h2>
      <p class="meta lede">
        Applications’ Fit column: the review’s score bar in small, from 0 to 100, before the score that counts (on file
        once exported, else projected). Ink 3 is what the master had, green what your edits added, amber what the PDF
        lost; a tick marks the ceiling. Hover a bar for the numbers. The number alone (`bar={false}`), styled the
        same with its percent sign: beside a name, in the sidebar’s Recent and the Overview’s lists.
      </p>
      <Card pad>
        <div class="bars">
          {#each barSamples as b, i (i)}
            <p class="meta">{b.state}</p>
            <ScoreBar now={b.now} projected={b.projected} onFile={b.onFile} ceiling={b.ceiling} />
          {/each}
        </div>
      </Card>
      <Card pad>
        <div class="bars bars--alone">
          {#each barSamples as b, i (i)}
            <p class="meta">{b.state}</p>
            <ScoreBar bar={false} now={b.now} projected={b.projected} onFile={b.onFile} ceiling={b.ceiling} />
          {/each}
        </div>
      </Card>
      <Card>
        <div class="table">
          <Table columns={barColumns} rows={barRows} bind:sort={barSort} key={(r) => r.id} label="Score bar (specimen)">
            {#snippet cell(row, column)}
              {#if column.key === "fit"}
                <ScoreBar now={row.now} projected={row.projected} onFile={row.onFile} ceiling={row.ceiling} />
              {:else if column.key === "company"}
                <CompanyMark>{row.company}</CompanyMark>
              {:else}
                {row.role}
              {/if}
            {/snippet}
          </Table>
        </div>
      </Card>
    </section>

    <!-- ═══════════ New primitives ═══════════ -->
    <section class="section">
      <h2 class="title">Menus, popovers, progress, company marks, sources</h2>
      <div class="grid cols-2">
        <Card pad>
          <div class="tile">
            <p class="label">Menu · default · ghost small · disabled</p>
            <div class="row">
              <Menu bind:value={master} options={masters} label="Master" />
              <Menu bind:value={menuValue} options={statuses} label="Status" variant="ghost" size="sm" />
              <Menu bind:value={menuValue} options={statuses} label="Status" variant="ghost" size="sm">
                {#snippet trigger(current)}
                  <Badge tone={statusTone[menuValue as keyof typeof statusTone]}>{current?.label}</Badge>
                {/snippet}
              </Menu>
              <Menu value="swe" options={masters} label="Master (disabled)" disabled />
              <Menu bind:value={bulkStatus} options={statuses} label="Set status" kbd="S">
                {#snippet trigger()}Set status{/snippet}
              </Menu>
            </div>
            <p class="meta">↑↓ or click opens; ↑↓ Home End move; ⏎ chooses; Esc closes.</p>
            <p class="label">Popover</p>
            <div class="row">
              <Button bind:element={popoverAnchor} onclick={() => (popoverOpen = !popoverOpen)}>Show popover</Button>
            </div>
            <Popover bind:open={popoverOpen} anchor={popoverAnchor}>
              <div class="picker">
                <p class="label">Anchored panel</p>
                <p class="meta">Esc or a click outside closes it.</p>
              </div>
            </Popover>
          </div>
        </Card>
        <Card pad>
          <div class="tile">
            <p class="label">Thinking · with steps done · with Cancel · bare</p>
            {#key progressRun}
              <Thinking
                label="Suggesting edits…"
                steps={[{ label: "Read the requirements", ms: 6200 }]}
                oncancel={() => toast("Cancelled", { tone: "neutral" })}
              />
              <Thinking label="Reading the requirements…" oncancel={() => toast("Cancelled", { tone: "neutral" })} />
              <Thinking label="Copying your resume…" />
            {/key}
            <div class="row"><Button size="sm" icon={RotateCcw} onclick={() => progressRun++}>Restart</Button></div>
          </div>
        </Card>
        <Card pad>
          <div class="tile">
            <p class="label">
              Working ring · a pane being written · centered Thinking · stage turning · finished (fades out) · reduced
              motion: a still outline, breathing
            </p>
            <div class="working-pane">
              {#if ringOn}
                {#key progressRun}
                  <Thinking
                    center
                    label={ringStage ? "Suggesting edits…" : "Reading the requirements…"}
                    steps={ringStage ? [{ label: "Read the requirements", ms: 4200 }] : []}
                    oncancel={() => (ringOn = false)}
                  />
                {/key}
              {/if}
              <WorkingRing on={ringOn} />
            </div>
            <div class="row">
              <Button size="sm" disabled={!ringOn || ringStage === 1} onclick={() => (ringStage = 1)}>Next stage</Button>
              <Button size="sm" disabled={!ringOn} onclick={() => (ringOn = false)}>Finish</Button>
              <Button
                size="sm"
                icon={RotateCcw}
                onclick={() => {
                  ringStage = 0;
                  ringOn = true;
                  progressRun++;
                }}>Restart</Button
              >
            </div>
          </div>
        </Card>
        <Card pad>
          <div class="tile">
            <p class="label">Company mark · 16 sidebar · 20 beside a name · 28 two lines · 48 job header · with its name</p>
            <div class="row">
              <CompanyMark size={16} />
              <CompanyMark size={20} />
              <CompanyMark size={28} />
              <CompanyMark size={48} />
            </div>
            <div class="row">
              <CompanyMark size={16} src={LOGO} />
              <CompanyMark size={20} src={LOGO} />
              <CompanyMark size={28} src={LOGO} />
              <CompanyMark size={48} src={LOGO} />
              <CompanyMark size={48} src={WIDE_LOGO} />
            </div>
            <CompanyMark>Northwind Traders, a company name long enough that it truncates in a narrow cell</CompanyMark>
            <CompanyMark src={LOGO}>Northwind Traders, with its logo</CompanyMark>
            <p class="meta">Blank until you set a logo. A logo shows as the company publishes it: scaled to fit, rounded like the tile, never recolored.</p>
            <p class="label">Agents (Settings → Agents, the setup's Agents step) · Claude Code · Codex · Antigravity</p>
            <div class="row">
              {#each AGENT_LIST as agent (agent.id)}<CompanyMark size={28} src={agent.icon} />{/each}
            </div>
            <p class="label">A model's maker (who answered in a chat or a dig) · Claude · GPT · Gemini · another, on Antigravity: the agent's icon</p>
            <div class="row">
              {#each ["claude-sonnet-5-5-20261001", "gpt-6-luna", "gemini-3.8-flash-medium", "kimi-k2"] as model (model)}
                <CompanyMark size={20} src={makerIcon(model) ?? AGENTS.antigravity.icon}>{modelName(model)}</CompanyMark>
              {/each}
            </div>
            <p class="meta">Each shows as its maker publishes it; none is ever grayed.</p>
          </div>
        </Card>
        <Card pad>
          <div class="tile">
            <p class="label">Agent mark · the sidebar's foot · signed in · signed out · not installed · not checked or unknown</p>
            {#each AGENT_LIST as agent (agent.id)}
              <div class="row">
                <AgentMark src={agent.icon} tone="gain" />
                <AgentMark src={agent.icon} tone="warn" />
                <AgentMark src={agent.icon} tone="danger" />
                <AgentMark src={agent.icon} tone="neutral" />
              </div>
            {/each}
            <Nav label="Agent states (specimen)">
              {#each [{ agent: AGENTS.claude, tone: "gain", sub: "Signed in · Max" }, { agent: AGENTS.codex, tone: "gain", sub: "Signed in · ChatGPT" }, { agent: AGENTS.antigravity, tone: "gain", sub: "Signed in" }, { agent: AGENTS.codex, tone: "warn", sub: "Signed out" }, { agent: AGENTS.antigravity, tone: "danger", sub: "Not installed" }, { agent: AGENTS.claude, tone: "neutral", sub: "Couldn't check sign-in" }] as const as state (state.agent.id + state.sub)}
                <NavItem href="#review" sub={state.sub}>
                  {#snippet lead()}<AgentMark src={state.agent.icon} tone={state.tone} />{/snippet}
                  {state.agent.name}
                </NavItem>
              {/each}
            </Nav>
            <p class="meta">The agent as the account you’re signed in to: a presence dot cut into its icon’s corner, its state in words under its name.</p>
          </div>
        </Card>
        <Card pad>
          <div class="tile">
            <p class="label">Company mark as a button · blank (hover or focus: the icon) · set · files over it</p>
            <div class="row">
              <CompanyMark
                size={48}
                src={demoLogo}
                label="Upload company photo"
                bind:element={logoMark}
                popovertarget="specimen-logo-panel"
              />
              <CompanyMark size={48} src={LOGO} label="Change Northwind's photo" />
              <CompanyMark size={48} label="Upload company photo" over />
              <CompanyMark size={48} src={LOGO} label="Change Northwind's photo" over />
              {#if !demoLogo}
                <Button variant="ghost" size="sm" popovertarget="specimen-logo-panel">Upload company photo</Button>
              {/if}
            </div>
            <Popover bind:open={logoOpen} anchor={logoMark} id="specimen-logo-panel" aria-label="Company photo">
              <LogoDrop
                set={demoLogo !== null}
                onpaste={() => ((demoLogo = LOGO), (logoOpen = false))}
                onchoose={() => ((demoLogo = WIDE_LOGO), (logoOpen = false))}
                onremove={() => ((demoLogo = null), (logoOpen = false))}
              />
            </Popover>
            <p class="meta">The first opens the panel: Paste sets a square logo, Choose file a wide one.</p>
          </div>
        </Card>
        <Card pad>
          <div class="tile">
            <p class="label">Logo drop · ready · files over it · setting · with a logo set</p>
            <div class="drops">
              <div class="drop-panel"><LogoDrop onpaste={() => {}} onchoose={() => {}} onremove={() => {}} /></div>
              <div class="drop-panel"><LogoDrop over onpaste={() => {}} onchoose={() => {}} onremove={() => {}} /></div>
              <div class="drop-panel"><LogoDrop busy onpaste={() => {}} onchoose={() => {}} onremove={() => {}} /></div>
              <div class="drop-panel"><LogoDrop set onpaste={() => {}} onchoose={() => {}} onremove={() => {}} /></div>
            </div>
          </div>
        </Card>
        <Card pad>
          <div class="tile">
            <p class="label">File drop (md, the setup's Experience step) · ready · files over it · taking the file</p>
            <FileDrop icon={FileUp} text="Drop your resume here" hint={`${RESUME_FORMATS}. You check every line before it’s added.`}>
              <Button variant="primary" kbd="I">Choose a file…</Button>
            </FileDrop>
            <FileDrop icon={FileUp} text="Drop to import it" hint={`${RESUME_FORMATS}. You check every line before it’s added.`} over>
              <Button variant="primary" kbd="I">Choose a file…</Button>
            </FileDrop>
            <FileDrop icon={FileUp} text="Reading your resume…" hint={RESUME_FORMATS} busy>
              <Button variant="primary" kbd="I" disabled>Choose a file…</Button>
            </FileDrop>
            <p class="meta">sm is the well inside Logo drop. A file dropped anywhere else on the window does nothing.</p>
          </div>
        </Card>
        <Card pad>
          <div class="tile">
            <p class="label">Sources · with LinkedIn · without · no note · an icon not bundled (the blank tile)</p>
            <Sources label="Searches" sources={searchSources(true)} note={SEARCH_SOURCES_NOTE} />
            <Sources label="Searches" sources={searchSources(false)} note={SEARCH_SOURCES_NOTE} />
            <Sources label="Pulls from" sources={searchSources(false).slice(0, 2)} />
            <Sources label="Searches" sources={[...searchSources(false).slice(0, 2), { name: "Recruitee", src: null }]} />
            <p class="meta">Each icon as its brand publishes it, at 16: scaled to fit, rounded like the tile. Hover one for its name.</p>
          </div>
        </Card>
      </div>
    </section>

    <section class="section">
      <h2 class="title">Table</h2>
      <p class="meta lede">
        Headers sort: the sorted column is ink with its arrow, others show the arrow faintly on hover or focus; a second
        click reverses. Role doesn’t sort; empty On file sorts last either way. One row can be selected (Northwind, in
        the selected wash); a click selects another. With checkboxes (second table), checked rows share that wash;
        Shift-click checks a range; the header is on, mixed or off for the rows shown; Fabrikam has no checkbox.
      </p>
      <Card>
        <div class="table">
          <Table
            columns={appColumns}
            rows={appRows}
            bind:sort={appSort}
            key={(r) => r.id}
            label="Applications (specimen)"
            selected={appSelected}
            onselect={(r) => (appSelected = r.id)}
          >
            {#snippet cell(row, column)}
              {#if column.key === "status"}
                <Badge tone={statusTone[row.status]}>{statuses.find((s) => s.value === row.status)?.label}</Badge>
              {:else if column.key === "resume"}
                <a class="file" href="#review">{row.resume}</a>
              {:else if column.key === "onFile"}
                {row.onFile ?? "—"}
              {:else if column.key === "company"}
                <CompanyMark>{row.company}</CompanyMark>
              {:else}
                {row[column.key as "date" | "role" | "now" | "projected"]}
              {/if}
            {/snippet}
          </Table>
        </div>
      </Card>
      <Card>
        <div class="table">
          <Table
            columns={appColumns}
            rows={appRows}
            key={(r) => r.id}
            label="Applications with checkboxes (specimen)"
            bind:checked={appChecked}
            checkable={(r) => r.status !== "rejected"}
            checkLabel={(r) => `Select ${r.company}`}
          >
            {#snippet cell(row, column)}
              {#if column.key === "status"}
                <Badge tone={statusTone[row.status]}>{statuses.find((s) => s.value === row.status)?.label}</Badge>
              {:else if column.key === "resume"}
                <a class="file" href="#review">{row.resume}</a>
              {:else if column.key === "onFile"}
                {row.onFile ?? "—"}
              {:else if column.key === "company"}
                <CompanyMark>{row.company}</CompanyMark>
              {:else}
                {row[column.key as "date" | "role" | "now" | "projected"]}
              {/if}
            {/snippet}
          </Table>
        </div>
      </Card>
    </section>

    <!-- ═══════════ Heatmap ═══════════ -->
    <section class="section">
      <h2 class="title">Heatmap</h2>
      <p class="meta lede">
        Postings added and applications sent, by day. Shades step against the busiest day shown; hover a day for what
        happened.
      </p>
      <Card pad>
        <Heatmap counts={heatBusy} end={heatEnd} label="Activity (specimen), last 26 weeks">
          {#snippet caption()}Most of your activity lands on Tuesdays.{/snippet}
        </Heatmap>
      </Card>
      <div class="grid cols-2">
        <Card pad>
          <div class="tile">
            <p class="label">No activity yet</p>
            <Heatmap counts={heatNone} end={heatEnd} weeks={12} label="No activity (specimen), last 12 weeks" />
          </div>
        </Card>
        <Card pad>
          <div class="tile">
            <p class="label">12 weeks, no caption</p>
            <Heatmap counts={heatBusy} end={heatEnd} weeks={12} label="Activity (specimen), last 12 weeks" />
          </div>
        </Card>
      </div>
    </section>

    <!-- ═══════════ Backdrop ═══════════ -->
    <section class="section">
      <h2 class="title">Backdrop</h2>
      <p class="meta lede">
        Settings → Appearance: a picture behind the top of the Overview, veiled and screened in paper so ink reads on
        it, fading out to paper (white in light, near black in dark). Switch the theme above to see both. The toolbar
        lies over it unpainted and without its crumb; scroll the sample and it takes its paper and hairline back.
      </p>
      <div class="backdrop-frame">
        <Toolbar over clear={!backdropScrolled}>
          {#snippet crumbs()}{/snippet}
          <Button icon={FileUp} kbd="I">Import resume</Button>
          <Button variant="primary" icon={ClipboardPaste} kbd="V">Paste from clipboard</Button>
        </Toolbar>
        <div class="backdrop-scroll" onscroll={(e) => (backdropScrolled = e.currentTarget.scrollTop > 0)}>
          <Backdrop name="meadow" />
          <div class="backdrop-content">
            <h3 class="headline">What’s up next, Jane?</h3>
            <Card pad>
              <div class="tile">
                <p class="label">Cards let it through, faintly</p>
                <p class="meta">Paste a job link or a posting’s text and it lands here.</p>
              </div>
            </Card>
          </div>
        </div>
      </div>
    </section>

    <!-- ═══════════ Tooltips ═══════════ -->
    <section class="section">
      <h2 class="title">Tooltips</h2>
      <p class="meta lede">
        One at a time, centered above what you hover (below when there’s no room above). The first opens after 300ms of
        rest, the next at once, so a chart reads as you move along it. Keyboard focus opens it at once; Esc, a click,
        scrolling or leaving closes it. A label, or data: a title and its value, rows, legend keys, a note.
      </p>
      <Card pad>
        <div class="tips">
          {#each tipKinds as kind (kind.name)}
            <div class="tile">
              <p class="label">{kind.name}</p>
              <Tooltip content={kind.content} />
            </div>
          {/each}
        </div>
      </Card>
      <Card pad>
        <div class="tile">
          <p class="label">Live · hover, or Tab to a button</p>
          <div class="row">
            {#each tipKinds as kind (kind.name)}
              <Button size="sm" {@attach tip(kind.content)}>{kind.name}</Button>
            {/each}
          </div>
        </div>
      </Card>
    </section>

    <!-- ═══════════ Feedback ═══════════ -->
    <section class="section">
      <h2 class="title">Feedback</h2>
      <div class="grid cols-2">
        <Card pad>
          <div class="tile">
            <p class="label">Toast · success with action · warning · error · neutral</p>
            <div class="row">
              <Button size="sm" onclick={exportDemo}>Success</Button>
              <Button size="sm" onclick={() => toast("Check the rewrite: “24/7” isn’t in your resume", { tone: "warn" })}>Warning</Button>
              <Button size="sm" onclick={() => toast("Claude Code isn’t signed in. Run `claude` in Terminal and sign in.", { tone: "danger" })}>Error</Button>
              <Button size="sm" onclick={() => toast("Cancelled", { tone: "neutral" })}>Neutral</Button>
            </div>
            <p class="meta">Errors stay until dismissed; the rest leave after 5 s (8 s with an action).</p>
          </div>
        </Card>
        <Card>
          {#key replay}
            <Empty title="No postings yet" body="Paste a job link or the posting’s text, and the score shows up in a few seconds.">
              {#snippet action()}<Button variant="primary" kbd="V">Paste from clipboard</Button>{/snippet}
            </Empty>
          {/key}
        </Card>
      </div>
    </section>

    <!-- ═══════════ Steps ═══════════ -->
    <section class="section">
      <h2 class="title">Steps</h2>
      <p class="meta lede">
        The first-run setup’s progress: a bar per step, done and current in accent, no names on screen (each is its
        tooltip and what assistive tech reads). The fill sweeps in from the leading edge as you go on and back out as you
        go back. Done steps are buttons back to themselves.
      </p>
      <div class="grid">
        <Card pad>
          <div class="tile">
            <p class="label">First step · a middle one (done steps are buttons) · the last · all done · no way back</p>
            <Steps steps={SETUP} current={0} label="Setup, first step" onselect={() => {}} />
            <Steps steps={SETUP} current={3} label="Setup, a middle step" onselect={() => {}} />
            <Steps steps={SETUP} current={5} label="Setup, the last step" onselect={() => {}} />
            <Steps steps={SETUP} current={6} label="Setup, all done" />
            <Steps steps={SETUP} current={3} label="Setup without a way back" />
            <p class="meta">Hover a step for its name; a done one lightens on hover.</p>
          </div>
        </Card>
      </div>
      <Card pad>
        <div class="tile">
          <p class="label">Live · a step turning: .turn in, exit({"{ side }"}) out</p>
          <Steps steps={SETUP.slice(0, TURNS.length)} current={turnAt} label="Sample steps" onselect={turnTo} />
          <div class="turns">
            {#key turnAt}
              <div
                class="turns__page"
                style:--side={turnForward ? 1 : -1}
                style:--wait={turned ? "var(--dur-fast)" : "0ms"}
                out:exit={{ side: turnForward ? -1 : 1 }}
              >
                <h3 class="title turn" style:--i="0">{TURNS[turnAt].title}</h3>
                <p class="meta turn" style:--i="1">{TURNS[turnAt].body}</p>
              </div>
            {/key}
          </div>
          <div class="row">
            <Button variant="ghost" disabled={turnAt === 0} onclick={() => turnTo(turnAt - 1)}>Back</Button>
            <Button variant="primary" disabled={turnAt === TURNS.length - 1} onclick={() => turnTo(turnAt + 1)}>
              Continue
            </Button>
          </div>
        </div>
      </Card>
    </section>

    <!-- ═══════════ Section nav ═══════════ -->
    <section class="section">
      <h2 class="title">Section nav</h2>
      <p class="meta lede">
        Settings’ links to its cards. The one you’re reading is ink with an accent tick: it follows the scroll, and the
        last is current at the end. A link jumps at once and moves focus to its section, which stays current while its
        top is in view. Under 720px of its container: a row over a hairline, the tick on it, scrolling sideways when
        the links don’t fit.
      </p>
      <div class="grid">
        <Card>
          <div class="jump" bind:this={jumpColumn}>
            <div class="jump__layout">
              <div class="jump__rail">
                <SectionNav sections={JUMPS_COLUMN} root={jumpColumn} label="Sample sections, column" />
              </div>
              <div class="jump__body">
                {#each JUMPS as j (j.id)}
                  <section id="spec-jump-column-{j.id}" class="jump__section" tabindex="-1">
                    <h3 class="jump__name">{j.label}</h3>
                    <p class="meta">{j.body}</p>
                    {#if j.tall}<div class="jump__well"></div>{/if}
                  </section>
                {/each}
              </div>
            </div>
          </div>
        </Card>
        <Card>
          <div class="narrow">
            <div class="jump" bind:this={jumpRow}>
              <div class="jump__layout">
                <div class="jump__rail">
                  <SectionNav sections={JUMPS_ROW} root={jumpRow} label="Sample sections, row" />
                </div>
                <div class="jump__body">
                  {#each JUMPS as j (j.id)}
                    <section id="spec-jump-row-{j.id}" class="jump__section" tabindex="-1">
                      <h3 class="jump__name">{j.label}</h3>
                      <p class="meta">{j.body}</p>
                      {#if j.tall}<div class="jump__well"></div>{/if}
                    </section>
                  {/each}
                </div>
              </div>
            </div>
          </div>
        </Card>
      </div>
    </section>

    <!-- ═══════════ Desk ═══════════ -->
    <section class="section">
      <h2 class="title">Desk</h2>
      <p class="meta lede">
        100% fits the page to the desk's width. With the job's map, the pane splits: the map fills its top, the desk
        moves down under it, and the page shrinks so the two fill the pane exactly; the map takes the height the page
        leaves, at least 160px. A remote job has no map.
      </p>
      <div class="grid cols-3">
        <div class="desk-frame">
          <Desk pages={FITS}>
            {#snippet status()}<Badge tone="gain" dot>Fits on 1 page</Badge>{/snippet}
          </Desk>
        </div>
        <div class="desk-frame">
          <Desk pages={FITS}>
            {#snippet status()}<Badge tone="gain" dot>Fits on 1 page</Badge>{/snippet}
            {#snippet above()}<PlaceMap places={[PLANO]} onopen={() => {}} />{/snippet}
          </Desk>
        </div>
        <div class="desk-frame">
          <Desk pages={OVER} zoom={75}>
            {#snippet status()}<Badge tone="warn" dot>2 pages: over by 1</Badge>{/snippet}
          </Desk>
        </div>
      </div>
    </section>

    <!-- ═══════════ Place map ═══════════ -->
    <section class="section">
      <h2 class="title">Place map</h2>
      <p class="meta lede">
        Where the job is, as a maps search shows a town: the pin in accent with its name, the boundary dashed, the towns
        and roads around it in graphite, the place and its region labeled on top. Static; a click (or M on the job)
        opens it in Maps. Several places: framed together, the label counts them, and a pager at the foot steps the
        camera to each and back to all (its count shows them all). Until the tiles arrive, and when they can't, the name
        and region stand in.
      </p>
      <div class="grid cols-2">
        <div class="map-frame"><PlaceMap places={[PLANO]} onopen={() => {}} /></div>
        <div class="map-frame"><PlaceMap places={[MCLEAN]} /></div>
        <div class="map-frame"><PlaceMap places={[RIVERWOODS, MCLEAN, PLANO]} onopen={() => {}} /></div>
        <div class="map-frame"><PlaceMap places={[RIVERWOODS, MCLEAN, PLANO]} current={1} onopen={() => {}} /></div>
        <div class="map-frame"><PlaceMap places={[PLANO]} unavailable /></div>
      </div>
    </section>

    <!-- ═══════════ Chrome ═══════════ -->
    <section class="section">
      <h2 class="title">Toolbar, nav, key footer</h2>
      <Card>
        <Toolbar>
          {#snippet crumbs()}<a href="#review">Applications</a>{/snippet}
          <Button icon={ExternalLink}>Open applications.csv</Button>
        </Toolbar>
        <div class="chrome">
          <div class="nav-sample">
            <Nav label="Nav states (specimen)">
              <NavItem href="#review" icon={ListChecks} current meta={12}>Current</NavItem>
              <NavItem href="#review" icon={Archive}>Default</NavItem>
              <NavItem href="#review" quiet meta={78}>
                {#snippet lead()}<CompanyMark size={16} />{/snippet}Quiet entry with a long title that truncates
              </NavItem>
              <NavItem
                href="#review"
                quiet
                sub="Senior Front-End Engineer, Payments Platform"
                status={{ label: "Rejected", tone: "danger" }}
              >
                {#snippet lead()}<CompanyMark size={28} />{/snippet}
                {#snippet trailing()}<ScoreBar bar={false} now={70} projected={76} />{/snippet}
                A company with a long name that truncates
              </NavItem>
              <NavItem
                href="#review"
                quiet
                sub="Senior Front-End Engineer, Payments Platform"
                status={{ label: "Applied", tone: "accent" }}
                busy="Suggesting…"
              >
                {#snippet lead()}<CompanyMark size={28} />{/snippet}
                {#snippet trailing()}<ScoreBar bar={false} projected={61} />{/snippet}
                Busy: a model call running for it
              </NavItem>
            </Nav>
          </div>
          <KeyFooter keys={reviewKeys} />
        </div>
        <!-- `bare`: the Commands button alone, inside another toolbar (the chat's composer). -->
        <div class="keys-bare">
          <span class="meta">Bare, in the chat composer’s toolbar</span>
          <KeyFooter bare keys={reviewKeys} />
        </div>
      </Card>
    </section>
  </div>
</div>

<style>
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    overscroll-behavior: contain;
  }
  .spec {
    display: grid;
    gap: var(--space-16);
    max-width: 1280px;
    margin-inline: auto;
    padding: var(--space-10) var(--space-10) var(--space-16);
  }
  .spec__head {
    display: grid;
    gap: var(--space-1);
  }
  .spec__head .meta {
    max-width: 64ch;
    margin: 0;
  }
  .section {
    display: grid;
    gap: var(--space-4);
  }
  .lede {
    max-width: 72ch;
    margin: calc(var(--space-2) * -1) 0 0;
  }
  .grid {
    display: grid;
    gap: var(--space-4);
  }
  .cols-2 {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  .cols-3 {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
  @media (max-width: 1100px) {
    .cols-2,
    .cols-3 {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }
  .nowrap {
    flex-wrap: nowrap;
  }
  .tile {
    display: grid;
    gap: var(--space-3);
    align-content: start;
  }
  code {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--ink-3);
  }
  .swatch {
    display: grid;
    grid-template-columns: 40px 1fr;
    gap: 0 var(--space-3);
    align-items: center;
  }
  .swatch i {
    grid-row: span 2;
    width: 40px;
    height: 40px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    box-shadow: var(--shadow-border);
  }
  .ramp {
    display: grid;
    gap: var(--space-3);
  }
  .ramp > div {
    display: grid;
    grid-template-columns: 200px 1fr;
    align-items: baseline;
    gap: var(--space-4);
  }
  .score-sample {
    font-family: var(--font-display);
    font-size: var(--text-score);
    line-height: var(--leading-score);
    letter-spacing: var(--tracking-score);
  }
  .score-sample small {
    font-size: 0.5em;
    color: var(--ink-3);
  }
  .lg {
    font-size: var(--text-lg);
    line-height: var(--leading-lg);
    letter-spacing: var(--tracking-lg);
  }
  .md {
    font-size: var(--text-md);
    line-height: var(--leading-md);
    letter-spacing: var(--tracking-md);
  }

  /* ── The framed window ── */
  .frame {
    display: grid;
    grid-template-columns: var(--sidebar-width) minmax(0, 1fr);
    height: 800px;
    overflow: hidden;
    border-radius: var(--radius-lg);
    border: 1px solid transparent;
    background: var(--paper);
    box-shadow: var(--shadow-page);
  }
  .frame__sidebar {
    display: grid;
    grid-template-rows: var(--toolbar-height) auto minmax(0, 1fr) auto;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--space-4);
    min-width: 0;
    padding: 0 var(--space-3) var(--space-3);
    background: var(--sidebar);
    box-shadow: inset -1px 0 0 var(--line);
  }
  .lights {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding-inline: var(--space-2);
  }
  .lights i {
    width: 12px;
    height: 12px;
    border-radius: var(--radius-full);
    background: var(--line-strong);
  }
  .frame__recent {
    display: grid;
    align-content: start;
    gap: var(--space-1);
  }
  .frame__recent .label {
    padding: var(--space-2) var(--space-2) 0;
  }
  .frame__main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .split {
    flex: 1;
    display: grid;
    grid-template-columns: minmax(var(--column-min), 520px) minmax(0, 1fr);
    min-height: 0;
  }
  .column {
    display: grid;
    align-content: start;
    gap: var(--space-6);
    min-height: 0;
    overflow: auto;
    overscroll-behavior: contain;
    padding: var(--space-6) var(--space-5) 0;
    box-shadow: inset -1px 0 0 var(--line);
  }
  .job {
    display: grid;
    gap: var(--space-2);
    padding-inline: var(--space-1);
  }
  /* As the job header: the 48px mark spans the company and the title, from the company's cap height to its baseline. */
  .named {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    column-gap: var(--space-3);
    row-gap: var(--space-0-5);
    align-items: start;
  }
  .lead {
    display: flex;
    grid-row: span 2;
    margin-top: 6px;
  }
  .company {
    margin: 0;
    font-size: var(--text-lg);
    line-height: var(--leading-lg);
    letter-spacing: var(--tracking-lg);
    font-weight: var(--weight-medium);
    color: var(--ink-2);
  }
  /* As the job screen: sections share the change groups' inset. */
  .skills,
  .changes {
    display: grid;
    margin-inline: calc(var(--space-2) * -1);
  }
  .fold {
    display: grid;
  }
  .chips {
    padding: var(--space-2) var(--space-3) 0;
  }
  .fold-all {
    display: flex;
    justify-content: flex-end;
    margin-block: calc(var(--space-3) * -1) calc(var(--space-4) * -1);
  }
  .note {
    display: flex;
    gap: var(--space-2);
    align-items: center;
    padding: 0 var(--space-3) var(--space-3) calc(var(--space-3) + 16px + var(--space-3));
  }
  .picker-anchor {
    margin-top: calc(var(--space-4) * -1);
  }
  .picker {
    display: grid;
    gap: var(--space-0-5);
    padding: var(--space-3);
    justify-items: start;
  }
  .picker .label {
    padding-bottom: var(--space-1);
  }
  .drops {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-4);
  }
  /* LogoDrop as its popover holds it. */
  .drop-panel {
    border: 1px solid transparent;
    border-radius: var(--radius-lg);
    background: var(--surface);
    box-shadow: var(--shadow-popover);
  }
  .keys-wrap {
    position: sticky;
    bottom: 0;
    margin-inline: calc(var(--space-5) * -1);
  }
  .rows {
    display: grid;
    padding: 0 var(--space-2) var(--space-2);
  }
  .demo-body {
    margin: 0;
    padding: var(--space-2) var(--space-3);
  }
  .working-pane {
    position: relative;
    display: grid;
    place-items: center;
    height: 280px;
    border-radius: var(--radius-md);
    background: var(--paper);
    overflow: hidden;
  }
  .part-progress {
    padding: 0 var(--space-3) var(--space-3);
  }
  .table {
    overflow: hidden;
    border-radius: var(--radius-lg);
  }
  .file {
    color: var(--accent-text);
    text-decoration: none;
  }
  .desk-frame {
    display: grid;
    height: 520px;
    overflow: hidden;
    border-radius: var(--radius-lg);
    border: 1px solid transparent;
    box-shadow: var(--shadow-border);
  }
  .map-frame {
    height: 200px;
    overflow: hidden;
    border-radius: var(--radius-lg);
    border: 1px solid transparent;
    box-shadow: var(--shadow-border);
  }
  .narrow {
    display: grid;
    gap: var(--space-4);
    max-width: 360px;
  }
  /* A sample scroll for SectionNav, as Settings': the rail held beside the sections, or a row held over them. */
  .jump {
    height: 280px;
    overflow: auto;
    overscroll-behavior: contain;
    container-type: inline-size;
    /* Inside the card's 1px border. */
    border-radius: calc(var(--radius-lg) - 1px);
  }
  .jump__layout {
    display: grid;
    grid-template-columns: 160px minmax(0, 1fr);
    align-items: start;
    gap: var(--space-6);
    padding: var(--space-5);
  }
  .jump__rail {
    position: sticky;
    top: var(--space-5);
  }
  .jump__body {
    display: grid;
    gap: var(--space-6);
  }
  .jump__section {
    display: grid;
    gap: var(--space-1);
    scroll-margin-top: var(--space-5);
  }
  .jump__section:focus {
    outline: none;
  }
  .jump__name,
  .jump__section .meta {
    margin: 0;
  }
  .jump__name {
    font-size: var(--text-base);
    font-weight: var(--weight-medium);
  }
  .jump__well {
    height: 240px;
    margin-top: var(--space-2);
    border-radius: var(--radius-md);
    background: var(--sunken);
  }
  @container (width < 720px) {
    .jump__layout {
      grid-template-columns: minmax(0, 1fr);
      gap: var(--space-5);
      padding-top: 0;
    }
    .jump__rail {
      top: 0;
      z-index: 1;
      margin-inline: calc(var(--space-5) * -1);
      padding: var(--space-2) var(--space-5) 0;
      background: var(--surface);
    }
    .jump__section {
      scroll-margin-top: calc(var(--control-md) + var(--space-4) + var(--space-5));
    }
  }
  /* The leaving step and the arriving one share the cell while they cross. */
  .turns {
    display: grid;
    min-height: 96px;
  }
  .turns__page {
    grid-area: 1 / 1;
    display: grid;
    align-content: start;
    gap: var(--space-2);
  }
  .turns__page p {
    margin: 0;
  }
  .backdrop-frame {
    position: relative;
    height: 640px;
    overflow: hidden;
    border-radius: var(--radius-lg);
    border: 1px solid transparent;
    background: var(--paper);
    box-shadow: var(--shadow-border);
  }
  .backdrop-scroll {
    position: relative;
    height: 100%;
    overflow: auto;
    overscroll-behavior: contain;
  }
  .backdrop-content {
    position: relative;
    display: grid;
    gap: var(--space-6);
    max-width: 560px;
    margin-inline: auto;
    padding: calc(var(--toolbar-height) + var(--space-10)) var(--space-6) 640px;
  }
  .backdrop-content :global(.card) {
    background: color-mix(in oklab, var(--surface) var(--backdrop-card), transparent);
  }
  .backdrop-content .headline {
    margin: 0;
  }
  .chrome {
    display: grid;
    grid-template-columns: var(--sidebar-width) minmax(0, 1fr);
    align-items: end;
  }
  .nav-sample {
    padding: var(--space-3);
  }
  .keys-bare {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--space-3);
    padding: var(--space-1-5) var(--space-3);
    box-shadow: inset 0 1px 0 var(--line);
  }

  /* ── The synthetic sheet (stands in for the renderer's SVG) ── */
  :global(.sheet) {
    height: 792px;
    padding: 40px 44px;
    color: var(--page-ink);
    font-family: var(--font-display);
    font-size: 10.5px;
    line-height: 1.32;
    letter-spacing: 0;
  }
  :global(.sheet h1) {
    margin: 0 0 1px;
    font-size: 19px;
    font-weight: var(--weight-semibold);
  }
  :global(.sheet h2) {
    margin: 6px 0 2px;
    padding: 1px 0;
    font-size: 11px;
    font-weight: var(--weight-semibold);
    box-shadow:
      inset 0 1px 0 var(--page-ink),
      inset 0 -1px 0 var(--page-ink);
  }
  :global(.sheet ul) {
    margin: 0;
    padding-inline-start: 14px;
  }
  :global(.sheet__contact) {
    margin-bottom: 4px;
  }
  :global(.sheet__entry) {
    display: flex;
    justify-content: space-between;
    margin-top: 3px;
    font-weight: var(--weight-semibold);
  }
  :global(.sheet__entry em) {
    font-weight: var(--weight-regular);
  }
  :global(.sheet__sub) {
    font-style: italic;
  }
  :global(.sheet__mark) {
    background: var(--page-mark);
  }

  /* Score bar: each state beside its bar, at a table cell's width. */
  .bars {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 176px;
    align-items: center;
    gap: var(--space-3) var(--space-8);
  }
  .bars p {
    margin: 0;
  }
  .bars--alone {
    grid-template-columns: minmax(0, 1fr) auto;
  }
  /* Tooltips: each kind in place, as wide as it draws. */
  .tips {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: var(--space-6) var(--space-4);
  }
  .tips .tile {
    justify-items: start;
  }
</style>
