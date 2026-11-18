//! Coach: the prep for a technical round per posting, with no earlier debriefs or stories, from the resolved `swe`
//! master, as the app's `prep.rs` builds it.

use resume_core::Requirements;
use resume_llm::prep::{self, Dropped, Prep, SentResume};

/// The round every prep is for.
pub const ROUND: &str = "technical";

/// `p` held to the `swe` master as sent and `req`, with no stories: what the app keeps of it, and what it dropped.
pub fn checked(sent: &SentResume, req: &Requirements, p: Prep) -> (Prep, Dropped) {
    prep::checked(p, sent, Some(req), &[])
}

/// Claims tied to the resume in a checked prep and those [`checked`] dropped, counted alike: line ids cited by
/// questions and gaps, gap-answer sentences, and stories named. A gap dropped whole for not being a must-have the page
/// lacks is relevance, not grounding, so it counts on neither side.
pub fn grounding(kept: &Prep, dropped: &Dropped) -> (u32, u32) {
    let k = kept
        .questions
        .iter()
        .map(|q| q.lines.len() + usize::from(q.story.is_some()))
        .sum::<usize>()
        + kept
            .gaps
            .iter()
            .map(|g| g.lines.len() + prep::sentences(&g.answer).len())
            .sum::<usize>();
    let d = dropped.lines.len() + dropped.sentences.len() + dropped.stories.len();
    (k as u32, d as u32)
}
