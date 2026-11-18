//! The fixtures every benchmark run reads, built into the binary: the postings, the bank with its `swe` master, the
//! trimmed master bank-aware Suggest tailors, and the hand-checked requirements per posting.

use std::collections::BTreeMap;

use resume_core::{Bank, Master, Method, Posting, Requirements, Resume, migrate_v1, resolve};

use crate::BenchError;

/// Where the gold requirements live in the repo, for reports that name it.
pub const GOLD_DIR: &str = "fixtures/requirements/gold";

/// `(id, posting text, gold requirements JSON)` per posting fixture (its file stem): a missing gold file fails the build,
/// so every role reads gold.
macro_rules! postings {
    ($($id:literal),* $(,)?) => {
        const POSTINGS: &[(&str, &str, &str)] = &[$((
            $id,
            include_str!(concat!("../../../fixtures/postings/", $id, ".md")),
            include_str!(concat!("../../../fixtures/requirements/gold/", $id, ".json")),
        )),*];
    };
}

postings!(
    "acme-swe-robotics",
    "aperture-swe-ai-wearable",
    "contoso-dev-integrations-ai",
    "fabrikam-mobile-dev-1",
    "globex-swe",
    "hooli-fullstack-swe",
    "initech-entry-swe",
    "northwind-jr-swe",
    "stark-ux-engineer",
    "vandelay-systems-automation",
    "wayne-ui-developer",
    "wonka-frontend-ui-designer",
);

const BANK_V1: &str = include_str!("../../../fixtures/banks/jane-doe.v1.json");
/// The `swe` master without its project, so bank-aware Suggest has a candidate.
const TRIM_MASTER: &str = include_str!("../../../fixtures/banks/jane-doe.swe-trim.master.json");

/// One posting.
pub struct Item {
    pub id: String,
    pub posting: Posting,
}

/// Everything the roles read: the postings in id order, the bank migrated from the v1 fixture with its `swe` master,
/// the trimmed master, and the gold requirements by posting id.
pub struct Fixtures {
    pub postings: Vec<Item>,
    pub bank: Bank,
    pub master: Master,
    /// `master` resolved: what Write and Coach send.
    pub resume: Resume,
    /// The `swe` master without its project: what bank-aware Tailor tailors.
    pub trim: Master,
    pub gold: BTreeMap<String, Requirements>,
}

impl Fixtures {
    /// The fixtures built into the crate, parsed.
    pub fn embedded() -> Result<Self, BenchError> {
        let json = |name: &str, e: serde_json::Error| BenchError::Fixture {
            name: name.to_owned(),
            message: e.to_string(),
        };
        let v1: Resume =
            serde_json::from_str(BANK_V1).map_err(|e| json("banks/jane-doe.v1.json", e))?;
        let m = migrate_v1(&v1);
        let resume = resolve(&m.bank, &m.master)?;
        let trim: Master = serde_json::from_str(TRIM_MASTER)
            .map_err(|e| json("banks/jane-doe.swe-trim.master.json", e))?;
        resolve(&m.bank, &trim)?;
        let postings = POSTINGS
            .iter()
            .map(|(stem, text, _)| parse_posting(stem, text))
            .collect::<Result<Vec<_>, _>>()?;
        let gold = POSTINGS
            .iter()
            .map(|(stem, _, gold)| {
                serde_json::from_str(gold)
                    .map(|r| ((*stem).to_owned(), r))
                    .map_err(|e| json(&format!("requirements/gold/{stem}.json"), e))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            postings,
            bank: m.bank,
            master: m.master,
            resume,
            trim,
            gold,
        })
    }

    /// The postings `only` names, or all of them.
    pub fn chosen(&self, only: &[String]) -> Vec<&Item> {
        self.postings
            .iter()
            .filter(|f| only.is_empty() || only.contains(&f.id))
            .collect()
    }

    /// Posting `id`.
    pub fn posting(&self, id: &str) -> Option<&Item> {
        self.postings.iter().find(|f| f.id == id)
    }

    /// Posting `id` and its gold requirements: what Tailor, Write and Coach read.
    pub fn item(&self, id: &str) -> Result<(&Item, &Requirements), BenchError> {
        self.posting(id)
            .zip(self.gold.get(id))
            .ok_or_else(|| BenchError::UnknownItem(id.to_owned()))
    }
}

/// A posting fixture as a `Posting`: front matter `method: pasted text`, or none, is pasted text with no metadata; any
/// other method passes the front matter's title, company and location as hints (as the Extract spike built them).
fn parse_posting(stem: &str, text: &str) -> Result<Item, BenchError> {
    let (front, body) = split_front_matter(text);
    let field = |key: &str| {
        front
            .lines()
            .find_map(|l| l.strip_prefix(key).and_then(|r| r.strip_prefix(':')))
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
    };
    let id = field("id").unwrap_or_else(|| stem.to_owned());
    let method = match field("method") {
        None => Method::Pasted,
        Some(m) => method_of(&m).ok_or_else(|| BenchError::Fixture {
            name: format!("postings/{stem}.md"),
            message: format!("unknown method `{m}`"),
        })?,
    };
    let posting = match method {
        Method::Pasted => Posting::pasted(body.to_owned()),
        m => {
            let source = field("source");
            Posting::new(
                m,
                source.clone(),
                source,
                field("title"),
                field("company"),
                field("location"),
                body.to_owned(),
            )
        }
    };
    Ok(Item { id, posting })
}

/// The front matter's `method` ("greenhouse board API", "applicantpro JSON-LD", "pasted text") as a `Method`.
fn method_of(s: &str) -> Option<Method> {
    let s = s.to_lowercase();
    let has = |k: &str| s.contains(k);
    Some(if has("pasted") {
        Method::Pasted
    } else if has("greenhouse") {
        Method::Greenhouse
    } else if has("lever") {
        Method::Lever
    } else if has("ashby") {
        Method::Ashby
    } else if has("linkedin") {
        Method::Linkedin
    } else if has("workday") {
        Method::Workday
    } else if has("json-ld") {
        Method::JsonLd
    } else if has("html") {
        Method::Html
    } else {
        return None;
    })
}

fn split_front_matter(text: &str) -> (&str, &str) {
    let Some(rest) = text.strip_prefix("---\n") else {
        return ("", text);
    };
    match rest.find("\n---\n") {
        Some(end) => (&rest[..end], &rest[end + 5..]),
        None => ("", text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_embedded_posting_has_gold() {
        let fx = Fixtures::embedded().unwrap();
        assert_eq!(fx.postings.len(), 12);
        for p in &fx.postings {
            assert!(fx.item(&p.id).is_ok(), "{} has no gold", p.id);
        }
    }
}
