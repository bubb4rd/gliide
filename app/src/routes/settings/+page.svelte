<!--
  Settings: one column of cards. Every control saves as you change it (fields on blur or ⏎); the theme applies at
  once. Claude Code's status is checked by running `claude`, never by reading its credentials.
-->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import RotateCw from "@lucide/svelte/icons/rotate-cw";
  import { appError, type Mode, type Settings, type Theme } from "$lib/api";
  import { app } from "$lib/stores/app.svelte";
  import { theme } from "$lib/stores/theme.svelte";
  import { Badge, Button, Card, Input, Menu, Segmented, Toolbar, toast } from "$lib/ui";

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

  type ModelKey = "extractModel" | "suggestModel";

  let checking = $state(false);
  /** Bumped when a save fails, so every control redraws from what's stored. */
  let resets = $state(0);
  /** Models whose menu says Custom… while the field is being filled. */
  let custom = $state<Record<ModelKey, boolean>>({ extractModel: false, suggestModel: false });
  let drafts = $state<Record<ModelKey | "claudePath", string>>({ extractModel: "", suggestModel: "", claudePath: "" });

  const s = $derived(app.settings);

  // The fields start from what's stored, and follow it after each save.
  $effect(() => {
    if (!s) return;
    drafts.extractModel = s.extractModel;
    drafts.suggestModel = s.suggestModel;
    drafts.claudePath = s.claudePath ?? "";
  });

  onMount(() => {
    if (!app.settings) app.loadSettings();
  });

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

  async function commitClaudePath() {
    const value = drafts.claudePath.trim();
    if (value === (s?.claudePath ?? "")) return;
    if (await save({ claudePath: value || null })) refreshAgent();
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
    // A folder holding ATS Fit's applications.csv has it imported, so the list may grow.
    if (await save({ exportDir: dir })) app.refreshApplications();
  }

  function onEnter(e: KeyboardEvent) {
    if (e.key === "Enter") (e.currentTarget as HTMLInputElement).blur();
  }

  const agent = $derived(app.agent);
  const agentBadge = $derived.by(() => {
    if (!agent) return { tone: "neutral", text: "Not checked" } as const;
    if (!agent.found) return { tone: "danger", text: "Not found" } as const;
    if (agent.signedIn === false) return { tone: "danger", text: "Signed out" } as const;
    if (agent.signedIn === null) return { tone: "neutral", text: "Sign-in unknown" } as const;
    return { tone: "gain", text: "Signed in" } as const;
  });
</script>

<Toolbar>
  {#snippet crumbs()}<b>Settings</b>{/snippet}
</Toolbar>

<div class="scroll">
  {#if s}
    {#key resets}
      <div class="column">
        <Card pad as="section" aria-labelledby="agent-title">
          <div class="card">
            <header class="head">
              <h2 class="name" id="agent-title">Claude Code</h2>
              <Badge tone={agentBadge.tone} dot>{agentBadge.text}</Badge>
              <Button size="sm" variant="ghost" icon={RotateCw} disabled={checking} onclick={refreshAgent}>
                {checking ? "Checking…" : "Refresh"}
              </Button>
            </header>
            <p class="lede">Reads the requirements and suggests the edits, on your subscription.</p>
            {#if agent?.problem}<p class="problem">{agent.problem}</p>{/if}
            <dl class="fields">
              <div class="field">
                <dt>Version</dt>
                <dd class="value num">{agent?.version ?? "—"}</dd>
              </div>
              <div class="field">
                <dt>Plan</dt>
                <dd class="value">{agent?.plan ?? "—"}</dd>
              </div>
              <div class="field field--stack">
                <dt><label for="claude-path">Path</label></dt>
                <dd>
                  <Input
                    id="claude-path"
                    size="md"
                    bind:value={drafts.claudePath}
                    placeholder="Automatic"
                    spellcheck="false"
                    autocomplete="off"
                    onblur={commitClaudePath}
                    onkeydown={onEnter}
                  />
                  <p class="hint">
                    {#if s.claudePath}
                      Leave empty to find <code>claude</code> on your PATH.
                    {:else if agent?.path}
                      Found <code>{agent.path}</code>; set a path to use another one.
                    {:else}
                      Found on your PATH when empty; set a path if it isn’t there.
                    {/if}
                  </p>
                </dd>
              </div>
            </dl>
          </div>
        </Card>

        <Card pad as="section" aria-labelledby="tailoring-title">
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

        <Card pad as="section" aria-labelledby="export-title">
          <div class="card">
            <h2 class="name" id="export-title">Export folder</h2>
            <p class="lede">Tailored PDFs and applications.csv go here.</p>
            <div class="folder">
              <span class="path" title={s.exportDir}>{s.exportDir}</span>
              <Button icon={FolderOpen} onclick={chooseFolder}>Choose…</Button>
            </div>
          </div>
        </Card>

        <Card pad as="section" aria-labelledby="appearance-title">
          <div class="card">
            <h2 class="name" id="appearance-title">Appearance</h2>
            <p class="lede">Light is the default; System follows macOS.</p>
            <dl class="fields">
              <div class="field">
                <dt>Theme</dt>
                <dd><Segmented options={THEMES} value={s.theme} label="Theme" onchange={setTheme} /></dd>
              </div>
            </dl>
          </div>
        </Card>
      </div>
    {/key}
  {/if}
</div>

<style>
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    overscroll-behavior: contain;
  }
  .column {
    display: grid;
    gap: var(--space-4);
    max-width: 640px;
    margin-inline: auto;
    padding: var(--space-8) var(--space-6) var(--space-12);
  }
  .card {
    display: grid;
    gap: var(--space-1);
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
  .head .name {
    flex: 1;
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
  .field--stack {
    display: grid;
    justify-content: stretch;
    gap: var(--space-2);
  }
  dt {
    flex: none;
    color: var(--ink-2);
  }
  dd {
    min-width: 0;
    margin: 0;
  }
  .value {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink);
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
</style>
