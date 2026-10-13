//! Typst embedded in-process: a resume JSON through a template into PDF, SVG and PNG pages, plus the template's line
//! and page metrics (ARCHITECTURE.md §6).
//!
//! A [`Renderer`] is long-lived: it keeps the parsed template and loaded fonts, and Typst's memoization makes a
//! re-render after a small edit incremental.

pub mod cover;
mod fonts;
mod readback;
mod template;
mod world;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use typst::WorldExt;
use typst::diag::{Severity, SourceDiagnostic};
use typst::foundations::{Dict, IntoValue, Label, Selector};
use typst::introspection::{Introspector, MetadataElem};
use typst::utils::{PicoStr, Scalar, hash128};
use typst_layout::PagedDocument;

pub use fonts::FontOptions;
pub use readback::pdf_text;
pub use template::{Manifest, Template};
use world::ResumeWorld;

/// Compilations a memoized result may go unused before it is evicted.
const CACHE_AGE: usize = 10;

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("typst: {}", .0.join("\n"))]
    Compile(Vec<String>),
    #[error("PDF export: {}", .0.join("\n"))]
    Pdf(Vec<String>),
    #[error("PNG export: {0}")]
    Png(String),
    #[error("template metadata: {0}")]
    Metrics(String),
    #[error("PDF read-back: the exported PDF doesn't parse")]
    ReadBack,
}

/// A wrapped-line count the template recorded: a bullet's or skills line's `id`, its `lines`, and `orig`, the count of
/// the master text it replaces (equal to `lines` when it replaces none).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineCount {
    pub id: String,
    #[serde(deserialize_with = "count")]
    pub lines: u32,
    #[serde(deserialize_with = "count")]
    pub orig: u32,
}

/// A count Typst may hand over as a float (`1 + calc.round(…)` is one): any whole, non-negative number.
fn count<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let n = f64::deserialize(d)?;
    if n.fract() != 0.0 || !(0.0..=f64::from(u32::MAX)).contains(&n) {
        return Err(serde::de::Error::custom(format!("{n} is not a line count")));
    }
    Ok(n as u32)
}

/// What the template reports about a compiled resume: `<pages>`, every `<bullet>` and every `<group>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metrics {
    pub pages: u32,
    pub bullets: Vec<LineCount>,
    pub groups: Vec<LineCount>,
}

/// A text to measure at the width of the line it targets (the template's measure mode).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    /// Returned with its count.
    pub id: String,
    pub kind: LineKind,
    /// A bullet's indent steps (1 under an entry with notes, as the template lays it out); 0 otherwise.
    pub depth: u32,
    pub text: String,
    /// The bold label before a note line's or skills line's text (`""` for none).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
}

/// The kind of line a [`Candidate`] targets, which sets its width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineKind {
    Bullet,
    Note,
    /// A skills line: `text` is its items joined by `", "`.
    Group,
}

/// Read-backs a [`Renderer`] remembers before it starts over: a review's toggles revisit the same resumes.
const READ_BACKS: usize = 256;

/// Compiles resumes through one template.
pub struct Renderer {
    world: ResumeWorld,
    manifest: Manifest,
    /// What a compile depends on besides its inputs: the template and the fonts.
    id: u128,
    /// PDF read-backs by [`Rendered::key`].
    texts: HashMap<u128, String>,
}

impl Renderer {
    pub fn new(template: &Template, fonts: FontOptions) -> Self {
        Self {
            world: ResumeWorld::new(&template.source, fonts::load(fonts)),
            manifest: template.manifest.clone(),
            id: hash128(&(&template.source, fonts.system)),
            texts: HashMap::new(),
        }
    }

    /// The template's manifest.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Compiles `data`, the resume as a JSON string (`sys.inputs.data`), with `sys.inputs.highlight` set to `"1"` when
    /// `highlight` (the review's preview; never the export).
    pub fn render(&mut self, data: &str, highlight: bool) -> Result<Rendered, RenderError> {
        let mut inputs = Dict::new();
        inputs.insert("data".into(), data.into_value());
        if highlight {
            inputs.insert("highlight".into(), "1".into_value());
        }
        let (doc, warnings) = self.compile(inputs)?;
        let metrics = metrics(&doc)?;
        Ok(Rendered {
            doc,
            metrics,
            warnings,
            key: hash128(&(self.id, data, highlight)),
        })
    }

    /// [`Rendered::pdf_text`], remembered: a compile is a function of the template, fonts and inputs, and its PDF
    /// carries no timestamp, so the same resume reads back the same text and each distinct one is read once.
    pub fn pdf_text(&mut self, rendered: &Rendered) -> Result<String, RenderError> {
        if let Some(text) = self.texts.get(&rendered.key) {
            return Ok(text.clone());
        }
        let text = rendered.pdf_text()?;
        if self.texts.len() >= READ_BACKS {
            self.texts.clear();
        }
        self.texts.insert(rendered.key, text.clone());
        Ok(text)
    }

    /// Every candidate's wrapped line count at the width of the line it targets, in order, from one compile in the
    /// template's measure mode (`sys.inputs.mode = "measure"`, `sys.inputs.candidates`).
    pub fn measure(&mut self, candidates: &[Candidate]) -> Result<Vec<(String, u32)>, RenderError> {
        if candidates.is_empty() {
            return Ok(Vec::new());
        }
        let json = serde_json::to_string(candidates)
            .map_err(|e| RenderError::Metrics(format!("candidates: {e}")))?;
        let mut inputs = Dict::new();
        inputs.insert("mode".into(), "measure".into_value());
        inputs.insert("candidates".into(), json.into_value());
        let (doc, _) = self.compile(inputs)?;
        let counts = line_counts(&doc, "measure")?;
        if counts.len() != candidates.len() {
            return Err(RenderError::Metrics(format!(
                "{} <measure> counts for {} candidates",
                counts.len(),
                candidates.len()
            )));
        }
        Ok(counts.into_iter().map(|c| (c.id, c.lines)).collect())
    }

    /// One compile with `inputs` as `sys.inputs`: the document and its formatted warnings.
    fn compile(&mut self, inputs: Dict) -> Result<(PagedDocument, Vec<String>), RenderError> {
        self.world.set_inputs(inputs);
        let warned = typst::compile::<PagedDocument>(&self.world);
        typst::comemo::evict(CACHE_AGE);
        let warnings = warned.warnings.iter().map(|d| self.describe(d)).collect();
        let doc = warned.output.map_err(|errs| {
            RenderError::Compile(errs.iter().map(|d| self.describe(d)).collect())
        })?;
        Ok((doc, warnings))
    }

    /// `error: message (resume.typ:12)`, then its hints.
    fn describe(&self, d: &SourceDiagnostic) -> String {
        let severity = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        let mut out = format!("{severity}: {}", d.message);
        let source = self.world.main_source();
        if d.span.id() == Some(source.id())
            && let Some(line) = self
                .world
                .range(d.span)
                .and_then(|r| source.lines().byte_to_line(r.start))
        {
            out += &format!(" (resume.typ:{})", line + 1);
        }
        for hint in &d.hints {
            out += &format!("\n  hint: {}", hint.v);
        }
        out
    }
}

/// A compiled resume.
pub struct Rendered {
    doc: PagedDocument,
    pub metrics: Metrics,
    /// Compiler warnings, formatted.
    pub warnings: Vec<String>,
    /// The compile's identity: its renderer's template and fonts, and its inputs.
    key: u128,
}

impl Rendered {
    pub fn page_count(&self) -> usize {
        self.doc.pages().len()
    }

    /// The PDF, with no creation timestamp so the same resume gives the same bytes.
    pub fn pdf(&self) -> Result<Vec<u8>, RenderError> {
        typst_pdf::pdf(&self.doc, &typst_pdf::PdfOptions::default())
            .map_err(|errs| RenderError::Pdf(errs.iter().map(|d| d.message.to_string()).collect()))
    }

    /// The text a parser reads back from [`Self::pdf`]'s bytes (see [`pdf_text`]): what the score's format check
    /// looks for the matched skills in.
    pub fn pdf_text(&self) -> Result<String, RenderError> {
        pdf_text(self.pdf()?).ok_or(RenderError::ReadBack)
    }

    /// One SVG document per page.
    pub fn svgs(&self) -> Vec<String> {
        self.doc
            .pages()
            .iter()
            .map(|p| typst_svg::svg(p, &typst_svg::SvgOptions::default()))
            .collect()
    }

    /// One PNG per page at `scale` pixels per point (2.0 is the `typst` CLI's default 144 ppi).
    pub fn pngs(&self, scale: f32) -> Result<Vec<Vec<u8>>, RenderError> {
        let options = typst_render::RenderOptions {
            pixel_per_pt: Scalar::new(f64::from(scale)),
            render_bleed: false,
        };
        self.doc
            .pages()
            .iter()
            .map(|p| {
                typst_render::render(p, &options)
                    .encode_png()
                    .map_err(|e| RenderError::Png(e.to_string()))
            })
            .collect()
    }
}

/// The template's `<pages>`, `<bullet>` and `<group>` metadata, read from the compiled document's introspector.
fn metrics(doc: &PagedDocument) -> Result<Metrics, RenderError> {
    let pages = values(doc, "pages")?
        .first()
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| RenderError::Metrics("no <pages> page count".into()))?;
    Ok(Metrics {
        pages: pages as u32,
        bullets: line_counts(doc, "bullet")?,
        groups: line_counts(doc, "group")?,
    })
}

/// The values of every metadata element labeled `label`, in document order.
fn values(doc: &PagedDocument, label: &str) -> Result<Vec<serde_json::Value>, RenderError> {
    let label = Label::new(PicoStr::intern(label)).expect("label is not empty");
    doc.introspector()
        .query(&Selector::Label(label))
        .iter()
        .map(|c| {
            let meta = c.to_packed::<MetadataElem>().ok_or_else(|| {
                RenderError::Metrics(format!("<{}> is not metadata", label.resolve()))
            })?;
            serde_json::to_value(&meta.value).map_err(|e| RenderError::Metrics(e.to_string()))
        })
        .collect()
}

/// The line counts labeled `label`.
fn line_counts(doc: &PagedDocument, label: &str) -> Result<Vec<LineCount>, RenderError> {
    values(doc, label)?
        .into_iter()
        .map(|v| {
            serde_json::from_value(v).map_err(|e| RenderError::Metrics(format!("<{label}>: {e}")))
        })
        .collect()
}
