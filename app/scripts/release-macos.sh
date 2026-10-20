#!/bin/sh
# Signed, notarized macOS release: Tauri signs the app with your Developer ID (hardened runtime), notarizes and staples
# it; this script then notarizes and staples the DMG it ships in and checks both the way Gatekeeper will.
#
# Needs, once:
# - A "Developer ID Application" identity in the login keychain (`security find-identity -v -p codesigning`).
#   With more than one, pick with APPLE_SIGNING_IDENTITY.
# - An App Store Connect API key: APPLE_API_ISSUER (Issuer ID), APPLE_API_KEY (Key ID), and its .p8 at
#   ~/.appstoreconnect/private_keys/AuthKey_<Key ID>.p8, or APPLE_API_KEY_PATH.
#
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

echo "Signing as: $APPLE_SIGNING_IDENTITY"
started=$(mktemp)
trap 'rm -f "$started"' EXIT
pnpm tauri build "$@"

# Bundles land in the workspace's target/ (target/<triple>/release/… with --target).
built=$(find ../target -path '*/release/bundle/*' \( -name '*.app' -o -name '*.dmg' \) -newer "$started" -prune)
[ -n "$built" ] || fail "no .app or .dmg built under ../target"

printf '%s\n' "$built" | while IFS= read -r path; do
	case $path in
	*.app)
		codesign --verify --deep --strict "$path"
		xcrun stapler validate "$path"
		spctl --assess --type execute --verbose=2 "$path"
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
echo "Ready to share:"
printf '%s\n' "$built"
