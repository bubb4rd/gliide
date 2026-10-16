// What the shell and every screen share: jobs (the sidebar's counts and Recent), applications, company logos, the
// library (bank and masters counts), settings and the agent's status. Each refresh keeps the last good value when a
// command fails (lists start empty, the agent unknown), so the window always draws; the failure goes to the console.

import {
  agentStatus,
  appError,
  applications,
  deleteJob,
  getSettings,
  jobs,
  library,
  logos,
  logoUrl,
  setSettings,
  setStatus as saveStatus,
  type AgentStatus,
  type AppStatus,
  type ApplicationRow,
  type JobRow,
  type Library,
  type LogoRow,
  type Settings,
} from "$lib/api";
import { LIFETIME_WITH_ACTION_MS, toast } from "$lib/ui/toast.svelte";
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
  /** Postings without an application yet ("Not sent" in Applications), newest first. */
  unsent = $derived(this.jobs.filter((j) => j.status === null));
  /** Each company's logo URL by company key (`companyKey` on jobs and applications); none without a logo. */
  logos = $state<Record<string, string>>({});
  /** Null until the first load, or when it failed. */
  library = $state<Library | null>(null);
  settings = $state<Settings | null>(null);
  /** Null while unknown (not checked yet, or the check failed). */
  agent = $state<AgentStatus | null>(null);
  /** Postings deleted while their Undo is still offered: out of every list, still in the store. */
  #deleting = new Set<string>();

  async refreshJobs() {
    const rows = await attempt("jobs", jobs);
    if (!rows) return;
    this.jobs = rows.filter((r) => !this.#deleting.has(r.id));
    this.jobsLoaded = true;
  }

  /**
   * Deletes a posting with Undo: it leaves every list at once and is deleted for good (its reviews and letter too;
   * its application stays, without the link) when the toast's Undo runs out. Quitting before then keeps it.
   */
  deleteJob(row: JobRow) {
    if (this.#deleting.has(row.id)) return;
    this.#deleting.add(row.id);
    this.jobs = this.jobs.filter((j) => j.id !== row.id);
    const timer = setTimeout(() => void this.#purge(row.id), LIFETIME_WITH_ACTION_MS);
    toast(`Deleted ${[row.company, row.role].filter(Boolean).join(" · ")}`, {
      action: {
        label: "Undo",
        run: () => {
          clearTimeout(timer);
          this.#deleting.delete(row.id);
          void this.refreshJobs();
        },
      },
    });
  }

  async #purge(id: string) {
    try {
      const rows = await deleteJob(id);
      this.#deleting.delete(id);
      this.jobs = rows.filter((r) => !this.#deleting.has(r.id));
      void this.refreshApplications();
    } catch (e) {
      this.#deleting.delete(id);
      toast(`Couldn’t delete the posting: ${appError(e).message}`, { tone: "danger" });
      void this.refreshJobs();
    }
  }

  async refreshApplications() {
    const rows = await attempt("applications", applications);
    if (rows) this.applications = rows;
  }

  /**
   * Sets an application's status at once, here and on its posting (the sidebar's Recent), and saves it (rewriting
   * applications.csv); puts it back, with the error in a toast, if the save fails.
   */
  async setStatus(row: ApplicationRow, status: AppStatus) {
    const put = (s: AppStatus) => {
      this.applications = this.applications.map((a) => (a.id === row.id ? { ...a, status: s } : a));
      if (row.job) this.jobs = this.jobs.map((j) => (j.id === row.job ? { ...j, status: s } : j));
    };
    const before = row.status;
    put(status);
    try {
      const saved = await saveStatus(row.id, status);
      this.applications = this.applications.map((a) => (a.id === saved.id ? saved : a));
    } catch (e) {
      put(before);
      toast(appError(e).message, { tone: "danger" });
    }
  }

  async refreshLogos() {
    const rows = await attempt("logos", logos);
    if (rows) this.logos = Object.fromEntries(rows.map((row) => [row.key, logoUrl(row)]));
  }

  /** Shows the logo just set (or removed, with `null`) everywhere its company appears. */
  putLogo(key: string, row: LogoRow | null) {
    const { [key]: _, ...rest } = this.logos;
    this.logos = row ? { ...rest, [key]: logoUrl(row) } : rest;
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

  /**
   * The first-run setup is due: settings and the library are in, the setup was never done or skipped, and there's no
   * bank yet (someone with a bank is already using the app, whatever their settings say).
   */
  setupDue = $derived(
    this.settings !== null && !this.settings.onboarded && this.library !== null && this.library.bank === null,
  );

  /** Ends the first-run setup (done or skipped) for good. */
  async finishSetup() {
    if (!this.settings || this.settings.onboarded) return;
    try {
      await this.saveSettings({ ...this.settings, onboarded: true });
    } catch (e) {
      toast(`Couldn’t save that setup is done: ${appError(e).message}`, { tone: "danger" });
    }
  }

  /**
   * Everything the shell shows, in parallel. Resolves once settings and the library are in, which is all the window
   * needs to pick its first screen; the rest goes on loading.
   */
  async load() {
    void Promise.all([this.refreshJobs(), this.refreshApplications(), this.refreshLogos(), this.refreshAgent()]);
    await Promise.all([this.loadSettings(), this.refreshLibrary()]);
  }
}

export const app = new AppStore();
