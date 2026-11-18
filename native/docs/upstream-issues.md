# Upstream issues

Bugs and sharp edges found in the sibling libraries `nsh-query` and `nsh-form` while building this app. Both repos are read-only for us: if something here blocks you, work around it in your area and add a row below with a reproduction and the workaround. Do not fork or patch the libraries.

Format: one row per issue — what breaks, how to reproduce it, the workaround this app uses, and whether it is still open. When the library fixes one, move it to the fixed list with the version that fixed it.

## Open

*(none recorded yet — this page is written from the areas' `docs/STATUS.md` notes during the audit pass)*

## NSH 0.4.0

Re-checked after the move to 0.5.0 (2026-11-11). The three entries still marked Open need a device run that the migration did not do; they are unverified on 0.5.0, not confirmed.

- **Android ignores a parent's `overflow_hidden` for a child moved by `bind_translate_x`.** The translated child draws past the clip. Workaround: `src/features/motion/reduced.rs` hides the drawer at opacity 0 while it rests shut. Open.
- **iOS: a canvas stroke swipes its page back.** A responder that blocks native gestures (`claim_touches`, as `src/features/canvas/sketch.rs` uses) keeps scroll views still but not the stack's swipe back, and on iOS 26 and later a rightward drag anywhere in the page starts it. Reproduced on NSH `main` (7cd08016) in an iOS 27.0 simulator; the code is the same in 0.4.0 and 0.5.0. Filed as [longbridge/nsh#1756](https://github.com/longbridge/nsh/issues/1756), fix proposed in [longbridge/nsh#1757](https://github.com/longbridge/nsh/pull/1757). No workaround in this app yet; `pop_on_swipe(false)` on the page would stop it at the cost of the swipe back. Open.
- **The dev APK is about 3 GB** (unstripped debug `libnsh.so` for three ABIs), which fails with `INSTALL_FAILED_INSUFFICIENT_STORAGE` on a default 6 GB AVD data partition. Workaround: create the AVD with a 20 GB data partition. Open.

## Known constraints (not bugs, but they bite)

These come from NSH itself and shape how the libraries are used here; `docs/PROMPT.md` "Facts that bite" is the full list.

- `Signal::update` needs `T: Clone`. State machines holding non-cloneable user types pair `stored(..)` with a version signal (see `~/fo/nsh-query/crates/nsh-query/src/tracked.rs` for the pattern nsh-query ships).
- A component's signals are disposed when its `cleanup` runs: reading one there panics. Capture what cleanup needs in an `Rc<Cell<..>>`, and use `create_root` for an owner that outlives the component (nsh-query uses this for caches and in-flight writes).

## Fixed

- **`nsh-platform-ios` 0.4.0 does not build from crates.io.** `src/lib.rs:45` has `#[path = "../../nsh-platform-macos/src/speech.rs"]`, a path that exists in NSH's workspace but not in the registry, where the directory is `nsh-platform-macos-0.4.0`. `nsh test --target ios` and `nsh run --target ios` fail with "couldn't find file ... nsh-platform-macos/src/speech.rs". Workaround on this machine: a symlink in the cargo registry, `~/.cargo/registry/src/index.crates.io-*/nsh-platform-macos -> nsh-platform-macos-0.4.0`. **Fixed in 0.5.0**: `nsh-platform-ios-0.5.0/src/lib.rs` has no `#[path]` any more, and `cargo check --lib --target aarch64-apple-ios-sim` passes on 0.5.0 (`docs/evidence/project/nsh-0.5.0/phase3-check-aarch64-apple-ios-sim.log`). The registry symlink only served 0.4.0 and is no longer needed by this repo; it was left in place because the cargo registry is shared.
