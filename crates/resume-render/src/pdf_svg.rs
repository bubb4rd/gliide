//! A PDF file's pages as SVG, for previewing PDFs the app didn't compile (the export folder's files).
//!
//! `hayro-svg` 0.7 draws text as `<use>`s of glyph outlines, but bakes each glyph's position on its line into the
//! outline, so a page of text defines every glyph it draws (about 4 MB for a resume). [`share_glyphs`] moves that
//! offset onto the `<use>` and keeps one outline per shape (a tenth of the size).
//!
//! It also names each page's definitions by their order (`g0`, `c0`…), so two pages inlined into one HTML document
//! would draw each other's glyphs: every id and reference on page `n` gets the prefix `pdf<n>-`.

use std::collections::HashMap;
use std::fmt::Write;

use hayro_svg::hayro_interpret::InterpreterSettings;
use hayro_svg::hayro_syntax::Pdf;
use hayro_svg::{RenderCache, SvgRenderSettings};

/// Where an id is defined or referenced in `hayro-svg`'s output.
const ID_SITES: [&str; 3] = [" id=\"", "href=\"#", "url(#"];
const GLYPH_DEFS: &str = "<defs id=\"outline-glyph\">";
const GLYPH_DEF: &str = "<path id=\"";
const GLYPH_USE: &str = "<use xlink:href=\"#g";
/// Outline coordinates are kept to this many points: far below a device pixel at any zoom, and coarse enough that
/// the same glyph at two positions (each `f32`) reads as the same shape.
const PRECISION: f64 = 1000.0;

/// One SVG document per page of `pdf`; `None` when it isn't a PDF.
pub fn pdf_svgs(pdf: Vec<u8>) -> Option<Vec<String>> {
    let pdf = Pdf::new(pdf).ok()?;
    let cache = RenderCache::new();
    let (interpret, render) = (InterpreterSettings::default(), SvgRenderSettings::default());
    let svgs = pdf
        .pages()
        .iter()
        .enumerate()
        .map(|(n, page)| {
            let svg = hayro_svg::convert(page, &cache, &interpret, &render);
            let svg = share_glyphs(&svg).unwrap_or(svg);
            prefix_ids(&svg, &format!("pdf{n}-"))
        })
        .collect();
    Some(svgs)
}

/// `svg` with one outline per glyph shape: each outline drawn from its first point, and each `<use>` translated to
/// where that point was. `None` (keep `svg` as it is) when the output isn't shaped as expected, or a glyph is painted
/// with anything but a color (a pattern's position would move with the translation).
fn share_glyphs(svg: &str) -> Option<String> {
    let defs_at = svg.find(GLYPH_DEFS)?;
    let defs_end = defs_at + svg[defs_at..].find("</defs>")? + "</defs>".len();

    // Each glyph id → (its shape's index, the offset of its first point).
    let mut shapes: Vec<String> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut glyphs: HashMap<&str, (usize, f64, f64)> = HashMap::new();
    let mut rest = &svg[defs_at + GLYPH_DEFS.len()..defs_end];
    while let Some(at) = rest.find(GLYPH_DEF) {
        rest = &rest[at + GLYPH_DEF.len()..];
        let (id, after) = rest.split_once("\" d=\"")?;
        let (d, after) = after.split_once('"')?;
        rest = after;
        let (shape, x, y) = from_origin(d)?;
        let i = match index.get(&shape) {
            Some(&i) => i,
            None => {
                index.insert(shape.clone(), shapes.len());
                shapes.push(shape);
                shapes.len() - 1
            }
        };
        glyphs.insert(id, (i, x, y));
    }

    let mut out = String::with_capacity(svg.len() / 4);
    rewrite_uses(&svg[..defs_at], &glyphs, &mut out)?;
    out.push_str(GLYPH_DEFS);
    for (i, shape) in shapes.iter().enumerate() {
        write!(out, "<path id=\"g{i}\" d=\"{shape}\"/>").ok()?;
    }
    out.push_str("</defs>");
    rewrite_uses(&svg[defs_end..], &glyphs, &mut out)?;
    Some(out)
}

/// `part` into `out`, each glyph `<use>` pointing at its shape and translated to the glyph's first point.
fn rewrite_uses(
    part: &str,
    glyphs: &HashMap<&str, (usize, f64, f64)>,
    out: &mut String,
) -> Option<()> {
    let mut rest = part;
    while let Some(at) = rest.find(GLYPH_USE) {
        out.push_str(&rest[..at]);
        rest = &rest[at + GLYPH_USE.len() - 1..];
        let (id, after) = rest.split_once('"')?;
        let (attrs, after) = after.split_once("/>")?;
        if attrs.contains("url(") {
            return None;
        }
        let &(shape, x, y) = glyphs.get(id)?;
        write!(out, "<use xlink:href=\"#g{shape}\"").ok()?;
        match attrs.split_once(" transform=\"") {
            Some((before, transform)) => {
                let (transform, after_transform) = transform.split_once('"')?;
                write!(
                    out,
                    "{before} transform=\"{transform} translate({x} {y})\"{after_transform}/>"
                )
                .ok()?;
            }
            None => write!(out, " transform=\"translate({x} {y})\"{attrs}/>").ok()?,
        }
        rest = after;
    }
    out.push_str(rest);
    Some(())
}

/// Path data `d` (absolute `M`, `L`, `Q`, `C`, `Z`, as `hayro-svg` writes it) moved so its first point is the origin,
/// rounded to [`PRECISION`]; and that first point (the origin for an empty outline, a space's).
fn from_origin(d: &str) -> Option<(String, f64, f64)> {
    let mut origin = None;
    let mut out = String::with_capacity(d.len());
    for token in d.split(' ').filter(|t| !t.is_empty()) {
        let (command, pair) = match token.as_bytes()[0] {
            b'M' | b'L' | b'Q' | b'C' | b'Z' => token.split_at(1),
            _ => ("", token),
        };
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(command);
        if pair.is_empty() {
            continue;
        }
        let (x, y) = pair.split_once(',')?;
        let (x, y): (f64, f64) = (x.parse().ok()?, y.parse().ok()?);
        let (x0, y0) = *origin.get_or_insert((x, y));
        write!(out, "{},{}", round(x - x0), round(y - y0)).ok()?;
    }
    let (x0, y0) = origin.unwrap_or_default();
    Some((out, round(x0), round(y0)))
}

fn round(v: f64) -> f64 {
    // `+ 0.0` turns `-0` into `0`, so the same shape always spells the same.
    (v * PRECISION).round() / PRECISION + 0.0
}

/// `svg` with `prefix` before every id it defines and every reference to one.
fn prefix_ids(svg: &str, prefix: &str) -> String {
    let mut out = String::with_capacity(svg.len() + svg.len() / 16);
    // Each site's next occurrence at or after `done`; only the one just consumed is searched again.
    let find = |from: usize, site: &str| svg[from..].find(site).map(|at| from + at);
    let mut next = ID_SITES.map(|site| find(0, site));
    let mut done = 0;
    while let Some((i, at)) = next
        .iter()
        .enumerate()
        .filter_map(|(i, at)| at.map(|at| (i, at)))
        .min_by_key(|&(_, at)| at)
    {
        let end = at + ID_SITES[i].len();
        out.push_str(&svg[done..end]);
        out.push_str(prefix);
        done = end;
        next[i] = find(end, ID_SITES[i]);
    }
    out.push_str(&svg[done..]);
    out
}

#[cfg(test)]
mod tests {
    use super::{prefix_ids, share_glyphs};

    #[test]
    fn prefixes_definitions_and_references() {
        let svg = r##"<svg><defs><path id="g0" d="M0"/><clipPath id="c0"/></defs><use xlink:href="#g0" fill="#000000" clip-path="url(#c0)"/></svg>"##;
        assert_eq!(
            prefix_ids(svg, "pdf1-"),
            r##"<svg><defs><path id="pdf1-g0" d="M0"/><clipPath id="pdf1-c0"/></defs><use xlink:href="#pdf1-g0" fill="#000000" clip-path="url(#pdf1-c0)"/></svg>"##
        );
    }

    #[test]
    fn one_outline_per_shape_each_use_translated_to_its_glyph() {
        let svg = r##"<svg><use xlink:href="#g0" transform="translate(36 50)" fill="#000000"/><use xlink:href="#g1" fill="#000000"/><use xlink:href="#g2" transform="translate(36 50)" fill="#000000"/><defs id="outline-glyph"><path id="g0" d="M1,2 L3,2 Z"/><path id="g1" d="M11.5,-2 L13.5,-2 Z"/><path id="g2" d="M1,2 L1,4 Z"/></defs></svg>"##;
        assert_eq!(
            share_glyphs(svg).unwrap(),
            r##"<svg><use xlink:href="#g0" transform="translate(36 50) translate(1 2)" fill="#000000"/><use xlink:href="#g0" transform="translate(11.5 -2)" fill="#000000"/><use xlink:href="#g1" transform="translate(36 50) translate(1 2)" fill="#000000"/><defs id="outline-glyph"><path id="g0" d="M0,0 L2,0 Z"/><path id="g1" d="M0,0 L0,2 Z"/></defs></svg>"##
        );
    }

    #[test]
    fn glyphs_painted_with_a_pattern_keep_their_outlines() {
        let svg = r##"<svg><use xlink:href="#g0" fill="url(#v0)"/><defs id="outline-glyph"><path id="g0" d="M1,2 L3,2 Z"/></defs></svg>"##;
        assert_eq!(share_glyphs(svg), None);
    }
}
