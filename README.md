# easyshmeezy

From a job posting to a tailored, one-page resume: score the fit against one of your master resumes, review honest
edits (rewrites, skills, entries from your experience bank) with a click each, and export a PDF in your design.
macOS first; Windows and iOS from the same Rust core.

Status: early development; see the roadmap in `ARCHITECTURE.md` §13.

| Document | What it covers |
|---|---|
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Stack, data model, model access through your own subscription (ACP, `claude -p`), scoring, change engine, rendering, roadmap |
| [`design/DESIGN.md`](design/DESIGN.md) | The "Paper & Ink" design system: tokens, components, motion, screen recipes |
| [`AGENTS.md`](AGENTS.md) | Rules for coding agents working in this repo |

Design specimen: `cd design && python3 -m http.server`, then open `http://localhost:8000/specimen.html`.
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
