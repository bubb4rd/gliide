// Picking a resume to import: the file dialog, then the import review for the chosen file (/bank/import).

import { goto } from "$app/navigation";
import { open } from "@tauri-apps/plugin-dialog";
import { appError } from "$lib/api";
import { toast } from "$lib/ui";

/** Asks for a resume (PDF, Word, Markdown, text, or ATS Fit's master.json) and opens its import review. */
export async function chooseResume(defaultPath?: string) {
  const path = await open({
    title: "Import a resume",
    multiple: false,
    directory: false,
    defaultPath,
    filters: [{ name: "Resume", extensions: ["pdf", "docx", "md", "markdown", "txt", "json"] }],
  }).catch((e) => {
    toast(appError(e).message, { tone: "danger" });
    return null;
  });
  if (path) await goto(`/bank/import?file=${encodeURIComponent(path)}`);
}
