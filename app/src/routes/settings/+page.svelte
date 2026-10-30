<!--
  Settings: one column of cards, with a rail of links to them (`SectionNav`; a row over the cards under 720px).
  Every control saves as you change it (fields on blur or ⏎); the theme applies at once. Agents lists Claude Code,
  Codex and Antigravity (each checked by running it, never by reading its credentials; one check runs them all), each
  with its own path; Use moves every model call to one that's found, and Tailoring's models are that agent's own.
  Feed holds the job feed's own settings (saved apart from the rest): each filter is Auto (worked out from your
  experience and masters, shown as it stands) or Custom;
  then its sources besides freehire: LinkedIn's search, a watchlist of careers boards and Fantastic.jobs through an
  Apify token. Notifications are off until Turn on (the only thing that asks macOS); its three kinds can't be checked
  while Interviews is off or macOS doesn't allow gliide's banners, read again whenever the window gains focus.
-->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import Bell from "@lucide/svelte/icons/bell";
  import BellOff from "@lucide/svelte/icons/bell-off";
  import CalendarClock from "@lucide/svelte/icons/calendar-clock";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import RotateCw from "@lucide/svelte/icons/rotate-cw";
  import X from "@lucide/svelte/icons/x";
  import {
    type Agent,
    type AgentStatus,
    appError,
    type Backdrop,
    feedSettings,
    setFeedSettings,
    type FeedSettingsDto,
    type FeedSettingsView,
    type Mode,
    type NotificationPermission,
    notificationPermission,
    openNotificationSettings,
    requestNotifications,
    type Settings,
    type Theme,
  } from "$lib/api";
  import { AGENT_LIST, type AgentInfo } from "$lib/agents";
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
    Icon,
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
  const MODEL_ROWS: { key: ModelKey; label: string }[] = [
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

  type ModelKey = "extractModel" | "suggestModel";

  let checking = $state(false);
  /** Bumped when a save fails, so every control redraws from what's stored. */
  let resets = $state(0);
  /** Models whose menu says Custom… while the field is being filled. */
  let custom = $state<Record<ModelKey, boolean>>({ extractModel: false, suggestModel: false });
  let drafts = $state<Record<ModelKey, string>>({ extractModel: "", suggestModel: "" });

  const s = $derived(app.settings);
  /** The agent model calls run on, and its models as stored. */
  const active = $derived(app.agentInfo);
  const activeModels = $derived(s ? s.agents[s.agent] : null);
  let scroller = $state<HTMLElement>();

  // The fields start from what's stored for the agent in use, and follow it after each save or switch.
  $effect(() => {
    if (!activeModels) return;
    drafts.extractModel = activeModels.extractModel;
    drafts.suggestModel = activeModels.suggestModel;
  });

  onMount(() => {
    if (!app.settings) app.loadSettings();
    feedSettings().then(
      (v) => (feedView = v),
      (e) => console.error(`feed settings: ${appError(e).message}`),
    );
    readPermission();
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
    { id: "notifications", label: "Notifications" },
    { id: "export", label: "Export folder" },
    { id: "appearance", label: "Appearance" },
    { id: "about", label: "About" },
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

  // ── Notifications: off until Turn on, which alone asks macOS; what macOS allows read again on every focus ──
  type NotifyKey = "dayBefore" | "hourBefore" | "debrief";
  const NOTIFY_ROWS: { key: NotifyKey; label: string; example: string }[] = [
    {
      key: "dayBefore",
      label: "Day before",
      example: "“Globex technical tomorrow at 2:00 PM. 8 questions ready, 2 without a story.”",
    },
    { key: "hourBefore", label: "Hour before", example: "“Globex technical in 1 hour”" },
    { key: "debrief", label: "Debrief", example: "An hour after it ends: “How did the Globex technical go?”" },
  ];
  const onDay = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });

  let permission = $state<NotificationPermission | null>(null);
  let turningOn = $state(false);
  const notify = $derived(s?.notifications);
  /** Turned on, but macOS doesn't allow the banners. */
  const blocked = $derived(!!notify?.interviews && permission === "denied");
  /** The three kinds can be checked. */
  const notifyLive = $derived(!!notify?.interviews && !blocked);
  /** "Rounds you’ve added. Turned on Oct 29, with Initech’s first round." */
  const interviewsLine = $derived.by(() => {
    if (!notify?.interviews) return "Rounds you’ve added. Off until you turn it on.";
    const at = notify.turnedOn ? new Date(notify.turnedOn) : null;
    if (!at || Number.isNaN(at.getTime())) return "Rounds you’ve added.";
    const how = notify.turnedOnWith === "Settings" ? "in Settings" : `with ${notify.turnedOnWith ?? "Settings"}`;
    return `Rounds you’ve added. Turned on ${onDay.format(at)}, ${how}.`;
  });

  function readPermission() {
    notificationPermission().then(
      (p) => (permission = p),
      (e) => console.error(`notification permission: ${appError(e).message}`),
    );
  }

  /** Asks macOS (its prompt shows the first time only), then turns Interviews on with Day before and Hour before. */
  async function turnOnNotifications() {
    if (!app.settings || turningOn) return;
    turningOn = true;
    try {
      permission = await requestNotifications();
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
      turningOn = false;
      return;
    }
    const n = app.settings.notifications;
    await save({
      notifications: {
        ...n,
        interviews: true,
        dayBefore: true,
        hourBefore: true,
        debrief: false,
        turnedOn: n.turnedOn ?? new Date().toISOString(),
        turnedOnWith: n.turnedOnWith ?? "Settings",
        // Turned on here, the offer under a first round has nothing left to offer.
        offered: true,
      },
    });
    turningOn = false;
  }

  /** The kinds keep their checks while Interviews is off; they show unchecked until it's back on. */
  function setNotify(patch: Partial<Settings["notifications"]>) {
    if (app.settings) save({ notifications: { ...app.settings.notifications, ...patch } });
  }

  function openMacSettings() {
    openNotificationSettings().catch((e) => toast(appError(e).message, { tone: "danger" }));
  }

  /** One check, every agent. */
  async function refreshAgents() {
    checking = true;
    await app.refreshAgents();
    checking = false;
  }

  /** Runs every model call on `agent` from now on; its own models and path come with it. */
  async function useAgent(agent: Agent) {
    if (!s || s.agent === agent) return;
    custom = { extractModel: false, suggestModel: false };
    await save({ agent });
  }

  function setTheme(t: Theme) {
    // Applied before the save returns, so the switch feels instant.
    theme.apply(t);
    save({ theme: t });
  }

  /** The agent in use's models, from its status, as menu options; Custom… last. */
  const modelOptions = $derived([
    ...(app.agent?.models ?? []).map((m) => ({ value: m.id, label: m.name })),
    { value: CUSTOM, label: "Custom…" },
  ]);
  const listed = $derived(new Set((app.agent?.models ?? []).map((m) => m.id)));

  const menuValue = (key: ModelKey) =>
    custom[key] || !listed.has(activeModels?.[key] ?? "") ? CUSTOM : (activeModels?.[key] ?? "");

  /** Saves one of the agent in use's models, keeping its path and the other agents'. */
  function saveModel(key: ModelKey, value: string) {
    if (!s) return Promise.resolve(false);
    return save({ agents: { ...s.agents, [s.agent]: { ...s.agents[s.agent], [key]: value } } });
  }

  function chooseModel(key: ModelKey, value: string) {
    if (value === CUSTOM) {
      custom[key] = true;
      drafts[key] = listed.has(drafts[key]) ? "" : drafts[key];
      tick().then(() => document.getElementById(`${key}-custom`)?.focus());
      return;
    }
    custom[key] = false;
    void saveModel(key, value);
  }

  async function commitModel(key: ModelKey) {
    const value = drafts[key].trim();
    if (!value) {
      // Nothing typed: back to what's stored.
      custom[key] = false;
      drafts[key] = activeModels?.[key] ?? "";
      return;
    }
    if (value === activeModels?.[key]) return;
    if (await saveModel(key, value)) custom[key] = !listed.has(value);
  }

  // ── Each agent's path: shown, then edited behind Change path (Save, Discard, or Remove to find it again); one
  //    editor open at a time ──
  let pathOpen = $state<Agent | null>(null);
  let pathDraft = $state("");
  let pathBusy = $state(false);
  const pathUnchanged = $derived(pathOpen !== null && pathDraft.trim() === (s?.agents[pathOpen].path ?? ""));

  function openPath(agent: Agent) {
    pathDraft = s?.agents[agent].path ?? "";
    pathOpen = agent;
    tick().then(() => document.getElementById(`${agent}-path`)?.focus());
  }

  /** Back to the path as stored, focus on its Change path. */
  function closePath() {
    const agent = pathOpen;
    pathOpen = null;
    pathDraft = "";
    if (agent) tick().then(() => document.getElementById(`${agent}-path-change`)?.focus());
  }

  /** `null` finds the agent on your PATH again. Stays open when the save fails, so the path can be fixed. */
  async function savePath(path: string | null) {
    const agent = pathOpen;
    if (pathBusy || !s || !agent) return;
    if (path === s.agents[agent].path) return closePath();
    pathBusy = true;
    const saved = await save({ agents: { ...s.agents, [agent]: { ...s.agents[agent], path } } });
    pathBusy = false;
    if (!saved) return;
    closePath();
    void refreshAgents();
  }

  function submitPath(e: SubmitEvent) {
    e.preventDefault();
    if (!pathUnchanged) void savePath(pathDraft.trim() || null);
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

  const statusOf = (agent: Agent) => app.agents?.find((a) => a.agent === agent) ?? null;
  function badgeOf(status: AgentStatus | null) {
    if (checking) return { tone: "neutral", text: "Checking…" } as const;
    if (!status) return { tone: "neutral", text: "Not checked" } as const;
    if (!status.found) return { tone: "danger", text: "Not found" } as const;
    if (status.signedIn === false) return { tone: "danger", text: "Signed out" } as const;
    if (status.signedIn === null) return { tone: "neutral", text: "Sign-in unknown" } as const;
    return { tone: "gain", text: "Signed in" } as const;
  }
  /** Its maker, then what the agent says about itself: "Anthropic · 2.1.0 · Max". */
  const metaOf = (info: AgentInfo, status: AgentStatus | null) =>
    [info.maker, ...(status && !status.found ? ["not installed on this Mac"] : [status?.version, status?.plan])]
      .filter(Boolean)
      .join(" · ");

  // ── About: this build and updates ──
  const update = $derived(app.update);
  const clock = new Intl.DateTimeFormat(undefined, { weekday: "short", hour: "numeric", minute: "2-digit" });
  /** Where updates stand, in a sentence. */
  const updateLine = $derived.by(() => {
    if (!update) return "Not checked yet.";
    if (update.state === "installing") {
      const pct = update.progress === null ? "" : ` ${Math.round(update.progress * 100)}%`;
      return `Downloading gliide ${update.available}…${pct}`;
    }
    if (update.state === "checking") return "Checking for updates…";
    if (update.available) return `gliide ${update.available} is available.`;
    const at = update.checkedAt ? new Date(update.checkedAt) : null;
    if (!at || Number.isNaN(at.getTime())) return "Not checked yet.";
    return update.error ? `Last checked ${clock.format(at)}.` : `Up to date · checked ${clock.format(at)}.`;
  });
</script>

<!-- What macOS allows can change in System Settings while gliide is away. -->
<svelte:window onfocus={readPermission} />

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
              <p class="lede">
                Every model call runs on the agent you choose, with your own plan. gliide never sees a credential.
              </p>
              <ul class="agents">
                {#each AGENT_LIST as info (info.id)}
                  {@const status = statusOf(info.id)}
                  {@const badge = badgeOf(status)}
                  {@const inUse = s.agent === info.id}
                  {@const path = s.agents[info.id].path}
                  <li class="agent">
                    <span class="agent__mark"><CompanyMark size={28} src={info.icon} /></span>
                    <span class="agent__name">
                      <span class="agent__title">{info.name}</span>
                      {#if inUse}<Badge tone="accent">In use</Badge>{/if}
                    </span>
                    <span class="agent__sub num">{metaOf(info, status)}</span>
                    <span class="agent__end">
                      {#if !inUse && status?.found}
                        <Button size="sm" aria-label="Use {info.name} for model calls" onclick={() => void useAgent(info.id)}
                          >Use</Button
                        >
                      {:else if status && !status.found}
                        <Button size="sm" variant="ghost" icon={ExternalLink} onclick={() => visit(info.install)}
                          >How to install</Button
                        >
                      {/if}
                      <Badge tone={badge.tone} dot>{badge.text}</Badge>
                      <Button
                        size="sm"
                        variant="ghost"
                        square
                        icon={RotateCw}
                        disabled={checking}
                        aria-label="Check the agents again"
                        {@attach tip("Check again")}
                        onclick={refreshAgents}
                      />
                    </span>
                    {#if status?.problem}<p class="problem agent__more">{status.problem}</p>{/if}
                    {#if pathOpen === info.id}
                      <form class="agent__more path-edit" onsubmit={submitPath} {@attach discardOnEscape}>
                        <label for="{info.id}-path">Path</label>
                        <Input
                          id="{info.id}-path"
                          size="md"
                          bind:value={pathDraft}
                          placeholder="Automatic"
                          spellcheck="false"
                          autocomplete="off"
                          aria-describedby="{info.id}-path-hint"
                        />
                        <p class="hint" id="{info.id}-path-hint">
                          Where the <code>{info.bin}</code> to run is. Empty finds it on your PATH.
                        </p>
                        <div class="path-actions">
                          {#if path}
                            <Button
                              variant="danger"
                              size="sm"
                              disabled={pathBusy}
                              {@attach tip(`Find ${info.bin} on your PATH again`)}
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
                          {#if path}
                            <code {@attach tip(path)}>{path}</code>
                          {:else if status?.path}
                            <code {@attach tip(status.path)}>{status.path}</code> · found on your PATH
                          {:else}
                            Automatic: found on your PATH
                          {/if}
                        </span>
                        <Button
                          id="{info.id}-path-change"
                          variant="ghost"
                          size="sm"
                          aria-label="Change {info.name}’s path"
                          onclick={() => openPath(info.id)}>Change path</Button
                        >
                      </div>
                    {/if}
                  </li>
                {/each}
              </ul>
            </div>
          </Card>

          <Card pad as="section" id="tailoring" tabindex={-1} aria-labelledby="tailoring-title">
            <div class="card">
              <h2 class="name" id="tailoring-title">Tailoring</h2>
              <p class="lede">
                {active.name}’s model for each step, and what suggestions draw on. Bank-aware brings in an entry from
                Experience where it fits a posting better than one the master shows.
              </p>
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
                            aria-label="{row.label}: {active.name} model name"
                            spellcheck="false"
                            autocomplete="off"
                            onblur={() => commitModel(row.key)}
                            onkeydown={onEnter}
                          />
                        </span>
                      {/if}
                      <Menu
                        value={menuValue(row.key)}
                        options={modelOptions}
                        label="{active.name} model for {row.label.toLowerCase()}"
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
                      value={s.suggestionsFrom}
                      label="Suggestions from"
                      onchange={(m) => save({ suggestionsFrom: m })}
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

          {#if notify}
            <Card pad as="section" id="notifications" tabindex={-1} aria-labelledby="notifications-title">
              <div class="card">
                <h2 class="name" id="notifications-title">Notifications</h2>
                <p class="lede">Banners from macOS, only for what you turn on.</p>
                <ul class="notify">
                  <li class="notify-row notify-row--lead">
                    <span class="iconbox"><Icon icon={CalendarClock} /></span>
                    <span>
                      <span class="notify-name">Interviews</span>
                      <span class="notify-sub">{interviewsLine}</span>
                    </span>
                    {#if notify.interviews}
                      <span class="toggle">
                        <Checkbox
                          checked
                          label="Interview notifications"
                          onchange={(on) => setNotify({ interviews: on })}
                        />
                      </span>
                    {:else}
                      <Button size="sm" icon={Bell} disabled={turningOn} onclick={turnOnNotifications}>Turn on</Button>
                    {/if}
                  </li>
                  {#each NOTIFY_ROWS as row (row.key)}
                    <li class={["notify-row", "notify-row--sub", !notifyLive && "notify-row--off"]}>
                      <span>
                        <span class="notify-name">{row.label}</span>
                        <span class="notify-sub">{row.example}</span>
                      </span>
                      <span class="toggle">
                        <Checkbox
                          checked={notifyLive && notify[row.key]}
                          label="{row.label} notifications"
                          disabled={!notifyLive}
                          onchange={(on) => setNotify({ [row.key]: on })}
                        />
                      </span>
                    </li>
                  {/each}
                  <li class="notify-row">
                    {#if !notify.interviews}
                      <p class="notify-foot">
                        Nothing is on yet. Turn on asks macOS once for permission; Day before and Hour before start on.
                      </p>
                    {:else if blocked}
                      <div class="notice">
                        <Icon icon={BellOff} />
                        <p class="notice__text">
                          <span class="notify-name">macOS doesn’t allow gliide’s banners</span>
                          <span class="notify-sub">
                            Turned off in System Settings → Notifications. Up next still shows every round.
                          </span>
                        </p>
                      </div>
                      <Button size="sm" icon={ExternalLink} onclick={openMacSettings}>Open macOS settings</Button>
                    {:else}
                      <p class="notify-foot">
                        {permission === "granted"
                          ? "macOS allows banners from gliide."
                          : "macOS hasn’t said yet whether gliide may show banners."}
                        They fire while gliide is open; Up next shows anything you missed.
                      </p>
                      <Button variant="ghost" size="sm" icon={ExternalLink} onclick={openMacSettings}>macOS settings</Button>
                    {/if}
                  </li>
                </ul>
              </div>
            </Card>
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

          <Card pad as="section" id="about" tabindex={-1} aria-labelledby="about-title">
            <div class="card">
              <h2 class="name" id="about-title">About</h2>
              <dl class="fields">
                <div class="field field--detail">
                  <dt class="num">gliide {update?.current ?? ""}</dt>
                  <dd>
                    {#if update?.available}
                      <Button
                        variant="primary"
                        size="sm"
                        disabled={update.state !== "idle"}
                        onclick={() => void app.installUpdate()}
                      >
                        {update.state === "installing" ? "Updating…" : "Restart to update"}
                      </Button>
                    {:else}
                      <Button size="sm" disabled={!update || update.state !== "idle"} onclick={() => void app.checkUpdate()}>
                        {update?.state === "checking" ? "Checking…" : "Check now"}
                      </Button>
                    {/if}
                  </dd>
                  <dd class="detail">
                    <p class="hint num" aria-live="polite">{updateLine}</p>
                    {#if update?.available && update.notes}<p class="notes">{update.notes}</p>{/if}
                    {#if update?.error}<p class="problem">{update.error}</p>{/if}
                  </dd>
                </div>
                <div class="field">
                  <dt>Check for updates</dt>
                  <dd class="toggle">
                    <Checkbox
                      checked={s.checkUpdates}
                      label="Check for updates"
                      onchange={(on) => save({ checkUpdates: on })}
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
  /* The agents: a 28px mark, the name (with "In use" on the one model calls run on) over its maker and what's known of
     it, then Use or How to install, the status and Check again at the end; hairlines between, as the fields'. */
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
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
    font-weight: var(--weight-medium);
  }
  .agent__title {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
  /* A release's notes, as written, under where updates stand. */
  .notes {
    margin: var(--space-1-5) 0 0;
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    color: var(--ink-2);
    white-space: pre-line;
    text-wrap: pretty;
  }
  /* Notifications: Interviews (a 28px icon box in accent, its name over a meta line, Turn on or its checkbox at the
     end), its three kinds indented to its name, then a foot saying what macOS allows; hairlines between, as the
     agents'. While off or blocked, the kinds' names are ink 3 and their checkboxes disabled. Neutral ink throughout:
     blocked is a setting, not an error. */
  .notify {
    display: grid;
    margin: var(--space-3) 0 0;
    padding: 0;
    list-style: none;
  }
  .notify-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    column-gap: var(--space-4);
    padding-block: var(--space-3);
    box-shadow: inset 0 1px 0 var(--line);
  }
  .notify-row:last-child {
    padding-bottom: 0;
  }
  .notify-row--lead {
    grid-template-columns: 28px minmax(0, 1fr) auto;
    column-gap: var(--space-3);
  }
  .notify-row--sub {
    padding-inline-start: calc(28px + var(--space-3));
  }
  .iconbox {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: var(--radius-sm);
    background: var(--accent-soft);
    color: var(--accent-text);
  }
  .notify-name {
    display: block;
    font-weight: var(--weight-medium);
  }
  .notify-sub {
    display: block;
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    color: var(--ink-3);
    text-wrap: pretty;
  }
  .notify-row--off .notify-name {
    color: var(--ink-3);
  }
  .notify-foot {
    margin: 0;
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    color: var(--ink-2);
    text-wrap: pretty;
  }
  .notice {
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr);
    align-items: center;
    column-gap: var(--space-2-5);
    min-width: 0;
    color: var(--ink-2);
  }
  .notice__text {
    margin: 0;
  }
  .notice .notify-name {
    color: var(--ink);
  }
</style>
