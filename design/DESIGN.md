# Paper & Ink: the gliide design system

Binding for every screen, page and component. When this document and a screen disagree, the screen is wrong.

| File | Role |
|---|---|
| `design/tokens.css` | The only place raw colors, shadows, font stacks and easing curves live. Light and dark |
| `app/src/lib/ui/` | The primitives as Svelte 5 components, each owning its styles; screens import them from `$lib/ui`. `app/src/app.css` imports the tokens and holds the base and the global utilities (`.num`, `.label`, `.title`, `.headline`, `.meta`, `.enter`, `.turn`) |
| `app/src/routes/specimen/` | The specimen: every component in every state, plus the composed Review screen, with a Light/Dark/System preview. `cd app && pnpm tauri dev`, then **Specimen** in the sidebar (dev builds), or `/specimen?theme=dark` |
| `design/lint.mjs` | Enforces the tokens and the better-ui rules on `design/` and `app/src/`. `node design/lint.mjs` must exit 0 |
| `design/fonts/` | Inter (UI, Fontsource latin subset) and Source Serif 4 (display, Adobe's official 4.005 release, unmodified: its OFL reserves the name "Source", so never subset or rename it), variable, with their OFL licenses; bundled, never loaded from a CDN |
| `.agents/skills/better-ui/` | The polish rules (radius, shadows, motion, icons) with exact values; this system adopts them unchanged |

## Direction

**Warm paper, graphite ink, one cobalt accent.** A serif for what you read at a glance (the job title, the score);
Inter for everything you operate. The resume page is the only object with real depth: it sits on a darker desk like a
sheet on a table, because it's the thing you're making. Everything else is flat, quiet and dense, like a native macOS
tool, so the page and the numbers carry the screen.

Premium here means five things, each enforced below:

1. **Restraint.** One accent, one icon library, two typefaces, no gradients (one exception: the accent sweeping through
   the text of a model call in flight, `Thinking`), no decoration. Variety comes from type.
2. **Precision.** A 4px grid, concentric radii, optical alignment, tabular numbers, hairlines instead of boxes.
3. **Speed you can feel.** High-frequency actions respond instantly; motion is spent only on rare moments.
4. **Native manners.** 13px UI, keyboard on every action, hover only on hover devices, a light window by default.
5. **Honesty in color.** Green only ever means points you gain; amber only means "left out, and why".

**Company logos are an exception to all five.** They're the company's brand, not our decoration: shown as the
company publishes them, never recolored, filtered or outlined, even where they break the palette. A logo is scaled to
fit `CompanyMark`'s square and rounded like the blank tile, which stands in until you set one (job header: click the
mark or **Upload company photo**, then drop, paste or choose an image). The one treatment a logo ever gets: an agent
gliide doesn't support yet (Codex, Gemini in the setup's Claude Code step) shows its icon grayed (`CompanyMark` `off`:
grayscale at half opacity), with "Not supported yet" beside it, so it reads as coming rather than available.

**A backdrop is the other exception, to Restraint only, and only when you choose one** (Settings → Appearance →
Overview backdrop; None by default). It's yours, like a desktop picture: a painting behind the top of the Overview,
veiled and screened in paper so ink reads on it, fading out to paper (white in light, near black in dark). Never on
another screen, never animated; the Overview's cards over it are `--surface` at `--backdrop-card`, so it shows through
them faintly. `Backdrop` in the components; the pictures and how the masks are made in `app/static/backdrops/README.md`.

### Evidence (Inspo MCP, 2026-10-12)

`recommend` over 24 sites for a calm, premium productivity tool: display class **grotesk-sans in 75%**, paper band
**mid**, accent hue **cool**. We take the gravity for UI text (Inter) and break it for display (a serif), the way the
references that feel most crafted do.

| Reference | What it gives us | What we leave |
|---|---|---|
| [Linear](https://inspomcp.dev/d/linear-app/DESIGN.md) | Dark theme register (charcoal, not black); 13px buttons; Inter tracking (`-0.011em` at 15px); 4px base; small radii (2–6) | Marketing-scale display type |
| [Granola](https://inspomcp.dev/d/granola-ai/DESIGN.md) | Warm off-white paper (`#fcfcf8` family); serif headline over a plain sans; green as the one success color | Its olive accent |
| [Zed](https://inspomcp.dev/d/zed-dev/DESIGN.md) | Serif display over a precise sans; cobalt (`#1c51dd` family) as the single action color; keyboard hints inside buttons | Mono body text |
| [Craft](https://inspomcp.dev/d/craft-do/DESIGN.md) | A document app whose chrome steps back for the page; serif for titles | Pastel illustration, large radii |
| Raycast, Superhuman, Arc (surveyed) | Keyboard-first density; one accent per product | Saturated brand gradients |

## Tokens

Use the variable; never its value. Adding a token needs a row here and a reason.

### Color (OKLCH, measured contrast)

| Token | Use | Never |
|---|---|---|
| `--paper` | Window background | Cards |
| `--surface` | Cards, rows, popovers, buttons | Window background |
| `--sunken` | Inputs, wells, kbd, segmented track | Raised things |
| `--desk` | Behind the resume page | Anywhere else |
| `--sidebar` | Sidebar, translucent over native vibrancy | Content |
| `--hover` / `--pressed` / `--selected` | Row and button washes; the selected row | Text |
| `--ink` | Primary text (16.6:1 on paper) | |
| `--ink-2` | Secondary text (7.4:1) | |
| `--ink-3` | Tertiary text, placeholders, meta (≥4.6:1 on every surface) | |
| `--ink-4` | Disabled and decorative only | Text that must be read |
| `--line` / `--line-strong` | Hairline separators / dividers that must show | Depth |
| `--input-border` | Form control edges (3:1 non-text) | Cards |
| `--accent` / `--accent-text` / `--accent-soft` / `--on-accent` | Primary action, focus ring, selection, links, the verdict badge | Meaning "good" |
| `--gain` / `--gain-text` / `--gain-soft` | Score gain, points, skills you have, success | Buttons |
| `--warn` / `--warn-soft` | A change left out by the fit check or validator; stretch verdict | Errors |
| `--danger` / `--danger-soft` | Cut, remove, errors | Warnings |
| `--mark` | Inserted keywords in change rows | |
| `--backdrop-veil` / `--backdrop-screen` | Opacity of the paper laid over a `Backdrop` picture, and of its dot screen; stronger in dark so light ink reads | Anything but `Backdrop` |
| `--backdrop-card` | The share of `--surface` in a card's background over a `Backdrop` (85%: the picture shows through, faintly) | Cards anywhere else |
| `--map-land` / `--map-park` / `--map-water` / `--map-road` / `--map-road-casing` | The job's map (`PlaceMap`): graphite cartography in both themes, water the one cool tint, so the accent pin and boundary carry it. Parks are a shade of land, never green (green means gain) | Anything but the map |
| `--page` / `--page-ink` / `--page-mark` | The resume sheet: white in both themes; marks match Typst's highlight | App surfaces |

**Light is the default.** Dark is the same hues on charcoal (`--paper` ≈ `#0f0e0d`) and applies only when the root
carries `data-theme="dark"`: Settings → Theme is **Light** (default) / **Dark** / **System**. System sets the attribute
from `prefers-color-scheme` and updates it on change, inside the theme-switch suppression. `lint.mjs` fails if the
dark block overrides a token the light `:root` doesn't define.

### Type

| Role | Token | Font | Use |
|---|---|---|---|
| Score | `--text-score` 56/56 | Serif, tabular | The projected score, nothing else |
| Headline | `--text-headline` 30/36 | Serif | The job title; one per screen |
| Title | `--text-title` 22/28 | Serif | Screen titles, empty-state titles |
| Large | `--text-lg` 16/24 | Inter | Section headings inside a screen |
| Reading | `--text-md` 14/22 | Inter | Resume lines: bullets, rewrites, notes |
| UI | `--text-base` 13/20 | Inter | Everything you operate (macOS native size) |
| Meta | `--text-sm` 12/16 | Inter | Counts, reasons, timestamps |
| Label | `.label` 11/16 | Inter | Group labels, eyebrows, table headers |

- Weights: 400, 500 (labels, buttons, nav), 600 (priority-1 skills, emphasis). Never 700 in UI.
- No full capitals in headlines, titles, labels, eyebrows or table headers: sentence case, never `text-transform:
  uppercase`. Titles that come from your data in capitals (a bank section's `WORK EXPERIENCE`) are shown in sentence
  case (`Work experience`; two-letter acronyms like `IT` stay). Only the resume page prints them as written.
- Every number that changes is `tabular-nums` (`.num`).
- Serif is display-only: never for controls, never below 22px except inside the resume page.
- Headlines `text-wrap: balance`; paragraphs `text-wrap: pretty`.

### Space, size, radius

- 4px grid: `--space-0-5` (2) … `--space-16` (64). Row padding 12, column padding 20–24, between groups 24.
- Controls: `--control-sm` 24, `--control-md` 28 (default), `--control-lg` 36 (inputs, primary calls on empty states).
- Window: `--sidebar-width` 232, `--toolbar-height` 52, change column `--column-min` 420 to `--column-max` 580, desk
  takes the rest; at 100% the page fills the desk's width less 12px a side (at most 816px, US Letter at 96 dpi).
- Radius: `xs` 4 (kbd, checkbox), `sm` 6 (buttons, inputs, nav items), `md` 8 (rows, segmented), `lg` 12 (cards,
  popovers, toasts), `xl` 16 (sheets), `full` (chips, badges). **Concentric:** outer = inner + padding + border
  (segmented: 6 + 2 = 8). Past 24px of padding, surfaces are independent.

### Elevation

| Token | Use |
|---|---|
| `--shadow-border` / `--shadow-border-hover` | Cards, buttons, kbd: a ring plus lift, from better-ui exactly. Always with `border: 1px solid transparent` for forced-colors mode |
| `--shadow-popover` | Menus, popovers, toasts |
| `--shadow-page` | The resume page and the window frame. Nothing else gets this much depth |

Borders are for structure only: separators (`--line`), inputs (`--input-border`), selected states. Never a border for depth.

### Motion budget

| Interaction | Motion |
|---|---|
| Toggling a change, typing, hovering a row, switching tabs or modes, score and bar updates | None, or ≤150ms on `background-color`/`opacity` (`--dur-instant` 100ms for row hover) |
| Pressing a button or chip | `scale: var(--press-scale)` (0.96) over `--dur-fast` 150ms, never when disabled |
| First result arriving, export success, empty state | `.enter`: opacity + 4px blur + 12px rise over 300ms, groups 100ms apart |
| Exit of a transient element | Opacity + 4px blur + `-12px` over 150ms, or instant |
| Icon swap on an infrequent state (copy → copied, export → done) | Cross-fade: scale 0.25→1, opacity, blur 4px→0, 300ms `--ease-emphasized` |
| Opening a menu or popover | Opacity in over `--dur-instant` 100ms (`@starting-style`); closes instantly |
| A tooltip | Opens after 300ms of rest (the next within 400ms of one closing: at once), opacity in over `--dur-instant` 100ms; moving to the next or opening on keyboard focus: no fade; closes instantly. No movement |
| A model call in flight | `Thinking`: the stage is ink with a band of `--accent-text` sweeping through it left to right (a text-clipped gradient, `linear`) every `--dur-loop` 1600ms; reduced motion and forced colors: the stage in `--accent-text`, fading. No bar, no icon: the stage sentence and its seconds say where it is |
| Moving between the setup's steps | The leaving step: `exit({ side })`, opacity + 4px blur + 12px toward where you came from over 150ms; the arriving one: `.turn`, its parts 100ms apart, each with opacity + 4px blur + 12px from where you're going over 300ms, after the exit. The `Steps` fill sweeps in (or back out) from the leading edge over 300ms `--ease-emphasized`. Reduced motion: the parts and the fill only fade |
| Theme switch | No transitions at all (`.theme-switching` on the root for two frames) |

Movement, scale and blur run only under `prefers-reduced-motion: no-preference`; reduced motion gets the opacity fade.
Transitions name their properties, never `all`. Every animated change also leaves a static cue (color, icon or label).

### Icons

- **Lucide**, one library everywhere. Outline by default; filled only for an active state.
- Rendered stroke matches the adjacent text: 1.5px beside 400, 2px beside 500–600 (buttons, chips), 2.5px for bold or
  a standalone emphasized icon. `Icon.svelte` renders Lucide with `absoluteStrokeWidth` (`weight="regular" | "medium" |
  "bold"`), so these are real pixels.
- Sizes: 16 (default), 14 (small buttons, meta), 12 (inside chips). `currentColor` only; state comes from CSS.

## Components

Every primitive and its states. Screens compose these from `$lib/ui` (`app/src/lib/ui/`); a new primitive is added
there and shown in the `/specimen` route in every state before any screen uses it.

| Component | Key props | States |
|---|---|---|
| `Icon` | `icon` (Lucide), `size` 12/14/16, `weight` | stroke per weight |
| `Button` | `variant` default/primary/ghost/danger, `size` sm/md/lg, `square`, `icon`, `iconEnd`, `kbd`, `static` | default, hover, pressed, focus, disabled; optional `Kbd` hint (primary tints it); 2px less padding on the icon side |
| `Kbd` | children | inside buttons, in the command menu |
| `Segmented` | `options`, `value`, `label` | on, off, hover, disabled |
| `Input` / `Textarea` | `value`, `size` | placeholder, focus (accent border + 3px ring), disabled; `Textarea size="lg"` is the reading size for inline edits |
| `Checkbox` | `checked`, `label` | off, on, focus, disabled |
| `Chip` | `state` have/add/missing/not-mine, `p1` | press, hover, disabled |
| `Badge` | `tone` neutral/accent/gain/warn/danger, `dot` | |
| `CompanyMark` | `size` 16/20/28/48, `src`, optional name (children); `label` makes it a button (`onclick` or `popovertarget`), `over`, `off` | blank tile; logo (scaled to fit, rounded like the tile, no tile or ring behind it); with its name, which truncates; as a button: image icon on hover and focus, accent ring and wash while files are dragged over it; `off`: grayscale at half opacity, for an agent not supported yet. 16 in one-line lists, 20 beside a name in one line, 28 beside two lines (the sidebar's Recent, the Applications table, the Overview's Up next, the setup's agents), 48 in the job header across the company and the title (the button). The agents' icons are bundled in `app/static/agents/` (where each came from in its README) |
| `Sources` | `sources` (`name`, `src`), `label`, `note` | where a list's postings come from: the label in meta, each source's icon as a 16px `CompanyMark` (a brand icon, so the logo rules hold: as published, scaled to fit, rounded like the tile) named in a tooltip and to assistive tech, then the note in meta; a source without an icon shows the blank tile; wraps between the icons and the note. The icons are bundled in `app/static/sources/` (where each came from in its README), never fetched |
| `LogoDrop` | `over`, `busy`, `set`, `onpaste`, `onchoose`, `onremove` | the job header's photo panel, inside a `Popover`: a dashed well to drop an image file on, Paste `⌘V`, Choose file…; files over it (accent well); setting; Remove once a logo is set |
| `Marked` | `text`, `marks` | inserted keywords in `<mark class="mark">`; the sheet's own highlight comes from the renderer |
| `ChangeRow` | `state` in/off/left-out/forced, `checked`, `selected`, `points`, snippets `text`/`actions`/`editor` | actions appear on hover, focus or selection, keeping their place while hidden; when they and the meta don't fit on one line, they wrap under it, never squeezing it; reason line ("Left out: …", "Kept on by you · …"); warning; "Your edit" owner; editing. The meta names only what the text doesn't show: terms it adds that aren't marked, its lines |
| `GroupHead` | `label`, `meta`, `collapsed` (bindable), `controls`, `ontoggle` | label + count + points; with `collapsed`, the whole head is a disclosure button (`aria-expanded`): open (chevron down after the label), folded (chevron right; the meta stays, so it still says what it holds), hover lifts label and chevron to ink 2, focus ring; no motion |
| `ScoreHeader` | `now`, `projected`, `ceiling`, `components` (`id`, `label`, `weight`, `present`, `value`, `brief`) | updates instantly; the parts in a popover on hover or focus: a row each with its Lucide icon, name, weight (share of the score) and how much is met (a 24px meter and the percent), its `brief` under it ("2 of 3 · missing Docker"); met in full: icon, meter and percent in gain; none met: percent in ink 3; parts the posting doesn't ask for named in one line at the foot |
| `ScoreBar` | `projected`, `now`, `onFile`, `ceiling`, `bar` | the review's score bar in small, for the Fit columns of Applications and Feed: an 8px sunken pill from 0 to 100 before the score that counts (on file, else projected), the number (500, ink) at the cell's edge. Ink 3 what the master had, green what your edits added (2px apart), amber what the PDF lost (outlined; the number in amber too), a 2px tick at the ceiling. `bar={false}`, wherever a fit score sits beside a name outside the job page (the sidebar's Recent, the Overview's Up next and New for you): the number alone with its percent sign (`78%`), styled as the bar's (500, ink; amber when the PDF lost points), with the same label and tooltip. States: tailored; small gain; at its ceiling; gain then lost in the PDF; lost, no before (imported); no edits; fit only (a posting not sent, a feed job); edges 0 and 100; each with and without the bar. Every number in its label (`role="img"`) and in its `Tooltip`: Fit and the score, then each part beside its legend key (Before your edits, From your edits, Lost in the PDF, Ceiling), and "Read back from the exported PDF" once it's on file; updates instantly; forced colors: CanvasText bars, Highlight gain, the lost part outlined |
| `Nav` / `NavItem` | `href`, `icon` or `lead` snippet, `current`, `meta` (+ `metaTitle`) or `trailing` snippet, `quiet`, `sub`, `status` | hover, current, count; `quiet` for list entries; two lines with `sub`: Recent jobs are a 28px `CompanyMark` (its edges on the company's cap height and the role's baseline), the company and its fit as a `ScoreBar` number (`trailing`, `bar={false}`; an application's scores once it has one, so it turns amber as Applications' does) over the role and the application's status (a dot in its badge tone, then its word), each line truncating, the status never |
| `SectionNav` | `sections` (`id`, `label`, `sub`), `root` (the scroller), `label` | quick ways to the sections of a long screen (Settings): a link per section, 13/28 500 in ink 3, a `sub` one indented 24px in 400; hover (ink 2 on the `--hover` wash); current (ink, a 2px `--accent` tick at its leading edge; forced colors: `Highlight`). Follows `root`'s scroll: a section is current once its top reaches its scroll margin, the last at the end, and the one you jumped to while its top is in view. A link jumps at once (no smooth scroll), moves `root` alone, and focuses its section (`tabindex="-1"`, no focus ring: the current link alone marks where you are); no URL change. Under 720px of its container (`container-type: inline-size`): a row over a hairline, every link 500, the tick on the hairline under the current one, scrolling sideways to keep it in view |
| `Card` | `pad` | |
| `Desk` | `pages` (SVG), `status` snippet, `zoom`, `above` snippet | fits / over the page limit (badge); zoom 50–200, 100% fits the desk; with `above` (the job's map) the pane splits: it fills the pane's top, edge to edge over a `--line` hairline, the desk under it, and at 100% the page shrinks (to 60% of the desk's width at least, then the desk scrolls) so the two fill the pane exactly, the page's foot 12px from the pane's; `above` takes the height left, at least 160px |
| `PlaceMap` | `places` (`name`, `region`, `lat`, `lon`, `bbox`, `boundary`), `onopen`, `unavailable` | where a job is, as a maps search shows a town: OpenFreeMap vector tiles drawn by MapLibre (lazy-loaded), framed to every place's box (up to zoom 12), static (no pan or zoom); each place a pin in `--accent` (its eye in `--on-accent`, haloed in land) with its name beside it (Inter 500, ink), the tiles' own label for it hidden; its boundary dashed in accent over a 6% accent wash; the towns (ink 3) and cities (ink 2) around it in Inter 400 with land halos, roads, water and state lines in the `--map-*` tokens, every color read from the tokens and repainted on a theme switch. Edge to edge, no radius or ring: its holder draws the edges. With `onopen`, the whole map is a button ("Open Plano, Texas, United States in Maps", tooltip Open in Maps, a 2px accent ring inside its edges on focus). Until the tiles arrive the name (500) over the region (meta) stand in, centered on land, and the map and its credit fade in over them (opacity, 300ms); tiles that can't load (or `unavailable`) leave them with "Map couldn’t load". The credit, in meta at the bottom right on land, reads "OpenFreeMap © OpenMapTiles © OpenStreetMap", under 320px just "© OpenStreetMap". Labels are drawn from the bundled Inter glyphs (`app/static/glyphs/`, Latin only) |
| `Toaster` + `toast()` | `tone` neutral/gain/warn/danger, `action` | enters with `.enter`, exits with blur and -12px; errors stay until dismissed |
| `Empty` | `title`, `body`, `action` snippet | title (serif), one sentence, one primary action with its key; staggered `.enter` |
| `Toolbar` | `crumbs` snippet, trailing children, `over`, `clear` | drags the window; `<hr>` draws a divider; compact: room for the traffic lights and the sidebar toggle; in a window without a sidebar (the setup): room for the lights alone; `over`: laid over the top of the screen's scroll area (which starts under it), in paper with its hairline; `clear` (with `over`): unpainted, no hairline, so a `Backdrop` shows through it, until the content scrolls under it |
| `KeyFooter` | `keys` (`KeyCommand`: `keys`, `label`, optional `run`, `disabled`) | collapsed: one ghost button, **Commands** `⌘K`, sticky at the column's foot; open: a menu above it listing every command with its keys; a command with `run` is an item (click, or ↑↓ Home End ⏎), disabled while it can't run; one without (↑↓ Select) is listed for its keys only; Esc, Tab or a click outside closes it |
| `Menu` | `value`, `options` (`label`, `meta`), `label` | closed, open, active option, chosen (check), disabled; ↑↓ Home End ⏎ Esc |
| `Popover` | `open`, `anchor`, `placement`, `mode` auto/manual | anchored, flips when it would overflow; `--shadow-popover` |
| `Tooltip` + `tip()` | `{@attach tip(content)}` on any element, or on a component that spreads its rest props onto its element (`Button`, `Chip`, `NavItem`); content, or a function read while open; `content`: a string, or `TipData` (`title`, `value`, `tone` gain/warn, `rows` of `label`, `value`, `key` base/gain/lost/tick, `tone`; `note`); null or "" shows none. The layout renders `<Tooltips />`, which shows one at a time. The only tooltip: never the native `title` attribute | label (one line, or a sentence that wraps at 280px; paths and links break anywhere); value alone; title and value; rows; rows with legend keys (the chart's own marks: pill segments, the ceiling tick); a note in ink 3. 12/16 type, `--surface`, `--shadow-popover`, `--radius-md`; 4×8 padding for a label, 8×12 for data. Centered 6px above its element, below when there's no room, 8px inside the window; never takes the pointer. Hover with a mouse (300ms rest, then instant while moving between tooltips) or keyboard focus opens it; leaving, a click (until you leave), Esc, scroll, resize or the window losing focus closes it. The element keeps its own accessible name; while open it's `aria-describedby` the tooltip. Never for an action's only label: icon buttons keep `aria-label` |
| `Thinking` | `label`, `steps` (`label`, `ms`), `startedAt`, `oncancel` | stage alone; with steps already done (a check and their seconds); with Cancel |
| `Steps` | `steps` (names), `current` (from 0; `steps.length` once all are done), `label`, `onselect` | a 4px bar per step over its name in meta; done and current filled in `--accent`, upcoming the `--line-strong` track; names: done ink 2, current ink 500, upcoming ink 3; with `onselect`, done steps are buttons back to themselves (hover lifts the name to ink); under 480px the names give way to one caption ("Step 2 of 6 · Appearance") |
| `Table` | `columns` (`sortBy`, `sortFirst`), `rows`, `cell` snippet, `sort` (bindable) | sticky header, tabular numbers, hover wash, no zebra; a sortable header is a button (`aria-sort`): sorted, ink with its arrow (on the label's outer side, so end-aligned labels keep the numbers' edge); unsorted, ink 3 with the arrow faint (`--ink-4`) on hover or focus; a second click reverses; first click descends for numbers and dates, ascends otherwise; empty values sort last both ways, ties keep the given order; with `selected` (and `onselect`), one row is the screen's selection, in the `--selected` wash (no hover wash over it), scrolled into view when it moves; a click or focus inside a row selects it (Feed: ↑↓ move it, the row keys act on it) |
| `Heatmap` | `counts` (by `dayKey`), `weeks`, `end`, `label`, `describe` (a day's `Tooltip` content), `caption` snippet | one cell per day, a column per week, Sunday on top; empty well, then four steps of `--accent` mixed into `--sunken` (in oklab, so the hue holds) against the busiest day shown (a data series, not "good"); month labels; hovering a day shows its tooltip (by default its date and count); Less–More legend; forced colors: `Highlight` at stepped opacity |
| `Backdrop` | `name` (`BACKDROPS`: `meadow`, `tokyo`) | fills the top 560px of its positioned parent (content above it is positioned too) and scrolls with it, behind a `clear` `Toolbar` that lies over it: the picture covering it, `--paper` at `--backdrop-veil` over it, a 3px dot screen in `--paper` at `--backdrop-screen`, then below its top 64px a 496px fade out to paper (a smootherstep mask at Retina resolution, its alpha dithered with blue noise so it never bands); `aria-hidden`; gone in forced colors; no motion |

## Layout

```
┌──────────┬────────────────────────────────────────────────────────────────┐
│ ● ● ●    │ Apps › Globex   [Resume|Cover letter] SWE ▾ │ [Export E] Del │  toolbar 52
│          ├──────────────────────────────┬─────────────────────────────────┤
│ Overview │ Tailor first · Remote, US    │  ● Fits on 1 page        − 100% + │
│ Apps     │ ▢ Globex                     │  ┌───────────────────────────┐   │
│ Exp.     │   Junior Software Engineer   │  │                           │   │
│ Resumes  │ 53% › 78%        Ceiling 85% │  │      resume page          │   │
│          │ ━━━━━━━━━━━━━━━━━━━━━━━━━━━━ │  │      (live SVG)           │   │
│ RECENT   │ SKILLS  chips…               │  │                           │   │
│ …        │ ☑ change row                 │  └───────────────────────────┘   │
│ Settings │ Commands ⌘K                  │              desk                │
└──────────┴──────────────────────────────┴─────────────────────────────────┘
  232 (vibrancy)   column 420–580                 rest
```

- **Window:** Tauri `titleBarStyle: "Overlay"`, transparent, with the traffic lights inset into the sidebar
  (`trafficLightPosition` 20, 28: centered on the 52px toolbar line). `app/src-tauri/src/window.rs` puts the macOS
  sidebar material (`window-vibrancy`, `NSVisualEffectMaterial::Sidebar`, dims when inactive) behind the webview and
  sets `data-vibrancy` on the root; only then is the page background transparent, so `--sidebar` shows the material
  and the main area paints `--paper`. Without it the sidebar is `--sidebar` over paper. Verified on macOS 26; the
  window's appearance follows the app theme so the material matches. Windows (Mica) is unverified until M7.
- **Dragging:** the `Toolbar` (`data-tauri-drag-region="deep"`: any empty part) and the sidebar's top 52px.
- **One primary action per screen**, in the toolbar's trailing edge, with its key; only a delete (the job's trash)
  comes after it, at the very end.
- **Keys:** every row action has a key; the key footer's **Commands** menu (`⌘K`) lists the screen's keys and runs
  any of them with a click.
- **Compact (iOS, narrow windows under 960px):** the sidebar collapses to a sheet (the toolbar's sidebar button,
  Esc or a click outside closes it), the desk becomes a tab.

### Screen recipes

| Screen | Composition |
|---|---|
| Overview (`/`, the window's first screen) | Toolbar: **Import resume** `I`, **Paste from clipboard** `V` (primary). With a backdrop chosen (Settings → Appearance), a `Backdrop` behind the top of the screen, toolbar included, scrolling with it: the toolbar lies over it `clear`, without its **Overview** crumb (the buttons unchanged), and takes its paper and hairline back once the content scrolls under it; the headline and heading rows sit on it, the cards over it in `--surface` at `--backdrop-card`, so it shows through them faintly. Column (max 880): the greeting as the headline ("What’s up next, Jane?", the bank's nickname or first name); then **Activity** and **Up next** side by side (5:4, tops aligned: each a heading row over its card; stacked once the column is under 760px). **Activity**: range `Segmented` (All, 30 days, 7 days) in its heading row; the card holds six stats in serif under meta labels, three a row with a hairline between the rows (postings, applications, interviews, active days, average fit, points from tailoring in `--gain-text`; the last two say how they're counted in a tooltip), then the last 26 weeks as a `Heatmap` with one sentence under it (a day's tooltip: its date over Postings added and Applications sent, both on every day). **Up next**: applications in interviews, then postings not sent yet, best fit first (at most 5), each row opening its job and stacked like the sidebar's Recent: a 28px `CompanyMark` across the company (500, ink) over the role (meta size, ink 2), each truncating, then the status `Badge` centered on both lines and the fit as a `ScoreBar` number (`bar={false}`; an application's scores once it has one, so it turns amber as Applications' does) over the day; **Import your resume** `I` first while there's no bank (the icon in a 28px box, so its text lines up with the companies). **New for you**, under both, the column's full width (hidden while the feed is off): the feed's fits, newest posting first (the employer's date, else when its source found it; best score first on a tie; at most 8), as Up next's rows with no status: the fit as a `ScoreBar` number over the posting's age (the employer's date, "2 days ago", else "Found today") and the source that found it ("2 days ago · LinkedIn"), and the knockouts, when there are any, as a third line in ink 3; a row reads the posting if it must (the age says "Opening…") and opens its job, which then joins Recent and Applications. Its heading row: what the feed is doing, as text only ("Checking Wing…", "Pulling new postings…"), how many it checked today ("12 checked today") and **See all** (ghost, to Feed). No fits: one sentence, or the last pull's error |
| Setup (`/welcome`, a new install's first screen) | Shown while settings say it was never done or skipped and there's no bank (settings saved before it existed count as done); no sidebar, nor in the import review it opens. A clear `Toolbar` with **Skip setup** (ghost); column (max 608): `Steps`, then the step: headline, one sentence in reading size (ink 2), a `Card` of rows (a 28px icon box in accent, the name 500 over a meta line, a control at the end), sometimes a meta note. Footer (hairline, same column): **Back** `⌘[` (ghost, from the second step) and the one way on `⏎` (primary; default "Skip for now" while Experience has nothing imported, whose **Choose a file…** `I` is then the primary). Steps: Welcome (the three promises), Appearance (theme and backdrop, applied at once), Claude Code (Claude's icon as a 28px `CompanyMark`, status `Badge`, the problem sentence, **Check again** `R`, **How to install** when not found; then **Other agents**, a label over a card of Codex and Gemini CLI, each `off` with its maker and plan in ink 3 and a neutral "Not supported yet" `Badge`), Experience (import; the review comes back here), Feed (Pull new postings, its sources, what Auto looks for), Ready (each part done or not, a way back to its step; the first keys). **Open the Overview** or Skip setup saves it as done |
| Applications (empty) | `Empty` centered on paper: "No postings yet", one sentence, **Paste from clipboard** `V` |
| Job → Review | As the specimen: job header (verdict badge and meta: location, salary range, source; then a 48px `CompanyMark` across the company, 16/24 500 ink 2, over the headline, its edges on the company's cap height and the title's baseline), score, skill chips, change groups, then **Other lines** (a `GroupHead`, then a `PartRow` per master line no change touches, labeled with its entry: Edit `⏎` and, for bullets and the summary, Rewrite with a note `N`; either makes it your edit, which lands selected in its section's group), Add your own `A` (a skill, a bullet, or a summary when the master has none), key footer (Commands `⌘K`); desk with page. Where the posting names a place (not remote, a country or a state), the preview pane splits: the job's `PlaceMap` fills its top, edge to edge over a hairline, and the desk moves down under it, the page shrunk at 100% so the two fill the pane exactly; the map opens in Maps on a click or `M` (on either document). No place, or no answer from the geocoder: the desk alone, as before. The salary range reads in full in the meta line ("$85,000–$115,000 a year", "Up to $45 an hour"), ink 2, and opens its panel on a click or `$`; without one, a ghost **Add salary** `$` sits at the meta line's end before **Upload company photo**. The panel (a `Popover`): low and high end (either may stay empty; typed as 80,000, 80k or 45.50), currency code, Per year / Per hour `Segmented`, **Save** `⏎`, and **Remove** (danger) when there's one; what's wrong says so in one line under the fields and Save waits. Saving makes it yours: a re-read of the posting keeps it. Every section (Skills and keywords, each change group, Other lines, Gaps, Knockouts) has a collapsible `GroupHead`: a click folds it, another opens it; folded sections are remembered across jobs and launches, and a group your new edit lands in opens. Under the score, at the trailing edge, one ghost button names the next step: **Collapse all** `C` while any section is open, **Expand all** `C` once every one is folded. ↑↓ moves through the changes, then the other lines, skipping folded sections. Crumbs: Applications › company, then, once the posting has an application (after its first export), its status `Menu` as Applications' Status column draws it (ghost, small, the status `Badge`), saved at once. Toolbar, in two groups split by a divider: what you're working on (the document `Segmented`: Resume, Cover letter, `Tab`; then the resume's master `Menu`, or the letter's Rewrite letter `W`), then export and delete (**Export** `E` or **Export letter** `E`, then the trash right after it: Delete posting `⌘⌫`, back to Applications, then deleted as there). Master only / Bank-aware is a setting (Settings → default suggestions), not a toolbar control |
| Experience (`/bank`; the sidebar's name for the experience bank) | Toolbar: **Import resume** `I`. Column (max 880): the bank's name as the headline over when it last changed and its rev (meta); no stats. Then each section: a heading row (16/24 500; its entries and bullets, or lines and items, in meta at the end) over a `Card` of rows with hairlines between. An entry (the whole row opens its editor): its name (14/22 500) and its dates as printed (meta ink 2), its subtitle and place (ink 2) and its length (meta), its first bullet in reading size (ink 2, two lines at most), then its bullets and variants in meta, its tags as neutral badges and the masters that show it as accent badges (with one master, only "In no master" on an entry it leaves out). A skills line: its label and masters over its items as neutral badges. The entry's editor (`/bank/entries/<id>`): first **Details**, a `PartRow` each for Name, Subtitle, Place and Dates (Edit `⏎` in an `Input`; the name can't be emptied, the rest can, an empty one reads "None" in ink 3), then one per line under the entry ("Line 1", its bold label before its text; Edit `⏎` in a label `Input` over a `Textarea`, Remove `⌫` with Undo in the toast) and Add a line `L`; each editor says "Changes it in" the masters that show the entry. Then each bullet's wordings (its text, then its variants) as rows with the masters showing each as badges; row actions Edit `⏎`, Make it the text `M`, Remove `⌫` (with Undo in the toast), Show in a master `1–9`; Add a variant `V`, Fold into… `F` per bullet. Compact: Entry \| Preview tabs. Desk: the entry in a master that shows it (`P` picks which), highlighted; the entry alone when none does |
| Import review | As Review: the column lists the file's sections (where each goes), each entry `New` or `Yours` (merged, with **Keep separate**), skills lines' new items, offered contact items, and **Save as** (new master's title, or one to replace); the desk shows the master it makes, what's new to the bank highlighted. `⌘⏎` adds, `Esc` discards |
| Resumes (`/masters`; the sidebar's name for the masters) | Column: **Masters**, a card per master (title, page-status `Badge`, entries and last change; Template `Menu`, Page limit `Segmented`, each saved at once); then **Export folder** (its path, a `Segmented` for what to show: All, Resumes, Cover letters, and Other only while a PDF's name says neither; then a `PartRow` per PDF shown, newest first: when it was saved over the file name). The count reads "18 PDFs", "6 of 18" while filtered; `Esc` shows all again; the filter stays as you left it when you come back. Nothing of a kind: one sentence and **Show all PDFs** `Esc`. Desk: the selected PDF (`↑↓`, among those shown), the page count as a neutral `Badge`, the file name in the bar (the full path on hover); while the next one opens, the last stays. A PDF that's gone since the listing: a danger toast and the folder listed again; one that can't be read: "Can’t show it" (danger) and why. No masters yet: one sentence and **Go to Experience** `E` over the folder; no masters and no PDFs: `Empty`, **Go to Experience** `E` |
| Applications | Every posting, newest first (the day it was sent, else added), as a full-width `Table` on `--surface`: tabular numbers, row hover wash, no zebra stripes. Over the table, on the same surface and inset to the first column: a filter `Input` (md, company or role, every word must match; `/` or `⌘F` focuses it, `Esc` clears it, then leaves it), a `Segmented` for what to show (All, Not sent, then each status) and the count at the trailing edge ("24 postings", "3 of 24" while filtered). Every column but Link sorts from its header (status along the pipeline: Not sent first; Salary by the range's top a year, hourly at 2,080 hours; Fit by the score that counts); filters and sort stay as you left them when you come back from a job. Nothing matching: `Empty` "No matches", one sentence, **Clear filters** `Esc`. Toolbar: Open applications.csv, **Paste from clipboard** `V` (primary). The Job column stacks like the sidebar's Recent: a 28px `CompanyMark` across both lines, the company (500, ink) over the role (meta size, ink 2), each truncating; the cell opens the job, the company turning accent on hover. Salary, after Job: the posting's range, short ("$85K–$115K", "$45–$55/hr"; in full on hover), an application's through its posting, "—" without one. Fit, after Salary: one `ScoreBar` per row for the score before tailoring, after it and as the PDF reads it (an application imported without its projected score uses the one on file); a posting not sent yet shows its fit score alone; "—" before its requirements are read. An application has its status as a `Badge` in a `Menu` (saves at once, rewrites applications.csv), its resume and its link. A posting not sent yet has a "Not sent" badge, its link, and a trash (a ghost square button, shown on the row's hover or focus, always on touch) that deletes it with Undo in the toast: it leaves every list at once and is deleted for good (reviews, drafts, letter) when the Undo runs out |
| Feed (`/feed`) | Toolbar: crumbs **Feed**, **Pull now** `P` (primary; "Pulling…" while one runs). On `--surface`, inset to the first column: a `Segmented` (Fits, Checked, Waiting, Search), then, as Applications, a filter `Input` (md, company or role, every word must match; `/` or `⌘F` focuses it, `Esc` clears it, then leaves it; on Search the search field takes its place) and the count at the trailing edge ("4 jobs", "2 of 12" while filtered; Search: "25 of 1,240"), then one meta line: what the feed is doing ("Checking Wing…"), else "Pulled 2:14 PM · next 5:14 PM · 12 of 20 checked today", or the last pull's error in `--danger` (a sentence per failed source, "LinkedIn: …", wrapping rather than cut off); on Search it says what the search asks instead: **Searches**, a `Sources` strip (LinkedIn while the feed pulls it, then freehire, Greenhouse, Lever, Ashby, Workday and SmartRecruiters, each named in a tooltip) and "and 260+ more boards through freehire", with a page's error at the trailing edge, under the count ("LinkedIn: …" in `--warn` with its warning icon, as a change row's reason: those results were left out, the rest still list; the next search clears it). The list is a `Table` whose every column but the actions sorts from its header (Fits and Checked start by Fit, best first; Waiting in the check queue's order and Search in the search's, freehire's by relevance then LinkedIn's, until you sort; each tab keeps its own sort, and ↑↓ follow the order shown): Job (as Applications': mark, company over role; a button that opens it), Fit (a `ScoreBar`, as Applications' for a posting not sent), Posted (the employer's age, else when its source found it, over that source in meta: freehire, LinkedIn, Watchlist or Fantastic.jobs; newest first), Master (the best one's title), State (along the feed: Fit an accent `Badge`, the rest neutral; Checking… in accent text; then the knockouts or the reason in meta, in full on hover) and the row actions, shown on the row's hover, focus or selection (always on touch): **Check** (not checked yet), the employer's page (a LinkedIn job's posting on LinkedIn, and its tooltip and key say so: LinkedIn's guest page hides the employer's link), Dismiss (a search result too: it leaves the results and the feed won't offer it again). Keys: ↑↓ select, ⏎ Open, `O` employer's page, `C` Check now (past the daily cap), `D` Dismiss, `P` Pull now, `/` Filter (Search: the search); the key footer (Commands `⌘K`) lists them. Search: a md `Input` ("Search jobs: a title, skill or company"; ⏎ searches, `Esc` clears, then leaves) and **Search**: freehire, and LinkedIn while the feed pulls it, with the feed's filters plus your words, a page at a time (freehire's, then LinkedIn's) with **Show more** under the table while either has more; Search's count is of both (freehire's matches plus LinkedIn's found so far); the tab, the filter, each tab's sort and the last search stay as you left them. Empty: `Empty` per tab ("No fits yet" with the threshold, "Nothing checked yet", "Nothing waiting", "Search jobs", "No results"); nothing matching the filter: "No matches", **Clear filter** `Esc`; feed off: "The feed is off", **Open Settings** `⌘,` |
| Settings | A `SectionNav` rail (160px, held in view) in the left gutter beside a single column of cards (max 640), its first link on the line of the card heading beside it; the column stays centered while the gutter has room for the rail, then moves right. The links: Agents, Tailoring, Feed with Sources under it (once the feed's settings are in), Export folder, Appearance; a jump lands a card where the column starts. Under 720px of the screen's width, the links are a row over the column, held at the top in paper as the cards scroll under it. The cards: **Agents**, a row each (hairlines between, as fields): a 28px `CompanyMark` with the agent's icon (`static/agents/`), its name (500) over its maker and what's known of it in meta ink 3 ("Anthropic · 2.1.0 · Max"; "not installed on this Mac"), then at the end Claude Code's status `Badge` ("Checking…" while it checks) and a ghost square Check again; under its name, the problem (danger wash) when there is one, then its path: one line, Path then the path in use in meta (yours, else the one found, "· found on your PATH"; in full on hover) and **Change path** (ghost sm) at the end. Change path opens the editor in its place: Path and a md `Input` (focused), a hint under it, then at the trailing edge **Remove** (danger sm, while a path is set: back to finding it on your PATH), **Discard** `Esc` (ghost sm) and **Save** `⏎` (primary sm, while it's changed; empty saves as Remove); each closes it, focus back on Change path, and a save checks Claude Code again; a failed save keeps it open with the toast. Then Codex (OpenAI) and Antigravity (Google), grayed: mark `off`, name in ink 3, "not supported yet" in the meta, a disabled **Connect**. Then models, **Feed**, export folder, **Appearance**: theme `Segmented` (Light, Dark, System), then Overview backdrop `Segmented` (None, Meadow, Tokyo, Sakura; saved at once). **Feed** (saved apart, each control at once; fields on blur or ⏎): Pull new postings (`Checkbox`), Check against (`Menu`: All masters, then each), then Seniority, Employment, Category and Countries, each with Auto \| Custom (`Segmented`) on its line and under it what Auto works out in a meta sentence ("Junior, from 2.5 years of experience."; no country: "Anywhere…") or the Custom `Checkbox` list in columns (starting from Auto's values; the last one checked can't be cleared); then three checkboxes (only jobs that sponsor a visa, include jobs that need a clearance, include jobs without a known place) and two md number fields, Daily checks ("a day") and Fit threshold ("%"), right-aligned, a whole number in range or back to what's stored with a toast saying why. The lede names the sources pulled ("New postings from freehire, LinkedIn and 3 careers boards, …"). Then **Sources**, a label over its own fields, each with a meta sentence under it: LinkedIn (`Checkbox`; it searches your masters' titles, and its jobs link to LinkedIn, not the employer), Watchlist (a md careers-link field and **Add**, ⏎ or the button; cleared once saved, kept with the backend's toast when it isn't a board gliide can watch; the boards listed by name and ATS in ink 3, each with open (its link in the tooltip) and Remove; none: "No boards yet…"; the sentence names the seven ATS), Fantastic.jobs (a masked md field for the Apify token, on blur or ⏎, empty turns it off; once a day, Apify's per-job price, and a **Get a token** link to Apify's console) |

## Voice

Short, specific, sentence case. Say what happened and what to do next: "Left out: “40%” isn’t in your resume",
"Saved globex_jr_sde_doe_resume.pdf". No exclamation marks, no "Oops", no emoji. Numbers as digits.

## Adding a feature or screen

1. Compose from `$lib/ui` (the components table). If something's missing, add the primitive to `app/src/lib/ui/`
   (export it from `index.ts`) and every state to the `/specimen` route first.
2. Need a value that isn't a token? Use the nearest token. If none fits, add a token to `tokens.css` (both themes)
   and a row in this document, with the reason.
3. `node design/lint.mjs` exits 0.
4. Run the better-ui review (`.agents/skills/better-ui/SKILL.md`, "Before you finish" and "Reporting") on the changed
   files and fix every `HIGH` and `MEDIUM`.
5. Look at it in both themes and with reduced motion on, at 1280×800 and at the compact width.
