// Picking a resume to import: the file dialog, then the import review for the chosen file (/bank/import).

import { goto } from "$app/navigation";
import { open } from "@tauri-apps/plugin-dialog";
import { appError } from "$lib/api";
import { toast } from "$lib/ui";

/**
 * Asks for a resume (PDF, Word, Markdown, text, or a gliide v1 JSON) and opens its import review, which goes to
 * `back` once it's added or discarded (Experience without one).
 */
export async function chooseResume(defaultPath?: string, back?: string | null) {
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
  if (!path) return;
  const query = new URLSearchParams({ file: path });
  if (back) query.set("back", back);
  await goto(`/bank/import?${query}`);
}
