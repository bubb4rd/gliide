<!--
  /welcome: the first-run setup, in a window without a sidebar. Six steps under one progress bar (`Steps`, its names
  only in tooltips and to assistive tech), you first and then your tools: Welcome (what gliide does and its three
  promises), Experience (drop or choose a resume; the import review comes back here; building one with your agent is
  coming), What it says (what the feed looks for, worked out from the resume and changed in place, what only you can
  say, and whether it pulls), Agents (Claude Code, Codex and Antigravity as a choice of one, saved as the agent every
  model call runs on, each found and signed in or not, checked again as you arrive and on R; the chosen one's problem
  and how to install it), Appearance (the theme, applied as you choose) and Ready (your experience, what you're looking
  for and your agent, each a way back to its step). The step is in the URL (`?step=says`) so the import review can come
  back to it. ⏎ goes on, ⌘[ goes back, a done step in the bar goes back to it; Skip setup and the last step's button
  end the setup for good. The footer follows the step (held at the window's foot once a step is taller than it). A
  step leaves sideways toward where you came from and the next enters from where you're going, its parts 100ms apart;
  with reduced motion they only fade.
-->
<script lang="ts">
  import { untrack } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import Check from "@lucide/svelte/icons/check";
  import CircleDashed from "@lucide/svelte/icons/circle-dashed";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import FileLock from "@lucide/svelte/icons/file-lock";
  import FileUp from "@lucide/svelte/icons/file-up";
  import Gauge from "@lucide/svelte/icons/gauge";
  import MessagesSquare from "@lucide/svelte/icons/messages-square";
  import RotateCw from "@lucide/svelte/icons/rotate-cw";
  import Search from "@lucide/svelte/icons/search";
  import ShieldCheck from "@lucide/svelte/icons/shield-check";
  import UserRound from "@lucide/svelte/icons/user-round";
  import { Badge, Button, Card, Checkbox, CompanyMark, FileDrop, Icon, Segmented, Steps, Toolbar, exit, toast } from "$lib/ui";
  import {
    type Agent,
    type AgentStatus,
    appError,
    feedSettings,
    setFeedSettings,
    type FeedSettingsDto,
    type FeedSettingsView,
    type Settings,
    type Theme,
  } from "$lib/api";
  import { AGENT_LIST, type AgentInfo } from "$lib/agents";
  import { CATEGORY, COUNTRIES, EMPLOYMENT, SENIORITY, countryName } from "$lib/feed";
  import { RESUME_FORMATS, chooseResume, watchResumeDrop } from "$lib/import";
  import { bindKeys } from "$lib/keys";
  import { app } from "$lib/stores/app.svelte";
  import { feed } from "$lib/stores/feed.svelte";
  import { theme } from "$lib/stores/theme.svelte";

  const STEPS = [
    { id: "welcome", name: "Welcome" },
    { id: "resume", name: "Experience" },
    { id: "says", name: "What it says" },
    { id: "agent", name: "Agents" },
    { id: "look", name: "Appearance" },
    { id: "ready", name: "Ready" },
  ] as const;
  type StepId = (typeof STEPS)[number]["id"];
  const NAMES = STEPS.map((s) => s.name);
  const THEMES: { value: Theme; label: string }[] = [
    { value: "light", label: "Light" },
    { value: "dark", label: "Dark" },
    { value: "system", label: "System" },
  ];
  const PROMISES = [
    {
      icon: Gauge,
      title: "Code does the scoring",
      body: "Every percentage comes from your resume’s rendered text, never from the model.",
    },
    {
      icon: ShieldCheck,
      title: "Nothing invented",
      body: "Each rewrite is checked against what you’ve actually done.",
    },
    {
      icon: FileLock,
      title: "Your resume stays yours",
      body: "Every edit is yours to take or leave, and your master is never changed.",
    },
  ];

  const at = $derived(
    Math.max(
      0,
      STEPS.findIndex((s) => s.id === page.url.searchParams.get("step")),
    ),
  );
  const step = $derived<StepId>(STEPS[at].id);
  const indexOf = (id: StepId) => STEPS.findIndex((s) => s.id === id);
  /** The way you last moved, which sets the side a step leaves by and the next enters from. */
  let forward = $state(true);
  /** A step has left: the next waits for it. The first on screen doesn't. */
  let turned = $state(false);
  /** The content has scrolled under the toolbar, which then needs its paper. */
  let scrolled = $state(false);
  /** The step runs on under the footer, held at the window's foot: it takes its paper and hairline. */
  let stuck = $state(false);
  let scroller = $state<HTMLDivElement>();

  function measure() {
    if (!scroller) return;
    scrolled = scroller.scrollTop > 0;
    stuck = scroller.scrollHeight - scroller.clientHeight - scroller.scrollTop > 1;
  }

  // A step's height changes as it turns, as a filter opens and as the window resizes.
  $effect(() => {
    if (!scroller) return;
    const observer = new ResizeObserver(measure);
    observer.observe(scroller);
    for (const child of scroller.children) observer.observe(child);
    return () => observer.disconnect();
  });

  function go(index: number) {
    if (index < 0 || index >= STEPS.length || index === at) return;
    forward = index > at;
    turned = true;
    editing = null;
    void goto(`/welcome?step=${STEPS[index].id}`, { replaceState: true, noScroll: true, keepFocus: true });
  }

  // The new step's heading takes focus, so assistive tech reads where you are.
  $effect(() => {
    const id = `step-${step}`;
    if (untrack(() => turned)) requestAnimationFrame(() => document.getElementById(id)?.focus({ preventScroll: true }));
  });

  async function finish() {
    await app.finishSetup();
    await goto("/", { replaceState: true });
  }

  function next() {
    if (step === "ready") void finish();
    else go(at + 1);
  }

  const plural = (n: number, one: string, many: string) => `${n.toLocaleString()} ${n === 1 ? one : many}`;

  // ── Experience ──
  const BACK = "/welcome?step=resume";
  const bank = $derived(app.library?.bank ?? null);
  const masters = $derived(app.library?.masters ?? []);
  /** "Jane (JJ) Doe" → "JJ"; else the first name (as the Overview greets you). */
  const firstName = $derived(
    bank ? bank.name.match(/\(([^)]+)\)/)?.[1]?.trim() || bank.name.trim().split(/\s+/)[0] || "" : "",
  );
  const importResume = () => chooseResume(app.settings?.exportDir, BACK);
  /** Files are dragged over the drop well. */
  let dropOver = $state(false);

  // A resume dropped on the well opens its import review, while the step waits for one.
  $effect(() => {
    if (step !== "resume" || bank) return;
    return watchResumeDrop(BACK, (over) => (dropOver = over));
  });

  // ── Agents ──
  let checking = $state(false);

  /** One check, every agent. */
  async function checkAgents() {
    if (checking) return;
    checking = true;
    await app.refreshAgents();
    checking = false;
  }

  /** The agent model calls run on (Claude Code until you choose another), and its status. */
  const chosen = $derived(app.agentInfo);
  const agent = $derived(app.agent);
  const statusOf = (id: Agent) => app.agents?.find((a) => a.agent === id) ?? null;
  function badgeOf(status: AgentStatus | null) {
    if (!status) return { tone: "neutral", text: checking ? "Checking…" : "Not checked" } as const;
    if (!status.found) return { tone: "danger", text: "Not found" } as const;
    if (status.signedIn === false) return { tone: "danger", text: "Signed out" } as const;
    if (status.signedIn === null) return { tone: "neutral", text: "Sign-in unknown" } as const;
    return { tone: "gain", text: "Signed in" } as const;
  }
  /** "Anthropic · 2.1.0 · Max" once found; before it's checked, whose plan it runs on. */
  function metaOf(info: AgentInfo, status: AgentStatus | null) {
    if (!status) return `${info.maker}, ${info.plan}`;
    if (!status.found) return `${info.maker} · not installed on this Mac`;
    return [info.maker, status.version, status.plan].filter(Boolean).join(" · ");
  }
  const agentBadge = $derived(badgeOf(agent));
  /** Reading a resume runs the agent: known not to be ready, the Experience step says so before you try. */
  const agentMissing = $derived(agentBadge.tone === "danger");

  function chooseAgent(id: Agent) {
    if (app.settings?.agent !== id) void save({ agent: id });
  }

  function visit(url: string) {
    openUrl(url).catch((e) => toast(`Couldn’t open ${url}: ${appError(e).message}`, { tone: "danger" }));
  }

  // ── What it says ──
  type FilterKey = "seniority" | "employment" | "category" | "countries";
  const AUTHORIZATION = [
    { value: "authorized", label: "Authorized" },
    { value: "sponsor", label: "Need sponsorship" },
  ];
  const CLEARANCE = [
    { value: "include", label: "Include" },
    { value: "leave", label: "Leave out" },
  ];

  let feedView = $state<FeedSettingsView | null>(null);
  let saving = $state(false);
  /** Bumped when a save fails, so the controls redraw from what's stored. */
  let resets = $state(0);
  /** The filter whose options are open. */
  let editing = $state<FilterKey | null>(null);
  const fs = $derived(feedView?.settings);

  async function loadFeed() {
    try {
      feedView = await feedSettings();
    } catch (e) {
      console.error(`feed settings: ${appError(e).message}`);
    }
  }

  async function saveFeed(patch: Partial<FeedSettingsDto>) {
    if (!feedView || saving) return;
    saving = true;
    try {
      feedView = await setFeedSettings({ ...feedView.settings, ...patch });
      // The filters change what's a fit, and pulling may have just been turned on.
      void feed.refresh();
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
      resets++;
    } finally {
      saving = false;
    }
  }

  const years = $derived(
    feedView ? (feedView.auto.years === 1 ? "1 year" : `${Math.round(feedView.auto.years * 10) / 10} years`) : "",
  );

  /**
   * Each filter as the feed has it: your choice, else what Auto works out from the bank (None in settings, so it keeps
   * following the bank), and where that came from.
   */
  const filters = $derived.by(() => {
    if (!feedView) return [];
    const { settings: chosen, auto } = feedView;
    // Countries: the usual ones, then any other you or Auto have.
    const countries = [...new Set([...COUNTRIES, ...auto.countries, ...(chosen.countries ?? [])])].map((c) => ({
      value: c,
      label: countryName(c),
    }));
    const rows: { key: FilterKey; label: string; any: string; options: { value: string; label: string }[] }[] = [
      { key: "seniority", label: "Seniority", any: "Any level", options: SENIORITY },
      { key: "employment", label: "Employment", any: "Any kind", options: EMPLOYMENT },
      { key: "category", label: "Category", any: "Any field", options: CATEGORY },
      { key: "countries", label: "Countries", any: "Any country", options: countries },
    ];
    return rows.map((row) => {
      const custom = chosen[row.key];
      const values = custom ?? auto[row.key];
      const words = values.map((v) => row.options.find((o) => o.value === v)?.label ?? v).join(", ");
      const from =
        custom !== null
          ? "your choice"
          : !bank
            ? "no resume yet"
            : !values.length
              ? "not in your resume"
              : row.key === "seniority"
                ? `from ${years} of experience`
                : "from your resume";
      return { ...row, custom, values, value: words || row.any, from };
    });
  });

  /** The first change to a filter starts from what Auto worked out, so only what you touch becomes yours. */
  function toggle(key: FilterKey, value: string, on: boolean) {
    const row = filters.find((f) => f.key === key);
    if (!row) return;
    const chosen = new Set(row.values);
    if (on) chosen.add(value);
    else chosen.delete(value);
    void saveFeed({ [key]: row.options.map((o) => o.value).filter((v) => chosen.has(v)) });
  }

  // What each step needs fresh as you arrive: the agent may have been installed meanwhile (and the Experience step
  // reads a resume through it); the feed's Auto follows the bank you just imported.
  $effect(() => {
    if (step === "agent" || step === "resume" || step === "ready") untrack(() => void checkAgents());
    if (step === "says" || step === "ready") untrack(() => void loadFeed());
  });

  // ── Appearance ──
  async function save(patch: Partial<Settings>) {
    if (!app.settings) return;
    try {
      await app.saveSettings({ ...app.settings, ...patch });
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
      theme.apply(app.settings.theme);
    }
  }

  function setTheme(t: Theme) {
    // Applied before the save returns, so the whole window changes at once.
    theme.apply(t);
    void save({ theme: t });
  }

  // ── The footer's one way on ──
  const onward = $derived.by(() => {
    if (step === "welcome") return { label: "Get started", primary: true };
    if (step === "resume" && !bank) return { label: "Skip for now", primary: false };
    if (step === "ready") return { label: "Open the Overview", primary: true };
    return { label: "Continue", primary: true };
  });

  $effect(() =>
    bindKeys({
      // A focused control keeps its own ⏎.
      Enter: (e) => {
        if (e.target instanceof Element && e.target.closest("button, a, input")) return false;
        next();
      },
      "mod+[": () => go(at - 1),
      i: () => {
        if (step !== "resume") return false;
        void importResume();
      },
      r: () => {
        if (step !== "agent") return false;
        void checkAgents();
      },
    }),
  );
</script>

{#snippet intro(title: string, body: string)}
  <header class="intro">
    <h1 class="headline turn" id="step-{step}" tabindex="-1" style:--i="0">{title}</h1>
    <p class="lede turn" style:--i="1">{body}</p>
  </header>
{/snippet}

<Toolbar over clear={!scrolled}>
  {#snippet crumbs()}{/snippet}
  {#if step !== "ready"}<Button variant="ghost" onclick={finish}>Skip setup</Button>{/if}
</Toolbar>

<div class="scroll" bind:this={scroller} onscroll={measure}>
  <div class="column">
    <Steps steps={NAMES} current={at} label="Setup" onselect={go} />

    <div class="stage">
      {#key step}
        <section
          class="page"
          aria-labelledby="step-{step}"
          style:--side={forward ? 1 : -1}
          style:--wait={turned ? "var(--dur-fast)" : "0ms"}
          out:exit={{ side: forward ? -1 : 1 }}
        >
          {#if step === "welcome"}
            {@render intro(
              "Your experience, fitted to every job",
              "Paste a posting. gliide measures how well you fit, suggests the few honest edits that raise it, and exports for you.",
            )}
            <ul class="rows rows--bare">
              {#each PROMISES as promise, i (promise.title)}
                <li class="row turn" style:--i={2 + i}>
                  <span class="lead"><Icon icon={promise.icon} weight="medium" /></span>
                  <span class="row__name">{promise.title}</span>
                  <span class="row__sub">{promise.body}</span>
                </li>
              {/each}
            </ul>
          {:else if step === "resume"}
            {@render intro(
              "Bring in your resume",
              "gliide reads it into your experience, so every edit it suggests comes from what you’ve done.",
            )}
            {#if bank}
              <Card class="turn" style="--i: 2">
                <div class="rows">
                  <div class="row row--status">
                    <span class="lead lead--gain"><Icon icon={Check} weight="medium" /></span>
                    <span class="row__name">{bank.name || "Your experience bank"}</span>
                    <span class="row__sub num">
                      {plural(bank.entries, "entry", "entries")} and {plural(bank.bullets, "bullet", "bullets")}
                      {#if masters.length}· {masters.map((m) => m.title).join(", ")}{/if}
                    </span>
                    <span class="row__end"><Button kbd="I" onclick={importResume}>Import another</Button></span>
                  </div>
                </div>
              </Card>
            {:else}
              <FileDrop
                class="turn"
                style="--i: 2"
                data-resume-drop
                icon={FileUp}
                over={dropOver}
                text={dropOver ? "Drop to import it" : "Drop your resume here"}
                hint="{RESUME_FORMATS}. You check every line before it’s added."
              >
                <Button variant="primary" kbd="I" onclick={importResume}>Choose a file…</Button>
              </FileDrop>
              {#if agentMissing}
                <div class="guard turn" style:--i="3">
                  <p class="meta">Reading a resume takes an agent, and {chosen.name} isn’t ready yet.</p>
                  <Button size="sm" onclick={() => go(indexOf("agent"))}>Set up an agent</Button>
                </div>
              {/if}
              <section class="group turn" style:--i={agentMissing ? 4 : 3} aria-labelledby="build-label">
                <h2 class="label" id="build-label">No resume yet?</h2>
                <Card>
                  <div class="rows">
                    <div class="row row--status row--later">
                      <span class="lead"><Icon icon={MessagesSquare} weight="medium" /></span>
                      <span class="row__name">Build one with your agent</span>
                      <span class="row__sub">Chat with it: pick a format, or have it start from a master or a resume type.</span>
                      <span class="row__end"><Badge>Coming soon</Badge></span>
                    </div>
                  </div>
                </Card>
              </section>
            {/if}
          {:else if step === "says"}
            {@render intro(
              bank ? "What your resume says" : "What to look for",
              bank
                ? "The feed looks for jobs like these. Change anything that’s off."
                : "Import your resume and gliide works these out from it. Until then, pick them here.",
            )}
            {#if fs}
              <Card class="turn" style="--i: 2">
                <ul class="rows">
                  <!-- Keyed on `resets` too, so a failed save redraws the checkboxes from what's stored. -->
                  {#each filters as filter (`${filter.key}:${resets}`)}
                    {@const open = editing === filter.key}
                    <li class="pick">
                      <div class="field">
                        <span class="field__text">
                          <span class="row__name">{filter.label}</span>
                          <span class="row__sub">{filter.value}<span class="from">{` · ${filter.from}`}</span></span>
                        </span>
                        <Button
                          size="sm"
                          variant="ghost"
                          aria-expanded={open}
                          aria-controls="pick-{filter.key}"
                          onclick={() => (editing = open ? null : filter.key)}
                        >
                          {open ? "Done" : "Change"}
                        </Button>
                      </div>
                      {#if open}
                        <div class="pick__options" id="pick-{filter.key}">
                          <div class="options">
                            {#each filter.options as option (option.value)}
                              {@const on = filter.values.includes(option.value)}
                              <label class="option">
                                <Checkbox
                                  checked={on}
                                  label={option.label}
                                  disabled={saving || (on && filter.values.length === 1)}
                                  onchange={(next) => toggle(filter.key, option.value, next)}
                                />
                                <span>{option.label}</span>
                              </label>
                            {/each}
                          </div>
                          {#if filter.custom !== null && bank}
                            <Button size="sm" variant="ghost" disabled={saving} onclick={() => saveFeed({ [filter.key]: null })}>
                              Use what your resume says
                            </Button>
                          {/if}
                        </div>
                      {/if}
                    </li>
                  {/each}
                </ul>
              </Card>
              <section class="group turn" style:--i="3" aria-labelledby="yours-label">
                <h2 class="label" id="yours-label">Only you can say</h2>
                {#key resets}
                  <Card>
                    <div class="rows">
                      <div class="field">
                        <span class="field__text">
                          <span class="row__name">Work authorization</span>
                          <span class="row__sub">With Need sponsorship, the feed keeps only jobs that sponsor a visa.</span>
                        </span>
                        <Segmented
                          options={AUTHORIZATION}
                          value={fs.needsVisa ? "sponsor" : "authorized"}
                          label="Work authorization"
                          disabled={saving}
                          onchange={(v) => saveFeed({ needsVisa: v === "sponsor" })}
                        />
                      </div>
                      <div class="field">
                        <span class="field__text">
                          <span class="row__name">Clearance roles</span>
                          <span class="row__sub">Jobs that need a security clearance.</span>
                        </span>
                        <Segmented
                          options={CLEARANCE}
                          value={fs.includeClearance ? "include" : "leave"}
                          label="Clearance roles"
                          disabled={saving}
                          onchange={(v) => saveFeed({ includeClearance: v === "include" })}
                        />
                      </div>
                      <label class="field field--check">
                        <span class="field__text">
                          <span class="row__name">Pull new postings</span>
                          <span class="row__sub num">
                            At launch and every {plural(fs.everyHours, "hour", "hours")}, up to {fs.dailyChecks} checked
                            a day {chosen.plan}.
                          </span>
                        </span>
                        <Checkbox
                          checked={fs.enabled}
                          label="Pull new postings"
                          disabled={saving}
                          onchange={(on) => saveFeed({ enabled: on })}
                        />
                      </label>
                    </div>
                  </Card>
                {/key}
              </section>
              <p class="note meta turn" style:--i="4">All of it is in Settings → Feed too.</p>
            {/if}
          {:else if step === "agent"}
            {@render intro(
              "Choose your agent",
              "gliide reads your resume and postings, and suggests edits, through the agent you choose, signed in on your own plan. It never sees a credential.",
            )}
            <Card class="turn" style="--i: 2">
              <div class="rows">
                <div class="rows choices" role="radiogroup" aria-label="The agent model calls run on">
                  {#each AGENT_LIST as info (info.id)}
                    {@const status = statusOf(info.id)}
                    {@const badge = badgeOf(status)}
                    {@const on = chosen.id === info.id}
                    <label class={["row", "row--status", "row--choice", on && "row--chosen"]}>
                      <input
                        class="visually-hidden"
                        type="radio"
                        name="agent"
                        value={info.id}
                        checked={on}
                        disabled={!app.settings}
                        onchange={() => chooseAgent(info.id)}
                      />
                      <span class="lead"><CompanyMark size={28} src={info.icon} /></span>
                      <span class="row__name">{info.name}</span>
                      <span class="row__sub num">{metaOf(info, status)}</span>
                      <span class="row__end row__end--badge">
                        <Badge tone={badge.tone} dot>{badge.text}</Badge>
                        <span class={["chosen", on && "chosen--on"]} aria-hidden="true"
                          ><Icon icon={Check} weight="medium" /></span
                        >
                      </span>
                    </label>
                  {/each}
                </div>
                {#if agent?.problem}<p class="problem">{agent.problem}</p>{/if}
                <div class="actions">
                  {#if agent && !agent.found}
                    <Button icon={ExternalLink} onclick={() => visit(chosen.install)}>How to install {chosen.name}</Button>
                  {/if}
                  <Button icon={RotateCw} kbd="R" disabled={checking} onclick={checkAgents}>
                    {checking ? "Checking…" : "Check again"}
                  </Button>
                </div>
              </div>
            </Card>
            {#if agentBadge.tone !== "gain"}
              <p class="note meta turn" style:--i="3">
                You can go on and set it up later in Settings. Until then, resumes can’t be read and postings can’t be
                scored.
              </p>
            {/if}
          {:else if step === "look"}
            {@render intro(
              "Choose how it looks",
              "The window changes as you choose. Settings → Appearance has this and the Overview’s backdrop.",
            )}
            <div class="field field--bare turn" style:--i="2">
              <span class="field__text">
                <span class="row__name">Theme</span>
                <span class="row__sub">Light is paper and ink; System follows your Mac.</span>
              </span>
              <Segmented
                options={THEMES}
                value={app.settings?.theme ?? theme.current}
                label="Theme"
                disabled={!app.settings}
                onchange={setTheme}
              />
            </div>
          {:else}
            {@render intro(
              firstName ? `You’re set, ${firstName}` : "You’re set",
              "Copy a job link or a posting, then press V on the Overview to see how well you fit.",
            )}
            <section class="group turn" style:--i="2" aria-labelledby="you-label">
              <h2 class="label" id="you-label">You</h2>
              <Card>
                <ul class="rows">
                  <li>
                    <button type="button" class="row row--status row--button" onclick={() => go(indexOf("resume"))}>
                      <span class={["lead", bank && "lead--accent"]}>
                        <Icon icon={bank ? UserRound : CircleDashed} weight="medium" />
                      </span>
                      <span class="row__name">{bank?.name || "Experience"}</span>
                      <span class="row__sub num">
                        {#if bank}
                          {[
                            feedView && feedView.auto.years > 0 ? `${years} of experience` : "",
                            `${plural(bank.entries, "entry", "entries")}, ${plural(bank.bullets, "bullet", "bullets")}`,
                          ]
                            .filter(Boolean)
                            .join(" · ")}
                        {:else}
                          No resume imported yet
                        {/if}
                      </span>
                      <span class="row__end meta">{bank ? "Change" : "Set up"}</span>
                    </button>
                  </li>
                  <li>
                    <button type="button" class="row row--status row--button" onclick={() => go(indexOf("says"))}>
                      <span class="lead lead--accent"><Icon icon={Search} weight="medium" /></span>
                      <span class="row__name">Looking for</span>
                      <span class="row__sub">{filters.length ? filters.map((f) => f.value).join(" · ") : "—"}</span>
                      <span class="row__end meta">Change</span>
                    </button>
                  </li>
                </ul>
              </Card>
            </section>
            <section class="group turn" style:--i="3" aria-labelledby="tools-label">
              <h2 class="label" id="tools-label">Your agent</h2>
              <Card>
                <ul class="rows">
                  <li>
                    <button type="button" class="row row--status row--button" onclick={() => go(indexOf("agent"))}>
                      <span class="lead"><CompanyMark size={28} src={chosen.icon} /></span>
                      <span class="row__name">{chosen.name}</span>
                      <span class="row__sub num">{metaOf(chosen, agent)}</span>
                      <span class="row__end row__end--badge">
                        <Badge tone={agentBadge.tone} dot>{agentBadge.text}</Badge>
                        <span class="meta">{agentBadge.tone === "gain" ? "Change" : "Set up"}</span>
                      </span>
                    </button>
                  </li>
                </ul>
              </Card>
            </section>
            {#if fs}
              <p class="note meta turn" style:--i="4">
                {fs.enabled
                  ? "The feed is pulling new postings; the ones that fit show up in New for you on the Overview."
                  : "The feed is off. Turn it on in Settings → Feed."}
              </p>
            {/if}
          {/if}
        </section>
      {/key}
    </div>
  </div>

  <footer class={["foot", stuck && "foot--stuck"]}>
    <div class="foot__in">
      {#if at > 0}<Button variant="ghost" kbd="⌘[" onclick={() => go(at - 1)}>Back</Button>{/if}
      <span class="foot__space"></span>
      <Button variant={onward.primary ? "primary" : "default"} kbd="⏎" onclick={next}>{onward.label}</Button>
    </div>
  </footer>
</div>

<style>
  /* The footer scrolls with the step, right under it, and holds at the window's foot once the step is taller. */
  .scroll {
    position: relative;
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
    overflow: auto;
    overscroll-behavior: contain;
  }
  /* Starts under the toolbar, which lies over it unpainted. */
  .column {
    display: grid;
    gap: var(--space-10);
    width: 100%;
    max-width: 608px;
    margin-inline: auto;
    padding: calc(var(--toolbar-height) + var(--space-8)) var(--space-6) var(--space-8);
  }

  /*
   * The leaving step and the arriving one share the cell while they cross. Its least height is a short step's, so the
   * footer stays put from one to the next and moves only for a taller one.
   */
  .stage {
    display: grid;
    min-height: 320px;
  }
  .page {
    grid-area: 1 / 1;
    display: grid;
    align-content: start;
    gap: var(--space-6);
    min-width: 0;
  }

  .intro {
    display: grid;
    gap: var(--space-2);
  }
  /* Focused by script as the step arrives, for assistive tech; it isn't a control. */
  .headline:focus-visible {
    outline: none;
  }
  .lede {
    margin: 0;
    max-width: 52ch;
    font-size: var(--text-md);
    line-height: var(--leading-md);
    letter-spacing: var(--tracking-md);
    color: var(--ink-2);
    text-wrap: pretty;
  }
  .note {
    margin: 0;
    text-wrap: pretty;
  }
  /* A labeled card: the label over it. */
  .group {
    display: grid;
    gap: var(--space-2);
  }
  .group > .label {
    margin: 0;
  }
  /* What isn't here yet: names, lines and icons in ink 3. */
  .row--later .row__name,
  .row--later .row__sub,
  .row--later .lead {
    color: var(--ink-3);
  }

  /* Card rows: hairlines between, none at the card's edge. Inside its 1px border: 12 - 1. */
  .rows {
    display: grid;
    margin: 0;
    padding: 0;
    overflow: hidden;
    border-radius: calc(var(--radius-lg) - 1px);
    list-style: none;
  }
  .rows > :global(* + *) {
    box-shadow: inset 0 1px 0 var(--line);
  }
  /* On paper, no card: the rows on the headline's edge, no hairlines, a group's space between. */
  .rows--bare {
    gap: var(--space-5);
    overflow: visible;
    border-radius: 0;
  }
  .rows--bare > :global(* + *) {
    box-shadow: none;
  }
  .rows--bare .row {
    padding: 0;
  }

  /* An icon in a 28px box, a name over one line of meta, and whatever ends the row centered on both. */
  .row {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    grid-template-areas:
      "lead name"
      "lead sub";
    align-items: center;
    column-gap: var(--space-3);
    padding: var(--space-3) var(--space-5);
  }
  .row--status {
    grid-template-columns: auto minmax(0, 1fr) auto;
    grid-template-areas:
      "lead name end"
      "lead sub end";
  }
  .row--button {
    width: 100%;
    border: 0;
    background: none;
    font: inherit;
    text-align: start;
    color: inherit;
    cursor: default;
    transition: background-color var(--dur-instant) var(--ease-out);
  }
  .lead {
    grid-area: lead;
    display: flex;
    width: 28px;
    height: 28px;
    align-items: center;
    justify-content: center;
    color: var(--accent-text);
  }
  .lead--gain {
    color: var(--gain-text);
  }
  .row--button .lead:not(.lead--gain, .lead--accent) {
    color: var(--ink-3);
  }
  .row__name {
    grid-area: name;
    min-width: 0;
    font-weight: var(--weight-medium);
  }
  .row__sub {
    grid-area: sub;
    min-width: 0;
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    color: var(--ink-2);
    text-wrap: pretty;
  }
  .row__end {
    grid-area: end;
    display: flex;
    margin-inline-start: var(--space-2);
  }
  .row__end--badge {
    align-items: center;
    gap: var(--space-3);
  }

  /* The agents, a choice of one: the chosen row in the selection wash with a check at its end (the radio itself is
     hidden; its focus ring is the row's, inset). The check changes at once; the wash as a row's hover does. */
  .choices {
    overflow: visible;
    border-radius: 0;
  }
  .row--choice {
    position: relative;
    cursor: default;
    transition: background-color var(--dur-instant) var(--ease-out);
  }
  .row--choice:has(:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .row--chosen {
    background: var(--selected);
  }
  .chosen {
    display: flex;
    color: var(--accent-text);
    visibility: hidden;
  }
  .chosen--on {
    visibility: visible;
  }

  .field {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-3) var(--space-5);
  }
  .field--bare {
    padding: 0;
  }
  .field__text {
    display: grid;
    flex: 1;
    min-width: 0;
  }
  /* A field's name over its sentence: one column, none of the row's areas. */
  .field__text > * {
    grid-area: auto;
  }
  .field--check {
    cursor: default;
  }

  /* A filter: where its value came from in ink 3, after it; open, its options under it. */
  .pick {
    display: grid;
  }
  .from {
    color: var(--ink-3);
  }
  .pick__options {
    display: grid;
    justify-items: start;
    gap: var(--space-2);
    padding: 0 var(--space-5) var(--space-4);
  }
  .options {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: var(--space-2) var(--space-4);
    width: 100%;
  }
  .option {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
    cursor: default;
  }

  .guard {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2) var(--space-3);
  }
  .guard p {
    flex: 1;
    margin: 0;
    min-width: 0;
    text-wrap: pretty;
  }

  .problem {
    margin: 0;
    padding: var(--space-3) var(--space-5);
    color: var(--ink-2);
    text-wrap: pretty;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    padding: var(--space-3) var(--space-5);
  }

  .foot {
    position: sticky;
    bottom: 0;
    flex: none;
    background: var(--paper);
  }
  .foot--stuck {
    box-shadow: inset 0 1px 0 var(--line);
  }
  .foot__in {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    max-width: 608px;
    min-height: var(--toolbar-height);
    margin-inline: auto;
    padding: var(--space-3) var(--space-6);
  }
  .foot__space {
    flex: 1;
  }

  @media (hover: hover) {
    .row--button:hover,
    .row--choice:not(.row--chosen):hover {
      background: var(--hover);
    }
  }
</style>
