# gliide

From a job posting to a tailored, one-page resume: score the fit against one of your master resumes, review honest
edits (rewrites, skills, entries from your experience bank) with a click each, and export a PDF in your design.
macOS first; Windows and iOS from the same Rust core.

Status: early development; see the roadmap in `ARCHITECTURE.md` §13.

| Document | What it covers |
|---|---|
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Stack, data model, model access through your own subscription (ACP, `claude -p`), scoring, change engine, rendering, roadmap |
| [`design/DESIGN.md`](design/DESIGN.md) | The "Paper & Ink" design system: tokens, components, motion, screen recipes |
| [`AGENTS.md`](AGENTS.md) | Rules for coding agents working in this repo |

The app (macOS): `cd app && pnpm install && pnpm tauri dev` (needs `cargo install tauri-cli --version "^2"` or the
pnpm `tauri` script, and Claude Code signed in). Its data lives in `~/Library/Application Support/dev.bubb4rd.gliide/`;
exports and `applications.csv` go to `~/Documents/Resumes` (Settings → Export folder). Release build:
`pnpm tauri build`. After changing `app/src-tauri/src/dto.rs`, `cargo test -p gliide` regenerates the TypeScript
bindings in `app/src/lib/api/bindings/`. App icon: `pnpm tauri icon src-tauri/app-icon.png` from `app/` (the logo on
macOS's 1024 grid: an 824 rounded square with a soft shadow); `src-tauri/icon-art.png` is the full-bleed art, for
iOS's own mask later.

Design specimen: **Specimen** in the app's sidebar (dev builds; the `/specimen` route).
Design lint: `node design/lint.mjs`.

Core and rendering (`cargo run -p ez-cli --bin ez -- …`; outputs belong under the gitignored `local/`):

| Command | Does |
|---|---|
| `ez migrate <master.json> --out <dir>` | ATS Fit's `master.json` → `bank.json` + `masters/swe.json` |
| `ez render --bank <f> --master <f> --out <dir> [--highlight] [--system-fonts] [--scale N] [--bench N]` | `resume.pdf`, `page-N.svg`, `page-N.png`, `metrics.json` |
| `ez schema --out schemas/` | Regenerates `schemas/bank.schema.json` and `schemas/master.schema.json` |
| `ez score <link\|file\|-\|text> --bank <f> --master <f> [--model sonnet] [--fresh] [--json <f>]` | Reads the posting, extracts its requirements with `claude -p` (cached in `local/cache/`), prints the composite score and every component |
| `ez rank <link\|file\|-\|text> --bank <f> --masters <dir>` | Scores every master in the folder against the posting, best first |
| `ez tailor <link\|file\|-\|text> --bank <f> --master <f> --out <dir> [--mode master-only\|bank-aware] [--draft <f>] [--model sonnet] [--extract-model sonnet] [--fresh]` | Extract and Suggest (both cached in `local/cache/`), the change set, the fit check; writes `tailored.json`, `resume.pdf`, highlighted `page-N.png`, `changes.json` (state, points, flags per change); prints now → projected (ceiling), the page status and every change |
