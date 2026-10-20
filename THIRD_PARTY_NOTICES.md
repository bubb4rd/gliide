# Third-party notices

gliide itself is licensed under the GNU Affero General Public License v3.0 only (`AGPL-3.0-only`, see
[`LICENSE`](LICENSE)). The material below is bundled with or used by it under its own license.

## Fonts

| Font | Files | License | License text |
|---|---|---|---|
| Inter (The Inter Project Authors) | `design/fonts/Inter-Variable.woff2`, `design/fonts/Inter-Variable-Italic.woff2` | SIL Open Font License 1.1 | [`design/fonts/Inter-OFL.txt`](design/fonts/Inter-OFL.txt) |
| Source Serif 4 (Adobe; Reserved Font Name "Source") | `design/fonts/SourceSerif4Variable-Roman.ttf.woff2`, `design/fonts/SourceSerif4Variable-Italic.ttf.woff2` | SIL Open Font License 1.1 | [`design/fonts/SourceSerif4-OFL.md`](design/fonts/SourceSerif4-OFL.md) |
| Liberation Serif (Red Hat, Google; Reserved Font Names "Liberation", "Arimo", "Tinos", "Cousine") | `templates/fonts/LiberationSerif-{Regular,Bold,Italic,BoldItalic}.ttf` (embedded in `resume-render`) | SIL Open Font License 1.1 | [`templates/fonts/LICENSE-LiberationSerif`](templates/fonts/LICENSE-LiberationSerif) |

## Map glyphs

`app/static/glyphs/inter-400/` and `app/static/glyphs/inter-500/` are signed-distance-field glyph ranges generated
from the bundled Inter (see [`app/static/glyphs/README.md`](app/static/glyphs/README.md)). They are derived from the
font and distributed under the same SIL Open Font License 1.1 ([`design/fonts/Inter-OFL.txt`](design/fonts/Inter-OFL.txt)).

## Map data and tiles

The review's map draws vector tiles from [OpenFreeMap](https://openfreemap.org) in the
[OpenMapTiles](https://openmaptiles.org) schema, and geocodes place names with OpenStreetMap's
[Nominatim](https://nominatim.org). Map data © [OpenStreetMap](https://www.openstreetmap.org/copyright)
contributors, available under the Open Database License (ODbL 1.0). The map shows this attribution on screen
(`app/src/lib/ui/placemap.ts`, `CREDIT`).

## Brand logos

The icons in `app/static/sources/` (freehire, LinkedIn, Greenhouse, Lever, Ashby, Workday, SmartRecruiters) and
`app/static/agents/` (Claude, OpenAI, Gemini, Antigravity), and any company logo the app fetches or you set, are
trademarks of their respective owners. They are used only to identify the services and companies they belong to and
are not covered by gliide's license. gliide is not affiliated with, sponsored or endorsed by any of these owners.
Where each icon came from is listed in each folder's `README.md`.

## Agent skill

`.agents/skills/better-ui/` (also reached through `.claude/skills/better-ui`) is from
[jakubkrehel/skills](https://github.com/jakubkrehel/skills), © 2026 Jakub Krehel, under the MIT License
([`.agents/skills/better-ui/LICENSE`](.agents/skills/better-ui/LICENSE)). It guides contributors' coding agents and is
not part of the app.

## Rust dependencies

Computed with `cargo metadata --format-version 1 --locked` on 2026-10-20: 791 third-party packages in the resolved
graph (all targets and platforms, build and dev dependencies included). License expressions as each crate declares
them:

| Packages | License expression |
|---:|---|
| 345 | MIT OR Apache-2.0 |
| 180 | MIT |
| 80 | Apache-2.0 OR MIT |
| 31 | MIT/Apache-2.0 |
| 27 | Unicode-3.0 |
| 25 | Apache-2.0 |
| 19 | Zlib OR Apache-2.0 OR MIT |
| 11 | Unlicense OR MIT |
| 6 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| 5 | BSD-3-Clause |
| 5 | MPL-2.0 |
| 5 | ISC |
| 4 | Apache-2.0/MIT |
| 4 | Unlicense/MIT |
| 4 | Zlib |
| 4 | MIT OR Apache-2.0 OR Zlib |
| 3 | BSD-2-Clause |
| 3 | Apache-2.0 OR ISC OR MIT |
| 3 | BSD-3-Clause OR Apache-2.0 |
| 2 | Apache-2.0 WITH LLVM-exception |
| 2 | BSL-1.0 |
| 2 | MIT OR Zlib OR Apache-2.0 |
| 2 | BSD-3-Clause OR MIT OR Apache-2.0 |
| 2 | MIT OR Apache-2.0 OR LGPL-2.1-or-later |
| 2 | BSD-2-Clause OR Apache-2.0 OR MIT |
| 1 each | 0BSD OR MIT OR Apache-2.0; ISC AND (Apache-2.0 OR ISC); ISC AND (Apache-2.0 OR ISC) AND Apache-2.0 AND MIT AND BSD-3-Clause AND (Apache-2.0 OR ISC OR MIT) AND (Apache-2.0 OR ISC OR MIT-0); MIT-0; BSD-3-Clause AND MIT; BSD-3-Clause/MIT; Apache-2.0 AND MIT; CC0-1.0 OR MIT-0 OR Apache-2.0; (Apache-2.0 OR MIT) AND BSD-3-Clause; Apache-2.0 / MIT; Apache-2.0 AND ISC; 0BSD OR CC0-1.0; Apache-2.0 OR BSL-1.0; (MIT OR Apache-2.0) AND Unicode-3.0; CDLA-Permissive-2.0 |

Not purely permissive:

| Package | License | Note |
|---|---|---|
| `cssparser` 0.37.0, `cssparser-macros` 0.7.1, `dtoa-short` 0.3.5, `selectors` 0.38.0 | MPL-2.0 | Through `scraper` (posting HTML parsing) and Tauri (`dom_query`, `wry`). File-level copyleft, compatible with AGPL-3.0; used unmodified |
| `option-ext` 0.2.0 | MPL-2.0 | Through `dirs` (via `typst-kit`); used unmodified |
| `r-efi` 5.3.0, 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | Through `getrandom`; used under MIT or Apache-2.0; compiled only for UEFI targets |

Every crate declares a license; none is unknown, and none is GPL, AGPL or LGPL-only.

## JavaScript dependencies

Computed with `pnpm licenses list --prod --json` in `app/` on 2026-10-20: 49 production packages.

| Packages | License |
|---:|---|
| 28 | MIT |
| 9 | ISC |
| 3 | BSD-3-Clause (`maplibre-gl`, `pbf`, `@mapbox/vector-tile`) |
| 3 | MIT OR Apache-2.0 (`@tauri-apps/plugin-clipboard-manager`, `-dialog`, `-opener`) |
| 2 | BSD-2-Clause (`@mapbox/tiny-sdf`, `@mapbox/unitbezier`) |
| 2 | Apache-2.0 (`aria-query`, `axobject-query`) |
| 1 | (MIT OR Apache-2.0) (`@maplibre/mlt`) |
| 1 | Apache-2.0 OR MIT (`@tauri-apps/api`) |

None is copyleft or unknown. The build tools (Svelte, SvelteKit, Vite, TypeScript, the Tauri CLI) are MIT or
Apache-2.0; Svelte's and SvelteKit's runtime (MIT) is compiled into the app's front end.
