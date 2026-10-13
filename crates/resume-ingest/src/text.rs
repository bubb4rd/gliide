//! HTML to posting text, and pasted text tidied.
//!
//! Paragraphs and headings become lines, list items `- ` lines (nested ones indented), entities are decoded and
//! whitespace collapsed; scripts, styles, hidden elements and page chrome are dropped. A heading, or a paragraph
//! that is all bold, gets a blank line before it so sections stay visible.

use ego_tree::NodeRef;
use scraper::{ElementRef, Html, Node};

/// Elements whose content is never posting text.
const ALWAYS_SKIP: &[&str] = &[
    "head", "script", "style", "noscript", "template", "svg", "iframe", "object", "button",
    "select", "dialog",
];
/// Page chrome, dropped when reading a whole page.
const CHROME: &[&str] = &["nav", "footer", "form", "aside"];
const BLOCKS: &[&str] = &[
    "p",
    "div",
    "section",
    "article",
    "main",
    "header",
    "ul",
    "ol",
    "dl",
    "dt",
    "dd",
    "table",
    "thead",
    "tbody",
    "tfoot",
    "tr",
    "blockquote",
    "pre",
    "figure",
    "figcaption",
    "hr",
    "address",
    "details",
    "summary",
    "center",
    "body",
    "html",
];
const HEADINGS: &[&str] = &["h1", "h2", "h3", "h4", "h5", "h6"];
/// An all-bold paragraph longer than this is emphasis, not a heading.
const MAX_HEADING: usize = 100;
/// Paragraphs that start with one of these are list items written by hand.
const BULLETS: &[char] = &[
    '•', '●', '▪', '◦', '·', '‣', '∙', '■', '□', '➢', '➤', '►', '*',
];

/// Text of an HTML fragment (a posting's description).
pub(crate) fn html_to_text(html: &str) -> String {
    let doc = Html::parse_fragment(html);
    let mut w = Writer::new(&[]);
    w.children(doc.tree.root());
    w.finish()
}

/// Text of an element of a whole page, without its navigation, footers, forms and asides. `header` is dropped too
/// unless the element is the page's `<main>` (whose header names the role).
pub(crate) fn page_text(el: ElementRef<'_>, keep_header: bool) -> String {
    let mut w = Writer::new(if keep_header {
        CHROME
    } else {
        &["nav", "footer", "form", "aside", "header"]
    });
    w.children(*el);
    w.finish()
}

/// Decoded text of an HTML-escaped string (`&lt;p&gt;` → `<p>`, `&amp;` → `&`), without interpreting tags.
pub(crate) fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    let doc = Html::parse_fragment(&format!("<textarea>{s}</textarea>"));
    doc.root_element().text().collect()
}

/// Whitespace tidied, text otherwise verbatim: line endings unified, runs of spaces and tabs collapsed, lines
/// trimmed, at most one blank line in a row.
pub(crate) fn tidy_pasted(s: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in s.lines() {
        let line = collapse(line);
        if line.is_empty() && out.last().is_none_or(String::is_empty) {
            continue;
        }
        out.push(line);
    }
    while out.last().is_some_and(String::is_empty) {
        out.pop();
    }
    out.join("\n")
}

/// Runs of whitespace (no-break spaces included) as one space, zero-width characters removed, ends trimmed.
fn collapse(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = false;
    for c in s.chars() {
        if is_invisible(c) {
            continue;
        }
        if c.is_whitespace() {
            space = !out.is_empty();
        } else {
            if space {
                out.push(' ');
                space = false;
            }
            out.push(c);
        }
    }
    out
}

fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{200b}' | '\u{200c}' | '\u{2060}' | '\u{feff}' | '\u{ad}'
    )
}

struct Writer {
    skip: &'static [&'static str],
    lines: Vec<String>,
    line: String,
    /// A space is owed before the next visible character.
    space: bool,
    /// The list-item marker for the line being built, until it is written.
    marker: Option<String>,
    list_depth: usize,
    bold: usize,
    /// Every visible character of the current line was bold.
    all_bold: bool,
    heading: bool,
}

impl Writer {
    fn new(skip: &'static [&'static str]) -> Self {
        Self {
            skip,
            lines: Vec::new(),
            line: String::new(),
            space: false,
            marker: None,
            list_depth: 0,
            bold: 0,
            all_bold: true,
            heading: false,
        }
    }

    fn children(&mut self, node: NodeRef<'_, Node>) {
        for child in node.children() {
            match child.value() {
                Node::Text(t) => self.text(&t.text),
                Node::Element(_) => {
                    if let Some(el) = ElementRef::wrap(child) {
                        self.element(el);
                    }
                }
                _ => {}
            }
        }
    }

    fn element(&mut self, el: ElementRef<'_>) {
        let e = el.value();
        let name = e.name();
        if ALWAYS_SKIP.contains(&name) || self.skip.contains(&name) || is_hidden(e) {
            return;
        }
        match name {
            "br" => self.break_line(),
            "li" => {
                self.break_line();
                let indent = "  ".repeat(self.list_depth.saturating_sub(1));
                self.marker = Some(format!("{indent}- "));
                self.children(*el);
                self.break_line();
                self.marker = None;
            }
            "ul" | "ol" => {
                self.break_line();
                self.list_depth += 1;
                self.children(*el);
                self.list_depth -= 1;
                self.break_line();
            }
            "b" | "strong" => {
                self.bold += 1;
                self.children(*el);
                self.bold -= 1;
            }
            "td" | "th" => {
                self.space = !self.line.is_empty();
                self.children(*el);
                self.space = !self.line.is_empty();
            }
            _ if HEADINGS.contains(&name) => {
                self.break_line();
                self.heading = true;
                self.children(*el);
                self.break_line();
                self.heading = false;
            }
            _ if BLOCKS.contains(&name) => {
                self.break_line();
                self.children(*el);
                self.break_line();
            }
            _ => self.children(*el),
        }
    }

    fn text(&mut self, t: &str) {
        for c in t.chars() {
            if is_invisible(c) {
                continue;
            }
            if c.is_whitespace() {
                self.space = !self.line.is_empty();
                continue;
            }
            if self.space {
                self.line.push(' ');
                self.space = false;
            }
            self.line.push(c);
            if self.bold == 0 {
                self.all_bold = false;
            }
        }
    }

    /// Ends the current line. An empty line writes nothing and keeps a pending list marker, so
    /// `<li><p>text</p></li>` is one item.
    fn break_line(&mut self) {
        self.space = false;
        if self.line.is_empty() {
            return;
        }
        let text = std::mem::take(&mut self.line);
        let all_bold = std::mem::replace(&mut self.all_bold, true);
        let (marker, text) = match self.marker.take() {
            Some(m) => (m, text.trim_start_matches(BULLETS).trim_start().to_owned()),
            None => match text.strip_prefix(BULLETS) {
                Some(rest) if rest.starts_with(' ') => {
                    ("- ".to_owned(), rest.trim_start().to_owned())
                }
                _ => (String::new(), text),
            },
        };
        if text.is_empty() {
            return;
        }
        let heading = marker.is_empty()
            && (self.heading || (all_bold && text.chars().count() <= MAX_HEADING));
        if heading && self.lines.last().is_some_and(|l| !l.is_empty()) {
            self.lines.push(String::new());
        }
        self.lines.push(marker + &text);
    }

    fn finish(mut self) -> String {
        self.break_line();
        self.lines.join("\n")
    }
}

/// `hidden`, `aria-hidden="true"` or an inline `display: none`.
fn is_hidden(e: &scraper::node::Element) -> bool {
    e.attr("hidden").is_some()
        || e.attr("aria-hidden") == Some("true")
        || e.attr("style").is_some_and(|s| {
            let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
            s.to_ascii_lowercase().contains("display:none")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_lists_and_entities() {
        let html = "<h2>About&nbsp;us</h2><p>We   build\n things &amp; ship <a href=x>them</a>.</p>\
                    <p><strong>What you'll do</strong></p><ul><li><p>Write Rust</p></li><li>Review\
                    <ul><li>nested</li></ul></li></ul><p>• Hand-made bullet</p><p>Line<br>break</p>\
                    <script>var x = 1;</script><style>p{}</style><p style=\"display: none\">hidden</p>";
        assert_eq!(
            html_to_text(html),
            "About us\nWe build things & ship them.\n\nWhat you'll do\n- Write Rust\n- Review\n  - nested\n\
             - Hand-made bullet\nLine\nbreak"
        );
    }

    #[test]
    fn inline_markup_keeps_words_together() {
        assert_eq!(
            html_to_text("<p><b>Java</b>Script and <em>Type</em>Script</p>"),
            "JavaScript and TypeScript"
        );
        assert_eq!(
            html_to_text("<p>a</p><p><br></p><p>&#x2022; b</p>"),
            "a\n- b"
        );
    }

    #[test]
    fn unescapes_once() {
        assert_eq!(unescape("&lt;p&gt;R&amp;amp;D&lt;/p&gt;"), "<p>R&amp;D</p>");
        assert_eq!(unescape("plain"), "plain");
    }

    #[test]
    fn pasted_text_whitespace() {
        let pasted = "\r\n  Senior\tEngineer  \r\n\r\n\r\n\r\n- Rust\u{a0}\u{a0}and Go \n\n";
        assert_eq!(tidy_pasted(pasted), "Senior Engineer\n\n- Rust and Go");
    }
}
