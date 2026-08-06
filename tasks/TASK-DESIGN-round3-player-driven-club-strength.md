# TASK DESIGN ROUND 3 — Player-driven dynamic club strength

Prereq: none for slices 1–4 (`goat-world`, `goat-core::generation`, self-contained). Slice 5
needs `TASK-DESIGN-round2-national-team-tactical-identity.md` (Doc B) — specifically its
`TacticalIdentity`/`role_weight` type — which is **already implemented on disk** as of this
writing (`crates/goat-core/src/tactical_identity.rs`, `Club.tactical_identity` in
`crates/goat-world/src/world.rs:64-66`), being built tonight (2026-07-22) by a separate Dev
round alongside `TASK-DESIGN-round2-world-genesis-scaleup.md` (Doc A). Slice 5 reuses that
type's existing public field; it does not add to or change its shape.

This doc supersedes the one-paragraph placeholder previously parked in
`TASK-DESIGN-round2-world-genesis-scaleup.md`'s "Parked for a future design round" section
(now replaced with a pointer to this doc — see that file's diff alongside this one).

Read first: `crates/goat-world/src/population.rs` (SoA background population, genesis,
lazy-promote — this doc's main surface), `crates/goat-core/src/generation.rs`
(`generate_player`/`roll_potentials` — the PC's full per-attribute generation, the hook point
for Slice 5), `crates/goat-world/src/world.rs` (`Club`, `WorldGenesis::generate` — genesis-time
club data), `crates/goat-world/src/batch_tick.rs` (season simulation — already has a live-
strength mechanism, see Slice 3), `crates/goat-core/src/tactical_identity.rs` and
`tasks/TASK-DESIGN-round2-national-team-tactical-identity.md` (Doc B — the sibling spec Slice
5 coordinates with).

## Origin

This round's design was worked out interactively with Tùng on 2026-07-22, in a dedicated
follow-up conversation to the round-2 approval. The five decisions below are **final** —
recorded here as a proper spec (verified against real code), not re-litigated as options.

## Ground rules for this doc

- **`Club.strength` (the genesis-time constant, `world.rs:63`) stays exactly as-is** — it
  still anchors what gets generated at genesis and at youth-intake time (Slice 2, Slice 4).
  This is explicitly **Option A** from Tùng's review: "keep the anchor, add a rare outlier."
  **Option B (compute `strength` itself from the roster, discard the static anchor) was
  explicitly rejected** — it would strip clubs of their structural identity (a historically
  "big" club stops meaning anything the moment its squad ages out) and its payoff depends on
  an AI-run transfer market that doesn't exist yet (backlog item #3, out of scope here — see
  "Out of scope" below).
- **`squad_size` moves from a single global constant to a per-club generated field.** No
  single number is "the" squad size any more; every club gets its own, correlated with (but
  not identical to) its `strength`.
- **No new persisted per-player-per-team "chemistry" or training state.** Slice 5's tactical
  bias is a genesis/lazy-promote-time *generation* input, not a trained value — mirrors Doc
  B's own "nothing new trains" resolution (Doc B §B.1) so the two docs don't disagree about
  whether tactical fit is static or dynamic. It is static.
- **No change to `PlayerStore`'s SoA columns or to `CreationChoices`/`generate_player`'s
  existing signature and behaviour for player-creation callers.** Slice 5 adds a new sibling
  entry point used only by `Population::promote`; every existing call site (PC creation, ~20
  of them across `goat-core`/`goat-tui`/tests) is untouched, byte-identical output.
- **Departure channel stays limited to retirement this round.** Contract expiry and AI-club-
  to-AI-club transfers (backlog item #3, "AI-run club economy") are **not** built here. Slice
  4's youth-intake mechanic is designed to compose cleanly with item #3 once it ships (adding
  a transfer-departure channel later should not require reworking the intake roll), but does
  not need it to function today.
- **Struct-of-arrays discipline holds.** No per-club/per-player struct grows into a full
  per-attribute-potential store at genesis scale (bible §9). Slice 5's full per-attribute
  generation only happens at lazy-promote time, on the same "cheap identity, full realization
  on contact" principle `population.rs:1-9`'s module doc already states and `Population::
  promote` (`population.rs:204-229`) already implements.

## Verified: current mechanics (read in full before this round's design)

**Squad size is one global constant today.** `population.rs:26`: `pub const SQUAD_SIZE: usize
= 25`. `genesis()` (`population.rs:118-150`) loops `for slot in 0..SQUAD_SIZE` identically for
every club — a 1-star club and a 99-star club both get exactly 25 players. Total headcount
(`population.rs:23-25`'s own comment): `world.clubs.len() * SQUAD_SIZE` = 1,200 × 25 =
30,000 (`NUM_CLUBS = 1,200` per `world.rs:100`, from Doc A's 20 nations × 3 tiers × 20 clubs).

**The genesis anchor formula and its real ceiling for a weak club.** `population.rs:132-135`:

```rust
let base = club.strength as i32;
let variance = rng.next_range_u32(0, 30) as i32 - 15;   // [-15, +15]
let potential_ovr = (base + variance).clamp(30, 99) as u8;
```

`club.strength` itself is generated in `world.rs:242` as `(tier_base - rank_decay +
noise).clamp(1, 99)`, so the weakest possible club has `strength = 1`. Plugging the extremes
into the anchor formula: `base = 1`, max `variance = +15` → `1 + 15 = 16`, clamped to `[30,
99]` → **30**. This holds for *any* club with `strength ≤ 14`: `base + 15 ≤ 29 < 30`, so the
clamp floor of 30 is the formula's output regardless of the exact (sub-15) strength value —
**every club that weak produces an identical, flat potential-30 ceiling, every single time,
for every player in its squad.** There is no roll, no matter how many players or how many
world seeds, that lets one of these clubs produce a genuine standout under the current
formula. This is the concrete gap Slice 2's outlier roll closes.

**Live club strength for match simulation already exists — for the non-orbit (background
league) path.** This was not built tonight; it's pre-existing `TASK-09A` work (`batch_tick.rs`
module doc, `git log` shows it landed in commit `2715398`, well before this round). `batch_tick.rs:52-62`:

```rust
fn club_strength(pop: &Population, squad: &[usize], elapsed_weeks: u32) -> u8 {
    if squad.is_empty() { return 1; }
    let sum: u32 = squad.iter().map(|&i| pop.current_ovr(i, elapsed_weeks) as u32).sum();
    (sum / squad.len() as u32).clamp(1, 99) as u8
}
```

`batch_tick_season` (`batch_tick.rs:73-167`) computes this per-club **live** strength (mean
current OVR of the current, non-retired-filtered-at-use-site squad) once per season and feeds
it into every match (`sim_team_match(strengths[f.home], strengths[f.away], ...)`,
`batch_tick.rs:95`) — `club.strength` (the static genesis constant) is **not** read anywhere
in this path. `promotion.rs`'s `ReplayCache::advance_one_season` (`promotion.rs:137-150`)
calls `batch_tick_season` directly, so promotion/relegation (Doc A Slice A3) already resolves
off live strength too. **Slice 3 below is therefore much smaller than it originally sounded**
— see Slice 3 for exactly what's left.

**`Population::promote` is the existing lazy-promote mechanic Tùng's item 5 asked to find.**
`population.rs:204-229`: given a background-population index and a date, builds
`CreationChoices` from the stored cheap identity columns (position, nationality, club) and
calls `generate_player(self.seed[idx], &choices)` — the same full per-attribute pipeline the
PC's own creation uses — then overwrites `current` to the age-appropriate fraction of the
freshly-rolled `potential`. This is the "on contact" realization point bible §245 and the
module doc (`population.rs:1-9`) describe: background players carry only `potential_ovr:
Vec<u8>` (`population.rs:44`) until this function runs. As of this reading there are **no
non-test call sites yet** (`grep` across the workspace) — the trigger points ("you face him,"
"a transfer links him") aren't wired up outside `population.rs`'s own tests. That wiring is
not this doc's job; Slice 5 only changes what `promote` *produces*, not when it's called.

**`TacticalIdentity` already exists (Doc B, in flight tonight).**
`crates/goat-core/src/tactical_identity.rs:22-26`:

```rust
pub struct TacticalIdentity {
    pub role_weight: [Fixed; NUM_ROLES], // NUM_ROLES = 14; average weight 1.0
}
```

`TacticalIdentity::generate(seed)` (`tactical_identity.rs:33-51`) rolls each of the 14
`role_weight` entries independently in `[0.400, 1.600]` then normalizes so they sum to `NUM_ROLES
* 1.0`. `Club.tactical_identity: TacticalIdentity` (`world.rs:64-66`) is already a field,
generated once per club at genesis (`world.rs:243-244`). Slice 5 reads this field; it adds
nothing to the struct.

## Slice 1 — Per-club squad size

### 1.1 — Data shape

Add a new field to `Club` (`world.rs:58-67`):

```rust
pub struct Club {
    pub id: ClubId,
    pub name: String,
    pub nation: NationId,
    pub strength: u8,
    pub squad_size: u8,               // NEW
    pub tactical_identity: TacticalIdentity,
}
```

`u8` is sufficient (range below tops out at 30) and matches `strength`'s type.

### 1.2 — Range and correlation with strength: 18–30, stature-weighted

Tùng's instruction was explicitly "bao nhiêu chả được" (whatever number works) — no single
number is load-bearing — but a *range correlated with strength* is: bigger/stronger clubs
should plausibly support deeper squads, mirroring `facilities_mult` (`world.rs:69-74`)
already doing the same "stronger club → more resourced" shape for training. Concrete formula,
generated alongside `strength` in `WorldGenesis::generate`'s per-club loop (`world.rs:236-253`):

```rust
let span = 12i32; // 30 - 18
let base = 18 + (strength as i32 * span) / 99;      // 18 at strength≈0 … 30 at strength=99
let noise = rng.next_range_u32(0, 2) as i32 - 1;    // ±1 jitter, same clubs don't tie exactly
let squad_size = (base + noise).clamp(18, 30) as u8;
```

**Why 18–30, not centered exactly on the old 25:** average squad size across this range is
24, giving a total headcount of `1,200 × 24 = 28,800` — close to, but slightly under, the
existing "~30,000, top edge of the bible's 20-30k band" figure (`population.rs:23-25`,
confirmed by Doc A's decisions log). This is a deliberate, small trade against Doc A's stated
target in exchange for the range actually meaning something (an 18-strong bottom-tier
minnow squad reads as thin; a 30-strong power club's squad reads as deep). **Flag for Tùng:**
if the exact 30,000 figure matters more than the range reading correctly, shift the band to
19–31 (average 25, same span) with zero other changes — noted as a one-line judgment call, not
re-derived from anything Tùng said.

### 1.3 — Ripple

`population.rs:120-124`'s genesis loop changes from a global `SQUAD_SIZE` to `club.squad_size`:

```rust
for club in &world.clubs {
    let club_id = club.id;
    for slot in 0..club.squad_size as usize {   // was: 0..SQUAD_SIZE
        ...
    }
}
```

`SQUAD_SIZE` the constant is deleted (no longer referenced anywhere after this — `squad_position`,
`player_seed` are unaffected, they only take `slot` as a parameter). Existing tests that assert
`pop.len() == world.clubs.len() * SQUAD_SIZE` (`population.rs:241`) must instead sum
`club.squad_size` across all clubs.

### TDD anchor

- `squad_size_correlates_with_strength`: generate a world, assert clubs sorted by `strength`
  are non-strictly monotonic in `squad_size` within noise tolerance (e.g. top-quartile-strength
  clubs average a materially higher `squad_size` than bottom-quartile clubs).
- `squad_size_always_in_band`: every generated club's `squad_size` is in `18..=30`.
- `genesis_headcount_matches_sum_of_squad_sizes`: `pop.len() == world.clubs.iter().map(|c|
  c.squad_size as usize).sum()`.
- `genesis_is_deterministic` (existing test, `population.rs:255-261`) still passes unmodified —
  determinism per seed is not touched by this slice.

## Slice 2 — Keep the anchor, add a rare outlier roll

### 2.1 — What changes vs. what doesn't

The existing anchor formula (`population.rs:132-135`, quoted above) is **not removed**. A new
independent branch is added in front of it: a small chance the player's `potential_ovr` ignores
`club.strength` entirely and rolls from the full valid band instead.

```rust
const OUTLIER_CHANCE_PCT: u32 = 2;   // Tùng's own example figure, 2026-07-22
const POTENTIAL_MIN: u8 = 30;
const POTENTIAL_MAX: u8 = 99;

fn roll_potential_ovr(rng: &mut GoatRng, club_strength: u8) -> u8 {
    if rng.next_range_u32(0, 99) < OUTLIER_CHANCE_PCT {
        // Outlier: club-strength anchor ignored entirely.
        return rng.next_range_u32(POTENTIAL_MIN as u32, POTENTIAL_MAX as u32) as u8;
    }
    let base = club_strength as i32;
    let variance = rng.next_range_u32(0, 30) as i32 - 15;
    (base + variance).clamp(POTENTIAL_MIN as i32, POTENTIAL_MAX as i32) as u8
}
```

Both `genesis()` (`population.rs:132-135`) and Slice 4's youth-intake roll call this same
function — one shared formula, not duplicated. This is the only structural change to
`genesis()`'s per-player loop this slice makes.

### 2.2 — Why this specific shape closes the gap the "Verified" section quantified

A club with `strength ≤ 14` produces a flat potential-30 ceiling under the anchor branch alone
(shown above). Under the outlier branch, that same weak club's players roll uniformly across
`[30, 99]` — mean 64.5 — independent of the club at all. At `OUTLIER_CHANCE_PCT = 2`, roughly
1 in 50 players at *any* club (not just weak ones — the outlier roll doesn't check
`club.strength` before applying) breaks free of the anchor. For an 18–30-player squad (Slice
1), that's a real, non-zero chance (`1 - 0.98^24 ≈ 39%` for a 24-player squad) that a single
season's genesis at a given club produces at least one such player — this is deliberately
common enough to be a recurring "unearthed at a nobody club" story across a full 1,200-club
world, not a once-in-a-save curiosity.

### 2.3 — Numbers Tùng should confirm, not silently assumed

- `OUTLIER_CHANCE_PCT = 2` is Tùng's own example from the design conversation, adopted as the
  literal first-pass constant (not re-derived) — but it is exactly that, a first-pass number,
  and follows the same "needs a `TASK-TUNE` pass once playtested" convention Doc B applies to
  its own placeholder constants (Doc B §B.3).
- The outlier band `[30, 99]` reuses the existing anchor formula's own clamp bounds
  (`population.rs:135`) rather than introducing a third pair of magic numbers — a judgment
  call for internal consistency, not something Tùng specified.

### TDD anchor

- `outlier_roll_breaks_the_weak_club_ceiling`: seed a `GoatRng` to force the outlier branch
  (deterministic seed search or a test-only injection point), assert the resulting
  `potential_ovr` for a `strength = 1` club can exceed 30 — the exact regression the "Verified"
  section's ceiling math describes.
- `outlier_rate_is_roughly_two_percent`: over a large fixed-seed sample (e.g. 100,000 rolls),
  assert the fraction landing outside the anchor band `[club.strength - 15, club.strength + 15]`
  (clamped) is within a wide statistical tolerance of 2%.
- `anchor_branch_unchanged_when_no_outlier`: for rolls that don't hit the outlier branch, the
  output must match the pre-Slice-2 formula bit-for-bit (regression guard against accidentally
  changing the common-case anchor behaviour).

## Slice 3 — Live club strength for match simulation

### 3.1 — What's actually new here (smaller than it sounds — see "Verified")

The background-league / promotion-relegation path (`batch_tick_season` →
`ReplayCache::advance_one_season`) **already** simulates every match on live, roster-derived
strength, not the static `club.strength`. What's missing is that this computation
(`club_strength`, `batch_tick.rs:52-62`, and its caller-side `squads_by_club`,
`batch_tick.rs:44-50`) is a **private helper local to `batch_tick.rs`**, only reachable through
the full-season batch simulation. There is no way today to ask "what is club X's live strength
right now" without running an entire season through it.

### 3.2 — New: a public, reusable live-strength query

Promote the formula to a public `Population` method, independent of the batch-tick loop:

```rust
impl Population {
    /// Live team strength (1-99): mean current OVR of a club's non-retired squad at
    /// `elapsed_weeks`. O(pop.len()) — a linear scan filtered by club id; fine for an
    /// occasional single-club query (e.g. a UI "opponent strength" lookup), but callers
    /// simulating every club in one pass (batch-tick) should keep using a precomputed
    /// squads-by-club grouping, not call this once per club.
    pub fn live_strength(&self, club_id: ClubId, elapsed_weeks: u32) -> u8 {
        let squad: Vec<usize> = (0..self.len())
            .filter(|&i| self.club[i] as usize == club_id && !self.is_retired(i, elapsed_weeks))
            .collect();
        Self::live_strength_from_squad(self, &squad, elapsed_weeks)
    }

    /// Same formula, given a precomputed squad (the batch-tick bulk path). Both `live_strength`
    /// and `batch_tick::club_strength` route through this — one formula, not two.
    pub fn live_strength_from_squad(&self, squad: &[usize], elapsed_weeks: u32) -> u8 {
        if squad.is_empty() { return 1; }
        let sum: u32 = squad.iter().map(|&i| self.current_ovr(i, elapsed_weeks) as u32).sum();
        (sum / squad.len() as u32).clamp(1, 99) as u8
    }
}
```

`batch_tick.rs`'s private `club_strength` fn is deleted; `batch_tick_season` calls
`pop.live_strength_from_squad(&squads[c], elapsed_weeks)` instead — same output, same
performance characteristics (still one pass over each club's precomputed squad, not a
per-club rescan), now with the formula owned in one place (`population.rs`, next to
`current_ovr`/`is_retired`, its natural home) instead of duplicated logic living inside a
simulation-loop file.

One behavioral fix folded in here: the pre-existing `club_strength` (`batch_tick.rs:52-62`)
does **not** filter by `is_retired` — it averages every squad member's `current_ovr`
regardless of retirement, and `current_ovr` itself doesn't know about retirement either (it's
a pure function of age/potential, not a "is this player playing" check). A retired player's
`current_ovr` computed past `RETIRE_AGE_YEARS` continues climbing/declining per
`development_fraction`'s formula (`population.rs:158-167`) with no floor at the retirement age
— it just keeps evaluating the same curve. `live_strength`/`live_strength_from_squad` above
add the `!is_retired` filter explicitly, so a club's live strength reflects only players who'd
actually turn out for it. This is a small, clearly-scoped correctness fix surfaced by writing
this slice, not a new feature — flagged so Dev doesn't mistake it for scope creep.

### 3.3 — What deliberately stays static (out of this slice, on purpose)

- `Club::facilities_mult()` (`world.rs:70-74`) keeps reading the static `club.strength` — it's
  a youth-development/training multiplier, not a match-strength lookup, and Tùng's item 3 was
  specifically "match-strength lookups," not every consumer of `strength`.
- `history.rs:119-129`'s backfilled "greatest team of the era" flavor generation keeps reading
  static `club.strength` — it computes retrospective champions for years *before* the game's
  present day even starts, for which there is no real roster to derive a live strength from
  (the background population doesn't exist yet at those in-fiction dates). Out of scope by
  construction, not by omission.

### 3.4 — Orbit-path wiring: a note, not a prescription

The PC's own-match code paths (`crates/goat-tui/src/career_sim.rs`, `crates/goat-tui/src/
main.rs`) currently reference `CLUBS[...].strength` against a `CLUBS`/`DIV_CLUBS` static-array
API that Doc A's implementation, in flight tonight in the same working tree, has already
removed from `goat-world`'s public exports (`crates/goat-world/src/lib.rs:27-31` no longer
re-exports them) — those two `goat-tui` files do not currently compile against `goat-world`'s
present shape. Citing exact line numbers there as "current" would be citing code Doc A's own
migration is about to rewrite out from under this doc. **The instruction for Dev, once Doc A's
`goat-tui` migration lands:** wherever the migrated orbit-path match-strength lookup ends up,
it should call `Population::live_strength` (3.2) instead of a static `club.strength` field —
same principle as the already-live background-league path, just extended to the PC's own club.
This doc does not prescribe *where* in the migrated `goat-tui` that call goes, since that
shape isn't settled yet.

### TDD anchor

- `live_strength_matches_live_strength_from_squad`: for a given population/club/date, the
  single-club query and the precomputed-squad path return identical values.
- `live_strength_excludes_retired_players`: construct a squad where the top-OVR member has
  passed `RETIRE_AGE_YEARS`; assert `live_strength` differs from (specifically, is lower than)
  a naive unfiltered mean — the 3.2 correctness-fix regression guard.
- `live_strength_changes_as_roster_ages`: same club/population, two different `elapsed_weeks`
  values on either side of a generational turnover, assert the returned strength differs —
  this is the actual gameplay payoff item 3 was written for ("clubs actually rise and fall
  over decades").
- `batch_tick_season`'s existing tests (`batch_tick.rs:175-226`) continue passing against the
  refactored call site — pure refactor, no behavior change to the batch-tick path itself
  (aside from the retired-filter fix in 3.2, which those tests don't currently distinguish;
  add one assertion there too if the fix needs its own coverage at that call site).

## Slice 4 — Youth academy replenishment at season-end

### 4.1 — Rejected: rigid 1-in-1-out

Tùng explicitly rejected "exactly 1 new player per 1 retiree" as "không thực tế" (not
realistic). Real academies don't produce a metronomic one graduate per departure — output
varies year to year, club to club. The design below rolls an independent random count per
club per season, with no arithmetic tie to that season's actual retirement count.

### 4.2 — Intake count: uniform 1–4 per club per season

```rust
let intake_count = rng.next_range_u32(1, 4); // uniform, mean 2.5
```

**Why this range, not something else:** `RETIRE_AGE_YEARS = 38` (`population.rs:21`) and
genesis age range `16..33` (`population.rs:129`) together imply a career span from academy
entry (16) to retirement (38) of 22 years. In steady state, a club replacing its squad evenly
across that span retires roughly `squad_size / 22` players per season — for the Slice 1
average `squad_size = 24`, that's ≈1.09 retirees/season. An intake mean of 2.5 deliberately
runs **above** the steady-state replacement rate: combined with the ±20% floating band (4.3),
this lets a club's squad organically grow toward its target size and occasionally overshoot
into the capped zone, rather than sitting exactly flat forever — closer to how a real academy
pipeline feels (some years produce a genuine batch, some years barely anyone breaks through)
while staying cheap (one small RNG draw per club per season, no new weekly cost, consistent
with bible §9's "computed on demand" discipline).

### 4.3 — Squad size floats within ±20% of the Slice-1 target, not pinned exactly

```rust
let target = club.squad_size as u32;
let ceiling = target + target / 5;                     // +20%
let active = (0..pop.len())
    .filter(|&i| pop.club[i] as usize == club.id && !pop.is_retired(i, elapsed_weeks))
    .count() as u32;
let intake_count = if active >= ceiling {
    0                                                    // squad already deep — skip intake
} else {
    rng.next_range_u32(1, 4)
};
```

No special-cased floor boost is needed: since the unconditional roll's mean (2.5) already
exceeds the steady-state retirement rate (≈1.09, from 4.2), a squad below its floor trends
back up on its own over a few seasons; the ceiling check alone is what keeps growth bounded
instead of monotonic forever. This produces the "float in a band, not pinned to an exact
number every season" behavior Tùng asked for with a single condition, not a two-sided
correction.

### 4.4 — A real correctness catch: `birth_age_weeks`' existing semantics don't hold for
mid-career intake

`Population::age_years_at` (`population.rs:182-184`) is `(self.birth_age_weeks[idx] +
elapsed_weeks) / 52` — this implicitly assumes every player's `birth_age_weeks` was recorded
*at world genesis* (`elapsed_weeks = 0`). A player generated mid-career at season `S` (elapsed
weeks `S * 52`) who should be age 16 *at that moment* would need `birth_age_weeks = (16 - S) *
52`, which is negative for any `S > 16` — not representable in the existing `u32` column and
not what the formula was written to do.

**Fix: add a new `intake_week: Vec<u32>` column**, defaulting to `0` for every
genesis-created player (which makes the formula below identical to today's behavior for all
existing players — a non-breaking change), and set to `season * 52` for Slice-4 intake
players:

```rust
pub struct Population {
    // ... existing columns ...
    pub intake_week: Vec<u32>,   // NEW — elapsed_weeks at which this player entered the pop
}

impl Population {
    fn age_years_at(&self, idx: usize, elapsed_weeks: u32) -> u32 {
        let weeks_since_intake = elapsed_weeks.saturating_sub(self.intake_week[idx]);
        (self.birth_age_weeks[idx] + weeks_since_intake) / 52
    }
}
```

`fingerprint()` (`population.rs:66-82`) must fold in `intake_week` alongside the other
identity columns — this changes the fingerprint's bit pattern for any population that includes
intake players, which is expected and correct (it's new identity data), but **breaks any
golden test that pins a literal fingerprint value including a post-intake population** — flag
for Dev to update those golden values, not treat the diff as a regression.

### 4.5 — Seeding: deterministic per `(world_seed, club_id, season, local_idx)`

Mirrors `player_seed` (`population.rs:109-113`) with a season term folded in:

```rust
fn intake_player_seed(world_seed: u64, club_id: u64, season: u32, local_idx: u64) -> u64 {
    world_seed
        ^ club_id.rotate_left(21).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (season as u64).rotate_left(31).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ local_idx.rotate_left(11).wrapping_mul(0x1656_67B1_9E37_79F9)
}
```

Same "generated but consistent" pattern as every other per-entity seed in this codebase
(`world.rs`'s `nation_seed`/`club_seed`, `population.rs`'s `player_seed`) — same
`(world_seed, season)` always reproduces the same intake, byte-for-bit, on every platform.

### 4.6 — Where this hooks in

A new function, called once per season alongside promotion/relegation:

```rust
/// Season-end youth intake for every club. Appends new SoA rows to `pop` in place
/// (never removes/reorders existing rows — retirement stays purely virtual, per
/// `is_retired`). Returns the total number of players added, for logging/telemetry.
pub fn apply_youth_intake(
    pop: &mut Population,
    world: &WorldGenesis,
    world_seed: u64,
    season: u32,
) -> u32 { ... }
```

Called from `ReplayCache::advance_one_season` (`promotion.rs:137-150`), alongside the existing
`batch_tick_season` + `apply_season_end` calls — ordering relative to those two doesn't matter
for correctness (intake shapes *next* season's roster either way), but calling it after
`batch_tick_season` (so this season's career-accumulator crediting isn't affected by
mid-season-boundary new arrivals) is the natural choice.

### 4.7 — Explicit non-dependency on backlog item #3

This mechanic's only departure channel is retirement (already existing, `is_retired`,
`RETIRE_AGE_YEARS`). It does not model, and does not need, contract expiry or AI-club-to-AI-
club transfers as either an arrival or departure channel — that's backlog item #3 ("AI-run
club economy"), explicitly out of scope for this round. When item #3 ships, it adds a
*second* departure channel (and possibly a transfer-driven arrival channel) alongside
retirement; nothing in 4.2–4.6's design needs to change to accommodate that later — the
ceiling check in 4.3 already treats "how many active players does this club have right now"
generically, regardless of why players left.

### TDD anchor

- `youth_intake_adds_players_deterministically`: same `(world_seed, season)` twice → identical
  new rows (same seeds, same attributes once promoted).
- `youth_intake_respects_ceiling`: a club artificially at/above its `squad_size * 1.2` active
  count gets zero intake that season.
- `youth_intake_uses_shared_outlier_formula`: intake players' `potential_ovr` distribution
  matches `roll_potential_ovr` (Slice 2.1) statistically — same function, not a re-derived one.
- `intake_player_age_is_correct_mid_career`: a player added at season `S` reports age 16 at
  `elapsed_weeks = S * 52` via `age_years_at` — the direct regression test for the 4.4 fix
  (assert this is **not** true under the old `birth_age_weeks`-only formula, to prove the fix
  is load-bearing).
- `fingerprint_changes_after_intake_but_is_still_deterministic`: fingerprint differs
  pre/post-intake (new identity data), but two independent runs through the same intake
  produce the same post-intake fingerprint.

## Slice 5 — Tactical-identity-biased lazy-promote potentials

### 5.1 — What stays cheap, what gets biased

Background players keep carrying only `potential_ovr: Vec<u8>` (`population.rs:44`) at genesis
and youth-intake time — Slices 2 and 4 do **not** give all ~29,000 background players full
per-attribute potentials. That would mean rolling and storing 30 `Fixed` values × ~29,000
players just to have them sit unused for players the PC never encounters — a straight violation
of bible §9's SoA/perf discipline and of `population.rs:1-9`'s own stated "cheap identity, full
realization on contact" principle, which `Population::promote` (`population.rs:204-229`, cited
in "Verified" above) already implements for exactly this reason. Slice 5 only changes what
happens **inside `promote`**, at the moment a background player is realized in full — it adds
a bias input to the *existing* full per-attribute roll, it does not move that roll earlier.

### 5.2 — Coordination with Doc B: read `TacticalIdentity.role_weight`, add nothing to it

Doc B's `TacticalIdentity` (`tactical_identity.rs:22-26`, quoted in "Verified") already exists
and is already attached to every `Club` (`world.rs:64-66`) by the time this slice's code runs
(Doc A/B are landing tonight, ahead of this doc's implementation). Slice 5 reads
`club.tactical_identity.role_weight` as-is — **no new field is added to `TacticalIdentity`,
no change to Doc B's `generate`/`team_fit` functions.** This is a deliberate constraint so the
two docs don't need to be sequenced against each other beyond "Doc B lands first," which it
already is, tonight.

### 5.3 — Bias derivation: reuse `ROLE_WEIGHT_TABLE`, no new table

The existing `ROLE_WEIGHT_TABLE: [[Fixed; NUM_ATTRS]; NUM_ROLES]` (`roles.rs:161`, already used
by `role_rating`, `derive.rs:43-61`, and by Doc B's own `team_fit`) already encodes exactly
"how much does each role care about each attribute." A club's tactical identity is a weighting
*over roles* (`role_weight`); combining the two gives a per-attribute bias with zero new
tables:

```rust
/// Per-attribute ratio of "how much this club's tactical identity rewards attribute i"
/// against a neutral (role_weight ≡ 1.0 for every role) baseline. 1.0 = neutral. Reuses
/// ROLE_WEIGHT_TABLE (roles.rs:161) — no new per-attribute-per-team table.
fn attribute_bias_ratio(identity: &TacticalIdentity) -> [Fixed; NUM_ATTRS] {
    let mut out = [Fixed::ONE; NUM_ATTRS];
    for i in 0..NUM_ATTRS {
        let mut weighted = Fixed::ZERO;
        let mut neutral = Fixed::ZERO;
        for r in 0..NUM_ROLES {
            let w = ROLE_WEIGHT_TABLE[r][i];
            weighted = weighted + identity.role_weight[r] * w;
            neutral = neutral + w;
        }
        if neutral != Fixed::ZERO {
            out[i] = weighted / neutral;
        }
    }
    out
}
```

Directionally: a technically-oriented club (high `role_weight` on playmaker/winger-family
roles, which `ROLE_WEIGHT_TABLE` weights heavily toward passing/dribbling/ball-control
attributes) produces `attribute_bias_ratio` values above 1.0 for exactly those attributes. A
physical club (high `role_weight` on target-forward/ball-winner-family roles) produces ratios
above 1.0 for strength/pace attributes instead. This is the concrete, implementable version of
Tùng's directional example — it isn't hand-authored per-attribute, it falls out of tables that
already exist.

### 5.4 — Applying the bias: a bounded nudge, not an override

Inside `roll_potentials` (`generation.rs:142-180`), add the nudge alongside the existing
per-attribute noise, only for position-relevant attributes (`w > 0`, same guard the existing
noise already uses at `generation.rs:170-174`):

```rust
const TACTICAL_BIAS_MAX_PTS: i32 = 5; // same order of magnitude as existing per-attr noise
                                       // (NOISE_WIDTH_PER_SPIKE=3 × spikiness 1-3 → ±3..±9,
                                       // tuning.rs:39) — a meaningful nudge, not an override

fn tactical_nudge(bias_ratio: Fixed) -> i32 {
    // bias_ratio typically in ~[0.6, 1.6] given role_weight's own [0.400,1.600] generation
    // band (tactical_identity.rs:38-40); clamp defensively regardless.
    let delta_permille = (bias_ratio.to_raw() - Fixed::ONE.to_raw()).clamp(-600, 600);
    (delta_permille * TACTICAL_BIAS_MAX_PTS) / 600
}
```

`roll_potentials`'s existing line `let val = (base + noise).clamp(1, 99);` (`generation.rs:176`)
becomes `let val = (base + noise + nudge).clamp(1, 99);`, where `nudge` is `0` when no identity
is supplied (see 5.5) and `tactical_nudge(attribute_bias_ratio(identity)[i])` otherwise.

**Why a bounded additive nudge, not a multiplier or a hard skew:** the position-tier base
(Key/Imp/Sec/Zero, `generation.rs:156-167`) is what actually determines an attribute's shape
for a given position — tactical identity should flavor a player within that shape, not
override it. A ±5-point nudge on a base that's already tiered by up to dozens of points (e.g.
`KEY_BASE_PCT = 95` vs `SEC_BASE_PCT = 91` of ceiling, `tuning.rs:47-58`) reads as "this
academy's graduates lean slightly toward the club's style," not "this club produces only one
kind of player" — consistent with Doc B's own non-blocking ground rule (a weight on a
probability, never a hard gate) even though this is generation, not selection.

### 5.5 — Wiring: a sibling entry point, zero ripple to existing callers

`generate_player`'s signature (`generation.rs:69`) and all ~20 existing call sites (PC
creation across `goat-core`/`goat-tui`/tests, enumerated by `grep` during this doc's research)
stay untouched. A new sibling function carries the optional bias:

```rust
pub fn generate_player(seed: u64, choices: &CreationChoices) -> PlayerView {
    generate_player_biased(seed, choices, None)
}

pub fn generate_player_biased(
    seed: u64,
    choices: &CreationChoices,
    tactical_identity: Option<&TacticalIdentity>,
) -> PlayerView {
    // identical body to today's generate_player, except step 4 calls
    // roll_potentials_biased(seed, ceiling, spikiness, primary_pos, tactical_identity)
    // instead of roll_potentials(...).
}
```

`Population::promote` (`population.rs:222`) is the **only** call site that changes, from
`generate_player(self.seed[idx], &choices)` to `generate_player_biased(self.seed[idx],
&choices, Some(&world.clubs[self.club[idx] as usize].tactical_identity))`. Every other caller
(PC creation, all golden tests) keeps calling the unbiased `generate_player`, byte-identical
output, zero test churn outside `population.rs`/`generation.rs`'s own new tests.

### TDD anchor

- `tactical_bias_shifts_technical_club_toward_technical_attrs`: construct a
  `TacticalIdentity` that heavily favors a technical-family role (mirroring Doc B's own
  `natural_and_awkward_fit_are_both_reachable` test pattern, `tactical_identity.rs:118-152`,
  which already isolates a single role's weight to the extremes), assert
  `generate_player_biased` with that identity produces a higher mean potential across
  technical-archetype attributes than `generate_player` (unbiased) across many seeds.
- `tactical_bias_is_bounded`: no attribute's potential under the biased path deviates from the
  unbiased path by more than `TACTICAL_BIAS_MAX_PTS`, across a wide seed sample — the "nudge,
  not override" guarantee.
- `unbiased_path_is_byte_identical`: `generate_player(seed, &choices)` and
  `generate_player_biased(seed, &choices, None)` produce identical `PlayerView`s for every
  existing golden-test seed — the zero-ripple guarantee for 5.5.
- `promote_passes_club_tactical_identity`: a `Population::promote` call for a club with a
  known lopsided `tactical_identity` produces a statistically detectable skew vs. the same
  seed's output from a neutral-identity club — the end-to-end regression for this slice.

## Out of scope (do not fold into this doc)

- **Contract expiry and AI-club-to-AI-club transfers** as arrival/departure channels — backlog
  item #3, "AI-run club economy," a separate parked item. Slice 4 is explicitly designed to
  compose with it later (4.7) but does not build any part of it now.
- **Full per-attribute potentials for the whole background population at genesis time** —
  explicitly rejected by bible §9's SoA discipline and by this doc's own Slice 5 design (5.1);
  only the lazy-promoted player, at promote-time, ever gets the full roll.
- **`Match`/`Fixture` as first-class persisted entities** — this was part of the *original*
  round-2 placeholder this doc supersedes, but did not carry forward into this round's
  five-item scope Tùng actually approved 2026-07-22. `season.rs`'s existing ephemeral
  fixture/table simulation is untouched by this doc.
- **A literal trained "chemistry"/familiarity axis for tactical fit** — Slice 5's bias is a
  generation-time input (shapes what a lazy-promoted player's potential *is*), not a trained,
  mutable per-player-per-club value. This mirrors Doc B's own explicit rejection of that
  reading for call-up/selection fit (Doc B §B.1) — the two docs are consistent on this point
  by design, not by coincidence.
- **Injury/age/form-weighted live strength** — Slice 3's `live_strength` is a plain mean of
  current OVR, same formula the pre-existing `batch_tick.rs::club_strength` already used.
  Weighting by injury status, recent form, or a best-XI (rather than whole-squad) selection are
  real possible refinements Tùng did not ask for this round — not designed here.
- **Variable pyramid depth, cup competitions, continental competitions** — already covered (as
  parked/out-of-scope) by `TASK-DESIGN-round2-world-genesis-scaleup.md`; not re-litigated or
  duplicated here.
- **Retuning `facilities_mult()` or any other existing consumer of static `club.strength`** —
  only the match-simulation lookups named in item 3 move to live strength (Slice 3.3); every
  other consumer is explicitly left alone.

## Decisions Design made as judgment calls — flag for Tùng's explicit sign-off

1. **Slice 1**: the exact `18–30` squad-size band and its linear correlation-with-strength
   formula. Tùng's instruction was "whatever number works" — Design picked a concrete range
   and formula per the task brief's own suggestion, but the exact numbers (and the resulting
   ~28,800 vs. the previously-stated 30,000 total headcount, 1.3) are Design's pick, not
   Tùng's.
2. **Slice 2**: `OUTLIER_CHANCE_PCT = 2` is Tùng's own example number, adopted directly — low
   risk, but still a first-pass constant per the codebase's own "needs a `TASK-TUNE` pass"
   convention (2.3).
3. **Slice 4**: the `1–4` uniform intake-count distribution and the `±20%` floating band's
   exact ceiling-skip mechanism (4.2, 4.3) are Design's concrete instantiation of Tùng's "e.g.
   1-4, tune a concrete distribution" and "e.g. ±20%" examples — adopted as literal first-pass
   numbers, not re-derived, but still numbers Tùng should knowingly bless before Dev locks
   them into a golden test.
4. **Slice 4**: the `intake_week` column and the `age_years_at` formula change (4.4) are a
   **technical necessity**, not a design preference — the mid-career-intake feature cannot be
   correct without it. Flagged because it changes `Population::fingerprint`'s golden values for
   any post-intake population, which needs conscious test-data updates, not because there's a
   real alternative being weighed.
5. **Slice 5**: `TACTICAL_BIAS_MAX_PTS = 5` and the specific `attribute_bias_ratio` derivation
   (5.3, 5.4) are Design's own construction — Tùng's brief said "you decide and specify a
   concrete, simple mapping," so this is exactly that, but the magnitude and the "reuse
   `ROLE_WEIGHT_TABLE` directly" approach (vs., say, a hand-authored attribute-group mapping)
   are Design's call and worth a quick sanity check once playtested.

## Definition of done (once Dev implements)

1. `cargo test --workspace` green, including every TDD-anchor test listed per slice above and
   updated golden-fingerprint values wherever Slice 4's `intake_week` column changes them.
2. `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` clean.
3. No new dependencies, no floats in sim state/logic, no unsafe.
4. `SQUAD_SIZE` (the old global constant, `population.rs:26`) is fully removed, not left as
   dead code alongside the new per-club field.
5. Playable gate: `cargo run -p goat-tui` → across a multi-season career, at least one weak
   club's roster is observed to produce an outlier-potential player (Slice 2) and at least one
   club's live match strength is observed to differ from its genesis-time `strength` value
   after enough seasons for roster turnover to matter (Slice 3/4 combined payoff).
6. `goat-save::save::VERSION` bumps if `intake_week`/`squad_size`/any new `Club`/`Population`
   field needs to round-trip through saves, with a backward-compat test following the existing
   precedent (`TASK-DESIGN-round1-pantheon-saves.md`'s v8→v9 fold).
7. This doc's Slice 5 remains consistent with whatever Doc B ships tonight — if Doc B's
   `TacticalIdentity` shape changes before this doc is implemented, Slice 5's `role_weight`
   references need a compatibility check against the shipped shape, not this doc's snapshot of
   it.
