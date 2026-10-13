//! Company logos you set (ARCHITECTURE.md §7, Company logos): an image file, or the clipboard's, stored under the
//! company's key and served to the UI at `logo://localhost/<key>`. Kept as the company publishes it: SVG as is, any
//! other image scaled down to fit [`SIDE`] (never cropped, padded or recolored) and stored as PNG.

use std::io::Cursor;
use std::path::Path;

use image::imageops::{self, FilterType};
use image::{DynamicImage, ImageFormat, ImageReader, Rgba32FImage};
use percent_encoding::percent_decode_str;
use tauri::http::{Response, StatusCode, header};

use crate::dto::{LogoRow, LogoSource};
use crate::error::{AppError, Result};
use crate::export::slug;
use crate::state::{App, lock};

/// The URI scheme the UI loads logos from.
pub const SCHEME: &str = "logo";
/// The longest side a raster logo is stored at: the largest mark (32px) at 4x.
const SIDE: u32 = 128;
/// The largest file read: room for a full-size photo or screenshot of the logo.
const MAX_FILE: u64 = 20 * 1024 * 1024;
/// The largest SVG kept (stored as is).
const MAX_SVG: usize = 512 * 1024;

/// The company's key: its name through [`slug`], so `Northwind Traders, Inc.` and `northwind traders inc` share a logo.
pub fn key(company: &str) -> String {
    slug(company)
}

/// Every company with a logo you set.
pub fn logos(app: &App) -> Result<Vec<LogoRow>> {
    Ok(lock(&app.store)
        .logos()?
        .into_iter()
        .map(|(key, version)| LogoRow { key, version })
        .collect())
}

/// Sets `company`'s logo from `source`, replacing any.
pub fn set_logo(app: &App, company: &str, source: LogoSource) -> Result<LogoRow> {
    let key = named(company)?;
    let (bytes, media_type) = match source {
        LogoSource::File { path } => from_file(Path::new(&path))?,
        LogoSource::Clipboard => from_clipboard()?,
    };
    let version = lock(&app.store).put_logo(&key, &bytes, media_type)?;
    Ok(LogoRow { key, version })
}

pub fn remove_logo(app: &App, company: &str) -> Result<()> {
    lock(&app.store).remove_logo(&named(company)?)?;
    Ok(())
}

/// The answer to a `logo://` request for `path` (`/<key>`, percent-encoded): the image, cached for good (its URL
/// changes with every new logo), else 404.
pub fn respond(app: &App, path: &str) -> Response<Vec<u8>> {
    let key = percent_decode_str(path.trim_start_matches('/')).decode_utf8_lossy();
    let logo = lock(&app.store).logo(&key);
    let response = match logo {
        Ok(Some(logo)) => Response::builder()
            .header(header::CONTENT_TYPE, logo.media_type)
            .header(header::CACHE_CONTROL, "max-age=31536000, immutable")
            .body(logo.bytes),
        Ok(None) => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Vec::new()),
        Err(_) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Vec::new()),
    };
    response.unwrap_or_default()
}

fn named(company: &str) -> Result<String> {
    let key = key(company);
    if key.is_empty() {
        return Err(AppError::missing(
            "This posting has no company name to set a logo for",
        ));
    }
    Ok(key)
}

/// The image file at `path`, as stored.
fn from_file(path: &Path) -> Result<(Vec<u8>, &'static str)> {
    if std::fs::metadata(path)?.len() > MAX_FILE {
        return Err(AppError::other("That image is over 20 MB: use a smaller one"));
    }
    encode(std::fs::read(path)?)
}

/// A copied image file (as Finder copies one), else the copied picture.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn from_clipboard() -> Result<(Vec<u8>, &'static str)> {
    let none = || AppError::missing("The clipboard has no image. Copy one, or drop its file here");
    let mut clipboard = arboard::Clipboard::new().map_err(|_| none())?;
    if let Some(path) = clipboard.get().file_list().ok().and_then(|f| f.into_iter().next()) {
        return from_file(&path);
    }
    let copied = clipboard.get_image().map_err(|_| none())?;
    let rgba = image::RgbaImage::from_raw(
        u32::try_from(copied.width).map_err(|_| unreadable())?,
        u32::try_from(copied.height).map_err(|_| unreadable())?,
        copied.bytes.into_owned(),
    )
    .ok_or_else(unreadable)?;
    to_png(DynamicImage::ImageRgba8(rgba))
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn from_clipboard() -> Result<(Vec<u8>, &'static str)> {
    Err(AppError::other("Pasting a logo works on the Mac app only; choose its file instead"))
}

/// An image file's bytes as stored: SVG as is; anything else decoded and stored as PNG.
fn encode(bytes: Vec<u8>) -> Result<(Vec<u8>, &'static str)> {
    if is_svg(&bytes) {
        if bytes.len() > MAX_SVG {
            return Err(AppError::other("That SVG is over 512 KB: use a smaller one"));
        }
        return Ok((bytes, "image/svg+xml"));
    }
    let image = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()?
        .decode()
        .map_err(|_| unreadable())?;
    to_png(image)
}

/// `image` as PNG, scaled down to fit [`SIDE`] when larger. Scales with premultiplied alpha, so the transparent
/// pixels' color never bleeds into the logo's edges.
fn to_png(image: DynamicImage) -> Result<(Vec<u8>, &'static str)> {
    let image = if image.width() > SIDE || image.height() > SIDE {
        let mut pixels = image.into_rgba32f();
        for p in pixels.pixels_mut() {
            let a = p[3];
            p[0] *= a;
            p[1] *= a;
            p[2] *= a;
        }
        let (w, h) = fit(pixels.width(), pixels.height());
        let mut scaled: Rgba32FImage = imageops::resize(&pixels, w, h, FilterType::Lanczos3);
        for p in scaled.pixels_mut() {
            let a = p[3].clamp(0.0, 1.0);
            p[3] = a;
            for c in &mut p.0[..3] {
                *c = if a > 0.0 { (*c / a).clamp(0.0, 1.0) } else { 0.0 };
            }
        }
        DynamicImage::ImageRgba8(DynamicImage::ImageRgba32F(scaled).into_rgba8())
    } else {
        image
    };
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .map_err(|_| unreadable())?;
    Ok((png, "image/png"))
}

/// `w`×`h` scaled to fit [`SIDE`]×[`SIDE`], aspect kept, at least 1px a side.
fn fit(w: u32, h: u32) -> (u32, u32) {
    let scale = f64::from(SIDE) / f64::from(w.max(h));
    let side = |n: u32| ((f64::from(n) * scale).round() as u32).max(1);
    (side(w), side(h))
}

/// Text that opens as an SVG document (after any BOM, whitespace, XML declaration or comment).
fn is_svg(bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
    let text = head.trim_start_matches('\u{feff}').trim_start();
    text.starts_with('<') && !text.starts_with("<html") && text.contains("<svg")
}

fn unreadable() -> AppError {
    AppError::other("That file isn't an image gliide can read: use a PNG, JPEG, GIF, WebP, TIFF, BMP, ICO or SVG")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wide logo, larger than [`SIDE`], on a transparent background whose hidden color is black.
    fn wide_logo() -> Vec<u8> {
        let mut img = image::RgbaImage::from_pixel(600, 200, image::Rgba([0, 0, 0, 0]));
        for x in 200..400 {
            for y in 50..150 {
                img.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
            }
        }
        let mut png = Vec::new();
        img.write_to(&mut Cursor::new(&mut png), ImageFormat::Png).unwrap();
        png
    }

    #[test]
    fn large_images_fit_without_cropping_or_dark_edges() {
        let (png, media_type) = encode(wide_logo()).unwrap();
        assert_eq!(media_type, "image/png");
        let stored = image::load_from_memory(&png).unwrap().into_rgba8();
        assert_eq!(stored.dimensions(), (128, 43));
        // Every visible pixel stays white: the transparent black never darkens the edge.
        for p in stored.pixels().filter(|p| p[3] > 0) {
            assert!(p[0] > 250 && p[1] > 250 && p[2] > 250, "{p:?}");
        }
    }

    #[test]
    fn svg_is_kept_as_is_and_other_files_are_refused() {
        let svg = b"\xef\xbb\xbf<?xml version=\"1.0\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\"/>".to_vec();
        assert_eq!(encode(svg.clone()).unwrap(), (svg, "image/svg+xml"));
        assert!(encode(b"<html><body><svg/></body></html>".to_vec()).is_err());
        assert!(encode(b"%PDF-1.7".to_vec()).is_err());
    }
}
