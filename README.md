<p align="center">
  <img src="app/src-tauri/icons/128x128@2x.png" alt="gliide logo" width="128">
</p>
<h1 align="center">
  gliide
</h1>

From a job posting to a tailored, one-page resume.

https://github.com/user-attachments/assets/f9a51564-1cff-4b48-8423-5c957fda79a1

- **Score the fit** of a posting, from a link or pasted text, against one of your master resumes.
- **Review honest edits** with a click each: rewrites, skills, and entries from your experience bank.
- **Export a PDF** in your design.
- **No API key.** Every model call goes through the agent CLI you already sign in to, on your own subscription.

Status: early development. macOS on Apple Silicon today; Windows and iOS are planned on the same Rust core
([roadmap](ARCHITECTURE.md#13-roadmap)).

## Install

Download `gliide_<version>_aarch64.dmg` from the [latest release](https://github.com/bubb4rd/gliide/releases/latest),
open it and drag gliide to Applications. The app is signed with a Developer ID and notarized by Apple, so it opens
without a Gatekeeper warning.

gliide needs one agent CLI installed and signed in with your own subscription:
[Claude Code](https://code.claude.com) (`claude`), [Codex](https://developers.openai.com/codex/cli) (`codex`, with a
ChatGPT plan) or [Antigravity](https://antigravity.google/docs/getting-started) (`agy`, with a Google AI plan). gliide
launches the one you choose for every model call and never asks for or stores an API key. First-run setup checks for
them.

gliide looks for a newer release at launch and once a day and says so in the sidebar; **Restart to update**
(Settings → About) installs it. Only a release signed with the project's update key installs.

## What leaves your machine

Your bank, masters, drafts and applications stay in the local SQLite store and your export folder. The app's own
network requests are these (links you open, such as a posting or Maps, go to your browser or Maps):

| Request | When | What is sent |
|---|---|---|
| Your agent CLI (`claude -p`, `codex exec` or `agy -p`) | Extract, Suggest, Rewrite, Instruct, import, cover letters | The prompt (posting text, the resume being tailored, your notes) goes to the model through the CLI you installed, under your own account with its maker |
| A posting's page or its board's API (Greenhouse, Lever, Ashby, Workday, SmartRecruiters, Workable, Recruitee…) | You paste a link; a feed job is read | The link |
| [freehire](https://freehire.me) public API and `logo.freehire.me` | Feed pulls and searches (when the feed is on); company logos | Your feed filters and search words; company names |
| Company websites | A feed company's logo freehire doesn't have | A request for the site's icon |
| Watchlist boards | Feed pulls, for the companies you add | The board's public job list request |
| LinkedIn's guest job search | **Only if you turn it on** (Settings → Feed; off by default). LinkedIn's User Agreement doesn't allow automated access; the choice and its consequences are yours | Master titles, countries, seniority, your search words |
| [Fantastic.jobs](https://fantastic.jobs) via [Apify](https://apify.com) | Only with your Apify token in Settings; once a day | Your token, master titles, countries, years and employment filters |
| OpenStreetMap [Nominatim](https://nominatim.org) | The review's map, once per place | The job's place name |
| [OpenFreeMap](https://openfreemap.org) tiles | The review's map | Tile requests for the map area |
| [GitHub](https://github.com/bubb4rd/gliide/releases) releases | At launch and once a day, unless Settings → About → Check for updates is off; Check now; Restart to update | A request for `latest.json` (and, when you update, the release's bundle). Nothing about you or your data |

No telemetry, analytics or crash reporting: the app sends nothing about you or your usage to the project. The webview
runs under a Content Security Policy (`app.security.csp` in `app/src-tauri/tauri.conf.json`): scripts only from the
app itself, and the only remote host it may reach is the map tiles'; everything else goes through the Rust side.

Map data © [OpenStreetMap](https://www.openstreetmap.org/copyright) contributors, available under the Open Database
License (ODbL); tiles by [OpenFreeMap](https://openfreemap.org) in the [OpenMapTiles](https://openmaptiles.org)
schema. The map shows this credit.

## Build from source

### Prerequisites

- macOS on Apple Silicon (the tested platform; no other platform is built yet).
- Rust stable, 1.92 or newer (`rust-version` in `Cargo.toml`), via [rustup](https://rustup.rs).
- Node.js 20.19+ or 22.12+ (`engines` in `app/package.json`) and pnpm 9 (`corepack enable` picks up
  `packageManager`).
- One agent CLI, signed in (see [Install](#install)).

### Build and run

```sh
cd app
pnpm install
pnpm tauri dev
```

`pnpm tauri dev` uses the `tauri` script in `app/package.json` (or install the CLI with
`cargo install tauri-cli --version "^2"`). App data lives in `~/Library/Application Support/dev.bubb4rd.gliide/`;
exports and `applications.csv` go to `~/Documents/Resumes` (Settings → Export folder).

A local release build: `pnpm tauri build` from `app/`. It's unsigned, which is fine on the machine that built it;
macOS warns on other Macs.

### Development

- Tests: `cargo test --workspace`, and `pnpm check` in `app/`. [`CONTRIBUTING.md`](CONTRIBUTING.md) lists everything
  CI runs.
- Design lint: `node design/lint.mjs`. Specimen: **Specimen** in the sidebar of a dev build (`/specimen`).
- After changing `app/src-tauri/src/dto.rs`, `cargo test -p gliide` regenerates the TypeScript bindings in
  `app/src/lib/api/bindings/`.
- App icon: `pnpm tauri icon src-tauri/app-icon.png` from `app/` (`icon-art.png` is the full-bleed art, kept for iOS).
- The `ez` command line runs the core and rendering without the app: [`crates/ez-cli/README.md`](crates/ez-cli/README.md).
- Releases: [`RELEASING.md`](RELEASING.md) (maintainers only).

## Documentation

| Document | What it covers |
|---|---|
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Stack, data model, model access through your own subscription (`claude -p`, `codex exec`, `agy -p`), scoring, change engine, rendering, roadmap |
| [`design/DESIGN.md`](design/DESIGN.md) | The "Paper & Ink" design system: tokens, components, motion, screen recipes |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | Setup, the checks a pull request must pass, DCO sign-off |
| [`AGENTS.md`](AGENTS.md) | The same rules, written for coding agents working in this repo |

## License

gliide is free software under the [GNU Affero General Public License v3.0 only](LICENSE) (`AGPL-3.0-only`).
Bundled fonts, glyphs and other third-party material keep their own licenses: see
[`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md). Security reports: [`SECURITY.md`](SECURITY.md).

**Trademarks.** The logos in `app/static/sources/` and `app/static/agents/` and any company logo the app shows are
trademarks of their owners, used only to identify their services. gliide is not affiliated with, sponsored or
endorsed by any of them.
