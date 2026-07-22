# TASK DESIGN ROUND 5 — AI-run club economy: finances, transfers, managers

Prereq: none to *start this design*, but this doc's Slice 6 (youth-academy investment) composes
directly with `tasks/TASK-DESIGN-round3-player-driven-club-strength.md`'s Slice 4 (youth intake)
and Slice 2 (outlier roll) — **verified already implemented on disk**, see "Verified" below —
and this doc's transfer-window cadence composes with the already-shipped calendar window layout
in `crates/goat-core/src/calendar_loop.rs`. Slice 7–8 (managers) reuse `TacticalIdentity`
(`crates/goat-core/src/tactical_identity.rs`), shipped as part of round-2 Doc B
(`tasks/TASK-DESIGN-round2-national-team-tactical-identity.md`).

This is **backlog item #3** from the parked list at the bottom of
`tasks/TASK-DESIGN-round2-world-genesis-scaleup.md` ("AI-run club economy: transfer market
between AI clubs, managers, club finances"), designed per Tùng's interactive session,
2026-07-22. That parked section already points at bible §7.3 ("Transfer Market & AI Clubs" —
AI clubs as "deep agents, not backdrops," each with "a strategy, finances + budget, a
squad-building plan, and its own manager") and bible §7.2 item 2 ("rich identity... finances").
Round-3's doc explicitly deferred both AI-club-to-AI-club transfers and any club-level finance
field to this round (`TASK-DESIGN-round3-player-driven-club-strength.md` §4.7, "Out of scope"),
so this doc is that deferred work, not a re-litigation of round-3's decisions.

Read first: `docs/MAIN.md` §7.2–7.3 (world genesis, transfer market framing, quoted above);
`crates/goat-world/src/world.rs` (`Club`, `WorldGenesis`, `seed_mix`/`nation_seed`/`club_seed` —
the seeding convention this doc's new seeds follow); `crates/goat-world/src/population.rs`
(`Population`, `current_ovr`, `is_retired`, `roll_potential_ovr`/youth-intake machinery from
round-3 Slice 2/4 — this doc's Slice 6 hooks directly into it); `crates/goat-world/src/
batch_tick.rs` (`batch_tick_season` — the per-season match-simulation loop this doc's manager
form-tracking hooks into); `crates/goat-world/src/promotion.rs` (`ReplayCache::advance_one_season`
— the season-tick entry point every prior round's new season-boundary mechanic hooks into, and
this doc's too); `crates/goat-core/src/tactical_identity.rs` (`TacticalIdentity`, reused
verbatim for manager identity); `crates/goat-core/src/derive.rs` + `crates/goat-core/src/
roles.rs` (`role_rating`, `ROLE_WEIGHT_TABLE` — why these are *not* reusable for population-wide
transfer search, see "Verified"); `crates/goat-core/src/state.rs` (Phase 8 PC-facing contract/
wage machinery — confirms zero overlap with this doc's AI-club-facing budget); `crates/goat-core/
src/calendar_loop.rs` (the already-shipped `TransferWinter`/`TransferSummer` window layout this
doc's cadence borrows).

## Ground rules for this doc

- **Club finances are one persisted number (`Club.budget: i64`, £k units, matching
  `state.rs`'s existing `pc_wage_annual`/`pc_savings` unit convention), fed by a sum of named
  income-contributor functions.** Today exactly one contributor exists (tier/strength-derived
  baseline, Slice 1). Every future contributor (sponsorship, matchday/ticket sales, shirt
  sales, prize money) is a new sibling function added to the same sum — the spending side
  (Slices 3–6, and manager wages implicitly) only ever reads the single summed `budget` number,
  never a specific contributor, so this composes without a rewrite. This is Tùng's explicit
  instruction, not a Design invention.
- **Not a fully simulated revenue model.** No per-player wage contracts for AI/background
  players (Slice 1.4 explains why — a real SoA-cost finding, not a preference), no ticket
  sales tied to actual attendance, no sponsorship-deal negotiation flow. Every income/expense
  number this doc adds is a formula over already-cheap fields (`strength`, `DivLevel`, squad
  size), consistent with bible §7.1's "background growth is formula-driven" discipline.
- **AI-vs-AI transfers only. No PC-facing UI, no change to Phase 8's PC contract/negotiation
  machinery (`state.rs`).** The PC's own transfers stay exactly as they are; this doc's market
  is a background/non-orbit-league mechanic, same tier as `batch_tick_season`. A future round
  could let the PC's own club participate as one more bidder in this market (bible §7.3: "your
  teammates arrive and leave") — **explicitly out of scope here**, see "Out of scope."
- **Season-granularity architecture stays intact — two transfer-window *passes* per season-tick,
  not real calendar-day events for background clubs.** `calendar_loop.rs`'s `TransferWinter`/
  `TransferSummer` windows already exist for the **orbit** (PC-facing, day-tick) calendar, but
  `batch_tick_season` (the background-league path this doc's market lives in) simulates a whole
  season's matches in one call, with no sub-season day resolution. Splitting that call into two
  match-halves so a literal mid-season window could interleave is a bigger structural change
  than Tùng asked for this round. **Design's call, flagged for sign-off (see "Decisions"
  section):** both windows run as two back-to-back passes at the same season-tick boundary
  (winter pass, then the season's matches, then summer pass, then manager evaluation) — see
  Slice 9 for the exact ordering and why.
- **No new persisted per-player wage data.** Background players keep their existing SoA shape
  (`seed`, `club`, `nation`, `position`, `birth_age_weeks`, `potential_ovr`, career
  accumulators — `population.rs:31-50`) unchanged. Wages are modeled as a club-level abstracted
  deduction (Slice 1.4), not summed from individual contracts.
- **"Weakest position," not "weakest role," for population-wide search — a real constraint,
  not a simplification of convenience.** See "Verified" for the concrete reason: the 14-role
  `role_rating`/`ROLE_WEIGHT_TABLE` system needs full per-attribute `current` values, which
  background (non-lazy-promoted) players do not carry. Searching the whole ~29,000-player
  background population at role granularity would force realizing (`Population::promote`)
  most of the population every window, defeating the entire "cheap identity, full realization
  on contact" principle (bible §9, `population.rs:1-9`) this codebase is built around. This
  doc's target search uses the coarse 3-way `position` field (`population.rs:38-39`:
  Defender/Midfielder/Forward) plus `current_ovr`, both cheap SoA columns, for the whole-market
  scan; the 14-role machinery is reserved for what it already does (orbit-path role fit,
  Doc B's national-team call-ups).
- **Managers are a new, lightweight, non-SoA entity type — justified in Slice 7.1.** There are
  only ~1,200 of them (one per club) plus a small reserve pool; this is `Club`-scale data
  (small `Vec` of a small struct), not `Population`-scale, so no column-oriented storage is
  needed.
- **Every invented number is flagged, not silently assumed** — same discipline as round-3/
  round-4. This doc invents substantially more numbers than either of those (a full valuation
  formula, an auction mechanism, a firing threshold) because Tùng's brief explicitly asked
  Design to "decide and justify" these, not because the bar for rigor is lower — see
  "Decisions Design made" for the complete list.

## Verified: current mechanics and constraints (read in full before this round's design)

**Nothing in this subsystem exists yet — re-confirmed against real code, 2026-07-22.** `grep`
across the workspace: no `Manager` type, no `Club`-level finance/budget field, no AI-club-
initiated transfer anywhere. `Club` (`world.rs:57-67`) has exactly four fields: `id`, `name`,
`nation`, `strength`, `tactical_identity` — no `squad_size` yet either (round-3's Slice 1 is
mid-implementation in this same working tree as of this writing, by a concurrent Dev round;
this doc does not depend on it landing first — Slice 1 below adds `budget` as its own
independent new field regardless of whether `squad_size` has landed).

**Phase 8 (`state.rs:108-115`, `332-382`, `627-758`) is entirely PC-facing** — `pc_wage_annual`,
`pc_savings`, wage-negotiation intents, end-of-season PC economy settlement. Zero overlap with
this doc's club-level, AI-facing budget. Confirms round-2's original parking note was accurate.

**Two transfer windows per year already exist — but only on the PC-facing orbit calendar.**
`crates/goat-core/src/calendar_loop.rs:32-56`'s `standard_season()`: `TransferWinter` (days
160–195, 35 days) and `TransferSummer` (days 330–364, wrapping the season boundary, 34 days),
inside a 365-day season (`SEASON_DAYS`). This confirms Tùng's "two windows/year" framing is
already the game's calendar shape — this doc's Slice 9 borrows the *name and count* (winter,
summer) but not the day-level machinery, per the "Ground rules" note above.

**The season-tick hook point every prior round used is `ReplayCache::advance_one_season`**
(`promotion.rs:137-149`): calls `batch_tick_season` (all of one season's matches), then
`apply_season_end` (promotion/relegation). Round-3 Slice 4 (youth intake) already established
the pattern of adding a new call inside this function (`TASK-DESIGN-round3...md` §4.6). This
doc's Slice 9 extends the same function further.

**`batch_tick_season` (`batch_tick.rs:73-167`) already computes live per-club strength and
iterates every match of every division, round by round** (`for round in 0..ROUNDS_PER_SEASON {
for f in round_fixtures(...) { let (gf, ga) = sim_team_match(...); table.apply_result(...); }
}`, `batch_tick.rs:93-97`). Per-match, per-club results (`gf`/`ga`, hence win/draw/loss) are
computed transiently inside this loop but never captured per-club outside the aggregate
`Table` — Slice 8 needs them (for manager rolling form) and adds a zero-ripple sibling function
to capture them, same pattern round-3 Slice 5 used for `generate_player`/`generate_player_biased`
(see Slice 8.3).

**Why population-wide role-granularity search is not viable (the finding behind the "Ground
rules" note above).** `derive::role_rating` (`derive.rs:43-61`) requires `current: &[Fixed;
NUM_ATTRS]` — the full 30-attribute vector. Background (non-realized) players only carry
`potential_ovr: Vec<u8>` (`population.rs:44`); their full per-attribute `current` only exists
after `Population::promote` (`population.rs:204-229`) lazy-realizes them — a per-player-seed
call to the same full generation pipeline the PC's own creation uses. Running this for even
"just each club's own squad" every transfer window, across 1,200 clubs × ~24 players, is
~28,800 realizations per window — roughly the *entire* background population, every window,
forever. That is exactly the cost bible §9 lazy-promotion exists to avoid. `current_ovr`
(`population.rs:189-193`), by contrast, is a cheap closed-form function of `potential_ovr` +
age — no realization needed, and it's the same formula `batch_tick.rs`'s `club_strength`
already scans the whole population with every season. This doc's target search reuses that
existing, already-population-scale-proven cost profile.

**The seed-mixing convention this doc's new deterministic draws follow.** `world.rs:140-144`
(`seed_mix(world_seed, salt, idx)`, XOR of two golden-ratio-constant multiplies) and
`population.rs:109-113`/`461-467` (`player_seed`/`intake_player_seed`, the same idea, each
module keeping its own small local copy rather than sharing one — no shared "seed util" module
exists in this codebase, each of `world.rs`/`population.rs` already has its own near-identical
private helper). This doc's Slices 5 and 7 add their own local copies of the same idiom,
matching existing precedent rather than introducing a new cross-module dependency.

**Name generation for a new person-like entity is already solved.** `history::name_from_seed`
(`history.rs:81-83`, public) wraps the existing 16×16 first/last-name word bank
(`history.rs:76-79`) used for promoted/cohort players and pantheon greats. Slice 7 reuses it
directly for manager names — no new word bank.

**`goat-save::save::VERSION` is currently `10`** (`save.rs:30`) — every new persisted field this
doc adds (`Club.budget`, the manager pool, per-club manager assignment) requires a version bump,
same as round-3 Slice 4 flagged for `intake_week`.

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
    /// plug into it without touching the spending side (Slices 3–6).
    pub budget: i64,
}
```

Persisted across seasons — **not** recomputed from scratch each window. Unspent money carries
forward; spent money stays spent. Can legitimately go negative (an overspent club in financial
distress is a meaningful state the bid-ceiling formula, Slice 5.2, reads and naturally excludes
from bidding — not a bug to clamp away).

### 1.2 — Income: a sum of named contributors, one implemented today

```rust
/// One additive contributor to a club's per-window income. Today only this one exists;
/// future rounds add sibling functions (sponsorship, matchday/ticket sales, shirt sales,
/// prize money — bible §7.2's "rich identity... finances" list) to `total_income`'s sum.
/// Every caller of `total_income` (Slice 1.5, genesis seeding) is untouched when a new
/// contributor is added — this is the whole point of the abstraction Tùng asked for.
fn tier_baseline_income(strength: u8, tier: DivLevel) -> i64 {
    let tier_mult: i64 = match tier {
        DivLevel::Top => 12,    // TV money, prestige sponsorship — top-flight clubs earn
        DivLevel::Second => 4,  // far more per point of strength than lower tiers
        DivLevel::Third => 1,
    };
    (strength as i64) * tier_mult * 20 // scaled to £k units (Slice 1.6 explains the scale)
}

/// The one number every spending formula in this doc reads. Today: `tier_baseline_income`
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
trade Tùng asked for, not an oversight; see valuation (Slice 2) for the matching compression.

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

### 1.5 — Window top-up: one mutation, called from Slice 9's wiring

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

### 1.6 — Genesis seeding

```rust
// In WorldGenesis::generate's per-club loop, alongside strength/tactical_identity:
club.budget = 2 * tier_baseline_income(club.strength, tier); // ~one season's income as a
                                                                // starting war-chest
```

### TDD anchor

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

Read once, called from Slices 3 (weakest-position), 4 (gem-hunting), and 5 (auction floor
price) — one formula, not three.

### 2.1 — Formula

```rust
/// A player's transfer valuation (the selling club's floor/reserve price), £k. Deliberately
/// weights *current* ability far more than *potential* — this underpricing of unrealized
/// potential is not a bug, it's the exact market inefficiency Slice 4's gem-hunting lane
/// exploits (2.3 below).
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
higher ceiling**, exactly the real-football "buy before they break out" dynamic, and the
concrete number Slice 4's gem-hunting logic is built to find.

### 2.3 — Why gem-hunting is a real, findable strategy and not a no-op

Round-3 Slice 2's outlier roll (`OUTLIER_CHANCE_PCT = 2`, `TASK-DESIGN-round3...md` §2.1)
guarantees roughly 1-in-50 players at *any* club — including bottom-tier minnows with a flat
`strength ≤ 14` genesis ceiling — roll a `potential_ovr` anywhere in `[30, 99]`, independent of
club strength. Because `market_valuation` (2.1) prices mostly off *current* OVR (which, for a
young player, is still low — `current_ovr` is `potential_ovr × development_fraction(age)`,
`population.rs:189-193`, and `development_fraction` is small for teenagers), an outlier
prospect at a weak club is **cheap by this formula despite a high ceiling** — the exact
"unearthed at a nobody club" story round-3 Slice 2.2 already designed for, now given an actual
buyer (Slice 4) instead of sitting inert.

### TDD anchor

- `valuation_favors_current_over_potential`: two players with equal `potential_ovr`, one with
  higher `current_ovr`, the higher-current one values higher (isolates the `base` term).
- `valuation_underprices_young_high_ceiling_players_relative_to_ovr_sum`: construct the 2.2
  example numerically, assert the young prospect's valuation is materially below the veteran's
  despite a higher `current_ovr + potential_ovr` sum — the direct regression for 2.3's claim.
- `age_value_pct_peaks_in_mid_twenties`: monotonic up into `21..=29`, monotonic down after.

## Slice 3 — Weakest-position detection & target search

### 3.1 — Detecting the gap: coarse `position`, not 14-role `RoleId` (per "Verified")

```rust
/// Which of the 3 coarse positions has this club's weakest *best* player, by current OVR.
/// Cheap: one pass over the (small, `~18-30`-player) squad, no realization.
fn weakest_position(pop: &Population, squad: &[usize], elapsed_weeks: u32) -> Option<u8> {
    (0u8..3).min_by_key(|&pos| {
        squad
            .iter()
            .filter(|&&i| pop.position[i] == pos && !pop.is_retired(i, elapsed_weeks))
            .map(|&i| pop.current_ovr(i, elapsed_weeks))
            .max()
            .unwrap_or(0) // an empty position group reads as maximally weak — correct: a
                           // club with *no* forward at all should prioritize buying one
    })
}
```

### 3.2 — Target search: one precomputed sorted list per position, per window, not per club

```rust
/// Built once per window (not once per club — 1,200 clubs re-scanning the whole population
/// each would be the same cost mistake `batch_tick.rs`'s own doc-comment on
/// `live_strength`/`live_strength_from_squad` already warns against, round-3 §3.2). Each
/// position's `Vec` is sorted descending by `current_ovr` once; every club's search below is
/// then a bounded prefix scan, not a rescan.
fn candidates_by_position(
    pop: &Population,
    elapsed_weeks: u32,
) -> [Vec<usize>; 3] {
    let mut out: [Vec<usize>; 3] = Default::default();
    for i in 0..pop.len() {
        if !pop.is_retired(i, elapsed_weeks) {
            out[pop.position[i] as usize].push(i);
        }
    }
    for list in &mut out {
        list.sort_by_key(|&i| std::cmp::Reverse(pop.current_ovr(i, elapsed_weeks)));
    }
    out
}

/// A club's single weakest-position target for this pass, or `None` if nothing affordable
/// beats what it already has. `lane_cap` is this club's weakest-position spending ceiling
/// this window (Slice 5.2 defines it from `club.budget`).
fn weakest_position_target(
    club_id: ClubId,
    pop: &Population,
    squad: &[usize],
    candidates: &[Vec<usize>; 3],
    lane_cap: i64,
    elapsed_weeks: u32,
) -> Option<usize> {
    let pos = weakest_position(pop, squad, elapsed_weeks)?;
    let own_best = squad
        .iter()
        .filter(|&&i| pop.position[i] == pos)
        .map(|&i| pop.current_ovr(i, elapsed_weeks))
        .max()
        .unwrap_or(0);
    // Bounded prefix scan (e.g. top 50 by current_ovr in this position) — a real upgrade
    // (current_ovr > own_best), not already at this club, and its market_valuation fits
    // within this lane's cap.
    candidates[pos as usize]
        .iter()
        .take(TARGET_SEARCH_PREFIX)
        .copied()
        .find(|&i| {
            pop.club[i] as usize != club_id
                && pop.current_ovr(i, elapsed_weeks) > own_best
                && market_valuation(
                    pop.current_ovr(i, elapsed_weeks),
                    pop.potential_ovr[i],
                    pop.age_years_at(i, elapsed_weeks),
                ) <= lane_cap
        })
}

const TARGET_SEARCH_PREFIX: usize = 50; // Design's own bound, see "Decisions"
```

### TDD anchor

- `weakest_position_finds_the_real_gap`: a squad with a strong defense/midfield and zero
  forwards returns `Forward`, matching the "empty group reads maximally weak" comment.
- `target_search_never_returns_own_players_or_downgrades`: for any squad/candidate list, the
  returned target (if any) is never already at `club_id` and always has strictly higher
  `current_ovr` than `own_best`.
- `target_search_respects_lane_cap`: a target whose `market_valuation` exceeds `lane_cap` is
  never returned, even if it's the top-ranked candidate.
- `candidates_by_position_is_deterministic_and_sorted`: same population twice → identical
  sorted lists; every list is non-increasing in `current_ovr`.

## Slice 4 — Gem-hunting target search

### 4.1 — Scoring: the current/potential gap `market_valuation` deliberately underprices

```rust
const GEM_HUNT_MAX_AGE: u32 = 21; // only unproven-ceiling players are "gems" — a peaked
                                   // veteran has no unrealized potential left to buy cheap

fn gem_hunt_score(current_ovr: u8, potential_ovr: u8, age: u32) -> i64 {
    if age > GEM_HUNT_MAX_AGE {
        return 0;
    }
    (potential_ovr as i64 - current_ovr as i64).max(0)
}
```

### 4.2 — Target search: whole population, not position-gated (gem-hunting is proactive, not
gap-filling — per Tùng's explicit "not just reactive to squad gaps" requirement)

```rust
/// Reuses the same `candidates_by_position` lists (Slice 3.2) purely as a cheap, already-
/// built "who's out there" index — re-sorted here by `gem_hunt_score` instead of
/// `current_ovr`, since this lane's ranking question is different from the weakest-position
/// lane's. Built once per window, same cost discipline as 3.2.
fn gem_targets_by_position(
    pop: &Population,
    candidates: &[Vec<usize>; 3],
    elapsed_weeks: u32,
) -> [Vec<usize>; 3] {
    let mut out = candidates.clone();
    for list in &mut out {
        list.sort_by_key(|&i| {
            std::cmp::Reverse(gem_hunt_score(
                pop.current_ovr(i, elapsed_weeks),
                pop.potential_ovr[i],
                pop.age_years_at(i, elapsed_weeks),
            ))
        });
    }
    out
}

fn gem_hunt_target(
    club_id: ClubId,
    pop: &Population,
    gem_lists: &[Vec<usize>; 3],
    lane_cap: i64,
    elapsed_weeks: u32,
) -> Option<usize> {
    // Scan across all 3 position lists' top prefixes, pick the single highest-scoring
    // affordable candidate overall — gem-hunting doesn't care which position the gem plays.
    gem_lists
        .iter()
        .flat_map(|list| list.iter().take(TARGET_SEARCH_PREFIX).copied())
        .filter(|&i| {
            pop.club[i] as usize != club_id
                && gem_hunt_score(
                    pop.current_ovr(i, elapsed_weeks),
                    pop.potential_ovr[i],
                    pop.age_years_at(i, elapsed_weeks),
                ) > 0
                && market_valuation(
                    pop.current_ovr(i, elapsed_weeks),
                    pop.potential_ovr[i],
                    pop.age_years_at(i, elapsed_weeks),
                ) <= lane_cap
        })
        .max_by_key(|&i| {
            gem_hunt_score(
                pop.current_ovr(i, elapsed_weeks),
                pop.potential_ovr[i],
                pop.age_years_at(i, elapsed_weeks),
            )
        })
}
```

### TDD anchor

- `gem_hunt_score_zero_past_max_age`: any player with `age > GEM_HUNT_MAX_AGE` scores exactly
  `0`, regardless of potential/current gap.
- `gem_hunt_prefers_outlier_style_prospects`: construct a population where one weak-club young
  player has a round-3-outlier-style high `potential_ovr` far above its low `current_ovr` —
  assert it's the top-ranked gem target ahead of ordinary anchor-formula players at the same
  club — the direct regression for Slice 2.3's claim.
- `gem_hunt_ignores_position_gaps`: a club with no squad weakness at all (every position
  already strong) can still return a non-`None` gem target, unlike Slice 3's
  `weakest_position_target`.

## Slice 5 — Deterministic bidding-round auction & transfer execution

### 5.1 — Lane caps: three shares of one real budget, spent in priority order

```rust
const LANE_WEAKEST_POSITION_PCT: i64 = 50;
const LANE_GEM_HUNT_PCT: i64 = 30;
const LANE_YOUTH_INVESTMENT_PCT: i64 = 20; // spent in Slice 6, not here

fn lane_cap(club_budget: i64, pct: i64) -> i64 {
    (club_budget.max(0) * pct) / 100
}
```

Lane caps are **not** separate reserved sub-ledgers — they're a cap on how much of the club's
*current* `budget` each lane's `bid_ceiling` (5.2) is allowed to draw on, computed fresh at the
moment each lane runs. Passes execute strictly in order — **weakest-position, then gem-hunt,
then youth-investment (Slice 6)** — so a club that spends in the first pass automatically has a
smaller `budget` (and therefore smaller caps) for the next pass within the same window. This
gives "fill the real gap" priority over "chase upside" without any double-booking risk, and
without needing a second ledger.

### 5.2 — Bid ceiling: budget + need + quality, one club's max willingness to pay

```rust
/// How much one club is willing to bid for one target this lane. Never exceeds the club's
/// actual (post-earlier-lane) budget, regardless of how much the need multiplier wants to add.
fn bid_ceiling(lane_budget: i64, need_mult_pct: i64, club_budget: i64) -> i64 {
    (lane_budget * need_mult_pct / 100).min(club_budget.max(0))
}

const NEED_MULT_WEAKEST_POSITION_PCT: i64 = 130; // a confirmed gap is worth overpaying 30% for
const NEED_MULT_GEM_HUNT_PCT: i64 = 110;         // opportunistic, smaller premium
```

### 5.3 — One pass, one snapshot: order-independence (a real determinism trap)

Every club in a pass computes its target (Slice 3.2 or 4.2) **against the pass's opening
state** — no club's search result depends on another club's search happening "before" or
"after" it in whatever order clubs happen to be iterated. Transfers are then resolved and
applied as one batch at the pass's end. This matters for the same reason bible §9 treats
determinism as sacred: without this rule, iterating clubs `0..NUM_CLUBS` vs. any other order
would silently produce different results from the *same seed*, breaking the "generated but
consistent" guarantee every other subsystem in this codebase relies on.

```rust
/// One full pass: every club's target (computed off the shared opening snapshot), grouped by
/// contested player, each contested player's auction resolved independently, all resulting
/// transfers applied together at the end.
fn run_transfer_pass(
    pop: &mut Population,
    world: &WorldGenesis,
    world_seed: u64,
    season: u32,
    window: u8, // 0 = winter, 1 = summer — folded into the auction seed (5.4)
    lane: TransferLane,
) {
    let elapsed_weeks = season * 52;
    let candidates = candidates_by_position(pop, elapsed_weeks);
    let gem_lists = gem_targets_by_position(pop, &candidates, elapsed_weeks);
    let squads = squads_by_club(pop, world.clubs.len());

    let mut targets: HashMap<usize /* player idx */, Vec<ClubId>> = HashMap::new();
    for club in &world.clubs {
        let cap = lane_cap(club.budget, lane.pct());
        let ceiling = bid_ceiling(cap, lane.need_mult_pct(), club.budget);
        let target = match lane {
            TransferLane::WeakestPosition => weakest_position_target(
                club.id, pop, &squads[club.id], &candidates, ceiling, elapsed_weeks,
            ),
            TransferLane::GemHunt => gem_hunt_target(
                club.id, pop, &gem_lists, ceiling, elapsed_weeks,
            ),
        };
        if let Some(idx) = target {
            targets.entry(idx).or_default().push(club.id);
        }
    }

    // Resolve each contested player independently, apply all transfers as one batch.
    let mut transfers = Vec::new();
    for (player_idx, bidder_clubs) in targets {
        let valuation = market_valuation(
            pop.current_ovr(player_idx, elapsed_weeks),
            pop.potential_ovr[player_idx],
            pop.age_years_at(player_idx, elapsed_weeks),
        );
        let bidders: Vec<(ClubId, i64)> = bidder_clubs
            .iter()
            .map(|&c| (c, bid_ceiling(lane_cap(world.clubs[c].budget, lane.pct()),
                                        lane.need_mult_pct(), world.clubs[c].budget)))
            .filter(|&(_, ceiling)| ceiling >= valuation)
            .collect();
        let mut rng = GoatRng::new(auction_seed(world_seed, season, window, pop.seed[player_idx]));
        if let Some((winner, fee)) = resolve_auction(&bidders, valuation, &mut rng) {
            transfers.push((player_idx, winner, fee));
        }
    }
    for (player_idx, winner, fee) in transfers {
        let seller = pop.club[player_idx] as usize;
        pop.club[player_idx] = winner as u16;
        // conservation: money moves within the closed club economy, never created/destroyed
        // by a transfer fee (only `total_income`, Slice 1.2, creates new money) — the fee
        // literally sums to zero across (buyer, seller), a natural invariant test (5.5).
    }
}
```

*(The mutable `world.clubs`/`Club.budget` debit/credit lines are elided above for readability —
Dev applies `world.clubs[winner].budget -= fee; world.clubs[seller].budget += fee;` inside the
final loop, after computing both indices, same batch-at-end timing.)*

### 5.4 — Auction resolution: deterministic ascending rounds, not instant highest-bidder

```rust
fn auction_seed(world_seed: u64, season: u32, window: u8, player_seed: u64) -> u64 {
    world_seed
        ^ (season as u64).rotate_left(17).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (window as u64).rotate_left(29).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ player_seed.rotate_left(41).wrapping_mul(0x1656_667B_19E3_779F)
}

const AUCTION_RAISE_PCT: i64 = 8; // price climbs 8% per contested round

/// `interested`: (club, bid_ceiling) pairs already filtered to `ceiling >= valuation`.
/// Returns `(winning_club, final_fee)`, or `None` if nobody clears the valuation.
fn resolve_auction(
    interested: &[(ClubId, i64)],
    valuation: i64,
    rng: &mut GoatRng,
) -> Option<(ClubId, i64)> {
    match interested.len() {
        0 => None,
        1 => Some((interested[0].0, valuation)), // uncontested — pays the ask, no reason to
                                                   // bid against yourself
        _ => {
            let mut price = valuation;
            let mut remaining: Vec<(ClubId, i64)> = interested.to_vec();
            loop {
                remaining.retain(|&(_, ceiling)| ceiling >= price);
                if remaining.len() <= 1 {
                    break;
                }
                price += (price * AUCTION_RAISE_PCT) / 100 + 1; // +1 guards a zero-valuation stall
            }
            if remaining.is_empty() {
                // Everyone dropped in the same round (ceilings clustered exactly below the
                // final raise) — deterministic seeded tie-break among the original bidders,
                // at the price band they all cleared one round earlier.
                let idx = rng.next_range_u32(0, interested.len() as u32 - 1) as usize;
                Some((interested[idx].0, price))
            } else {
                Some((remaining[0].0, price))
            }
        }
    }
}
```

This is exactly the "auction/bidding mechanism, not simple need-filling" and "price driven up
by competition" behavior Tùng specified: an uncontested target costs exactly its valuation; a
target two or more clubs want costs strictly more, in visible, replayable increments, with a
seeded (not order-dependent) resolution when ceilings tie.

### 5.5 — Not designed: seller-side reluctance

Every player is transferable at the right price this round — a club never refuses an offer
above `market_valuation` regardless of how much it needs that specific player. A real
"reluctance premium" for a club's own key players is a plausible future refinement, not built
here (see "Out of scope").

### TDD anchor

- `uncontested_target_pays_exactly_valuation`: one interested bidder → `fee == valuation`
  exactly.
- `contested_target_pays_strictly_more_than_valuation`: two-plus interested bidders with
  ceilings above valuation → `fee > valuation`, and `fee` increases monotonically with the
  number of competing bidders at a fixed valuation (more competition → higher price).
- `auction_result_is_order_independent`: shuffling the `interested` slice's input order before
  calling `resolve_auction` (holding the seed fixed) produces the identical `(winner, fee)` —
  the direct regression for 5.3's determinism claim.
- `auction_is_deterministic_per_seed`: same `(world_seed, season, window, player_seed)` twice →
  identical result; different `player_seed` → a different (not necessarily different-valued,
  but independently-derived) tie-break stream.
- `budget_conservation_across_a_transfer`: after `run_transfer_pass`, `buyer.budget +
  seller.budget` post-transfer equals their pre-transfer sum minus the fee's zero-sum transfer
  (i.e. `buyer_before - fee + (seller_before + fee) == buyer_before + seller_before`) — the
  "money moves, isn't created" invariant from 5.3.
- `losing_bidders_budget_unaffected`: a club that participates in a contested auction but
  doesn't win has an unchanged `budget` afterward (money only commits on winning).

## Slice 6 — Youth-academy investment lane

Distinct lever from Slices 3–5: instead of buying from another club, a club spends its
remaining window budget on its **own** academy. Composes directly with round-3 Slice 4's youth
intake (`TASK-DESIGN-round3...md` §4.2-4.6) rather than replacing it.

### 6.1 — Data shape

```rust
pub struct Club {
    // ...existing fields, plus Slice 1's `budget`...
    /// How much this club's own academy currently out-punches its genesis `strength` for
    /// intake purposes. NEW. Decays without reinvestment (6.3) — an ongoing commitment, not
    /// a one-time purchase.
    pub academy_boost: u8,
}

const ACADEMY_BOOST_MAX: u8 = 20;
```

### 6.2 — Investment: diminishing returns

```rust
fn apply_academy_investment(club: &mut Club, spend: i64) {
    if spend <= 0 {
        return;
    }
    // Gets pricier per point the higher the existing boost — early investment is cheap,
    // pushing a boosted academy even further gets progressively harder.
    let cost_per_point = 1_000 + (club.academy_boost as i64) * 300;
    let gained = (spend / cost_per_point.max(1)) as u8;
    club.academy_boost = club.academy_boost.saturating_add(gained).min(ACADEMY_BOOST_MAX);
    club.budget -= spend; // real spend, not a loan
}
```

A club's youth-investment lane spend this window is simply `lane_cap(club.budget,
LANE_YOUTH_INVESTMENT_PCT)` (Slice 5.1) — no target search needed, the whole lane amount goes
straight into `apply_academy_investment` if the club chooses to invest (see 6.4 for the
choice-gating).

### 6.3 — Decay: sustained investment required

```rust
const ACADEMY_BOOST_DECAY_PCT: i64 = 15; // per season, not per window

fn decay_academy_boost(club: &mut Club) {
    club.academy_boost = ((club.academy_boost as i64 * (100 - ACADEMY_BOOST_DECAY_PCT)) / 100) as u8;
}
```

Called once per season (Slice 9's wiring), after both windows' investment for that season has
landed — so a club gets up to two investment opportunities per season before any decay bites.

### 6.4 — Hook into round-3's existing intake formula: one call-site change

Round-3 Slice 4's `apply_youth_intake` rolls each new player's `potential_ovr` via
`roll_potential_ovr(rng, club.strength)` (round-3 §2.1/§4.6). This slice's only ripple into
that existing code is the anchor value passed in:

```rust
// Round-3's existing call site, minimally changed:
let effective_strength = club.strength.saturating_add(club.academy_boost).min(99);
let potential_ovr = roll_potential_ovr(&mut rng, effective_strength); // was: club.strength
```

No change to `roll_potential_ovr`'s own signature, the outlier-roll mechanism, the intake-count
formula (round-3 §4.2/§4.3), or the `intake_week` column — this is exactly the "compose without
redesigning" instruction, a one-line anchor substitution at the one call site round-3 already
built.

**Whether to invest at all, this round:** a simple always-invest-the-full-lane-cap policy (no
club ever leaves this lane's money on the table) — Design's simplification, flagged below,
since Tùng's brief didn't specify a club-level "prefer buying over academy investment" decision
rule and a fixed policy is the smallest concrete choice that satisfies "clubs invest in their
own youth academy" without inventing a whole club-personality system.

### TDD anchor

- `academy_investment_has_diminishing_returns`: equal `spend` at `academy_boost = 0` vs.
  `academy_boost = 10` gains fewer points at the higher starting boost.
- `academy_boost_never_exceeds_cap`: repeated large investments still clamp at
  `ACADEMY_BOOST_MAX`.
- `academy_boost_decays_without_reinvestment`: a club with `academy_boost > 0` and zero spend
  for several seasons trends toward `0`.
- `academy_boost_raises_effective_intake_anchor`: round-3's `youth_intake_uses_shared_outlier_
  formula`-style test, re-run with a nonzero `academy_boost`, shows a higher mean
  `potential_ovr` among that club's intake vs. an identical club with `academy_boost = 0` —
  the end-to-end regression tying this slice to round-3's existing mechanic.

## Slice 7 — Manager entity, appointment, and tactical-identity shift

### 7.1 — Why a new lightweight entity, not a reuse of `Club` or a player

Managers aren't players (no attributes, no position, no `potential_ovr`, no retirement-by-age)
and aren't part of `Club`'s own identity (a club survives a managerial change; `strength`/
`tactical_identity`/`budget` all persist across a firing). A small, separate, non-SoA struct —
same weight class as `Club` itself (a handful of fields, ~1,200 of them, no per-attribute
columns) — is the natural fit; `Population`'s SoA discipline doesn't apply here because the
count is two orders of magnitude smaller and nothing about a manager needs per-attribute
storage.

```rust
pub type ManagerId = u32;

pub struct Manager {
    pub id: ManagerId,
    pub name: String, // reuses history::name_from_seed, "Verified" above
    /// The manager's own natural tactical bias — same type Doc B already built, reused
    /// verbatim, not extended.
    pub identity_bias: TacticalIdentity,
    /// Rolling points-per-match ring buffer (3/1/0), most recent overwrites oldest. Used by
    /// Slice 8's firing trigger.
    pub recent_points: [u8; MANAGER_FORM_WINDOW],
    pub recent_idx: u8,
    pub tenure_start_season: u32,
}

const MANAGER_FORM_WINDOW: usize = 10; // rolling 10-match form window, flagged in "Decisions"

/// The whole manager population: one per club, plus a reserve pool. A closed system — total
/// manager count is fixed at genesis (`NUM_CLUBS + MANAGER_POOL_SIZE`); firing returns a
/// manager to `free_agents`, hiring draws one out. Never grows, never runs out.
pub struct ManagerPool {
    pub managers: Vec<Manager>,       // index = ManagerId as usize
    pub club_manager: Vec<ManagerId>, // per-club current manager, index = ClubId
    pub free_agents: Vec<ManagerId>,  // currently unemployed, available to hire
}

const MANAGER_POOL_SIZE: usize = NUM_CLUBS / 4; // 300 — flagged in "Decisions"
```

Threaded through `ReplayCache` next to `pop` (`promotion.rs`'s existing `ReplayCache` struct
gains a `managers: ManagerPool` field), same "new mutable, persisted, replay-advanced state"
treatment `pop` itself already gets.

### 7.2 — Generation: seeded, deterministic, following existing local-helper precedent

```rust
fn manager_seed(world_seed: u64, m: usize) -> u64 {
    world_seed
        ^ 0xD3u64.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (m as u64).rotate_left(23).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
}

impl Manager {
    fn generate(world_seed: u64, id: ManagerId) -> Self {
        let seed = manager_seed(world_seed, id as usize);
        Manager {
            id,
            name: history::name_from_seed(seed),
            identity_bias: TacticalIdentity::generate(seed ^ 1), // distinct sub-seed from the
                                                                  // name draw, same "^1" idiom
                                                                  // tactical_identity.rs's own
                                                                  // callers already use for
                                                                  // sibling-but-distinct rolls
            recent_points: [0; MANAGER_FORM_WINDOW],
            recent_idx: 0,
            tenure_start_season: 0,
        }
    }
}

impl ManagerPool {
    pub fn genesis(world_seed: u64, world: &WorldGenesis) -> Self {
        let total = world.clubs.len() + MANAGER_POOL_SIZE;
        let managers: Vec<Manager> = (0..total)
            .map(|m| Manager::generate(world_seed, m as ManagerId))
            .collect();
        let club_manager: Vec<ManagerId> = (0..world.clubs.len() as ManagerId).collect();
        let free_agents: Vec<ManagerId> =
            (world.clubs.len() as ManagerId..total as ManagerId).collect();
        Self { managers, club_manager, free_agents }
    }
}
```

This mirrors `world.rs`'s own local `seed_mix`-style helper rather than importing it (that
function is module-private to `world.rs`; `population.rs` already keeps its own separate copy
of the same idiom rather than exporting a shared one — existing precedent, not a new pattern).
Salt `0xD3` is Design's own next-unused-byte guess after `nation_seed`'s `0xA1`/`club_seed`'s
`0xB2` (`world.rs:146-152`) — Dev should grep for any other salt bytes in use before locking
this in, since Design did not exhaustively audit every salt constant in the codebase.

### 7.3 — Appointment: a blend, not a hard overwrite

```rust
const MANAGER_IDENTITY_BLEND_PCT: i64 = 700; // new manager's weight, out of 1000 (Fixed scale)

/// Called whenever a club gets a new manager (initial genesis assignment excluded — genesis
/// managers and genesis clubs start with independently-generated identities, no blend needed
/// at t=0). Shifts `club.tactical_identity` toward the incoming manager's own bias rather
/// than replacing it outright — "shift," per Tùng's own word, not "reset."
fn apply_manager_identity_shift(club: &mut Club, manager: &Manager) {
    for r in 0..NUM_ROLES {
        let old = club.tactical_identity.role_weight[r].to_raw() as i64;
        let new = manager.identity_bias.role_weight[r].to_raw() as i64;
        let blended = (old * (1000 - MANAGER_IDENTITY_BLEND_PCT) + new * MANAGER_IDENTITY_BLEND_PCT) / 1000;
        club.tactical_identity.role_weight[r] = Fixed::raw(blended as i32);
    }
    // Re-normalize so the weights still sum to NUM_ROLES * 1.0, the invariant
    // TacticalIdentity::generate itself maintains (tactical_identity.rs:45-49) — a blend of
    // two already-normalized vectors can drift slightly from integer rounding; Dev re-applies
    // the same sum-and-rescale step `generate` uses, not a new normalization routine.
}
```

70/30 (new manager dominant, old identity as inertia) is Design's own split — flagged below.

### TDD anchor

- `manager_genesis_pool_is_closed`: `managers.len() == club_manager.len() + free_agents.len()`
  always, immediately after genesis and after any sequence of fire/hire operations (Slice 8) —
  the "never grows, never runs out" invariant.
- `manager_generation_is_deterministic`: same `(world_seed, id)` twice → identical `Manager`
  (name, `identity_bias`).
- `identity_shift_moves_toward_manager_not_past_it`: for any starting `club.tactical_identity`
  and any `manager.identity_bias`, every post-shift `role_weight[r]` lies strictly between the
  pre-shift value and the manager's value (never overshoots) — the "blend not override" bound.
- `identity_shift_preserves_the_sum_invariant`: post-shift `role_weight` sums to
  `NUM_ROLES * 1.0` within the same integer-rounding slack `identity_weights_sum_to_average_one`
  (`tactical_identity.rs:106-116`) already tolerates.

## Slice 8 — Manager performance tracking, firing trigger, pool-based rehire

### 8.1 — Capturing per-match results: a zero-ripple sibling to `batch_tick_season`

Same pattern round-3 Slice 5 used for `generate_player`/`generate_player_biased`
(`TASK-DESIGN-round3...md` §5.5) — the existing function keeps its exact signature and output
for every current caller; a new sibling carries the extra data this slice needs.

```rust
pub fn batch_tick_season(
    pop: &mut Population, world: &WorldGenesis, league_clubs: &[Vec<ClubId>],
    world_seed: u64, season: u32, elapsed_weeks: u32,
) -> (Vec<SeasonResult>, Vec<Table>) {
    let (results, tables, _match_points) =
        batch_tick_season_with_match_points(pop, world, league_clubs, world_seed, season, elapsed_weeks);
    (results, tables)
}

/// Identical body to today's `batch_tick_season`, plus: inside the existing per-round loop
/// (`batch_tick.rs:93-97`), immediately after each `table.apply_result(...)`, push this
/// match's points for both sides into the returned `Vec`. No other line changes.
pub fn batch_tick_season_with_match_points(
    pop: &mut Population, world: &WorldGenesis, league_clubs: &[Vec<ClubId>],
    world_seed: u64, season: u32, elapsed_weeks: u32,
) -> (Vec<SeasonResult>, Vec<Table>, Vec<(ClubId, u8)>) {
    // ...identical to today's body...
    let mut match_points = Vec::new();
    // inside the round loop:
    //   let (gf, ga) = sim_team_match(...);
    //   table.apply_result(f.home, f.away, gf, ga);
    //   let (home_pts, away_pts) = points_from_result(gf, ga); // 3/1/0, home/away
    //   match_points.push((f.home, home_pts));
    //   match_points.push((f.away, away_pts));
    // ...rest identical...
    (results, tables, match_points)
}

fn points_from_result(gf: u32, ga: u32) -> (u8, u8) {
    match gf.cmp(&ga) {
        std::cmp::Ordering::Greater => (3, 0),
        std::cmp::Ordering::Equal => (1, 1),
        std::cmp::Ordering::Less => (0, 3),
    }
}
```

`ReplayCache::advance_one_season` (Slice 9) switches to the `_with_match_points` variant and
feeds the result into `ManagerPool`; every other existing caller/test of `batch_tick_season`
(there are several across `goat-world`'s test suite) is byte-identical, zero test churn outside
the two new tests below.

### 8.2 — Updating manager form: ring-buffer push per match

```rust
impl ManagerPool {
    pub fn record_match_points(&mut self, match_points: &[(ClubId, u8)]) {
        for &(club_id, pts) in match_points {
            let mgr_id = self.club_manager[club_id];
            let mgr = &mut self.managers[mgr_id as usize];
            let idx = mgr.recent_idx as usize;
            mgr.recent_points[idx] = pts;
            mgr.recent_idx = ((idx + 1) % MANAGER_FORM_WINDOW) as u8;
        }
    }
}
```

The ring buffer is **not** reset at season boundaries — it persists across the season-tick the
same way `Manager` itself does, so a manager's form genuinely spans a rolling 10-match window
regardless of where in the season it falls (matches real "fired mid-slump," not "only
evaluated exactly at year-end").

### 8.3 — Firing trigger: rolling actual points-per-game vs. expected, by strength

```rust
/// Expected points-per-game for a club of this strength, on the standard 3/1/0 scale.
/// Linear: strength 50 (dead average) → 1.5 ppg (a genuine mid-table rate); strength 99 →
/// ~2.24 ppg; strength 1 → ~0.76 ppg. Coefficients are Design's own fit, not derived from any
/// real points-per-strength data (none exists in this codebase) — flagged below.
fn expected_ppg(strength: u8) -> Fixed {
    Fixed::raw(1_500 + (strength as i32 - 50) * 15)
}

fn actual_ppg(recent_points: &[u8; MANAGER_FORM_WINDOW]) -> Fixed {
    let sum: i32 = recent_points.iter().map(|&p| p as i32).sum();
    Fixed::raw(sum * 1_000 / MANAGER_FORM_WINDOW as i32)
}

const FIRING_UNDERPERFORMANCE_PCT: i64 = 70; // actual ppg below 70% of expected → fired

fn should_fire(manager: &Manager, strength: u8, season: u32) -> bool {
    // Grace period: a manager needs a full rolling window of real results before being
    // eligible for dismissal — naturally satisfied once `tenure_start_season` is at least
    // one full window old; Dev enforces this by only calling `should_fire` once the ring
    // buffer has actually cycled once (a `matches_played: u16` counter is the simplest
    // concrete way to track that — Design did not add a field for it above; flagged below).
    let actual = actual_ppg(&manager.recent_points);
    let threshold = expected_ppg(strength) * Fixed::raw(FIRING_UNDERPERFORMANCE_PCT * 10);
    actual < threshold
}
```

**A real gap Design found while writing this**: `should_fire` as specified needs to know
whether the ring buffer has actually filled once (otherwise a brand-new manager with 9 zeroed
slots and 1 real bad result reads as catastrophic underperformance from partial data). The
struct in Slice 7.1 does not yet have a `matches_played` counter to gate this — **flagged
explicitly in "Decisions" as a field Dev must add**, not silently patched over here, since
adding it changes `Manager`'s shape (and therefore the save format) beyond what Slice 7.1
already specified.

### 8.4 — Replacement: draw from the free-agent pool, deterministic

```rust
fn hire_replacement(pool: &mut ManagerPool, club: &mut Club, club_id: ClubId, world_seed: u64, season: u32) {
    let fired = pool.club_manager[club_id];
    pool.free_agents.push(fired);
    pool.managers[fired as usize].recent_points = [0; MANAGER_FORM_WINDOW]; // fresh start
    pool.managers[fired as usize].recent_idx = 0;

    let seed = manager_seed(world_seed, 0xE4) ^ (club_id as u64) ^ (season as u64);
    let mut rng = GoatRng::new(seed);
    let draw = rng.next_range_u32(0, pool.free_agents.len() as u32 - 1) as usize;
    let hired = pool.free_agents.remove(draw);

    pool.club_manager[club_id] = hired;
    pool.managers[hired as usize].tenure_start_season = season;
    apply_manager_identity_shift(club, &pool.managers[hired as usize]); // Slice 7.3
}
```

The fired manager going back into `free_agents` (rather than being discarded) is what keeps
`ManagerPool` closed (7.1's invariant) — a sacked manager is available to be hired elsewhere
later, same seed-derived draw mechanism, matching real-world managerial merry-go-rounds without
needing to model reputation/preference on the hiring side (every free agent is equally likely
to be drawn — Design's own simplification, flagged below).

### TDD anchor

- `match_points_sum_matches_table_points`: `record_match_points`'s total across a full season
  equals the same season's `Table`'s own points column, cross-checked against the existing
  `Table::apply_result` logic — confirms the sibling function's per-match capture is faithful
  to the aggregate it's derived alongside.
- `should_fire_true_for_sustained_underperformance`: a manager whose `recent_points` are all
  `0` at a `strength = 80` club (expected ~1.95 ppg) returns `true`.
- `should_fire_false_for_matching_or_exceeding_expectation`: a manager whose `recent_points`
  average at/above `expected_ppg` for their club's strength returns `false`, even at strength
  extremes (1 and 99).
- `hire_replacement_keeps_pool_closed`: after any fire/hire cycle,
  `managers.len() == club_manager.len() + free_agents.len()` still holds (extends 7's
  invariant test to the fire/hire path specifically).
- `hire_replacement_resets_form_but_keeps_identity`: the rehired-later fired manager's
  `recent_points` are zeroed at their new club, but `identity_bias` is byte-identical to what
  it was before firing.
- `identity_shift_applies_on_every_hire_not_just_genesis`: a club's `tactical_identity` after
  `hire_replacement` differs from its pre-hire value whenever the new manager's `identity_bias`
  differs from the old one.

## Slice 9 — Season-tick wiring: where this all hooks into `advance_one_season`

```rust
pub fn advance_one_season(&mut self, world: &mut WorldGenesis) -> Vec<PromoRelegationEvent> {
    let season = self.resolved_through + 1;
    let elapsed_weeks = season * 52;

    // 1. Winter window: budgets top up, then both buy-lanes run, then youth investment.
    for club in &mut world.clubs {
        open_transfer_window(club, &self.pop, &squad_of(&self.pop, club.id), club.tier(world), elapsed_weeks);
    }
    run_transfer_pass(&mut self.pop, world, self.world_seed, season, 0, TransferLane::WeakestPosition);
    run_transfer_pass(&mut self.pop, world, self.world_seed, season, 0, TransferLane::GemHunt);
    run_academy_investment_pass(&mut self.pop, world, self.world_seed, season, 0); // Slice 6

    // 2. The season's matches — now captures per-match points for manager form (Slice 8.1).
    let (_results, tables, match_points) = batch_tick_season_with_match_points(
        &mut self.pop, world, &self.membership, self.world_seed, season, elapsed_weeks,
    );
    self.managers.record_match_points(&match_points);

    // 3. Summer window: same three passes again, off the post-season-matches budget state.
    for club in &mut world.clubs {
        open_transfer_window(club, &self.pop, &squad_of(&self.pop, club.id), club.tier(world), elapsed_weeks);
    }
    run_transfer_pass(&mut self.pop, world, self.world_seed, season, 1, TransferLane::WeakestPosition);
    run_transfer_pass(&mut self.pop, world, self.world_seed, season, 1, TransferLane::GemHunt);
    run_academy_investment_pass(&mut self.pop, world, self.world_seed, season, 1);
    for club in &mut world.clubs {
        decay_academy_boost(club); // once per season, after both windows (Slice 6.3)
    }

    // 4. Manager evaluation — after a full season of form data, before next season's roster
    //    churn (round-3 Slice 4's youth intake) so a freshly-fired club's replacement manager
    //    is in place before intake reads `club.tactical_identity` at all (it doesn't today,
    //    but this ordering is future-proof against that changing).
    for club_id in 0..world.clubs.len() {
        let mgr_id = self.managers.club_manager[club_id];
        if should_fire(&self.managers.managers[mgr_id as usize], world.clubs[club_id].strength, season) {
            hire_replacement(&mut self.managers, &mut world.clubs[club_id], club_id, self.world_seed, season);
        }
    }

    // 5. Existing round-3/round-2 machinery, untouched.
    apply_youth_intake(&mut self.pop, world, self.world_seed, season); // round-3 §4.6
    let events = apply_season_end(world, &mut self.membership, season, &tables);
    self.resolved_through = season;
    events
}
```

**Why this exact ordering, spelled out**: budgets must top up before any spending pass reads
them (1 before the buy-lanes); the season's matches must run before manager form can be scored
off them (2 before 4); both transfer windows should bracket the season's matches, not both run
back-to-back before any match is played, so a club's summer-window spending reflects what its
squad actually did that season, not just its winter-window shape (1 and 3 on either side of 2);
manager evaluation happens once per season, after the summer window (not per-window), since
firing a manager mid-transfer-window would create an ordering dependency between two systems
this doc otherwise keeps independent. This whole ordering is **Design's own construction** —
Tùng specified the pieces (two windows, budget, auction, manager pressure) but not their
relative sequencing within one season-tick — flagged below.

`world.clubs` gains `&mut` access it didn't need before (budgets/academy_boost/
tactical_identity all mutate now) — `WorldGenesis` today is treated as effectively immutable
after genesis by every existing caller (`ReplayCache::advance_one_season`'s current signature
takes `&WorldGenesis`, not `&mut`); this is a real, load-bearing signature change to
`ReplayCache::advance_one_season` and every caller of it (`promotion.rs`'s own tests, and
whatever orbit-path code the round-3 Slice 3.4 migration note anticipated) — flagged below as
a wider-than-usual ripple for Dev to scope carefully, not a small change.

### TDD anchor

- `full_season_tick_is_deterministic`: two identical `(world_seed, season)` runs through
  `advance_one_season` produce byte-identical `world.clubs` (`budget`, `academy_boost`,
  `tactical_identity`), `self.pop.club` assignments, and `self.managers` state — the
  end-to-end regression tying every slice above together.
- `total_system_budget_change_equals_total_income_minus_total_wages`: summing every club's
  `budget` delta across one full season-tick (both windows) equals the sum of every club's
  `total_income` minus `window_wage_deduction` minus academy-investment spend — transfer fees
  net to zero system-wide (5.5's conservation invariant, now checked at the whole-season
  scale, not just per-transfer).
- `manager_firing_reflects_the_full_season_not_just_one_window`: a manager whose form is bad
  in the winter half but recovers by the summer window is evaluated once, at season-end, off
  the full rolling window — not fired mid-season by this doc's own machinery (there is no
  mid-season fire path; confirms 4's "once per season" claim).

## Out of scope (do not fold into this doc)

- **PC-facing transfer-market participation** — the PC's own club is not yet a bidder in this
  market. Bible §7.3's "your teammates arrive and leave" flavor for the PC specifically is a
  natural, compatible future extension (the PC's club could simply be one more club iterated
  in Slices 3–5's passes) but is not wired here — Phase 8 (`state.rs`) stays exactly as-is.
- **Seller-side reluctance / squad-retention logic** — every player transfers at the right
  price this round (Slice 5.5). A club refusing to sell its own most-important player even for
  a huge offer is a real, plausible future refinement, not designed here.
- **Per-player wage contracts for AI players** — wages stay an abstracted per-window deduction
  (Slice 1.4), not individual contract data. Adding real per-player wage negotiation for AI
  squads (separate from the PC's own Phase 8 wage negotiation) is future scope.
- **Sponsorship, matchday/ticket sales, shirt sales, prize-money income contributors** —
  Slice 1.2's `total_income` is explicitly designed to accept these later without a rewrite,
  but none of them are built in this round; today `tier_baseline_income` is the only term.
- **Manager reputation/preference-based hiring** — every free agent is equally likely to be
  drawn (Slice 8.4); no "bigger clubs attract better/more famous managers" weighting.
- **Sub-season (day-level) transfer window integration with the orbit calendar** — this doc's
  two windows are season-tick passes (Slice 9), not literal calendar-day events for background
  clubs; wiring the *orbit* (PC-facing) calendar's already-existing `TransferWinter`/
  `TransferSummer` flashpoints (`calendar_loop.rs`) to actually open/close this market in
  lockstep with day-level time is a future integration, not designed here.
- **Round-4 competition-result feedback into finances or manager pressure** — this doc notes
  two composition points (below) but does not design them: (1) `total_income`'s future
  `prize_money_income` contributor naturally reads round-4's `Competition`/results machinery
  once that lands; (2) `expected_ppg`'s "expected performance for that club's tier/strength"
  could, in a future round, be raised for a club in continental-qualification or
  promotion-relegation contention (round-4's `FixtureImportance`, round-2's `PROMO_RELEGATION_
  N = 3`) rather than uniform-by-strength as it is today. Neither is built here — flagged as
  interaction points, not redesigns, per the brief's explicit instruction not to touch
  round-2/3/4.
- **Round-2/round-3/round-4 decisions themselves** — not re-litigated. This doc reads
  `Club.strength`, `TacticalIdentity`, `PROMO_RELEGATION_N`, and the youth-intake formula as
  fixed inputs.

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
4. **Slice 3.2**: `TARGET_SEARCH_PREFIX = 50` — a perf/quality tradeoff bound, not measured
   against a real `--release` benchmark this round (same caveat round-2's A2.5/A3.1 items
   flagged for their own perf numbers).
5. **Slice 4.1**: `GEM_HUNT_MAX_AGE = 21` — Design's own cutoff, matching round-3's own choice
   to treat teenage-to-early-20s as the "prospect" band implicitly (round-3's genesis age range
   is `16..33`, `population.rs:129`).
6. **Slice 5.1**: the `50/30/20` lane split and pass ordering (weakest-position first) — Design's
   instantiation of "balanced against a single finite budget," matching Tùng's stated priority
   ("prioritize buying a replacement" for the weakest position) but the exact percentages are
   Design's pick.
7. **Slice 5.2**: `NEED_MULT_WEAKEST_POSITION_PCT = 130`, `NEED_MULT_GEM_HUNT_PCT = 110` —
   Design's own overpay-willingness numbers.
8. **Slice 5.4**: the entire ascending-round auction mechanism — `AUCTION_RAISE_PCT = 8`, the
   round structure, and the RNG-seeded tie-break rule — is Design's concrete answer to Tùng's
   explicit "you decide... consider a bidding-round structure" instruction. The *shape*
   (ascending rounds, not sealed-bid, not instant-highest-bidder) is a real design choice worth
   Tùng's sign-off before Dev locks it into golden tests, not just the numbers.
9. **Slice 6**: `ACADEMY_BOOST_MAX = 20`, the `1,000 + boost×300` diminishing-returns cost
   curve, and `ACADEMY_BOOST_DECAY_PCT = 15` — Design's own numbers for a lever Tùng specified
   only at the concept level ("invest in own youth academy").
10. **Slice 6.4**: the "always invest the full youth-investment lane cap" policy — a
    simplification Design chose over building a per-club "buy vs. develop" preference system,
    flagged explicitly in Slice 6.4's own text.
11. **Slice 7.1**: `MANAGER_POOL_SIZE = 300` (`NUM_CLUBS / 4`) and `MANAGER_FORM_WINDOW = 10`
    — both Design's own picks; Tùng's brief left pool size and window length as open "you
    decide" items explicitly.
12. **Slice 7.2**: salt byte `0xD3` for `manager_seed` — Design's own next-unused-byte guess,
    **not exhaustively verified against every salt constant in the codebase** (only `world.rs`'s
    two were checked). Dev must grep before landing this.
13. **Slice 7.3**: `MANAGER_IDENTITY_BLEND_PCT = 700` (70% new manager / 30% club legacy) — a
    concrete answer to "shift the tactical identity," Design's own split.
14. **Slice 8.3**: the entire `expected_ppg`/`should_fire` firing trigger — `expected_ppg`'s
    linear coefficients, and `FIRING_UNDERPERFORMANCE_PCT = 70` — is Design's concrete
    instantiation of Tùng's explicit "you decide and justify a concrete formula" instruction.
    **Also flagging a genuine gap found while writing this slice**: `Manager` (Slice 7.1) needs
    an added `matches_played: u16` (or equivalent) field to gate `should_fire` against a
    not-yet-full ring buffer — Design surfaced this mid-design rather than silently patching
    Slice 7.1's struct after the fact; Dev should add it to Slice 7.1's shape before
    implementing, not treat it as a Slice 8-only concern.
15. **Slice 9**: the entire season-tick ordering (windows bracket the season's matches;
    manager evaluation once, after both windows) and the `ReplayCache::advance_one_season`
    signature change from `&WorldGenesis` to `&mut WorldGenesis` — a real, wider-than-usual
    ripple Design flagged explicitly rather than glossing over; every existing caller of
    `advance_one_season` needs updating, not just this doc's own new code.

## Definition of done (once Dev implements)

1. `cargo test --workspace` green, including every TDD-anchor test listed per slice above.
2. `goat-save::save::VERSION` bumped past `10`, with the new persisted fields
   (`Club.budget`, `Club.academy_boost`, `ManagerPool`'s three fields, `Manager.matches_played`
   per item 14 above) serialized/deserialized and round-tripped
   (`crates/goat-save/tests/save_roundtrip.rs` extended, mirroring how round-3 flagged
   `intake_week` for the same treatment).
3. `ReplayCache::advance_one_season`'s signature change (`&WorldGenesis` → `&mut WorldGenesis`,
   Slice 9) is propagated to every existing caller — `promotion.rs`'s own tests at minimum;
   Dev greps for all call sites before landing, since this doc did not enumerate every one.
4. A `--release` benchmark of one full season-tick (both windows + both transfer lanes + youth
   investment + manager evaluation, across all 1,200 clubs) is taken once implemented, to
   validate Slice 3.2's `TARGET_SEARCH_PREFIX = 50` bound and the general O(population)
   per-window cost this doc's "Verified" section reasoned about but did not measure —
   unmeasured/growing per-season cost is exactly the same category of open item round-2's A2.5
   flagged for genesis/replay time.
5. At least one integration test plays a fixed seed through several seasons and asserts:
   (a) at least one club's manager is fired and replaced (exercises the full Slice 8 path
   end-to-end, not just unit-level `should_fire`); (b) at least one contested auction resolves
   with `fee > valuation` (exercises Slice 5's competitive-bidding claim, not just the
   uncontested path); (c) at least one gem-hunting target is a round-3-outlier-style player
   (ties Slice 4 back to round-3 Slice 2's mechanic, the same style of cross-slice integration
   assertion round-3's own Definition of Done §5 used).
