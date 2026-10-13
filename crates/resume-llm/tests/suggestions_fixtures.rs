//! Every recorded Suggest output in `fixtures/suggestions/` is valid against `prompts/suggest.schema.json` and
//! deserializes into `resume_core::suggestions::Suggestions`, so the schema and the type accept the same shape.

use std::path::{Path, PathBuf};

use resume_core::suggestions::Suggestions;
use resume_llm::suggest;
use serde_json::Value;

fn json_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let path = entry.unwrap().path();
        if path.is_dir() {
            json_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "json") {
            out.push(path);
        }
    }
}

#[test]
fn every_fixture_fits_the_schema_and_the_type() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/suggestions");
    let mut files = Vec::new();
    json_files(&dir, &mut files);
    files.sort();
    assert!(!files.is_empty(), "no fixtures under {}", dir.display());

    let schema = suggest::schema();
    let mut failures = Vec::new();
    for file in &files {
        let name = file.strip_prefix(&dir).unwrap().display().to_string();
        let value: Value = match serde_json::from_str(&std::fs::read_to_string(file).unwrap()) {
            Ok(v) => v,
            Err(e) => {
                failures.push(format!("{name}: not JSON: {e}"));
                continue;
            }
        };
        for e in schema.errors(&value) {
            failures.push(format!("{name}: schema: {e}"));
        }
        if let Err(e) = serde_json::from_value::<Suggestions>(value) {
            failures.push(format!("{name}: Suggestions: {e}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} problems in {} fixtures:\n{}",
        failures.len(),
        files.len(),
        failures.join("\n")
    );
}
