# goat-bridge — FFI surface for the Flutter client

`api.rs` is hand-written; `frb_generated.rs` is **codegen output — never
hand-edit it**. It is field-locked: any DTO/`pub fn` change breaks its
compilation until regenerated.

## Regenerating after an api.rs change

Prereq (once): `cargo install flutter_rust_bridge_codegen --version 2.9.0`
(must match the pinned `flutter_rust_bridge = "=2.9.0"` in Cargo.toml).

```bash
cd app/            # the minimal Dart target (pubspec + flutter_rust_bridge.yaml)
flutter_rust_bridge_codegen generate
cd .. && cargo build -p goat-bridge && cargo test -p goat-bridge
```

This refreshes `crates/goat-bridge/src/frb_generated.rs` and the Dart bindings
under `app/lib/src/rust/`. Commit both. The codegen auto-aligns the
`flutter_rust_bridge` version in Cargo.toml to the pinned 2.9.0 — do not bump
it manually without asking.
