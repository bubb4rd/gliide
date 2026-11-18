//! The reference: every route's results from the newest `ez eval` runs, shipped with the app so a route scores before
//! you run it yourself. `ez eval reference` writes `reference.json`.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::{Role, RouteResult};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    /// `YYYY-MM-DD` of the newest runs it holds.
    pub date: String,
    /// Role → route (`agent:model`) → its results.
    pub roles: BTreeMap<Role, BTreeMap<String, RouteResult>>,
}

/// The shipped reference, parsed on first use.
pub fn reference() -> &'static Reference {
    static REFERENCE: LazyLock<Reference> = LazyLock::new(|| {
        serde_json::from_str(include_str!("../reference.json"))
            .expect("reference.json parses (its test checks it)")
    });
    &REFERENCE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_reference_parses() {
        let r = reference();
        assert_eq!(r.date.len(), 10, "{}", r.date);
        for role in Role::ALL {
            assert!(
                r.roles.get(&role).is_some_and(|rs| !rs.is_empty()),
                "{role:?}"
            );
        }
    }
}
