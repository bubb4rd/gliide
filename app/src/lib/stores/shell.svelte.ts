// Window chrome state: under 960px the sidebar collapses into a sheet (DESIGN.md Layout, Compact); Settings opens as
// a modal sheet over whatever screen is showing.

const COMPACT = "(max-width: 959px)";

/** Settings' panes, in the rail's order. */
export type SettingsPane =
  | "agents"
  | "usage"
  | "feed"
  | "sources"
  | "notifications"
  | "export"
  | "appearance"
  | "about";

class Shell {
  /** The window is narrower than 960px: the sidebar is a sheet. */
  compact = $state(false);
  /** The sidebar sheet is showing (compact only). */
  sidebarOpen = $state(false);
  /** No sidebar at all: the first-run setup, and the import review it opens. The layout sets it per screen. */
  bare = $state(false);
  /** Settings is open on this pane, or closed. */
  settings = $state<SettingsPane | null>(null);
  /** The pane Settings opens on when none is asked for: the last one shown. */
  #lastPane: SettingsPane = "agents";

  constructor() {
    if (typeof window === "undefined") return;
    const query = matchMedia(COMPACT);
    this.compact = query.matches;
    query.addEventListener("change", (e) => {
      this.compact = e.matches;
      this.sidebarOpen = false;
    });
  }

  /** Opens Settings on `pane`, or where it was last left; shows `pane` when it's already open. */
  openSettings(pane?: SettingsPane) {
    this.sidebarOpen = false;
    this.settings = pane ?? this.#lastPane;
    this.#lastPane = this.settings;
  }

  closeSettings() {
    this.settings = null;
  }
}

export const shell = new Shell();
