//! Dig (local/interviewer/SPEC.md §6): gliide asks about one bank entry and turns your answers into facts and proposed
//! additions. Questions: one model call, kept in the store's `digs` table under the prompt and the entry's content
//! (so they come back until the entry changes, unless asked again). Capture: one model call; the facts your answers
//! back are kept at once (one bank revision), the additions checked against the entry, its facts and your answers and
//! proposed for you to accept, and the answers kept in `dig_answers` so a story proposed from them stays grounded.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{Local, SecondsFormat, Utc};
use resume_core::Requirements;
use resume_core::bank::{BankEntry, FactKind, FactSource};
use resume_core::facts::add_fact;
use resume_core::matcher::{Page, find_literal};
use resume_core::posting::text_hash;
use resume_llm::dig::{self, Answered, Checked};
use resume_store as store;

use crate::dto::{
    DigAnswer, DigCapture, DigOp, DigQuestion, DigQuestions, DigSkip, Stage, StorySource,
};
use crate::entry::{entry_view, fact_line};
use crate::error::{AppError, Result};
use crate::state::{App, blocking, lock, model_call};
use crate::stories::{DigAnswersDoc, draft_of};

/// The entry's Dig questions: the ones kept for its content now, unless `again`; else one model call, kept.
pub async fn dig_questions(app: &Arc<App>, entry: &str, again: bool) -> Result<DigQuestions> {
    let id = entry.to_owned();
    let (e, masters, terms, key, kept) = blocking(app, move |app| {
        let store = lock(&app.store);
        let (_, bank) = store.bank()?.ok_or_else(AppError::no_bank)?;
        let e = find(&bank, &id)?.clone();
        let masters: Vec<String> = store
            .masters()?
            .into_iter()
            .map(|(_, m)| m)
            .filter(|m| {
                m.sections
                    .iter()
                    .any(|s| s.entries.iter().any(|x| x.id == id))
            })
            .map(|m| m.title)
            .collect();
        let mut reqs: Vec<Requirements> = Vec::new();
        for p in store.postings()? {
            if let Some(r) = store.latest_requirements(&p.id)? {
                reqs.push(r);
            }
        }
        let key = text_hash(&format!(
            "{}\n{}\n{}",
            dig::QUESTIONS_PROMPT,
            dig::QUESTIONS_SCHEMA,
            serde_json::to_string(&e)?
        ));
        let kept = if again { None } else { store.dig(&key)? };
        let terms = unstated_terms(&e, &reqs);
        Ok((e, masters, terms, key, kept))
    })
    .await?;
    if let Some(d) = kept {
        return view(&d);
    }
    let llm = app.claude().await?;
    let model = app.settings().suggest_model;
    let today = Local::now().format("%Y-%m-%d").to_string();
    let job = format!("entry:{}", e.id);
    let (m, ee) = (model.clone(), e.clone());
    let (reply, out) = model_call(app, &job, Stage::Digging, async move {
        let input = dig::QuestionsInput {
            entry: &ee,
            masters: &masters,
            terms: &terms,
            today: &today,
        };
        dig::ask(&llm, &dig::questions_schema(), &input, &m).await
    })
    .await?;
    app.note_served(&model, &out);
    let q = dig::questions_checked(reply, &e);
    let row = store::Dig {
        key,
        entry: e.id.clone(),
        json: serde_json::to_string(&q)?,
        model: out.served(&model).unwrap_or(&model).to_owned(),
        at: String::new(),
    };
    let stored = blocking(app, move |app| Ok(lock(&app.store).put_dig(&row)?)).await?;
    view(&stored)
}

fn view(d: &store::Dig) -> Result<DigQuestions> {
    let q: dig::Questions = serde_json::from_str(&d.json)?;
    Ok(DigQuestions {
        entry: d.entry.clone(),
        read: q.read,
        questions: q.questions.into_iter().map(question).collect(),
        model: d.model.clone(),
    })
}

/// One model call: your `answers` (and `note`) about entry `entry` read. The facts they back are kept at once (one
/// bank revision); the additions they support are proposed; what couldn't be used says why.
pub async fn dig_capture(
    app: &Arc<App>,
    entry: &str,
    answers: Vec<DigAnswer>,
    note: String,
) -> Result<DigCapture> {
    let answers: Vec<Answered> = answers
        .into_iter()
        .map(|a| Answered {
            question: dig::Question {
                id: a.question.id,
                about: a.question.about,
                about_label: a.question.about_label,
                text: a.question.text,
                why: a.question.why,
            },
            answer: a.answer,
            not_applicable: a.not_applicable,
        })
        .collect();
    let note = note.trim().to_owned();
    if note.is_empty() && answers.iter().all(|a| a.said().is_none()) {
        return Err(AppError::other("Answer a question first"));
    }
    let id = entry.to_owned();
    let e = blocking(app, move |app| {
        let store = lock(&app.store);
        let (_, bank) = store.bank()?.ok_or_else(AppError::no_bank)?;
        Ok(find(&bank, &id)?.clone())
    })
    .await?;
    let llm = app.claude().await?;
    let model = app.settings().suggest_model;
    let today = Local::now().format("%Y-%m-%d").to_string();
    let job = format!("entry:{}", e.id);
    let (m, ee, aa, nn) = (model.clone(), e.clone(), answers.clone(), note.clone());
    let (reply, out) = model_call(app, &job, Stage::Digging, async move {
        let input = dig::CaptureInput {
            entry: &ee,
            answers: &aa,
            note: &nn,
            today: &today,
        };
        dig::capture(&llm, &dig::capture_schema(), &input, &m).await
    })
    .await?;
    app.note_served(&model, &out);
    let served = out.served(&model).unwrap_or(&model).to_owned();
    let entry = e.id;
    blocking(app, move |app| {
        let capture = uuid::Uuid::new_v4().to_string();
        let written = reply.facts.len();
        let (facts, mut skipped) = dig::facts_checked(reply.facts, &answers, &note);
        let store = lock(&app.store);
        let (_, mut bank) = store.bank()?.ok_or_else(AppError::no_bank)?;
        let source = FactSource {
            kind: FactKind::Dig,
            at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            posting: None,
            company: None,
        };
        let mut new_facts: Vec<Option<String>> = vec![None; written];
        for (i, f) in &facts {
            let id = add_fact(&mut bank, &entry, &f.text, &f.skills, source.clone())
                .map_err(|e| AppError::other(e.to_string()))?;
            new_facts[*i] = Some(id);
        }
        if !facts.is_empty() {
            store.put_bank(&bank)?;
        }
        let doc = DigAnswersDoc {
            answers: answers.clone(),
            note: note.clone(),
        };
        store.add_dig_answers(&store::DigAnswers {
            id: capture.clone(),
            entry: entry.clone(),
            json: serde_json::to_string(&doc)?,
            at: String::new(),
        })?;
        drop(store);
        let (ops, dropped) =
            dig::ops_checked(reply.ops, &bank, &entry, &new_facts, &answers, &note);
        skipped.extend(dropped);
        let e = find(&bank, &entry)?;
        let mut kept_ids: Vec<&String> = Vec::new();
        for id in new_facts.iter().flatten() {
            if !kept_ids.contains(&id) {
                kept_ids.push(id);
            }
        }
        let facts = kept_ids
            .iter()
            .filter_map(|id| e.facts.iter().find(|f| f.id == **id))
            .map(fact_line)
            .collect();
        let follow_ups = dig::follow_ups_checked(reply.follow_ups, e)
            .into_iter()
            .map(question)
            .collect();
        let ops = ops.into_iter().map(|op| dig_op(op, &capture)).collect();
        let mut all_skipped: Vec<DigSkip> = reply
            .skipped
            .into_iter()
            .map(|s| DigSkip {
                what: s.what.trim().to_owned(),
                why: s.why.trim().to_owned(),
            })
            .collect();
        all_skipped.extend(skipped.into_iter().map(|s| DigSkip {
            what: s.what,
            why: s.why,
        }));
        Ok(DigCapture {
            reply: reply.reply.trim().to_owned(),
            facts,
            ops,
            follow_ups,
            skipped: all_skipped,
            entry: entry_view(app, &entry, None)?,
            model: served,
        })
    })
    .await
}

pub(crate) fn question(q: dig::Question) -> DigQuestion {
    DigQuestion {
        id: q.id,
        about: q.about,
        about_label: q.about_label,
        text: q.text,
        why: q.why,
    }
}

/// A checked addition as the thread proposes it, keyed by its content; a story drawn from capture `capture`.
fn dig_op(op: Checked, capture: &str) -> DigOp {
    let key = |parts: &[&str]| text_hash(&parts.join("\u{1f}"))[..16].to_owned();
    match op {
        Checked::AddVariant { bullet, text, why } => DigOp::AddVariant {
            key: key(&["add-variant", &bullet, &text]),
            bullet,
            text,
            why,
        },
        Checked::AddBullet { text, why } => DigOp::AddBullet {
            key: key(&["add-bullet", &text]),
            text,
            why,
        },
        Checked::TagSkills {
            bullet,
            skills,
            why,
        } => DigOp::TagSkills {
            key: key(&["tag-skills", &bullet, &skills.join("\u{1e}")]),
            bullet,
            skills,
            why,
        },
        Checked::AddStory { story, why } => DigOp::AddStory {
            key: key(&[
                "add-story",
                &story.title,
                &story.situation,
                &story.task,
                &story.action,
                &story.result,
            ]),
            story: Box::new(draft_of(story, StorySource::Dig, capture)),
            why,
        },
    }
}

fn find<'b>(bank: &'b resume_core::Bank, id: &str) -> Result<&'b BankEntry> {
    bank.sections
        .iter()
        .flat_map(|s| &s.entries)
        .find(|e| e.id == id)
        .ok_or_else(|| AppError::missing("This entry isn't in your bank any more"))
}

/// The requirement terms `reqs` (your postings' latest requirements) ask for that entry `e` doesn't state anywhere (its
/// header, notes, bullets with their variants and skills, facts), most asked first (then most weighed), at most
/// [`dig::MAX_TERMS`]: `(term, postings asking)`.
pub fn unstated_terms(e: &BankEntry, reqs: &[Requirements]) -> Vec<(String, usize)> {
    let mut text = vec![e.name.clone(), e.subtitle.clone()];
    text.extend(e.notes.iter().map(|n| format!("{}: {}", n.label, n.text)));
    for b in &e.bullets {
        text.push(b.text.clone());
        text.extend(b.variants.iter().cloned());
        text.extend(b.skills.iter().cloned());
    }
    text.extend(e.fact_lines().map(str::to_owned));
    let page = Page::of_text(&text.join("\n"));
    let norm = |t: &str| t.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    // key → (term as first written, postings, weight).
    let mut counts: HashMap<String, (String, usize, f64)> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for r in reqs {
        let mut seen: Vec<String> = Vec::new();
        for k in &r.skills {
            let key = norm(&k.term);
            if key.is_empty() || seen.contains(&key) {
                continue;
            }
            seen.push(key.clone());
            if find_literal(&page, &k.term, &k.aliases).is_some() {
                continue;
            }
            let c = counts.entry(key.clone()).or_insert_with(|| {
                order.push(key.clone());
                (k.term.clone(), 0, 0.0)
            });
            c.1 += 1;
            c.2 += resume_core::matcher::weight(k.priority, k.required);
        }
    }
    let mut out: Vec<(String, usize, f64)> = order
        .into_iter()
        .filter_map(|k| counts.remove(&k))
        .collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.total_cmp(&a.2)));
    out.into_iter()
        .take(dig::MAX_TERMS)
        .map(|(t, n, _)| (t, n))
        .collect()
}

#[cfg(test)]
mod tests {
    use resume_core::requirements::{Seniority, Skill, SkillKind};
    use resume_core::{Requirements, Resume, migrate_v1};

    use super::unstated_terms;

    fn req(skills: &[(&str, u8, bool)]) -> Requirements {
        Requirements {
            role: "SWE".into(),
            company: "Acme".into(),
            short_role: "swe".into(),
            location: String::new(),
            seniority: Seniority::Entry,
            skills: skills
                .iter()
                .map(|(t, p, r)| Skill {
                    term: (*t).into(),
                    aliases: vec![],
                    kind: SkillKind::Tool,
                    priority: *p,
                    required: *r,
                    group: None,
                })
                .collect(),
            title_terms: vec![],
            years_min: None,
            degree: None,
            certifications: vec![],
            knockouts: vec![],
        }
    }

    #[test]
    fn terms_the_entry_doesnt_state_most_asked_first() {
        let v1: Resume =
            serde_json::from_str(include_str!("../../../fixtures/banks/jane-doe.v1.json")).unwrap();
        let bank = migrate_v1(&v1).bank;
        let acme = &bank.sections[0].entries[0];
        let reqs = [
            req(&[("Docker", 2, false), ("Rust", 1, true), ("Kafka", 1, true)]),
            req(&[("docker", 3, false), ("Terraform", 1, true)]),
            req(&[("Docker", 1, true), ("Docker", 1, true)]),
        ];
        let terms = unstated_terms(acme, &reqs);
        // Rust is on acme.1; Docker is asked 3 times; Kafka and Terraform once each, weighed alike.
        assert_eq!(
            terms,
            [
                ("Docker".to_owned(), 3),
                ("Kafka".to_owned(), 1),
                ("Terraform".to_owned(), 1)
            ]
        );
    }
}
