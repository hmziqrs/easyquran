# native/vendor

SuperNSH crates the app depends on whose published releases still require `nsh-core ^0.6.0`. Left as they are they would put a second, incompatible `nsh-core` in the graph, so their published sources live here on NSH 0.7.1 until each has its own 0.7 release. Do not develop them here.

`src/` (and `tsx/`, `android/`, `nsh.json` where the crate ships them) is the source of the crates.io `.crate`; tests, examples, README and `Cargo.lock` are left out. `LICENSE` is the upstream repository's (the `.crate` files carry none).

| Crate | Upstream | Local changes to the source |
| --- | --- | --- |
| `nsh-auth` | 0.1.0 | none |
| `nsh-connectivity` | 0.0.1 | none |
| `nsh-deep-links` | 0.1.0 | `src/platform/apple.rs`, below |
| `nsh-firebase-analytics` | 0.1.0 | none |
| `nsh-firebase-core` | 0.1.0 | none |
| `nsh-firebase-messaging` | 0.1.0 | none |
| `nsh-form` | 0.1.0 | none |
| `nsh-query` | 0.1.0 | none |
| `nsh-secure-storage` | 0.1.0 | none |

Every `Cargo.toml` is the upstream `Cargo.toml.orig` with: `nsh-core` / `nsh-base` at `0.7.1` (was `0.6.0`), the values it inherited from its workspace written out, `readme`, `[lints]` and the dev-dependencies removed, `publish = false` added, and `nsh-firebase-core` by path in the two crates that use it. The inline `#[cfg(test)]` modules are unchanged and are not run (the crates are not workspace members).

`nsh-deep-links`, `src/platform/apple.rs`: NSH 0.7's macOS and iOS hosts hand Universal Links of `nsh.json` `"links"` to core, which the crate already reads. Its own `continueUserActivity` hook now takes a link only where no host implementation did; before, each Universal Link would have been delivered twice. Its inline test of the hook keeps upstream's expectations and, for a host that returns `NO`, still holds.

## How they are wired

`path` next to `version` in `native/Cargo.toml`, not `[patch.crates-io]`: `nsh run` / `nsh package` build the app from a workspace of their own (`.nsh/rust`), which carries path dependencies over but not this workspace's `[patch]` table, so a patched app would get the crates.io copies (and the second `nsh-core`) there. `exclude = ["vendor"]` keeps them out of the workspace; `rustfmt.toml` keeps `cargo fmt --all`, which follows path dependencies, from rewriting them.

## Removing one

When its release for NSH 0.7 is out: drop the `path` from its line in `native/Cargo.toml`, set the new version, delete its directory here. `nsh-firebase-core` goes together with `nsh-firebase-analytics` and `nsh-firebase-messaging`. With the last one, delete this directory, the `exclude` and the comments in `native/Cargo.toml`.
