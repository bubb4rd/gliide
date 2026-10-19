<!--
  /welcome: the first-run setup, in a window without a sidebar. Six steps under one progress bar (`Steps`): Welcome
  (what gliide does and its three promises), Appearance (theme and Overview backdrop, applied as you choose), Claude
  Code (found and signed in; checked again as you arrive and on R), Experience (import a resume; the import review
  comes back here), Feed (pulling new postings, and what it looks for) and Ready (what's set, each a way back to its
  step, and the keys to start with). The step is in the URL (`?step=feed`) so the import review can come back to it.
  ⏎ goes on, ⌘[ goes back, a done step in the bar goes back to it; Skip setup and the last step's button end the setup
  for good. A step leaves sideways toward where you came from and the next enters from where you're going, its parts
  100ms apart; with reduced motion they only fade.
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
  import RotateCw from "@lucide/svelte/icons/rotate-cw";
  import ShieldCheck from "@lucide/svelte/icons/shield-check";
  import { Badge, Button, Card, Checkbox, CompanyMark, Icon, Kbd, Segmented, Sources, Steps, Toolbar, exit, toast } from "$lib/ui";
  import {
    appError,
    feedSettings,
    setFeedSettings,
    type Backdrop,
    type FeedSettingsView,
    type Settings,
    type Theme,
  } from "$lib/api";
  import { CATEGORY, EMPLOYMENT, SEARCH_SOURCES_NOTE, SENIORITY, countryName, searchSources } from "$lib/feed";
  import { chooseResume } from "$lib/import";
  import { bindKeys } from "$lib/keys";
  import { app } from "$lib/stores/app.svelte";
  import { feed } from "$lib/stores/feed.svelte";
  import { theme } from "$lib/stores/theme.svelte";

  /** Agents gliide will run once they're supported (ARCHITECTURE.md §1): shown, grayed, so you know what's coming. */
  const LATER = [
    { name: "Codex", maker: "OpenAI, with your ChatGPT plan", src: "/agents/openai.png" },
    { name: "Gemini CLI", maker: "Google, with your Google AI plan", src: "/agents/gemini.png" },
  ];

  const STEPS = [
    { id: "welcome", name: "Welcome" },
    { id: "look", name: "Appearance" },
    { id: "agent", name: "Claude Code" },
    { id: "resume", name: "Experience" },
    { id: "feed", name: "Feed" },
    { id: "ready", name: "Ready" },
  ] as const;
  type StepId = (typeof STEPS)[number]["id"];
  const NAMES = STEPS.map((s) => s.name);
  const THEMES: { value: Theme; label: string }[] = [
    { value: "light", label: "Light" },
    { value: "dark", label: "Dark" },
    { value: "system", label: "System" },
  ];
  const BACKDROPS: { value: Backdrop; label: string }[] = [
    { value: "none", label: "None" },
    { value: "meadow", label: "Meadow" },
    { value: "tokyo", label: "Tokyo" },
    { value: "sakura", label: "Sakura" },
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
  const INSTALL = "https://code.claude.com/docs/en/setup";

  const at = $derived(
    Math.max(
      0,
      STEPS.findIndex((s) => s.id === page.url.searchParams.get("step")),
    ),
  );
  const step = $derived<StepId>(STEPS[at].id);
  /** The way you last moved, which sets the side a step leaves by and the next enters from. */
  let forward = $state(true);
  /** A step has left: the next waits for it. The first on screen doesn't. */
  let turned = $state(false);
  /** The content has scrolled under the toolbar, which then needs its paper. */
  let scrolled = $state(false);

  function go(index: number) {
    if (index < 0 || index >= STEPS.length || index === at) return;
    forward = index > at;
    turned = true;
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

  // ── Claude Code ──
  let checking = $state(false);

  async function checkAgent() {
    if (checking) return;
    checking = true;
    await app.refreshAgent();
    checking = false;
  }

  const agent = $derived(app.agent);
  const agentBadge = $derived.by(() => {
    if (!agent) return { tone: "neutral", text: checking ? "Checking…" : "Not checked" } as const;
    if (!agent.found) return { tone: "danger", text: "Not found" } as const;
    if (agent.signedIn === false) return { tone: "danger", text: "Signed out" } as const;
    if (agent.signedIn === null) return { tone: "neutral", text: "Sign-in unknown" } as const;
    return { tone: "gain", text: "Signed in" } as const;
  });
  const agentMeta = $derived(
    agent?.found ? [agent.version, agent.plan].filter(Boolean).join(" · ") || "Found" : "Not installed on this Mac",
  );

  function visit(url: string) {
    openUrl(url).catch((e) => toast(`Couldn’t open ${url}: ${appError(e).message}`, { tone: "danger" }));
  }

  // ── Experience ──
  const bank = $derived(app.library?.bank ?? null);
  const masters = $derived(app.library?.masters ?? []);
  /** "Jane (JJ) Doe" → "Bo"; else the first name (as the Overview greets you). */
  const firstName = $derived(
    bank ? bank.name.match(/\(([^)]+)\)/)?.[1]?.trim() || bank.name.trim().split(/\s+/)[0] || "" : "",
  );
  const importResume = () => chooseResume(app.settings?.exportDir, "/welcome?step=resume");
  const plural = (n: number, one: string, many: string) => `${n.toLocaleString()} ${n === 1 ? one : many}`;

  // ── Feed ──
  let feedView = $state<FeedSettingsView | null>(null);
  let pulling = $state(false);

  async function loadFeed() {
    try {
      feedView = await feedSettings();
    } catch (e) {
      console.error(`feed settings: ${appError(e).message}`);
    }
  }

  async function setPull(on: boolean) {
    if (!feedView || pulling) return;
    pulling = true;
    try {
      feedView = await setFeedSettings({ ...feedView.settings, enabled: on });
      void feed.refresh();
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
    } finally {
      pulling = false;
    }
  }

  /** What the feed looks for, as Settings → Feed has it: your choices, else what Auto works out from the bank. */
  const looksFor = $derived.by(() => {
    if (!feedView || !bank) return [];
    const { settings: chosen, auto } = feedView;
    const words = (values: string[], options: { value: string; label: string }[]) =>
      values.map((v) => options.find((o) => o.value === v)?.label ?? v).join(", ");
    return [
      { label: "Seniority", value: words(chosen.seniority ?? auto.seniority, SENIORITY) },
      { label: "Employment", value: words(chosen.employment ?? auto.employment, EMPLOYMENT) },
      { label: "Category", value: words(chosen.category ?? auto.category, CATEGORY) },
      { label: "Countries", value: (chosen.countries ?? auto.countries).map(countryName).join(", ") || "Anywhere" },
    ].map((row) => ({ ...row, value: row.value || "Any" }));
  });
  const years = $derived(
    feedView ? (feedView.auto.years === 1 ? "1 year" : `${Math.round(feedView.auto.years * 10) / 10} years`) : "",
  );

  // What each step needs fresh as you arrive: Claude Code may have been installed meanwhile; the feed's Auto follows
  // the bank you just imported.
  $effect(() => {
    if (step === "agent") untrack(() => void checkAgent());
    if (step === "feed" || step === "ready") untrack(() => void loadFeed());
  });

  // ── Ready ──
  const indexOf = (id: StepId) => STEPS.findIndex((s) => s.id === id);
  const summary = $derived([
    {
      step: indexOf("look"),
      name: "Appearance",
      value: [
        THEMES.find((t) => t.value === theme.current)?.label ?? "Light",
        app.settings && app.settings.backdrop !== "none"
          ? `${BACKDROPS.find((b) => b.value === app.settings?.backdrop)?.label} backdrop`
          : "",
      ]
        .filter(Boolean)
        .join(" · "),
      done: true,
    },
    { step: indexOf("agent"), name: "Claude Code", value: agentBadge.text, done: agentBadge.tone === "gain" },
    {
      step: indexOf("resume"),
      name: "Experience",
      value: bank
        ? `${plural(bank.entries, "entry", "entries")}, ${plural(bank.bullets, "bullet", "bullets")}`
        : "Not imported yet",
      done: !!bank,
    },
    {
      step: indexOf("feed"),
      name: "Feed",
      value: feedView ? (feedView.settings.enabled ? "Pulling new postings" : "Off") : "—",
      done: !!feedView?.settings.enabled,
    },
  ]);

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
        void checkAgent();
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

<div class="scroll" onscroll={(e) => (scrolled = e.currentTarget.scrollTop > 0)}>
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
              "Your resume, fitted to every job",
              "Paste a posting and gliide scores how well you fit, proposes the few honest edits that raise it, and exports a one-page PDF.",
            )}
            <Card class="turn" style="--i: 2">
              <ul class="rows">
                {#each PROMISES as promise (promise.title)}
                  <li class="row">
                    <span class="lead"><Icon icon={promise.icon} weight="medium" /></span>
                    <span class="row__name">{promise.title}</span>
                    <span class="row__sub">{promise.body}</span>
                  </li>
                {/each}
              </ul>
            </Card>
          {:else if step === "look"}
            {@render intro(
              "Choose how it looks",
              "The window changes as you choose. Both can be changed later in Settings → Appearance.",
            )}
            <Card class="turn" style="--i: 2">
              <div class="rows">
                <div class="field">
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
                <div class="field">
                  <span class="field__text">
                    <span class="row__name">Overview backdrop</span>
                    <span class="row__sub">A painting behind the top of the Overview, veiled so text reads on it.</span>
                  </span>
                  <Segmented
                    options={BACKDROPS}
                    value={app.settings?.backdrop ?? "none"}
                    label="Overview backdrop"
                    disabled={!app.settings}
                    onchange={(backdrop) => void save({ backdrop })}
                  />
                </div>
              </div>
            </Card>
          {:else if step === "agent"}
            {@render intro(
              "Connect Claude Code",
              "gliide reads postings and suggests edits through the Claude Code you’re signed in to, on your own plan. It never sees a credential.",
            )}
            <Card class="turn" style="--i: 2">
              <div class="rows">
                <div class="row row--status">
                  <span class="lead"><CompanyMark size={28} src="/agents/claude.png" /></span>
                  <span class="row__name">Claude Code</span>
                  <span class="row__sub num">{agentMeta}</span>
                  <span class="row__end"><Badge tone={agentBadge.tone} dot>{agentBadge.text}</Badge></span>
                </div>
                {#if agent?.problem}<p class="problem">{agent.problem}</p>{/if}
                <div class="actions">
                  {#if agent && !agent.found}
                    <Button icon={ExternalLink} onclick={() => visit(INSTALL)}>How to install</Button>
                  {/if}
                  <Button icon={RotateCw} kbd="R" disabled={checking} onclick={checkAgent}>
                    {checking ? "Checking…" : "Check again"}
                  </Button>
                </div>
              </div>
            </Card>
            <section class="later turn" style:--i="3" aria-labelledby="later-label">
              <h2 class="label" id="later-label">Other agents</h2>
              <Card>
                <ul class="rows">
                  {#each LATER as agent (agent.name)}
                    <li class="row row--status row--later">
                      <span class="lead"><CompanyMark size={28} src={agent.src} off /></span>
                      <span class="row__name">{agent.name}</span>
                      <span class="row__sub">{agent.maker}</span>
                      <span class="row__end"><Badge>Not supported yet</Badge></span>
                    </li>
                  {/each}
                </ul>
              </Card>
            </section>
            {#if agentBadge.tone !== "gain"}
              <p class="note meta turn" style:--i="4">
                You can go on and connect it later in Settings. Until then, postings can’t be scored.
              </p>
            {/if}
          {:else if step === "resume"}
            {@render intro(
              "Bring in your experience",
              "Import the resume you send today. Its lines become your experience bank, and the resume itself your first master.",
            )}
            <Card class="turn" style="--i: 2">
              <div class="rows">
                {#if bank}
                  <div class="row row--status">
                    <span class="lead lead--gain"><Icon icon={Check} weight="medium" /></span>
                    <span class="row__name">{bank.name || "Your experience bank"}</span>
                    <span class="row__sub num">
                      {plural(bank.entries, "entry", "entries")} and {plural(bank.bullets, "bullet", "bullets")}
                      {#if masters.length}· {masters.map((m) => m.title).join(", ")}{/if}
                    </span>
                    <span class="row__end"><Button kbd="I" onclick={importResume}>Import another</Button></span>
                  </div>
                {:else}
                  <div class="row row--status">
                    <span class="lead"><Icon icon={FileUp} weight="medium" /></span>
                    <span class="row__name">Your resume</span>
                    <span class="row__sub">PDF, Word, Markdown or text. You check every line before it’s added.</span>
                    <span class="row__end">
                      <Button variant="primary" kbd="I" onclick={importResume}>Choose a file…</Button>
                    </span>
                  </div>
                {/if}
              </div>
            </Card>
          {:else if step === "feed"}
            {@render intro(
              "Find postings that fit",
              "The feed pulls new postings on its own, checks each one against your resume, and keeps the ones that fit.",
            )}
            <Card class="turn" style="--i: 2">
              <div class="rows">
                <label class="field field--check">
                  <span class="field__text">
                    <span class="row__name">Pull new postings</span>
                    <span class="row__sub num">
                      {#if feedView}
                        At launch and every {plural(feedView.settings.everyHours, "hour", "hours")}, up to
                        {feedView.settings.dailyChecks} checked a day.
                      {:else}
                        At launch and every few hours.
                      {/if}
                    </span>
                  </span>
                  <Checkbox
                    checked={feedView?.settings.enabled ?? false}
                    label="Pull new postings"
                    disabled={!feedView || pulling}
                    onchange={setPull}
                  />
                </label>
                <div class="field field--sources">
                  <Sources label="Pulls from" sources={searchSources(!!feedView?.settings.linkedin)} note={SEARCH_SOURCES_NOTE} />
                </div>
                {#if looksFor.length}
                  <dl class="looks">
                    {#each looksFor as row (row.label)}
                      <div class="look">
                        <dt class="meta">{row.label}</dt>
                        <dd>{row.value}</dd>
                      </div>
                    {/each}
                  </dl>
                {/if}
              </div>
            </Card>
            <p class="note meta turn" style:--i="3">
              {#if bank && feedView}
                Worked out from your {years} of experience. Change any of it in Settings → Feed.
              {:else}
                Import your resume first, and the feed works out what to look for from it.
              {/if}
            </p>
          {:else}
            {@render intro(
              firstName ? `You’re set, ${firstName}` : "You’re set",
              "Copy a job link or a posting’s text, then press V on the Overview: the score shows up in seconds.",
            )}
            <Card class="turn" style="--i: 2">
              <ul class="rows">
                {#each summary as item (item.name)}
                  <li>
                    <button type="button" class="row row--status row--button" onclick={() => go(item.step)}>
                      <span class={["lead", item.done && "lead--gain"]}>
                        <Icon icon={item.done ? Check : CircleDashed} weight="medium" />
                      </span>
                      <span class="row__name">{item.name}</span>
                      <span class="row__sub num">{item.value}</span>
                      <span class="row__end meta">{item.done ? "Change" : "Set up"}</span>
                    </button>
                  </li>
                {/each}
              </ul>
            </Card>
            <dl class="keys turn" style:--i="3">
              <div class="key"><dt><Kbd>V</Kbd></dt><dd>Paste a posting</dd></div>
              <div class="key"><dt><Kbd>I</Kbd></dt><dd>Import a resume</dd></div>
              <div class="key"><dt><Kbd>⌘K</Kbd></dt><dd>Commands</dd></div>
              <div class="key"><dt><Kbd>⌘,</Kbd></dt><dd>Settings</dd></div>
            </dl>
          {/if}
        </section>
      {/key}
    </div>
  </div>
</div>

<footer class="foot">
  <div class="foot__in">
    {#if at > 0}<Button variant="ghost" kbd="⌘[" onclick={() => go(at - 1)}>Back</Button>{/if}
    <span class="foot__space"></span>
    <Button variant={onward.primary ? "primary" : "default"} kbd="⏎" onclick={next}>{onward.label}</Button>
  </div>
</footer>

<style>
  .scroll {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: auto;
    overscroll-behavior: contain;
  }
  /* Starts under the toolbar, which lies over it unpainted. */
  .column {
    display: grid;
    gap: var(--space-12);
    max-width: 608px;
    margin-inline: auto;
    padding: calc(var(--toolbar-height) + var(--space-8)) var(--space-6) var(--space-12);
  }

  /* The leaving step and the arriving one share the cell while they cross. */
  .stage {
    display: grid;
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
  /* Agents that aren't supported yet: a label over their card, names and lines in ink 3, the marks grayed. */
  .later {
    display: grid;
    gap: var(--space-2);
  }
  .row--later .row__name,
  .row--later .row__sub {
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
  .row--button .lead:not(.lead--gain) {
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

  .field {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-3) var(--space-5);
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
  .field--sources {
    padding-block: var(--space-3);
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

  .looks {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-3) var(--space-4);
    margin: 0;
    padding: var(--space-3) var(--space-5) var(--space-4);
  }
  .look {
    display: grid;
    min-width: 0;
  }
  dd {
    margin: 0;
  }

  .keys {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2) var(--space-5);
    margin: 0;
  }
  .key {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--ink-2);
  }

  .foot {
    flex: none;
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
    .row--button:hover {
      background: var(--hover);
    }
  }
</style>
