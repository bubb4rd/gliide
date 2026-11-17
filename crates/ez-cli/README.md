# ez

The command line over gliide's core and rendering, for working on them without the app. Run it from the repository
root:

```sh
cargo run -p ez-cli --bin ez -- <command> …
```

Outputs belong under the gitignored `local/`. Commands that call a model take a route, `agent:model` (`claude:sonnet`,
`codex:gpt-6-luna`, `antigravity:gemini-3.8-flash-low`), or `agent` alone for its default model: `--read` for
Extract, `--tailor` for Suggest, `--route` for Import, each `claude` unless named. `score`, `rank` and `tailor` cache
model output in `--cache <dir>` (default `local/cache/`, keyed by posting text, prompt and model, as the app's cache
is) and `--fresh` ignores it. A posting is a link, a file, `-` for stdin, or the text itself. `ez <command> --help`
has every flag.

## migrate

```sh
ez migrate <master.json> --out <dir>
```

A v1 single-resume `master.json` → `bank.json` + `masters/swe.json`.

## import

```sh
ez import <file> --out <dir> [--bank <f>] [--title "Graphic Design"] [--separate <id,…>] [--contact]
          [--route <agent:model>]
```

A resume (`.pdf`, `.docx`, `.md`, `.txt`; or a v1 JSON such as an earlier run's `imported.json`, which skips the
model) copied with the agent and merged into the bank. Writes `imported.json`, `bank.json`, `masters/<id>.json` and
prints where each part went.

## render

```sh
ez render --bank <f> --master <f> --out <dir> [--highlight] [--system-fonts] [--scale N] [--template <dir>]
          [--bench N]
```

Writes `resume.pdf`, `page-N.svg`, `page-N.png` and `metrics.json`. `--template` is a folder with `resume.typ` and
`manifest.json`; without it, the master's built-in template.

## schema

```sh
ez schema --out schemas/
```

Regenerates `schemas/bank.schema.json`, `schemas/master.schema.json` and `schemas/story.schema.json`.

## score

```sh
ez score <posting> --bank <f> --master <f> [--read <agent:model>] [--cache <dir>] [--fresh] [--json <f>]
```

Reads the posting, extracts its requirements on the Read route, prints the composite score and every component.

## rank

```sh
ez rank <posting> --bank <f> --masters <dir> [--read <agent:model>] [--cache <dir>] [--fresh]
```

Scores every master in the folder against the posting, best first.

## tailor

```sh
ez tailor <posting> --bank <f> --master <f> --out <dir> [--mode master-only|bank-aware] [--draft <f>]
          [--read <agent:model>] [--tailor <agent:model>] [--cache <dir>] [--fresh] [--scale N]
```

Extract and Suggest, the change set, the fit check. Writes `tailored.json`, `resume.pdf`, highlighted `page-N.png`
and `changes.json` (state, points, flags per change); prints now → projected (ceiling), the page status and every
change.

## eval

```sh
ez eval read   --routes claude:sonnet,codex:gpt-6-luna [--runs 3] [--only <id,…>] [--out <dir>]
ez eval tailor --routes … [--runs 3] [--mode bank-aware|master-only] [--only …] [--out …]
ez eval write  --routes … [--runs 1] [--only …] [--out …]
ez eval coach  --routes … [--runs 1] [--only …] [--out …]
ez eval report [--out <dir>]
```

Runs one role's calls on every fixture posting (`fixtures/postings/`), for each route, one call at a time, and
compares the routes. Read: Extract, scored against `fixtures/requirements/gold/` (recall, precision, exact fields,
score drift). Tailor: Suggest with gold requirements through `changes_of` and `tailor` (grounding, lift, code
checks); bank-aware tailors `fixtures/banks/jane-doe.swe-trim.master.json`, master-only the full `swe` master. Write:
a cover letter per posting. Coach: a technical round's prep per posting and Dig's questions per bank entry
(`dig-<entry>`). The bank is `fixtures/banks/jane-doe.v1.json` migrated in memory.

Each run is recorded as `<out>/<role>/<agent>-<model>/<posting>/<n>.json` (default out `local/eval/<YYYY-MM-DD>`);
a recorded run is skipped, so a stopped eval resumes. `report.md` (metrics × routes with the pass bars marked, then
per-posting medians and spread) and `routes.json` (the numbers) are rebuilt from the recorded runs after every role
command and by `ez eval report`. Tailor, Write and Coach also write `blind.md` with each route's output under shuffled
labels, and `key.json`: mark `[x]` in `blind.md` and the report adds each route's keep or preference rate. A sheet
with marks is never rewritten. Quit the app while an eval runs: its feed checks share the agents' plans.
