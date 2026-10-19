# Backdrops

The pictures `Backdrop` (`$lib/ui`) shows behind the top of the Overview, chosen in Settings → Appearance. Bundled,
never fetched.

| File | What it is |
|---|---|
| `meadow.webp` | Meadow: a painted hillside under a morning sky, two foxes, a flock of birds. Supplied by the owner (2026-10-14), 1672×940, re-encoded from PNG (`cwebp -q 82 -m 6 -sharp_yuv`) |
| `tokyo.webp` | Tokyo: a painted Tokyo street on a rainy night, neon on wet asphalt, two people under umbrellas. Supplied by the owner (2026-10-15), 1672×940, downscaled from a 2848×1600 PNG and re-encoded (`cwebp -q 82 -m 6 -sharp_yuv -resize 1672 0`) |
| `sakura.webp` | Sakura: a woodblock-style river at dusk under cherry blossoms, paper lanterns, fireworks over pink clouds. Supplied by the owner (2026-10-16), 1672×940, downscaled from a 2848×1600 PNG and re-encoded (`cwebp -q 82 -m 6 -sharp_yuv -resize 1672 0`) |
| `fade.png` | The mask that fades a backdrop out to paper: 128×1120, shown at 64×560 CSS pixels (one mask pixel per Retina pixel), repeated across. Opaque for the top 64px, then a smootherstep from opaque to clear over 496px. Its alpha is dithered with a 64×64 blue-noise threshold map (void-and-cluster, Gaussian σ 1.9): `floor(v × 255 + threshold)`, so the fade never bands |
| `screen.svg` | The dot screen over a backdrop: one 1px square in a 3px tile, drawn in `--paper` |

A new picture: add it here, to `BACKDROPS` in `Backdrop.svelte`, and to the `Backdrop` enum in
`app/src-tauri/src/dto.rs` (then `cargo test -p gliide` regenerates the bindings).
