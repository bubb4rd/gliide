//! Resume files for the import (ARCHITECTURE.md §3 "Import"): a PDF, DOCX, Markdown or plain-text resume read as its
//! text and links, for the import call's `<resume>` and `<links>` blocks; a v1 master JSON read as the resume
//! it already is.
//!
//! Every file's text is normalized the same way: `\r\n` reads as `\n`, each line loses its trailing whitespace, the
//! whole text its leading and trailing blank lines. A PDF reads as [`resume_render::pdf_read`] reads it. Markdown
//! keeps its markup; its `[text](url)` links (not images) are its links. Plain text has none.
//!
//! A DOCX reads as Word lays it out, without styling: the first-page and default headers, the body, then the
//! footers, one line per paragraph (a tab where it has a tab, a line break where it has one), at most one blank line
//! in a row. A list item (its own numbering or its style's) starts with `• `. A table row whose cells each hold at
//! most one paragraph reads as one line, cells joined by a tab (`Company<TAB>Dates`); a row with a fuller cell (a
//! layout table) reads cell by cell, a line per paragraph. Text boxes read after the paragraph that anchors them.
//! Links are external `w:hyperlink`s and `HYPERLINK` fields (simple and complex); links to a bookmark in the
//! document are not links. Deleted text (tracked changes) doesn't read; inserted text does.

use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::io::{Cursor, Read};
use std::path::Path;

use resume_core::Resume;
use resume_render::{Link, pdf_read};
use roxmltree::{Document, Node};
use zip::ZipArchive;
use zip::result::ZipError;

use crate::PipelineError;

/// A resume as the import reads it.
#[derive(Debug, Clone, PartialEq)]
pub enum ResumeSource {
    /// A v1 master JSON (the legacy format): already structured, no model call needed.
    AtsFit(Resume),
    /// A PDF, DOCX, Markdown or plain-text resume: text and links for the import call.
    File(ResumeFile),
}

/// A resume file's text and links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeFile {
    pub text: String,
    /// In document order, each distinct text and URL once.
    pub links: Vec<Link>,
    /// The PDF's page count; `None` for other files.
    pub pages: Option<usize>,
}

/// Reads `path` by its extension (case-insensitive): `pdf`, `docx`, `md`/`markdown`/`txt`, `json`. Any other
/// extension is [`PipelineError::UnsupportedFile`] before the file is opened.
pub fn read_resume(path: &Path) -> Result<ResumeSource, PipelineError> {
    let ext = path
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !matches!(
        ext.as_str(),
        "pdf" | "docx" | "md" | "markdown" | "txt" | "json"
    ) {
        return Err(PipelineError::UnsupportedFile);
    }
    let bytes = std::fs::read(path).map_err(|source| PipelineError::ReadFile {
        path: path.to_owned(),
        source,
    })?;
    Ok(match ext.as_str() {
        "json" => ResumeSource::AtsFit(
            serde_json::from_slice(&bytes).map_err(PipelineError::NotMasterJson)?,
        ),
        "pdf" => ResumeSource::File(pdf_file(bytes)?),
        "docx" => ResumeSource::File(docx_file(&bytes)?),
        _ => ResumeSource::File(text_file(bytes, ext != "txt")?),
    })
}

/// A PDF's text, links and page count.
fn pdf_file(bytes: Vec<u8>) -> Result<ResumeFile, PipelineError> {
    let read = pdf_read(bytes).ok_or(PipelineError::NotPdf)?;
    let text = normalize(&read.text);
    if text.is_empty() {
        return Err(PipelineError::NoPdfText);
    }
    Ok(ResumeFile {
        text,
        links: read.links,
        pages: Some(read.pages),
    })
}

/// A UTF-8 text file (a byte order mark aside); with its `[text](url)` links when `markdown`.
fn text_file(bytes: Vec<u8>, markdown: bool) -> Result<ResumeFile, PipelineError> {
    let text = String::from_utf8(bytes).map_err(|_| PipelineError::NotUtf8)?;
    let text = normalize(text.strip_prefix('\u{feff}').unwrap_or(&text));
    if text.is_empty() {
        return Err(PipelineError::EmptyFile);
    }
    let links = if markdown {
        markdown_links(&text)
    } else {
        Vec::new()
    };
    Ok(ResumeFile {
        text,
        links,
        pages: None,
    })
}

/// `text` with `\r\n` as `\n`, each line's trailing whitespace trimmed, and the whole trimmed.
fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out.truncate(out.trim_end().len());
    out.drain(..out.len() - out.trim_start().len());
    out
}

/// The `[text](url)` links of Markdown `text`, on one line each; images (`![alt](src)`) aren't links. A link's text
/// loses the emphasis around it (`[**GitHub**](…)` reads `GitHub`); a title after the URL is dropped.
fn markdown_links(text: &str) -> Vec<Link> {
    let mut links = Vec::new();
    for line in text.lines() {
        let mut rest = line;
        while let Some(open) = rest.find('[') {
            let image = rest[..open].ends_with('!');
            let inside = &rest[open + 1..];
            let Some(close) = inside.find(']') else {
                break;
            };
            let label = &inside[..close];
            // `[a [b](u)`: the link starts at the last `[`.
            if let Some(inner) = label.rfind('[') {
                rest = &inside[inner..];
                continue;
            }
            rest = &inside[close + 1..];
            let Some((target, after)) = rest.strip_prefix('(').and_then(|t| t.split_once(')'))
            else {
                continue;
            };
            rest = after;
            let url = target.split_whitespace().next().unwrap_or_default();
            let url = url.strip_prefix('<').unwrap_or(url);
            let url = url.strip_suffix('>').unwrap_or(url);
            if !image {
                let text = label.trim().trim_matches(['*', '_', '`']).trim();
                push_link(&mut links, text, url);
            }
        }
    }
    links
}

/// Adds a link to `links` unless its URL is empty or `links` has it: its text with whitespace runs as one space.
fn push_link(links: &mut Vec<Link>, text: &str, url: &str) {
    let url = url.trim();
    if url.is_empty() {
        return;
    }
    let link = Link {
        text: text.split_whitespace().collect::<Vec<_>>().join(" "),
        url: url.to_owned(),
    };
    if !links.contains(&link) {
        links.push(link);
    }
}

/// A DOCX's text and links, as the module docs describe.
fn docx_file(bytes: &[u8]) -> Result<ResumeFile, PipelineError> {
    let mut zip =
        ZipArchive::new(Cursor::new(bytes)).map_err(|e| PipelineError::NotDocx(e.to_string()))?;
    let document = part(&mut zip, "word/document.xml")?
        .ok_or_else(|| PipelineError::NotDocx("no word/document.xml".into()))?;
    let document = parse(&document)?;
    let rels = rels(&mut zip, "word/document.xml")?;

    let list_styles = match part(&mut zip, "word/styles.xml")? {
        Some(styles) => list_styles(&parse(&styles)?),
        None => HashSet::new(),
    };
    let mut reader = Docx {
        rels: HashMap::new(),
        list_styles,
        links: Vec::new(),
        fields: Vec::new(),
    };

    // Headers and footers each section shows on its first page or by default, once each.
    let (mut headers, mut footers) = (Vec::new(), Vec::new());
    for reference in document.descendants() {
        let parts = match reference.tag_name().name() {
            "headerReference" => &mut headers,
            "footerReference" => &mut footers,
            _ => continue,
        };
        if attr(reference, "type").is_some_and(|t| t == "even") {
            continue;
        }
        if let Some(target) = attr(reference, "id").and_then(|id| rels.get(id))
            && let Some(name) = part_name(target)
            && !parts.contains(&name)
        {
            parts.push(name);
        }
    }

    let mut lines = Vec::new();
    for name in &headers {
        reader.part(&mut zip, name, &mut lines)?;
    }
    if let Some(body) = document.root_element().children().find(|n| is(*n, "body")) {
        reader.rels = rels;
        reader.blocks(body, &mut lines);
    }
    for name in &footers {
        reader.part(&mut zip, name, &mut lines)?;
    }

    // Paragraphs may hold line breaks; every line is trimmed, and blank lines run to one.
    let mut text = String::new();
    let mut blank = true;
    for line in lines.iter().flat_map(|p| p.split('\n')) {
        let line = line.trim();
        if line.is_empty() {
            if !blank {
                text.push('\n');
            }
            blank = true;
        } else {
            text.push_str(line);
            text.push('\n');
            blank = false;
        }
    }
    let text = normalize(&text);
    if text.is_empty() {
        return Err(PipelineError::EmptyFile);
    }
    Ok(ResumeFile {
        text,
        links: reader.links,
        pages: None,
    })
}

type Zip<'a> = ZipArchive<Cursor<&'a [u8]>>;

/// The zip member `name` as text; `None` when the zip has no such member.
fn part(zip: &mut Zip<'_>, name: &str) -> Result<Option<String>, PipelineError> {
    let mut file = match zip.by_name(name) {
        Ok(file) => file,
        Err(ZipError::FileNotFound) => return Ok(None),
        Err(e) => return Err(PipelineError::NotDocx(e.to_string())),
    };
    let mut xml = String::new();
    file.read_to_string(&mut xml)
        .map_err(|e| PipelineError::NotDocx(format!("{name}: {e}")))?;
    Ok(Some(xml))
}

fn parse(xml: &str) -> Result<Document<'_>, PipelineError> {
    Document::parse(xml).map_err(|e| PipelineError::NotDocx(e.to_string()))
}

/// The relationships of the part `name` (`word/_rels/<file>.rels`): id to target.
fn rels(zip: &mut Zip<'_>, name: &str) -> Result<HashMap<String, String>, PipelineError> {
    let (dir, file) = name.rsplit_once('/').unwrap_or(("", name));
    let Some(xml) = part(zip, &format!("{dir}/_rels/{file}.rels"))? else {
        return Ok(HashMap::new());
    };
    Ok(parse(&xml)?
        .root_element()
        .children()
        .filter(|n| is(*n, "Relationship"))
        .filter_map(|r| Some((attr(r, "Id")?.to_owned(), attr(r, "Target")?.to_owned())))
        .collect())
}

/// The zip member a `word/document.xml` relationship targets: relative to `word/`, or absolute from the root.
fn part_name(target: &str) -> Option<String> {
    if target.contains("://") {
        return None;
    }
    Some(match target.strip_prefix('/') {
        Some(absolute) => absolute.to_owned(),
        None => format!("word/{target}"),
    })
}

/// The paragraph styles whose paragraphs are list items: those with numbering of their own.
fn list_styles(styles: &Document<'_>) -> HashSet<String> {
    styles
        .root_element()
        .children()
        .filter(|s| is(*s, "style"))
        .filter(|s| s.children().find(|n| is(*n, "pPr")).is_some_and(numbered))
        .filter_map(|s| Some(attr(s, "styleId")?.to_owned()))
        .collect()
}

/// Whether paragraph properties `ppr` number the paragraph (`w:numId` 0 takes numbering away).
fn numbered(ppr: Node<'_, '_>) -> bool {
    ppr.children().find(|n| is(*n, "numPr")).is_some_and(|num| {
        num.children()
            .find(|n| is(*n, "numId"))
            .and_then(|id| attr(id, "val"))
            .is_none_or(|id| id != "0")
    })
}

/// Whether `node` is an element named `name`, in any namespace (transitional and strict OOXML name them alike).
fn is(node: Node<'_, '_>, name: &str) -> bool {
    node.is_element() && node.tag_name().name() == name
}

/// `node`'s attribute `name`, in any namespace.
fn attr<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.attributes()
        .find(|a| a.name() == name)
        .map(|a| a.value())
}

/// A complex field (`w:fldChar`) being read.
struct Field {
    /// The field code so far (`w:instrText`).
    code: String,
    /// For a `HYPERLINK` field past its code: the URL, and where its result starts in the paragraph's line.
    link: Option<(String, usize)>,
}

/// Reads a DOCX's parts into lines and links.
struct Docx {
    /// The relationships of the part being read.
    rels: HashMap<String, String>,
    list_styles: HashSet<String>,
    links: Vec<Link>,
    /// Open complex fields, innermost last.
    fields: Vec<Field>,
}

impl Docx {
    /// A header or footer part, appended to `out`.
    fn part(
        &mut self,
        zip: &mut Zip<'_>,
        name: &str,
        out: &mut Vec<String>,
    ) -> Result<(), PipelineError> {
        let Some(xml) = part(zip, name)? else {
            return Ok(());
        };
        self.rels = rels(zip, name)?;
        self.blocks(parse(&xml)?.root_element(), out);
        Ok(())
    }

    /// The paragraphs and tables in `node`, appended to `out` a paragraph or table row per line.
    fn blocks(&mut self, node: Node<'_, '_>, out: &mut Vec<String>) {
        for child in node.children().filter(Node::is_element) {
            match child.tag_name().name() {
                "p" => self.paragraph(child, out),
                "tbl" => self.table(child, out),
                "sdt" | "sdtContent" | "customXml" | "ins" | "moveTo" => self.blocks(child, out),
                "AlternateContent" => {
                    if let Some(choice) = child.children().find(|n| is(*n, "Choice")) {
                        self.blocks(choice, out);
                    }
                }
                _ => {}
            }
        }
    }

    fn paragraph(&mut self, p: Node<'_, '_>, out: &mut Vec<String>) {
        // A link field the last paragraph opened goes on from this one's start.
        for field in &mut self.fields {
            if let Some((_, start)) = &mut field.link {
                *start = 0;
            }
        }
        let mut line = String::new();
        let mut boxes = Vec::new();
        self.inline(p, &mut line, &mut boxes);
        let list = p.children().find(|n| is(*n, "pPr")).is_some_and(|ppr| {
            numbered(ppr)
                || ppr
                    .children()
                    .find(|n| is(*n, "pStyle"))
                    .and_then(|s| attr(s, "val"))
                    .is_some_and(|s| self.list_styles.contains(s))
        });
        if list && !line.trim().is_empty() {
            line.insert_str(0, "• ");
        }
        out.push(line);
        out.append(&mut boxes);
    }

    /// A table, a line per row when no cell holds more than one paragraph, otherwise cell by cell.
    fn table(&mut self, tbl: Node<'_, '_>, out: &mut Vec<String>) {
        for row in tbl.children().filter(|n| is(*n, "tr")) {
            let cells: Vec<Vec<String>> = row
                .children()
                .filter(|n| is(*n, "tc"))
                .map(|tc| {
                    let mut lines = Vec::new();
                    self.blocks(tc, &mut lines);
                    lines.retain(|l| !l.trim().is_empty());
                    lines
                })
                .collect();
            if cells.iter().all(|c| c.len() <= 1) {
                let cells: Vec<&str> = cells.iter().flatten().map(|l| l.trim()).collect();
                if !cells.is_empty() {
                    out.push(cells.join("\t"));
                }
            } else {
                out.extend(cells.into_iter().flatten());
            }
        }
    }

    /// The runs in `node`, appended to `line`; text boxes they anchor go to `boxes`.
    fn inline(&mut self, node: Node<'_, '_>, line: &mut String, boxes: &mut Vec<String>) {
        for child in node.children().filter(Node::is_element) {
            match child.tag_name().name() {
                "t" => line.push_str(child.text().unwrap_or_default()),
                "tab" | "ptab" => line.push('\t'),
                "br" | "cr" => line.push('\n'),
                "noBreakHyphen" => line.push('-'),
                "r" | "ins" | "moveTo" | "smartTag" | "customXml" | "sdt" | "sdtContent"
                | "bdo" | "dir" => self.inline(child, line, boxes),
                "hyperlink" => {
                    let start = line.len();
                    self.inline(child, line, boxes);
                    // `w:anchor` alone links to a bookmark in the document.
                    if let Some(url) = attr(child, "id").and_then(|id| self.rels.get(id)) {
                        push_link(&mut self.links, &line[start..], url);
                    }
                }
                "fldSimple" => {
                    let start = line.len();
                    self.inline(child, line, boxes);
                    if let Some(url) = attr(child, "instr").and_then(hyperlink_url) {
                        push_link(&mut self.links, &line[start..], url);
                    }
                }
                "fldChar" => match attr(child, "fldCharType") {
                    Some("begin") => self.fields.push(Field {
                        code: String::new(),
                        link: None,
                    }),
                    Some("separate") => {
                        if let Some(field) = self.fields.last_mut() {
                            field.link =
                                hyperlink_url(&field.code).map(|url| (url.to_owned(), line.len()));
                        }
                    }
                    Some("end") => {
                        if let Some(Field {
                            link: Some((url, start)),
                            ..
                        }) = self.fields.pop()
                        {
                            push_link(&mut self.links, &line[start.min(line.len())..], &url);
                        }
                    }
                    _ => {}
                },
                "instrText" => {
                    if let Some(field) = self.fields.last_mut() {
                        field.code.push_str(child.text().unwrap_or_default());
                    }
                }
                "AlternateContent" => {
                    if let Some(choice) = child.children().find(|n| is(*n, "Choice")) {
                        self.inline(choice, line, boxes);
                    }
                }
                "drawing" | "pict" => {
                    // The outermost text boxes; the ones inside them read with them.
                    for txbx in child.descendants().filter(|n| {
                        is(*n, "txbxContent")
                            && !n
                                .ancestors()
                                .skip(1)
                                .take_while(|a| a != &child)
                                .any(|a| is(a, "txbxContent"))
                    }) {
                        self.blocks(txbx, boxes);
                    }
                }
                _ => {}
            }
        }
    }
}

/// The URL of a `HYPERLINK` field code (`HYPERLINK "https://…" \o "tip"`); `None` for another field or a link to a
/// bookmark alone (`HYPERLINK \l "name"`).
fn hyperlink_url(code: &str) -> Option<&str> {
    let mut args = field_args(code);
    if !args.next()?.eq_ignore_ascii_case("HYPERLINK") {
        return None;
    }
    while let Some(arg) = args.next() {
        match arg {
            // Switches with an argument: a bookmark, a tooltip, a target frame.
            "\\l" | "\\o" | "\\t" => {
                args.next();
            }
            switch if switch.starts_with('\\') => {}
            url => return Some(url),
        }
    }
    None
}

/// A field code's words: quoted (`"a b"`) or whitespace-separated.
fn field_args(mut code: &str) -> impl Iterator<Item = &str> {
    std::iter::from_fn(move || {
        code = code.trim_start();
        if code.is_empty() {
            return None;
        }
        let (arg, rest) = match code.strip_prefix('"') {
            Some(quoted) => quoted.split_once('"').unwrap_or((quoted, "")),
            None => code.split_once(char::is_whitespace).unwrap_or((code, "")),
        };
        code = rest;
        Some(arg)
    })
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};
    use std::path::{Path, PathBuf};

    use resume_render::{FontOptions, Link, Manifest, Renderer, Template};
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    use super::{
        ResumeFile, ResumeSource, docx_file, hyperlink_url, pdf_file, read_resume, text_file,
    };
    use crate::PipelineError;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
    }

    fn file(path: &Path) -> ResumeFile {
        match read_resume(path).unwrap() {
            ResumeSource::File(file) => file,
            ResumeSource::AtsFit(_) => panic!("{} read as a master.json", path.display()),
        }
    }

    fn link(text: &str, url: &str) -> Link {
        Link {
            text: text.into(),
            url: url.into(),
        }
    }

    /// A zip of `parts` (name, contents), stored.
    fn zip(parts: &[(&str, &str)]) -> Vec<u8> {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        for (name, contents) in parts {
            zip.start_file(*name, options).unwrap();
            zip.write_all(contents.as_bytes()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    const NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:v="urn:schemas-microsoft-com:vml""#;
    const RELS: &str = r#"xmlns="http://schemas.openxmlformats.org/package/2006/relationships""#;

    /// A DOCX shaped as Word writes one: a header and footer, links of every kind, a table, lists, tracked changes and
    /// a text box.
    fn word_docx() -> Vec<u8> {
        let document = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document {NS}><w:body>
<w:p><w:r><w:t>Jane Q. Doe</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">jane.doe@example.com | </w:t></w:r><w:hyperlink r:id="rId1"><w:r><w:t>LinkedIn</w:t></w:r></w:hyperlink><w:r><w:t xml:space="preserve"> | </w:t></w:r><w:fldSimple w:instr=" HYPERLINK &quot;https://github.com/janeqdoe-example&quot; "><w:r><w:t>GitHub</w:t></w:r></w:fldSimple><w:r><w:t xml:space="preserve"> | </w:t></w:r><w:hyperlink w:anchor="skills"><w:r><w:t>Skills</w:t></w:r></w:hyperlink></w:p>
<w:p/>
<w:p><w:r><w:t xml:space="preserve">   </w:t></w:r></w:p>
<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Work Experience</w:t></w:r></w:p>
<w:tbl>
<w:tr><w:tc><w:p><w:r><w:t>Acme Widgets Co.</w:t></w:r></w:p></w:tc><w:tc><w:p/></w:tc><w:tc><w:p><w:r><w:t>June 2023 – Present</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:p><w:r><w:t>Left column</w:t></w:r></w:p><w:p><w:r><w:t>second paragraph</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Right column</w:t></w:r></w:p></w:tc></w:tr>
</w:tbl>
<w:p><w:r><w:t>Software Engineer</w:t><w:tab/><w:t>Springfield, IL</w:t><w:br/><w:t>Remote</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t xml:space="preserve">Built the order pipeline in </w:t></w:r><w:del><w:r><w:delText>Go</w:delText></w:r></w:del><w:ins><w:r><w:t>Rust</w:t></w:r></w:ins><w:r><w:t>.</w:t></w:r></w:p>
<w:p><w:pPr><w:pStyle w:val="ListBullet"/></w:pPr><w:r><w:t xml:space="preserve">Shipped </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> HYPERLINK "https://example.com/tidepool" \o "Tidepool" </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>Tidepool</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t xml:space="preserve"> to 1,000 users.</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:numId w:val="0"/></w:numPr></w:pPr><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText>PAGE</w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>Not a list item</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p>
<w:p><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wps:txbx><w:txbxContent><w:p><w:r><w:t>Boxed note</w:t></w:r></w:p></w:txbxContent></wps:txbx></w:drawing></mc:Choice><mc:Fallback><w:pict><v:textbox><w:txbxContent><w:p><w:r><w:t>Boxed note</w:t></w:r></w:p></w:txbxContent></v:textbox></w:pict></mc:Fallback></mc:AlternateContent></w:r><w:r><w:t>Anchor paragraph</w:t></w:r></w:p>
<w:p><w:bookmarkStart w:id="0" w:name="skills"/><w:r><w:t>Skills: Rust, TypeScript</w:t></w:r><w:bookmarkEnd w:id="0"/></w:p>
<w:sectPr><w:headerReference w:type="default" r:id="rId2"/><w:headerReference w:type="even" r:id="rId4"/><w:footerReference w:type="default" r:id="rId3"/></w:sectPr>
</w:body></w:document>"#
        );
        let rels = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships {RELS}>
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://linkedin.com/in/janeqdoe-example" TargetMode="External"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>
<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/>
<Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header2.xml"/>
</Relationships>"#
        );
        let header = format!(
            r#"<w:hdr {NS}><w:p><w:r><w:t xml:space="preserve">Portfolio: </w:t></w:r><w:hyperlink r:id="rId1"><w:r><w:t>janedoe.example.com</w:t></w:r></w:hyperlink></w:p></w:hdr>"#
        );
        let header_rels = format!(
            r#"<Relationships {RELS}><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://janedoe.example.com" TargetMode="External"/></Relationships>"#
        );
        let even = format!(r#"<w:hdr {NS}><w:p><w:r><w:t>Even pages</w:t></w:r></w:p></w:hdr>"#);
        let footer = format!(r#"<w:ftr {NS}><w:p><w:r><w:t>Page 1</w:t></w:r></w:p></w:ftr>"#);
        let styles = format!(
            r#"<w:styles {NS}><w:style w:type="paragraph" w:styleId="Heading1"><w:pPr><w:keepNext/></w:pPr></w:style><w:style w:type="paragraph" w:styleId="ListBullet"><w:pPr><w:numPr><w:numId w:val="2"/></w:numPr></w:pPr></w:style></w:styles>"#
        );
        zip(&[
            ("word/document.xml", &document),
            ("word/_rels/document.xml.rels", &rels),
            ("word/header1.xml", &header),
            ("word/_rels/header1.xml.rels", &header_rels),
            ("word/header2.xml", &even),
            ("word/footer1.xml", &footer),
            ("word/styles.xml", &styles),
        ])
    }

    #[test]
    fn a_word_docx_reads_a_line_per_paragraph_with_its_links() {
        let file = docx_file(&word_docx()).unwrap();
        assert_eq!(
            file.text.lines().collect::<Vec<_>>(),
            [
                "Portfolio: janedoe.example.com",
                "Jane Q. Doe",
                "jane.doe@example.com | LinkedIn | GitHub | Skills",
                "",
                "Work Experience",
                "Acme Widgets Co.\tJune 2023 – Present",
                "Left column",
                "second paragraph",
                "Right column",
                "Software Engineer\tSpringfield, IL",
                "Remote",
                "• Built the order pipeline in Rust.",
                "• Shipped Tidepool to 1,000 users.",
                "Not a list item",
                "Anchor paragraph",
                "Boxed note",
                "Skills: Rust, TypeScript",
                "Page 1",
            ]
        );
        // The link to the `skills` bookmark isn't one.
        assert_eq!(
            file.links,
            [
                link("janedoe.example.com", "https://janedoe.example.com"),
                link("LinkedIn", "https://linkedin.com/in/janeqdoe-example"),
                link("GitHub", "https://github.com/janeqdoe-example"),
                link("Tidepool", "https://example.com/tidepool"),
            ]
        );
        assert_eq!(file.pages, None);
    }

    #[test]
    fn a_textutil_docx_reads_as_written() {
        // `textutil -convert docx jane-doe.html`: macOS writes the bullets as text and drops links and tables.
        let file = file(&fixture("import/jane-doe.docx"));
        assert_eq!(
            file.text.lines().collect::<Vec<_>>(),
            [
                "Jane Q. Doe",
                "jane.doe@example.com | LinkedIn | github.com/janeqdoe-example",
                "Work Experience",
                "Acme Widgets Co.",
                "June 2023 – Present",
                "Software Engineer",
                "•\tBuilt the order pipeline in Rust, cutting checkout latency by 40%.",
                "•\tLed the migration of 12 services to Kubernetes.",
                "Skills",
                "Rust, TypeScript, Postgres",
            ]
        );
        assert_eq!(file.links, []);
    }

    #[test]
    fn markdown_reads_its_links_not_its_images() {
        let file = file(&fixture("import/jane-doe.md"));
        assert!(
            file.text
                .starts_with("# Jane Q. Doe\n\njane.doe@example.com"),
            "{}",
            file.text
        );
        assert!(
            file.text
                .ends_with("[LinkedIn](https://linkedin.com/in/janeqdoe-example)")
        );
        assert_eq!(
            file.links,
            [
                link("LinkedIn", "https://linkedin.com/in/janeqdoe-example"),
                link("GitHub", "https://github.com/janeqdoe-example"),
                link("Acme Widgets Co.", "https://example.com/acme"),
            ]
        );
        assert_eq!(file.pages, None);
    }

    #[test]
    fn text_is_normalized_and_plain_text_has_no_links() {
        let file = text_file(
            "\u{feff}\r\n  \r\n  Jane Doe  \r\n[Site](https://example.com)\t\r\n\r\n".into(),
            false,
        )
        .unwrap();
        assert_eq!(file.text, "Jane Doe\n[Site](https://example.com)");
        assert_eq!(file.links, []);
        assert!(matches!(
            text_file(" \r\n ".into(), true),
            Err(PipelineError::EmptyFile)
        ));
        assert!(matches!(
            text_file(vec![0xff, 0xfe, b'J'], false),
            Err(PipelineError::NotUtf8)
        ));
    }

    #[test]
    fn a_v1_master_json_reads_as_its_resume() {
        let path = fixture("banks/jane-doe.v1.json");
        let ResumeSource::AtsFit(resume) = read_resume(&path).unwrap() else {
            panic!("not read as a master.json");
        };
        assert_eq!(
            resume,
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
        );
    }

    #[test]
    fn a_json_that_isnt_a_v1_resume_says_so() {
        let err = read_resume(&fixture("score/jane-doe-swe.requirements.json")).unwrap_err();
        assert!(matches!(err, PipelineError::NotMasterJson(_)), "{err:?}");
        let message = err.to_string();
        assert!(
            message.starts_with("This file isn't a v1 resume JSON (")
                && message.ends_with(
                    "). Choose your resume file, or a JSON in gliide's v1 resume format."
                ),
            "{message}"
        );
    }

    #[test]
    fn other_extensions_are_refused_before_the_file_is_read() {
        for name in ["resume.doc", "resume.RTF", "resume.pages", "resume"] {
            let err = read_resume(Path::new(name)).unwrap_err();
            assert!(
                matches!(err, PipelineError::UnsupportedFile),
                "{name}: {err:?}"
            );
        }
        // Extensions are case-insensitive: this one is read, and isn't there.
        let err = read_resume(Path::new("no/such/resume.PDF")).unwrap_err();
        assert!(matches!(err, PipelineError::ReadFile { .. }), "{err:?}");
        assert!(
            err.to_string()
                .starts_with("Couldn't read no/such/resume.PDF: "),
            "{err}"
        );
    }

    /// `body` through a template of its own, as PDF bytes.
    fn pdf(body: &str) -> Vec<u8> {
        let template = Template {
            source: format!(
                "#set page(width: 8cm, height: auto, margin: 5mm)\n#metadata(1) <pages>\n{body}"
            ),
            manifest: Manifest {
                name: "test".into(),
                ats_safe: true,
                columns: 1,
            },
        };
        Renderer::new(&template, FontOptions::default())
            .render("{}", false)
            .unwrap()
            .pdf()
            .unwrap()
    }

    #[test]
    fn a_pdf_reads_its_text_links_and_pages() {
        let path = std::env::temp_dir().join(format!("gliide-import-{}.PDF", std::process::id()));
        std::fs::write(
            &path,
            pdf("Jane Doe\n\n#link(\"https://example.com\")[example.com]"),
        )
        .unwrap();
        let read = read_resume(&path);
        std::fs::remove_file(&path).unwrap();
        let ResumeSource::File(file) = read.unwrap() else {
            panic!("not read as a file");
        };
        assert_eq!(
            file,
            ResumeFile {
                text: "Jane Doe\nexample.com".into(),
                links: vec![link("example.com", "https://example.com")],
                pages: Some(1),
            }
        );
    }

    #[test]
    fn a_pdf_without_text_or_that_isnt_one_says_so() {
        assert!(matches!(
            pdf_file(pdf("#rect(width: 1cm, height: 1cm)")),
            Err(PipelineError::NoPdfText)
        ));
        assert!(matches!(
            pdf_file(b"not a pdf".to_vec()),
            Err(PipelineError::NotPdf)
        ));
    }

    #[test]
    fn a_zip_without_a_document_or_not_a_zip_isnt_a_docx() {
        for bytes in [
            zip(&[("notes.txt", "hi")]),
            b"PK\x03\x04 not a zip".to_vec(),
        ] {
            let err = docx_file(&bytes).unwrap_err();
            assert!(matches!(err, PipelineError::NotDocx(_)), "{err:?}");
        }
    }

    #[test]
    fn a_hyperlink_field_code_names_its_url() {
        assert_eq!(
            hyperlink_url(r#" HYPERLINK "https://example.com" \l "top" \o "Tip" "#),
            Some("https://example.com")
        );
        assert_eq!(
            hyperlink_url(r#"HYPERLINK \o "Tip" https://example.com"#),
            Some("https://example.com")
        );
        assert_eq!(hyperlink_url(r#"HYPERLINK \l "skills""#), None);
        assert_eq!(hyperlink_url("PAGE"), None);
    }
}
