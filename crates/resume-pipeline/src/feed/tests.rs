use serde_json::json;

use super::*;

const TODAY: (u32, u32) = (2026, 10);

fn entry(id: &str, subtitle: &str, start: &str, end: &str) -> serde_json::Value {
    json!({
        "id": id, "name": id, "url": "", "place": "Tempe, AZ", "dates": "", "start": start, "end": end,
        "subtitle": subtitle,
        "bullets": [{ "id": format!("{id}.1"), "text": "Built it", "skills": ["Postgres", "Tailwind CSS"] }]
    })
}

/// A bank with these experience entries and one school ending `school_end`, and a master showing all of it.
fn bank(jobs: &[(&str, &str)], school_end: &str) -> (Bank, Master) {
    let jobs: Vec<_> = jobs
        .iter()
        .enumerate()
        .map(|(i, (s, e))| entry(&format!("job{i}"), "Software Engineer Intern", s, e))
        .collect();
    let bank: Bank = serde_json::from_value(json!({
        "schema": 2, "name": "Jo", "contact": [],
        "sections": [
            { "id": "work", "kind": "experience", "title": "Experience", "entries": jobs },
            { "id": "school", "kind": "education", "title": "Education",
              "entries": [entry("asu", "B.S. Computer Science", "2022-08", school_end)] },
            { "id": "skills", "kind": "skills", "title": "Skills",
              "groups": [{ "id": "langs", "label": "Languages", "items": ["C++", "Node.js", "PostgreSQL"] }] }
        ]
    }))
    .unwrap();
    let work: Vec<_> = (0..jobs.len())
        .map(|i| json!({ "id": format!("job{i}"), "bullets": [{ "id": format!("job{i}.1") }] }))
        .collect();
    let master: Master = serde_json::from_value(json!({
        "id": "swe", "title": "Software Engineering", "template": "classic-serif", "pageLimit": 1,
        "sections": [
            { "id": "work", "entries": work },
            { "id": "skills", "groups": [{ "id": "langs", "items": ["C++", "Node.js", "PostgreSQL"] }] }
        ]
    }))
    .unwrap();
    (bank, master)
}

fn auto(jobs: &[(&str, &str)], school_end: &str) -> Profile {
    let (bank, master) = bank(jobs, school_end);
    profile(&bank, &[&master], TODAY)
}

#[test]
fn seniority_by_years() {
    // 11 months (0.9 y), long out of school.
    let p = auto(&[("2025-01", "2025-11")], "2020-05");
    assert!(!p.student_or_recent);
    assert_eq!(p.seniority, ["intern", "junior"]);
    assert_eq!(p.employment, ["full_time"]);
    // 14 months (1.2 y).
    let p = auto(&[("2024-01", "2025-02")], "2020-05");
    assert_eq!(p.seniority, ["junior"]);
    // The same with school just finished: internships too.
    let p = auto(&[("2024-01", "2025-02")], "2026-05");
    assert_eq!(p.seniority, ["intern", "junior"]);
    assert_eq!(p.employment, ["full_time", "internship"]);
    // 42 months (3.5 y), overlapping jobs counted once.
    let p = auto(&[("2020-01", "2022-06"), ("2022-01", "2023-06")], "2019-05");
    assert_eq!(p.years, 3.5);
    assert_eq!(p.seniority, ["middle"]);
    let p = auto(&[("2010-01", PRESENT)], "2009-05");
    assert_eq!(p.seniority, ["senior"]);
}

#[test]
fn recent_school_is_within_twelve_months() {
    assert!(auto(&[], "2025-11").student_or_recent); // 11 months ago
    assert!(!auto(&[], "2025-09").student_or_recent); // 13 months ago
    assert!(auto(&[], PRESENT).student_or_recent);
    assert!(auto(&[], "2027-05").student_or_recent);
}

#[test]
fn auto_reads_category_place_and_skills() {
    let p = auto(&[("2025-01", "2025-11")], "2020-05");
    assert_eq!(
        p.category,
        ["software_engineering", "backend", "frontend", "fullstack"]
    );
    assert_eq!(p.countries, ["us"]);
    for s in [
        "cpp",
        "nodejs",
        "postgresql",
        "postgres",
        "tailwind",
        "tailwindcss",
    ] {
        assert!(p.skills.contains(&s.to_owned()), "{s} in {:?}", p.skills);
    }
    // Experience subtitles' role words count; level words like `intern` don't.
    assert!(p.title_words.contains(&"engineer".to_owned()));
    assert!(!p.title_words.contains(&"intern".to_owned()));
    assert!(!p.title_words.contains(&"computer".to_owned()));

    let settings = FeedSettings {
        seniority: Some(vec!["senior".into()]),
        include_clearance: false,
        ..FeedSettings::default()
    };
    let q = query(&settings, &p);
    assert_eq!(q.seniority, ["senior"]);
    assert_eq!(q.category, p.category);
    assert_eq!(q.requires_clearance, Some(false));
    assert_eq!(q.visa_sponsorship, None);
}

fn job(title: &str, skills: &[&str], years_min: Option<u32>) -> FeedJob {
    FeedJob {
        origin: Origin::Freehire,
        description: None,
        slug: "x".into(),
        title: title.into(),
        company: "Acme".into(),
        company_slug: None,
        location: None,
        url: "https://acme.example/jobs/1".into(),
        source: "workday".into(),
        external_id: None,
        skills: skills.iter().map(|s| (*s).to_owned()).collect(),
        posted_at: None,
        found_at: "2026-10-13T10:00:00Z".into(),
        seniority: None,
        employment_type: None,
        work_mode: None,
        years_min,
        salary: None,
    }
}

#[test]
fn local_check_drops_and_orders() {
    // 1.2 years: up to 3 asked is kept.
    let p = auto(&[("2024-01", "2025-02")], "2020-05");
    let none = HashSet::new();
    let any = &Query::default();
    let ok = local_check(
        &job("Backend Engineer", &["postgresql", "cpp"], Some(3)),
        &p,
        any,
        &none,
    );
    assert!(ok.keep);
    // Two shared skills, and `engineer` from your titles.
    assert_eq!(ok.overlap, 2.5);
    let over = local_check(
        &job("Backend Engineer", &["postgresql"], Some(4)),
        &p,
        any,
        &none,
    );
    assert!(!over.keep);
    assert!(over.reason.unwrap().contains("4 years"));

    let known: HashSet<String> = ["https://acme.example/jobs/1".to_owned()].into();
    assert!(!local_check(&job("Backend Engineer", &["cpp"], None), &p, any, &known).keep);

    let unrelated = local_check(
        &job("Registered Nurse", &["patient-care"], None),
        &p,
        any,
        &none,
    );
    assert!(!unrelated.keep);
    assert_eq!(unrelated.overlap, 0.0);
    // A title word alone is enough to keep it; a level word isn't one.
    let titled = local_check(
        &job("Software Intern", &["patient-care"], None),
        &p,
        any,
        &none,
    );
    assert!(titled.keep);
    assert_eq!(titled.overlap, 0.5);
    assert!(
        !local_check(
            &job("Copywriting Intern", &["patient-care"], None),
            &p,
            any,
            &none
        )
        .keep
    );
}

fn pull(seniority: &[&str], countries: &[&str]) -> Query {
    Query {
        seniority: seniority.iter().map(|s| (*s).to_owned()).collect(),
        countries: countries.iter().map(|s| (*s).to_owned()).collect(),
        ..Query::default()
    }
}

/// Boards list every level and LinkedIn's levels are coarse; freehire filters by level itself.
#[test]
fn watchlist_and_linkedin_titles_outside_the_seniority_are_dropped() {
    let p = auto(&[("2024-01", "2025-02")], "2020-05");
    let none = HashSet::new();
    let junior = &pull(&["intern", "junior"], &[]);
    let watched = |title: &str| FeedJob {
        origin: Origin::Watchlist,
        ..job(title, &[], None)
    };
    let senior = local_check(&watched("Senior Backend Engineer"), &p, junior, &none);
    assert!(!senior.keep);
    assert_eq!(
        senior.reason.as_deref(),
        Some("The title says senior; the feed looks for intern, junior.")
    );
    assert!(!local_check(&watched("Staff Software Engineer"), &p, junior, &none).keep);
    assert!(!local_check(&watched("Engineering Lead, Platform"), &p, junior, &none).keep);
    assert!(local_check(&watched("Software Engineer Intern"), &p, junior, &none).keep);
    assert!(local_check(&watched("Backend Engineer"), &p, junior, &none).keep);
    assert!(local_check(&watched("Software Engineer, New Grad"), &p, junior, &none).keep);
    let linkedin = FeedJob {
        origin: Origin::Linkedin,
        ..job("Principal Software Engineer, Platform", &[], None)
    };
    assert!(!local_check(&linkedin, &p, &pull(&["middle"], &[]), &none).keep);
    // freehire's own level filter already ran.
    let listed = job("Senior Backend Engineer", &[], None);
    assert!(local_check(&listed, &p, junior, &none).keep);
}

/// Boards list every place; a place naming another country drops the job, one naming none keeps it.
#[test]
fn watchlist_places_outside_the_countries_are_dropped() {
    let p = auto(&[("2024-01", "2025-02")], "2020-05");
    let none = HashSet::new();
    let us = &pull(&[], &["us"]);
    let at = |place: &str| FeedJob {
        origin: Origin::Watchlist,
        location: Some(place.into()),
        ..job("Backend Engineer", &[], None)
    };
    let abroad = local_check(&at("China, Shanghai"), &p, us, &none);
    assert!(!abroad.keep);
    assert_eq!(
        abroad.reason.as_deref(),
        Some("China, Shanghai is outside your countries.")
    );
    assert!(!local_check(&at("London, UK"), &p, us, &none).keep);
    assert!(!local_check(&at("Toronto, ON, Canada"), &p, us, &none).keep);
    for place in [
        "US, CA, Santa Clara",
        "San Francisco, CA",
        "New York City",
        "Remote",
        "Austin, Texas, United States",
    ] {
        assert!(local_check(&at(place), &p, us, &none).keep, "{place}");
    }
    assert!(local_check(&at("London, UK"), &p, &pull(&[], &["uk"]), &none).keep);
}

#[test]
fn linkedin_searches_follow_the_query() {
    let mut p = auto(&[("2024-01", "2025-02")], "2020-05");
    p.titles = vec!["Software Engineer".into(), "Data Analyst".into()];
    let s = FeedSettings {
        seniority: Some(vec!["intern".into(), "junior".into(), "middle".into()]),
        employment: Some(vec!["full_time".into(), "internship".into()]),
        countries: Some(vec!["us".into(), "ca".into(), "zz".into()]),
        ..FeedSettings::default()
    };
    let searches = linkedin_searches(&s, &p, 7200);
    let asked: Vec<(&str, Option<&str>)> = searches
        .iter()
        .map(|s| (s.keywords.as_str(), s.location.as_deref()))
        .collect();
    assert_eq!(
        asked,
        [
            ("Software Engineer", Some("United States")),
            ("Software Engineer", Some("Canada")),
            ("Data Analyst", Some("United States")),
            ("Data Analyst", Some("Canada")),
        ]
    );
    assert_eq!(searches[0].experience, [1, 2, 3, 4]);
    assert_eq!(searches[0].job_types, ['F', 'I']);
    assert_eq!(searches[0].posted_within_secs, Some(7200));

    // Countries gliide can't name: no search rather than one everywhere.
    let unnamed = FeedSettings {
        countries: Some(vec!["zz".into()]),
        ..FeedSettings::default()
    };
    assert!(linkedin_searches(&unnamed, &p, 3600).is_empty());
    assert!(fantastic_query(&unnamed, &p, TimeRange::Day).is_none());
}

#[test]
fn skill_keys_meet_freehires_vocabulary() {
    assert_eq!(skill_key("C++"), "cpp");
    assert_eq!(skill_key("C#"), "csharp");
    assert_eq!(skill_key("Node.js"), "nodejs");
    assert_eq!(skill_key("CI/CD"), skill_key("ci-cd"));
    assert_eq!(skill_key("GitHub Actions"), skill_key("github-actions"));
}
