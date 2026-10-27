//! Your stories (local/interviewer/SPEC.md §10, §14): their own record beside the bank (`resume_core::story`), listed
//! with the themes none covers, each resolved against the bank as it is now (its entry's name, its bullets' and facts'
//! words, or that they're gone), how long it takes to tell, and where it was practiced; saved (a draft drawn from an
//! answer is held to that answer), removed (Undo saves it back by its id), and told alone (Tell it: one practice call
//! with the story as the question's context).

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{SecondsFormat, Utc};
use resume_core::Bank;
use resume_core::story::{self as core, Story};
use resume_llm::dig::Answered;
use resume_llm::practice::{self, PracticeInput};
use resume_llm::prep::SentResume;
use resume_store::PracticeRow;

use crate::dto::{
    PracticeTurn, Stage, StoriesView, StoryDraft, StoryLink, StorySource, StoryTheme, StoryUse,
    StoryView,
};
use crate::error::{AppError, Result};
use crate::prep::turn;
use crate::rounds::application_of;
use crate::state::{App, blocking, lock, model_call};

/// What a Dig capture keeps of its answers (`dig_answers.json`): what grounds the stories it proposes.
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct DigAnswersDoc {
    pub answers: Vec<Answered>,
    pub note: String,
}

/// Every story, newest first, and the themes none covers.
pub fn stories(app: &App) -> Result<StoriesView> {
    let store = lock(&app.store);
    let bank = store.bank()?.map(|(_, b)| b);
    let all = store.stories()?;
    let uses = uses(&store)?;
    drop(store);
    Ok(StoriesView {
        uncovered: core::uncovered(&all).into_iter().map(theme).collect(),
        stories: all
            .iter()
            .map(|s| view(s, bank.as_ref(), &uses))
            .collect(),
    })
}

/// Story `id`.
pub fn story(app: &App, id: &str) -> Result<StoryView> {
    let store = lock(&app.store);
    let s = store
        .story(id)?
        .ok_or_else(|| AppError::missing("This story was removed"))?;
    let bank = store.bank()?.map(|(_, b)| b);
    let uses = uses(&store)?;
    Ok(view(&s, bank.as_ref(), &uses))
}

/// Saves `draft`: a new story when its id is `None`, else the one under its id (inserted when it isn't stored: Undo).
/// An existing story's draft without `from` keeps where the story came from. A draft drawn from an answer (`from`) may
/// state only numbers its bullets, its facts or that answer state; words you type are yours.
pub fn save_story(app: &App, draft: StoryDraft) -> Result<StoryView> {
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let store = lock(&app.store);
    let bank = store.bank()?.map(|(_, b)| b);
    let existing = match draft.id.as_deref() {
        Some(id) => store.story(id)?,
        None => None,
    };
    let source = match (&existing, &draft.from) {
        (Some(s), None) => s.source.clone(),
        _ => core::StorySource {
            kind: match draft.source {
                Some(StorySource::Dig) => core::StoryKind::Dig,
                Some(StorySource::Practice) => core::StoryKind::Practice,
                Some(StorySource::Edit) | None => core::StoryKind::Edit,
            },
            at: existing
                .as_ref()
                .map_or_else(|| now.clone(), |s| s.source.at.clone()),
            from: draft.from.clone(),
        },
    };
    let story = Story {
        id: draft
            .id
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        title: tidy(&draft.title),
        entry: draft
            .entry
            .map(|e| e.trim().to_owned())
            .filter(|e| !e.is_empty()),
        bullets: once(draft.bullets),
        facts: once(draft.facts),
        situation: draft.situation.trim().to_owned(),
        task: draft.task.trim().to_owned(),
        action: draft.action.trim().to_owned(),
        result: draft.result.trim().to_owned(),
        skills: skills(draft.skills),
        themes: {
            let mut themes: Vec<core::StoryTheme> = Vec::new();
            for t in draft.themes.into_iter().map(core_theme) {
                if !themes.contains(&t) {
                    themes.push(t);
                }
            }
            themes
        },
        source,
        updated_at: now,
    };
    if let Some(from) = &draft.from {
        let answer = answer_text(&store, from)?;
        let empty = Bank {
            schema: resume_core::bank::BANK_SCHEMA,
            name: String::new(),
            contact: vec![],
            sections: vec![],
        };
        let found = core::unsourced(&story, bank.as_ref().unwrap_or(&empty), &answer);
        if !found.is_empty() {
            return Err(AppError::other(format!(
                "This story says what the answer it came from doesn't: {}",
                found.join("; ")
            )));
        }
    }
    store.put_story(&story)?;
    let uses = uses(&store)?;
    Ok(view(&story, bank.as_ref(), &uses))
}

/// Removes story `id` (its tellings stay; saving it back by its id finds them again).
pub fn remove_story(app: &App, id: &str) -> Result<()> {
    lock(&app.store).delete_story(id)?;
    Ok(())
}

/// One model call: `answer` to `asked` (a question the story answers), telling story `story` alone, read back against
/// its entry as the bank holds it, and kept as a practice turn with no round.
pub async fn tell_story(
    app: &Arc<App>,
    story: &str,
    asked: String,
    answer: String,
) -> Result<PracticeTurn> {
    let answer = answer.trim().to_owned();
    if answer.is_empty() {
        return Err(AppError::other("Type your answer first"));
    }
    let asked = asked.trim().to_owned();
    let id = story.to_owned();
    let (s, sent) = blocking(app, move |app| {
        let store = lock(&app.store);
        let s = store
            .story(&id)?
            .ok_or_else(|| AppError::missing("This story was removed"))?;
        let sent = match (&s.entry, store.bank()?) {
            (Some(entry), Some((_, bank))) => SentResume::entry(&bank, entry),
            _ => SentResume::default(),
        };
        Ok((s, sent))
    })
    .await?;
    let lines: Vec<String> = s
        .bullets
        .iter()
        .filter(|b| sent.line(b).is_some())
        .cloned()
        .collect();
    let llm = app.claude().await?;
    let model = app.settings().suggest_model;
    let job = format!("story:{}", s.id);
    let (m, st, se, a, q, l) = (
        model.clone(),
        s.clone(),
        sent.clone(),
        answer.clone(),
        asked.clone(),
        lines.clone(),
    );
    let (reply, out) = model_call(app, &job, Stage::Practicing, async move {
        let input = PracticeInput {
            question: &q,
            why: "",
            lines: &l,
            answer: &a,
            terms: &st.skills,
            sent: &se,
            story: Some(&st),
        };
        practice::read(&llm, &practice::schema(), &input, &m).await
    })
    .await?;
    app.note_served(&model, &out);
    let (f, dropped) = practice::checked(reply, &sent, &lines, &answer, Some(&s));
    if !dropped.is_empty() {
        eprintln!("gliide: telling story {} dropped {dropped:?}", s.id);
    }
    let row = PracticeRow {
        id: uuid::Uuid::new_v4().to_string(),
        round_id: None,
        story_id: Some(s.id.clone()),
        question: s.id,
        asked,
        answer,
        feedback: serde_json::to_string(&f)?,
        at: String::new(),
    };
    let stored = blocking(app, move |app| Ok(lock(&app.store).add_practice(&row)?)).await?;
    turn(stored)
}

/// Every telling of the story (alone or answering a prep question), oldest first.
pub fn story_turns(app: &App, story: &str) -> Result<Vec<PracticeTurn>> {
    lock(&app.store)
        .story_practice(story)?
        .into_iter()
        .map(turn)
        .collect()
}

/// A story a model drew from an answer, as the UI keeps it until you save it: drawn from `from` (a practice turn's or
/// a Dig capture's id).
pub(crate) fn draft_of(s: practice::StoryDraft, source: StorySource, from: &str) -> StoryDraft {
    StoryDraft {
        id: None,
        title: s.title,
        entry: s.entry,
        bullets: s.bullets,
        facts: s.facts,
        situation: s.situation,
        task: s.task,
        action: s.action,
        result: s.result,
        skills: s.skills,
        themes: s.themes.into_iter().map(theme).collect(),
        source: Some(source),
        from: Some(from.to_owned()),
    }
}

/// Where each story was told, newest first: "Globex · Technical", or "Told alone".
fn uses(store: &resume_store::Store) -> Result<HashMap<String, Vec<StoryUse>>> {
    let mut labels: HashMap<String, String> = HashMap::new();
    let mut out: HashMap<String, Vec<StoryUse>> = HashMap::new();
    for p in store.told_practice()? {
        let Some(story) = p.story_id else { continue };
        let label = match &p.round_id {
            None => "Told alone".to_owned(),
            Some(round) => match labels.get(round) {
                Some(l) => l.clone(),
                None => {
                    let l = match store.round(round)? {
                        Some(r) => {
                            let company = application_of(store, &r.application_id)
                                .map(|a| a.company.trim().to_owned())
                                .unwrap_or_default();
                            let kind = kind_label(&r.kind);
                            if company.is_empty() {
                                kind
                            } else {
                                format!("{company} · {kind}")
                            }
                        }
                        None => "Practice".to_owned(),
                    };
                    labels.insert(round.clone(), l.clone());
                    l
                }
            },
        };
        out.entry(story).or_default().push(StoryUse { label, at: p.at });
    }
    Ok(out)
}

/// A round kind's name as a label: `recruiter-screen` → "Recruiter screen".
fn kind_label(kind: &str) -> String {
    let words = kind.replace('-', " ");
    let mut chars = words.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Story `s` as the screens show it, against `bank` as it is now.
fn view(s: &Story, bank: Option<&Bank>, uses: &HashMap<String, Vec<StoryUse>>) -> StoryView {
    let entry_name = s.entry.as_deref().and_then(|id| {
        bank?
            .sections
            .iter()
            .flat_map(|x| &x.entries)
            .find(|e| e.id == id)
            .map(|e| e.name.clone())
    });
    let mut links: Vec<StoryLink> = s
        .bullets
        .iter()
        .map(|id| {
            let text = bank.and_then(|b| core::bullet(b, id)).map(|b| b.text.clone());
            StoryLink {
                id: id.clone(),
                fact: false,
                gone: text.is_none(),
                text: text.unwrap_or_default(),
            }
        })
        .collect();
    links.extend(s.facts.iter().map(|id| {
        let text = bank.and_then(|b| core::fact(b, id)).map(|f| f.text.clone());
        StoryLink {
            id: id.clone(),
            fact: true,
            gone: text.is_none(),
            text: text.unwrap_or_default(),
        }
    }));
    StoryView {
        id: s.id.clone(),
        title: s.title.clone(),
        entry: s.entry.clone(),
        entry_name,
        situation: s.situation.clone(),
        task: s.task.clone(),
        action: s.action.clone(),
        result: s.result.clone(),
        skills: s.skills.clone(),
        themes: s.themes.iter().copied().map(theme).collect(),
        links,
        source: match s.source.kind {
            core::StoryKind::Dig => StorySource::Dig,
            core::StoryKind::Practice => StorySource::Practice,
            core::StoryKind::Edit => StorySource::Edit,
        },
        updated_at: s.updated_at.clone(),
        seconds: s.seconds(),
        used: uses.get(&s.id).cloned().unwrap_or_default(),
    }
}

/// The words of the answer `from` names: a practice turn's answer, or every answer (and the note) a Dig capture read;
/// empty when neither is stored.
fn answer_text(store: &resume_store::Store, from: &str) -> Result<String> {
    if let Some(p) = store.practice_row(from)? {
        return Ok(p.answer);
    }
    if let Some(d) = store.dig_answers(from)? {
        let doc: DigAnswersDoc = serde_json::from_str(&d.json)?;
        let mut text: Vec<&str> = doc.answers.iter().filter_map(Answered::said).collect();
        text.push(&doc.note);
        return Ok(text.join("\n"));
    }
    Ok(String::new())
}

pub(crate) fn theme(t: core::StoryTheme) -> StoryTheme {
    match t {
        core::StoryTheme::Conflict => StoryTheme::Conflict,
        core::StoryTheme::Failure => StoryTheme::Failure,
        core::StoryTheme::Leadership => StoryTheme::Leadership,
        core::StoryTheme::Ambiguity => StoryTheme::Ambiguity,
        core::StoryTheme::Deadline => StoryTheme::Deadline,
        core::StoryTheme::Impact => StoryTheme::Impact,
        core::StoryTheme::Learning => StoryTheme::Learning,
        core::StoryTheme::Teamwork => StoryTheme::Teamwork,
        core::StoryTheme::Initiative => StoryTheme::Initiative,
    }
}

fn core_theme(t: StoryTheme) -> core::StoryTheme {
    match t {
        StoryTheme::Conflict => core::StoryTheme::Conflict,
        StoryTheme::Failure => core::StoryTheme::Failure,
        StoryTheme::Leadership => core::StoryTheme::Leadership,
        StoryTheme::Ambiguity => core::StoryTheme::Ambiguity,
        StoryTheme::Deadline => core::StoryTheme::Deadline,
        StoryTheme::Impact => core::StoryTheme::Impact,
        StoryTheme::Learning => core::StoryTheme::Learning,
        StoryTheme::Teamwork => core::StoryTheme::Teamwork,
        StoryTheme::Initiative => core::StoryTheme::Initiative,
    }
}

fn tidy(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Each trimmed, empty ones dropped, each once.
fn once(ids: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in ids {
        let id = id.trim().to_owned();
        if !id.is_empty() && !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

/// Each tidied, empty ones dropped, each once ignoring case.
fn skills(items: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in items.iter().map(|s| tidy(s)) {
        if !s.is_empty() && !out.iter().any(|x| x.eq_ignore_ascii_case(&s)) {
            out.push(s);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::kind_label;

    #[test]
    fn round_kinds_read_as_labels() {
        assert_eq!(kind_label("recruiter-screen"), "Recruiter screen");
        assert_eq!(kind_label("technical"), "Technical");
        assert_eq!(kind_label(""), "");
    }
}
