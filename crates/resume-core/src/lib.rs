//! Bank and master types, `resolve`, and the v1 migrator (ARCHITECTURE.md §2).

pub mod bank;
pub mod dates;
pub mod master;
pub mod migrate;
pub mod resolve;
pub mod resume;

pub use bank::Bank;
pub use master::Master;
pub use migrate::{Migrated, migrate_v1};
pub use resolve::{ResolveError, resolve};
pub use resume::Resume;

/// JSON Schema of the bank document (`schemas/bank.schema.json`).
pub fn bank_schema() -> schemars::Schema {
    schemars::schema_for!(Bank)
}

/// JSON Schema of a master document (`schemas/master.schema.json`).
pub fn master_schema() -> schemars::Schema {
    schemars::schema_for!(Master)
}
