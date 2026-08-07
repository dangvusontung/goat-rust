# TASK-BRIDGE-refresh Slice B.0 — log (2026-08-07T17:28:38+07:00)

## toolchain
```
Flutter/Dart: Dart SDK version: 3.12.2 (stable) (Tue Jun 9 01:11:39 2026 -0700) on "linux_x64"
codegen: flutter_rust_bridge_codegen 2.9.0 (cargo install --locked; REPLACED a previously installed 2.12.0)
```

## codegen run (from app/)
```text
$ flutter_rust_bridge_codegen generate
... Done!
outputs: app/lib/src/rust/{api.dart, frb_generated.dart, frb_generated.io.dart, frb_generated.web.dart} + crates/goat-bridge/src/frb_generated.rs regenerated
```

## drift found & corrected by the regen
```
crates/goat-bridge/Cargo.toml had flutter_rust_bridge = "=2.12.0" (drift from the
documented pin) — codegen auto-aligned it to the pinned =2.9.0; Cargo.lock follows.
frb_generated.rs diff vs committed: version stamps + cosmetic codegen style only
(Vec::with_capacity -> vec![]), no API changes.
```

## gate: build + tests
```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.54s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
workspace suites ok: 39
only failure: smoke_stdin 10 pre-existing (baseline unchanged)
```

## flutter pub get (app/)
```
Changed 13 dependencies! (resolves flutter_rust_bridge 2.9.0 from pub.dev)
```

---

# B.1 economy/sponsors/life — log (2026-08-07T17:52:31+07:00)

## codegen regen after DTO change (D2 loop proof)
```text
$ cd app && flutter_rust_bridge_codegen generate
Done!  (frb_generated.rs + Dart bindings regenerated for the 6 new GoatGameState fields + 7 new pub fns)
```

## cargo test -p goat-bridge
```
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test bridge_phase10_actions_match_direct_reduce ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## workspace
```
suites ok: 39
only failure: smoke_stdin 10 pre-existing (baseline unchanged)
```

## fmt/clippy
```
fmt clean
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.09s
```
