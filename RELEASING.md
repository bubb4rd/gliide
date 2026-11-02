# Releasing (maintainers)

Official releases are signed with a Developer ID and notarized, and carry a bundle signed with the update key, which
installed copies take. Contributors never need any of this; a local `pnpm tauri build` from `app/` is unsigned, which
is fine on the machine that built it.

1. Bump the version in `app/package.json`, `app/src-tauri/Cargo.toml` and `app/src-tauri/tauri.conf.json` (the
   script refuses to run while they differ); commit, tag `vX.Y.Z` and push both.
2. `pnpm release:mac` from `app/`, with `RELEASE_NOTES` set to show beside the update in Settings → About. It needs
   a Developer ID Application certificate, an App Store Connect API key and the update key
   (`~/.tauri/gliide-updater.key` and its password); `app/scripts/release-macos.sh` lists what it reads. It writes
   the `.dmg`, `gliide.app.tar.gz` (with its `.sig`) and `latest.json`.
3. Attach all three to the GitHub release `vX.Y.Z`, marked latest (the script prints the `gh release create`
   line). Installed copies read `releases/latest/download/latest.json`, so a release without it updates no one.

## The update key

Made once with `pnpm tauri signer generate -w ~/.tauri/gliide-updater.key`; its public half is
`plugins.updater.pubkey` in `app/src-tauri/tauri.conf.json`. Keep it with its password somewhere safe: without it,
installed copies can never take another update; anyone holding both can ship one to them.
