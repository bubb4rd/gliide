// Setting a company's logo (the job header's mark and its panel): an image file dropped on either, chosen in the file
// dialog, or pasted. Drop targets carry `data-logo-drop="<company>"`. Tauri takes files dropped on the window and
// hands their paths to `onDragDropEvent` (the page never sees the drop), so the drag is tracked here while a screen
// with targets watches.

import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { appError, removeLogo, setLogo, type LogoSource } from "$lib/api";
import { app } from "$lib/stores/app.svelte";
import { toast } from "$lib/ui";

class Logo {
  /** The company whose drop target the dragged files are over. */
  over = $state<string | null>(null);
  /** A logo is being set or removed. */
  busy = $state(false);

  /** Sets `company`'s logo from `source`, everywhere it shows; false (and a toast saying why) when it couldn't. */
  set = async (company: string, source: LogoSource) => {
    if (this.busy) return false;
    this.busy = true;
    try {
      const row = await setLogo(company, source);
      app.putLogo(row.key, row);
      return true;
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
      return false;
    } finally {
      this.busy = false;
    }
  };

  choose = async (company: string) => {
    const path = await open({
      title: `Choose a photo for ${company}`,
      multiple: false,
      directory: false,
      filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg", "gif", "webp", "tif", "tiff", "bmp", "ico", "svg"] }],
    }).catch((e) => {
      toast(appError(e).message, { tone: "danger" });
      return null;
    });
    return path ? this.set(company, { kind: "file", path }) : false;
  };

  remove = async (company: string, key: string) => {
    if (this.busy) return;
    this.busy = true;
    try {
      await removeLogo(company);
      app.putLogo(key, null);
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
    } finally {
      this.busy = false;
    }
  };

  /**
   * Tracks files dragged over drop targets, and sets the logo of the one they're dropped on (then calls `onset`),
   * until the returned function is called (an `$effect` cleanup).
   */
  watch(onset: () => void): () => void {
    let stopped = false;
    let unlisten: (() => void) | undefined;
    void getCurrentWebview()
      .onDragDropEvent(async ({ payload }) => {
        if (payload.type === "leave") {
          this.over = null;
          return;
        }
        const { x, y } = payload.position.toLogical(window.devicePixelRatio);
        const target = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-logo-drop]");
        const company = target?.dataset.logoDrop ?? null;
        if (payload.type !== "drop") {
          this.over = company;
          return;
        }
        this.over = null;
        const path = payload.paths[0];
        if (company && path && (await this.set(company, { kind: "file", path }))) onset();
      })
      .then((stop) => {
        if (stopped) stop();
        else unlisten = stop;
      });
    return () => {
      stopped = true;
      unlisten?.();
      this.over = null;
    };
  }
}

export const logo = new Logo();
