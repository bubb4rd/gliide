//! Bank and master types, `resolve`, the v1 migrator (ARCHITECTURE.md §2), the posting and requirements types (§3,
//! §7), and the literal matcher, composite score and master ranking (§4).

pub mod bank;
pub mod dates;
pub mod master;
pub mod matcher;
pub mod migrate;
pub mod posting;
pub mod rank;
pub mod requirements;
pub mod resolve;
pub mod resume;
pub mod score;
pub mod text;

pub use bank::Bank;
pub use master::Master;
pub use migrate::{Migrated, migrate_v1};
pub use posting::{Method, Posting};
pub use rank::rank;
pub use requirements::Requirements;
pub use resolve::{ResolveError, resolve};
pub use resume::Resume;
pub use score::{PageFacts, Score, score};
pub use text::resume_text;

/// JSON Schema of the bank document (`schemas/bank.schema.json`).
pub fn bank_schema() -> schemars::Schema {
    schemars::schema_for!(Bank)
}

/// JSON Schema of a master document (`schemas/master.schema.json`).
pub fn master_schema() -> schemars::Schema {
    schemars::schema_for!(Master)
}
