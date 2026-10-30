//! A fake agent CLI: a shell script that records each call's arguments, stdin and the files its arguments name, then
//! prints the reply queued for that call and exits with its status.
#![allow(dead_code)]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

const SCRIPT: &str = r#"#!/bin/sh
dir="$(dirname "$0")"
n=$(( $(cat "$dir/count" 2>/dev/null || echo 0) + 1 ))
echo "$n" > "$dir/count"
for a in "$@"; do printf '%s\n' "$a"; done > "$dir/args.$n"
prev=""
for a in "$@"; do
  [ "$prev" = "--output-schema" ] && cp "$a" "$dir/schema.$n"
  case "$a" in model_instructions_file=*)
    p="${a#model_instructions_file=}"; p="${p#\"}"; p="${p%\"}"; cp "$p" "$dir/instructions.$n";;
  esac
  prev="$a"
done
cat > "$dir/stdin.$n"
if [ -f "$dir/sleep" ]; then sleep "$(cat "$dir/sleep")"; fi
reply="$dir/reply.$n"; [ -f "$reply" ] || reply="$dir/reply"
cat "$reply"
stderr="$dir/stderr"; [ -f "$stderr" ] && cat "$stderr" >&2
status="$dir/status.$n"; [ -f "$status" ] || status="$dir/status"
exit "$(cat "$status" 2>/dev/null || echo 0)"
"#;

pub struct Fake {
    dir: tempfile::TempDir,
    bin: PathBuf,
}

impl Fake {
    /// A fake named `name` (`claude`, `codex`, `agy`).
    pub fn new(name: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join(name);
        std::fs::write(&bin, SCRIPT).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        Self { dir, bin }
    }

    pub fn bin(&self) -> &PathBuf {
        &self.bin
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// Queues `body` for call `n` (or every call, with `None`).
    pub fn reply(&self, n: Option<u32>, body: &str) -> &Self {
        let name = n.map_or("reply".to_owned(), |n| format!("reply.{n}"));
        std::fs::write(self.path(&name), body).unwrap();
        self
    }

    pub fn stderr(&self, text: &str) -> &Self {
        std::fs::write(self.path("stderr"), text).unwrap();
        self
    }

    pub fn status(&self, code: i32) -> &Self {
        std::fs::write(self.path("status"), code.to_string()).unwrap();
        self
    }

    pub fn sleep(&self, secs: u32) -> &Self {
        std::fs::write(self.path("sleep"), secs.to_string()).unwrap();
        self
    }

    pub fn calls(&self) -> u32 {
        std::fs::read_to_string(self.path("count")).map_or(0, |s| s.trim().parse().unwrap())
    }

    /// What the fake recorded: `args.N`, `stdin.N`, `schema.N`, `instructions.N`.
    pub fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.path(name)).unwrap()
    }

    /// Call `n`'s arguments, one per item.
    pub fn args(&self, n: u32) -> Vec<String> {
        self.read(&format!("args.{n}"))
            .lines()
            .map(str::to_owned)
            .collect()
    }
}
