//! PDF text read-back: the text a parser extracts from a PDF's bytes, for the score's format check (ARCHITECTURE.md
//! §4: "PDF read-back matches").
//!
//! Every glyph is read through its font's `ToUnicode` map (ligatures read as their letters, symbols as what they
//! stand for), then laid out as a reader lays it out: glyphs that share a baseline make a line, read left to right,
//! with a space wherever the gap to the next glyph is wider than a sixth of the font size and no space glyph fills
//! it; lines top to bottom, each ending in a newline. So a term a line break splits (`front-` / `end`) reads with a
//! break in it, as an ATS reads it. `ActualText` spans are not applied (PDFKit doesn't apply them either).

use hayro_interpret::font::Glyph;
use hayro_interpret::hayro_cmap::BfString;
use hayro_interpret::hayro_syntax::Pdf;
use hayro_interpret::{
    BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache,
    InterpreterSettings, Paint, PathDrawMode, SoftMask, interpret_page,
};
use kurbo::{Affine, BezPath, Point, Rect};

/// A gap wider than this many font sizes between two glyphs on a line reads as a space.
const SPACE_GAP: f64 = 1.0 / 6.0;
/// Glyphs whose baselines are closer than this many font sizes are on the same line.
const SAME_LINE: f64 = 0.3;

/// The text of `pdf`: its lines, page by page, one per line of output; `None` when it isn't a PDF.
pub fn pdf_text(pdf: Vec<u8>) -> Option<String> {
    let mut out = String::new();
    for glyphs in pages(pdf)? {
        lay_out(glyphs, &mut out);
    }
    out.truncate(out.trim_end().len());
    Some(out)
}

/// Every page's glyphs that map to text, in drawing order.
fn pages(pdf: Vec<u8>) -> Option<Vec<Vec<Placed>>> {
    let pdf = Pdf::new(pdf).ok()?;
    let cache = InterpreterCache::new();
    let pages = pdf
        .pages()
        .iter()
        .map(|page| {
            let mut context = Context::new(
                Affine::IDENTITY,
                Rect::new(0.0, 0.0, 1.0, 1.0),
                &cache,
                pdf.xref(),
                InterpreterSettings::default(),
            );
            let mut glyphs = Glyphs::default();
            interpret_page(page, &mut context, &mut glyphs);
            glyphs.0
        })
        .collect();
    Some(pages)
}

/// A glyph's text where it was drawn, in PDF user space (y up).
#[derive(Debug)]
struct Placed {
    text: String,
    /// Origin on the baseline.
    x: f64,
    y: f64,
    /// Where the next glyph would start: the origin plus the advance.
    end: f64,
    /// The font size, in points.
    size: f64,
}

/// Collects every drawn glyph that maps to text; nothing else is read.
#[derive(Default)]
struct Glyphs(Vec<Placed>);

impl<'a> Device<'a> for Glyphs {
    fn set_soft_mask(&mut self, _: Option<SoftMask<'a>>) {}
    fn set_blend_mode(&mut self, _: BlendMode) {}
    fn draw_path(&mut self, _: &BezPath, _: Affine, _: &Paint<'a>, _: &PathDrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn draw_image(&mut self, _: Image<'a, '_>, _: Affine) {}
    fn pop_clip_path(&mut self) {}
    fn pop_transparency_group(&mut self) {}

    fn draw_glyph(
        &mut self,
        glyph: &Glyph<'a>,
        transform: Affine,
        glyph_transform: Affine,
        _: &Paint<'a>,
        _: &GlyphDrawMode,
    ) {
        let text = match glyph.as_unicode() {
            Some(BfString::Char(c)) => c.to_string(),
            Some(BfString::String(s)) => s,
            None => return,
        };
        // Glyph space is 1000 units per em.
        let t = transform * glyph_transform;
        let origin = t * Point::ORIGIN;
        let advance = match glyph {
            Glyph::Outline(g) => g.advance_width().map_or(0.0, f64::from),
            Glyph::Type3(_) => 0.0,
        };
        let [_, _, c, d, _, _] = t.as_coeffs();
        self.0.push(Placed {
            text,
            x: origin.x,
            y: origin.y,
            end: (t * Point::new(advance, 0.0)).x,
            size: c.hypot(d) * 1000.0,
        });
    }
}

/// `glyphs` as lines of text, top to bottom, appended to `out`.
fn lay_out(mut glyphs: Vec<Placed>, out: &mut String) {
    // Top to bottom; the sort is stable, so glyphs on one baseline keep their drawing order until sorted by x.
    glyphs.sort_by(|a, b| b.y.total_cmp(&a.y));
    let mut start = 0;
    while start < glyphs.len() {
        let (y, size) = (glyphs[start].y, glyphs[start].size);
        let len = glyphs[start..]
            .iter()
            .position(|g| y - g.y > SAME_LINE * size.max(g.size))
            .unwrap_or(glyphs.len() - start);
        let line = &mut glyphs[start..start + len];
        line.sort_by(|a, b| a.x.total_cmp(&b.x));
        let mut prev: Option<&Placed> = None;
        for g in line.iter() {
            if let Some(p) = prev
                && g.x - p.end > SPACE_GAP * p.size.max(g.size)
                && !p.text.ends_with(char::is_whitespace)
                && !g.text.starts_with(char::is_whitespace)
            {
                out.push(' ');
            }
            out.push_str(&g.text);
            prev = Some(g);
        }
        // A line's trailing space glyph (Typst keeps the one a line breaks at) doesn't read, as in PDFKit.
        out.truncate(out.trim_end_matches([' ', '\t']).len());
        out.push('\n');
        start += len;
    }
}

#[cfg(test)]
mod tests {
    use super::{pages, pdf_text};
    use crate::{FontOptions, Manifest, Renderer, Template};

    /// `body` on a page 8 cm wide, with `sys.inputs.data` as its text, through a template of its own.
    fn pdf(body: &str, data: &str, fonts: FontOptions) -> Vec<u8> {
        let template = Template {
            source: format!(
                "#set page(width: 8cm, height: auto, margin: 5mm)\n\
                 #set text(font: \"Liberation Serif\", size: 11pt)\n\
                 #metadata(1) <pages>\n{body}"
            ),
            manifest: Manifest {
                name: "test".into(),
                ats_safe: true,
                columns: 1,
            },
        };
        Renderer::new(&template, fonts)
            .render(data, false)
            .unwrap()
            .pdf()
            .unwrap()
    }

    #[test]
    fn ligatures_read_as_their_letters() {
        // Libertinus Serif (Typst's embedded fonts) has `fi`, `ffi` and `fl` ligatures; Liberation Serif has none.
        let words = "Efficient workflow, office filings";
        let bytes = pdf(
            "#text(font: \"Libertinus Serif\", sys.inputs.data)",
            words,
            FontOptions { system: true },
        );
        let glyphs = &pages(bytes.clone()).unwrap()[0];
        assert!(
            glyphs.iter().any(|g| g.text.chars().count() > 1),
            "no ligature glyph: {:?}",
            glyphs.iter().map(|g| &g.text).collect::<Vec<_>>()
        );
        assert_eq!(pdf_text(bytes).unwrap(), words);
    }

    #[test]
    fn symbols_read_as_printed() {
        let words = "C++, C#, .NET, Node.js, CI/CD, Women's • 100% – done";
        let bytes = pdf(
            "#text(size: 8pt, sys.inputs.data)",
            words,
            FontOptions::default(),
        );
        assert_eq!(pdf_text(bytes).unwrap(), words);
    }

    #[test]
    fn a_line_break_at_a_hyphen_splits_the_term() {
        // Some width fits `Built the front-` but not `front-end`: the line breaks after the hyphen.
        let texts: Vec<String> = (20..40)
            .map(|mm| {
                let bytes = pdf(
                    &format!("#block(width: {mm}mm, sys.inputs.data)"),
                    "Built the front-end",
                    FontOptions::default(),
                );
                pdf_text(bytes).unwrap()
            })
            .collect();
        assert!(
            texts.iter().any(|t| t == "Built the front-\nend"),
            "{texts:?}"
        );
    }

    #[test]
    fn runs_on_one_baseline_read_as_one_line_with_a_space_between() {
        let bytes = pdf(
            "#grid(columns: (1fr, auto), [Acme Corp], [June 2025])\nSoftware Engineer",
            "",
            FontOptions::default(),
        );
        assert_eq!(
            pdf_text(bytes).unwrap(),
            "Acme Corp June 2025\nSoftware Engineer"
        );
    }

    #[test]
    fn pages_read_in_order() {
        let bytes = pdf("One\n#pagebreak()\nTwo", "", FontOptions::default());
        assert_eq!(pdf_text(bytes).unwrap(), "One\nTwo");
    }

    #[test]
    fn bytes_that_are_not_a_pdf_have_no_text() {
        assert_eq!(pdf_text(b"not a pdf".to_vec()), None);
    }
}
