//! Company logos (ARCHITECTURE.md §7, Company logos), stored under the company's key and served to the UI at
//! `logo://localhost/<key>`: the ones you set (an image file, or the clipboard's), and for the companies the feed
//! shows, ones fetched from freehire, else from the company's own site. Kept as the company publishes it: SVG as is,
//! any other image scaled down to fit [`SIDE`] (never cropped, padded or recolored) and stored as PNG.

use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use chrono::{DateTime, TimeDelta, Utc};
use image::imageops::{self, FilterType};
use image::{DynamicImage, ImageFormat, ImageReader, Rgba32FImage};
use percent_encoding::percent_decode_str;
use resume_ingest::logos::{MAX_LOGO_BYTES, company_website, freehire_logo_url, icon_links};
use resume_ingest::{Fetcher, HttpFetcher};
use resume_store::{LogoFound, LogoLookup, LogoSource as Stored};
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
/// A fetched logo is looked up again after this many days.
const REFRESH_DAYS: i64 = 90;
/// A company whose lookup found no logo is tried again after this many days.
const RETRY_NONE_DAYS: i64 = 14;
/// A fetched raster image shorter than this on its longer side isn't a logo (a 16px favicon).
const MIN_SIDE: u32 = 32;

/// The company's key: its name through [`slug`], so `Northwind Traders, Inc.` and `northwind traders inc` share a logo.
pub fn key(company: &str) -> String {
    slug(company)
}

/// Every company with a logo: yours or fetched.
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

/// Removes `company`'s logo; a fetched one is recorded as removed, so it isn't fetched again.
pub fn remove_logo(app: &App, company: &str) -> Result<()> {
    let key = named(company)?;
    let store = lock(&app.store);
    if store.logo(&key)?.is_some_and(|l| l.source != Stored::Yours) {
        store.put_logo_lookup(&key, LogoFound::Removed)?;
    }
    store.remove_logo(&key)?;
    Ok(())
}

// ── Fetched logos ──

/// A company the feed shows, as its rows name it.
struct Company {
    key: String,
    name: String,
    /// freehire's company, for its website.
    slug: Option<String>,
}

/// A logo as stored: its bytes and media type.
type Image = (Vec<u8>, &'static str);

/// freehire (or the store) couldn't be reached: the run ends, and the next one retries.
struct Stop;

/// Holds `App.feed.fetching_logos` for a run, cleared however it ends.
struct Fetching(Option<Arc<App>>);

impl Fetching {
    /// Ends the run, unless it was started again meanwhile (then it goes once more); whether it ended.
    fn end(&mut self) -> bool {
        let Some(app) = &self.0 else {
            return true;
        };
        let mut run = lock(&app.feed);
        if std::mem::take(&mut run.logos_again) {
            return false;
        }
        run.fetching_logos = false;
        drop(run);
        self.0 = None;
        true
    }
}

impl Drop for Fetching {
    fn drop(&mut self) {
        if let Some(app) = self.0.take() {
            let mut run = lock(&app.feed);
            run.fetching_logos = false;
            run.logos_again = false;
        }
    }
}

/// Fetches logos for the companies the feed shows, one at a time: freehire's logo for the name, else the company's
/// own site's icon (its website from freehire's company page). A logo you set is never replaced. Each lookup is
/// recorded: a company is looked up again after 90 days (14 when nothing was found), and never after you removed
/// its fetched logo. Every logo stored sends the `feed` event. One run at a time: started while one runs, that run
/// goes once more. Failing to reach freehire ends the run quietly; the next pull or check starts another. Postings
/// you paste are never looked up: no third party learns which companies you look at.
pub async fn fetch_logos(app: Arc<App>) {
    {
        let mut run = lock(&app.feed);
        if run.fetching_logos {
            run.logos_again = true;
            return;
        }
        run.fetching_logos = true;
    }
    let mut fetching = Fetching(Some(Arc::clone(&app)));
    let fetcher = HttpFetcher::new();
    loop {
        let Ok(companies) = due(&app) else {
            return;
        };
        for company in &companies {
            match fetch_one(&app, &fetcher, company).await {
                Ok(true) => crate::feed::changed(&app),
                Ok(false) => {}
                Err(Stop) => return,
            }
        }
        if fetching.end() {
            return;
        }
    }
}

/// The companies the feed shows that are due a lookup, once each: not yours, and not looked up recently.
fn due(app: &App) -> Result<Vec<Company>> {
    let mut companies: Vec<Company> = Vec::new();
    let mut at: HashMap<String, usize> = HashMap::new();
    for (name, slug) in crate::feed::shown_companies(app)? {
        let key = key(&name);
        if key.is_empty() {
            continue;
        }
        if let Some(&i) = at.get(&key) {
            let known = &mut companies[i];
            if known.slug.is_none() {
                known.slug = slug;
            }
            continue;
        }
        at.insert(key.clone(), companies.len());
        companies.push(Company { key, name, slug });
    }
    let now = Utc::now();
    let store = lock(&app.store);
    let mut due = Vec::new();
    for c in companies {
        if store.logo_lookup(&c.key)?.is_some_and(|l| fresh(&l, now))
            || store.logo(&c.key)?.is_some_and(|l| l.source == Stored::Yours)
        {
            continue;
        }
        due.push(c);
    }
    Ok(due)
}

/// A lookup too recent to repeat: under 90 days after one that found a logo, 14 after one that found none; one whose
/// logo you removed, for good.
fn fresh(l: &LogoLookup, now: DateTime<Utc>) -> bool {
    let days = match l.found {
        LogoFound::Removed => return true,
        LogoFound::Nothing => RETRY_NONE_DAYS,
        LogoFound::Freehire | LogoFound::Site => REFRESH_DAYS,
    };
    DateTime::parse_from_rfc3339(&l.tried_at)
        .is_ok_and(|t| now - t.with_timezone(&Utc) < TimeDelta::days(days))
}

/// Looks one company's logo up, stores it unless you set one, and records the lookup; whether a logo was stored.
async fn fetch_one(
    app: &App,
    fetcher: &HttpFetcher,
    c: &Company,
) -> std::result::Result<bool, Stop> {
    let mut found = freehire_logo(fetcher, &c.name)
        .await?
        .map(|logo| (logo, Stored::Freehire));
    if found.is_none()
        && let Some(slug) = &c.slug
    {
        found = site_logo(fetcher, slug).await?.map(|logo| (logo, Stored::Site));
    }
    let store = lock(&app.store);
    let (stored, lookup) = match &found {
        Some(((bytes, media_type), source)) => (
            store
                .put_fetched_logo(&c.key, bytes, media_type, *source)
                .map_err(|_| Stop)?,
            if *source == Stored::Site {
                LogoFound::Site
            } else {
                LogoFound::Freehire
            },
        ),
        None => (false, LogoFound::Nothing),
    };
    store.put_logo_lookup(&c.key, lookup).map_err(|_| Stop)?;
    Ok(stored)
}

/// freehire's logo for the company's name.
async fn freehire_logo(
    fetcher: &HttpFetcher,
    name: &str,
) -> std::result::Result<Option<Image>, Stop> {
    match fetcher
        .get_image(&freehire_logo_url(name), MAX_LOGO_BYTES)
        .await
    {
        Ok(Some((bytes, _))) => Ok(as_logo(bytes).await),
        Ok(None) => Ok(None),
        Err(_) => Err(Stop),
    }
}

/// The first of the company's own site's icons that is a logo. Only freehire's failures stop the run: a site that
/// doesn't answer just has no icon found.
async fn site_logo(fetcher: &HttpFetcher, slug: &str) -> std::result::Result<Option<Image>, Stop> {
    let Some(website) = company_website(fetcher, slug).await.map_err(|_| Stop)? else {
        return Ok(None);
    };
    let (html, page) = match fetcher.get(&website).await {
        Ok(r) if (200..300).contains(&r.status) => (r.body, r.url),
        Ok(r) => (String::new(), r.url),
        Err(_) => return Ok(None),
    };
    for url in icon_links(&html, &page) {
        if let Ok(Some((bytes, _))) = fetcher.get_image(&url, MAX_LOGO_BYTES).await
            && let Some(logo) = as_logo(bytes).await
        {
            return Ok(Some(logo));
        }
    }
    Ok(None)
}

/// A fetched image as stored, when it's a logo: SVG as is; a raster at least [`MIN_SIDE`] on its longer side, fit
/// to [`SIDE`] as PNG. Decoded off the async runtime.
async fn as_logo(bytes: Vec<u8>) -> Option<Image> {
    tokio::task::spawn_blocking(move || {
        if is_svg(&bytes) {
            return encode(bytes).ok();
        }
        let image = ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .ok()?
            .decode()
            .ok()?;
        if image.width().max(image.height()) < MIN_SIDE {
            return None;
        }
        to_png(image).ok()
    })
    .await
    .ok()
    .flatten()
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
