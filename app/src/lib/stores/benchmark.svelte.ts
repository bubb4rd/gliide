// Settings → Benchmark's data: each role's leaderboard (gliide's reference, shipped with this release; this Mac's runs;
// your own calls) and the run on this Mac. The pane loads the view; the layout follows the `benchmark` event, which
// carries the run's status after every call and on every state change: merged at once (the rail's count), then the
// view read again once the pane has loaded it, as scores move while runs land. A failed load keeps the last good view
// and says why; a failed start, resume or stop says why in a toast.

import type { UnlistenFn } from "@tauri-apps/api/event";
import {
  appError,
  benchmark as loadBenchmark,
  benchmarkResume,
  benchmarkStart,
  benchmarkStop,
  onBenchmark,
  type BenchmarkView,
  type BenchPlan,
  type BenchRunStatus,
} from "$lib/api";
import { toast } from "$lib/ui/toast.svelte";

class BenchmarkStore {
  /** Null until the first `benchmark()` answers. */
  view = $state<BenchmarkView | null>(null);
  /** Why the last load failed; null once one answers. */
  failed = $state<string | null>(null);
  /** The current or last run on this Mac, as the latest event or load said; null: never run. */
  run = $state<BenchRunStatus | null>(null);
  /** When `run` last arrived (`Date.now()`), so its elapsed time can tick on between events. */
  runAt = $state(0);
  /** A run is going. */
  busy = $derived(this.run?.state === "running");
  /** A start, resume or stop in flight. */
  pending = $state(false);
  #loading: Promise<void> | null = null;
  #again = false;

  /** Loads the view; calls during a load run once more after it, never in parallel. */
  async refresh(): Promise<void> {
    if (this.#loading) {
      this.#again = true;
      return this.#loading;
    }
    this.#loading = (async () => {
      do {
        this.#again = false;
        try {
          const view = await loadBenchmark();
          this.view = view;
          this.#setRun(view.run);
          this.failed = null;
        } catch (e) {
          this.failed = appError(e).message;
          console.error(`benchmark: ${this.failed}`);
        }
      } while (this.#again);
      this.#loading = null;
    })();
    return this.#loading;
  }

  #setRun(run: BenchRunStatus | null) {
    this.run = run;
    this.runAt = Date.now();
  }

  /** Runs `call` (start, resume, stop) once at a time; its status replaces the run's. Resolves whether it worked. */
  async #act(call: () => Promise<BenchRunStatus | void>): Promise<boolean> {
    if (this.pending) return false;
    this.pending = true;
    try {
      const status = await call();
      if (status) this.#setRun(status);
      void this.refresh();
      return true;
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
      return false;
    } finally {
      this.pending = false;
    }
  }

  /** Starts a run of `plan`: this Mac's earlier runs of its roles on its routes are replaced. */
  start(plan: BenchPlan) {
    return this.#act(() => benchmarkStart(plan));
  }

  /** Picks a stopped run up where it stopped. */
  resume() {
    return this.#act(benchmarkResume);
  }

  /** Stops the run after aborting the call in flight; what's done is kept. */
  stop() {
    return this.#act(benchmarkStop);
  }

  /** Follows the `benchmark` event until the returned function runs. */
  async listen(): Promise<UnlistenFn> {
    try {
      return await onBenchmark((status) => {
        this.#setRun(status);
        if (this.view) void this.refresh();
      });
    } catch (e) {
      console.error(`benchmark events: ${appError(e).message}`);
      return () => {};
    }
  }
}

export const benchmark = new BenchmarkStore();
