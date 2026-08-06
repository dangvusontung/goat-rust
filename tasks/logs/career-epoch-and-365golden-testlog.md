# Career epoch (1a) + 365-day training golden — test log (2026-08-06T21:42:42+07:00)

## cargo fmt --check
```
(clean)
```

## cargo clippy --workspace --all-targets -- -D warnings (tail)
```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.09s
```

## cargo test --workspace
```
suites ok: 39
only failure: goat-tui --test smoke_stdin — 10 pre-existing (baseline diff: BASELINE_UNCHANGED)
```

## goat-save v19 tests
```
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test old_v18_save_without_career_base_year_defaults_to_2025 ... ok
test save_load_restores_career_base_year_through_bytes ... ok
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## goat-training 365-day golden
```
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
frozen: attr 70.010, energy 96.000, 15 soft days, breakthroughs [129, 364] ("reaches 65" / "reaches 70" on the year's last day)
```

## Wall-clock layering grep (must be empty in core crates)
```
$ grep -rn "SystemTime\|now()\|chrono" crates/goat-core/src crates/goat-world/src crates/goat-match/src crates/goat-training/src crates/goat-save/src
(no matches)
```

## TUI probe (real year flows)
```
scripted new game seed 42 → "--- ROUND 1 / 38 · Game Week 1 · Aug 2026 ---" (host year: 2026)
```

## Web node smoke (after new_game base_year param)
```
PASS  table survives roundtrip

SMOKE OK
```
