//! The resume's text as an ATS parser reads it off the page, in reading order.

use crate::resume::{Resume, Section};

/// The page's text in reading order: the name, the contact line, then each section's title, its entries (name and
/// place, dates, subtitle, notes, bullets) and its skills lines. One line per printed line or cell.
pub fn resume_text(r: &Resume) -> String {
    blocks(r)
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>()
        .join("\n")
}

/// [`resume_text`] in blocks: the header (name and contact, section `None`), then one block per section, with its id.
pub(crate) fn blocks(r: &Resume) -> Vec<(Option<&str>, String)> {
    let contact = r
        .contact
        .iter()
        .map(|c| c.text.as_str())
        .collect::<Vec<_>>()
        .join(" | ");
    let mut out = vec![(None, format!("{}\n{contact}", r.name))];
    out.extend(
        r.sections
            .iter()
            .map(|s| (Some(s.id.as_str()), section_text(s))),
    );
    out
}

/// One section's lines, as in [`resume_text`].
pub(crate) fn section_text(s: &Section) -> String {
    let mut lines = vec![s.title.clone()];
    for e in &s.entries {
        lines.push(if e.place.is_empty() {
            e.name.clone()
        } else {
            format!("{}, {}", e.name, e.place)
        });
        for line in [&e.dates, &e.subtitle] {
            if !line.is_empty() {
                lines.push(line.clone());
            }
        }
        for n in &e.notes {
            lines.push(if n.label.is_empty() {
                n.text.clone()
            } else {
                format!("{}: {}", n.label, n.text)
            });
        }
        lines.extend(e.bullets.iter().map(|b| b.text.clone()));
    }
    for g in &s.groups {
        lines.push(format!("{}: {}", g.label, g.items.join(", ")));
    }
    lines.join("\n")
}
