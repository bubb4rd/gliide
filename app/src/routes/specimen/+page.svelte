<!--
  The design system's specimen: the composed Review screen, then every primitive in every state, in the theme
  chosen at the top (Light, Dark, System; a preview that isn't saved). Content is synthetic (Jane Q. Doe).
  Open /specimen?theme=dark to start dark.
-->
<script lang="ts">
  import { untrack } from "svelte";
  import { page } from "$app/state";
  import Archive from "@lucide/svelte/icons/archive";
  import Briefcase from "@lucide/svelte/icons/briefcase";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Download from "@lucide/svelte/icons/download";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import FileText from "@lucide/svelte/icons/file-text";
  import ListChecks from "@lucide/svelte/icons/list-checks";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Plus from "@lucide/svelte/icons/plus";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Search from "@lucide/svelte/icons/search";
  import SlidersHorizontal from "@lucide/svelte/icons/sliders-horizontal";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import X from "@lucide/svelte/icons/x";
  import type { Theme } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import { app } from "$lib/stores/app.svelte";
  import { theme } from "$lib/stores/theme.svelte";
  import {
    Badge,
    Button,
    Card,
    ChangeRow,
    Checkbox,
    Chip,
    Desk,
    Empty,
    GroupHead,
    Icon,
    Input,
    Kbd,
    KeyFooter,
    Marked,
    Menu,
    Nav,
    NavItem,
    PartRow,
    Popover,
    Progress,
    ScoreHeader,
    Segmented,
    Table,
    Textarea,
    Toolbar,
    toast,
  } from "$lib/ui";
  import { FITS, OVER } from "./sample";

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

  let selected = $state(0);
  let editing = $state<string | null>(null);
  let draft = $state("");
  let noting = $state<string | null>(null);
  let note = $state("");
  let mode = $state<"master" | "bank">("bank");
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
    }),
  );

  const masters = [
    { value: "swe", label: "Software Engineering", meta: "53%" },
    { value: "it", label: "IT Support", meta: "41%" },
    { value: "design", label: "Product Design", meta: "22%" },
  ];
  const parts = [
    { label: "Hard skills and tools", value: 0.72, weight: 0.4, present: true, detail: "13 of 18 terms" },
    { label: "Domain and soft", value: 0.6, weight: 0.15, present: true, detail: "3 of 5 terms" },
    { label: "Job title", value: 1, weight: 0.15, present: true, detail: "“Software Engineer” on the page" },
    { label: "Experience", value: 0.5, weight: 0.15, present: true, detail: "1 of 2 years asked" },
    { label: "Education", value: 1, weight: 0.1, present: true, detail: "B.S. Computer Science" },
    { label: "Format", value: 0, weight: 0.05, present: false, detail: "Not in the posting" },
  ];
  const reviewKeys = [
    { keys: ["↑", "↓"], label: "Select" },
    { keys: ["Space"], label: "Toggle" },
    { keys: ["⏎"], label: "Edit" },
    { keys: ["A"], label: "Add your own" },
    { keys: ["E"], label: "Export" },
  ];

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
  const appColumns = [
    { key: "date", label: "Date", width: "112px" },
    { key: "company", label: "Company", width: "18%" },
    { key: "role", label: "Role" },
    { key: "now", label: "Now", numeric: true, width: "72px" },
    { key: "projected", label: "Projected", numeric: true, width: "96px" },
    { key: "onFile", label: "On file", numeric: true, width: "80px" },
    { key: "status", label: "Status", width: "128px" },
    { key: "resume", label: "Resume", width: "22%" },
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
            <NavItem href="#review" icon={Briefcase} meta={12}>Jobs</NavItem>
            <NavItem href="#review" icon={Archive} meta={9}>Experience bank</NavItem>
            <NavItem href="#review" icon={FileText} meta={3}>Masters</NavItem>
            <NavItem href="#review" icon={ListChecks} meta={41}>Applications</NavItem>
          </Nav>
          <div class="frame__recent">
            <p class="label">Recent</p>
            <Nav label="Recent (specimen)">
              <NavItem href="#review" quiet current meta={78}>Globex · Junior Software Engineer</NavItem>
              <NavItem href="#review" quiet meta={71}>Northwind Traders · Support Engineer</NavItem>
              <NavItem href="#review" quiet meta={64}>Fabrikam · Product Designer</NavItem>
              <NavItem href="#review" quiet meta={59}>Contoso · IT Analyst</NavItem>
            </Nav>
          </div>
          <div>
            <Nav label="App (specimen)">
              <NavItem href="#review" icon={SlidersHorizontal}>Settings</NavItem>
            </Nav>
            <div class="agent"><i></i>Claude Code · signed in</div>
          </div>
        </aside>

        <div class="frame__main">
          <Toolbar>
            {#snippet crumbs()}
              <a href="#review">Jobs</a><Icon icon={ChevronRight} size={14} weight="medium" /><b>Globex</b>
            {/snippet}
            <Segmented
              options={[
                { value: "master", label: "Master only" },
                { value: "bank", label: "Bank-aware" },
              ]}
              bind:value={mode}
              label="Suggestions from"
            />
            <hr />
            <Menu bind:value={master} options={masters} label="Master" placement="bottom-end" />
            <Button variant="primary" icon={Download} kbd="E" onclick={exportDemo}>Export</Button>
          </Toolbar>

          <div class="split">
            <div class="column">
              {#key replay}
                <div class="job enter" style:--i="0">
                  <div class="row">
                    <Badge tone="accent">Tailor first</Badge>
                    <span class="meta">Globex · Remote, US · read from globex.com</span>
                  </div>
                  <h1 class="headline">Junior Software Engineer</h1>
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

                <div class="skills enter" style:--i="2">
                  <p class="label">Skills the posting asks for</p>
                  <div class="row">
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
                  </div>
                </div>

                <div class="changes enter" style:--i="3">
                  {#each groups as group (group.label)}
                    <GroupHead label={group.label} meta={group.meta} />
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
          <div><code>--text-title · serif</code><span class="title">Experience bank</span></div>
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
              <Chip state="have" disabled title="Only stated in your bullets">Agile</Chip>
            </div>
            <p class="label">Checkbox · off · on · disabled</p>
            <div class="row">
              <Checkbox bind:checked={check.off} label="Off example" />
              <Checkbox bind:checked={check.on} label="On example" />
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
            <Progress label="Rewriting the paragraph…" expectedMs={15000} oncancel={() => {}} />
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

    <!-- ═══════════ New primitives ═══════════ -->
    <section class="section">
      <h2 class="title">Menus, popovers, progress</h2>
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
            <p class="label">Progress · determinate · indeterminate</p>
            {#key progressRun}
              <Progress label="Reading the requirements…" expectedMs={6000} oncancel={() => toast("Cancelled", { tone: "neutral" })} />
              <Progress label="Suggesting edits…" />
            {/key}
            <div class="row"><Button size="sm" icon={RotateCcw} onclick={() => progressRun++}>Restart</Button></div>
          </div>
        </Card>
      </div>
    </section>

    <section class="section">
      <h2 class="title">Table</h2>
      <Card>
        <div class="table">
          <Table columns={appColumns} rows={appRows} key={(r) => r.id} label="Applications (specimen)">
            {#snippet cell(row, column)}
              {#if column.key === "status"}
                <Badge tone={statusTone[row.status]}>{statuses.find((s) => s.value === row.status)?.label}</Badge>
              {:else if column.key === "resume"}
                <a class="file" href="#review">{row.resume}</a>
              {:else if column.key === "onFile"}
                {row.onFile ?? "—"}
              {:else}
                {row[column.key as "date" | "company" | "role" | "now" | "projected"]}
              {/if}
            {/snippet}
          </Table>
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

    <!-- ═══════════ Desk ═══════════ -->
    <section class="section">
      <h2 class="title">Desk</h2>
      <div class="grid cols-2">
        <div class="desk-frame">
          <Desk pages={FITS}>
            {#snippet status()}<Badge tone="gain" dot>Fits on 1 page</Badge>{/snippet}
          </Desk>
        </div>
        <div class="desk-frame">
          <Desk pages={OVER} zoom={75}>
            {#snippet status()}<Badge tone="warn" dot>2 pages: over by 1</Badge>{/snippet}
          </Desk>
        </div>
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
              <NavItem href="#review" icon={Briefcase} current meta={12}>Current</NavItem>
              <NavItem href="#review" icon={Archive}>Default</NavItem>
              <NavItem href="#review" quiet meta={78}>Quiet entry with a long title that truncates</NavItem>
            </Nav>
          </div>
          <KeyFooter keys={reviewKeys} />
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
  .agent {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-1-5) var(--space-2);
    font-size: var(--text-sm);
    color: var(--ink-3);
  }
  .agent i {
    width: 6px;
    height: 6px;
    border-radius: var(--radius-full);
    background: var(--gain);
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
  .skills {
    display: grid;
    gap: var(--space-3);
    padding-inline: var(--space-1);
  }
  .changes {
    display: grid;
    margin-inline: calc(var(--space-2) * -1);
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
  .keys-wrap {
    position: sticky;
    bottom: 0;
    margin-inline: calc(var(--space-5) * -1);
  }
  .rows {
    display: grid;
    padding: 0 var(--space-2) var(--space-2);
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
  .chrome {
    display: grid;
    grid-template-columns: var(--sidebar-width) minmax(0, 1fr);
    align-items: end;
  }
  .nav-sample {
    padding: var(--space-3);
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
</style>
