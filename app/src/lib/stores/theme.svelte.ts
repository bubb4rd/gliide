// The theme: Light (default), Dark, or System (follows the OS, live). Applies `data-theme="dark"` on the root inside
// the theme-switch suppression (no transitions for two frames), matches the window's appearance so the native
// sidebar material and traffic lights agree, and remembers the choice for the pre-paint script in app.html.

import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Theme } from "$lib/api";

const STORAGE_KEY = "ez-theme";
const DARK = "(prefers-color-scheme: dark)";

/** Sets the root's theme with every transition suppressed, so colors don't all animate at once (better-ui). */
function paint(dark: boolean) {
  const root = document.documentElement;
  if ((root.dataset.theme === "dark") === dark) return;
  root.classList.add("theme-switching");
  if (dark) root.dataset.theme = "dark";
  else delete root.dataset.theme;
  void document.body.offsetHeight; // flush styles while transitions are off
  requestAnimationFrame(() => requestAnimationFrame(() => root.classList.remove("theme-switching")));
}

/** The native window's appearance; outside Tauri (a browser) there is none. */
async function matchWindow(theme: Theme) {
  if (!("__TAURI_INTERNALS__" in window)) return;
  await getCurrentWindow().setTheme(theme === "system" ? null : theme);
}

class ThemeStore {
  /** The chosen theme (Settings → Theme). */
  current = $state<Theme>("light");
  /** What shows now: `system` resolved against the OS. */
  dark = $state(false);

  #query: MediaQueryList | null = null;

  constructor() {
    if (typeof window === "undefined") return;
    this.#query = matchMedia(DARK);
    this.#query.addEventListener("change", () => {
      if (this.current === "system") this.#resolve();
    });
    // app.html painted the saved theme already; this matches the window to it until settings load.
    const saved = localStorage.getItem(STORAGE_KEY);
    this.preview(saved === "dark" || saved === "system" ? saved : "light");
  }

  /** Applies `theme` and remembers it. Settings calls this when the choice changes. */
  apply(theme: Theme) {
    localStorage.setItem(STORAGE_KEY, theme);
    this.preview(theme);
  }

  /** Shows `theme` without remembering it (the specimen's preview toggle). */
  preview(theme: Theme) {
    this.current = theme;
    this.#resolve();
    matchWindow(theme).catch((e) => console.error("theme: couldn't set the window appearance", e));
  }

  #resolve() {
    this.dark = this.current === "dark" || (this.current === "system" && !!this.#query?.matches);
    paint(this.dark);
  }
}

export const theme = new ThemeStore();
