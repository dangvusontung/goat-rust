# TASK DESIGN ROUND 4, SLICE 5 — `SuspensionLedger` scoping + final conflict-resolution wiring across all 7 competition kinds

**Split-file note (read this first):** this file is 1 of 4 that together replace
`tasks/TASK-DESIGN-round4-competitions.md` (now a short pointer doc). The split happened after
Tùng resolved all 10 "[DECISION NEEDED]" items from that doc's design conversation, 2026-07-22,
so Dev can implement in guarded, independently-committable chunks. Sibling files:
`-slice1-foundation.md`, `-slice2-3-club-cups.md`, `-slice4-national-teams.md`. This file is
fully self-contained — implement it without reading the others or the original doc, though it is
the one file in this batch that genuinely needs all 3 siblings' code present to test real
composition (see Prereq).

Prereq: **all 3 sibling files must be landed and committed first** —
`-slice1-foundation.md` (Competition/FixtureImportance/conflict resolution),
`-slice2-3-club-cups.md` (domestic cup + continental club tiers), and
`-slice4-national-teams.md` (World Cup + continental championships). This is deliberate, not
laziness: this file is the integration pass that proves the other four competition-kind designs
actually compose — it needs a PC who can genuinely be suspended in a cup while playing a league
match, or genuinely in League + Domestic Cup + a continental tier simultaneously, to be
meaningfully tested at all. If any sibling isn't present, stop and land it first.

Read first: `crates/goat-core/src/state.rs` (the current single-scalar suspension state this
file replaces); `crates/goat-save/src/save.rs` (the versioned save format and its existing
v9→v10 migration precedent, the pattern this file's v10→v11 bump follows).

## Ground rules

- **No new mechanics of its own.** This slice finalizes wiring across designs the other 3 files
  already fully specify — it does not invent a new competition kind or new gameplay rule.
- **No floats in sim state/logic, no unsafe** — same project-wide constraint as every other
  file in this doc series.

## Verified: grounding for this file

- `crates/goat-core/src/state.rs:903-905` — `pc_suspension_weeks: u32` is **one single global
  scalar**, decremented once per resolved round with no competition scoping at all
  (`state.rs:901-905`: `if state.pc_suspension_weeks > 0 { state.pc_suspension_weeks -= 1; }`,
  gated on "bible AC-06: a ban counts down by matches actually played, not by elapsed days" — the
  *match-counted-not-day-counted* part is already correct today; this slice generalizes it
  across competitions, it does not fix a bug in the single-competition case).
- `crates/goat-save/src/save.rs:30` — `pub const VERSION: u32 = 10` today. The file already has
  an established v9→v10 migration precedent to follow for the v10→v11 bump this slice needs.
- `crates/goat-calendar/src/engine.rs:91-96,266-268` — `congestion_score` (a 10-day-window
  fixture count) and `SOFT_FLUSH_THRESHOLD = 3` (soft-flashpoint-batching threshold) are the
  existing tuning constants this slice re-checks now that a PC's fixture load can genuinely
  include League + Domestic Cup + a continental tier + a national-team call-up simultaneously —
  the highest fixture density anywhere in the game.

## 5.1 — `SuspensionLedger`, replacing the single global `pc_suspension_weeks` scalar

```rust
// crates/goat-calendar/src/clock.rs or a new suspension.rs
pub struct SuspensionLedger {
    pub player_id: PlayerId,   // or a bare index, matching however goat-core identifies players
    pub competition_id: CompetitionId,   // scoped per competition — a cup ban doesn't bleed into league
    pub matches_remaining: u32,          // decrements only when a match of THIS competition is played
}
```

`crates/goat-core/src/state.rs:903-905`'s `pc_suspension_weeks: u32` is replaced by a small
`Vec<SuspensionLedger>` (in practice tiny — the PC is rarely suspended in more than one
competition simultaneously, but the type must support it correctly). **Suspension counts by
match actually played in that exact competition, not by elapsed calendar days or rounds of any
other competition** — this slice generalizes the existing correct single-competition logic
across all 7 `CompetitionKind` variants (League, DomesticCup, ContinentalTier1-3, WorldCup,
ContinentalChampionship — all defined in `-slice1-foundation.md`'s 1.1), it doesn't change the
underlying rule.

## 5.2 — `goat-save::VERSION` bump

Adding `Vec<SuspensionLedger>` (replacing a bare `u32`) is a save-format change —
`goat-save::save::VERSION` (currently 10, `save.rs:30`) bumps to **11**, with a backward-compat
test reading an old single-scalar `pc_suspension_weeks` save and converting it into a single
league-scoped `SuspensionLedger` entry (`CompetitionKind::League`), following the exact v9→v10
precedent already in `save.rs`.

## 5.3 — Congestion-score sanity check across the full competition set

`goat-calendar::engine::congestion_score` (`engine.rs:91-96`, a simple 10-day-window fixture
count) needs re-checking once a PC's club can simultaneously be in League + Domestic Cup + up to
3 rounds deep in a continental tier, while the PC personally might also have a national-team
call-up — this is the point where fixture density is genuinely at its highest in the whole game
and `shouldFlushSoft`'s soft-flashpoint-batching feel (`engine.rs:266-268`, currently a bare
count threshold, `SOFT_FLUSH_THRESHOLD = 3`) most needs real playtesting, not just unit tests.
**Not a new number to invent here** — flagging that this is where the existing `TASK-TUNE`
convention (first-pass placeholder constants, tune later against a prototype) most needs to be
applied, now that all of the sibling files are actually playable together.

- TDD: `crates/goat-core/tests/` — `suspension_in_one_competition_does_not_block_availability_
  in_another` (the concrete regression this slice exists to prevent — suspend the PC in the
  domestic cup, confirm their next league fixture still selects them normally), a save-roundtrip
  test in `crates/goat-save/tests/save_roundtrip.rs` for the v10→v11 migration.
- Playable gate: `cargo run -p goat-tui` → PC picks up a domestic-cup suspension → next league
  fixture still selects the PC normally → next domestic-cup fixture correctly benches them.

**Size: medium, risk: medium.** Smaller than the other 3 files individually, but it's the file
that proves the other three actually compose correctly, which is exactly the kind of integration
risk that doesn't show up until it's attempted — do not skip real playtesting here in favor of
unit tests alone.

## Out of scope (this file)

- Any new competition kind, bracket format, or qualification rule — all fully specified in the
  3 sibling files. This is wiring/scoping only.

## Definition of done (this file, and the whole round-4 design)

1. `cargo test --workspace` green, including every TDD anchor above and every TDD anchor from
   all 3 sibling files (this is the point where the full round-4 test suite runs together for
   the first time).
2. No new failures beyond the **10 pre-existing `goat-tui` `smoke_stdin` failures** (same list as
   every sibling file's Definition of Done — verified 2026-07-22, out of scope, pre-existing).
3. `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` clean.
4. No new dependencies without an explicit, separately-confirmed exception.
5. `goat-save::save::VERSION` bumps to 11, with a backward-compat test per the existing v9→v10
   precedent.
6. All playable gates from all 4 files in this batch pass via `cargo run -p goat-tui`.
7. No floats in sim state/logic, no unsafe.
8. **Commit after this file lands.** This is the last slice — once it's in, the round-4 design
   (all 4 split files) is fully implemented.
