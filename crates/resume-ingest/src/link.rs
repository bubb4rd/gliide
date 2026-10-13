//! What a pasted link points at, and which request reads it.

use url::Url;

/// A job link, by the source that reads it best.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Link {
    /// `job-boards.greenhouse.io/{board}/jobs/{id}` and its older forms.
    Greenhouse { board: String, id: String },
    /// `jobs.lever.co/{company}/{id}`; `eu` for `jobs.eu.lever.co`.
    Lever {
        eu: bool,
        company: String,
        id: String,
    },
    /// `jobs.ashbyhq.com/{org}/{id}`.
    Ashby { org: String, id: String },
    /// A LinkedIn job, from its own page or a search/feed link's `currentJobId`.
    Linkedin { id: String },
    /// Anything else: the page itself.
    Page { url: String },
}

impl Link {
    /// The pasted text as a link, or `None` when it's posting text. A link is one word starting `http(s)://`, or a
    /// bare `host.tld/path` (`linkedin.com/jobs/view/…`).
    pub(crate) fn parse(input: &str) -> Option<Self> {
        let s = input.trim();
        if s.is_empty() || s.contains(char::is_whitespace) {
            return None;
        }
        let lower = s.to_ascii_lowercase();
        let url = if lower.starts_with("https://") || lower.starts_with("http://") {
            Url::parse(s).ok()?
        } else if is_bare_host_path(s) {
            Url::parse(&format!("https://{s}")).ok()?
        } else {
            return None;
        };
        url.host_str()?;
        Some(Self::classify(url))
    }

    fn classify(mut url: Url) -> Self {
        url.set_fragment(None);
        let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
        let segments: Vec<&str> = url
            .path_segments()
            .map(|s| s.filter(|s| !s.is_empty()).collect())
            .unwrap_or_default();
        let query = |key: &str| {
            url.query_pairs()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.into_owned())
        };

        match host.as_str() {
            "job-boards.greenhouse.io" | "boards.greenhouse.io" => {
                if let [board, "jobs", id, ..] = segments.as_slice()
                    && is_digits(id)
                {
                    return Self::Greenhouse {
                        board: (*board).to_owned(),
                        id: (*id).to_owned(),
                    };
                }
                if let (Some(board), Some(id)) = (query("for"), query("token"))
                    && is_digits(&id)
                {
                    return Self::Greenhouse { board, id };
                }
            }
            "jobs.lever.co" | "jobs.eu.lever.co" => {
                if let [company, id, ..] = segments.as_slice()
                    && is_uuid(id)
                {
                    return Self::Lever {
                        eu: host == "jobs.eu.lever.co",
                        company: (*company).to_owned(),
                        id: (*id).to_owned(),
                    };
                }
            }
            "jobs.ashbyhq.com" => {
                if let [org, id, ..] = segments.as_slice()
                    && is_uuid(id)
                {
                    return Self::Ashby {
                        org: (*org).to_owned(),
                        id: id.to_ascii_lowercase(),
                    };
                }
            }
            h if h == "linkedin.com" || h.ends_with(".linkedin.com") => {
                if let Some(id) = query("currentJobId").filter(|id| is_digits(id)) {
                    return Self::Linkedin { id };
                }
                if let Some(id) = linkedin_path_id(&segments) {
                    return Self::Linkedin { id };
                }
            }
            _ => {}
        }
        Self::Page { url: url.into() }
    }

    /// The URL to GET.
    pub(crate) fn request_url(&self) -> String {
        match self {
            Self::Greenhouse { board, id } => {
                format!("https://boards-api.greenhouse.io/v1/boards/{board}/jobs/{id}")
            }
            Self::Lever { eu, company, id } => {
                let host = if *eu {
                    "api.eu.lever.co"
                } else {
                    "api.lever.co"
                };
                format!("https://{host}/v0/postings/{company}/{id}")
            }
            Self::Ashby { org, id } => format!("https://jobs.ashbyhq.com/{org}/{id}"),
            Self::Linkedin { id } => {
                format!("https://www.linkedin.com/jobs-guest/jobs/api/jobPosting/{id}")
            }
            Self::Page { url } => url.clone(),
        }
    }

    /// The posting's own page, for when the source doesn't name one.
    pub(crate) fn page_url(&self) -> String {
        match self {
            Self::Greenhouse { board, id } => {
                format!("https://job-boards.greenhouse.io/{board}/jobs/{id}")
            }
            Self::Lever { eu, company, id } => {
                let host = if *eu {
                    "jobs.eu.lever.co"
                } else {
                    "jobs.lever.co"
                };
                format!("https://{host}/{company}/{id}")
            }
            Self::Ashby { org, id } => format!("https://jobs.ashbyhq.com/{org}/{id}"),
            Self::Linkedin { id } => format!("https://www.linkedin.com/jobs/view/{id}/"),
            Self::Page { url } => url.clone(),
        }
    }
}

/// `example.com/path`: a dotted host of letters, digits and hyphens, a top-level domain of letters, then a path.
fn is_bare_host_path(s: &str) -> bool {
    let Some((host, _)) = s.split_once('/') else {
        return false;
    };
    let labels: Vec<&str> = host.split('.').collect();
    labels.len() >= 2
        && labels
            .iter()
            .all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        && labels
            .last()
            .is_some_and(|tld| tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()))
}

/// `/jobs/view/{id}`, `/jobs/view/{slug}-{id}`, `/comm/jobs/view/{id}` and the guest API's
/// `/jobs-guest/jobs/api/jobPosting/{id}`.
fn linkedin_path_id(segments: &[&str]) -> Option<String> {
    let after = segments
        .windows(2)
        .position(|w| w == ["jobs", "view"])
        .map(|i| i + 2)
        .or_else(|| {
            segments
                .iter()
                .position(|s| *s == "jobPosting")
                .map(|i| i + 1)
        })?;
    let last = segments.get(after)?;
    let id = last.rsplit('-').next()?;
    is_digits(id).then(|| id.to_owned())
}

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// `8-4-4-4-12` hex digits.
fn is_uuid(s: &str) -> bool {
    let groups: Vec<&str> = s.split('-').collect();
    groups.len() == 5
        && groups
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(g, n)| g.len() == n && g.bytes().all(|b| b.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(s: &str) -> Link {
        Link::parse(s).unwrap_or_else(|| panic!("{s} should be a link"))
    }

    #[test]
    fn text_is_not_a_link() {
        assert_eq!(
            Link::parse("Senior engineer, https://example.com/jobs/1"),
            None
        );
        assert_eq!(Link::parse("We are hiring."), None);
        assert_eq!(Link::parse("example.com"), None);
        assert_eq!(Link::parse("v1.2/notes"), None);
        assert_eq!(Link::parse(""), None);
        assert_eq!(Link::parse("ftp://example.com/job"), None);
    }

    #[test]
    fn greenhouse() {
        let want = Link::Greenhouse {
            board: "perfectserve".into(),
            id: "6188374004".into(),
        };
        assert_eq!(
            link("https://job-boards.greenhouse.io/perfectserve/jobs/6188374004"),
            want
        );
        assert_eq!(
            link("https://boards.greenhouse.io/perfectserve/jobs/6188374004?gh_src=x#app"),
            want
        );
        assert_eq!(
            link("https://boards.greenhouse.io/embed/job_app?for=perfectserve&token=6188374004"),
            want
        );
        assert_eq!(
            want.request_url(),
            "https://boards-api.greenhouse.io/v1/boards/perfectserve/jobs/6188374004"
        );
        // A board's index is a page.
        assert!(matches!(
            link("https://job-boards.greenhouse.io/perfectserve"),
            Link::Page { .. }
        ));
    }

    #[test]
    fn lever() {
        let l = link("https://jobs.lever.co/spotify/2193db3f-77c5-43b8-b030-8f92c9882bf1/apply");
        assert_eq!(
            l,
            Link::Lever {
                eu: false,
                company: "spotify".into(),
                id: "2193db3f-77c5-43b8-b030-8f92c9882bf1".into(),
            }
        );
        assert_eq!(
            l.request_url(),
            "https://api.lever.co/v0/postings/spotify/2193db3f-77c5-43b8-b030-8f92c9882bf1"
        );
        assert_eq!(
            l.page_url(),
            "https://jobs.lever.co/spotify/2193db3f-77c5-43b8-b030-8f92c9882bf1"
        );
        let eu = link("jobs.eu.lever.co/acme/2193db3f-77c5-43b8-b030-8f92c9882bf1");
        assert!(
            eu.request_url()
                .starts_with("https://api.eu.lever.co/v0/postings/acme/")
        );
    }

    #[test]
    fn ashby() {
        let l = link(
            "https://jobs.ashbyhq.com/runpod/B64A3935-931a-4b7f-9899-7ebe152164c5/application",
        );
        assert_eq!(
            l,
            Link::Ashby {
                org: "runpod".into(),
                id: "b64a3935-931a-4b7f-9899-7ebe152164c5".into(),
            }
        );
        assert_eq!(
            l.request_url(),
            "https://jobs.ashbyhq.com/runpod/b64a3935-931a-4b7f-9899-7ebe152164c5"
        );
    }

    #[test]
    fn linkedin() {
        let want = Link::Linkedin {
            id: "4475737628".into(),
        };
        let search = "https://www.linkedin.com/jobs/search-results/?currentJobId=4475737628&eBP=NOT_ELIGIBLE_FOR_CHARGING&refId=abc%3D%3D&trackingId=def&keywords=software%20engineer&origin=JOB_SEARCH_PAGE_JOB_FILTER";
        assert_eq!(link(search), want);
        assert_eq!(
            link("https://www.linkedin.com/jobs/collections/recommended/?currentJobId=4475737628"),
            want
        );
        assert_eq!(link("linkedin.com/jobs/view/4475737628"), want);
        assert_eq!(
            link("https://www.linkedin.com/jobs/view/software-engineer-at-vilo-4475737628/?trk=x"),
            want
        );
        assert_eq!(
            link("https://www.linkedin.com/comm/jobs/view/4475737628"),
            want
        );
        assert_eq!(
            link("https://www.linkedin.com/jobs-guest/jobs/api/jobPosting/4475737628"),
            want
        );
        assert_eq!(
            want.request_url(),
            "https://www.linkedin.com/jobs-guest/jobs/api/jobPosting/4475737628"
        );
        assert_eq!(
            want.page_url(),
            "https://www.linkedin.com/jobs/view/4475737628/"
        );
        assert!(matches!(
            link("https://www.linkedin.com/company/viloai"),
            Link::Page { .. }
        ));
    }

    #[test]
    fn other_pages() {
        assert_eq!(
            link("https://gcrealtyinc.applicantpro.com/jobs/4234604-460188.html#top"),
            Link::Page {
                url: "https://gcrealtyinc.applicantpro.com/jobs/4234604-460188.html".into()
            }
        );
        assert_eq!(
            link("www.indeed.com/viewjob?jk=abc"),
            Link::Page {
                url: "https://www.indeed.com/viewjob?jk=abc".into()
            }
        );
    }
}
