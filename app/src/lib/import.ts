// Picking a resume to import: the file dialog, or a file dropped on a `data-resume-drop` well, then the import review
// for the chosen file (/bank/import).

import { goto } from "$app/navigation";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { appError } from "$lib/api";
import { toast } from "$lib/ui";

/** What import reads: PDF, Word, Markdown, text, or a gliide v1 JSON. */
const EXTENSIONS = ["pdf", "docx", "md", "markdown", "txt", "json"];
export const RESUME_FORMATS = "PDF, Word, Markdown or text";

/** Opens the import review for `path`, which goes to `back` once it's added or discarded (Experience without one). */
async function review(path: string, back?: string | null) {
  const query = new URLSearchParams({ file: path });
  if (back) query.set("back", back);
  await goto(`/bank/import?${query}`);
}

/** Asks for a resume and opens its import review. */
export async function chooseResume(defaultPath?: string, back?: string | null) {
  const path = await open({
    title: "Import a resume",
    multiple: false,
    directory: false,
    defaultPath,
    filters: [{ name: "Resume", extensions: EXTENSIONS }],
  }).catch((e) => {
    toast(appError(e).message, { tone: "danger" });
    return null;
  });
  if (path) await review(path, back);
}

/**
 * Tracks files dragged over a `data-resume-drop` well (`onover`) and opens the import review of the one dropped on it,
 * until the returned function is called (an `$effect` cleanup). Tauri takes drops on the window and hands their paths
 * to `onDragDropEvent`; the page never sees them.
 */
export function watchResumeDrop(back: string, onover: (over: boolean) => void): () => void {
  let stopped = false;
  let unlisten: (() => void) | undefined;
  void getCurrentWebview()
    .onDragDropEvent(({ payload }) => {
      if (payload.type === "leave") return onover(false);
      const { x, y } = payload.position.toLogical(window.devicePixelRatio);
      const on = !!document.elementFromPoint(x, y)?.closest("[data-resume-drop]");
      if (payload.type !== "drop") return onover(on);
      onover(false);
      const path = payload.paths[0];
      if (!on || !path) return;
      const extension = path.split(".").pop()?.toLowerCase() ?? "";
      if (EXTENSIONS.includes(extension)) void review(path, back);
      else toast(`gliide can’t read ${extension ? `.${extension} files` : "that file"}. Drop a ${RESUME_FORMATS} file.`, { tone: "danger" });
    })
    .then((stop) => {
      if (stopped) stop();
      else unlisten = stop;
    });
  return () => {
    stopped = true;
    unlisten?.();
    onover(false);
  };
}
