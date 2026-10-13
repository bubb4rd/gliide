# Paper & Ink: the gliide design system

Binding for every screen, page and component. When this document and a screen disagree, the screen is wrong.

| File | Role |
|---|---|
| `design/tokens.css` | The only place raw colors, shadows, font stacks and easing curves live. Light and dark |
| `app/src/lib/ui/` | The primitives as Svelte 5 components, each owning its styles; screens import them from `$lib/ui`. `app/src/app.css` imports the tokens and holds the base and the global utilities (`.num`, `.label`, `.title`, `.headline`, `.meta`, `.enter`) |
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

1. **Restraint.** One accent, one icon library, two typefaces, no gradients, no decoration. Variety comes from type.
2. **Precision.** A 4px grid, concentric radii, optical alignment, tabular numbers, hairlines instead of boxes.
3. **Speed you can feel.** High-frequency actions respond instantly; motion is spent only on rare moments.
4. **Native manners.** 13px UI, keyboard on every action, hover only on hover devices, a light window by default.
5. **Honesty in color.** Green only ever means points you gain; amber only means "left out, and why".

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
| Label | `.label` 11/16 caps +0.04em | Inter | Group labels |

- Weights: 400, 500 (labels, buttons, nav), 600 (priority-1 skills, emphasis). Never 700 in UI.
- Every number that changes is `tabular-nums` (`.num`).
- Serif is display-only: never for controls, never below 22px except inside the resume page.
- Headlines `text-wrap: balance`; paragraphs `text-wrap: pretty`.

### Space, size, radius

- 4px grid: `--space-0-5` (2) … `--space-16` (64). Row padding 12, column padding 20–24, between groups 24.
- Controls: `--control-sm` 24, `--control-md` 28 (default), `--control-lg` 36 (inputs, primary calls on empty states).
- Window: `--sidebar-width` 232, `--toolbar-height` 52, change column `--column-min` 420 to 640, desk takes the rest.
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
| A model call of unknown length (indeterminate progress) | A segment sweeps the track every `--dur-loop` 1600ms; reduced motion: an opacity pulse. Determinate progress fills from elapsed time, no animation |
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
| `Kbd` | children | inside buttons, in the key footer |
| `Segmented` | `options`, `value`, `label` | on, off, hover, disabled |
| `Input` / `Textarea` | `value`, `size` | placeholder, focus (accent border + 3px ring), disabled; `Textarea size="lg"` is the reading size for inline edits |
| `Checkbox` | `checked`, `label` | off, on, focus, disabled |
| `Chip` | `state` have/add/missing/not-mine, `p1` | press, hover, disabled |
| `Badge` | `tone` neutral/accent/gain/warn/danger, `dot` | |
| `Marked` | `text`, `marks` | inserted keywords in `<mark class="mark">`; the sheet's own highlight comes from the renderer |
| `ChangeRow` | `state` in/off/left-out/forced, `checked`, `selected`, `points`, snippets `text`/`actions`/`editor` | actions appear on hover, focus or selection; reason line ("Left out: …", "Kept on by you · …"); warning; "Your edit" owner; editing |
| `GroupHead` | `label`, `meta` | label + count + points |
| `ScoreHeader` | `now`, `projected`, `ceiling`, `components` | updates instantly; the parts in a popover on hover or focus |
| `Nav` / `NavItem` | `href`, `icon`, `current`, `meta`, `quiet` | hover, current, count; `quiet` for list entries (Recent) |
| `Card` | `pad` | |
| `Desk` | `pages` (SVG), `status` snippet, `zoom` | fits / over the page limit (badge); zoom 50–200, 100% fits the desk |
| `Toaster` + `toast()` | `tone` neutral/gain/warn/danger, `action` | enters with `.enter`, exits with blur and -12px; errors stay until dismissed |
| `Empty` | `title`, `body`, `action` snippet | title (serif), one sentence, one primary action with its key; staggered `.enter` |
| `Toolbar` | `crumbs` snippet, trailing children | drags the window; `<hr>` draws a divider; compact: room for the traffic lights and the sidebar toggle |
| `KeyFooter` | `keys` | sticky at the column's foot |
| `Menu` | `value`, `options` (`label`, `meta`), `label` | closed, open, active option, chosen (check), disabled; ↑↓ Home End ⏎ Esc |
| `Popover` | `open`, `anchor`, `placement`, `mode` auto/manual | anchored, flips when it would overflow; `--shadow-popover` |
| `Progress` | `label`, `expectedMs`, `oncancel` | determinate (fills to 90% by the expected time, then creeps), indeterminate |
| `Table` | `columns`, `rows`, `cell` snippet | sticky header, tabular numbers, hover wash, no zebra |

## Layout

```
┌──────────┬────────────────────────────────────────────────────────────────┐
│ ● ● ●    │ Jobs › Globex             [Master only|Bank-aware] │ SWE ▾ [Export E] │  toolbar 52
│          ├──────────────────────────────┬─────────────────────────────────┤
│ Jobs     │ Tailor first · Globex · …    │  ● Fits on 1 page        − 100% + │
│ Bank     │ Junior Software Engineer     │  ┌───────────────────────────┐   │
│ Masters  │ 53% › 78%        Ceiling 85% │  │                           │   │
│ Apps     │ ━━━━━━━━━━━━━━━━━━━━━━━━━━━━ │  │      resume page          │   │
│          │ SKILLS  chips…               │  │      (live SVG)           │   │
│ RECENT   │ WORK · …          3 · +15    │  │                           │   │
│ …        │ ☑ change row                 │  └───────────────────────────┘   │
│ Settings │ ↑↓ Select  Space Toggle  …   │              desk                │
└──────────┴──────────────────────────────┴─────────────────────────────────┘
  232 (vibrancy)   column 420–640                 rest
```

- **Window:** Tauri `titleBarStyle: "Overlay"`, transparent, with the traffic lights inset into the sidebar
  (`trafficLightPosition` 20, 28: centered on the 52px toolbar line). `app/src-tauri/src/window.rs` puts the macOS
  sidebar material (`window-vibrancy`, `NSVisualEffectMaterial::Sidebar`, dims when inactive) behind the webview and
  sets `data-vibrancy` on the root; only then is the page background transparent, so `--sidebar` shows the material
  and the main area paints `--paper`. Without it the sidebar is `--sidebar` over paper. Verified on macOS 26; the
  window's appearance follows the app theme so the material matches. Windows (Mica) is unverified until M7.
- **Dragging:** the `Toolbar` (`data-tauri-drag-region="deep"`: any empty part) and the sidebar's top 52px.
- **One primary action per screen**, in the toolbar's trailing edge, with its key.
- **Keys:** every row action has a key; the sticky key footer lists the screen's keys.
- **Compact (iOS, narrow windows under 960px):** the sidebar collapses to a sheet (the toolbar's sidebar button,
  Esc or a click outside closes it), the desk becomes a tab.

### Screen recipes

| Screen | Composition |
|---|---|
| Job (empty) | `Empty` centered on paper: "No postings yet", one sentence, **Paste from clipboard** `V` |
| Job → Review | As the specimen: job header (badge, meta, headline), score, skill chips, change groups, key footer; desk with page |
| Experience bank | Column: entries grouped by section with tag filter chips; selecting an entry opens its editor (bullets as rows with variants); desk previews the entry in the default template |
| Masters | Column: a master's sections as checkbox rows over the bank, drag to order; desk previews the resolved master |
| Applications | Full-width `Table` on `--surface`: tabular numbers, status as a `Badge`, row hover wash, no zebra stripes |
| Settings | Single column of cards (max 640): agents with signed-in `Badge`, models, export folder, theme `Segmented` (Light, Dark, System) |

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
