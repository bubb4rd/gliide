// The job feed as every screen shows it: its rows and status (Overview's New for you, /feed, the sidebar's count).
// Loaded by the layout and kept current by the `feed` event; a command that returns the feed hands it to `set`. A
// failed load keeps the last good value (null until the first answers) and goes to the console. The event also
// reloads the logos, since the app sends it when it stores a fetched one.

import type { UnlistenFn } from "@tauri-apps/api/event";
import { appError, feed as loadFeed, onFeed, type FeedView } from "$lib/api";
import { app } from "./app.svelte";

class FeedStore {
  /** Null until the first `feed()` answers, or while it fails. */
  view = $state<FeedView | null>(null);
  items = $derived(this.view?.items ?? []);
  status = $derived(this.view?.status ?? null);
  /** Checked at or over the threshold, best score first (the order `feed()` gives). */
  fits = $derived(this.items.filter((i) => i.state === "fit"));
  /** A refresh in flight, and whether another was asked for meanwhile: events come in bursts. */
  #loading: Promise<void> | null = null;
  #again = false;

  /** Loads the feed; calls during a load run once more after it, never in parallel. */
  async refresh(): Promise<void> {
    if (this.#loading) {
      this.#again = true;
      return this.#loading;
    }
    this.#loading = (async () => {
      do {
        this.#again = false;
        try {
          this.view = await loadFeed();
        } catch (e) {
          console.error(`feed: ${appError(e).message}`);
        }
      } while (this.#again);
      this.#loading = null;
    })();
    return this.#loading;
  }

  set(view: FeedView) {
    this.view = view;
  }

  /** Follows the `feed` event until the returned function runs: the status at once, then the rows and logos. */
  async listen(): Promise<UnlistenFn> {
    try {
      return await onFeed((status) => {
        if (this.view) this.view = { ...this.view, status };
        void this.refresh();
        void app.refreshLogos();
      });
    } catch (e) {
      console.error(`feed events: ${appError(e).message}`);
      return () => {};
    }
  }
}

export const feed = new FeedStore();
