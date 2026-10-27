// What the shell and every screen share: jobs (the sidebar's counts and Recent), applications, company logos, the
// library (bank and masters counts), settings, the agent's status, the models its aliases last ran, the model calls
// running and updates. Each refresh keeps the last good value when a command fails (lists start empty, the agent
// unknown), so the window always draws; the failure goes to the console.

import {
  agentStatus,
  appError,
  applications,
  checkUpdate,
  deleteJob,
  getSettings,
  installUpdate,
  jobs,
  library,
  logos,
  logoUrl,
  models as servedModels,
  onProgress,
  onUpdate,
  restoreRounds,
  running,
  setSettings,
  setStatus as saveStatus,
  updateStatus,
  type AgentStatus,
  type AppStatus,
  type ApplicationRow,
  type JobRow,
  type Progress,
  type Library,
  type LogoRow,
  type Settings,
  type UpdateStatus,
} from "$lib/api";
import { modelName } from "$lib/models";
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
  /** Each `claude --model` alias or name asked for → the model id that last answered it; empty until one has. */
  models = $state<Record<string, string>>({});
  /** The model calls running, oldest first (`listen` keeps it current). */
  calls = $state<Progress[]>([]);
  /** Updates: this build's version, a newer release once a check found one, a check or install running. */
  update = $state<UpdateStatus | null>(null);
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
   * Sets the applications' status at once, here and on their postings (the sidebar's Recent), and saves them in one
   * call (rewriting applications.csv); puts each back, with the error in a toast, if the save fails. Moving to Offer or
   * Rejected cancels their upcoming rounds; a toast says so, with Undo. Resolves whether it saved.
   */
  async setStatus(rows: ApplicationRow[], status: AppStatus): Promise<boolean> {
    if (!rows.length) return true;
    const before = new Map(rows.map((r) => [r.id, r.status]));
    const byJob = new Map(rows.flatMap((r) => (r.job ? [[r.job, r.id] as const] : [])));
    const put = (of: (id: string) => AppStatus) => {
      this.applications = this.applications.map((a) => (before.has(a.id) ? { ...a, status: of(a.id) } : a));
      this.jobs = this.jobs.map((j) => {
        const id = byJob.get(j.id);
        return id ? { ...j, status: of(id) } : j;
      });
    };
    put(() => status);
    try {
      const set = await saveStatus([...before.keys()], status);
      this.#putApplications(set.applications);
      if (set.cancelled.length) {
        const n = set.cancelled.length;
        toast(n === 1 ? "Cancelled the upcoming round" : `Cancelled ${n} upcoming rounds`, {
          action: {
            label: "Undo",
            run: async () => {
              const rows = await attempt("rounds", () => restoreRounds(set.cancelled));
              if (rows) this.#putApplications(rows);
            },
          },
        });
      }
      return true;
    } catch (e) {
      put((id) => before.get(id) ?? status);
      toast(appError(e).message, { tone: "danger" });
      return false;
    }
  }

  /** Puts these applications, as saved, in place of the ones with their ids. */
  putApplications(rows: ApplicationRow[]) {
    this.#putApplications(rows);
    const status = new Map(rows.flatMap((r) => (r.job ? [[r.job, r.status] as const] : [])));
    this.jobs = this.jobs.map((j) => (status.has(j.id) ? { ...j, status: status.get(j.id) ?? j.status } : j));
  }

  #putApplications(rows: ApplicationRow[]) {
    const saved = new Map(rows.map((a) => [a.id, a]));
    this.applications = this.applications.map((a) => saved.get(a.id) ?? a);
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

  /** Looks for a newer release now (Settings → About's Check now). */
  async checkUpdate() {
    const status = await attempt("check update", checkUpdate);
    if (status) this.update = status;
  }

  /** Installs the newer release and relaunches; a failure stays in `update.error` and a toast. */
  async installUpdate() {
    try {
      await installUpdate();
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
    }
  }

  /** After a model call: what each alias ran, for the names the chat shows. */
  async refreshModels() {
    const served = await attempt("models", servedModels);
    if (served) this.models = served;
  }

  /** The model `asked` (an alias or name) runs, by name: "Sonnet 5.5" once a call said which, "Sonnet" before. */
  modelName(asked: string) {
    return modelName(this.models[asked] ?? asked);
  }

  /** Keeps `calls` and `update` current until the returned function runs. */
  async listen(): Promise<() => void> {
    let stops: (() => void)[];
    try {
      stops = await Promise.all([
        onProgress((calls) => (this.calls = calls)),
        onUpdate((status) => (this.update = status)),
      ]);
    } catch (e) {
      console.error(`events: ${appError(e).message}`);
      return () => {};
    }
    const [now, update] = await Promise.all([attempt("running", running), attempt("update status", updateStatus)]);
    if (now) this.calls = now;
    if (update) this.update = update;
    return () => stops.forEach((stop) => stop());
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
    void Promise.all([
      this.refreshJobs(),
      this.refreshApplications(),
      this.refreshLogos(),
      this.refreshAgent(),
      this.refreshModels(),
    ]);
    await Promise.all([this.loadSettings(), this.refreshLibrary()]);
  }
}

export const app = new AppStore();
