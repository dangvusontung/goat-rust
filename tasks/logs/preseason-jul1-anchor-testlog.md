# Pre-season (Jul-1 anchor) — test log (2026-08-06T22:34:56+07:00)

## cargo fmt / clippy
```
fmt clean
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.11s
```

## cargo test --workspace
```
suites ok: 39
only failure: smoke_stdin 10 pre-existing (baseline diff: BASELINE_UNCHANGED)
```

## calendar unit tests (re-derived for Jul-1 anchor + 45-week grid)
```
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 127 filtered out; finished in 0.00s
opener 2025: Sat 30 Aug 2025 (week 7 slot 0); two-match week 8: Tue 2 Sep / Sat 6 Sep 2025
```

## window remap (goat-core calendar_loop tests)
```
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 47 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 17 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s
InternationalBreak 79-93, TransferWinter 209-244, OffSeason 318-347, TransferSummer 348-364
```

## TUI scripted flow (real probe)
```
pre-season: menu shows "(Pre-season week N/7 — training and friendlies only)"; [C] ticks a real week; [P] plays "--- FRIENDLY (pre-season) · Draymoor Rovers vs Redcliffe Town ---" to FULL TIME; S1 Round 1/38 untouched by the friendly; after 7 ticks: "Pre-season complete — the league campaign opens this week."
```

## smoke_stdin task tests (all pass)
```
test continue_trains_week_and_offers_match_once_in_one_match_week ... ok
test continue_two_match_week_stops_once_per_match ... ok
test preseason_friendly_is_playable_and_leaves_the_league_untouched ... ok
test continue_break_week_elapses_without_a_keypress ... ok
test promotion_relegation_fires_at_season_boundary_and_applies_once ... ok
test promoted_clubs_appear_in_next_season_table ... ok
```
