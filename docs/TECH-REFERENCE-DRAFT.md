# TECH REFERENCE (draft) — lookup companion to DESIGN_BIBLE.md

> Purpose: a dry, scannable engineering reference — what actually exists in code today,
> not vision/intent. DESIGN_BIBLE.md stays the source for *why*; this is the source for
> *how* and *what's actually implemented*. Verified against `crates/` source on branch
> `feature/update-v2`, 2026-07-22. Anything not directly confirmed against source is
> marked UNVERIFIED.

---

## 1. Module map

| Crate | Purpose | Status |
|---|---|---|
| `goat-fixed` | Fixed-point math, denominator 1000 (3 decimals). `Fixed(i32)`, range ±2,147,483. Attrs live in [1000, 99000] (1.000–99.000). Weights/multipliers in [0,1000]. | **Frozen** — do not touch, 7 tests |
| `goat-rng` | Seeded, injectable RNG with `.fork(name)` per-domain streams (calendar/match/transfer/injury never share a stream). | **Frozen** — 9 tests |
| `goat-core` | Domain model: attrs, roles, positions, generation, derive (ratings/OVR), week loop, `WorldState`/`Intent`/`reduce()`. | Substantial — 30+ unit tests, multiple golden suites |
| `goat-training` | Training routines/intensity/growth/energy, extracted from core. | Exists as standalone crate; **not yet wired** into the live week loop (see §3) |
| `goat-calendar` | Day-tick time orchestrator: `GameClock`, flashpoint arbitration, season boundary pipeline, RNG forking per domain. | Substantial — 7 unit + 10 golden tests |
| `goat-match` | Beat engine, discipline, headspace, `auto_play_match`. | Substantial — 9 golden tests, `discipline::RefPersonality` exists |
| `goat-world` | Genesis, clubs/fixtures, tiered sim, calendar wiring, transfer state. | Substantial — 23+ unit, 10 golden, phase-9 spec tests |
| `goat-meta` | Legacy/pantheon, reputation, contracts, life, money. | Substantial — 7 golden legacy tests + phase-10 spec suites (economy/life/lifestyle/sponsors/retire) |
| `goat-save` | Tiny-save serialization. `SaveData`, `pub const VERSION: u32 = 8` (CLAUDE.md's workspace-layout comment says "v4 format" — **stale**, actual is v8). | Substantial — 7 roundtrip tests |
| `goat-traits` | Player traits (mastery tiers per TRAITS.md). | 7 tests; catalogue is content-pipeline work per docs, unclear how much is implemented — UNVERIFIED depth |
| `goat-bridge` | FFI for Flutter via `flutter_rust_bridge = "=2.12.0"` (pinned; CLAUDE.md says 2.9.0 — **stale**, actual Cargo.lock has 2.12.0). `api.rs` is 1838 lines — genuinely large surface, not scaffolding. | Active, in real use by a Flutter client (see update-v2 branch tasks) |
| `goat-tui` | Text renderer binary + `career-sim`/`world-sim` dev harnesses. Zero sim logic (per CLAUDE.md rule). | Playable — full character creation, week loop, match verified working (this session, seed 1) |

---

## 2. Core data model

**Attributes** — 30 stored sub-attrs (`AttrId`, `NUM_ATTRS`), 6 derived display families (Pace/Shooting/Passing/Dribbling/Defending/Physical, computed by `derive_attrs()` as **unweighted average** per family — no weighting at the family-display level). Each attr: `current`, `potential`, aging archetype (Physical/Technical/Mental, drives trainability — not verified where exactly the archetype table lives, likely `attrs.rs`).

**Positions** — `PrimaryPosition` enum, **8 variants** (ST/W/WM/CAM/CM/DM/FB/CB — L/R are mirrors) — this is the update-v2 branch's "specific position selection" work, DONE per its task file. `PrimaryPosition::ALL` exists (used by `best_position_rating`).

**Roles** — `RoleId`, 14 roles, `FamiliarityTier` (Natural/Competent/Awkward/Unfamiliar) with a `.multiplier()` fn. `ROLE_WEIGHT_TABLE[role]` is a sparse per-attribute weight table (many zero weights — role only scores attrs it uses, per PLAYER_RATING.md C.2's "load-bearing correction" against averaging elite specialists down).

**Rating formulas** (`crates/goat-core/src/derive.rs`):

```
role_rating(current, role, familiarity)
  = weighted_avg(current, ROLE_WEIGHT_TABLE[role])  ×  familiarity.multiplier()
  // simple sparse weighted average, no peak lift — used by match engine / role panel

position_rating(current, pos, familiarity)
  = ( weighted_avg × POS_RATING_AVG_PCT  +  best_key_attr × POS_RATING_PEAK_PCT )
    × familiarity.multiplier()
  // POS_RATING_AVG_PCT = 0.300, POS_RATING_PEAK_PCT = 0.700 (tuning.rs)

ovr(current, primary_pos) = position_rating(current, primary_pos, Natural)
best_position_rating(current) = max over all 8 PrimaryPosition of position_rating(_, _, Natural)
```

**⚠️ Finding — PLAYER_RATING.md is stale on the peak-lift ratio.** The doc's §C.3 says "illustrative blend: 70% average + 30% best-Key." The actual code is the **opposite** — 30% weighted-average + 70% peak-lift — and it's not a bug: `tuning.rs` has a substantial comment trail referencing "C.10" and "previously 40/60 (C.7)" sections that **don't exist in the current `PLAYER_RATING.md` file** (which only goes to §C.6). The doc was never updated past an earlier revision. Current reasoning (per the code comment): 30/70 was chosen deliberately so a fully-trained elite player (Key≈99) can reach OVR 99, and a "vacuum specialist" (one 97 attr, rest ~40) rates ~82 rather than higher — i.e., the ratio has been re-tuned at least twice since the doc was written. **If you're tuning ratings, trust `tuning.rs`, not `PLAYER_RATING.md` §C.3.**

**Also corrects an earlier live-test claim in this session:** OVR 77 vs. best-*role*-rating 51 (Central Mid, Natural) for the same player is **not a bug** — `PLAYER_RATING.md` §C.4 explicitly supersedes the bible's "OVR = best role rating" and redefines OVR as the rating at the player's *current primary position* (`position_rating`, a different formula/table than `role_rating`). The two numbers are expected to differ; they're not the same metric.

**Fixed-point rounding:** all divisions are truncating integer division on raw units (rounds toward zero) — consistent project-wide per the `derive.rs` doc comment.

---

## 3. Week / season loop

**Two clocks, historically desynced** — this is exactly what the update-v2 branch's headline fix addresses:
1. **Season calendar** — `season_round` (30 rounds / 38 Mon–Sun weeks incl. breaks), in `goat-world/src/calendar.rs`.
2. **Player clock** — `age_weeks`, historically ticked only inside `Intent::AdvanceWeek` (training), independent of match-driven season-round advancement. Bug: a player 2 rounds into the season could still read age "16y 0w" because no `AdvanceWeek` had fired.

Branch `feature/update-v2` (1 commit, `0cfbf81`) syncs these. Related fixes in the same commit:
- **Double-tick fix (DONE 2026-07-03):** a double-fixture calendar week previously aged the player 2 weeks in 1. Fixed via `week_ends` on `ApplyRoundResult` (see `goat_world::week_ends_after_round`); `AdvanceWeek` now no-ops if the week already ticked. Pinned by `double_fixture_week_ticks_once` test.
- **Retire-mid-season banking (status unclear — new task, no DONE marker seen):** `Intent::Retire` previously only set `pc_retired = true` without banking the in-progress season's `pc_career_goals`/`pc_career_matches`/`pc_career_output_sum` into career totals (those were only banked by `ApplySeasonEndLegacy`). Found by `spec_bridge_parity.rs`.

**`Intent` enum** (`goat-core/src/state.rs`, ~15+ variants, grouped by the phase that introduced them): `NoOp`, `CreatePlayer{seed, choices}`, `ApplyAttrDelta{player_id, attr, delta}`, `SetRoutine{routine}` (Phase 3), `AdvanceWeek`, `AdvanceWeeks{n}`, `ApplyMatchResult{familiarity_xp, energy_cost, injury_weeks}` (Phase 4, core never sees beat types — the TUI/bridge manages the beat loop and calls this once on completion), `ApplySeasonEndLegacy` (Phase 7), `Retire`, plus later phase-8/9/10 intents (transfer/life/economy — not individually enumerated here, see `state.rs` directly).

**Calendar design (per CALENDAR.md spec — verify against actual `goat-calendar` source if precision matters, this section is spec-level not fully cross-checked against implementation):** day-tick granularity (not week-tick) internally; `tick_one_day()` is the *only* function allowed to mutate `epoch_day`; subsystems poll in a **fixed-order `Vec`** (never HashMap iteration — this ordering is documented as an ABI, reordering breaks save determinism, gated behind a `SIM_VERSION`); `advance_until_flashpoint()` loops silent days and stops on the first `HardStop` or when buffered `SoftFlashpoint`s cross a flush threshold; `simSeasonHeadless` is the *same code path* as live play, differing only in auto-resolving flashpoints instead of returning to a renderer — this is why `career-sim`'s 20-season headless runs are trustworthy as regression tests, not a separate simulation.

RNG streams are forked per domain (`calendarRng`, `matchRng`, `transferRng`, `injuryRng`) precisely so that playing vs. skipping a match doesn't shift downstream transfer/injury rolls.

---

## 4. Match engine

**Two axes, one direction:** Output → Result, never the reverse (MATCH.md §A.1). Your Output is injected into team attacking/defending strength by a position weight; team result is an independent stochastic process layered on top — this is the mechanical root of "hat-trick, team still loses."

**Beat anatomy:** trigger conditions → setup → 2–4 attribute/trait-gated choices → contest resolution (attrs vs. difficulty + RNG) → transitions → ripple consequences (form/headspace/momentum/scoreline). Beats are *selected* from an authored library via context-weighted selector, never generated at runtime (no model calls mid-match).

**Output's unit** (MATCH.md §A.10, a real formula, not just an axis name):
```
output_value = goal_probability_swing × stage_multiplier × difficulty_of_act
```
This is what lets a defensive save/tackle register on the same scale as a goal — a last-ditch tackle preventing a high-probability goal scores near what a goal would; a routine midfield tackle scores near zero. Closes a gap the base bible left open (Output silently meaning "goals+assists" would make a great CB invisible to the pantheon).

**Headspace** (in-match only): Confidence / Nerves / Frustration / Flow axes; feeds contest odds, available choices, and which beats trigger at all. Composure (a stored attribute) damps volatility/recovery speed. `discipline::RefPersonality` exists in source — referees have personality but no per-player memory; dirty reputation (not memory) tightens officiating.

**Layout:** no (x,y) coordinate system — danger lives in ~18-20 discrete named semantic zones (near post, penalty spot, right half-space, etc.), authored/inferred, never simulated. Position-specific resolution: striker gets fine box-zone resolution, midfielder gets pressure-direction/body-orientation/space-between-lines axes (a coarse zone grid can't express these — MATCH.md explicitly calls out an earlier omission where midfield was left at the coarsest resolution).

**Source confirms:** `goat_match::sim::{auto_play_match, BeatLibrary, MatchSetup}` and `discipline::RefPersonality` are real, used directly by `career-sim` — beats.json is `include_str!`'d into the TUI binary at compile time (167/424-ish beats per earlier memory note on a *different* corpus — NOT the same beats.json, don't conflate the two projects).

---

## 5. World layer

Tiered simulation (per DESIGN_BIBLE §7.1, referenced not fully re-verified line-by-line against `goat-world` internals): deep-sim the player's orbit (club/league/direct rivals) match-by-match; batch-tick distant leagues at season granularity; lazy-promote a background player to full-fidelity only on contact (faced, linked in a transfer); background attribute growth for non-orbit players computed on demand from `(seed, birth data, date)` rather than stored/stepped weekly.

Confirmed in source: `fixture_for_round`, `rest_weeks_after_round`, `round_fixtures`, `week_ends_after_round`, `sim_team_match`, `Table`, `BASE_CAREER_YEAR`, `CLUBS`, `DIV_CLUBS`, `DIV_ENG_SEC`, `ROUNDS_PER_SEASON` all exist and are used directly by `career-sim`/`goat-tui`. The playable TUI currently offers only **2 nations** (England, Brazil) and **England Championship** clubs list (16 clubs shown, e.g. Man City ***** down to Nottm Forest ****) — a "small world first" slice per ROADMAP.md's stated principle, not the full 20-30k genesis (that's Phase 9's full-scale genesis, separately confirmed DONE via git log `2715398 Phase 9: full-scale world`).

New test added on update-v2: `fixture_mirror_check.rs` — validates every club pair meets both home and away across the fixture generation.

---

## 6. Save format

`goat-save::save::SaveData`, `pub const VERSION: u32 = 8`. CLAUDE.md's own workspace-layout comment claims "tiny-save serialization (v4 format)" — **stale**, the version has moved to 8 without that top-level doc being updated. Follows the "tiny save" principle: persist results/records/current-season materialized state; past-season fixtures are discarded and regenerated from seed on demand (not verified line-by-line, but consistent with 7 passing roundtrip tests and the CALENDAR.md spec's NFR-02: load under 1s).

---

## 7. Flutter bridge

`crates/goat-bridge/src/api.rs` — **1838 lines**, a large, real FFI surface (not a stub). Confirmed functions include: `new_game`, `advance_week`/`advance_weeks`, `set_routine`, `play_round`, `get_full_season_fixtures`, `get_table`, `get_attributes`/`get_families`/`get_roles`, `get_legacy`, `get_season_awards`, `apply_season_end`, `start_next_season`, `accept_transfer`/`agitate_for_transfer`/`get_transfer_offers`, `get_peers`, `retire`, `save_game`/`load_game`/`get_state`, `list_clubs`, `load_beat_library`. `frb_generated.rs` (2606 lines) is the flutter_rust_bridge-generated glue — do not hand-edit.

**`flutter_rust_bridge` pinned at 2.12.0** (per `Cargo.lock` / build output — CLAUDE.md's coding-conventions section says "2.9.0 pinned," which is stale; don't bump further without asking per that same rule, but the baseline reference number in CLAUDE.md itself needs a correction).

Task files on `update-v2` (`TASK-CORE-creation-role-choice.md`, `TASK-CORE-time-model-age-sync.md`, `TASK-CORE-double-week-tick.md`, `TASK-CORE-retire-banking.md`) are explicitly framed as "Requested by / Found via the Flutter client" — this is a real, active integration, not a future placeholder.

---

## 8. Known doc/code drift summary (all findings above, collected)

| Doc says | Code actually has | Verdict |
|---|---|---|
| Bible §5.2: OVR = best role rating | `derive.rs`: OVR = position_rating at *current primary position* | Superseded by PLAYER_RATING.md §C.4 — doc hierarchy resolves this, not a bug |
| PLAYER_RATING.md §C.3: 70% avg / 30% peak-lift | `tuning.rs`: 30% avg / 70% peak-lift, retuned at least twice (references "C.7"/"C.10" not present in the doc file) | Doc stale — trust `tuning.rs` |
| CLAUDE.md workspace layout: "tiny-save... v4 format" | `goat-save`: `VERSION: u32 = 8` | Doc stale |
| CLAUDE.md coding conventions: `flutter_rust_bridge = "=2.9.0"` pinned | Actual pinned version: 2.12.0 | Doc stale |
| CLAUDE.md source-of-truth filenames (`BecomeTheGOAT-RustCore-TechDoc.md`, `BecomeTheGOAT-TechnicalDesign.md`) | Actual files: `docs/MAIN.md` (merged bible+appendix+calendar), no separate TechnicalDesign file found | Filenames drifted; MAIN.md appears to be the intended target |

None of these are functional bugs — they're documentation that didn't get updated alongside code changes. Worth a cleanup pass, low urgency.
