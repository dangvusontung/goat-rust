# TASK DESIGN ROUND 5, SLICE 1-2 — Club budget & market valuation (foundation)

**Split-file note (read this first):** this file is 1 of 6 that together replace
`tasks/TASK-DESIGN-round5-club-economy.md` (now a short pointer doc). The split happened
2026-07-22 because the original doc (1315 lines) was too large for one Dev pass — same reason
`tasks/TASK-DESIGN-round4-competitions.md` was split into 4 files earlier the same night.
Sibling files: `-slice3-4-scouting.md`, `-slice5-transfers.md`, `-slice6-academy.md`,
`-slice7-8-managers.md`, `-slice9-integration.md`. This file is fully self-contained — implement
it without reading the others or the original doc.

Prereq: **none.** This is the foundation slice — land it, fully tested and committed, before
starting any sibling file. Every later slice in this round reads `Club.budget` (this file's
Slice 1) and/or calls `market_valuation` (this file's Slice 2).

Read first: `docs/MAIN.md` §7.2–7.3 (world genesis, transfer-market framing — AI clubs as "deep
agents, not backdrops," each with "a strategy, finances + budget, a squad-building plan, and its
own manager"); `crates/goat-world/src/world.rs` (`Club`, `WorldGenesis`); `crates/goat-core/src/
state.rs` (Phase 8 PC-facing wage/economy convention this file's units follow); `crates/goat-world/
src/population.rs` (`current_ovr`, `potential_ovr`, `age_years_at` — the cheap SoA columns
`market_valuation` reads).

## Ground rules for this file

- **Club finances are one persisted number (`Club.budget: i64`, £k units, matching `state.rs`'s
  existing `pc_wage_annual`/`pc_savings` unit convention), fed by a sum of named
  income-contributor functions.** Today exactly one contributor exists (tier/strength-derived
  baseline, Slice 1.2). Every future contributor (sponsorship, matchday/ticket sales, shirt
  sales, prize money) is a new sibling function added to the same sum — the spending side
  (siblings' Slices 3–6, and manager wages implicitly) only ever reads the single summed
  `budget` number, never a specific contributor. This is Tùng's explicit instruction, not a
  Design invention.
- **Not a fully simulated revenue model.** No per-player wage contracts for AI/background
  players (1.4 explains why — a real SoA-cost finding, not a preference), no ticket sales tied
  to actual attendance, no sponsorship-deal negotiation flow. Every income/expense number is a
  formula over already-cheap fields (`strength`, `DivLevel`, squad size), consistent with bible
  §9.1's "background growth is formula-driven" discipline.
- **AI-vs-AI transfers only. No PC-facing UI, no change to Phase 8's PC contract/negotiation
  machinery (`state.rs`).** The PC's own transfers stay exactly as they are.
- **No new persisted per-player wage data.** Background players keep their existing SoA shape
  (`seed`, `club`, `nation`, `position`, `birth_age_weeks`, `potential_ovr`, career
  accumulators — `population.rs:31-50`) unchanged. Wages are modeled as a club-level abstracted
  deduction (1.4), not summed from individual contracts.
- **Every invented number is flagged, not silently assumed** — same discipline as round-3/
  round-4. See "Decisions" below.

## Verified: current mechanics and constraints

- **Nothing in this subsystem exists yet** — re-confirmed against real code, 2026-07-22. `grep`
  across the workspace: no `Manager` type, no `Club`-level finance/budget field, no AI-club-
  initiated transfer anywhere. `Club` (`world.rs:57-67`) has exactly four fields today: `id`,
  `name`, `nation`, `strength`, `tactical_identity` — no `squad_size` yet either (round-3's
  Slice 1 is mid-implementation in this same working tree by a concurrent Dev round; this file
  does not depend on it landing first — `budget` is added below as its own independent field
  regardless).
- **Phase 8 (`state.rs:108-115`, `332-382`, `627-758`) is entirely PC-facing** —
  `pc_wage_annual`, `pc_savings`, wage-negotiation intents, end-of-season PC economy settlement.
  Zero overlap with this file's club-level, AI-facing budget. `state.rs:235` sets `£20k/yr` at
  PC start — the unit-scale anchor this file's `× 20` (1.3) matches.
- **`goat-save::save::VERSION` is currently `10`** (`save.rs:30`) — the new persisted `Club.
  budget` field this file adds requires a version bump (Definition of Done below), same as
  round-3 flagged for `intake_week`.

## Slice 1 — Club budget: one persisted number, additive-contributor income

### 1.1 — Data shape

```rust
pub struct Club {
    pub id: ClubId,
    pub name: String,
    pub nation: NationId,
    pub strength: u8,
    pub tactical_identity: TacticalIdentity,
    /// Running transfer/wage war-chest, £k. NEW. A single number by design (Tùng's
    /// explicit instruction) — see `total_income` below for how future revenue sources
    /// plug into it without touching the spending side.
    pub budget: i64,
}
```

Persisted across seasons — **not** recomputed from scratch each window. Unspent money carries
forward; spent money stays spent. Can legitimately go negative (an overspent club in financial
distress is a meaningful state a later slice's bid-ceiling formula reads and naturally excludes
from bidding — not a bug to clamp away).

### 1.2 — Income: a sum of named contributors, one implemented today

```rust
/// One additive contributor to a club's per-window income. Today only this one exists;
/// future rounds add sibling functions (sponsorship, matchday/ticket sales, shirt sales,
/// prize money — bible §7.2's "rich identity... finances" list) to `total_income`'s sum.
/// Every caller of `total_income` (1.5, genesis seeding) is untouched when a new
/// contributor is added — this is the whole point of the abstraction Tùng asked for.
fn tier_baseline_income(strength: u8, tier: DivLevel) -> i64 {
    let tier_mult: i64 = match tier {
        DivLevel::Top => 12,    // TV money, prestige sponsorship — top-flight clubs earn
        DivLevel::Second => 4,  // far more per point of strength than lower tiers
        DivLevel::Third => 1,
    };
    (strength as i64) * tier_mult * 20 // scaled to £k units (1.3 explains the scale)
}

/// The one number every spending formula in this round reads. Today: `tier_baseline_income`
/// alone. Composing a new source later is a one-line addition here, nothing else changes.
pub fn total_income(club: &Club, tier: DivLevel) -> i64 {
    tier_baseline_income(club.strength, tier)
    // future: + sponsorship_income(club)
    // future: + matchday_income(club, attendance_proxy)
    // future: + shirt_sales_income(club, stature)
    // future: + prize_money_income(club, this_season_results)  — composes naturally with
    //   round-4's competition results (cup runs, continental qualification) once that
    //   doc's `Competition`/`FixtureImportance` machinery is read here — not built now.
}
```

### 1.3 — Why £k units, and the scale (`× 20`)

Matches `state.rs`'s existing `pc_wage_annual`/`pc_savings` unit convention (`£20k/yr` at PC
start, `state.rs:235`) so the two systems' numbers are at least dimensionally comparable, even
though this round never mixes them. At `strength = 99`, `tier = Top`: `99 × 12 × 20 = 23,760`
(£23.8M) per window. At `strength = 50`, `tier = Top`: `12,000` (£12M). At `strength = 1`,
`tier = Third`: `20` (£20k — a bottom-tier minnow, correctly almost nothing). This is
intentionally compressed vs. real-world transfer economics (a genuine 99-OVR player would cost
far more than any of these clubs could ever raise) — an explicit "abstracted, not simulated"
trade Tùng asked for, not an oversight; see Slice 2 for the matching compression in valuation.

### 1.4 — Wages: an abstracted seasonal deduction, not per-player contracts

No new `Population` column (per "Ground rules"). Instead, a formula over the squad's own
already-cheap `current_ovr`/size, applied once per window as a deduction against `total_income`
rather than a separate persisted wage-bill number:

```rust
/// Abstracted wage cost for one window: proportional to squad quality (mean current OVR,
/// the same "live strength" quantity `Population::live_strength_from_squad`, round-3 Slice
/// 3.2, already computes cheaply) and squad size. Elite squads cost more to run, not just
/// to buy into.
fn window_wage_deduction(pop: &Population, squad: &[usize], elapsed_weeks: u32) -> i64 {
    if squad.is_empty() {
        return 0;
    }
    let mean_ovr = pop.live_strength_from_squad(squad, elapsed_weeks) as i64; // 1-99
    let per_player_wage = mean_ovr * mean_ovr / 10; // superlinear — elite wages compound
    per_player_wage * squad.len() as i64
}
```

At `mean_ovr = 80`, `squad.len() = 24`: `(6,400/10) × 24 = 15,360` per window — a strong club's
wage bill approaches its own income at that strength/tier combination, which is the intended
tension (a club can't buy indefinitely without a squad-quality cost catching up).

### 1.5 — Window top-up: one mutation, called from the integration slice's wiring

```rust
pub fn open_transfer_window(
    club: &mut Club,
    pop: &Population,
    squad: &[usize],
    tier: DivLevel,
    elapsed_weeks: u32,
) {
    club.budget += total_income(club, tier) - window_wage_deduction(pop, squad, elapsed_weeks);
}
```

`squad: &[usize]` is the list of population indices currently at this club — a small helper
(`squad_of`/`squads_by_club`, indexing `pop.club[i] == club.id`) that every prior round's
club-scoped code already builds ad hoc; write it locally here if no shared helper exists yet.

### 1.6 — Genesis seeding

```rust
// In WorldGenesis::generate's per-club loop, alongside strength/tactical_identity:
club.budget = 2 * tier_baseline_income(club.strength, tier); // ~one season's income as a
                                                                // starting war-chest
```

### TDD anchor (Slice 1)

- `total_income_is_monotonic_in_strength_and_tier`: for fixed tier, higher `strength` →
  higher `total_income`; for fixed `strength`, `Top > Second > Third`.
- `budget_can_go_negative_and_stays_negative_until_income_recovers`: force a club's wage
  deduction above its income for several windows, assert `budget` tracks the running sum
  exactly (no implicit floor at 0).
- `window_wage_deduction_scales_with_squad_quality_and_size`: two squads of equal size, one
  with uniformly higher `current_ovr`, produce a strictly higher deduction; two squads of
  equal quality, different sizes, the larger one deducts more.
- `genesis_seeds_two_windows_of_income`: every club's genesis `budget` equals exactly
  `2 * tier_baseline_income(...)` for its own strength/tier.

## Slice 2 — Market valuation: the shared "what is this player worth" formula

Read once, called from every sibling file that needs "what is this player worth" (weakest-
position search, gem-hunting, auction floor price) — one formula, not three.

### 2.1 — Formula

```rust
/// A player's transfer valuation (the selling club's floor/reserve price), £k. Deliberately
/// weights *current* ability far more than *potential* — this underpricing of unrealized
/// potential is not a bug, it's the exact market inefficiency the gem-hunting lane (a sibling
/// slice) exploits (2.3 below).
pub fn market_valuation(current_ovr: u8, potential_ovr: u8, age: u32) -> i64 {
    let ovr = current_ovr as i64;
    let base = ovr * ovr; // superlinear — elite current ability costs disproportionately more
    let ceiling_hint = (potential_ovr as i64 - ovr).max(0) * POTENTIAL_HINT_WEIGHT;
    ((base + ceiling_hint) * age_value_pct(age)) / 100
}

const POTENTIAL_HINT_WEIGHT: i64 = 3; // the market pays a *little* for scouted upside, not
                                       // its full eventual value — see 2.3

fn age_value_pct(age: u32) -> i64 {
    match age {
        0..=20 => 60,   // unproven teenager discount
        21..=29 => 100, // peak transfer-value years
        30..=33 => 65,
        _ => 30,
    }
}
```

### 2.2 — Why this shape, concretely

A 30-year-old at `current_ovr = 85`, `potential_ovr = 85` (a peaked veteran): `base = 7,225`,
`ceiling_hint = 0`, `age_pct = 65` → valuation `4,696`. A 19-year-old at `current_ovr = 60`,
`potential_ovr = 90` (a rising talent, round-3's genesis anchor formula could easily produce
this at a strong club, or the outlier roll at *any* club): `base = 3,600`, `ceiling_hint =
90 (30 × 3)`, `age_pct = 60` → valuation `2,214` — **cheaper than the peaked veteran despite a
higher ceiling**, exactly the real-football "buy before they break out" dynamic.

### 2.3 — Why gem-hunting is a real, findable strategy and not a no-op

Round-3 Slice 2's outlier roll (`OUTLIER_CHANCE_PCT = 2`,
`tasks/TASK-DESIGN-round3-player-driven-club-strength.md` §2.1) guarantees roughly 1-in-50
players at *any* club — including bottom-tier minnows with a flat `strength ≤ 14` genesis
ceiling — roll a `potential_ovr` anywhere in `[30, 99]`, independent of club strength. Because
`market_valuation` (2.1) prices mostly off *current* OVR (which, for a young player, is still
low — `current_ovr` is `potential_ovr × development_fraction(age)`, `population.rs:189-193`,
and `development_fraction` is small for teenagers), an outlier prospect at a weak club is
**cheap by this formula despite a high ceiling** — the exact "unearthed at a nobody club" story
round-3 Slice 2.2 already designed for, now given an actual buyer (a sibling slice's
gem-hunting lane) instead of sitting inert. This is the load-bearing claim a sibling file's
`TASK-DESIGN-round5-club-economy-slice3-4-scouting.md` builds its gem-hunting search on top of.

### TDD anchor (Slice 2)

- `valuation_favors_current_over_potential`: two players with equal `potential_ovr`, one with
  higher `current_ovr`, the higher-current one values higher (isolates the `base` term).
- `valuation_underprices_young_high_ceiling_players_relative_to_ovr_sum`: construct the 2.2
  example numerically, assert the young prospect's valuation is materially below the veteran's
  despite a higher `current_ovr + potential_ovr` sum — the direct regression for 2.3's claim.
- `age_value_pct_peaks_in_mid_twenties`: monotonic up into `21..=29`, monotonic down after.

## Out of scope (this file)

- **Sponsorship, matchday/ticket sales, shirt sales, prize-money income contributors** — 1.2's
  `total_income` is explicitly designed to accept these later without a rewrite, but none of
  them are built in this round; today `tier_baseline_income` is the only term.
- **Per-player wage contracts for AI players** — wages stay an abstracted per-window deduction
  (1.4), not individual contract data.
- Weakest-position/gem-hunt search, the auction, youth-academy investment, managers,
  season-tick wiring — all sibling files' work. This file only builds the budget/valuation
  primitives they read.

## Decisions Design made as judgment calls — flag for Tùng's explicit sign-off

1. **Slice 1**: the entire `tier_baseline_income` formula — tier multipliers (`12/4/1`), the
   `× 20` scale, and the genesis seed (`2 ×` one window's income) — is Design's own numeric
   construction from Tùng's "derived from strength/tier" instruction. Needs a `TASK-TUNE` pass
   once playtested, same convention round-3 used for its own first-pass constants.
2. **Slice 1.4**: `window_wage_deduction`'s formula (`mean_ovr² / 10 × squad_size`) and its
   choice to be an abstracted deduction rather than any per-player data — the "abstracted, not
   simulated" framing is Tùng's, the exact shape is Design's.
3. **Slice 2**: the entire `market_valuation` formula, including `POTENTIAL_HINT_WEIGHT = 3`
   and the `age_value_pct` curve — Design's own construction, the load-bearing number every
   other slice depends on. Highest-priority item for a tuning pass; the *qualitative* shape
   (current-weighted, underprices potential) is the actual design decision Tùng should
   confirm, the exact coefficients are secondary.

These are first-pass numbers for a later `TASK-TUNE` pass once playtested — not blocking items
Tùng needs to approve before Dev starts, same framing round-3's own judgment calls used.

## Definition of done (Slice 1-2)

1. `cargo test --workspace` green, including every TDD-anchor test listed above.
2. `goat-save::save::VERSION` bumped past `10`, with the new `Club.budget` field serialized/
   deserialized and round-tripped (`crates/goat-save/tests/save_roundtrip.rs` extended,
   mirroring how round-3 flagged `intake_week` for the same treatment).
3. `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` clean.
4. No new dependencies. No floats in sim state/logic, no unsafe.
5. No new failures beyond the **10 pre-existing `goat-tui` `smoke_stdin` failures** (verified
   2026-07-22, out of scope, unrelated to this work, caused by a `generate_club_name()` bug in
   `crates/goat-world/src/world.rs`): `confirm_screen_blank_enter_reprompts_instead_of_
   discarding_character`, `double_w_in_same_round_shows_message_not_silent_noop`,
   `game_sheet_and_player_sheet_boxes_close_for_short_and_long_club_names`, `key_moments_lines_
   close_with_ellipsis_not_ragged_cutoff`, `legacy_screen_notes_mid_season_batching`,
   `main_loop_unrecognized_command_messages_and_continues`, `player_sheet_explains_ovr_is_
   position_weighted`, `save_overwrite_requires_explicit_confirmation`, `save_to_empty_slot_
   succeeds_without_confirmation`, `status_header_shows_energy_percent_and_labeled_discipline_
   count`.
6. **Commit this slice before starting any sibling file.** This is the entire reason the
   original doc was split — an interruption after this commit loses nothing already landed.
