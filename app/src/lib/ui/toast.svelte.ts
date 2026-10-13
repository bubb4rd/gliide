// Toasts: short confirmations and errors, said once ("Saved globex_jr_sde_doe_resume.pdf"). The layout renders
// them with <Toaster />; anything can call `toast()`.

export type ToastTone = "neutral" | "gain" | "warn" | "danger";

export interface ToastAction {
  label: string;
  run: () => unknown;
}

export interface Toast {
  id: number;
  message: string;
  tone: ToastTone;
  action?: ToastAction;
}

/** How long a toast stays: long enough to read and reach its action. */
const LIFETIME_MS = 5000;
const LIFETIME_WITH_ACTION_MS = 8000;

let next = 0;
export const toasts: Toast[] = $state([]);

export function dismiss(id: number) {
  const i = toasts.findIndex((t) => t.id === id);
  if (i >= 0) toasts.splice(i, 1);
}

/** Shows `message`; returns its id for `dismiss`. Errors (`danger`) stay until dismissed. */
export function toast(message: string, options: { tone?: ToastTone; action?: ToastAction } = {}): number {
  const id = next++;
  const tone = options.tone ?? "gain";
  toasts.push({ id, message, tone, action: options.action });
  if (tone !== "danger") setTimeout(() => dismiss(id), options.action ? LIFETIME_WITH_ACTION_MS : LIFETIME_MS);
  return id;
}
