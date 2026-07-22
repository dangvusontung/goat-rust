# TASK DESIGN ROUND 4, SLICE 1 — `Competition` entity, `FixtureImportance` ladder, wiring real fixtures through `goat-calendar`

**Split-file note (read this first):** this file is 1 of 4 that together replace
`tasks/TASK-DESIGN-round4-competitions.md` (now a short pointer doc). The split happened after
Tùng resolved all 10 "[DECISION NEEDED]" items from that doc's design conversation, 2026-07-22,
specifically so Dev can implement in guarded, independently-committable chunks — an
interruption mid-pass loses at most one slice's progress, not the whole round. Sibling files:
`-slice2-3-club-cups.md`, `-slice4-national-teams.md`, `-slice5-integration.md`. This file is
fully self-contained — implement it without reading the others or the original doc.

Prereq: **none.** This is the foundation slice. Land it, fully tested and committed, before
starting any of the 3 sibling files — every one of them schedules its fixtures through the
`Competition`/`FixtureImportance`/conflict-resolution machinery this slice adds.

Read first: `docs/MAIN.md`'s **Calendar Simulator — Core Spec** section (Data Models ~line
913–996, Core Loop pseudo-code ~line 1000–1150) — the `Competition`/`Fixture`/
`FixtureImportance` shape this slice brings into code; `crates/goat-calendar/src/*.rs` (the
already-built Phase 1 scaffold this slice extends, not replaces).

## Ground rules

- **Reuse `goat-calendar`'s existing Phase 1 scaffold; this is that crate's Phase 2, not a
  parallel system.** `CompetitionId`, `Fixture` (with `competition_id`/`scheduled_day`/
  `original_day`/`is_orbit`), `DayContext`, `DayReport`, `StopClass`, `SubsystemId` (already has
  an `International` variant), and `CalendarEngine` (`tick_one_day`/`advance_until_flashpoint`/
  `advance_bounded`/`sim_season_headless`) already exist. `engine.rs:145` has a literal comment:
  `// Phase 1 stub: SuspensionLedger (per-competition, counts by match not day) added in Phase
  2.` This slice is the design for that comment's Phase 2. Do not propose a new calendar/fixture
  engine from scratch.
- **No change to the existing league round-robin simulation itself.** `fixtures.rs`'s
  `generate_fixtures`/`round_fixtures` (circle-method round-robin, `ROUNDS_PER_SEASON = 38`) and
  `season.rs`'s `Table` keep working exactly as they do today for the league competition kind.
- **No change to promotion/relegation, player generation, attribute storage, or match-engine
  internals.** New competition kinds (siblings' job) are new *fixtures* fed through the existing
  match-resolution path — not a new way of simulating a match. This slice only builds the
  entity/ladder/scheduling plumbing they'll be fed through.
- **"Generated but consistent."** Every new RNG-driven decision this slice's siblings introduce
  must be a pure function of `world_seed` (+ competition/season/round indices), on its own
  forked RNG stream, per bible §9's sacred determinism plumbing — never sharing a stream with
  match/transfer/injury/calendar RNG. (This slice itself introduces no new RNG stream — the
  conflict-resolution sort in 1.4 is deterministic, not randomized.)

## Verified: what already exists (grounding for this slice)

- `crates/goat-calendar/src/clock.rs:7-10,87-98` — `CompetitionId` exists as a bare `pub type
  CompetitionId = u32` (**no `Competition` struct with `kind`/`priority` yet** — this is exactly
  what 1.1 adds). `Fixture` already has `competition_id: CompetitionId`, `scheduled_day`/
  `original_day` (reschedule audit trail), and `is_orbit: bool` — **but no `importance:
  FixtureImportance` field and no `leg_for_id` for two-legged ties** (both added by 1.1).
- `crates/goat-calendar/src/subsystem.rs:17-26` — `SubsystemId` already has an `International`
  variant (doc comment: "Must interrupt immediately: match day, transfer offer, serious injury,
  call-up") — the slot for national-team fixtures already exists in the subsystem registry;
  nothing behind it yet (a sibling file's job, not this slice's).
- `crates/goat-calendar/src/engine.rs:71-116,144-145` — `fixtures_for_day` only filters by
  `is_orbit && scheduled_day == day`; there is **no conflict resolution at all today** — if two
  orbit fixtures land on the same day, nothing picks a winner or reschedules the loser.
  `congestion_score` (`engine.rs:91-96`) already computes a 10-day-window fixture count, and
  `days_until_next_fixture` (`engine.rs:79-86`) already exists — both are inputs 1.4's
  conflict-resolution reschedule logic can reuse directly, no new plumbing needed for them.
- **`goat-calendar` is not yet fully wired to the real league fixtures.**
  `crates/goat-core/src/calendar_loop.rs:86-110`'s `advance_calendar_week` constructs
  `CalendarEngine::new(world_seed, season, Vec::new())` — an **empty fixture list.** The
  `standard_season()` helper in that same file (`calendar_loop.rs:29-53`) hardcodes one
  `Season { id: 1, start_day: 0, end_day: 364 }` with 3 windows (`InternationalBreak` day
  30-44, `TransferWinter` day 160-195, `TransferSummer` day 330-364) — `CalendarEngine` today
  only drives window-detection flavor via `WindowWatch`, it does not carry the real league
  round-robin fixtures from `goat-world::fixtures`. 1.3 closes this gap.
- `crates/goat-world/src/calendar.rs:1-31` — the existing season week-grid
  (`SEASON_CALENDAR_WEEKS = 38`, `BASE_CAREER_YEAR = 2025`, Aug 15-anchored) has 3 break weeks
  per season and only spans roughly mid-August to mid-May — leaving a ~3-month off-season gap
  every year with zero simulated content today. Not this slice's concern (a sibling file's), but
  the day-numbering this slice's `Fixture.scheduled_day` uses must stay consistent with it.

## 1.1 — `Competition` struct + `CompetitionKind`

```rust
// crates/goat-calendar/src/clock.rs (extends the existing module)

pub struct Competition {
    pub id: CompetitionId,
    pub kind: CompetitionKind,
    /// Conflict-resolution priority; higher wins a same-day clash (see 1.2).
    pub priority: i32,
    /// Relevant to the PC's club/nation this season.
    pub is_orbit: bool,
}

pub enum CompetitionKind {
    League,
    DomesticCup,
    ContinentalTier1,   // Champions-League-equivalent
    ContinentalTier2,   // Europa-equivalent
    ContinentalTier3,   // Conference-equivalent
    WorldCup,
    ContinentalChampionship,   // Euro/Copa América/Asian Cup/etc. — one enum value, many nations' instances
}
```

This is **7 kinds**, not the bible's original 4 (`league | domesticCup | continental |
international`) — `continental` splits into the 3 club tiers a sibling slice needs, and
`international` splits into `WorldCup` vs `ContinentalChampionship` since they need different
cadence/offset and must be distinguishable for legacy-evidence purposes ("won a World Cup" and
"won a continental championship" are different trophies).

`Fixture` (`clock.rs:87-98`) gains the two bible fields it's missing:

```rust
pub struct Fixture {
    // ...existing fields unchanged...
    pub importance: FixtureImportance,
    pub leg_for_id: Option<FixtureId>,   // two-legged ties (continental knockout, some cup rounds)
}
```

## 1.2 — `FixtureImportance` ladder — **CONFIRMED 2026-07-22, no change from proposal**

```rust
pub enum FixtureImportance {
    DeadRubber,
    League,
    Derby,                    // a league match, rivalry-flagged
    ContinentalTier3,
    DomesticCupEarly,         // rounds before the tier-1 entry round (sibling slice's 2.1)
    ContinentalTier2,
    DomesticCupLate,          // tier-1-entry round onward, incl. semis
    ContinentalTier1,
    DomesticCupFinal,
    ContinentalTier1Final,
}
```

Tùng confirmed this ladder as proposed, including the one explicitly-flagged judgment call:
**`ContinentalTier1` (group/knockout stage) DOES outrank `DomesticCupLate` (semifinal)** in a
same-day clash — real football gives UCL matches priority for rest-day allocation over domestic
cup replays, and that reasoning holds. No further sign-off needed; implement as written.

**National-team fixtures (`WorldCup`/`ContinentalChampionship`) are deliberately NOT in this
ladder at all.** Per the bible's own pseudocode (`docs/MAIN.md:1106-1107`: "international
windows always win club fixtures (FIFA-style)") — national-team competitions don't win a
same-day *priority* contest against club fixtures, they **exclude club fixtures from being
scheduled in their window at all**, one level higher than the importance ladder. This is a hard
calendar-level rule (an international window makes every date inside it unavailable to club
fixture generation), not a same-day tie-break — a sibling file (national teams) details how this
plays out given the existing international-break window is far too short for a whole World Cup.

## 1.3 — Wire real league fixtures through `CalendarEngine` (closes the `Vec::new()` gap)

`calendar_loop.rs:91`'s `CalendarEngine::new(world_seed, season, Vec::new())` must start passing
the PC's orbit fixtures (this slice: just their league fixtures from `goat_world::fixtures`;
once sibling slices ship: cup/continental/national-team fixtures too) instead of an empty vec.
This is mechanical (`fixtures_for_club`/equivalent already exists in `goat-world`, just needs
threading into the `Fixture` shape `goat-calendar` expects — `competition_id`, `importance`,
`scheduled_day` computed from `goat_world::calendar`'s existing week-grid) but is the concrete
step that makes "multiple competitions" a real, testable thing instead of a type that exists but
carries no data.

## 1.4 — `resolveFixturesForDay`-equivalent conflict resolution

Implement the bible's pseudocode (`docs/MAIN.md:1090-1117`) against `goat-calendar`'s
`CalendarEngine`: when `fixtures_for_day` returns more than one orbit fixture for a day, sort by
`(competition.priority DESC, importance DESC, fixture.id ASC)`, keep the winner, reschedule the
rest to the next legal slot (avoiding other scheduled days, active windows, and a minimum rest
gap — the existing `days_until_next_fixture`/`congestion_score` context already computed by
`build_context` gives the inputs this needs). Two-legged ties (`leg_for_id`) reschedule together,
never independently.

- TDD: `crates/goat-calendar/tests/` new module `conflict_resolution` —
  `higher_priority_competition_wins_same_day_clash`, `bumped_fixture_reschedules_forward_not_
  backward`, `two_legged_tie_reschedules_together` (mirroring the bible's own AC set and
  `promotion.rs`'s existing idempotency-test style).
- Playable gate: `cargo run -p goat-tui` → a season where the PC's club has both a league match
  and a cup match on the same calendar day (synthetic/seeded — a real cup fixture doesn't exist
  until a sibling slice ships, so seed a synthetic second orbit fixture for this test) → the
  resolved fixture list shows exactly one of them on that day, the other moved with a visible
  reschedule note.

**Size: large, risk: medium-high.** Not because any one piece is hard, but because it's the
slice everything else depends on, and it touches the one already-shipped-and-tested crate
(`goat-calendar`) rather than adding a new one — regressions here ripple into every existing
`goat-calendar`/`calendar_loop.rs` test.

## Out of scope (this file)

- Domestic cup bracket, continental club qualification/format, World Cup/continental
  championship, `SuspensionLedger` scoping — all sibling files' work. This slice only builds the
  entity/ladder/scheduling plumbing they schedule fixtures through; it does not need to know
  their shapes to be complete and correctly tested on its own.

## Definition of done (Slice 1)

1. `cargo test --workspace` green, including the new `conflict_resolution` module and any
   updated `goat-calendar`/`calendar_loop.rs` tests this slice's wiring touches.
2. No new failures beyond the **10 pre-existing `goat-tui` `smoke_stdin` failures** (verified
   2026-07-22, out of scope, unrelated to this work): `confirm_screen_blank_enter_reprompts_
   instead_of_discarding_character`, `double_w_in_same_round_shows_message_not_silent_noop`,
   `game_sheet_and_player_sheet_boxes_close_for_short_and_long_club_names`, `key_moments_lines_
   close_with_ellipsis_not_ragged_cutoff`, `legacy_screen_notes_mid_season_batching`,
   `main_loop_unrecognized_command_messages_and_continues`, `player_sheet_explains_ovr_is_
   position_weighted`, `save_overwrite_requires_explicit_confirmation`, `save_to_empty_slot_
   succeeds_without_confirmation`, `status_header_shows_energy_percent_and_labeled_discipline_
   count`.
3. `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` clean.
4. No new dependencies.
5. Playable gate (above) passes via `cargo run -p goat-tui`.
6. No floats in sim state/logic, no unsafe — existing project-wide constraints.
7. **Commit this slice before starting `-slice2-3-club-cups.md`.** This is the entire reason the
   original doc was split into 4 files — an interruption after this commit loses nothing already
   landed.
