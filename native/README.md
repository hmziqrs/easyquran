# EasyQuran native

The EasyQuran app for macOS, iOS and Android (Linux and Windows from the same crate), built on [Native Shell](https://nsh.dev) 0.5.1 in Rust mode. It is the native counterpart of `web/` and aims at 1:1 feature parity.

- What must be built, and its state: [PARITY.md](PARITY.md)
- Rules, layout and gates for anyone working here: [AGENTS.md](AGENTS.md)
- The shell, UI wrappers and foundation started from the MIT-licensed nsh-starter: [LICENSE.nsh-starter](LICENSE.nsh-starter)

## Layout

- `crates/eq-core`: domain logic in plain Rust (no NSH), tested with `cargo test -p eq-core`.
- `src/`: the NSH app crate: shell, UI wrappers, services over NSH, stores, feature screens.

## Run

```sh
cargo run                       # desktop, on this machine
nsh run --target ios            # iOS simulator
nsh run --target android        # first adb device
```

Quran databases are provisioned into `assets/quran/` out of band and are never committed.

## Gates

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`just check` and `just test` run the same.
