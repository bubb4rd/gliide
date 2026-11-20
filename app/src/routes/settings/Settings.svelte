<!--
  Settings: a modal `Sheet` over whatever screen is showing (⌘, or the sidebar's Settings; the layout mounts it while
  `shell.settings` names a pane). The rail: a search that narrows the panes to those whose settings match, the panes,
  and this build's version at its foot; the pane shown takes the rest, one at a time. Every control saves as you change
  it (fields on blur or ⏎); the theme applies at once. Agents (`Agents.svelte`) sets who does what as a trade: your mix
  (the presets and your own, by model time a month and Tailoring's quality), a line per role with every route the
  benchmark timed placed by its time against the role's target (Tailoring's a plot against quality), and the agents on
  this Mac. Feed holds the job feed's own settings (saved apart
  from the rest): whether it pulls, and whether it pulls while gliide is closed too (a launchd agent macOS may hold
  back in Login Items, read again whenever the window gains focus); each filter is Auto (worked out from your
  experience and masters, shown as it stands) or Custom;
  then its sources besides freehire: LinkedIn's search, a watchlist of careers boards and Fantastic.jobs through an
  Apify token. Notifications are off until Turn on (the only thing that asks macOS); its three kinds can't be checked
  while Interviews is off or macOS doesn't allow gliide's banners, read again whenever the window gains focus. Usage
  (`Usage.svelte`) shows the model calls' tokens and cost; Benchmark (`Benchmark.svelte`) ranks the routes per role and
  runs the suite on this Mac. Agents' presets and Benchmark's Use the best per role apply routes the same way
  (`app.applyRoutes`, with Undo).
-->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import Bell from "@lucide/svelte/icons/bell";
  import BellOff from "@lucide/svelte/icons/bell-off";
  import CalendarClock from "@lucide/svelte/icons/calendar-clock";
  import Bot from "@lucide/svelte/icons/bot";
  import Gauge from "@lucide/svelte/icons/gauge";
  import ChartColumn from "@lucide/svelte/icons/chart-column";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import Info from "@lucide/svelte/icons/info";
  import Palette from "@lucide/svelte/icons/palette";
  import RadioTower from "@lucide/svelte/icons/radio-tower";
  import Rss from "@lucide/svelte/icons/rss";
  import X from "@lucide/svelte/icons/x";
  import {
    appError,
    type Backdrop,
    feedSettings,
    setFeedSettings,
    type FeedSettingsDto,
    type FeedSettingsView,
    type NotificationPermission,
    notificationPermission,
    openLoginItems,
    openNotificationSettings,
    requestNotifications,
    type Settings,
    type Theme,
  } from "$lib/api";
  import { CATEGORY, COUNTRIES, EMPLOYMENT, SENIORITY, countryName } from "$lib/feed";
  import type { KeyBindings } from "$lib/keys";
  import { app } from "$lib/stores/app.svelte";
  import { benchmark } from "$lib/stores/benchmark.svelte";
  import { feed } from "$lib/stores/feed.svelte";
  import { shell, type SettingsPane } from "$lib/stores/shell.svelte";
  import { theme } from "$lib/stores/theme.svelte";
  import {
    Button,
    Card,
    Checkbox,
    Icon,
    Input,
    Menu,
    Nav,
    NavItem,
    ReleaseNotes,
    Segmented,
    Sheet,
    tip,
    toast,
  } from "$lib/ui";
  import Agents from "./Agents.svelte";
  import Benchmark from "./Benchmark.svelte";
  import Usage from "./Usage.svelte";

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

  /** Bumped when a save fails, so every control redraws from what's stored. */
  let resets = $state(0);

  const s = $derived(app.settings);

  // ── The panes, and the search over them ──
  /** Each pane: its name, its icon, and the words of the settings it holds, for the search. */
  const PANES: { id: SettingsPane; label: string; icon: typeof Bot; terms: string }[] = [
    {
      id: "agents",
      label: "Agents",
      icon: Bot,
      terms:
        "your mix fastest best edits model time quality target who does what reading tailoring writing coaching set all models other model suggestions from bank-aware master only on this mac claude code codex antigravity path sign in check again",
    },
    {
      id: "benchmark",
      label: "Benchmark",
      icon: Gauge,
      terms:
        "leaderboard score quality reliability speed typical time cost reference run this mac routes best per role kept edits",
    },
    { id: "usage", label: "Usage", icon: ChartColumn, terms: "tokens cost model calls export csv" },
    {
      id: "feed",
      label: "Feed",
      icon: Rss,
      terms:
        "pull new postings while gliide is closed background launchd login items check against masters seniority employment category countries visa clearance place daily checks fit threshold",
    },
    {
      id: "sources",
      label: "Sources",
      icon: RadioTower,
      terms: "feed linkedin watchlist careers boards greenhouse lever ashby workday fantastic.jobs apify token",
    },
    {
      id: "notifications",
      label: "Notifications",
      icon: Bell,
      terms: "interviews day before hour before debrief banners macos",
    },
    { id: "export", label: "Export folder", icon: FolderOpen, terms: "pdfs applications.csv choose folder" },
    {
      id: "appearance",
      label: "Appearance",
      icon: Palette,
      terms: "theme light dark system overview backdrop meadow tokyo sakura",
    },
    { id: "about", label: "About", icon: Info, terms: "version updates check now restart to update" },
  ];

  const pane = $derived(shell.settings ?? "agents");
  let query = $state("");
  let search = $state<ReturnType<typeof Input>>();
  /** The panes whose name or settings hold every word typed; those whose name holds them first. */
  const shown = $derived.by(() => {
    const words = query.toLowerCase().split(/\s+/).filter(Boolean);
    const holds = (text: string) => words.every((w) => text.toLowerCase().includes(w));
    const found = PANES.filter((p) => holds(`${p.label} ${p.terms}`));
    return [...found.filter((p) => holds(p.label)), ...found.filter((p) => !holds(p.label))];
  });

  /** ⏎ shows the first pane found; Esc clears what's typed before it closes the sheet. */
  function onSearchKey(e: KeyboardEvent) {
    if (e.key === "Enter" && shown[0]) {
      e.preventDefault();
      shell.openSettings(shown[0].id);
    } else if (e.key === "Escape" && query) {
      e.preventDefault();
      e.stopPropagation();
      query = "";
    }
  }

  /** The pane's own keys while it's shown (Benchmark's setup: ⏎ starts the run, Esc closes it). */
  let paneKeys = $state<KeyBindings>({});
  const sheetKeys = $derived<KeyBindings>({
    ...(pane === "benchmark" ? paneKeys : {}),
    "mod+,": () => shell.closeSettings(),
    "mod+f": () => search?.focus(),
    "/": () => search?.focus(),
  });

  /** Benchmark's place in the rail while a run is going: "40/324". */
  const runCount = $derived(benchmark.busy && benchmark.run ? `${benchmark.run.done}/${benchmark.run.total}` : undefined);

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

  /** What macOS says of the pull while gliide is closed: read again on focus, as you may have allowed it meanwhile. */
  function readBackground() {
    if (pane !== "feed" || !feedView) return;
    feedSettings().then(
      (v) => {
        if (feedView) feedView.background = v.background;
      },
      (e) => console.error(`feed settings: ${appError(e).message}`),
    );
  }

  /** The sentence under Pull while gliide is closed: what it does, or why it can't, or what macOS wants first. */
  const backgroundLine = $derived.by(() => {
    const status = feedView?.background;
    if (status === "unbundled") return "Only gliide installed as an app can pull while closed; this build runs outside one.";
    if (status === "unsupported") return "Pulling while gliide is closed needs macOS 13 or later.";
    if (status === "notFound") return "This copy of gliide can’t pull while closed. Install the latest release again.";
    if (heldBack) return "macOS is holding it back. Allow gliide in System Settings → General → Login Items.";
    return "Every 3 hours macOS opens gliide without a window to pull and check, within your daily checks.";
  });
  /** On, and registered, but held back in Login Items until you allow it. */
  const heldBack = $derived(feedView?.background === "requiresApproval" && !!fs?.background);
  /** macOS can run the agent here (an installed app, macOS 13 or later, its plist in the bundle). */
  const backgroundPossible = $derived(
    !!feedView && !["unbundled", "unsupported", "notFound"].includes(feedView.background),
  );

  function openLogin() {
    openLoginItems().catch((e) => toast(appError(e).message, { tone: "danger" }));
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

  function setTheme(t: Theme) {
    // Applied before the save returns, so the switch feels instant.
    theme.apply(t);
    save({ theme: t });
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
<svelte:window
  onfocus={() => {
    readPermission();
    readBackground();
  }}
/>

{#snippet rail()}
  <Input
    bind:this={search}
    size="md"
    placeholder="Search"
    aria-label="Search settings"
    spellcheck="false"
    autocomplete="off"
    enterkeyhint="search"
    bind:value={query}
    onkeydown={onSearchKey}
  />
  {#if shown.length}
    <Nav label="Settings panes">
      {#each shown as p (p.id)}
        <NavItem
          icon={p.icon}
          current={pane === p.id}
          meta={p.id === "benchmark" ? runCount : undefined}
          metaTitle="Benchmark calls run"
          onclick={() => shell.openSettings(p.id)}>{p.label}</NavItem>
      {/each}
    </Nav>
  {:else}
    <p class="none">No settings match “{query.trim()}”.</p>
  {/if}
  {#if update?.current}<p class="version num">gliide {update.current}</p>{/if}
{/snippet}

<Sheet
  label="Settings"
  title={PANES.find((p) => p.id === pane)?.label ?? "Settings"}
  {rail}
  keys={sheetKeys}
  onclose={() => shell.closeSettings()}
>
  {#if s}
    {#key resets}
      {#if pane === "agents"}
        <p class="lede">
          Each kind of work runs on the agent and model you choose, with your own plan. Faster or better is a trade:
          every route below sits where the benchmark timed it. gliide never sees a credential.
        </p>
        <Agents />
      {:else if pane === "benchmark"}
        <!-- Its lede names the best route per role, so the pane draws it under its actions. -->
        <Benchmark bind:keys={paneKeys} />
      {:else if pane === "usage"}
        <p class="lede">
          Tokens your model calls used, and what they’d cost at API prices. On a subscription that’s a measure of what
          you used, not your bill.
        </p>
        <Usage />
      {:else if pane === "feed" || pane === "sources"}
        {#if fs && feedView}
          {#key feedResets}
            {#if pane === "feed"}
              <p class="lede">
                New postings from {sources}, pulled every {fs.everyHours}
                {fs.everyHours === 1 ? "hour" : "hours"} and checked against your masters up to your daily checks.
              </p>
              <Card pad>
                <div class="card">
              <dl class="fields">
                <div class="field">
                  <dt>Pull new postings</dt>
                  <dd class="toggle">
                    <Checkbox checked={fs.enabled} label="Pull new postings" onchange={(on) => saveFeed({ enabled: on })} />
                  </dd>
                </div>
                <div class="field field--detail">
                  <dt>Pull while gliide is closed</dt>
                  <dd class="toggle">
                    <Checkbox
                      checked={fs.background}
                      label="Pull while gliide is closed"
                      disabled={!fs.enabled || !backgroundPossible}
                      onchange={(on) => saveFeed({ background: on })}
                    />
                  </dd>
                  <dd class={["detail", heldBack && "detail--action"]}>
                    <p class="hint">{backgroundLine}</p>
                    {#if heldBack}
                      <Button size="sm" icon={ExternalLink} onclick={openLogin}>Open Login Items</Button>
                    {/if}
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
                </div>
              </Card>
            {:else}
              <p class="lede">Each pull asks freehire, then every source that’s on here.</p>
              <Card pad>
                <div class="card">
              <dl class="fields" aria-label="Sources">
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
            {/if}
          {/key}
        {/if}
      {:else if pane === "notifications"}
        <p class="lede">Banners from macOS, only for what you turn on.</p>
        {#if notify}
          <Card pad>
            <div class="card">
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
      {:else if pane === "export"}
        <p class="lede">Tailored PDFs and applications.csv go here.</p>
        <Card pad>
          <div class="card">
          <div class="folder">
            <span class="path" {@attach tip(s.exportDir)}>{s.exportDir}</span>
            <Button icon={FolderOpen} onclick={chooseFolder}>Choose…</Button>
          </div>
          </div>
        </Card>
      {:else if pane === "appearance"}
        <p class="lede">Light is the default; System follows macOS. A backdrop paints the top of the Overview.</p>
        <Card pad>
          <div class="card">
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
      {:else}
        <Card pad>
          <div class="card">
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
                {#if update?.available && update.notes}<div class="notes"><ReleaseNotes notes={update.notes} /></div>{/if}
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
      {/if}
    {/key}
  {/if}
</Sheet>

<style>
  /* The rail: the search, the panes, then the version at its foot (DESIGN.md Settings). */
  .none,
  .version {
    margin: 0;
    padding-inline: var(--space-2);
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    color: var(--ink-3);
  }
  .none {
    text-wrap: pretty;
    overflow-wrap: anywhere;
  }
  .version {
    margin-top: auto;
  }
  /* A pane: its lede, then its card. */
  .card {
    display: grid;
    gap: var(--space-1);
  }
  .lede,
  .problem,
  .hint {
    margin: 0;
    text-wrap: pretty;
  }
  .lede {
    margin-bottom: var(--space-4);
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
  .fields {
    display: grid;
    margin: var(--space-3) 0 0;
  }
  /* What opens a card has no hairline over it: the card's edge is enough. */
  .card > :first-child {
    margin-top: 0;
  }
  .card > .fields:first-child > .field:first-child,
  .notify > .notify-row:first-child {
    padding-top: 0;
    box-shadow: none;
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
  /* A meta sentence with its one action at the trailing edge (Pull while gliide is closed, held back by macOS). */
  .detail--action {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
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
  /* A release's notes, as a list of its changes, under where updates stand. */
  .notes {
    margin: var(--space-3) 0 0;
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
