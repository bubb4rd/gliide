//! The data a cover letter template (a built-in's `letter.typ`, [`Template::built_in_letter`](crate::Template)) reads.
//! It renders through a [`Renderer`](crate::Renderer) like a resume: `render(json, false)`, then `metrics.pages`, SVG
//! pages and the PDF.

use serde::Serialize;

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
    use super::{LetterContact, LetterData};
    use crate::{FontOptions, Renderer, Template};

    #[test]
    fn every_built_in_letter_renders_and_reports_its_pages() {
        let short = vec!["One paragraph.".to_owned(); 3];
        let long = vec!["A sentence that runs on and on. ".repeat(40); 4];
        let data = |paragraphs: &[String]| {
            serde_json::to_string(&LetterData {
                name: "Jane Doe",
                contact: vec![
                    LetterContact {
                        text: "jane@example.com",
                        url: "mailto:jane@example.com",
                    },
                    LetterContact {
                        text: "github.com/jane",
                        url: "https://github.com/jane",
                    },
                ],
                date: "October 13, 2026",
                company: "Acme",
                role: "Software Engineer",
                greeting: "Dear Acme Hiring Team,",
                paragraphs,
                closing: "Sincerely,",
            })
            .unwrap()
        };
        for name in Template::BUILT_IN {
            let letter = Template::built_in_letter(name).unwrap();
            let mut renderer = Renderer::new(&letter, FontOptions::default());
            let one = renderer.render(&data(&short), false).unwrap();
            assert_eq!(one.metrics.pages, 1, "{name}");
            let text = one.pdf_text().unwrap();
            for part in [
                "Jane Doe",
                "github.com/jane",
                "Re: Software Engineer",
                "Sincerely,",
            ] {
                assert!(text.contains(part), "{name}: {part:?} not in {text}");
            }
            let pages = renderer.render(&data(&long), false).unwrap().metrics.pages;
            assert_eq!(pages, 2, "{name}");
        }
    }
}
