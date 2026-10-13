// Adding a posting from the clipboard: a link or the posting's text becomes a posting, and its review opens.
// Applications and the Overview bind it to V (and ⌘V).

import { goto } from "$app/navigation";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { addJob, appError } from "$lib/api";
import { app } from "$lib/stores/app.svelte";
import { toast } from "$lib/ui";

class Paste {
  /** The clipboard is being read and the posting added; the buttons say "Reading…". */
  reading = $state(false);

  run = async () => {
    if (this.reading) return;
    this.reading = true;
    try {
      const text = (await readText().catch(() => ""))?.trim();
      if (!text) {
        toast("The clipboard has no text. Copy a job link or the posting first.", { tone: "neutral" });
        return;
      }
      const job = await addJob(text);
      void app.refreshJobs();
      await goto(`/jobs/${job.id}`);
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
    } finally {
      this.reading = false;
    }
  };
}

export const paste = new Paste();
