//! The fonts a world can see: the bundled ones always, the system's on request.

use typst::foundations::Bytes;
use typst::text::{Font, FontInfo};
use typst_kit::fonts::FontStore;

/// Liberation Serif (SIL OFL 1.1, `templates/fonts/LICENSE-LiberationSerif`), metric-compatible with Times New Roman.
const BUNDLED: [&[u8]; 4] = [
    include_bytes!("../../../templates/fonts/LiberationSerif-Regular.ttf"),
    include_bytes!("../../../templates/fonts/LiberationSerif-Bold.ttf"),
    include_bytes!("../../../templates/fonts/LiberationSerif-Italic.ttf"),
    include_bytes!("../../../templates/fonts/LiberationSerif-BoldItalic.ttf"),
];

/// Which fonts a [`Renderer`](crate::Renderer) loads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FontOptions {
    /// Also load the system's fonts and Typst's embedded ones, in the `typst` CLI's order (system, embedded, then the
    /// bundled fonts as if passed with `--font-path`). Off, only the bundled fonts load, so line breaks are the same on
    /// every machine.
    pub system: bool,
}

pub(crate) fn load(options: FontOptions) -> FontStore {
    let mut store = FontStore::new();
    if options.system {
        store.extend(typst_kit::fonts::system());
        store.extend(typst_kit::fonts::embedded());
    }
    store.extend(bundled());
    store
}

fn bundled() -> impl Iterator<Item = (Font, FontInfo)> {
    BUNDLED.into_iter().flat_map(|data| {
        Font::iter(Bytes::new(data)).map(|font| {
            let info = font.info().clone();
            (font, info)
        })
    })
}
