// The review's folded sections, by id: `skills`, `lines`, `gaps`, `knockouts`, and `group:<id>` for a change group
// (its bank section's id, or `bank`). Remembered across jobs and launches: a section you fold stays folded until you
// open it again.

import { SvelteSet } from "svelte/reactivity";

const STORAGE_KEY = "ez-folded";

function load(): string[] {
  if (typeof localStorage === "undefined") return [];
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "[]");
    return Array.isArray(saved) ? saved.filter((id): id is string => typeof id === "string") : [];
  } catch {
    return [];
  }
}

class Folds {
  #ids = new SvelteSet<string>(load());

  has(id: string): boolean {
    return this.#ids.has(id);
  }

  /** Folds (`true`) or opens every id in `ids`. */
  set(ids: string[], folded: boolean) {
    for (const id of ids) {
      if (folded) this.#ids.add(id);
      else this.#ids.delete(id);
    }
    localStorage.setItem(STORAGE_KEY, JSON.stringify([...this.#ids]));
  }
}

export const folds = new Folds();
