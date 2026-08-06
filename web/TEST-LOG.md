# GOAT web demo — test log

Captured 2026-08-06 on the workspace host (Linux, wasm-pack 0.15.0, node on
PATH). All outputs below are real captured command output, unedited except for
stripping `Compiling …` lines from the wasm-pack logs.

## cargo fmt

```
$ cargo fmt --check
(no output — clean)
```

## cargo clippy (whole workspace, all targets, including goat-web)

```
$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.11s
```

## cargo test --workspace

37 test suites pass. The only failure is the known pre-existing baseline:
`goat-tui --test smoke_stdin` with exactly 10 failures (present on clean HEAD,
unrelated to this change).

```
$ cargo test --workspace
suites passing: 37

--- tail ---
failures:
    confirm_screen_blank_enter_reprompts_instead_of_discarding_character
    double_w_in_same_round_shows_message_not_silent_noop
    game_sheet_and_player_sheet_boxes_close_for_short_and_long_club_names
    key_moments_lines_close_with_ellipsis_not_ragged_cutoff
    legacy_screen_notes_mid_season_batching
    main_loop_unrecognized_command_messages_and_continues
    player_sheet_explains_ovr_is_position_weighted
    save_overwrite_requires_explicit_confirmation
    save_to_empty_slot_succeeds_without_confirmation
    status_header_shows_energy_percent_and_labeled_discipline_count

test result: FAILED. 10 passed; 10 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s

error: test failed, to rerun pass `-p goat-tui --test smoke_stdin`
```

## wasm-pack builds

```
$ wasm-pack build crates/goat-web --target web --out-dir ../../web/pkg
    Finished `release` profile [optimized] target(s) in 0.04s
[INFO]: ⬇️  Installing wasm-bindgen...
[INFO]: Optimizing wasm binaries with `wasm-opt`...
[INFO]: Optional fields missing from Cargo.toml: 'description', 'repository', and 'license'. These are not necessary, but recommended
[INFO]: ✨   Done in 2.72s
[INFO]: 📦   Your wasm pkg is ready to publish at web/pkg.

-rw-rw-r-- 1 tungdvs tungdvs 396182 Aug  6 19:57 web/pkg/goat_web_bg.wasm
```

```
$ wasm-pack build crates/goat-web --target nodejs --out-dir ../../web/pkg-node
    Finished `release` profile [optimized] target(s) in 0.03s
[INFO]: ⬇️  Installing wasm-bindgen...
[INFO]: Optimizing wasm binaries with `wasm-opt`...
[INFO]: Optional fields missing from Cargo.toml: 'description', 'repository', and 'license'. These are not necessary, but recommended
[INFO]: ✨   Done in 2.69s
[INFO]: 📦   Your wasm pkg is ready to publish at web/pkg-node.

-rw-rw-r-- 1 tungdvs tungdvs 396182 Aug  6 19:57 web/pkg-node/goat_web_bg.wasm
```

## Node smoke test (full season + boundary + save/load roundtrip)

`web/smoke.mjs`: new game at seed 42 (ST, first nation / league / club), round
1 played interactively through the beat loop, rounds 2–38 via
`train()`+`skip_match()`, then `season_end()` + `start_next_season()`, then a
`save_game()` → `load_game()` roundtrip comparison.

```
$ cd web && node smoke.mjs

PASS  get_nations returns 20 nations
  nation[0]: England (stature 81)
PASS  get_leagues returns 3 leagues
  leagues: England Premier League (tier 0), England Division Two (tier 1), England Division Three (tier 2)
PASS  get_clubs returns 20 clubs
  club[0]: Draymoor Rovers (strength 79)
PASS  new_game starts season 1 round 0
  Smoke Test @ Draymoor Rovers — England Premier League (England)
PASS  train round 1 not already-trained
  beat 1/15 vs Draymoor United: They've won a corner. You must pick up your man and hold position.
PASS  interactive match completed
  FT: Draymoor Rovers 4–1 Draymoor United  (★★★★★ · output 94)
  PC: 2g 1a 0 decisive 0 clutch
PASS  round 1 played
PASS  table has 20 rows
  Table top-3 after round 1:
   1. Draymoor Rovers  Pld 1 W1 D0 L0 GF4 GA1 Pts 3  <- PC
   2. Redcliffe United  Pld 1 W1 D0 L0 GF3 GA0 Pts 3
   3. Wynstead Albion  Pld 1 W1 D0 L0 GF2 GA0 Pts 3
PASS  38 rounds played
PASS  season flagged over
PASS  already-trained guard fired at least once
  Season 1 done: 78g 19a 0 decisive 0 clutch
  season_end:
   Season 1 complete — Draymoor Rovers finished 1 in England Premier League. CHAMPIONS!
   Wage collected: 20 (annual 20). Player of the Year: YOU.
   Promotion/relegation resolves when the next season starts.
  start_next_season:
   Season 2 begins.
   Relegated: Ashford City (England Premier League → England Division Two)
   Relegated: Oakhaven Town (England Premier League → England Division Two)
   Relegated: Draymoor United (England Premier League → England Division Two)
   Promoted: Greymarsh Wanderers (England Division Two → England Premier League)
   Promoted: Solmoor City (England Division Two → England Premier League)
   Promoted: Oakhaven Albion (England Division Two → England Premier League)
   Relegated: Stonebridge Athletic (England Division Two → England Division Three)
   Relegated: Thornbury Wanderers (England Division Two → England Division Three)
   Relegated: Brackwell Town (England Division Two → England Division Three)
   Promoted: Thornbury Town (England Division Three → England Division Two)
   Promoted: Marlow City (England Division Three → England Division Two)
   Promoted: Solmoor Albion (England Division Three → England Division Two)
PASS  season 2 started
PASS  promotion events returned
PASS  save_game returns bytes
  save size: 1436 bytes
PASS  save/load roundtrip preserves player_name, club_name, league_name, nation_name, season_number, season_round, week_label
PASS  table survives roundtrip

SMOKE OK
```

## Static page serve check (added by reviewer, same day)

The browser DOM itself was not opened (no browser on this host), but the page
and all wasm-pack artifacts serve correctly over HTTP:

```
$ cd web && python3 -m http.server 8123 &
$ curl -s -o /dev/null -w "%{http_code}" http://localhost:8123/index.html            → 200
$ curl -s -o /dev/null -w "%{http_code}" http://localhost:8123/main.js               → 200
$ curl -s -o /dev/null -w "%{http_code}" http://localhost:8123/pkg/goat_web.js       → 200
$ curl -s -o /dev/null -w "%{http_code}" http://localhost:8123/pkg/goat_web_bg.wasm  → 200
```

Cross-check worth noting: the node smoke's promotion/relegation events at the
S1→S2 boundary (seed 42, England) are byte-identical to the goat-tui playable
gate run of A3.3 — same seed, same events, two different renderers.
