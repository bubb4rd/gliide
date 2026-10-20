# Contributing to gliide

Thanks for helping. Issues and pull requests are welcome on [bubb4rd/gliide](https://github.com/bubb4rd/gliide).

## Set up

Prerequisites are in the [README](README.md#prerequisites). Then:

```sh
cd app && pnpm install      # front-end dependencies
pnpm tauri dev              # run the app (from app/)
```

## Before you open a pull request

Run what CI runs (`.github/workflows/ci.yml`):

```sh
cd app && pnpm check && pnpm build && cd ..   # Svelte/TypeScript check; build/ is embedded by the Tauri crate
cargo test --workspace --locked              # every crate, including the app's (regenerates app/src/lib/api/bindings/)
node design/lint.mjs                         # design-system lint, must exit 0
```

After changing `app/src-tauri/src/dto.rs`, commit the regenerated TypeScript bindings in
`app/src/lib/api/bindings/`.

## Where the rules are

- [`ARCHITECTURE.md`](ARCHITECTURE.md): how the pieces fit, and what each crate owns. Update it when behavior changes.
- [`design/DESIGN.md`](design/DESIGN.md): binding for any UI change: tokens from `design/tokens.css` only,
  components from `app/src/lib/ui/`, a new primitive shown in the `/specimen` route first. Update it with the change.
- [`AGENTS.md`](AGENTS.md): the same rules, written for coding agents.

Never commit real resumes, postings or personal data: test data goes in `fixtures/` with fictional people
(see `fixtures/banks/jane-doe.v1.json`), and your own data under the gitignored `local/`.

## License and sign-off

gliide is licensed under the [GNU Affero General Public License v3.0 only](LICENSE). By contributing, you agree that
your contribution is licensed under `AGPL-3.0-only` as well.

Every commit must carry a [Developer Certificate of Origin](https://developercertificate.org) sign-off, certifying
that you wrote the change or otherwise have the right to submit it under this license:

```
Signed-off-by: Jane Doe <jane.doe@example.com>
```

`git commit -s` adds it with your configured name and email.
