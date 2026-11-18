//! Write: a cover letter per posting from the resolved `swe` master, untailored.

use resume_core::Requirements;
use resume_llm::cover::Letter;

use crate::Item;

/// The company and role a letter or prep is for: the posting's hints, else its requirements.
pub fn job(item: &Item, req: &Requirements) -> (String, String) {
    let pick = |hint: &Option<String>, from: &String| hint.clone().unwrap_or_else(|| from.clone());
    (
        pick(&item.posting.company, &req.company),
        pick(&item.posting.title, &req.role),
    )
}

/// Words in the greeting, paragraphs and closing.
pub fn words(l: &Letter) -> usize {
    std::iter::once(&l.greeting)
        .chain(&l.paragraphs)
        .chain(std::iter::once(&l.closing))
        .map(|s| s.split_whitespace().count())
        .sum()
}
