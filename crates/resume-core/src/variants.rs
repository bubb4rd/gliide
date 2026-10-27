//! Variants: other phrasings of a bullet that you wrote and approved (ARCHITECTURE.md §2). A master shows one wording
//! of each bullet it holds, its text or a variant by index, and Suggest prefers choosing one of them over writing new
//! words. These edits keep every master showing the words it showed, whatever moves: removing a variant or making one
//! the bullet's text renumbers what masters point at, and folding a bullet into another moves the masters that showed
//! it onto the same words, now a variant of the other. A new bullet (Dig's add-bullet) joins its entry in no master, and
//! a bullet's skills are set whole.

use crate::bank::{Bank, BankBullet};
use crate::master::{Master, MasterBullet};
use crate::validate::same_text;

/// Which wording of a bullet: its text (`None`) or its variant `i`.
pub type Wording = Option<usize>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VariantError {
    #[error("Your bank has no bullet `{0}`")]
    Bullet(String),
    #[error("That bullet has no variant {0}")]
    Variant(usize),
    #[error("A wording can't be empty")]
    Empty,
    #[error("That bullet already reads that way")]
    Duplicate,
    #[error("Only bullets of the same entry can be folded together")]
    OtherEntry,
    #[error("{0} shows both bullets: take one of them out of it first")]
    BothShown(String),
    #[error("Your bank has no entry `{0}`")]
    Entry(String),
}

/// Adds a bullet reading `text` at the end of entry `entry`, in no master yet: its id, the next free `{entry}.{n}`
/// in the bank (as an import numbers them). Refuses a text one of the entry's bullets already reads.
pub fn add_bullet(bank: &mut Bank, entry: &str, text: &str) -> Result<String, VariantError> {
    let text = wording_text(text)?;
    let taken: std::collections::HashSet<String> = bank
        .sections
        .iter()
        .flat_map(|s| &s.entries)
        .flat_map(|e| &e.bullets)
        .map(|b| b.id.clone())
        .collect();
    let e = bank
        .sections
        .iter_mut()
        .flat_map(|s| &mut s.entries)
        .find(|e| e.id == entry)
        .ok_or_else(|| VariantError::Entry(entry.to_owned()))?;
    if e.bullets.iter().flat_map(wordings).any(|w| same_text(w, &text)) {
        return Err(VariantError::Duplicate);
    }
    let id = (1..)
        .map(|n| format!("{entry}.{n}"))
        .find(|id| !taken.contains(id))
        .expect("some number is free");
    e.bullets.push(BankBullet {
        id: id.clone(),
        text,
        variants: vec![],
        skills: vec![],
    });
    Ok(id)
}

/// Sets the skills `bullet` demonstrates to `skills`: each trimmed (inner spacing collapsed), empty ones dropped, each
/// once ignoring case, in order.
pub fn set_skills(bank: &mut Bank, bullet: &str, skills: &[String]) -> Result<(), VariantError> {
    let b = bullet_mut(bank, bullet)?;
    let mut out: Vec<String> = Vec::with_capacity(skills.len());
    for s in skills {
        let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
        if !s.is_empty() && !out.iter().any(|x| x.eq_ignore_ascii_case(&s)) {
            out.push(s);
        }
    }
    b.skills = out;
    Ok(())
}

/// Adds `text` as a variant of `bullet`: its index.
pub fn add_variant(bank: &mut Bank, bullet: &str, text: &str) -> Result<usize, VariantError> {
    let text = wording_text(text)?;
    let b = bullet_mut(bank, bullet)?;
    if wordings(b).any(|w| same_text(w, &text)) {
        return Err(VariantError::Duplicate);
    }
    b.variants.push(text);
    Ok(b.variants.len() - 1)
}

/// Sets one wording of `bullet` to `text`: every master that shows that wording shows the new words.
pub fn set_wording(
    bank: &mut Bank,
    bullet: &str,
    wording: Wording,
    text: &str,
) -> Result<(), VariantError> {
    let text = wording_text(text)?;
    let b = bullet_mut(bank, bullet)?;
    let duplicate = wordings(b)
        .enumerate()
        .any(|(i, w)| as_wording(i) != wording && same_text(w, &text));
    if duplicate {
        return Err(VariantError::Duplicate);
    }
    let slot = match wording {
        None => &mut b.text,
        Some(i) => b.variants.get_mut(i).ok_or(VariantError::Variant(i))?,
    };
    *slot = text;
    Ok(())
}

/// Removes variant `i` of `bullet`. A master that showed it shows the bullet's text; later variants move down one.
pub fn remove_variant(
    bank: &mut Bank,
    masters: &mut [Master],
    bullet: &str,
    i: usize,
) -> Result<(), VariantError> {
    let b = bullet_mut(bank, bullet)?;
    if i >= b.variants.len() {
        return Err(VariantError::Variant(i));
    }
    b.variants.remove(i);
    for mb in shown_mut(masters, bullet) {
        mb.variant = match mb.variant {
            Some(v) if v == i => None,
            Some(v) if v > i => Some(v - 1),
            v => v,
        };
    }
    Ok(())
}

/// Makes variant `i` the text of `bullet`, and its text variant `i`. Every master keeps the words it showed.
pub fn make_main(
    bank: &mut Bank,
    masters: &mut [Master],
    bullet: &str,
    i: usize,
) -> Result<(), VariantError> {
    let b = bullet_mut(bank, bullet)?;
    let v = b.variants.get_mut(i).ok_or(VariantError::Variant(i))?;
    std::mem::swap(&mut b.text, v);
    for mb in shown_mut(masters, bullet) {
        mb.variant = match mb.variant {
            None => Some(i),
            Some(v) if v == i => None,
            v => v,
        };
    }
    Ok(())
}

/// Folds bullet `from` into `into`, a bullet of the same entry: `from`'s wordings become variants of `into` (one
/// already among them is reused), its skills join `into`'s, and it leaves the bank. Masters that showed `from` show the
/// same words as a variant of `into`; a master showing both can't be folded without losing a line, so it's refused.
pub fn fold_into(
    bank: &mut Bank,
    masters: &mut [Master],
    from: &str,
    into: &str,
) -> Result<(), VariantError> {
    if from == into {
        return Err(VariantError::OtherEntry);
    }
    let entry = bank
        .sections
        .iter_mut()
        .flat_map(|s| &mut s.entries)
        .find(|e| e.bullets.iter().any(|b| b.id == from))
        .ok_or_else(|| VariantError::Bullet(from.to_owned()))?;
    let Some(to) = entry.bullets.iter().position(|b| b.id == into) else {
        return Err(VariantError::OtherEntry);
    };
    if let Some(m) = masters.iter().find(|m| {
        let shows = |id: &str| bullets(m).any(|b| b.id == id);
        shows(from) && shows(into)
    }) {
        return Err(VariantError::BothShown(m.title.clone()));
    }
    let at = entry
        .bullets
        .iter()
        .position(|b| b.id == from)
        .expect("found above");
    let folded = entry.bullets.remove(at);
    let target = &mut entry.bullets[if to > at { to - 1 } else { to }];
    // Where each of `from`'s wordings lands among `into`'s.
    let mut map: Vec<Wording> = Vec::with_capacity(folded.variants.len() + 1);
    for w in std::iter::once(&folded.text).chain(&folded.variants) {
        let found = wordings(target)
            .position(|x| same_text(x, w))
            .map(|i| i.checked_sub(1));
        map.push(match found {
            Some(w) => w,
            None => {
                target.variants.push(w.clone());
                Some(target.variants.len() - 1)
            }
        });
    }
    for s in folded.skills {
        if !target.skills.iter().any(|x| x.eq_ignore_ascii_case(&s)) {
            target.skills.push(s);
        }
    }
    for mb in shown_mut(masters, from) {
        let w = mb.variant.map_or(0, |v| v + 1);
        *mb = MasterBullet {
            id: into.to_owned(),
            variant: map.get(w).copied().flatten(),
        };
    }
    Ok(())
}

/// Makes `master` show `wording` of `bullet`. False when the master doesn't show the bullet (nothing changes).
pub fn show(
    bank: &Bank,
    master: &mut Master,
    bullet: &str,
    wording: Wording,
) -> Result<bool, VariantError> {
    let b = bank
        .sections
        .iter()
        .flat_map(|s| &s.entries)
        .flat_map(|e| &e.bullets)
        .find(|b| b.id == bullet)
        .ok_or_else(|| VariantError::Bullet(bullet.to_owned()))?;
    if let Some(i) = wording.filter(|&i| i >= b.variants.len()) {
        return Err(VariantError::Variant(i));
    }
    let mut shown = false;
    for mb in shown_mut(std::slice::from_mut(master), bullet) {
        mb.variant = wording;
        shown = true;
    }
    Ok(shown)
}

/// The bullet's text, then its variants.
fn wordings(b: &BankBullet) -> impl Iterator<Item = &String> {
    std::iter::once(&b.text).chain(&b.variants)
}

/// Position `i` in [`wordings`] as a [`Wording`].
fn as_wording(i: usize) -> Wording {
    i.checked_sub(1)
}

fn wording_text(text: &str) -> Result<String, VariantError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(VariantError::Empty);
    }
    Ok(text.to_owned())
}

fn bullet_mut<'a>(bank: &'a mut Bank, id: &str) -> Result<&'a mut BankBullet, VariantError> {
    bank.sections
        .iter_mut()
        .flat_map(|s| &mut s.entries)
        .flat_map(|e| &mut e.bullets)
        .find(|b| b.id == id)
        .ok_or_else(|| VariantError::Bullet(id.to_owned()))
}

fn bullets(m: &Master) -> impl Iterator<Item = &MasterBullet> {
    m.sections
        .iter()
        .flat_map(|s| &s.entries)
        .flat_map(|e| &e.bullets)
}

/// Every place a master shows `bullet`.
fn shown_mut<'a>(
    masters: &'a mut [Master],
    bullet: &'a str,
) -> impl Iterator<Item = &'a mut MasterBullet> {
    masters
        .iter_mut()
        .flat_map(|m| &mut m.sections)
        .flat_map(|s| &mut s.entries)
        .flat_map(|e| &mut e.bullets)
        .filter(move |b| b.id == bullet)
}
