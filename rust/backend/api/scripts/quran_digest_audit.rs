#!/usr/bin/env cargo --bin quran_digest_audit

use sha2::{Digest, Sha256};

const GOLDEN_UTHMANI_CORPUS: &str = "32cc746d817cad9fd4366c7597bfceb177e7649233616c0a80309074b2eb99ee";
const GOLDEN_SIMPLE_CLEAN_CORPUS: &str =
    "375934722ccbfab0d97754df464deac0dcffe962dc0632cc1ce5c6ca25dcea67";
const GOLDEN_UTHMANI_FILE: &str =
    "581cc5405831bc072fccd8db55cd1db72c5c5440c39bd975edbf03447efecf53";
const GOLDEN_SIMPLE_CLEAN_FILE: &str =
    "a0c52760d6660ac5be1de5c76bb10df7a839a3e8a87ecb0e636fe2ed45b2e4a3";

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

fn check(label: &str, got: &str, want: &str) -> bool {
    let ok = got == want;
    if ok {
        println!("PASS  {label}");
    } else {
        println!("FAIL  {label}\n        got  {got}\n        want {want}");
    }
    ok
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    let settings = ruxlog::config::Settings::from_env();
    let qset = &settings.quran;
    let store = ruxlog::quran::load_quran_store(qset).await?;

    let mut ok = true;
    ok &= check(
        "uthmani corpus",
        &sha256_hex(store.uthmani.joined_for_digest().as_bytes()),
        GOLDEN_UTHMANI_CORPUS,
    );
    ok &= check(
        "simple-clean corpus",
        &sha256_hex(store.simple_clean.joined_for_digest().as_bytes()),
        GOLDEN_SIMPLE_CLEAN_CORPUS,
    );
    ok &= check(
        "uthmani file",
        &sha256_hex(&std::fs::read(&qset.uthmani_path)?),
        GOLDEN_UTHMANI_FILE,
    );
    ok &= check(
        "simple-clean file",
        &sha256_hex(&std::fs::read(&qset.simple_clean_path)?),
        GOLDEN_SIMPLE_CLEAN_FILE,
    );

    if ok {
        println!("\nAll Arabic golden digests verified.");
        Ok(())
    } else {
        std::process::exit(1);
    }
}
