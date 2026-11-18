//! Read's quality: a run's requirement skills against gold's.

use std::collections::BTreeMap;

use resume_core::matcher::{Page, find_literal};
use resume_core::requirements::Skill;

/// Whether two requirement skills name the same thing: either's term or one of its aliases (its own and the alias
/// dictionary's) appears as a whole term in the other's term or aliases, as `resume_core::matcher` finds terms on a
/// page. So `Python` matches `Python 3`, and `JS` matches `JavaScript` through the dictionary.
pub fn same_skill(a: &Skill, b: &Skill) -> bool {
    let page = |s: &Skill| {
        Page::new(
            std::iter::once(s.term.as_str())
                .chain(s.aliases.iter().map(String::as_str))
                .map(|t| (None, t)),
        )
    };
    find_literal(&page(b), &a.term, &a.aliases).is_some()
        || find_literal(&page(a), &b.term, &b.aliases).is_some()
}

/// A run's skills against gold's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Overlap {
    /// Gold units found among the run's skills, of all gold units. A one-of group is one unit, found when any of its
    /// members is.
    pub recall: Option<f64>,
    /// The run's skills that match some gold skill (of any unit), of all the run's skills.
    pub precision: Option<f64>,
    /// Recall over the must-have units: those with a member that is priority 1 or required.
    pub must_recall: Option<f64>,
}

pub fn overlap(gold: &[Skill], run: &[Skill]) -> Overlap {
    let mut units: Vec<Vec<&Skill>> = Vec::new();
    let mut groups: BTreeMap<&str, usize> = BTreeMap::new();
    for g in gold {
        match g.group.as_deref() {
            Some(id) => match groups.get(id) {
                Some(&i) => units[i].push(g),
                None => {
                    groups.insert(id, units.len());
                    units.push(vec![g]);
                }
            },
            None => units.push(vec![g]),
        }
    }
    let found = |u: &[&Skill]| u.iter().any(|g| run.iter().any(|r| same_skill(g, r)));
    let share = |hit: usize, of: usize| (of > 0).then(|| hit as f64 / of as f64);
    let must: Vec<&Vec<&Skill>> = units
        .iter()
        .filter(|u| u.iter().any(|g| g.priority == 1 || g.required))
        .collect();
    Overlap {
        recall: share(units.iter().filter(|u| found(u)).count(), units.len()),
        precision: share(
            run.iter()
                .filter(|r| gold.iter().any(|g| same_skill(g, r)))
                .count(),
            run.len(),
        ),
        must_recall: share(must.iter().filter(|u| found(u)).count(), must.len()),
    }
}

#[cfg(test)]
mod tests {
    use resume_core::requirements::SkillKind;

    use super::*;

    fn skill(
        term: &str,
        aliases: &[&str],
        priority: u8,
        required: bool,
        group: Option<&str>,
    ) -> Skill {
        Skill {
            term: term.into(),
            aliases: aliases.iter().map(|a| (*a).into()).collect(),
            kind: SkillKind::Hard,
            priority,
            required,
            group: group.map(str::to_owned),
        }
    }

    #[test]
    fn aliases_match_either_way() {
        let gold = skill("Kubernetes", &["K8s"], 2, false, None);
        assert!(same_skill(&gold, &skill("k8s", &[], 2, false, None)));
        assert!(same_skill(&skill("K8s", &[], 2, false, None), &gold));
        assert!(same_skill(
            &gold,
            &skill("Kubernetes clusters", &[], 2, false, None)
        ));
        assert!(!same_skill(&gold, &skill("Docker", &[], 2, false, None)));
        // A whole term only: `Java` is not in `JavaScript`.
        assert!(!same_skill(
            &skill("Java", &[], 2, false, None),
            &skill("JavaScript", &[], 2, false, None)
        ));
    }

    #[test]
    fn a_one_of_group_is_one_unit() {
        let gold = vec![
            skill("Python", &[], 1, true, None),
            skill("React", &[], 2, false, Some("ui")),
            skill("Vue", &[], 2, false, Some("ui")),
            skill("Svelte", &[], 2, false, Some("ui")),
            skill("Terraform", &[], 3, false, None),
        ];
        // Python and one member of the group: 2 of 3 units; Terraform missing.
        let run = vec![
            skill("python", &[], 1, true, None),
            skill("Vue.js", &["Vue"], 2, false, None),
            skill("Cobol", &[], 3, false, None),
        ];
        let o = overlap(&gold, &run);
        assert_eq!(o.recall, Some(2.0 / 3.0));
        // Cobol matches nothing gold has.
        assert_eq!(o.precision, Some(2.0 / 3.0));
        // Only Python is a must-have, and it is found.
        assert_eq!(o.must_recall, Some(1.0));
    }

    #[test]
    fn must_recall_misses_a_required_skill() {
        let gold = vec![
            skill("Go", &[], 2, true, None),
            skill("SQL", &[], 1, false, None),
            skill("Rust", &[], 3, false, None),
        ];
        let o = overlap(&gold, &[skill("SQL", &[], 1, false, None)]);
        assert_eq!(o.must_recall, Some(0.5));
        assert_eq!(o.recall, Some(1.0 / 3.0));
        assert_eq!(o.precision, Some(1.0));
        assert_eq!(overlap(&[], &[]).recall, None);
    }
}
