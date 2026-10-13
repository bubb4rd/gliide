# easyshmeezy

From a job posting to a tailored, one-page resume: score the fit against one of your master resumes, review honest
edits (rewrites, skills, entries from your experience bank) with a click each, and export a PDF in your design.
macOS first; Windows and iOS from the same Rust core.

Status: planning. Nothing builds yet.

| Document | What it covers |
|---|---|
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Stack, data model, model access through your own subscription (ACP, `claude -p`), scoring, change engine, rendering, roadmap |
| [`design/DESIGN.md`](design/DESIGN.md) | The "Paper & Ink" design system: tokens, components, motion, screen recipes |
| [`AGENTS.md`](AGENTS.md) | Rules for coding agents working in this repo |

Design specimen: `cd design && python3 -m http.server`, then open `http://localhost:8000/specimen.html`.
Design lint: `node design/lint.mjs`.
