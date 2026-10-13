//! Suggestions into changes (ARCHITECTURE.md §5), as ATS Fit's `changes_of` and `draft_changes`.
//!
//! | Suggestion | Change |
//! |---|---|
//! | `bullets[]` rewrite / add / cut | `rewrite` (`use_variant` when it is one of the bullet's variants) / `add` / `cut` |
//! | `keywords[]` on a note line | add → `note_add`; edit / drop → `swap` |
//! | `keywords[]` on a bullet with no rewrite or cut | edit / drop → `swap` (dropped when the bullet is rewritten) |
//! | `keywords[]` on a Skills line | add / edit / drop → `skill_add` / `skill_rename` / `skill_remove` |
//! | `skills[]` | `skill_add` / `skill_remove` / `skill_rename` / `skill_move` |
//! | `bank[]` (bank-aware only) | include → `include`; swap → `swap_entry` |
//! | `order[]` | `reorder` |
//! | `summary` | `summary` |

use std::collections::{BTreeMap, BTreeSet};

use crate::apply::{
    appended, bank_bullet, bank_entry, find_phrase, note_ref, position_ci, summary_text,
    swap_phrase,
};
use crate::bank::{Bank, SectionKind};
use crate::change::{self, Change, Draft, Mode, Op, Origin};
use crate::master::Master;
use crate::matcher::{self, Page};
use crate::requirements::Requirements;
use crate::resolve::resolve;
use crate::resume::{Group, Resume};
use crate::suggestions::{BankOp, BulletOp, Have, KeywordOp, SkillOp, Suggestions};
use crate::validate::{Grounding, same_text, stale, swap_warning};

/// The changes `s` proposes to `master`, each naming a line of the resolved master, with the grounding validator's
/// findings in `flags`. Edits whose line the master doesn't show are dropped, as are no-ops (a rewrite to the same text,
/// a term the line already has), keyword edits on a bullet that is rewritten or cut (the rewrite carries them), skill
/// additions whose evidence is `no`, and, in [`Mode::MasterOnly`], every bank edit. `before` is always read from the
/// master. Identical keys are kept once, the first.
///
/// A master that doesn't resolve against `bank` has no lines, so no changes.
pub fn changes_of(
    s: &Suggestions,
    bank: &Bank,
    master: &Master,
    req: &Requirements,
    mode: Mode,
) -> Vec<Change> {
    let Ok(resume) = resolve(bank, master) else {
        return vec![];
    };
    let cx = Cx {
        ix: Index::new(&resume),
        bank,
        s,
        req,
        ground: Grounding::new(bank, s, req),
    };
    let mut out = Out::default();
    let rewritten = cx.bullets(&mut out);
    cx.keywords(&mut out, &rewritten);
    cx.skills(&mut out);
    if mode == Mode::BankAware {
        cx.bank_edits(&mut out);
    }
    cx.order(&mut out, mode);
    cx.summary(&mut out, &resume);
    out.changes
}

/// `changes` with your work layered on: your wording for a change replaces its text (a rewrite, variant, swap or note
/// addition becomes a `set` of its line; an `add`, Skills item or summary keeps its op with your text), keeping the
/// key, marked yours and no longer flagged; then your own changes, marked yours. Empty wording is ignored, and wording
/// for an op with no text (a cut, an include…) changes nothing.
pub fn draft_changes(changes: &[Change], draft: &Draft) -> Vec<Change> {
    changes
        .iter()
        .map(|c| match draft.wording.get(&c.key).map(|t| t.trim()) {
            Some(t) if !t.is_empty() => worded(c, t),
            _ => c.clone(),
        })
        .chain(draft.custom.iter().map(|c| Change {
            origin: Origin::Yours,
            ..c.clone()
        }))
        .collect()
}

fn worded(c: &Change, text: &str) -> Change {
    let text = text.to_owned();
    let op = match &c.op {
        Op::Rewrite { bullet, .. } | Op::UseVariant { bullet, .. } => Op::Set {
            line: bullet.clone(),
            text,
        },
        Op::Swap { line, .. } | Op::Set { line, .. } => Op::Set {
            line: line.clone(),
            text,
        },
        Op::NoteAdd { note, .. } => Op::Set {
            line: note.clone(),
            text,
        },
        Op::Add { entry, .. } => Op::Add {
            entry: entry.clone(),
            text,
        },
        Op::SkillAdd { group, .. } => Op::SkillAdd {
            group: group.clone(),
            item: text,
        },
        Op::SkillRename { group, from, .. } => Op::SkillRename {
            group: group.clone(),
            from: from.clone(),
            to: text,
        },
        Op::Summary { .. } => Op::Summary { text },
        _ => return c.clone(),
    };
    let new_text = match &op {
        Op::Set { text, .. } | Op::Add { text, .. } | Op::Summary { text } => text,
        Op::SkillAdd { item, .. } => item,
        Op::SkillRename { to, .. } => to,
        _ => unreachable!("worded ops carry text"),
    };
    let page = Page::of_text(new_text);
    let hits = c
        .hits
        .iter()
        .filter(|h| matcher::find_requirement(&page, h, &[]).0.with_near())
        .cloned()
        .collect();
    Change {
        key: c.key.clone(),
        op,
        origin: Origin::Yours,
        before: c.before.clone(),
        hits,
        why: c.why.clone(),
        priority: c.priority,
        section: c.section.clone(),
        flags: vec![],
    }
}

#[derive(Default)]
struct Out {
    changes: Vec<Change>,
    keys: BTreeSet<String>,
}

impl Out {
    /// Keeps `c` unless its key is taken. New bullets under one entry share a target, so the second and later get
    /// their ordinal in the key (`hash(add, entry, "#2")`): stable as long as the suggestions list them in that order.
    fn push(&mut self, mut c: Change) {
        if matches!(c.op, Op::Add { .. }) {
            let mut n = 1;
            while self.keys.contains(&c.key) {
                n += 1;
                c.key = change::key(&c.op, &format!("{}#{n}", c.before));
            }
        }
        if self.keys.insert(c.key.clone()) {
            self.changes.push(c);
        }
    }
}

/// A line of the resolved master: its text and section.
struct LineAt<'a> {
    text: &'a str,
    section: &'a str,
}

/// The resolved master's lines by id.
struct Index<'a> {
    bullets: BTreeMap<&'a str, LineAt<'a>>,
    notes: BTreeMap<String, LineAt<'a>>,
    /// Entry id → section id.
    entries: BTreeMap<&'a str, &'a str>,
    groups: BTreeMap<&'a str, (&'a Group, &'a str)>,
    /// Section id → its entry ids, in order.
    sections: BTreeMap<&'a str, Vec<&'a str>>,
}

impl<'a> Index<'a> {
    fn new(r: &'a Resume) -> Self {
        let mut ix = Index {
            bullets: BTreeMap::new(),
            notes: BTreeMap::new(),
            entries: BTreeMap::new(),
            groups: BTreeMap::new(),
            sections: BTreeMap::new(),
        };
        for s in &r.sections {
            let section = s.id.as_str();
            ix.sections
                .insert(section, s.entries.iter().map(|e| e.id.as_str()).collect());
            for e in &s.entries {
                ix.entries.insert(&e.id, section);
                for b in &e.bullets {
                    ix.bullets.insert(
                        &b.id,
                        LineAt {
                            text: &b.text,
                            section,
                        },
                    );
                }
                for (i, n) in e.notes.iter().enumerate() {
                    ix.notes.insert(
                        format!("{}#{}", e.id, i + 1),
                        LineAt {
                            text: &n.text,
                            section,
                        },
                    );
                }
            }
            for g in &s.groups {
                ix.groups.insert(&g.id, (g, section));
            }
        }
        ix
    }
}

struct Cx<'a> {
    ix: Index<'a>,
    bank: &'a Bank,
    s: &'a Suggestions,
    req: &'a Requirements,
    ground: Grounding<'a>,
}

impl Cx<'_> {
    /// Bullet rewrites, cuts and additions; the ids of bullets rewritten or cut.
    fn bullets(&self, out: &mut Out) -> BTreeSet<String> {
        let mut rewritten = BTreeSet::new();
        for b in &self.s.bullets {
            let id = b.id.trim();
            let after = b.after.trim();
            let priority = b.priority.clamp(1, 3);
            match b.op {
                BulletOp::Rewrite | BulletOp::Cut => {
                    let Some(line) = self.ix.bullets.get(id) else {
                        continue;
                    };
                    let (op, written) = if b.op == BulletOp::Cut {
                        (Op::Cut { bullet: id.into() }, None)
                    } else {
                        if after.is_empty() {
                            continue;
                        }
                        let variants =
                            bank_bullet(self.bank, id).map_or(&[][..], |bb| &bb.variants);
                        let v = b
                            .variant
                            .map(|v| v as usize)
                            .filter(|&v| v < variants.len())
                            .or_else(|| variants.iter().position(|t| same_text(t, after)));
                        let text = v.map_or(after, |v| variants[v].as_str());
                        if same_text(text, line.text) {
                            continue;
                        }
                        match v {
                            Some(variant) => (
                                Op::UseVariant {
                                    bullet: id.into(),
                                    variant,
                                },
                                None,
                            ),
                            None => (
                                Op::Rewrite {
                                    bullet: id.into(),
                                    text: after.into(),
                                },
                                Some(after),
                            ),
                        }
                    };
                    let mut c = Change::new(
                        op,
                        Origin::Suggested,
                        line.text.into(),
                        b.hits.clone(),
                        b.why.clone(),
                        priority,
                        line.section.into(),
                    );
                    c.flags.extend(stale(&b.before, line.text));
                    if let Some(after) = written {
                        c.flags
                            .extend(self.sourced(id, after, &b.sources, Some(line.text)));
                    }
                    rewritten.insert(id.to_owned());
                    out.push(c);
                }
                BulletOp::Add => {
                    let Some(&section) = self.ix.entries.get(id) else {
                        continue;
                    };
                    if after.is_empty() {
                        continue;
                    }
                    let mut c = Change::new(
                        Op::Add {
                            entry: id.into(),
                            text: after.into(),
                        },
                        Origin::Suggested,
                        String::new(),
                        b.hits.clone(),
                        b.why.clone(),
                        priority,
                        section.into(),
                    );
                    c.flags = self.sourced(id, after, &b.sources, None);
                    out.push(c);
                }
            }
        }
        rewritten
    }

    /// Findings for `text`, written for `line` (a bullet, or the entry a new bullet goes under), drawn from the lines
    /// `sources` (and `replaces`): ids that don't exist, sources under another entry, then the grounding checks against
    /// those lines.
    fn sourced(
        &self,
        line: &str,
        text: &str,
        sources: &[String],
        replaces: Option<&str>,
    ) -> Vec<String> {
        let mut flags = Vec::new();
        let mut lines: Vec<String> = replaces.map(str::to_owned).into_iter().collect();
        for id in sources.iter().map(|s| s.trim()) {
            match source_text(self.bank, id) {
                Some(t) => lines.push(t),
                None => flags.push(format!("cites “{id}”, which is not in your bank")),
            }
            if id != line {
                flags.extend(self.ground.cross_entry(line, id, text));
            }
        }
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        flags.extend(self.ground.check(text, &refs));
        flags
    }

    fn keywords(&self, out: &mut Out, rewritten: &BTreeSet<String>) {
        for k in &self.s.keywords {
            let line = k.line.trim();
            let term = k.term.trim();
            if term.is_empty() {
                continue;
            }
            let priority = k.priority.clamp(1, 3);
            let hits = self.hits(&k.skill, term);
            let on_line = self.ix.notes.get(line).map(|l| (l, true)).or_else(|| {
                self.ix
                    .bullets
                    .get(line)
                    .filter(|_| !rewritten.contains(line))
                    .map(|l| (l, false))
            });
            if let Some((l, is_note)) = on_line {
                let c = match k.op {
                    KeywordOp::Add => {
                        if !is_note
                            || self.denied(&k.skill)
                            || self.denied(term)
                            || find_phrase(l.text, term).is_some()
                        {
                            continue;
                        }
                        let mut c = Change::new(
                            Op::NoteAdd {
                                note: line.into(),
                                term: term.into(),
                            },
                            Origin::Suggested,
                            l.text.into(),
                            hits,
                            k.why.clone(),
                            priority,
                            l.section.into(),
                        );
                        c.flags = self.ground.check(&appended(l.text, term), &[l.text]);
                        c
                    }
                    KeywordOp::Edit => {
                        if self.denied(&k.skill) || self.denied(term) {
                            continue;
                        }
                        let Some(c) = self.swap(line, l, &k.from, term, hits, &k.why, priority)
                        else {
                            continue;
                        };
                        c
                    }
                    KeywordOp::Drop => {
                        let Some(c) = self.swap(line, l, term, "", vec![], &k.why, priority) else {
                            continue;
                        };
                        c
                    }
                };
                out.push(c);
            } else if let Some(&(g, section)) = self.ix.groups.get(line) {
                match k.op {
                    KeywordOp::Add => {
                        if let Some(c) =
                            self.skill_add(g, section, term, &k.skill, &k.why, priority)
                        {
                            out.push(c);
                        }
                    }
                    KeywordOp::Edit => {
                        if let Some(c) =
                            self.skill_rename(g, section, &k.from, term, &k.skill, &k.why, priority)
                        {
                            out.push(c);
                        }
                    }
                    // One cut may name several items ("Swift, SwiftUI"); each on the line is its own change.
                    KeywordOp::Drop => {
                        for t in term.split(',') {
                            if let Some(i) = position_ci(&g.items, t) {
                                out.push(Change::new(
                                    Op::SkillRemove {
                                        group: g.id.clone(),
                                        item: g.items[i].clone(),
                                    },
                                    Origin::Suggested,
                                    g.items[i].clone(),
                                    vec![],
                                    k.why.clone(),
                                    priority,
                                    section.into(),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    /// `from` → `to` in place on line `id`; `None` for an empty `from` or a swap to the same words.
    #[allow(clippy::too_many_arguments)]
    fn swap(
        &self,
        id: &str,
        l: &LineAt<'_>,
        from: &str,
        to: &str,
        hits: Vec<String>,
        why: &str,
        priority: u8,
    ) -> Option<Change> {
        let (from, to) = (from.trim(), to.trim());
        if from.is_empty() || matcher::key(from) == matcher::key(to) {
            return None;
        }
        let mut c = Change::new(
            Op::Swap {
                line: id.into(),
                from: from.into(),
                to: to.into(),
            },
            Origin::Suggested,
            l.text.into(),
            hits,
            why.into(),
            priority,
            l.section.into(),
        );
        match swap_phrase(l.text, from, to) {
            Some(new) => c.flags.extend(self.ground.check(&new, &[l.text])),
            None => c.flags.push(format!("“{from}” is not in this line")),
        }
        c.flags.extend(swap_warning(from, to));
        Some(c)
    }

    fn skills(&self, out: &mut Out) {
        for e in &self.s.skills {
            let Some(&(g, section)) = self.ix.groups.get(e.group.trim()) else {
                continue;
            };
            let item = e.item.trim();
            if item.is_empty() {
                continue;
            }
            let priority = e.priority.clamp(1, 3);
            let c = match e.op {
                SkillOp::Add => self.skill_add(g, section, item, &e.skill, &e.why, priority),
                SkillOp::Rename => {
                    self.skill_rename(g, section, item, &e.to, &e.skill, &e.why, priority)
                }
                SkillOp::Remove => {
                    let at = position_ci(&g.items, item);
                    let item = at.map_or(item, |i| g.items[i].as_str());
                    let mut c = Change::new(
                        Op::SkillRemove {
                            group: g.id.clone(),
                            item: item.into(),
                        },
                        Origin::Suggested,
                        item.into(),
                        vec![],
                        e.why.clone(),
                        priority,
                        section.into(),
                    );
                    if at.is_none() {
                        c.flags.push(format!("“{item}” is not on this skills line"));
                    }
                    Some(c)
                }
                SkillOp::Move => self.ix.groups.get(e.to.trim()).and_then(|&(dest, _)| {
                    if dest.id == g.id {
                        return None;
                    }
                    let at = position_ci(&g.items, item);
                    let item = at.map_or(item, |i| g.items[i].as_str());
                    let mut c = Change::new(
                        Op::SkillMove {
                            from: g.id.clone(),
                            to: dest.id.clone(),
                            item: item.into(),
                        },
                        Origin::Suggested,
                        item.into(),
                        vec![],
                        e.why.clone(),
                        priority,
                        section.into(),
                    );
                    if at.is_none() {
                        c.flags.push(format!("“{item}” is not on this skills line"));
                    }
                    Some(c)
                }),
            };
            if let Some(c) = c {
                out.push(c);
            }
        }
    }

    /// `item` added to skills line `g`; `None` when the line has it or the evidence says you don't.
    fn skill_add(
        &self,
        g: &Group,
        section: &str,
        item: &str,
        skill: &str,
        why: &str,
        priority: u8,
    ) -> Option<Change> {
        if self.denied(skill) || self.denied(item) || position_ci(&g.items, item).is_some() {
            return None;
        }
        let mut c = Change::new(
            Op::SkillAdd {
                group: g.id.clone(),
                item: item.into(),
            },
            Origin::Suggested,
            String::new(),
            self.hits(skill, item),
            why.into(),
            priority,
            section.into(),
        );
        c.flags = self.ground.check_item(item);
        Some(c)
    }

    /// Item `from` on skills line `g` reworded as `to`; `None` for no new wording or one the evidence says you lack.
    #[allow(clippy::too_many_arguments)]
    fn skill_rename(
        &self,
        g: &Group,
        section: &str,
        from: &str,
        to: &str,
        skill: &str,
        why: &str,
        priority: u8,
    ) -> Option<Change> {
        let (from, to) = (from.trim(), to.trim());
        if from.is_empty()
            || to.is_empty()
            || matcher::key(from) == matcher::key(to)
            || self.denied(skill)
            || self.denied(to)
        {
            return None;
        }
        let at = position_ci(&g.items, from);
        let from = at.map_or(from, |i| g.items[i].as_str());
        let mut c = Change::new(
            Op::SkillRename {
                group: g.id.clone(),
                from: from.into(),
                to: to.into(),
            },
            Origin::Suggested,
            from.into(),
            self.hits(skill, to),
            why.into(),
            priority,
            section.into(),
        );
        if at.is_none() {
            c.flags.push(format!("“{from}” is not on this skills line"));
        }
        c.flags.extend(self.ground.check_item(to));
        Some(c)
    }

    fn bank_edits(&self, out: &mut Out) {
        for e in &self.s.bank {
            let id = e.entry.trim();
            let Some((_, be)) = bank_entry(self.bank, id) else {
                continue;
            };
            if self.ix.entries.contains_key(id) {
                continue;
            }
            let mut flags = Vec::new();
            let mut bullets = Vec::new();
            for b in &e.bullets {
                if be.bullets.iter().any(|x| x.id == b.trim()) {
                    bullets.push(b.trim().to_owned());
                } else {
                    flags.push(format!("“{}” is not a bullet of {}", b.trim(), be.name));
                }
            }
            let (op, before) = match e.op {
                BankOp::Include => (
                    Op::Include {
                        entry: id.into(),
                        bullets,
                    },
                    String::new(),
                ),
                BankOp::Swap => {
                    let out_id = e.replaces.trim();
                    if !self.ix.entries.contains_key(out_id) {
                        continue;
                    }
                    let Some((_, oe)) = bank_entry(self.bank, out_id) else {
                        continue;
                    };
                    (
                        Op::SwapEntry {
                            out: out_id.into(),
                            entry: id.into(),
                            bullets,
                        },
                        oe.name.clone(),
                    )
                }
            };
            let mut c = Change::new(
                op,
                Origin::Suggested,
                before,
                e.skills.clone(),
                e.why.clone(),
                3,
                "bank".into(),
            );
            c.flags = flags;
            out.push(c);
        }
    }

    fn order(&self, out: &mut Out, mode: Mode) {
        for o in &self.s.order {
            let section = o.section.trim();
            let Some(current) = self.ix.sections.get(section) else {
                continue;
            };
            let in_bank = |id: &str| {
                mode == Mode::BankAware
                    && self
                        .bank
                        .sections
                        .iter()
                        .any(|s| s.id == section && s.entries.iter().any(|e| e.id == id))
            };
            let mut flags = Vec::new();
            let mut listed: Vec<&str> = Vec::new();
            for id in o.entries.iter().map(|e| e.trim()) {
                if current.contains(&id) {
                    listed.push(id);
                } else if !in_bank(id) {
                    flags.push(format!("“{id}” is not in this section"));
                }
            }
            let mut result = listed.clone();
            result.extend(current.iter().filter(|id| !listed.contains(id)));
            let extra = o.entries.len() > listed.len();
            if result == *current && !extra {
                continue;
            }
            let mut c = Change::new(
                Op::Reorder {
                    section: section.into(),
                    entries: o.entries.iter().map(|e| e.trim().to_owned()).collect(),
                },
                Origin::Suggested,
                current.join(", "),
                vec![],
                String::new(),
                3,
                section.into(),
            );
            c.flags = flags;
            out.push(c);
        }
    }

    fn summary(&self, out: &mut Out, r: &Resume) {
        let Some(text) = self.s.summary.as_deref().map(str::trim) else {
            return;
        };
        let before = summary_text(r).unwrap_or("");
        if text.is_empty() || same_text(text, before) {
            return;
        }
        let section = r
            .sections
            .iter()
            .find(|s| s.kind == Some(SectionKind::Summary))
            .map_or("summary", |s| s.id.as_str());
        let page = Page::of_text(text);
        let hits = self
            .req
            .skills
            .iter()
            .filter(|k| matcher::find_literal(&page, &k.term, &k.aliases).is_some())
            .map(|k| k.term.clone())
            .collect();
        let mut c = Change::new(
            Op::Summary { text: text.into() },
            Origin::Suggested,
            before.into(),
            hits,
            String::new(),
            2,
            section.into(),
        );
        c.flags = self.ground.check(text, &[self.ground.bank_text()]);
        out.push(c);
    }

    /// The requirement terms a keyword or Skills edit states: its `skill`, else `term` when it is a requirement skill.
    fn hits(&self, skill: &str, term: &str) -> Vec<String> {
        if !skill.trim().is_empty() {
            return vec![skill.trim().to_owned()];
        }
        self.requirement(term)
            .map(|t| vec![t.to_owned()])
            .unwrap_or_default()
    }

    /// The requirement skill `name` is (its term or one of its aliases), as the requirements word it.
    fn requirement(&self, name: &str) -> Option<&str> {
        let k = matcher::key(name);
        self.req
            .skills
            .iter()
            .find(|r| {
                matcher::key(&r.term) == k
                    || matcher::all_aliases(&r.term, &r.aliases)
                        .iter()
                        .any(|a| matcher::key(a) == k)
            })
            .map(|r| r.term.as_str())
    }

    /// The evidence says the resume can't back `name` (a requirement skill, or one of its aliases).
    fn denied(&self, name: &str) -> bool {
        if name.trim().is_empty() {
            return false;
        }
        let names = [Some(name), self.requirement(name)];
        self.s.evidence.iter().any(|e| {
            e.have == Have::No
                && names
                    .iter()
                    .flatten()
                    .any(|n| matcher::key(n) == matcher::key(&e.skill))
        })
    }
}

/// The text of bank line `id`: a bullet (with its variants), a note line (`entry#n`), an entry (every line of it), a
/// skills line (every item) or a section (everything in it).
fn source_text(bank: &Bank, id: &str) -> Option<String> {
    if let Some(b) = bank_bullet(bank, id) {
        return Some(
            std::iter::once(&b.text)
                .chain(&b.variants)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    if let Some((entry, i)) = note_ref(id) {
        let (_, e) = bank_entry(bank, entry)?;
        return e.notes.get(i).map(|n| format!("{}: {}", n.label, n.text));
    }
    if let Some((_, e)) = bank_entry(bank, id) {
        return Some(entry_text(e));
    }
    for s in &bank.sections {
        if let Some(g) = s.groups.iter().find(|g| g.id == id) {
            return Some(format!("{}: {}", g.label, g.items.join(", ")));
        }
        if s.id == id {
            let mut lines = vec![s.title.clone()];
            lines.extend(s.entries.iter().map(entry_text));
            lines.extend(
                s.groups
                    .iter()
                    .map(|g| format!("{}: {}", g.label, g.items.join(", "))),
            );
            return Some(lines.join("\n"));
        }
    }
    None
}

fn entry_text(e: &crate::bank::BankEntry) -> String {
    let mut lines = vec![
        e.name.clone(),
        e.place.clone(),
        e.dates.clone(),
        e.subtitle.clone(),
    ];
    lines.extend(e.notes.iter().map(|n| format!("{}: {}", n.label, n.text)));
    for b in &e.bullets {
        lines.push(b.text.clone());
        lines.extend(b.variants.iter().cloned());
    }
    lines.join("\n")
}
