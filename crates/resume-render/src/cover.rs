//! The cover letter's template (`templates/cover-letter`, ATS Fit's `cover.typ`) and the data it reads. It renders
//! through a [`Renderer`](crate::Renderer) like a resume: `render(json, false)`, then `metrics.pages`, SVG pages and
//! the PDF.

use serde::Serialize;

use crate::{Manifest, Template};

/// `templates/cover-letter`, built in.
pub fn template() -> Template {
    Template {
        source: include_str!("../../../templates/cover-letter/letter.typ").to_owned(),
        manifest: serde_json::from_str::<Manifest>(include_str!(
            "../../../templates/cover-letter/manifest.json"
        ))
        .expect("cover-letter manifest is valid"),
    }
}

/// What the letter template reads (`sys.inputs.data`, as JSON).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LetterData<'a> {
    /// The letterhead: the resume's name and contact line.
    pub name: &'a str,
    pub contact: Vec<LetterContact<'a>>,
    /// As printed ("October 13, 2026").
    pub date: &'a str,
    pub company: &'a str,
    pub role: &'a str,
    pub greeting: &'a str,
    pub paragraphs: &'a [String],
    pub closing: &'a str,
}

/// One item of the contact line: its text, and its link (`""` for none; `mailto:` links print as plain text).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LetterContact<'a> {
    pub text: &'a str,
    pub url: &'a str,
}

#[cfg(test)]
mod tests {
    use super::{LetterContact, LetterData, template};
    use crate::{FontOptions, Renderer};

    #[test]
    fn a_long_letter_reports_its_pages() {
        let mut renderer = Renderer::new(&template(), FontOptions::default());
        let short = vec!["One paragraph.".to_owned(); 3];
        let data = |paragraphs: &[String]| {
            serde_json::to_string(&LetterData {
                name: "Jane Doe",
                contact: vec![LetterContact {
                    text: "jane@example.com",
                    url: "mailto:jane@example.com",
                }],
                date: "October 13, 2026",
                company: "Acme",
                role: "Software Engineer",
                greeting: "Dear Acme Hiring Team,",
                paragraphs,
                closing: "Sincerely,",
            })
            .unwrap()
        };
        let one = renderer.render(&data(&short), false).unwrap();
        assert_eq!(one.metrics.pages, 1);
        let long = vec!["A sentence that runs on and on. ".repeat(40); 4];
        assert_eq!(
            renderer.render(&data(&long), false).unwrap().metrics.pages,
            2
        );
    }
}
