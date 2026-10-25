#!/bin/sh
# Signed, notarized macOS release: Tauri signs the app with your Developer ID (hardened runtime), notarizes and staples
# it, and packs it again as the updater's bundle (gliide.app.tar.gz) signed with the update key; this script then
# notarizes and staples the DMG it ships in, checks both the way Gatekeeper will, and writes latest.json, which
# installed copies read to find the release (tauri.conf.json → plugins.updater).
#
# Needs, once:
# - A "Developer ID Application" identity in the login keychain (`security find-identity -v -p codesigning`).
#   With more than one, pick with APPLE_SIGNING_IDENTITY.
# - An App Store Connect API key: APPLE_API_ISSUER (Issuer ID), APPLE_API_KEY (Key ID), and its .p8 at
#   ~/.appstoreconnect/private_keys/AuthKey_<Key ID>.p8, or APPLE_API_KEY_PATH.
# - The update key: ~/.tauri/gliide-updater.key, or TAURI_SIGNING_PRIVATE_KEY (its path or contents); its password
#   in TAURI_SIGNING_PRIVATE_KEY_PASSWORD, else asked for. Its public half is tauri.conf.json's plugins.updater.pubkey.
#   Lose the key or its password and installed copies can never take another update.
#
# RELEASE_NOTES (optional) becomes latest.json's notes, which Settings → About shows beside the update.
# Arguments go to `tauri build`, e.g. `pnpm release:mac --target universal-apple-darwin`.
set -eu
cd "$(dirname "$0")/.."

fail() {
	echo "release-macos: $*" >&2
	exit 1
}

if [ -z "${APPLE_SIGNING_IDENTITY:-}" ]; then
	ids=$(security find-identity -v -p codesigning | sed -n 's/^.*"\(Developer ID Application: .*\)"$/\1/p' | sort -u)
	case $(printf '%s' "$ids" | grep -c .) in
	0) fail 'no "Developer ID Application" identity in the keychain; create one in Xcode → Settings → Accounts → Manage Certificates' ;;
	1) APPLE_SIGNING_IDENTITY=$ids ;;
	*) fail "several Developer ID identities; set APPLE_SIGNING_IDENTITY to one of:
$ids" ;;
	esac
fi
[ -n "${APPLE_API_ISSUER:-}" ] || fail "set APPLE_API_ISSUER to the Issuer ID from App Store Connect → Users and Access → Integrations"
[ -n "${APPLE_API_KEY:-}" ] || fail "set APPLE_API_KEY to the API key's Key ID"
APPLE_API_KEY_PATH=${APPLE_API_KEY_PATH:-$HOME/.appstoreconnect/private_keys/AuthKey_$APPLE_API_KEY.p8}
[ -f "$APPLE_API_KEY_PATH" ] || fail "no API key file at $APPLE_API_KEY_PATH"
export APPLE_SIGNING_IDENTITY APPLE_API_ISSUER APPLE_API_KEY APPLE_API_KEY_PATH

TAURI_SIGNING_PRIVATE_KEY=${TAURI_SIGNING_PRIVATE_KEY:-$HOME/.tauri/gliide-updater.key}
case $TAURI_SIGNING_PRIVATE_KEY in
/*) [ -f "$TAURI_SIGNING_PRIVATE_KEY" ] || fail "no update key at $TAURI_SIGNING_PRIVATE_KEY (see the README's Releasing)" ;;
esac
if [ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD+set}" ]; then
	[ -t 0 ] || fail "set TAURI_SIGNING_PRIVATE_KEY_PASSWORD (no terminal to ask in)"
	printf 'Update key password: ' >&2
	stty -echo
	IFS= read -r TAURI_SIGNING_PRIVATE_KEY_PASSWORD || true
	stty echo
	echo >&2
fi
export TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PRIVATE_KEY_PASSWORD

version=$(node -p "require('./src-tauri/tauri.conf.json').version")
[ "$version" = "$(node -p "require('./package.json').version")" ] || fail "package.json's version isn't $version (tauri.conf.json's)"
grep -q "^version = \"$version\"$" src-tauri/Cargo.toml || fail "src-tauri/Cargo.toml's version isn't $version (tauri.conf.json's)"

echo "Signing as: $APPLE_SIGNING_IDENTITY"
echo "Version: $version"
started=$(mktemp)
unpacked=$(mktemp -d)
trap 'rm -rf "$started" "$unpacked"' EXIT
pnpm tauri build "$@"

# Bundles land in the workspace's target/ (target/<triple>/release/… with --target). bundle_dmg.sh leaves its scratch
# image (rw.<pid>.<name>.dmg) beside the app; it's no bundle.
built=$(find ../target -path '*/release/bundle/*' \( -name '*.app' -o -name '*.dmg' -o -name '*.app.tar.gz' \) \
	! -name 'rw.*.dmg' -newer "$started" -prune)
[ -n "$built" ] || fail "no .app or .dmg built under ../target"
bundle=$(printf '%s\n' "$built" | grep '\.app\.tar\.gz$' | head -n 1)
[ -n "$bundle" ] || fail "no updater bundle (.app.tar.gz) built; is bundle.createUpdaterArtifacts on?"
[ -f "$bundle.sig" ] || fail "no signature beside $bundle"

printf '%s\n' "$built" | while IFS= read -r path; do
	case $path in
	*.app)
		codesign --verify --deep --strict "$path"
		xcrun stapler validate "$path"
		spctl --assess --type execute --verbose=2 "$path"
		;;
	*.app.tar.gz)
		# What an installed copy will unpack: the same notarized, stapled app.
		tar -xzf "$path" -C "$unpacked"
		for app in "$unpacked"/*.app; do
			codesign --verify --deep --strict "$app"
			xcrun stapler validate "$app"
		done
		;;
	*.dmg)
		codesign --verify "$path" 2>/dev/null || codesign --sign "$APPLE_SIGNING_IDENTITY" --timestamp "$path"
		xcrun notarytool submit "$path" --key "$APPLE_API_KEY_PATH" --key-id "$APPLE_API_KEY" \
			--issuer "$APPLE_API_ISSUER" --wait
		xcrun stapler staple "$path"
		spctl --assess --type open --context context:primary-signature --verbose=2 "$path"
		;;
	esac
done

# The platforms the bundle runs on: a universal build serves both.
case $bundle in
*universal-apple-darwin*) platforms="darwin-aarch64 darwin-x86_64" ;;
*x86_64-apple-darwin*) platforms="darwin-x86_64" ;;
*aarch64-apple-darwin*) platforms="darwin-aarch64" ;;
*) [ "$(uname -m)" = arm64 ] && platforms="darwin-aarch64" || platforms="darwin-x86_64" ;;
esac
latest=$(dirname "$bundle")/latest.json
url="https://github.com/bubb4rd/gliide/releases/download/v$version/$(basename "$bundle")"
VERSION=$version URL=$url SIG=$(cat "$bundle.sig") PLATFORMS=$platforms NOTES=${RELEASE_NOTES:-} node -e '
	const { VERSION, URL, SIG, PLATFORMS, NOTES } = process.env;
	const platforms = Object.fromEntries(PLATFORMS.split(" ").map((p) => [p, { signature: SIG, url: URL }]));
	const latest = { version: VERSION, notes: NOTES, pub_date: new Date().toISOString(), platforms };
	require("fs").writeFileSync(process.argv[1], JSON.stringify(latest, null, 2) + "\n");
' "$latest"

dmg=$(printf '%s\n' "$built" | grep '\.dmg$' | head -n 1)
echo "Ready to share: attach all three to the GitHub release v$version, marked latest:"
printf '  %s\n' "$dmg" "$bundle" "$latest"
echo "  gh release create v$version --title 'gliide $version' --notes \"\$RELEASE_NOTES\" \\"
echo "    '$dmg' '$bundle' '$latest'"
