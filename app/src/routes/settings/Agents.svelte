<!--
  Settings → Agents (DESIGN.md Settings → Agents): faster or better is a trade, so every route sits where the benchmark
  timed it. Three cards. Your mix: the presets (Fastest, Best edits) and your own setup as named positions, each with
  its model time a month (each role's median times its calls in the last 30 days) and Tailoring's quality; hovering one
  previews it on every line; Set all to… puts every role on one agent. Who does what: a row per role, its name, what it
  covers, its calls in 30 days and its route (a `Menu` of each agent's models, grouped by agent, Other model… to type
  one), then a `RouteLine` of the routes the benchmark timed for it (Tailoring's a plot against quality) where a click
  or ⏎ moves it, and a readout of where it is or what a route in hand would change. A role whose agent can't run says so
  under its line and offers the nearest route that can, and Check again; it never switches by itself. Tailoring's row
  also holds Suggestions from. On this Mac: Claude Code, Codex and Antigravity (each checked by running it, never by
  reading its credentials; one check runs them all), each with its presence, version, plan and roles, a switch (locked
  while a role runs on it), and its path behind a disclosure. Every route change goes through `app.applyRoutes`, with
  Undo in its toast.
-->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import CircleAlert from "@lucide/svelte/icons/circle-alert";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import Gauge from "@lucide/svelte/icons/gauge";
  import RotateCw from "@lucide/svelte/icons/rotate-cw";
  import {
    type Agent,
    type AgentStatus,
    appError,
    type Mode,
    type Role,
    routeLatencies,
    type RouteLatency,
    type Settings,
    usage,
  } from "$lib/api";
  import { AGENT_LIST, AGENTS, PRESETS, ROLES, type AgentInfo, type Preset } from "$lib/agents";
  import {
    alwaysValid,
    callsByRole,
    frontier,
    keptText,
    minutesDelta,
    mixOf,
    nearest,
    positionOf,
    positionsOf,
    routeKey,
    sameRoute,
    sec,
    type Position,
    type Route,
    type Routes,
  } from "$lib/positions";
  import { app } from "$lib/stores/app.svelte";
  import { benchmark } from "$lib/stores/benchmark.svelte";
  import { shell } from "$lib/stores/shell.svelte";
  import {
    AgentMark,
    Button,
    Card,
    CompanyMark,
    Icon,
    Input,
    Menu,
    MixPicker,
    type MixCard,
    RouteLine,
    type RoutePoint,
    Segmented,
    Switch,
    type TipData,
    tip,
    toast,
  } from "$lib/ui";

  const MODES: { value: Mode; label: string }[] = [
    { value: "master-only", label: "Master only" },
    { value: "bank-aware", label: "Bank-aware" },
  ];
  /** Set all to…'s value while the roles run on different agents: none of its options. */
  const MIXED = "";
  /** A route menu's value for typing a model on an agent: `custom:<agent>`. */
  const CUSTOM = "custom:";

  const s = $derived(app.settings);
  const view = $derived(benchmark.view);

  let checking = $state(false);
  /** How fast each role's route answers on this Mac; empty until loaded. */
  let paces = $state<RouteLatency[]>([]);
  /** Each role's calls in the last 30 days; null until Usage answers. */
  let calls = $state<Record<Role, number> | null>(null);
  /** The agent whose model a role's field is typing, while it shows. */
  let typing = $state<Record<Role, Agent | null>>({ read: null, tailor: null, write: null, coach: null });
  let drafts = $state<Record<Role, string>>({ read: "", tailor: "", write: "", coach: "" });
  /** A route in hand on a line (hovered or focused), and a named position in hand on Your mix. */
  let hover = $state<{ role: Role; key: string } | null>(null);
  let preview = $state<string | null>(null);

  onMount(() => {
    void benchmark.refresh();
    usage().then(
      (v) => (calls = callsByRole(v.rows, v.today)),
      (e) => console.error(`usage: ${appError(e).message}`),
    );
  });

  // Each role's pace, read again after every save (a route or model may have changed).
  $effect(() => {
    if (!s) return;
    routeLatencies().then(
      (v) => (paces = v),
      (e) => console.error(`route latencies: ${appError(e).message}`),
    );
  });

  async function save(patch: Partial<Settings>): Promise<boolean> {
    if (!app.settings) return false;
    try {
      await app.saveSettings({ ...app.settings, ...patch });
      return true;
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
      return false;
    }
  }

  // ── Words ──
  const nameOf = (role: Role) => ROLES.find((r) => r.id === role)!.name;
  const lower = (role: Role) => nameOf(role).toLowerCase();
  function andList(words: string[]) {
    return words.length < 2 ? (words[0] ?? "") : `${words.slice(0, -1).join(", ")} and ${words.at(-1)}`;
  }
  const plural = (n: number, word: string) => `${n.toLocaleString()} ${word}${n === 1 ? "" : "s"}`;
  const model = (r: Route) => app.modelName(r.model);

  // ── Agents: who can run work ──
  const statusOf = (agent: Agent) => app.agents?.find((a) => a.agent === agent) ?? null;
  /** Found and signed in: it can be turned on. */
  const ready = (status: AgentStatus | null) => !!status?.found && status.signedIn === true;
  /** On, found and signed in, and it lists the model: a role can run there. */
  const canRun = (r: Route) => app.offers(r.agent) && app.lists(r.agent, r.model);

  /** Why an agent can't run work, or null (and null while it hasn't been checked). */
  function why(agent: Agent) {
    const st = statusOf(agent);
    const n = AGENTS[agent].name;
    if (!st) return null;
    if (!st.found) return { tone: "danger", short: "not installed", text: `${n} isn’t installed` } as const;
    if (st.signedIn === false) return { tone: "warn", short: "signed out", text: `${n} is signed out` } as const;
    if (st.signedIn === null)
      return { tone: "neutral", short: "sign-in unknown", text: `Couldn’t check ${n}’s sign-in` } as const;
    if (!s?.agents[agent].enabled) return { tone: "neutral", short: "off", text: `${n} is off` } as const;
    return null;
  }

  /** The presence dot's tone and its words (the icon's tooltip): amber signed out, as the sidebar's. */
  function presence(status: AgentStatus | null) {
    if (checking) return { tone: "neutral", text: "Checking…" } as const;
    if (!status) return { tone: "neutral", text: "Not checked" } as const;
    if (!status.found) return { tone: "danger", text: "Not installed" } as const;
    if (status.signedIn === false) return { tone: "warn", text: "Signed out" } as const;
    if (status.signedIn === null) return { tone: "neutral", text: "Couldn’t check sign-in" } as const;
    return { tone: "gain", text: status.plan ? `Signed in · ${status.plan}` : "Signed in" } as const;
  }

  /** The roles that run on `agent`, in Settings' order. */
  const rolesOn = (agent: Agent) => ROLES.filter((r) => s?.routes[r.id] === agent);

  /** One check, every agent. */
  async function refreshAgents() {
    checking = true;
    await app.refreshAgents();
    checking = false;
  }

  // ── Routes: where each role is, and the routes it could take ──
  const routes = $derived<Routes>({
    read: { agent: app.routes.read.info.id, model: app.routes.read.model },
    tailor: { agent: app.routes.tailor.info.id, model: app.routes.tailor.model },
    write: { agent: app.routes.write.info.id, model: app.routes.write.model },
    coach: { agent: app.routes.coach.info.id, model: app.routes.coach.model },
  });
  const boards = $derived(view?.roles ?? []);
  const boardOf = (role: Role) => boards.find((b) => b.role === role) ?? null;
  const positions = $derived(
    Object.fromEntries(
      ROLES.map(({ id }) => {
        const b = boards.find((x) => x.role === id);
        return [id, b ? positionsOf(b) : []];
      }),
    ) as Record<Role, Position[]>,
  );
  const targetOf = (role: Role) => boardOf(role)?.targetS ?? 0;

  /** Runs `role` on `r` from now on (Undo in the toast). */
  function setRoute(role: Role, r: Route) {
    if (sameRoute(routes[role], r)) return;
    typing[role] = null;
    hover = null;
    void app.applyRoutes({ [role]: { agent: r.agent, model: r.model } });
  }

  /** Under a role's line: its route's median time on this Mac once five calls are timed; null while what's measured
   *  is another route's (a save not read back yet). */
  function paceOf(role: Role) {
    const p = paces.find((x) => x.role === role);
    const r = routes[role];
    if (!p || p.agent !== r.agent || p.model !== r.model) return null;
    const n = plural(p.calls, "call");
    if (p.p50Ms !== null) return `${(p.p50Ms / 1000).toFixed(1)} s typical here · ${n}`;
    return p.calls ? `${n} timed here so far` : "Not timed here yet";
  }

  /** Where a position's time came from. */
  const sourceOf = (p: Position) =>
    p.source === "thisMac" ? "This Mac’s runs" : p.source === "reference" ? "gliide’s reference" : "Your calls";

  function pointTip(role: Role, p: Position): TipData {
    const cur = sameRoute(p, routes[role]);
    const w = why(p.agent);
    const rows: NonNullable<TipData["rows"]> = [
      { label: "Median", value: sec(p.seconds), sub: sourceOf(p) },
      { label: "Target", value: `${targetOf(role)} s` },
    ];
    if (p.valid !== null)
      rows.push({
        label: "Valid first try",
        value: `${Math.round(p.valid * 100)}%`,
        tone: p.valid < 1 ? "warn" : undefined,
      });
    if (p.quality !== null) rows.push({ label: "Quality", value: Math.round(p.quality) });
    const kept = keptText(p.kept);
    if (kept) rows.push({ label: "Your edits", value: kept });
    const pace = cur ? paceOf(role) : null;
    if (pace) rows.push({ label: "Here", value: pace.replace(" typical here", "").replace(" here", "") });
    return {
      title: `${AGENTS[p.agent].name} · ${model(p)}`,
      rows,
      note: w
        ? `${w.text}, so it can’t take ${lower(role)}.`
        : !app.lists(p.agent, p.model)
          ? `${AGENTS[p.agent].name} doesn’t list it on this Mac.`
          : cur
            ? `${nameOf(role)} runs here.`
            : `Choose it to run ${lower(role)} here.`,
    };
  }

  const presetOfId = (id: string | null) => PRESETS.find((p) => p.id === id) ?? null;
  const inHand = $derived(presetOfId(preview));

  function pointsOf(role: Role): RoutePoint[] {
    return positions[role].map((p) => {
      const cur = sameRoute(p, routes[role]);
      const blocked = why(p.agent)?.short ?? (statusOf(p.agent) && !app.lists(p.agent, p.model) ? "not listed" : null);
      const timed = `${sec(p.seconds)}${p.quality !== null ? `, quality ${Math.round(p.quality)}` : ""}`;
      return {
        key: p.key,
        icon: AGENTS[p.agent].icon,
        label: model(p),
        seconds: p.seconds,
        quality: p.quality,
        invalid: !alwaysValid(p),
        blocked,
        current: cur,
        preview: !!inHand && sameRoute(inHand.routes[role], p) && !cur,
        description: `${AGENTS[p.agent].name} ${model(p)}, ${timed}${alwaysValid(p) ? "" : ", not valid every first try"}${blocked ? `, ${blocked}` : ""}`,
        tip: pointTip(role, p),
      };
    });
  }

  /** Tailoring's plot: the quality range its routes span, out to the next 5. */
  function plotOf(role: Role) {
    if (role !== "tailor") return null;
    const q = positions[role].map((p) => p.quality).filter((x): x is number => x !== null);
    if (!q.length) return null;
    const min = Math.max(0, Math.floor((Math.min(...q) - 2) / 5) * 5);
    const max = Math.min(100, Math.ceil((Math.max(...q) + 2) / 5) * 5);
    return { min, max: Math.max(max, min + 5) };
  }

  function choosePoint(role: Role, key: string) {
    const p = positions[role].find((x) => x.key === key);
    if (p && canRun(p)) setRoute(role, p);
  }

  /** Under a role's line: where it is, or what the route or named position in hand would change. */
  function readout(role: Role): { name: string; text: string; here: string | null } {
    const cur = routes[role];
    const here = positionOf(positions[role], cur);
    const n = calls?.[role] ?? null;
    let alt: Route | null = null;
    let prefix = "";
    if (hover && hover.role === role && hover.key !== routeKey(cur)) {
      const key = hover.key;
      alt = positions[role].find((p) => p.key === key) ?? null;
    } else if (inHand) {
      const r = inHand.routes[role];
      if (sameRoute(r, cur)) return { name: inHand.name, text: ` keeps ${lower(role)} on ${model(cur)}.`, here: null };
      alt = r;
      prefix = `${inHand.name} moves it to `;
    }
    if (alt) {
      const a = positionOf(positions[role], alt);
      const w = why(alt.agent);
      const but = w ? `, but ${w.text.charAt(0).toLowerCase()}${w.text.slice(1)}` : "";
      if (!a) return { name: `${prefix}${model(alt)}`, text: ` isn’t benchmarked for ${lower(role)}${but}.`, here: null };
      if (!here)
        return {
          name: `${prefix}${model(alt)}`,
          text: `: ${sec(a.seconds)}${a.quality !== null ? `, quality ${Math.round(a.quality)}` : ""}${but}.`,
          here: null,
        };
      const parts: string[] = [];
      const d = a.seconds - here.seconds;
      parts.push(Math.abs(d) < 0.05 ? "the same time" : `${sec(Math.abs(d))} ${d < 0 ? "faster" : "slower"}`);
      if (a.quality !== null && here.quality !== null) {
        const dq = Math.round(a.quality) - Math.round(here.quality);
        parts.push(dq === 0 ? "the same quality" : `quality ${Math.abs(dq)} ${dq > 0 ? "higher" : "lower"}`);
      }
      if (alwaysValid(a) !== alwaysValid(here))
        parts.push(alwaysValid(a) ? "valid every first try" : "not valid every first try");
      let monthly: string | null = null;
      if (n) {
        const m = (d * n) / 60;
        monthly =
          Math.abs(m) < 1
            ? `Under a minute a month either way at ${plural(n, "call")}`
            : `About ${Math.round(Math.abs(m))} min a month ${m < 0 ? "less" : "more"} at ${plural(n, "call")}`;
      }
      return { name: `${prefix}${model(alt)}`, text: `: ${parts.join(", ")}${but}.`, here: monthly };
    }
    if (!here)
      return {
        name: model(cur),
        text: ` on ${AGENTS[cur.agent].name} isn’t benchmarked, so it has no place on the line yet.`,
        here: paceOf(role),
      };
    const t = targetOf(role);
    const d = here.seconds - t;
    const facts = [
      `${sec(here.seconds)}, ${Math.abs(d) < 0.05 ? "on target" : `${sec(Math.abs(d))} ${d < 0 ? "under" : "over"} target`}`,
    ];
    if (here.quality !== null) facts.push(`quality ${Math.round(here.quality)}`);
    if (!alwaysValid(here)) facts.push("not valid every first try");
    if (role === "write") facts.push("timed only: cover letters have no quality score");
    return { name: model(cur), text: ` ${facts.join(" · ")}`, here: paceOf(role) };
  }

  /** A role whose agent can't run: what's wrong, how to fix it, and the nearest route that can run. */
  function stuckOf(role: Role) {
    const cur = routes[role];
    const w = why(cur.agent);
    if (!w) return null;
    const info = AGENTS[cur.agent];
    return {
      tone: w.tone,
      head: `${w.text}, so ${lower(role)} ${w.tone === "neutral" ? "may" : "will"} fail.`,
      short: w.short,
      info,
      near: nearest(positions[role], cur, targetOf(role) || 1, canRun),
    };
  }

  // ── Your mix: whole setups as positions ──
  /** A preset's agents are on and signed in, and each lists the model it names (Claude Code's aliases always do). */
  const presetReady = (p: Preset) => ROLES.every(({ id }) => canRun(p.routes[id]));
  /** Every role already runs on the route `p` names for it. */
  const presetOn = (p: Preset) => ROLES.every(({ id }) => sameRoute(p.routes[id], routes[id]));
  const totalCalls = $derived(calls ? Object.values(calls).reduce((a, b) => a + b, 0) : 0);

  /** A setup's time and quality, or why it has no total. */
  function mixFor(r: Routes) {
    const m = mixOf(r, boards, calls ?? { read: 0, tailor: 0, write: 0, coach: 0 });
    const noTotal = !view
      ? "Reading the benchmark…"
      : !calls
        ? "Counting your calls…"
        : !totalCalls
          ? "No model calls in the last 30 days to weigh it by"
          : m.missing.length
            ? `${andList(m.missing.map(nameOf))} ${m.missing.length > 1 ? "aren’t" : "isn’t"} benchmarked on it`
            : undefined;
    return { ...m, minutes: noTotal ? null : m.minutes, noTotal };
  }

  const yours = $derived(mixFor(routes));
  const onPreset = $derived(PRESETS.find(presetOn) ?? null);

  /** "27 min less, quality 6 lower". */
  function deltaOf(m: ReturnType<typeof mixFor>) {
    if (m.minutes === null || yours.minutes === null) return null;
    const parts = [minutesDelta(m.minutes - yours.minutes)];
    if (m.quality !== null && yours.quality !== null) {
      const dq = Math.round(m.quality) - Math.round(yours.quality);
      parts.push(dq === 0 ? "the same quality" : `quality ${Math.abs(dq)} ${dq > 0 ? "higher" : "lower"}`);
    }
    return parts.join(", ");
  }

  /** "Needs Antigravity, which isn’t installed." */
  function needs(p: Preset) {
    const agents = [
      ...new Set(
        ROLES.map(({ id }) => p.routes[id])
          .filter((r) => !canRun(r))
          .map((r) => r.agent),
      ),
    ];
    const w = agents.map((a) => why(a)).find(Boolean);
    const state = !w
      ? "doesn’t list its model on this Mac"
      : w.short === "not installed"
        ? "isn’t installed"
        : w.short === "off"
          ? "is off"
          : w.short === "signed out"
            ? "is signed out"
            : "couldn’t be checked";
    return `Needs ${andList(agents.map((a) => AGENTS[a].name))}, which ${state}.`;
  }

  const mixCards = $derived.by((): MixCard[] => {
    const cards: MixCard[] = PRESETS.map((p) => {
      const m = mixFor(p.routes);
      const on = presetOn(p);
      const ok = presetReady(p);
      return {
        key: p.id,
        name: p.name,
        minutes: m.minutes,
        noTotal: m.noTotal,
        quality: m.quality,
        on,
        disabled: on || ok ? null : needs(p),
        foot: on ? "Where you are" : ok ? deltaOf(m) : needs(p),
      };
    });
    if (onPreset) {
      cards.push({
        key: "own",
        name: "Your own",
        minutes: null,
        quality: null,
        disabled: "Choose a route on any line below",
        note: "Choose a route on any line below to make your own mix.",
      });
    } else {
      const blocked = ROLES.filter(({ id }) => !!why(routes[id].agent));
      const w = blocked.length ? why(routes[blocked[0].id].agent) : null;
      cards.push({
        key: "own",
        name: "Your own",
        minutes: yours.minutes,
        noTotal: yours.noTotal,
        quality: yours.quality,
        here: true,
        foot: w ? `${w.text}, so ${andList(blocked.map((r) => r.name.toLowerCase()))} can’t run.` : "Where you are",
        footTone: w ? "warn" : null,
      });
    }
    return cards;
  });

  function choosePreset(id: string) {
    const p = presetOfId(id);
    preview = null;
    if (p && presetReady(p)) void app.applyRoutes(p.routes);
  }

  // ── Set all to…: the agents on and signed in; every role on one, each with that agent's own model for it ──
  const option = (a: AgentInfo) => ({ value: a.id as string, label: a.name, meta: statusOf(a.id)?.plan ?? undefined });
  const allOptions = $derived(AGENT_LIST.filter((a) => app.offers(a.id)).map(option));
  const allOn = $derived(s && new Set(Object.values(s.routes)).size === 1 ? s.routes.read : MIXED);

  /** Runs every role in `roles` on `agent`, each with that agent's own model for it (Undo in the toast). */
  function moveRoles(roles: Role[], agent: Agent) {
    if (!s) return;
    void app.applyRoutes(Object.fromEntries(roles.map((id) => [id, { agent, model: s.agents[agent].models[id] }])));
  }

  function setAll(value: string) {
    if (value) moveRoles(ROLES.map((r) => r.id), value as Agent);
  }

  // ── A role's route menu: each agent's models, grouped, then Other model… to type one ──
  function routeOptions(role: Role) {
    const cur = routes[role];
    const agents = AGENT_LIST.filter((a) => a.id === cur.agent || app.offers(a.id));
    return agents.flatMap((a) => {
      const listed = statusOf(a.id)?.models ?? [];
      const models =
        a.id !== cur.agent || listed.some((m) => m.id === cur.model)
          ? listed
          : [...listed, { id: cur.model, name: model(cur) }];
      return [
        ...models.map((m) => {
          const p = positionOf(positions[role], { agent: a.id, model: m.id });
          return {
            value: routeKey({ agent: a.id, model: m.id }),
            label: m.name,
            meta: p ? `${sec(p.seconds)}${p.quality !== null ? ` · ${Math.round(p.quality)}` : ""}` : undefined,
            group: a.name,
          };
        }),
        { value: `${CUSTOM}${a.id}`, label: "Other model…", group: a.name },
      ];
    });
  }
  const agentOfValue = (value: string) =>
    (value.startsWith(CUSTOM) ? value.slice(CUSTOM.length) : value.slice(0, value.indexOf(":"))) as Agent;

  function chooseRoute(role: Role, value: string) {
    if (value.startsWith(CUSTOM)) {
      typing[role] = agentOfValue(value);
      drafts[role] = "";
      tick().then(() => document.getElementById(`${role}-custom`)?.focus());
      return;
    }
    const agent = agentOfValue(value);
    setRoute(role, { agent, model: value.slice(agent.length + 1) });
  }

  /** A typed model, on blur or ⏎: the role runs on it (off the line until it's benchmarked); empty changes nothing. */
  function commitModel(role: Role) {
    const agent = typing[role];
    const value = drafts[role].trim();
    typing[role] = null;
    if (agent && value) setRoute(role, { agent, model: value });
  }

  function onModelKey(e: KeyboardEvent, role: Role) {
    if (e.key === "Enter") (e.currentTarget as HTMLInputElement).blur();
    else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      drafts[role] = "";
      typing[role] = null;
    }
  }

  /** Turns an agent on (offered by the route menus, may run calls) or off (never runs one; still checked). */
  function setEnabled(agent: Agent, enabled: boolean) {
    if (s) void save({ agents: { ...s.agents, [agent]: { ...s.agents[agent], enabled } } });
  }

  /**
   * Why an agent's switch can't change, or null. An agent that's on can always go off, unless a role runs on it; one
   * that's off goes on only once it's found and signed in.
   */
  function lockOf(info: AgentInfo, status: AgentStatus | null, on: boolean) {
    if (status?.found === false) return `${info.name} isn’t installed on this Mac.`;
    const roles = rolesOn(info.id);
    if (roles.length) {
      const one = roles.length === 1;
      return `${andList(roles.map((r) => r.name))} ${one ? "runs" : "run"} on ${info.name}. Move ${one ? "it" : "them"} to another agent first.`;
    }
    if (on || ready(status)) return null;
    if (status?.signedIn === false)
      return `Sign in to ${info.name} to turn it on. Run ${info.bin} in Terminal, then Check again.`;
    return `Couldn’t check ${info.name}’s sign-in. Check again first.`;
  }

  function visit(url: string) {
    openUrl(url).catch((e) => toast(`Couldn’t open ${url}: ${appError(e).message}`, { tone: "danger" }));
  }

  /** "/Users/jane/.nvm/versions/node/v20.19.0/bin/claude" → "~/.nvm/…/bin/claude"; in full in its tooltip. */
  function shortPath(path: string) {
    const parts = path.replace(/^\/Users\/[^/]+(?=\/|$)/, "~").split("/");
    return parts.length > 5 ? [...parts.slice(0, 2), "…", ...parts.slice(-2)].join("/") : parts.join("/");
  }

  // ── Each agent's details: its path, behind the row's disclosure (one open at a time), then edited behind Change
  //    path (Save, Discard, or Remove to find it again) ──
  let detailsOpen = $state<Agent | null>(null);
  let pathOpen = $state<Agent | null>(null);
  let pathDraft = $state("");
  let pathBusy = $state(false);
  const pathUnchanged = $derived(pathOpen !== null && pathDraft.trim() === (s?.agents[pathOpen].path ?? ""));

  function toggleDetails(agent: Agent) {
    pathOpen = null;
    pathDraft = "";
    detailsOpen = detailsOpen === agent ? null : agent;
  }

  function openPath(agent: Agent) {
    pathDraft = s?.agents[agent].path ?? "";
    detailsOpen = agent;
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

  /** "Nov 16" for the reference's day. */
  const shortDay = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });
  function day(at: string) {
    const ymd = /^(\d{4})-(\d{2})-(\d{2})$/.exec(at);
    const d = ymd ? new Date(+ymd[1], +ymd[2] - 1, +ymd[3]) : new Date(at);
    return Number.isNaN(d.getTime()) ? at : shortDay.format(d);
  }
</script>

{#if s}
  <div class="cards">
    <Card pad>
      <div class="card">
        <div class="head">
          <p class="label">Your mix</p>
          {#if allOptions.some((o) => o.value !== allOn)}
            <Menu
              value={allOn}
              options={allOptions}
              label="Set all to"
              size="sm"
              variant="ghost"
              placement="bottom-end"
              onchange={setAll}
            >
              {#snippet trigger()}Set all to…{/snippet}
              {#snippet lead(o)}
                <AgentMark
                  size={16}
                  src={AGENTS[o.value as Agent].icon}
                  tone={presence(statusOf(o.value as Agent)).tone}
                />
              {/snippet}
            </Menu>
          {/if}
        </div>
        <MixPicker cards={mixCards} label="Your mix" onchoose={choosePreset} onpreview={(k) => (preview = k)} />
        <p class="foot meta">
          Model time is each role’s benchmark median times its calls in the last 30 days{calls
            ? ` (${plural(totalCalls, "call")} in all)`
            : ""}. Quality is the benchmark’s score for Tailoring’s route, out of 100.
        </p>
      </div>
    </Card>

    <Card pad>
      <div class="card">
        <div class="head">
          <p class="label">Who does what</p>
          <span class="legend meta" aria-hidden="true">
            <span><i class="key key--tick"></i>Target</span>
            <span><i class="key key--ring"></i>Runs here</span>
            <span class="key--alert"><Icon icon={CircleAlert} size={12} weight="medium" />Not valid every first try</span>
          </span>
        </div>
        {#each ROLES as role (role.id)}
          {@const cur = routes[role.id]}
          {@const ro = readout(role.id)}
          {@const stuck = stuckOf(role.id)}
          <div class="role">
            <div class="role__about">
              <p class="role__name">{role.name}</p>
              <p class="meta role__covers">{role.covers}</p>
              {#if calls}<p class="meta num role__calls">{plural(calls[role.id], "call")} in 30 days</p>{/if}
              <div class="role__route">
                {#if typing[role.id]}
                  <Input
                    id="{role.id}-custom"
                    size="md"
                    bind:value={drafts[role.id]}
                    placeholder="Model name"
                    aria-label="{role.name}: {AGENTS[typing[role.id]!].name} model name"
                    spellcheck="false"
                    autocomplete="off"
                    onblur={() => commitModel(role.id)}
                    onkeydown={(e: KeyboardEvent) => onModelKey(e, role.id)}
                  />
                {:else}
                  <Menu
                    value={routeKey(cur)}
                    options={routeOptions(role.id)}
                    label="{role.name} runs on"
                    size="sm"
                    onchange={(v) => chooseRoute(role.id, v)}
                  >
                    {#snippet lead(o)}
                      <AgentMark
                        size={16}
                        src={AGENTS[agentOfValue(o.value)].icon}
                        tone={presence(statusOf(agentOfValue(o.value))).tone}
                      />
                    {/snippet}
                  </Menu>
                {/if}
              </div>
            </div>
            <div class="role__main">
              {#if !view}
                <p class="meta empty">
                  {benchmark.failed ? `Couldn’t read the benchmark: ${benchmark.failed}` : "Reading the benchmark…"}
                </p>
              {:else}
                <RouteLine
                  points={pointsOf(role.id)}
                  target={targetOf(role.id)}
                  plot={plotOf(role.id)}
                  frontier={role.id === "tailor" ? frontier(positions[role.id]) : []}
                  label="{role.name}: routes by time{role.id === 'tailor' ? ' and quality' : ''}"
                  onchoose={(k) => choosePoint(role.id, k)}
                  onpreview={(k) => (hover = k ? { role: role.id, key: k } : hover?.role === role.id ? null : hover)}
                />
                <p class="readout" aria-live="polite">
                  <b>{ro.name}</b>{ro.text}
                  {#if ro.here}<span class="readout__here num">{ro.here}</span>{/if}
                </p>
              {/if}
              {#if stuck}
                <div class="prompt prompt--{stuck.tone}" role="status">
                  <Icon icon={CircleAlert} />
                  <div>
                    <p class="prompt__head">{stuck.head}</p>
                    <p class="prompt__how">
                      {#if stuck.short === "signed out"}Run <code>{stuck.info.bin}</code> in Terminal and sign in, then
                        Check again.{:else if stuck.short === "not installed"}Install {stuck.info.name}, then Check
                        again.{:else if stuck.short === "off"}Turn {stuck.info.name} on below.{:else}Check again.{/if}
                      {stuck.near ? "Or take the nearest route that can run:" : "No other benchmarked route can run now."}
                    </p>
                  </div>
                  <div class="prompt__actions">
                    {#if stuck.near}
                      {@const near = stuck.near}
                      <Button size="sm" variant="primary" onclick={() => setRoute(role.id, near)}>
                        <CompanyMark size={16} src={AGENTS[near.agent].icon} />Move to {model(near)} ·
                        <span class="num">{sec(near.seconds)}</span>
                      </Button>
                    {/if}
                    <Button size="sm" variant="ghost" icon={RotateCw} disabled={checking} onclick={refreshAgents}
                      >{checking ? "Checking…" : "Check again"}</Button
                    >
                  </div>
                </div>
              {/if}
              {#if role.id === "tailor"}
                <div class="pref">
                  <div>
                    <span class="pref__name">Suggestions from</span>
                    <span class="meta pref__hint">
                      Bank-aware brings in an entry from Experience where it fits a posting better than one the master
                      shows.
                    </span>
                  </div>
                  <Segmented
                    options={MODES}
                    value={s.suggestionsFrom}
                    label="Suggestions from"
                    onchange={(m) => save({ suggestionsFrom: m })}
                  />
                </div>
              {/if}
            </div>
          </div>
        {/each}
        <div class="foot foot--row">
          <p class="meta">
            Placed by the benchmark: this Mac’s runs where there are any, else gliide’s reference{view
              ? ` (${day(view.referenceDate)})`
              : ""}. Each line runs to twice its role’s target, so the targets line up; slower routes are counted at
            its end, and every route is in its role’s menu.
          </p>
          <Button size="sm" variant="ghost" icon={Gauge} onclick={() => shell.openSettings("benchmark")}>Benchmark</Button>
        </div>
      </div>
    </Card>

    <Card pad>
      <div class="card">
        <div class="head">
          <p class="label" id="agents-list">On this Mac</p>
          <Button size="sm" variant="ghost" icon={RotateCw} disabled={checking} onclick={refreshAgents}>
            {checking ? "Checking…" : "Check again"}
          </Button>
        </div>
        <ul class="agents" aria-labelledby="agents-list">
          {#each AGENT_LIST as info (info.id)}
            {@const status = statusOf(info.id)}
            {@const dot = presence(status)}
            {@const roles = rolesOn(info.id)}
            {@const missing = status?.found === false}
            {@const on = s.agents[info.id].enabled && !missing}
            {@const lock = lockOf(info, status, on)}
            {@const path = s.agents[info.id].path}
            {@const open = detailsOpen === info.id}
            <li class={["agent", !on && "agent--off"]}>
              <span class="agent__mark" role="img" aria-label="{info.name}: {dot.text}" {@attach tip(dot.text)}>
                {#if missing}
                  <CompanyMark size={28} src={info.icon} />
                {:else}
                  <AgentMark src={info.icon} tone={dot.tone} />
                {/if}
              </span>
              <span class="agent__name">
                <span class="agent__title">{info.name}</span>
                {#if missing}
                  <span class="meta">Not installed</span>
                {:else if !on}
                  <span class="meta">Off</span>
                {:else}
                  {#if status?.version}<span class="meta num">v{status.version}</span>{/if}
                  {#if status?.plan}<span class="meta">{status.plan} plan</span>{/if}
                  {#if roles.length}<span class="meta"
                      >{roles.length === ROLES.length ? "Every role" : roles.map((r) => r.name).join(", ")}</span
                    >{/if}
                {/if}
              </span>
              <span class="agent__end">
                {#if missing}
                  <Button size="sm" variant="ghost" icon={ExternalLink} onclick={() => visit(info.install)}
                    >How to install</Button
                  >
                {/if}
                <Switch
                  checked={on}
                  label={info.name}
                  locked={!!lock}
                  {@attach tip(lock)}
                  onchange={(next) => setEnabled(info.id, next)}
                />
                <Button
                  size="sm"
                  variant="ghost"
                  square
                  icon={open ? ChevronDown : ChevronRight}
                  aria-expanded={open}
                  aria-controls="{info.id}-details"
                  aria-label="{info.name} details"
                  onclick={() => toggleDetails(info.id)}
                />
              </span>
              {#if on && !roles.length && status?.problem}
                <!-- Its problem names commands in backticks ("Run `codex` in Terminal…"): shown as code. -->
                <p class="problem problem--{dot.tone} agent__more">
                  {#each status.problem.split("`") as part, i (i)}{#if i % 2}<code>{part}</code>{:else}{part}{/if}{/each}
                </p>
              {/if}
              {#if open}
                <div class="agent__more" id="{info.id}-details">
                  {#if pathOpen === info.id}
                    <form class="path-edit" onsubmit={submitPath} {@attach discardOnEscape}>
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
                        <Button variant="ghost" size="sm" kbd="Esc" disabled={pathBusy} onclick={closePath}
                          >Discard</Button
                        >
                        <Button type="submit" variant="primary" size="sm" kbd="⏎" disabled={pathBusy || pathUnchanged}>
                          {pathBusy ? "Saving…" : "Save"}
                        </Button>
                      </div>
                    </form>
                  {:else}
                    <div class="path-row">
                      <span class="path-label">Path</span>
                      <span class="path-value">
                        {#if path}
                          <code {@attach tip(path)}>{shortPath(path)}</code>
                        {:else if status?.path}
                          <code {@attach tip(status.path)}>{shortPath(status.path)}</code> · found on your PATH
                        {:else}
                          {missing ? "Not found on your PATH" : "Automatic: found on your PATH"}
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
                </div>
              {/if}
            </li>
          {/each}
        </ul>
      </div>
    </Card>
  </div>
{/if}

<style>
  .cards {
    display: grid;
    gap: var(--space-4);
    container-type: inline-size;
  }
  .card {
    display: grid;
    gap: var(--space-1);
  }
  /* A card's label with its action at the end; as tall as a small button whether it shows or not. */
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
    min-height: var(--control-sm);
    padding-bottom: var(--space-1-5);
  }
  .foot {
    margin: var(--space-2) 0 0;
    color: var(--ink-3);
    text-wrap: pretty;
  }
  .foot--row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    padding-top: var(--space-3);
    box-shadow: inset 0 1px 0 var(--line);
  }
  .foot--row p {
    margin: 0;
    text-wrap: pretty;
  }
  /* The lines' key: the target's tick, the ring where a role runs, the alert. */
  .legend {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--space-1) var(--space-3);
    color: var(--ink-3);
  }
  .legend > span {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1-5);
    white-space: nowrap;
  }
  .key {
    display: inline-block;
    flex: none;
  }
  .key--tick {
    width: 1px;
    height: var(--space-3);
    background: var(--ink-2);
  }
  .key--ring {
    width: var(--space-2-5);
    height: var(--space-2-5);
    border-radius: var(--radius-xs);
    outline: 1.5px solid var(--accent);
  }
  .key--alert :global(svg) {
    color: var(--warn);
  }
  /* A role: its name, what it covers, its calls and its route at the start; its line, readout, prompt (and on
     Tailoring, Suggestions from) beside them; hairlines between. Under 560px wide, one over the other. */
  .role {
    display: grid;
    grid-template-columns: minmax(136px, 168px) minmax(0, 1fr);
    column-gap: var(--space-6);
    row-gap: var(--space-3);
    padding-block: var(--space-4);
    box-shadow: inset 0 1px 0 var(--line);
  }
  .role p {
    margin: 0;
  }
  .role__name {
    font-weight: var(--weight-medium);
  }
  .role__covers {
    margin-top: var(--space-0-5);
    color: var(--ink-3);
    text-wrap: pretty;
  }
  .role__calls {
    margin-top: var(--space-1);
    color: var(--ink-3);
  }
  .role__route {
    margin-top: var(--space-2-5);
    max-width: 100%;
  }
  .role__main {
    min-width: 0;
  }
  .empty {
    padding-block: var(--space-6);
    color: var(--ink-3);
  }
  .readout {
    margin-top: var(--space-1);
    min-height: calc(var(--leading-sm) * 2);
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    color: var(--ink-2);
    text-wrap: pretty;
  }
  .readout b {
    font-weight: var(--weight-medium);
    color: var(--ink);
  }
  .readout__here {
    display: block;
    color: var(--ink-3);
  }
  .pref {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    margin-top: var(--space-3);
    padding-top: var(--space-3);
    box-shadow: inset 0 1px 0 var(--line);
  }
  .pref__name {
    color: var(--ink-2);
  }
  .pref__hint {
    display: block;
    margin-top: var(--space-0-5);
    color: var(--ink-3);
    text-wrap: pretty;
  }
  @container (width < 560px) {
    .role {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  code {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
  }
  /* Under a role whose agent can't run: what's wrong and how to fix it, then Move to the nearest route that can, and
     Check again. In its agent's tone: amber signed out, danger not installed, neutral sign-in unknown or off. */
  .prompt {
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr);
    column-gap: var(--space-2-5);
    margin-top: var(--space-2);
    padding: var(--space-3);
    border-radius: var(--radius-md);
    background: var(--warn-soft);
  }
  .prompt > :global(svg) {
    margin-top: 2px;
    color: var(--warn);
  }
  .prompt--danger {
    background: var(--danger-soft);
  }
  .prompt--danger > :global(svg) {
    color: var(--danger);
  }
  .prompt--neutral {
    background: var(--sunken);
  }
  .prompt--neutral > :global(svg) {
    color: var(--ink-2);
  }
  .prompt__head,
  .prompt__how {
    margin: 0;
    text-wrap: pretty;
  }
  .prompt__head {
    font-weight: var(--weight-medium);
  }
  .prompt__how {
    color: var(--ink-2);
  }
  .prompt__actions {
    grid-column: 2;
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    margin-top: var(--space-2-5);
  }
  /* The agents: a 28px icon with its presence dot (its state in words in its tooltip), the name and version (or Off,
     Not installed), its plan and the roles it runs ("Every role"), then How to install, the switch and the
     disclosure; one line each, hairlines between. */
  .agents {
    display: grid;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .agent {
    display: grid;
    grid-template-columns: 28px minmax(0, 1fr) auto;
    align-items: center;
    column-gap: var(--space-3);
    padding-block: var(--space-2-5);
    box-shadow: inset 0 1px 0 var(--line);
  }
  .agent:last-child {
    padding-bottom: 0;
  }
  .agent__mark {
    display: flex;
  }
  .agent__name {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    min-width: 0;
    white-space: nowrap;
  }
  .agent__title {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: var(--weight-medium);
  }
  .agent--off .agent__title {
    color: var(--ink-2);
  }
  .agent__end {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .agent__more {
    grid-column: 2 / -1;
  }
  .problem,
  .hint {
    margin: 0;
    text-wrap: pretty;
  }
  .problem {
    margin-top: var(--space-2);
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-sm);
    color: var(--danger);
    background: var(--danger-soft);
  }
  .problem--warn {
    color: var(--warn);
    background: var(--warn-soft);
  }
  .problem--neutral {
    color: var(--ink-2);
    background: var(--sunken);
  }
  .hint {
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    overflow-wrap: anywhere;
    color: var(--ink-3);
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
  }
  .path-actions {
    grid-column: 1 / -1;
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
  }
</style>
