// Window chrome state: under 960px the sidebar collapses into a sheet (DESIGN.md Layout, Compact).

const COMPACT = "(max-width: 959px)";

class Shell {
  /** The window is narrower than 960px: the sidebar is a sheet. */
  compact = $state(false);
  /** The sidebar sheet is showing (compact only). */
  sidebarOpen = $state(false);
  /** No sidebar at all: the first-run setup, and the import review it opens. The layout sets it per screen. */
  bare = $state(false);

  constructor() {
    if (typeof window === "undefined") return;
    const query = matchMedia(COMPACT);
    this.compact = query.matches;
    query.addEventListener("change", (e) => {
      this.compact = e.matches;
      this.sidebarOpen = false;
    });
  }
}

export const shell = new Shell();
