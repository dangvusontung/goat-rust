# TASK DESIGN ROUND 4, SLICES 2+3 — Domestic cup + continental club competitions

**Split-file note (read this first):** this file is 1 of 4 that together replace
`tasks/TASK-DESIGN-round4-competitions.md` (now a short pointer doc). The split happened after
Tùng resolved all 10 "[DECISION NEEDED]" items from that doc's design conversation, 2026-07-22,
so Dev can implement in guarded, independently-committable chunks. Sibling files:
`-slice1-foundation.md`, `-slice4-national-teams.md`, `-slice5-integration.md`. This file is
fully self-contained — implement it without reading the others or the original doc.

This file combines the original doc's Slice 2 (domestic cup) and Slice 3 (continental club
tiers) into one because they share the same draw/bracket machinery (round-by-round random
redraw, bye handling) and the original design doc explicitly said they can run somewhat in
parallel once the foundation slice lands. Each keeps its own TDD anchors and playable gate below
— they are two competition kinds, not one merged mechanic.

Prereq: **`TASK-DESIGN-round4-competitions-slice1-foundation.md` must be landed and committed
first.** This file needs, from that slice: `Competition`/`CompetitionKind` (incl.
`DomesticCup`/`ContinentalTier1`/`ContinentalTier2`/`ContinentalTier3`), `FixtureImportance`
(incl. `DomesticCupEarly`/`DomesticCupLate`/`DomesticCupFinal`/`ContinentalTier1`/`Tier2`/`Tier3`/
`ContinentalTier1Final`), `Fixture.leg_for_id` (two-legged ties), and the conflict-resolution
pass that schedules multiple competitions' fixtures without collision. If that slice isn't
present in the working tree, stop and implement it first — do not re-derive it here.

Read first: `crates/goat-world/src/{world,fixtures,season,promotion}.rs` (the league/promotion
machinery this sits alongside, not replaces); `crates/goat-rng` (the forked-stream pattern).

## Ground rules

- **No change to league round-robin, promotion/relegation, player generation, or match-engine
  internals.** New fixtures feed through the existing match-resolution path
  (`sim_team_match` for AI-vs-AI, `goat-match` for PC-orbit fixtures).
- **"Generated but consistent."** Every draw (cup pairing, bye assignment, continental
  qualification cutoff, group draw, knockout draw) is a pure function of `world_seed` (+
  competition/season/round/nation indices), on its own forked RNG stream, per bible §9 — never
  sharing a stream with match/transfer/injury/calendar RNG.
- **No protective seeding, anywhere in this file.** Confirmed by Tùng ("cứ real mà quất"): draws
  are genuinely random each round, byes go to a uniformly random club/team, and two top-tier
  entrants (two Tier-1 domestic clubs, or two continental clubs from the same powerhouse nation)
  can meet in any round including early ones. This is a deliberate, confirmed design choice, not
  an oversight.

## Verified: grounding for this file

- `crates/goat-world/src/world.rs:95-102` — `NUM_NATIONS = 20`, `TIERS_PER_NATION = 3`,
  `CLUBS_PER_DIV = 20` — i.e. 20 nations, each with 20 Tier-1 + 20 Tier-2 + 20 Tier-3 clubs (60
  clubs/nation). `GeneratedNation.stature: u8` (`world.rs:44-49`, rolled `25..95` at
  `world.rs:213`) already exists and is genesis-generated — no new per-nation quality metric is
  needed for continental slot allocation (3.2 below).
- `crates/goat-world/src/promotion.rs` — `ReplayCache`/`apply_season_end` (replay-from-seed,
  `PROMO_RELEGATION_N = 3` top/bottom cut) is untouched. Continental qualification (3.1) *reads*
  final Tier-1 league position; it does not feed back into promotion/relegation.
- **No tournament-bracket code exists anywhere today** (verified 2026-07-22) — this file is the
  first thing to add single-elimination and group-stage bracket machinery to the codebase.

---

## Slice 2 — Domestic cup: single-elimination, tier-staggered entry, round-by-round random redraw

**Decision (verbatim, confirmed by Tùng, no change):** every club in a nation is eligible,
FA-Cup style — lower-tier clubs enter earliest, top-tier clubs enter later via byes.

### 2.1 — Entry-by-tier scheme — **CONFIRMED 2026-07-22 as proposed**

At the confirmed 60-clubs/nation scale (20 Tier-1 + 20 Tier-2 + 20 Tier-3), a staggered-entry
bracket that avoids awkward byes as long as possible:

| Round | Entrants this round | Total in the round | Byes | Winners advance |
|---|---|---|---|---|
| R1 | Tier 3 (20 clubs) | 20 | 0 | 10 |
| R2 | Tier 2 (20 clubs) + R1's 10 winners | 30 | 0 | 15 |
| R3 | Tier 1 (20 clubs) + R2's 15 winners | 35 | 1 | 17 + 1 bye = 18 |
| R4 | — | 18 | 0 | 9 |
| R5 | — | 9 | 1 | 4 + 1 bye = 5 |
| R6 | — | 5 | 1 | 2 + 1 bye = 3 |
| R7 (semifinal-equivalent) | — | 3 | 1 | 1 + 1 bye = 2 |
| R8 (Final) | — | 2 | 0 | 1 (champion) |

**8 rounds total.** Tier 3 enters R1, Tier 2 enters R2, Tier 1 enters R3 — mirrors the real FA
Cup's staggered entry and gives lower-tier clubs a distinct multi-round cup-run arc before
meeting the top flight. Confirmed as proposed; implement exactly this table.

**Bye assignment rule (confirmed):** when a round has an odd number of clubs, one club is drawn
at random (same draw RNG as the round's pairing draw, 2.2) to receive the bye — **not**
preferentially given to a higher-tier or higher-strength club.

### 2.2 — Random draw, redrawn every round, seed-deterministic

Each round's pairings are drawn fresh — not a fixed bracket set once at the top. Two Tier-1
clubs entering the same round (R3 onward) **can** draw each other — no protective separation.

```rust
// crates/goat-world/src/domestic_cup.rs (new)
fn draw_round(world_seed: u64, season: u32, nation: NationId, round: usize, clubs: &[ClubId]) -> Vec<(ClubId, ClubId)> {
    let mut rng = GoatRng::new(world_seed ^ domestic_cup_salt(nation) ^ (season as u64) ^ (round as u64));
    // Fisher-Yates shuffle of `clubs` (bye-holder removed first if odd), then pair sequentially.
    // Home/away assignment: a second independent roll per pair (pairing order deciding
    // home/away is the default — no strong preference either way was ever raised).
}
```

New forked RNG stream, `domestic_cup`, independent of `calendar`/`match`/`transfer`/`injury`.

- TDD: `domestic_cup_draw_is_deterministic_per_seed` (same seed+season+round → identical
  pairings), `two_tier1_clubs_can_meet_in_their_entry_round` (statistical: over many seeds, at
  least one instance of two Tier-1 clubs drawn together in R3), `bracket_converges_to_one_
  champion_per_nation_per_season` (structural, mirrors `promotion.rs`'s invariant-style tests),
  `bye_recipient_is_uniformly_random_not_tier_biased` (statistical, over many seeds no
  systematic tier favored for byes).
- Playable gate: `cargo run -p goat-tui` → advance through a season with the PC's club in the
  domestic cup → cup fixtures appear interleaved with league fixtures on the calendar,
  elimination is visible after a loss, a champion is crowned by season's end.

**Size: large, risk: medium.** The draw/bracket logic itself is a well-understood algorithm; the
risk is entirely in the foundation slice's conflict-resolution/calendar-congestion interaction
(an extra ~8-round competition materially increases fixture density for clubs that go deep).

---

## Slice 3 — Continental club competitions: 3 tiers, stature-ranked qualification

**Decision (verbatim, confirmed by Tùng):** 3 tiers (Champions-League/Europa/Conference shape),
slot count varies by federation ranking.

### 3.1 — Qualification pool: each nation's Tier-1 domestic league final position only — **CONFIRMED 2026-07-22**

Continental qualification is read off final position in each nation's **Tier-1 domestic league
only** (the 20-club top flight), not "top N clubs across all 3 domestic tiers of a nation" —
matches real UEFA qualification exactly (Europa/Conference slots go to top-flight clubs
finishing outside the Champions League places, never to 2nd-tier clubs). A
domestic-cup-winner-grants-continental-berth mechanic is **not** included — see "Out of scope."

### 3.2 — Slot-count formula, ranked by `GeneratedNation.stature` — **REPLACED, new table (2026-07-22)**

**This replaces the original doc's band table wholesale — that table is superseded, not
adjusted.** The original 4-band table summed to 67 Tier-1-continental slots across 20 nations
(vs. real UCL's 32-36) and was rejected; a follow-up strict-linear-to-zero-by-rank-6 proposal
(15 slots total, 15/20 nations getting zero) was also rejected as too exclusionary. Tùng's final
instruction: a **per-nation-rank taper for EACH of the 3 continental tiers independently**
(ranked by `stature` descending, `nation_rank` 1 = strongest .. 20 = weakest), front-loaded
toward strong nations and tapering to 0 for weak nations, engineered so each tier's **total
slot count summed across all 20 nations** comes out to approximately Tier1=32, Tier2=48,
Tier3=64 — the exact per-rank shape delegated to Design, not requiring further sign-off.

Reuses `stature: u8` directly (`world.rs:44-49`) — rank position, not a raw threshold, so the
allocation stays proportional regardless of how a given `world_seed` distributes the 20 stature
rolls. Same 5 rank-bands used for all 3 tiers (clean, one table, easy to read/verify):

| `nation_rank` band | # nations | Tier-1 slots/nation | Tier-2 slots/nation | Tier-3 slots/nation |
|---|---|---|---|---|
| 1–2 | 2 | 4 | 6 | 8 |
| 3–6 | 4 | 3 | 5 | 6 |
| 7–10 | 4 | 2 | 3 | 4 |
| 11–14 | 4 | 1 | 1 | 2 |
| 15–20 | 6 | 0 | 0 | 0 |

Totals (all exact, not just "approximately"): Tier1 = 2×4+4×3+4×2+4×1+6×0 = **32**. Tier2 =
2×6+4×5+4×3+4×1+6×0 = **48**. Tier3 = 2×8+4×6+4×4+4×2+6×0 = **64**. Each column is
monotonically non-increasing by rank, tapering to exactly 0 for the bottom 6 nations (the
weakest 30%) in **every** tier — a deliberate consequence of Tùng's "taper to 0" instruction,
flagged explicitly here: **the weakest 6 of 20 nations get zero continental football at all,
in any of the 3 tiers, every season.** This is what "taper to 0 for weak nations" literally
means at this scale; it was not separately re-confirmed beyond the instruction itself, so if
that reads as too harsh once playtested, it's `TASK-TUNE` territory (adjust the bottom band from
0 to 1, re-summing the totals), not a re-design.

32/48/64 are all divisible by 4, composing directly with the group-stage format below (3.3):
8 / 12 / 16 groups of 4, respectively.

### 3.3 — Fixture format: group stage then knockout — **NEW DESIGN, replaces the single-elimination recommendation (2026-07-22)**

Tùng picked group-stage-then-knockout (real UCL/Europa/Conference shape) over Design's original
single-elimination recommendation. Concrete shape, designed against the 32/48/64 totals above
and checked against the foundation slice's congestion scoring before locking in (see "Composability
check" below):

- **Group stage:** 4-team groups, drawn from all qualified clubs across all 20 nations for that
  tier (no protective seeding — a group can contain 2+ clubs from the same nation; this matches
  modern UEFA rules, which dropped the same-country restriction, and is consistent with this
  file's "no protective seeding" ground rule). **Single round-robin** (each club plays the other
  3 group members once — 3 matches/club, 6 matches/group), not double round-robin —
  a deliberate scope-control choice (see composability check) trading some authenticity (real
  UCL's group phase is home-and-away) for materially lower fixture density, the same kind of
  trade-off the original doc's single-elimination recommendation made, just one level lower in
  the format instead of skipping groups entirely.
  - Tier 1: 32 clubs → **8 groups.**
  - Tier 2: 48 clubs → **12 groups.**
  - Tier 3: 64 clubs → **16 groups.**
- **Advancement:** top 2 of each group advance to a single-elimination knockout bracket.
  - Tier 1: 8 groups × 2 = **16** — a clean power of 2. Knockout: R16 → QF → SF → F, 4 rounds,
    no byes needed.
  - Tier 3: 16 groups × 2 = **32** — a clean power of 2. Knockout: R32 → R16 → QF → SF → F,
    5 rounds, no byes needed.
  - Tier 2: 12 groups × 2 = **24** — **not** a power of 2. Reuse Slice 2's exact odd-count bye
    machinery (uniformly random bye recipient, redrawn each round, no tier bias) at the one
    round where the bracket goes odd: R24 (12 matches, no bye) → R12 (6 matches, no bye) → R6
    (3 matches, no bye) → **3 clubs remain, odd — 1 bye drawn per Slice 2's rule** → 2 clubs →
    Final. 5 knockout rounds total, one bye round. This is a direct, deliberate reuse of Slice
    2's bracket-oddness handling rather than a second bye mechanism invented for this tier.
- **Legs:** every knockout round is **two-legged** (`Fixture.leg_for_id`, added by the foundation
  slice) **except the Final, which is single-leg** — mirrors real UCL (a one-off showpiece final
  at a neutral venue, unlike the two-legged earlier rounds). `FixtureImportance::
  ContinentalTier1Final` (etc., foundation slice's ladder) is exactly this one match.
- **Draw RNG:** own forked stream per tier, `continental_tier{1,2,3}`, salted additionally by
  phase (group draw vs. a given knockout round) and round index — same pattern as `domestic_cup_
  salt`, independent of `calendar`/`match`/`transfer`/`injury`/`domestic_cup`.

**Composability check (why this shape, not another):** a club that goes all the way in one
continental tier plays 10 (Tier 1: 3 group + 3×2 legs for R16/QF/SF + 1 final) to 12 (Tier 2/3)
matches in that competition alone. Stacked on top of a 38-round league season and up to 8
domestic-cup rounds, a club deep in all three in the same season plays roughly 56-60 matches —
in line with a real elite club's season length, not an outlier the foundation slice's congestion
scoring (10-day-window fixture count, `engine.rs:91-96`) or `SOFT_FLUSH_THRESHOLD = 3`
(`engine.rs:266-268`, per the foundation doc) should choke on structurally, though real
playtesting of the tuning constants themselves is still `TASK-TUNE` territory, not decided here.
A single round-robin (not double) at the group stage is what keeps this in range — flagging
explicitly that double round-robin was considered and rejected for density reasons, not
overlooked.

- TDD: `continental_slots_match_taper_table` (structural, asserts the table above per nation
  rank), `continental_group_draw_deterministic_per_seed`, `continental_group_stage_top_two_
  advance` (structural: exactly 2 clubs per group progress, by group standings), `continental_
  tier2_knockout_uses_a_bye_round_at_24_to_3` (structural, confirms the specific odd-bracket
  point and that the bye reuses Slice 2's uniform-random rule), `continental_champion_per_tier_
  per_season` (one champion crowned per tier, mirrors `promotion.rs`'s invariant style).
- Playable gate: `cargo run -p goat-tui` → PC's club finishes in a continental qualification
  position → next season's fixture list includes group-stage fixtures against clubs from other
  nations, then (if it advances) knockout fixtures with visible two-legged aggregate scoring,
  single-leg for the final.

**Size: extra-large, risk: high.** Three parallel competitions (not one), a genuinely new
cross-nation qualification computation (needs full previous-season table data for all 20
nations' Tier-1 leagues — today only ever computed on-demand for flavor screens, this needs to
become a real per-season-boundary computation, similar in shape to how `promotion.rs`'s
`ReplayCache` already extends incrementally), plus a brand-new group-stage table type (nothing
in the codebase today has this shape — even Slice 2's bracket is simpler, single-elimination
only). Recommend treating this as its own dedicated implementation pass within this file, not
rushed alongside Slice 2 just because they share one file.

## Out of scope (this file)

- **Continental-cup-via-domestic-cup-winner** — not requested, adds a timing dependency between
  the cup final and continental qualification cutoff not resolved here. Flag as a possible
  future enhancement.
- **Rebalancing the taper table (3.2) as the world's stature distribution is observed over real
  playtesting** — `TASK-TUNE` territory once this ships, not a design decision here.
- **Club finances/prize money for winning any of these competitions** — separately parked
  (bible §7.3's AI-club-economy item). This file adds fixtures and a simulated result, not any
  money flow from winning them.
- Anything in the national-team or `SuspensionLedger`/integration sibling files.

## Definition of done (this file)

1. `cargo test --workspace` green, including every TDD anchor above for both Slice 2 and Slice 3.
2. No new failures beyond the **10 pre-existing `goat-tui` `smoke_stdin` failures** (same list as
   the foundation file's Definition of Done — verified 2026-07-22, out of scope, pre-existing).
3. `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` clean.
4. No new dependencies.
5. Both playable gates (domestic cup, continental) pass via `cargo run -p goat-tui`.
6. A real perf check on the continental qualification computation (full previous-season table
   data across all 20 nations' Tier-1 leagues, computed at season boundary) — same discipline
   the round-2 world-genesis doc established for genesis/replay cost, not estimated blind.
7. No floats in sim state/logic, no unsafe.
8. **Commit after this file lands, before starting `-slice4-national-teams.md`.**
