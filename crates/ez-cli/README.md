# ez

The command line over gliide's core and rendering, for working on them without the app. Run it from the repository
root:

```sh
cargo run -p ez-cli --bin ez -- <command> …
```

Outputs belong under the gitignored `local/`. Commands that call a model take `--agent claude|codex|antigravity`
(`claude` unless named) and `--model <name>` (the agent's default unless named); `score`, `rank` and `tailor` cache
model output in `--cache <dir>` (default `local/cache/`, keyed by posting text, prompt and model) and `--fresh`
ignores it. A posting is a link, a file, `-` for stdin, or the text itself. `ez <command> --help` has every flag.

## migrate

```sh
ez migrate <master.json> --out <dir>
```

A v1 single-resume `master.json` → `bank.json` + `masters/swe.json`.

## import

```sh
ez import <file> --out <dir> [--bank <f>] [--title "Graphic Design"] [--separate <id,…>] [--contact]
          [--agent …] [--model …]
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
ez score <posting> --bank <f> --master <f> [--agent …] [--model …] [--cache <dir>] [--fresh] [--json <f>]
```

Reads the posting, extracts its requirements with the agent, prints the composite score and every component.

## rank

```sh
ez rank <posting> --bank <f> --masters <dir> [--agent …] [--model …] [--cache <dir>] [--fresh]
```

Scores every master in the folder against the posting, best first.

## tailor

```sh
ez tailor <posting> --bank <f> --master <f> --out <dir> [--mode master-only|bank-aware] [--draft <f>]
          [--agent …] [--model …] [--extract-model <name>] [--cache <dir>] [--fresh] [--scale N]
```

Extract and Suggest, the change set, the fit check. Writes `tailored.json`, `resume.pdf`, highlighted `page-N.png`
and `changes.json` (state, points, flags per change); prints now → projected (ceiling), the page status and every
change.
