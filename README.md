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

## Prerequisites

- macOS on Apple Silicon (the tested platform; no other platform is built yet).
- Rust stable, 1.85 or newer (the workspace uses edition 2024), via [rustup](https://rustup.rs).
- Node.js 20.19+ or 22.12+ (Vite 8) and pnpm 9 (`corepack enable` picks up `packageManager` in `app/package.json`).
- An agent CLI installed and signed in with your own subscription, such as
  [Claude Code](https://code.claude.com) (`claude`). gliide launches it; it never asks for or stores an API key.

## Build and run

The app (macOS): `cd app && pnpm install && pnpm tauri dev` (needs `cargo install tauri-cli --version "^2"` or the
pnpm `tauri` script, and Claude Code signed in). Its data lives in `~/Library/Application Support/dev.bubb4rd.gliide/`;
exports and `applications.csv` go to `~/Documents/Resumes` (Settings → Export folder). Release build:
`pnpm tauri build` (unsigned); signed and notarized for sharing: `pnpm release:mac` from `app/` (needs a Developer ID
Application certificate in the keychain and an App Store Connect API key; see `app/scripts/release-macos.sh`). After
changing `app/src-tauri/src/dto.rs`, `cargo test -p gliide` regenerates the TypeScript
bindings in `app/src/lib/api/bindings/`. App icon: `pnpm tauri icon src-tauri/app-icon.png` from `app/` (the logo on
macOS's 1024 grid: an 824 rounded square with a soft shadow); `src-tauri/icon-art.png` is the full-bleed art, for
iOS's own mask later.

Design specimen: **Specimen** in the app's sidebar (dev builds; the `/specimen` route).
Design lint: `node design/lint.mjs`. Tests: `cargo test --workspace` and `pnpm check` in `app/`.

Core and rendering (`cargo run -p ez-cli --bin ez -- …`; outputs belong under the gitignored `local/`):

| Command | Does |
|---|---|
| `ez migrate <master.json> --out <dir>` | A v1 single-resume `master.json` → `bank.json` + `masters/swe.json` |
| `ez import <file> --out <dir> [--bank <f>] [--title "Graphic Design"] [--separate <id,…>] [--contact] [--model sonnet]` | A resume (`.pdf`, `.docx`, `.md`, `.txt`; or a v1 JSON such as an earlier run's `imported.json`, which skips the model) copied with `claude -p` and merged into the bank; writes `imported.json`, `bank.json`, `masters/<id>.json` and prints where each part went |
| `ez render --bank <f> --master <f> --out <dir> [--highlight] [--system-fonts] [--scale N] [--bench N]` | `resume.pdf`, `page-N.svg`, `page-N.png`, `metrics.json` |
| `ez schema --out schemas/` | Regenerates `schemas/bank.schema.json` and `schemas/master.schema.json` |
| `ez score <link\|file\|-\|text> --bank <f> --master <f> [--model sonnet] [--fresh] [--json <f>]` | Reads the posting, extracts its requirements with `claude -p` (cached in `local/cache/`), prints the composite score and every component |
| `ez rank <link\|file\|-\|text> --bank <f> --masters <dir>` | Scores every master in the folder against the posting, best first |
| `ez tailor <link\|file\|-\|text> --bank <f> --master <f> --out <dir> [--mode master-only\|bank-aware] [--draft <f>] [--model sonnet] [--extract-model sonnet] [--fresh]` | Extract and Suggest (both cached in `local/cache/`), the change set, the fit check; writes `tailored.json`, `resume.pdf`, highlighted `page-N.png`, `changes.json` (state, points, flags per change); prints now → projected (ceiling), the page status and every change |

## What leaves your machine

Your bank, masters, drafts and applications stay in the local SQLite store and your export folder. The app's own
network requests are these (links you open, such as a posting or Maps, go to your browser or Maps):

| Request | When | What is sent |
|---|---|---|
| Your agent CLI (e.g. `claude -p`) | Extract, Suggest, Rewrite, Instruct, import, cover letters | The prompt (posting text, the resume being tailored, your notes) goes to the model through the CLI you installed, under your own account with its maker |
| A posting's page or its board's API (Greenhouse, Lever, Ashby, Workday, SmartRecruiters, Workable, Recruitee…) | You paste a link; a feed job is read | The link |
| [freehire](https://freehire.me) public API and `logo.freehire.me` | Feed pulls and searches (when the feed is on); company logos | Your feed filters and search words; company names |
| Company websites | A feed company's logo freehire doesn't have | A request for the site's icon |
| Watchlist boards | Feed pulls, for the companies you add | The board's public job list request |
| LinkedIn's guest job search | **Only if you turn it on** (Settings → Feed; off by default). LinkedIn's User Agreement doesn't allow automated access; the choice and its consequences are yours | Master titles, countries, seniority, your search words |
| [Fantastic.jobs](https://fantastic.jobs) via [Apify](https://apify.com) | Only with your Apify token in Settings; once a day | Your token, master titles, countries, years and employment filters |
| OpenStreetMap [Nominatim](https://nominatim.org) | The review's map, once per place | The job's place name |
| [OpenFreeMap](https://openfreemap.org) tiles | The review's map | Tile requests for the map area |

No telemetry, analytics or crash reporting: the app sends nothing about you or your usage to the project. The webview
runs under a Content Security Policy (`app.security.csp` in `app/src-tauri/tauri.conf.json`): scripts only from the
app itself, and the only remote host it may reach is the map tiles'; everything else goes through the Rust side.

Map data © [OpenStreetMap](https://www.openstreetmap.org/copyright) contributors, available under the Open Database
License (ODbL); tiles by [OpenFreeMap](https://openfreemap.org) in the [OpenMapTiles](https://openmaptiles.org)
schema. The map shows this credit.

## License

gliide is free software under the [GNU Affero General Public License v3.0 only](LICENSE) (`AGPL-3.0-only`).
Bundled fonts, glyphs and other third-party material keep their own licenses: see
[`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md). Contributing: [`CONTRIBUTING.md`](CONTRIBUTING.md); security
reports: [`SECURITY.md`](SECURITY.md).

**Trademarks.** The logos in `app/static/sources/` and `app/static/agents/` and any company logo the app shows are
trademarks of their owners, used only to identify their services. gliide is not affiliated with, sponsored or
endorsed by any of them.
