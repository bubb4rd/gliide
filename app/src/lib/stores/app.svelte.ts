// What the shell and every screen share: jobs (the sidebar's counts and Recent), applications, the library (bank and
// masters counts), settings and the agent's status. Each refresh keeps the last good value when a command fails
// (lists start empty, the agent unknown), so the window always draws; the failure goes to the console.

import {
  agentStatus,
  appError,
  applications,
  getSettings,
  jobs,
  library,
  setSettings,
  type AgentStatus,
  type ApplicationRow,
  type JobRow,
  type Library,
  type Settings,
} from "$lib/api";
import { theme } from "./theme.svelte";

async function attempt<T>(what: string, call: () => Promise<T>): Promise<T | undefined> {
  try {
    return await call();
  } catch (e) {
    console.error(`${what}: ${appError(e).message}`);
    return undefined;
  }
}

class AppStore {
  /** Postings, newest first. */
  jobs = $state<JobRow[]>([]);
  /** The first `jobs()` answered: until then an empty list means "not known yet", not "no postings". */
  jobsLoaded = $state(false);
  applications = $state<ApplicationRow[]>([]);
  /** Null until the first load, or when it failed. */
  library = $state<Library | null>(null);
  settings = $state<Settings | null>(null);
  /** Null while unknown (not checked yet, or the check failed). */
  agent = $state<AgentStatus | null>(null);

  async refreshJobs() {
    const rows = await attempt("jobs", jobs);
    if (!rows) return;
    this.jobs = rows;
    this.jobsLoaded = true;
  }

  async refreshApplications() {
    const rows = await attempt("applications", applications);
    if (rows) this.applications = rows;
  }

  async refreshLibrary() {
    const lib = await attempt("library", library);
    if (lib) this.library = lib;
  }

  async refreshAgent() {
    const status = await attempt("agent status", agentStatus);
    if (status) this.agent = status;
  }

  async loadSettings() {
    const settings = await attempt("settings", getSettings);
    if (!settings) return;
    this.settings = settings;
    theme.apply(settings.theme);
  }

  /** Saves settings and applies the theme; rejects with an `AppError` so the Settings screen can say why. */
  async saveSettings(settings: Settings) {
    this.settings = await setSettings(settings);
    theme.apply(this.settings.theme);
  }

  /** Everything the shell shows, in parallel. */
  async load() {
    await Promise.all([
      this.loadSettings(),
      this.refreshJobs(),
      this.refreshApplications(),
      this.refreshLibrary(),
      this.refreshAgent(),
    ]);
  }
}

export const app = new AppStore();
