//! eq-core's build-time generators. Each generator lives in `gen/<name>.rs`
//! (not `build/`: the repo's root `.gitignore` ignores every `build` dir),
//! reads its sources by path relative to this crate, prints its own
//! `cargo:rerun-if-changed` lines and writes one file into `OUT_DIR`, which
//! the matching module `include!`s. A failing generator stops the build with
//! its message: a generated table is never silently empty.
//!
//! Generators:
//! - `i18n`: `web/messages/**` (the inlang catalogues) into `i18n_table.rs`
//!   for `src/i18n`.
//!
//! To add one: put `gen/<name>.rs` beside `gen/i18n.rs` with a
//! `pub fn generate(crate_dir: &Path, out_dir: &Path) -> Result<(), String>`,
//! declare it below and add it to `GENERATORS`.

use std::{
    env,
    path::{Path, PathBuf},
};

#[path = "gen/i18n.rs"]
mod i18n;

type Generator = fn(&Path, &Path) -> Result<(), String>;

const GENERATORS: &[(&str, Generator)] = &[("i18n", i18n::generate)];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=gen");
    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    for (name, generate) in GENERATORS {
        if let Err(message) = generate(&crate_dir, &out_dir) {
            eprintln!("eq-core build: generator `{name}` failed:\n{message}");
            std::process::exit(1);
        }
    }
}
