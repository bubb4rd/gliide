<!--
  Settings: one column of cards, with a rail of links to them (`SectionNav`; a row over the cards under 720px).
  Every control saves as you change it (fields on blur or ⏎); the theme applies at once. Agents lists Claude Code
  (its status checked by running `claude`, never by reading its credentials) and, grayed, the agents gliide can't run
  yet. Feed holds the job feed's own settings (saved apart
  from the rest): each filter is Auto (worked out from your experience and masters, shown as it stands) or Custom;
  then its sources besides freehire: LinkedIn's search, a watchlist of careers boards and Fantastic.jobs through an
  Apify token.
-->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import RotateCw from "@lucide/svelte/icons/rotate-cw";
  import X from "@lucide/svelte/icons/x";
  import {
    appError,
    type Backdrop,
    feedSettings,
    setFeedSettings,
    type FeedSettingsDto,
    type FeedSettingsView,
    type Mode,
    type Settings,
    type Theme,
  } from "$lib/api";
  import { CATEGORY, COUNTRIES, EMPLOYMENT, SENIORITY, countryName } from "$lib/feed";
  import { app } from "$lib/stores/app.svelte";
  import { feed } from "$lib/stores/feed.svelte";
  import { theme } from "$lib/stores/theme.svelte";
  import {
    Badge,
    Button,
    Card,
    Checkbox,
    CompanyMark,
    Input,
    Menu,
    type NavSection,
    SectionNav,
    Segmented,
    Toolbar,
    tip,
    toast,
  } from "$lib/ui";

  const CUSTOM = "custom";
  const MODELS = [
    { value: "sonnet", label: "Sonnet" },
    { value: "opus", label: "Opus" },
    { value: "haiku", label: "Haiku" },
    { value: CUSTOM, label: "Custom…" },
  ];
  const ALIASES = new Set(MODELS.map((m) => m.value).filter((v) => v !== CUSTOM));
  const MODEL_ROWS: { key: "extractModel" | "suggestModel"; label: string }[] = [
    { key: "extractModel", label: "Reading the requirements" },
    { key: "suggestModel", label: "Suggesting edits" },
  ];
  const MODES: { value: Mode; label: string }[] = [
    { value: "master-only", label: "Master only" },
    { value: "bank-aware", label: "Bank-aware" },
  ];
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
  /** Agents gliide will run once they're supported (ARCHITECTURE.md §1): listed, grayed, so you know what's coming. */
  const LATER = [
    { name: "Codex", maker: "OpenAI", src: "/agents/openai.png" },
    { name: "Antigravity", maker: "Google", src: "/agents/antigravity.png" },
  ];

  type ModelKey = "extractModel" | "suggestModel";

  let checking = $state(false);
  /** Bumped when a save fails, so every control redraws from what's stored. */
  let resets = $state(0);
  /** Models whose menu says Custom… while the field is being filled. */
  let custom = $state<Record<ModelKey, boolean>>({ extractModel: false, suggestModel: false });
  let drafts = $state<Record<ModelKey | "claudePath", string>>({ extractModel: "", suggestModel: "", claudePath: "" });

  const s = $derived(app.settings);
  let scroller = $state<HTMLElement>();

  // The fields start from what's stored, and follow it after each save.
  $effect(() => {
    if (!s) return;
    drafts.extractModel = s.extractModel;
    drafts.suggestModel = s.suggestModel;
    drafts.claudePath = s.claudePath ?? "";
  });

  onMount(() => {
    if (!app.settings) app.loadSettings();
    feedSettings().then(
      (v) => (feedView = v),
      (e) => console.error(`feed settings: ${appError(e).message}`),
    );
  });

  // ── Feed ──
  type FilterKey = "seniority" | "employment" | "category" | "countries";
  const FILTER_MODES = [
    { value: "auto", label: "Auto" },
    { value: "custom", label: "Custom" },
  ];
  const ALL_MASTERS = "";
  const BOARDS_ATS = "Greenhouse, Lever, Ashby, Workday, SmartRecruiters, Workable or Recruitee";
  const APIFY_TOKENS = "https://console.apify.com/settings/integrations";

  let feedView = $state<FeedSettingsView | null>(null);
  /** Bumped when a feed save fails, so its controls redraw from what's stored. */
  let feedResets = $state(0);
  let feedDrafts = $state({ dailyChecks: "", threshold: "", apifyToken: "" });
  /** The careers link being added to the watchlist: cleared once saved, kept when it isn't. */
  let watchDraft = $state("");
  let watchInput = $state<ReturnType<typeof Input>>();
  let adding = $state(false);
  const fs = $derived(feedView?.settings);

  /** The cards in their order; Feed and its Sources once the feed's settings are in. */
  const sections = $derived<NavSection[]>([
    { id: "agents", label: "Agents" },
    { id: "tailoring", label: "Tailoring" },
    ...(feedView
      ? [
          { id: "feed", label: "Feed" },
          { id: "feed-sources", label: "Sources", sub: true },
        ]
      : []),
    { id: "export", label: "Export folder" },
    { id: "appearance", label: "Appearance" },
  ]);

  $effect(() => {
    if (!fs) return;
    feedDrafts.dailyChecks = String(fs.dailyChecks);
    feedDrafts.threshold = String(fs.threshold);
    feedDrafts.apifyToken = fs.apifyToken ?? "";
  });

  /** Where postings come from now: "freehire, LinkedIn and 3 careers boards". */
  const sources = $derived.by(() => {
    if (!fs || !feedView) return "freehire";
    const boards = feedView.boards.length;
    const on = [
      "freehire",
      fs.linkedin && "LinkedIn",
      boards > 0 && (boards === 1 ? "1 careers board" : `${boards} careers boards`),
      fs.apifyToken && "Fantastic.jobs",
    ].filter((source): source is string => !!source);
    return on.length > 1 ? `${on.slice(0, -1).join(", ")} and ${on.at(-1)}` : on[0];
  });

  const filters = $derived.by(() => {
    if (!feedView) return [];
    const { settings: chosen, auto } = feedView;
    // Countries: the usual ones, then any other you or Auto have.
    const countries = [...new Set([...COUNTRIES, ...auto.countries, ...(chosen.countries ?? [])])].map((c) => ({
      value: c,
      label: countryName(c),
    }));
    const rows: { key: FilterKey; label: string; options: { value: string; label: string }[] }[] = [
      { key: "seniority", label: "Seniority", options: SENIORITY },
      { key: "employment", label: "Employment", options: EMPLOYMENT },
      { key: "category", label: "Category", options: CATEGORY },
      { key: "countries", label: "Countries", options: countries },
    ];
    return rows.map((row) => {
      const words = auto[row.key].map((v) => row.options.find((o) => o.value === v)?.label ?? v);
      const years = auto.years === 1 ? "1 year" : `${Math.round(auto.years * 10) / 10} years`;
      const autoText =
        row.key === "countries" && !words.length
          ? "Anywhere: your experience names no country."
          : !words.length
            ? "Nothing to go on yet: import your resume in Experience."
            : `${words.join(", ")}${row.key === "seniority" ? `, from ${years} of experience` : ""}.`;
      return { ...row, custom: chosen[row.key], autoText };
    });
  });

  async function saveFeed(patch: Partial<FeedSettingsDto>): Promise<boolean> {
    if (!feedView) return false;
    try {
      feedView = await setFeedSettings({ ...feedView.settings, ...patch });
      // The threshold and the filters change what's a fit.
      void feed.refresh();
      return true;
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
      feedResets++;
      return false;
    }
  }

  /** Adds the typed careers link; when it isn't one gliide can watch, the toast says why and the link stays. */
  async function addBoard() {
    const link = watchDraft.trim();
    if (!fs || !link || adding) return;
    adding = true;
    const saved = await saveFeed({ watchlist: [...fs.watchlist, link] });
    adding = false;
    if (saved) watchDraft = "";
    // The failed save redrew the card: back into the field to fix the link.
    else tick().then(() => watchInput?.focus());
  }

  function onWatchKey(e: KeyboardEvent) {
    if (e.key !== "Enter") return;
    e.preventDefault();
    void addBoard();
  }

  /** Focus moves to the next board's Remove (the previous one's when it was the last; the field when none are left). */
  async function removeBoard(url: string) {
    if (!fs || !feedView) return;
    const index = feedView.boards.findIndex((b) => b.url === url);
    await saveFeed({ watchlist: fs.watchlist.filter((u) => u !== url) });
    await tick();
    const removes = document.querySelectorAll<HTMLButtonElement>("[data-remove-board]");
    if (removes.length) removes[Math.min(index, removes.length - 1)].focus();
    else watchInput?.focus();
  }

  /** An empty token turns Fantastic.jobs off. */
  function commitApifyToken() {
    if (!fs) return;
    const token = feedDrafts.apifyToken.trim();
    if (token !== (fs.apifyToken ?? "")) saveFeed({ apifyToken: token || null });
    else feedDrafts.apifyToken = token;
  }

  function visit(url: string) {
    openUrl(url).catch((e) => toast(`Couldn’t open ${url}: ${appError(e).message}`, { tone: "danger" }));
  }

  /** Custom starts from what Auto works out, so switching changes nothing until you do. */
  function setFilterMode(key: FilterKey, mode: string) {
    if (feedView) saveFeed({ [key]: mode === "auto" ? null : [...feedView.auto[key]] });
  }

  function toggleFilter(key: FilterKey, options: { value: string }[], value: string, on: boolean) {
    const chosen = new Set(fs?.[key] ?? []);
    if (on) chosen.add(value);
    else chosen.delete(value);
    // In the list's order, then any value the list doesn't show.
    const listed = options.map((o) => o.value);
    saveFeed({ [key]: [...listed.filter((v) => chosen.has(v)), ...[...chosen].filter((v) => !listed.includes(v))] });
  }

  /** A whole number from `min` to `max`, saved; anything else goes back to what's stored, and says why. */
  function commitNumber(key: "dailyChecks" | "threshold", min: number, max: number) {
    if (!fs) return;
    const typed = feedDrafts[key].trim();
    const n = Number(typed);
    if (!typed || !Number.isInteger(n) || n < min || n > max) {
      feedDrafts[key] = String(fs[key]);
      if (typed) toast(`Enter a whole number from ${min} to ${max}.`, { tone: "danger" });
      return;
    }
    if (n !== fs[key]) saveFeed({ [key]: n });
  }

  async function save(patch: Partial<Settings>): Promise<boolean> {
    if (!app.settings) return false;
    try {
      await app.saveSettings({ ...app.settings, ...patch });
      return true;
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
      theme.apply(app.settings.theme);
      resets++;
      return false;
    }
  }

  async function refreshAgent() {
    checking = true;
    await app.refreshAgent();
    checking = false;
  }

  function setTheme(t: Theme) {
    // Applied before the save returns, so the switch feels instant.
    theme.apply(t);
    save({ theme: t });
  }

  const menuValue = (key: ModelKey) => (custom[key] || !ALIASES.has(s?.[key] ?? "") ? CUSTOM : (s?.[key] ?? ""));

  function chooseModel(key: ModelKey, value: string) {
    if (value === CUSTOM) {
      custom[key] = true;
      drafts[key] = ALIASES.has(drafts[key]) ? "" : drafts[key];
      tick().then(() => document.getElementById(`${key}-custom`)?.focus());
      return;
    }
    custom[key] = false;
    save({ [key]: value });
  }

  async function commitModel(key: ModelKey) {
    const value = drafts[key].trim();
    if (!value) {
      // Nothing typed: back to what's stored.
      custom[key] = false;
      drafts[key] = s?.[key] ?? "";
      return;
    }
    if (value === s?.[key]) return;
    if (await save({ [key]: value })) custom[key] = !ALIASES.has(value);
  }

  // ── Claude Code's path: shown, then edited behind Change path (Save, Discard, or Remove to find it again) ──
  let pathOpen = $state(false);
  let pathBusy = $state(false);
  const pathUnchanged = $derived(drafts.claudePath.trim() === (s?.claudePath ?? ""));

  function openPath() {
    drafts.claudePath = s?.claudePath ?? "";
    pathOpen = true;
    tick().then(() => document.getElementById("claude-path")?.focus());
  }

  /** Back to the path as stored, focus on Change path. */
  function closePath() {
    pathOpen = false;
    drafts.claudePath = s?.claudePath ?? "";
    tick().then(() => document.getElementById("claude-path-change")?.focus());
  }

  /** `null` finds `claude` on your PATH again. Stays open when the save fails, so the path can be fixed. */
  async function savePath(path: string | null) {
    if (pathBusy) return;
    if (path === (s?.claudePath ?? null)) return closePath();
    pathBusy = true;
    const saved = await save({ claudePath: path });
    pathBusy = false;
    if (!saved) return;
    closePath();
    void refreshAgent();
  }

  function submitPath(e: SubmitEvent) {
    e.preventDefault();
    if (!pathUnchanged) void savePath(drafts.claudePath.trim() || null);
  }

  /** Esc anywhere in the path's editor discards it, as its Discard button says. */
  function discardOnEscape(form: HTMLFormElement) {
    const onkey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      closePath();
    };
    form.addEventListener("keydown", onkey);
    return () => form.removeEventListener("keydown", onkey);
  }

  async function chooseFolder() {
    const dir = await open({
      title: "Export folder",
      directory: true,
      multiple: false,
      defaultPath: s?.exportDir,
    }).catch((e) => {
      toast(appError(e).message, { tone: "danger" });
      return null;
    });
    if (!dir || dir === s?.exportDir) return;
    // A folder holding an existing applications.csv has it imported, so the list may grow.
    if (await save({ exportDir: dir })) app.refreshApplications();
  }

  function onEnter(e: KeyboardEvent) {
    if (e.key === "Enter") (e.currentTarget as HTMLInputElement).blur();
  }

  const agent = $derived(app.agent);
  const agentBadge = $derived.by(() => {
    if (checking) return { tone: "neutral", text: "Checking…" } as const;
    if (!agent) return { tone: "neutral", text: "Not checked" } as const;
    if (!agent.found) return { tone: "danger", text: "Not found" } as const;
    if (agent.signedIn === false) return { tone: "danger", text: "Signed out" } as const;
    if (agent.signedIn === null) return { tone: "neutral", text: "Sign-in unknown" } as const;
    return { tone: "gain", text: "Signed in" } as const;
  });
  /** Its maker, then what `claude` says about itself: "Anthropic · 2.1.0 · Max". */
  const claudeMeta = $derived(
    ["Anthropic", ...(agent && !agent.found ? ["not installed on this Mac"] : [agent?.version, agent?.plan])]
      .filter(Boolean)
      .join(" · "),
  );
</script>

<Toolbar>
  {#snippet crumbs()}<b>Settings</b>{/snippet}
</Toolbar>

<div class="scroll" bind:this={scroller}>
  {#if s}
    <div class="layout">
      <div class="rail">
        <SectionNav {sections} root={scroller} label="Settings sections" />
      </div>
      {#key resets}
        <div class="column">
          <Card pad as="section" id="agents" tabindex={-1} aria-labelledby="agents-title">
            <div class="card">
              <h2 class="name" id="agents-title">Agents</h2>
              <p class="lede">Read the requirements and suggest the edits, on your own plan. gliide never sees a credential.</p>
              <ul class="agents">
                <li class="agent">
                  <span class="agent__mark"><CompanyMark size={28} src="/agents/claude.png" /></span>
                  <span class="agent__name">Claude Code</span>
                  <span class="agent__sub num">{claudeMeta}</span>
                  <span class="agent__end">
                    <Badge tone={agentBadge.tone} dot>{agentBadge.text}</Badge>
                    <Button
                      size="sm"
                      variant="ghost"
                      square
                      icon={RotateCw}
                      disabled={checking}
                      aria-label="Check Claude Code again"
                      {@attach tip("Check again")}
                      onclick={refreshAgent}
                    />
                  </span>
                  {#if agent?.problem}<p class="problem agent__more">{agent.problem}</p>{/if}
                  {#if pathOpen}
                    <form class="agent__more path-edit" onsubmit={submitPath} {@attach discardOnEscape}>
                      <label for="claude-path">Path</label>
                      <Input
                        id="claude-path"
                        size="md"
                        bind:value={drafts.claudePath}
                        placeholder="Automatic"
                        spellcheck="false"
                        autocomplete="off"
                        aria-describedby="claude-path-hint"
                      />
                      <p class="hint" id="claude-path-hint">
                        Where the <code>claude</code> to run is. Empty finds it on your PATH.
                      </p>
                      <div class="path-actions">
                        {#if s.claudePath}
                          <Button
                            variant="danger"
                            size="sm"
                            disabled={pathBusy}
                            {@attach tip("Find claude on your PATH again")}
                            onclick={() => void savePath(null)}>Remove</Button
                          >
                        {/if}
                        <Button variant="ghost" size="sm" kbd="Esc" disabled={pathBusy} onclick={closePath}>Discard</Button>
                        <Button type="submit" variant="primary" size="sm" kbd="⏎" disabled={pathBusy || pathUnchanged}>
                          {pathBusy ? "Saving…" : "Save"}
                        </Button>
                      </div>
                    </form>
                  {:else}
                    <div class="agent__more path-row">
                      <span class="path-label">Path</span>
                      <span class="path-value">
                        {#if s.claudePath}
                          <code {@attach tip(s.claudePath)}>{s.claudePath}</code>
                        {:else if agent?.path}
                          <code {@attach tip(agent.path)}>{agent.path}</code> · found on your PATH
                        {:else}
                          Automatic: found on your PATH
                        {/if}
                      </span>
                      <Button id="claude-path-change" variant="ghost" size="sm" onclick={openPath}>Change path</Button>
                    </div>
                  {/if}
                </li>
                {#each LATER as later (later.name)}
                  <li class="agent agent--later">
                    <span class="agent__mark"><CompanyMark size={28} src={later.src} off /></span>
                    <span class="agent__name">{later.name}</span>
                    <span class="agent__sub">{later.maker} · not supported yet</span>
                    <span class="agent__end">
                      <Button size="sm" disabled aria-label="Connect {later.name}: not supported yet">Connect</Button>
                    </span>
                  </li>
                {/each}
              </ul>
            </div>
          </Card>

          <Card pad as="section" id="tailoring" tabindex={-1} aria-labelledby="tailoring-title">
            <div class="card">
              <h2 class="name" id="tailoring-title">Tailoring</h2>
              <p class="lede">The model for each step, and where suggestions come from by default.</p>
              <dl class="fields">
                {#each MODEL_ROWS as row (row.key)}
                  <div class="field">
                    <dt>{row.label}</dt>
                    <dd class="controls">
                      {#if menuValue(row.key) === CUSTOM}
                        <span class="custom">
                          <Input
                            id="{row.key}-custom"
                            size="md"
                            bind:value={drafts[row.key]}
                            placeholder="Model name"
                            aria-label="{row.label}: model name"
                            spellcheck="false"
                            autocomplete="off"
                            onblur={() => commitModel(row.key)}
                            onkeydown={onEnter}
                          />
                        </span>
                      {/if}
                      <Menu
                        value={menuValue(row.key)}
                        options={MODELS}
                        label="Model for {row.label.toLowerCase()}"
                        placement="bottom-end"
                        onchange={(v) => chooseModel(row.key, v)}
                      />
                    </dd>
                  </div>
                {/each}
                <div class="field">
                  <dt>Suggestions from</dt>
                  <dd>
                    <Segmented
                      options={MODES}
                      value={s.defaultMode}
                      label="Default suggestions from"
                      onchange={(m) => save({ defaultMode: m })}
                    />
                  </dd>
                </div>
              </dl>
            </div>
          </Card>

          {#if fs && feedView}
            {#key feedResets}
              <Card pad as="section" id="feed" tabindex={-1} aria-labelledby="feed-title">
                <div class="card">
                  <h2 class="name" id="feed-title">Feed</h2>
                  <p class="lede">
                    New postings from {sources}, pulled every {fs.everyHours}
                    {fs.everyHours === 1 ? "hour" : "hours"} and checked against your masters up to your daily checks.
                  </p>
                  <dl class="fields">
                    <div class="field">
                      <dt>Pull new postings</dt>
                      <dd class="toggle">
                        <Checkbox checked={fs.enabled} label="Pull new postings" onchange={(on) => saveFeed({ enabled: on })} />
                      </dd>
                    </div>
                    <div class="field">
                      <dt>Check against</dt>
                      <dd>
                        <Menu
                          value={fs.master ?? ALL_MASTERS}
                          options={[
                            { value: ALL_MASTERS, label: "All masters" },
                            ...feedView.masters.map((m) => ({ value: m.id, label: m.title })),
                          ]}
                          label="Masters the feed checks against"
                          placement="bottom-end"
                          onchange={(m) => saveFeed({ master: m === ALL_MASTERS ? null : m })}
                        />
                      </dd>
                    </div>
                    {#each filters as filter (filter.key)}
                      <div class="field field--detail">
                        <dt>{filter.label}</dt>
                        <dd>
                          <Segmented
                            options={FILTER_MODES}
                            value={filter.custom === null ? "auto" : "custom"}
                            label="{filter.label}: auto or custom"
                            onchange={(mode) => setFilterMode(filter.key, mode)}
                          />
                        </dd>
                        <dd class="detail">
                          {#if filter.custom === null}
                            <p class="hint">{filter.autoText}</p>
                          {:else}
                            {@const chosen = filter.custom}
                            <div class="options">
                              {#each filter.options as option (option.value)}
                                {@const on = chosen.includes(option.value)}
                                <label class="option">
                                  <Checkbox
                                    checked={on}
                                    label={option.label}
                                    disabled={on && chosen.length === 1}
                                    onchange={(next) => toggleFilter(filter.key, filter.options, option.value, next)}
                                  />
                                  <span>{option.label}</span>
                                </label>
                              {/each}
                            </div>
                          {/if}
                        </dd>
                      </div>
                    {/each}
                    <div class="field">
                      <dt>Only jobs that sponsor a visa</dt>
                      <dd class="toggle">
                        <Checkbox
                          checked={fs.needsVisa}
                          label="Only jobs that sponsor a visa"
                          onchange={(on) => saveFeed({ needsVisa: on })}
                        />
                      </dd>
                    </div>
                    <div class="field">
                      <dt>Include jobs that need a clearance</dt>
                      <dd class="toggle">
                        <Checkbox
                          checked={fs.includeClearance}
                          label="Include jobs that need a clearance"
                          onchange={(on) => saveFeed({ includeClearance: on })}
                        />
                      </dd>
                    </div>
                    <div class="field">
                      <dt>Include jobs without a known place</dt>
                      <dd class="toggle">
                        <Checkbox
                          checked={fs.includeUnknownLocation}
                          label="Include jobs without a known place"
                          onchange={(on) => saveFeed({ includeUnknownLocation: on })}
                        />
                      </dd>
                    </div>
                    <div class="field">
                      <dt><label for="feed-daily">Daily checks</label></dt>
                      <dd class="controls">
                        <span class="number">
                          <Input
                            id="feed-daily"
                            size="md"
                            inputmode="numeric"
                            autocomplete="off"
                            class="num"
                            bind:value={feedDrafts.dailyChecks}
                            onblur={() => commitNumber("dailyChecks", 0, 500)}
                            onkeydown={onEnter}
                          />
                        </span>
                        <span class="unit">a day</span>
                      </dd>
                    </div>
                    <div class="field">
                      <dt>
                        <label for="feed-threshold" {@attach tip("A checked posting whose best master scores at least this is a fit")}>
                          Fit threshold
                        </label>
                      </dt>
                      <dd class="controls">
                        <span class="number">
                          <Input
                            id="feed-threshold"
                            size="md"
                            inputmode="numeric"
                            autocomplete="off"
                            class="num"
                            bind:value={feedDrafts.threshold}
                            onblur={() => commitNumber("threshold", 0, 100)}
                            onkeydown={onEnter}
                          />
                        </span>
                        <span class="unit">%</span>
                      </dd>
                    </div>
                  </dl>
                  <h3 class="group label" id="feed-sources" tabindex="-1">Sources</h3>
                  <dl class="fields fields--group" aria-labelledby="feed-sources">
                    <div class="field field--detail">
                      <dt>LinkedIn</dt>
                      <dd class="toggle">
                        <Checkbox checked={fs.linkedin} label="Search LinkedIn" onchange={(on) => saveFeed({ linkedin: on })} />
                      </dd>
                      <dd class="detail">
                        <p class="hint">
                          Searches LinkedIn for your masters’ titles on each pull. Its jobs link to LinkedIn, not to the
                          employer. Off by default: LinkedIn’s User Agreement doesn’t allow automated access. gliide reads
                          its public guest job search only while this is on, and that use is your responsibility.
                        </p>
                      </dd>
                    </div>
                    <div class="field field--detail">
                      <dt><label for="feed-watch">Watchlist</label></dt>
                      <dd class="controls">
                        <span class="link-field">
                          <Input
                            bind:this={watchInput}
                            id="feed-watch"
                            size="md"
                            type="url"
                            placeholder="A careers link"
                            spellcheck="false"
                            autocomplete="off"
                            aria-describedby="feed-watch-hint"
                            bind:value={watchDraft}
                            onkeydown={onWatchKey}
                          />
                        </span>
                        <Button disabled={!watchDraft.trim() || adding} onclick={addBoard}>
                          {adding ? "Adding…" : "Add"}
                        </Button>
                      </dd>
                      <dd class="detail">
                        {#if feedView.boards.length}
                          <ul class="boards" aria-label="Watched boards">
                            {#each feedView.boards as board (board.url)}
                              <li class="board">
                                <span class="board-name">{board.name}</span>
                                <span class="meta">{board.ats}</span>
                                <span class="board-actions">
                                  <Button
                                    variant="ghost"
                                    size="sm"
                                    square
                                    icon={ExternalLink}
                                    aria-label="Open the {board.name} careers page"
                                    {@attach tip(board.url)}
                                    onclick={() => visit(board.url)}
                                  />
                                  <Button
                                    variant="ghost"
                                    size="sm"
                                    square
                                    icon={X}
                                    aria-label="Remove {board.name} ({board.ats}) from the watchlist"
                                    {@attach tip("Remove")}
                                    data-remove-board
                                    onclick={() => removeBoard(board.url)}
                                  />
                                </span>
                              </li>
                            {/each}
                          </ul>
                          <p class="hint" id="feed-watch-hint">A {BOARDS_ATS} link; each pull checks every board.</p>
                        {:else}
                          <p class="hint" id="feed-watch-hint">
                            No boards yet. Add a company’s {BOARDS_ATS} link and each pull checks its openings.
                          </p>
                        {/if}
                      </dd>
                    </div>
                    <div class="field field--detail">
                      <dt><label for="feed-apify">Fantastic.jobs</label></dt>
                      <dd class="controls">
                        <span class="token">
                          <Input
                            id="feed-apify"
                            size="md"
                            type="password"
                            placeholder="Apify token"
                            spellcheck="false"
                            autocomplete="off"
                            aria-describedby="feed-apify-hint"
                            bind:value={feedDrafts.apifyToken}
                            onblur={commitApifyToken}
                            onkeydown={onEnter}
                          />
                        </span>
                      </dd>
                      <dd class="detail">
                        <p class="hint" id="feed-apify-hint">
                          Runs once a day. Apify bills per job, from about $4 per 1,000; its Free plan includes $5 of usage
                          a month.
                          <a
                            class="link"
                            href={APIFY_TOKENS}
                            {@attach tip(APIFY_TOKENS)}
                            onclick={(e) => {
                              e.preventDefault();
                              visit(APIFY_TOKENS);
                            }}>Get a token</a
                          >
                        </p>
                      </dd>
                    </div>
                  </dl>
                </div>
              </Card>
            {/key}
          {/if}

          <Card pad as="section" id="export" tabindex={-1} aria-labelledby="export-title">
            <div class="card">
              <h2 class="name" id="export-title">Export folder</h2>
              <p class="lede">Tailored PDFs and applications.csv go here.</p>
              <div class="folder">
                <span class="path" {@attach tip(s.exportDir)}>{s.exportDir}</span>
                <Button icon={FolderOpen} onclick={chooseFolder}>Choose…</Button>
              </div>
            </div>
          </Card>

          <Card pad as="section" id="appearance" tabindex={-1} aria-labelledby="appearance-title">
            <div class="card">
              <h2 class="name" id="appearance-title">Appearance</h2>
              <p class="lede">Light is the default; System follows macOS. A backdrop paints the top of the Overview.</p>
              <dl class="fields">
                <div class="field">
                  <dt>Theme</dt>
                  <dd><Segmented options={THEMES} value={s.theme} label="Theme" onchange={setTheme} /></dd>
                </div>
                <div class="field">
                  <dt>Overview backdrop</dt>
                  <dd>
                    <Segmented
                      options={BACKDROPS}
                      value={s.backdrop}
                      label="Overview backdrop"
                      onchange={(b) => save({ backdrop: b })}
                    />
                  </dd>
                </div>
              </dl>
            </div>
          </Card>
        </div>
      {/key}
    </div>
  {/if}
</div>

<style>
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    overscroll-behavior: contain;
    container-type: inline-size;
  }
  /* The rail in the left gutter, the column centered while the gutter has room for it, then pushed right. */
  .layout {
    display: grid;
    grid-template-columns: minmax(calc(160px + var(--space-6)), 1fr) minmax(0, 640px) minmax(0, 1fr);
    align-items: start;
    padding: var(--space-8) var(--space-6) var(--space-12);
  }
  /* Held beside the column, its first link on the line of the card heading beside it. */
  .rail {
    position: sticky;
    top: var(--space-8);
    justify-self: end;
    width: 160px;
    margin-inline-end: var(--space-6);
    padding-top: var(--space-5);
  }
  .column {
    display: grid;
    gap: var(--space-4);
    min-width: 0;
  }
  /* A jump lands a section where the column starts. */
  .column > :global(section),
  .group {
    scroll-margin-top: var(--space-8);
  }
  /* Focus moves to a section you jump to, so Tab goes on from there; the rail's current link marks it, not a ring. */
  .column > :global(section:focus),
  .group:focus {
    outline: none;
  }
  /* Under 720px: the links in a row, held over the cards in paper as they scroll under it. */
  @container (width < 720px) {
    .layout {
      grid-template-columns: minmax(0, 640px);
      justify-content: center;
      padding-top: 0;
    }
    .rail {
      top: 0;
      z-index: 1;
      justify-self: stretch;
      width: auto;
      margin: 0 calc(var(--space-6) * -1) var(--space-6);
      padding: var(--space-2) var(--space-6) 0;
      background: var(--paper);
    }
    .column > :global(section),
    .group {
      scroll-margin-top: calc(var(--control-md) + var(--space-4) + var(--space-6));
    }
  }
  .card {
    display: grid;
    gap: var(--space-1);
  }
  .name {
    min-width: 0;
    margin: 0;
    font-size: var(--text-lg);
    line-height: var(--leading-lg);
    letter-spacing: var(--tracking-lg);
    font-weight: var(--weight-medium);
  }
  .lede,
  .problem,
  .hint {
    margin: 0;
    text-wrap: pretty;
  }
  .lede {
    color: var(--ink-2);
  }
  .problem {
    margin-top: var(--space-2);
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-sm);
    color: var(--danger);
    background: var(--danger-soft);
  }
  .hint {
    margin-top: var(--space-1-5);
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    overflow-wrap: anywhere;
    color: var(--ink-3);
  }
  code {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
  }
  /* The agents: a 28px mark, the name over its maker and what's known of it, the status and actions at the end;
     hairlines between, as the fields'. One gliide can't run yet is grayed: its mark off, its name in ink 3. */
  .agents {
    display: grid;
    margin: var(--space-3) 0 0;
    padding: 0;
    list-style: none;
  }
  .agent {
    display: grid;
    grid-template-columns: 28px minmax(0, 1fr) auto;
    grid-template-areas:
      "mark name end"
      "mark sub end";
    align-items: center;
    column-gap: var(--space-3);
    padding-block: var(--space-3);
    box-shadow: inset 0 1px 0 var(--line);
  }
  .agent:last-child {
    padding-bottom: 0;
  }
  .agent__mark {
    grid-area: mark;
    display: flex;
  }
  .agent__name {
    grid-area: name;
    min-width: 0;
    font-weight: var(--weight-medium);
  }
  .agent__sub {
    grid-area: sub;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    color: var(--ink-3);
  }
  .agent__end {
    grid-area: end;
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .agent--later .agent__name {
    color: var(--ink-3);
  }
  /* Under the name: the problem, then the path: one line ending in Change path, or its editor (the field after its
     label, the hint under the field, then Remove, Discard and Save at the trailing edge). */
  .agent__more {
    grid-column: 2 / -1;
  }
  .path-row,
  .path-edit {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--space-1-5) var(--space-3);
    margin-top: var(--space-2);
  }
  .path-edit {
    grid-template-columns: auto minmax(0, 1fr);
  }
  .path-label,
  .path-edit label {
    color: var(--ink-2);
  }
  .path-value {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    color: var(--ink-3);
  }
  .path-value code {
    color: var(--ink-2);
  }
  .path-edit .hint {
    grid-column: 2;
    margin-top: 0;
  }
  .path-actions {
    grid-column: 1 / -1;
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
  }
  .fields {
    display: grid;
    margin: var(--space-3) 0 0;
  }
  .field {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    min-height: var(--control-lg);
    padding-block: var(--space-2);
    box-shadow: inset 0 1px 0 var(--line);
  }
  .field:last-child {
    padding-bottom: 0;
  }
  dt {
    flex: none;
    color: var(--ink-2);
  }
  dd {
    min-width: 0;
    margin: 0;
  }
  .controls {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .custom {
    width: 200px;
  }
  .folder {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin-top: var(--space-3);
    /* 6 (button) + 1 padding + 1 border = 8, as the segmented track. */
    padding: 1px 1px 1px var(--space-3);
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    color: var(--ink-2);
  }
  /* A field with more under it: its name and control on one line (a filter's Auto | Custom, a source's toggle or
     field), then a meta sentence, the Custom checkboxes or the watchlist's boards across the whole field. */
  .field--detail {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--space-2) var(--space-4);
  }
  .detail {
    grid-column: 1 / -1;
  }
  .detail .hint:first-child {
    margin-top: 0;
  }
  /* A group of fields under its own label, 24px after the one before. */
  .group {
    margin-top: var(--space-6);
  }
  .fields--group {
    margin-top: var(--space-2);
  }
  .link-field {
    width: 280px;
  }
  .token {
    width: 240px;
  }
  .boards {
    display: grid;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  /* A watched board: its name and ATS, then open and remove at the field's trailing edge. */
  .board {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: var(--control-md);
    min-width: 0;
  }
  .board-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink);
  }
  .board .meta {
    flex: none;
  }
  .board-actions {
    display: flex;
    gap: var(--space-1);
    margin-inline-start: auto;
  }
  .link {
    border-radius: var(--radius-xs);
    color: var(--accent-text);
    text-decoration: none;
  }
  @media (hover: hover) {
    .link:hover {
      text-decoration: underline;
    }
  }
  .options {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(168px, 1fr));
    gap: var(--space-1) var(--space-4);
  }
  .option {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: var(--control-sm);
    min-width: 0;
    color: var(--ink);
  }
  .option span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .number {
    width: 72px;
  }
  .unit {
    min-width: 4ch;
    color: var(--ink-3);
  }
  /* A checkbox on the label's center line, not the dd's text baseline. */
  .toggle {
    display: flex;
  }
</style>
