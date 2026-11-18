// Spoken practice: an answer said out loud into a composer's draft (Practice, a story's Tell it). The microphone
// opens with Speak (⌘D) and what macOS hears on this Mac streams into the draft after what was already there; Speak
// again (or Send, or Esc) closes it and the last words land. The draft stays yours to fix before sending; what's sent
// carries how long you spoke and its words (`spoken`), which the turn keeps and shows under the answer.

import {
  appError,
  cancelListening,
  onSpeech,
  openSpeechSettings,
  speechStatus,
  startListening,
  stopListening,
  type SpeechStatus,
  type Spoken,
} from "$lib/api";
import { toast } from "$lib/ui";

/** `base` then `heard`, a space between when both have words. */
const joined = (base: string, heard: string) => [base, heard.trim()].filter(Boolean).join(" ");

/** Words a minute, at whole numbers; none under a few seconds, where it says nothing. */
export function pace(s: Spoken): number | null {
  return s.ms < 5000 || !s.words ? null : Math.round(s.words / (s.ms / 60000));
}

/** How long it took to say, as the turn's meta shows it: "1 min 42 s", "38 s". */
export function spokenTime(ms: number): string {
  const total = Math.round(ms / 1000);
  const m = Math.floor(total / 60);
  const s = total % 60;
  if (!m) return `${s} s`;
  return s ? `${m} min ${s} s` : `${m} min`;
}

/** The meta under a spoken answer: "Said in 1 min 42 s · 148 words a minute". */
export function spokenLine(s: Spoken): string {
  const wpm = pace(s);
  return wpm === null ? `Said in ${spokenTime(s.ms)}` : `Said in ${spokenTime(s.ms)} · ${wpm} words a minute`;
}

/** One composer's microphone: reads the draft and writes heard words into it. */
export class Speech {
  /** What macOS allows; `null` until read. */
  status = $state<SpeechStatus | null>(null);
  /** The microphone is open (`performance.now()` when it opened). */
  openedAt = $state<number | null>(null);
  /** Opening (macOS may be asking) or closing (the last words landing). */
  busy = $state(false);
  /** The draft's spoken part so far (several Speaks add up); `null` while nothing in it was said. */
  spoken = $state<Spoken | null>(null);

  #draft: { get: () => string; set: (text: string) => void };
  /** The draft as it was when the microphone opened: heard words go after it. */
  #base = "";
  #unlisten: (() => void) | null = null;
  /** The stop under way, so a send waits for its last words. */
  #stopping: Promise<void> | null = null;
  /** Cancelled while opening: what opens is closed again at once. */
  #dropped = false;

  constructor(draft: { get: () => string; set: (text: string) => void }) {
    this.#draft = draft;
  }

  get listening(): boolean {
    return this.openedAt !== null;
  }

  /** Why the microphone can't open here, for its tooltip; `null` while it can (or macOS will ask). */
  get off(): string | null {
    return this.status?.state === "unavailable" ? (this.status.reason ?? "Spoken practice isn’t available") : null;
  }

  async refresh(): Promise<SpeechStatus | null> {
    try {
      this.status = await speechStatus();
    } catch {
      this.status = null;
    }
    return this.status;
  }

  /** Speak: opens the microphone, else closes it. */
  toggle = (hints: string[] = []) => (this.listening ? this.stop() : this.start(hints));

  /** The microphone is opening (macOS may be asking): ⏎ waits for it to open or fail. */
  get opening(): boolean {
    return this.busy && this.#stopping === null;
  }

  async start(hints: string[]) {
    if (this.listening || this.busy) return;
    if (this.status?.state === "denied") return this.#denied(this.status);
    if (this.off) return;
    this.busy = true;
    this.#dropped = false;
    this.#base = this.#draft.get().trimEnd();
    const unlisten = await onSpeech((h) => {
      if (this.openedAt === null) return;
      this.#draft.set(joined(this.#base, h.text));
      if (h.stopped) void this.stop();
    });
    try {
      await startListening(hints);
      // Left (cancel) while macOS was asking: close what just opened.
      if (this.#dropped) {
        unlisten();
        void cancelListening();
        return;
      }
      this.#unlisten = unlisten;
      this.openedAt = performance.now();
    } catch (e) {
      unlisten();
      if (this.#dropped) return;
      const now = await this.refresh();
      if (now?.state === "denied") this.#denied(now);
      else toast(appError(e).message, { tone: "danger" });
    } finally {
      this.busy = false;
    }
  }

  /** Closes the microphone; the last words land in the draft. Resolves once they have (a stop already under way
   *  included), so a send right after it sends them. */
  stop(): Promise<void> {
    if (this.#stopping) return this.#stopping;
    if (!this.listening || this.busy) return Promise.resolve();
    this.busy = true;
    this.#unlisten?.();
    this.#unlisten = null;
    this.#stopping = this.#close().finally(() => {
      this.#stopping = null;
      this.openedAt = null;
      this.busy = false;
    });
    return this.#stopping;
  }

  async #close() {
    try {
      const heard = await stopListening();
      this.#draft.set(joined(this.#base, heard.text));
      if (!heard.text.trim()) {
        toast("Didn’t hear anything. Check the microphone in System Settings → Sound → Input.", { tone: "warn" });
      } else {
        const s = this.spoken;
        this.spoken = s ? { ms: s.ms + heard.spoken.ms, words: s.words + heard.spoken.words } : heard.spoken;
      }
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
    }
  }

  /** Closes the microphone and drops what it heard (leaving the screen), also while it's still opening. */
  cancel() {
    this.#unlisten?.();
    this.#unlisten = null;
    if (this.listening) void cancelListening();
    else if (this.opening) this.#dropped = true;
    this.openedAt = null;
  }

  /** The draft was sent or cleared: what comes next starts unspoken. */
  reset() {
    this.spoken = null;
  }

  #denied(s: SpeechStatus) {
    const pane = s.pane;
    toast(s.reason ?? "Spoken practice is turned off for gliide.", {
      tone: "warn",
      action: pane ? { label: "Open Settings", run: () => void openSpeechSettings(pane) } : undefined,
    });
  }
}
