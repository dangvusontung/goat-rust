# TASK DESIGN ROUND 4, SLICE 4 — National team competitions: World Cup + continental championships

**Split-file note (read this first):** this file is 1 of 4 that together replace
`tasks/TASK-DESIGN-round4-competitions.md` (now a short pointer doc). The split happened after
Tùng resolved all 10 "[DECISION NEEDED]" items from that doc's design conversation, 2026-07-22,
so Dev can implement in guarded, independently-committable chunks. Sibling files:
`-slice1-foundation.md`, `-slice2-3-club-cups.md`, `-slice5-integration.md`. This file is fully
self-contained — implement it without reading the others or the original doc. (The qualifying
round-robin scheduling this file needs is specified inline below in full; it does not require
reading the domestic-cup file's bracket code.)

Prereq: **`TASK-DESIGN-round4-competitions-slice1-foundation.md` must be landed and committed
first (hard prereq)** — this file needs `Competition`/`CompetitionKind` (incl. `WorldCup`/
`ContinentalChampionship`), `Fixture.leg_for_id`, and `CalendarWindow`/`WindowKind` plumbing.
**`TASK-DESIGN-round4-competitions-slice2-3-club-cups.md` should land first too (soft prereq)**
— not because this file's code depends on it (this file's knockout stage is only 4 teams, always
even, so it never needs the bye-handling that file builds), but because the original design
conversation recommended sequencing national teams after the club-cup slice establishes the
project's first seeded-draw/bracket pattern, so two people aren't inventing overlapping "draw +
bracket" scaffolding at the same time. If interrupted and only slice2-3 isn't done yet, this
file can still be implemented standalone — just note the sequencing was skipped.

Read first: `crates/goat-world/src/world.rs` (`GeneratedNation.stature`/`tactical_identity`),
`tasks/TASK-DESIGN-round2-national-team-tactical-identity.md` (Doc B — call-ups, caps,
`TacticalIdentity`, the "recompute eligibility fresh each window, no persisted roster" pattern
this slice reuses rather than inventing a second national-squad concept).

## Ground rules

- **Reuse `TacticalIdentity` and `stature` — both already exist, nothing new is generated for
  "what a national team is."** `GeneratedNation` already carries `tactical_identity:
  TacticalIdentity` (`world.rs:49-54`) and `stature: u8` (`world.rs:44-49`, range 25–95,
  rolled at `world.rs:213`). This slice only adds *fixtures and a simulated result* for that
  already-existing entity, reusing Doc B's B.2 "recompute eligibility fresh each window, no
  persisted roster" design exactly.
- **"Generated but consistent."** Every draw (qualifying-group assignment, qualifying-group
  pairing, tournament-proper group draw, knockout draw) is a pure function of `world_seed` (+
  cycle/season/round indices), on its own forked RNG stream, independent of `calendar`/`match`/
  `transfer`/`injury`/`domestic_cup`/`continental_tier{1,2,3}`.
- **No protective seeding.** Same rule as the club-competition file — random draws throughout,
  no pot system separating strong and weak nations in either the qualifying groups or the
  tournament-proper groups.
- **No change to player generation, match-engine internals, or Doc B's call-up/caps machinery.**

## Verified: grounding for this file

- `crates/goat-world/src/world.rs:95-102` — `NUM_NATIONS = 20`. All national-team design in this
  file works against exactly 20 generated nations (no confederations/regions exist as a
  `GeneratedNation` field — qualifying "groups" below are a seeded random partition, not a
  geography-based one).
- `crates/goat-calendar/src/clock.rs:28-35` — `WindowKind` already has 4 variants:
  `TransferSummer`, `TransferWinter`, `InternationalBreak`, **and `OffSeason`** — `OffSeason` is
  **already declared and already has TUI rendering support** (`crates/goat-tui/src/main.rs:2039`:
  `WindowKind::OffSeason => ("☼", "The off-season has begun.")`) but **no window of that kind is
  ever constructed anywhere** — `calendar_loop.rs`'s `standard_season()` (`calendar_loop.rs:
  29-53`) only ever builds `InternationalBreak`/`TransferWinter`/`TransferSummer` windows. This is
  a load-bearing find: **the tournament window this slice needs is not a new `WindowKind`
  variant — it's wiring up a variant that already exists but has sat unused.** 4.2 below reuses
  it rather than adding a 5th `WindowKind`.
- `crates/goat-core/src/calendar_loop.rs:29-53` — `standard_season()`'s existing window layout,
  on the fixed 365-day season-relative axis (`SEASON_DAYS = 365`, `calendar_loop.rs:18`):
  `InternationalBreak` day 30-44, `TransferWinter` day 160-195, `TransferSummer` day 330-364.
  Day 0 of this axis aligns with `goat-world::calendar`'s Aug-15 season anchor (confirmed by
  cross-checking: `InternationalBreak` day 30 ≈ mid-September, matching that crate's week-4
  international break; `TransferWinter` day 160 ≈ late January, matching its week-21 winter
  break). 4.2's new window is placed on this same axis.
- `crates/goat-world/src/calendar.rs:14-22` — the 38-week season grid (`SEASON_CALENDAR_WEEKS =
  38`, Aug-15 anchored) spans roughly day 0 (Aug 15) to day ~266 (early May) — leaving days
  ~267-364 (roughly early May to mid-August) with **zero simulated content today** apart from
  the existing `TransferSummer` window (330-364). This slice's tournament window (4.2) is the
  first thing proposed to actually live in the unused portion of that gap (days ~267-329).
- `crates/goat-meta/src/legacy.rs:10-41` — `LegacyEvidence` has `career_caps: u32`
  (`legacy.rs:38`) and `career_international_goals: u32` (`legacy.rs:40`), added by Doc B, with
  no school-weighting logic reading them yet ("raw evidence for a future axis pass"). **No
  `career_world_cups_won`/`career_continental_championships_won` field exists.** 4.5 adds them,
  mirroring `career_caps` exactly.
- `crates/goat-core/src/state.rs:98-99,228,591` — `pc_career_caps: u32` and the season→career
  folding pattern: `Intent::ApplySeasonEndLegacy` carries a `season_caps` field, folded via
  `state.pc_career_caps += season_caps` at the same pipeline position as every other counter
  (`state.rs:591`). 4.5's two new counters follow this exact shape.

## 4.1 — Cadence: World Cup seasons 1, 5, 9, 13…; continental championship seasons 3, 7, 11, 15…

`season_number % 4 == 1` → World Cup year, `season_number % 4 == 3` → continental-championship
year. Matches the real 4-year cycle with a 2-year stagger. **No change to this formula** — see
4.1a below for what Tùng actually decided about season-1 alignment.

### 4.1a — Season-1 alignment — **CONFIRMED 2026-07-22: no special-casing, formula stays as-is**

The original doc flagged "Design picked season-1-is-a-World-Cup-year somewhat arbitrarily" as an
open question. Tùng's answer: **the PC's career start should NOT be artificially aligned to a
World-Cup year (or any tournament year) at all.** The international tournament calendar runs on
its own fixed, independent cycle from world genesis — wherever season 1 happens to land in that
cycle (currently: a World Cup year, by coincidence of the `% 4 == 1` formula, not by design
intent) is fine, no special-casing needed. **Do not change the cadence formula to shift season 1
away from a World Cup year, and do not add logic to force it to land on one either** — the
formula above is left exactly as originally proposed; only the framing changes (it's coincidence,
not a deliberate anchor, and that's an acceptable, confirmed state).

## 4.2 — Calendar window: reuse the already-declared, currently-unused `WindowKind::OffSeason` — **CONFIRMED 2026-07-22**

Tùng confirmed a dedicated calendar window (not a stretch of the existing `InternationalBreak`),
literally: "cứ tháng 1x/6 đến 1x/7 mà quất" (roughly mid-June through mid-July). Concrete
day-of-year boundaries, chosen to (a) land in that literal window and (b) not collide with the
existing `TransferSummer` window (day 330-364):

```rust
CalendarWindow {
    kind: WindowKind::OffSeason,   // already exists, clock.rs:35 — not a new variant
    start_day: 300,   // ≈ June 11 (Aug-15-anchored axis: day 304 ≈ Jun 15)
    end_day: 329,     // ≈ July 10 (day 334 ≈ Jul 15)
}
```

Day 300-329 sits entirely inside the existing, currently-empty portion of the off-season gap
(days ~267-364) and ends exactly where `TransferSummer` (330-364) begins — no gap, no overlap.
This is also a pleasant side effect worth noting: newly-crowned World Cup/continental-champion-
ship players become transfer-market subjects immediately after the tournament resolves, matching
how real football's biggest post-tournament transfer activity happens right when the summer
window opens.

Only add this window to a season's window list **in a tournament season** (`season_number % 4
== 1` or `== 3`, per 4.1) — in a non-tournament season, no `OffSeason` window fires and no
special off-season content plays out (matching how `TransferSummer` etc. already only matter
when populated with something to do).

Only the tournament *finals* (the month-long event itself) live in this window — qualifiers
(4.3) do not, since a full qualifying campaign is much longer than 30 days and instead uses the
existing in-season `InternationalBreak` window across multiple seasons.

## 4.3 — Qualifying: 4 groups of 5 nations, single round-robin, spread across 3 non-tournament seasons — **CONFIRMED 2026-07-22 as proposed (2 qualifiers/window, 6 total/cycle), qualifying-group shape newly designed here**

Tùng confirmed the original doc's placeholder number: **2 qualifying fixtures per
international-break window, across the 3 seasons preceding a tournament season (6 total
qualifying matches per cycle).** The original doc did not specify how nations are grouped for
qualifying or how a fixture list of exactly 6 matches produces "a small number of finalist
nations" — that's new design work this file does, consistent with 4.3's confirmed 6-match budget
and with 4.4's tournament-proper shape below (both numbers were designed together, not
independently).

**Qualifying-group formation:** the 20 nations are split into **4 groups of 5**, by a seeded
random partition (own forked RNG stream, `national_qualifying_draw`, redrawn fresh at the start
of each qualifying cycle — i.e., the groups for the World-Cup cycle starting season 2 need not
match the groups for the continental-championship cycle starting season 4). No confederations/
regions exist in this codebase (`GeneratedNation` has no such field, verified above) so this is
a "generated but consistent" random grouping, not a geography-flavor one — consistent with how
the rest of this doc series treats every other draw.

**Qualifying schedule — single round-robin within each 5-nation group, using 4 of the 6 budgeted
matches:**

A 5-team single round-robin is 10 total games (`C(5,2)`), scheduled via the standard
odd-team-count circle method: at any one round, 4 of the 5 nations play (2 simultaneous games)
and 1 sits out — 5 rounds complete the full round-robin (each nation plays 4 games, sitting out
exactly once). This slice maps that 5-round schedule onto the 3 available international-break
windows (2 match-slots each, 6 total) as: window 1 → rounds 1-2, window 2 → rounds 3-4, window 3
→ round 5 **only** (1 of its 2 slots used). **This leaves 1 of the 6 budgeted qualifying slots
per cycle genuinely unused** — flagged explicitly rather than silently invented into a mechanic
Tùng didn't ask for (a tie-break playoff, extra friendly, etc. were all considered and rejected
here as unnecessary scope for a first pass; leaving one slot idle is simpler and still uses 5 of
6 budgeted matches, close to the full budget).

**Advancement:** top 2 nations by qualifying-group points (standard 3/1/0) advance per group —
4 groups × 2 = **8 finalist nations** out of 20 (40%) reach the tournament proper. Tie-break on
points: goal difference, then goals scored, then a seeded coin-flip (own RNG substream) — same
tie-break shape as the domestic-cup/continental-club file's structural invariant tests expect
elsewhere in this doc series, kept consistent here rather than inventing a different rule.

Qualifying fixtures use the existing in-season `InternationalBreak` window (day 30-44,
`calendar_loop.rs:31-35`), which — per the bible's own pseudocode and the foundation slice's 1.2
— already always wins against any club fixture that would otherwise land there. No new
conflict-resolution mechanism is needed here; this is the one place this slice touches the
priority ladder, and it's already handled by the foundation slice's window-exclusivity rule.

## 4.4 — Tournament proper: 8 finalists, 2 groups of 4, round-robin then knockout — **NEW DESIGN, same shape for both World Cup and continental championship**

Same format for both tournament kinds (Tùng asked for consistency between them, and there's no
stated reason for World Cup and continental championship to differ in shape at this scale):

- **8 finalist nations** (from 4.3) split into **2 groups of 4** by a seeded random draw (own
  forked stream, `national_tournament_draw`, no protective seeding).
- **Group stage:** single round-robin within each group of 4 — 3 matches/nation, 6
  matches/group, 12 total. Scheduled over 3 match-days within the `OffSeason` window (4.2): day
  302, day 308, day 314 (6-day gaps — realistic rest for a life-sim, not simulated
  minute-by-minute fatigue).
- **Advancement:** top 2 per group (goal-difference/goals-scored/seeded-coinflip tie-break, same
  rule as 4.3) → 4 nations.
- **Knockout:** semifinal (day 320) then final (day 327) — **single-leg matches, not two-legged**
  (unlike the continental *club* tiers' two-legged knockout rounds in the sibling club-cups
  file) — this deliberately follows real international-tournament convention (World Cup/Euro
  knockout rounds are one-off matches, only club continental competitions are two-legged), and
  keeps this the smallest, simplest bracket in the whole round-4 design (only 2 knockout rounds,
  always exactly 4 → 2 → 1, never an odd count, so no bye-handling is ever needed here).
- All tournament-proper days (302-327) sit inside the 300-329 `OffSeason` window with buffer on
  both ends.

**Why 8 finalists / 2 groups of 4, not some other split:** this doc series' club-tier design
(sibling `-slice2-3-club-cups.md`) uses 4-team groups throughout (32/48/64 slots ÷ 4), and this
slice extends the same atomic group-size choice to the national-team tournament for consistency,
even though the national total (8, not a multiple tied to those slot counts) is independently
derived from the 20-nation qualifying pool. It also happens to match the tournament's own real-
world shape reasonably well at this world's much smaller scale (20 nations total, vs. ~200 in
reality) — 40% of all nations reaching the finals reads as genuine qualification, not a token
gate.

**Simulated result, not just call-ups — this is genuinely new territory.** Doc B explicitly
scoped "simulating actual international tournament outcomes" as out of its round. This slice is
that follow-up: qualifying and tournament-proper fixtures are resolved through the existing
match-simulation path using national-team strength derived from `TacticalIdentity`/
`stature`-weighted eligible-population quality — recomputed fresh each fixture per Doc B's B.2
pattern (no persisted national-squad roster), the same as every call-up already works today.

- TDD: `crates/goat-world/src/` new module (`national_tournament.rs` or similar) —
  `qualifying_group_partition_is_deterministic_per_seed`, `qualifying_round_robin_completes_in_
  five_rounds_with_one_bye_per_round` (structural, the 5-team circle-method schedule),
  `top_two_per_qualifying_group_advance` (structural, 8 finalists total), `tournament_group_
  stage_produces_correct_advancers`, `tournament_knockout_is_single_leg_not_two_legged` (the one
  deliberate divergence from the club-tier file — worth its own explicit regression test so it
  doesn't accidentally get "fixed" to match the club pattern later), `tournament_produces_
  exactly_one_champion`, `world_cup_and_continental_championship_never_land_in_the_same_season`
  (structural invariant on the 4-year/2-year-offset cadence from 4.1).
- Playable gate: `cargo run -p goat-tui` → advance a career through a World-Cup-cycle's 3
  qualifying seasons → a qualifying-window flashpoint appears each international break (reusing
  Doc B's shipped call-up UI, not a new screen) → advancing into the tournament season shows the
  `OffSeason` window's flashpoint (`crates/goat-tui/src/main.rs:2039`'s existing "☼ The
  off-season has begun" line, now finally backed by real content) → the tournament resolves → if
  the PC's nation wins, a legacy counter increments (4.5), visible from the existing [G] Legacy
  screen.

## 4.5 — Legacy consequence: new `LegacyEvidence` counters, mirroring `career_caps` exactly — **CONFIRMED 2026-07-22**

Add `career_world_cups_won: u32` and `career_continental_championships_won: u32` to
`LegacyEvidence` (`legacy.rs:10-41`), following the exact `career_caps` precedent
(`legacy.rs:38`): no new school-weighting logic this round (raw evidence for a future axis pass,
same framing Doc B used), folded at the same `Intent::ApplySeasonEndLegacy` pipeline position
(`state.rs:591`) via new `season_world_cups_won`/`season_continental_championships_won` fields on
that intent, mirroring `season_caps` → `pc_career_caps` exactly (`state.rs:98-99,228,591`). Add
matching `pc_career_world_cups_won`/`pc_career_continental_championships_won` counters to
`WorldState` (`state.rs`), same shape as `pc_career_caps`.

- TDD: `crates/goat-core/tests/` new test mirroring the existing `pc_career_caps` folding test
  exactly, for these two new counters.

**Size: extra-large, risk: high.** This is arguably the single riskiest slice in the whole
round-4 design: it needs a genuinely new tournament-bracket engine (group stage + knockout — a
shape the sibling club-cups file also builds, but this one differs in leg count), the first
actual use of the long-dormant `WindowKind::OffSeason` variant, and a cadence-correctness
invariant that must hold for the entire length of a career (20-40+ seasons). Strongly recommend
building this after `-slice2-3-club-cups.md` is solid, per the soft prereq above.

## Out of scope (this file)

- **Youth/age-group national tournaments, women's competitions, or any second parallel
  national-team structure** — matches Doc B's own "singular senior team per nation" scoping.
- **Variable qualifying-campaign size / extra in-season international-break windows beyond the
  confirmed 6-per-cycle budget** — a larger, more realistic qualifying campaign would require
  changing `goat-world::calendar`'s `WEEK_MATCH_COUNTS` layout itself, out of scope here.
- **Club finances/prize money, media/pundit coverage of tournament outcomes** — separately
  parked bible items, unaffected by this file.
- `SuspensionLedger` scoping across all 7 competition kinds — the integration sibling file's job.

## Definition of done (this file)

1. `cargo test --workspace` green, including every TDD anchor above.
2. No new failures beyond the **10 pre-existing `goat-tui` `smoke_stdin` failures** (same list as
   the foundation file's Definition of Done — verified 2026-07-22, out of scope, pre-existing).
3. `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` clean.
4. No new dependencies.
5. Playable gate (above) passes via `cargo run -p goat-tui`.
6. A real perf check on the tournament-cycle simulation (qualifying + finals), same discipline as
   the round-2 world-genesis doc established for genesis/replay cost — not estimated blind.
7. No floats in sim state/logic, no unsafe.
8. **Commit after this file lands, before starting `-slice5-integration.md`.**
