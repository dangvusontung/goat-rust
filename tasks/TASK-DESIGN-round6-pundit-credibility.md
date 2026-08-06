# TASK DESIGN ROUND 6 — Pundit credibility tiers as a Reputation-impact multiplier

Prereq: none — self-contained within `goat-meta` (pundits/reputation/pantheon) plus one new
`Intent`/`reduce` arm in `goat-core::state` and a small ripple into `goat-tui`'s season-end
loop. No dependency on any in-flight round (Doc A world-genesis, Doc B tactical identity,
round 3/4/5) — this doc touches none of their surfaces.

Read first: `docs/MAIN.md` §8.7 (Pundits & Media) and §8.2 (Reputation — the 4-facet system
this doc's multiplier hooks into), `crates/goat-meta/src/pundits.rs` (the existing `Pundit`/
`PUNDITS`/`pundit_comment` machinery this doc extends), `crates/goat-meta/src/reputation.rs`
(the existing `Reputation`/`update_sporting_rep`/`update_club_fan_rep` this doc's multiplier
feeds into), `crates/goat-meta/src/pantheon.rs` (`SCHOOLS`, `all_rankings` — the ranking
pundits already key their sentiment off), `crates/goat-tui/src/main.rs:1058-1188`
(`run_awards_and_pundits` — the one live call site this doc rewires).

## Origin

This round's design was worked out interactively with Tùng today, 2026-07-22, as a direct
follow-up to the placeholder parked in `TASK-DESIGN-round2-world-genesis-scaleup.md`'s
"Parked for a future design round" section (backlog item #4). The four decisions below are
**final**, recorded here as a proper spec against real code — not re-litigated as options.
That parked placeholder's framing is corrected below (see "Verified"): its claim that the
pundit system is "0% implemented" was already wrong at the time it was written, not something
that changed since.

## Ground rules for this doc

- **No new pundit personalities, no new Pantheon schools.** `NUM_PUNDITS = 4`, `NUM_SCHOOLS =
  4` (`pundits.rs:43`, `pantheon.rs:69`), and the existing 1:1 `Pundit.school_idx` mapping to
  `SCHOOLS` (Trophy Cabinet / Eye-Test Romantics / Stats Purists / Loyalty Traditionalists) are
  untouched. Confirmed explicitly by Tùng this round, after an initial "more pundits" framing
  turned out to mean "more credibility depth per pundit," not a bigger roster.
- **No continuous 0–100 credibility score.** Credibility is a small discrete tier enum.
  Tùng: "Theo bậc đi" (go with tiers).
- **Tier assignment is deliberately simple/near-random this round, behind one function
  boundary.** Tùng: "Tạm thời cái này random nhể? Abstract cái cách tính tier lại cho dễ mở
  rộng" (this can be random for now — abstract the tier-computation so it's easy to extend
  later). The real "grows with tenure / proven-right accuracy" formula is the intended
  eventual behavior but is explicitly **not** designed here (see "Out of scope") — this round
  only needs the tier field plus the consuming mechanism (item 4) built against a
  simple/placeholder assignment function, isolated so the real formula can replace it later
  without touching anything downstream.
- **No new persisted state.** Tier is a pure, deterministic function of `(world_seed,
  pundit_index)` — same "generated but consistent" pattern as every other per-entity seed in
  this codebase (`world.rs`'s `club_seed`/`nation_seed`, `population.rs`'s `player_seed`,
  `tactical_identity.rs`'s `generate(seed)`). No `SaveData::VERSION` bump, no new column
  anywhere.
- **Only the one live pundit-comment trigger gets wired to the multiplier this round.**
  Verified below: `pundit_comment` is only ever actually called from the season-end awards
  block. The dormant `PunditContext::Pantheon`/`AwardWon`/`AwardLost` variants (never
  constructed anywhere in the workspace today) are not given new trigger points by this doc —
  that would be adding new pundit-commentary *moments*, which is a UI/flow task, not what
  Tùng asked for (the multiplier mechanism on top of the pundit system that already exists).

## Verified: current state of pundits and Reputation in code (corrects the parked placeholder)

**Both systems already exist and are already committed — this is not greenfield.** Grepping
the workspace: `crates/goat-meta/src/pundits.rs` (`Pundit`, `PUNDITS: [Pundit; 4]`,
`pundit_comment`) and `crates/goat-meta/src/reputation.rs` (`Reputation`, `compute_reputation`,
`update_sporting_rep`, `update_club_fan_rep`) are both real, non-stub modules, re-exported from
`crates/goat-meta/src/lib.rs:14-20`. `git log`/`git status` confirm both files are already on
`HEAD`, part of the pre-existing "Phase 7" work (`state.rs:105`'s own `// ── Phase 7
reputation scalars ──` comment) — not something a concurrent session added today. **The
placeholder's claim in `TASK-DESIGN-round2-world-genesis-scaleup.md` ("§8.7's pundit system
itself is 0% implemented — no pundit-related struct anywhere in the workspace") was factually
wrong when written**, not stale from later changes. This materially changes this round's risk
profile: most of the scaffolding this doc needs already exists; the work is additive
(a tier + a feedback wire), not foundational.

**`Pundit` today (`pundits.rs:8-23`):** a `#[derive(Debug, Clone, Copy)]` struct — `name`,
`role`, `personality`, `school_idx: usize`, plus four `&'static str` templates
(`praise`/`neutral`/`doubt`/`season_reaction`). `PUNDITS: [Pundit; NUM_PUNDITS]` is a `const`
array of 4 hand-authored characters, each `school_idx` matching `SCHOOLS`'s order 1:1
(`pantheon.rs:71-100`: 0=Trophy Cabinet, 1=Eye-Test Romantics, 2=Stats Purists, 3=Loyalty
Traditionalists — Marco Torres/Alice Brennan/Kwame Asante/Pavel Straka respectively). No
credibility/influence field exists on `Pundit` today — confirmed by reading the struct in
full.

**`Reputation` today (`reputation.rs:8-19`):** four `Fixed` facets — `sporting`,
`marketability` (stub, always 50 until Phase 10 per its own doc comment), `character`,
`club_fan` — matching bible §8.2 exactly. `WorldState` (`state.rs`) stores the *live,
mutable* versions of three of these as plain `i32` fields: `pc_sporting_rep`, `pc_club_fan_rep`
(`state.rs:106-107`), `pc_character_rep` (`state.rs:137`); `compute_reputation` (called
elsewhere for display) derives the `Fixed`-typed struct from these scalars plus the
discipline-rep inverse. `pc_marketability: i32` (`state.rs:128`) is tracked separately as its
own Phase-10 scalar, not through `reputation.rs`'s stub.

**Pundits currently have zero effect on Reputation — this is the exact gap item 4 closes.**
`run_awards_and_pundits` (`main.rs:1058-1188`) is the **only** call site of `pundit_comment` in
the entire workspace (grepped; `PunditContext::Pantheon`/`AwardWon`/`AwardLost` are
constructed nowhere). Its order of operations, read in full:

1. `new_sporting`/`new_club_fan` are computed from **performance alone**
   (`update_sporting_rep(state.pc_sporting_rep, season_avg, finish_pos)` /
   `update_club_fan_rep(...)`, `main.rs:1115-1121`) — no pundit input anywhere in this
   computation.
2. `state = reduce(state, Intent::ApplySeasonEndLegacy { ..., new_sporting_rep, new_club_fan_rep,
   ... }, ...)` (`main.rs:1131-1152`) — `state.rs:622-623` shows the handler **overwrites**
   `state.pc_sporting_rep`/`pc_club_fan_rep` with these values directly (`=`, not `+=`).
3. *Only after* the reduce, `rankings = all_rankings(&ev, &axes)` is computed and the pundit
   loop runs (`main.rs:1154-1185`), printing each pundit's `season_reaction` comment to `out`.
   **The rank each pundit computes is thrown away**: `let (_, rank, _) =
   rankings[pundit.school_idx];` is fetched, used for nothing except a `let _ = rank;` at the
   very end of the loop (`main.rs:1184`) — praise/doubt/neutral sentiment is computed and
   immediately discarded, never applied to anything.

**A second, related gap: the season-end path never actually branches on sentiment at all.**
`pundit_comment`'s `PunditContext::Season` arm (`pundits.rs:100-101`) always selects
`pundit.season_reaction` — the *same* template regardless of whether the season was good or
bad, just interpolated with that season's numbers. The praise/neutral/doubt branch
(`pundits.rs:102-110`, `rank <= 3` / `<= 7` / else) only exists for the `Pantheon` context
variant, which — per the point above — is never actually constructed anywhere. So today,
season-end pundit "reactions" carry no sentiment classification whatsoever; the rank-based
praise/neutral/doubt logic that already exists sits dormant, attached to a context variant with
no caller. **Slice 4 reuses this exact dormant logic** rather than inventing new sentiment
rules — see 4.1.

## Slice 1 — Lock the 4-personality, 4-school shape (no code change, a guard rail)

**Decision (item 1, verbatim):** the 4 named pundits stay mapped 1:1 to the 4 existing
Pantheon schools; this doc does not add a 5th pundit or a 5th school.

Nothing in `pundits.rs`/`pantheon.rs`'s existing data changes under this slice — this is
purely a locked ground rule, stated as its own slice because it was an explicit point of
back-and-forth with Tùng this round (he first asked for "more pundits," then clarified he
meant credibility, not personality count) and because Slices 2–4 below are written assuming
`NUM_PUNDITS == NUM_SCHOOLS == 4` stays true. Recorded as its own slice so a future round
doesn't have to re-derive whether this was settled.

### TDD anchor

- `pundit_count_locked_at_four`: `assert_eq!(NUM_PUNDITS, 4)` and `assert_eq!(NUM_SCHOOLS,
  4)` in `pundits.rs`/`pantheon.rs` test modules (neither file has any tests today — this is
  the first for both). A regression guard, not a functional test: if a future change bumps
  either count without a new design pass, this fails loudly instead of silently drifting.
- `every_pundit_school_idx_in_range`: `PUNDITS.iter().all(|p| p.school_idx < NUM_SCHOOLS)` —
  cheap sanity check that the 1:1 mapping stays valid.

**Size: trivial, risk: none.** A documentation/guard-rail slice, no behavior change.

---

## Slice 2 — `PunditTier`, a discrete 3-tier credibility axis

### 2.1 — Three tiers, not four: Rookie / Established / Legend

**Decision (item 2):** credibility is a small discrete enum, not a continuous score. Tùng's
own example in conversation was "Rookie → Established → Legend" — Design adopts that exact
3-tier set rather than inventing a 4th, for two reasons: (a) it's the literal example Tùng
gave, and (b) a clean 3-way low/medium/high split is enough to make the multiplier (Slice 4)
legible without adding granularity that doesn't do anything useful until the real
accuracy-tracking mechanism (deferred, see "Out of scope") can actually distinguish finer
gradations. **Flag for Tùng:** the task brief itself said "3-4 is probably right" — if a 4th
tier (e.g. an "Icon" tier above Legend, mirroring `Reputation::label`'s own 6-tier "Iconic" top
band, `reputation.rs:29-31`) is wanted for symmetry with that labeling scheme, that's a
one-line enum change plus one more multiplier constant, not a redesign — but it's Design's call
to default to 3, not a re-derivation of something Tùng already decided.

```rust
/// A pundit's credibility, currently assigned by `tier_for` (2.3) — a deliberately simple/
/// placeholder function. The real "grows with tenure and being proven right" mechanic
/// (bible-adjacent, raised by Tùng, not designed here) will replace `tier_for`'s body without
/// needing to touch anything that reads `PunditTier` — that is the entire point of keeping
/// this behind one function boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PunditTier {
    Rookie,
    Established,
    Legend,
}
```

### 2.2 — Field placement: NOT on the `Pundit` const struct

**A real design fork, resolved:** `Pundit` (`pundits.rs:8-23`) is a `#[derive(Copy)]` value
inside a `const PUNDITS` array — the same character (e.g. Marco Torres) is shared across every
save. Credibility, by contrast, must vary **per save** (a rookie-tier Marco Torres in one
career, a Legend-tier Marco Torres in another, both deterministic within their own save) —
exactly the same "world-seed-derived, not baked into a compile-time const" shape Doc A's
`WorldGenesis` already established for club data. So `PunditTier` is **not** a new field on
`Pundit` — it is computed on demand by `tier_for` (2.3), never stored on the const struct and
never persisted. This keeps `Pundit` byte-identical and avoids yet another `Copy`→`Clone`
migration (the exact mechanical cost Doc A's Slice A1 had to pay for `Club`) — there is no
reason to pay that cost here since tier is cheap to recompute every time it's needed (a single
seeded RNG draw, same cost class as `tactical_identity::generate`).

### 2.3 — `tier_for`: the one function boundary, deliberately simple this round

```rust
/// Deliberately simple/placeholder tier assignment — a seeded roll per (world_seed,
/// pundit index), deterministic within a save, independent across saves. This is the ONE
/// function the real "grows with tenure / proven-right accuracy" formula replaces later;
/// nothing outside this function should ever compute a tier by any other means.
pub fn tier_for(pundit_idx: usize, world_seed: u64) -> PunditTier {
    let mut rng = GoatRng::new(pundit_tier_seed(world_seed, pundit_idx));
    match rng.next_range_u32(0, 99) {
        0..=19 => PunditTier::Rookie,        // 20%
        20..=84 => PunditTier::Established,  // 65%
        _ => PunditTier::Legend,             // 15%
    }
}

fn pundit_tier_seed(world_seed: u64, pundit_idx: usize) -> u64 {
    world_seed ^ (pundit_idx as u64).rotate_left(23).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}
```

Same shape as `world.rs`'s `club_seed(world_seed, c)` / `nation_seed(world_seed, n)` — an
index-keyed XOR-and-multiply, no new RNG idiom introduced. `pundit_idx` is the pundit's
position in `PUNDITS` (0..4) — no new id field needed on `Pundit` itself.

**Numbers flagged for Tùng, not silently assumed:** the 20% / 65% / 15% split (Rookie /
Established / Legend) is Design's own pick — "mostly Established, Legend genuinely rare,
Rookie a real but minority chance" — matching the flavor of "a green rookie writer's hot take
should barely move public perception" from the design conversation, but the exact percentages
are not something Tùng specified and should get a `TASK-TUNE`-style sanity check once
playtested (same convention Doc A/round-3 already apply to their own placeholder constants).

### TDD anchor

- `tier_for_is_deterministic_per_seed`: same `(pundit_idx, world_seed)` twice → identical tier.
- `tier_for_varies_across_pundits_and_seeds`: across a spread of seeds and all 4 indices, all
  three tiers are actually reachable (not a broken distribution that only ever rolls one
  variant) — a coarse statistical check, not an exact-frequency pin (following the codebase's
  own "assert invariants, not frozen values, for new randomized behavior" convention).
- `tier_for_roughly_matches_declared_split`: over a large fixed-seed sample (e.g. 10,000 rolls
  across varying `pundit_idx`), the Rookie/Established/Legend proportions land within a wide
  statistical tolerance of 20/65/15 — same style as round-3's
  `outlier_rate_is_roughly_two_percent`.

**Size: small, risk: low.** A new enum plus one small pure function; no ripple to any existing
caller since nothing reads `PunditTier` yet until Slice 4 wires it in.

---

## Slice 3 — Surface the tier at the one live pundit-comment call site

### 3.1 — Minimal wiring: compute the tier, thread it through

`run_awards_and_pundits`'s pundit loop (`main.rs:1160-1185`) currently does
`for pundit in PUNDITS.iter()`, discarding both the array index and the computed `rank`. This
slice changes it to `for (idx, pundit) in PUNDITS.iter().enumerate()` and computes `let tier =
pundits::tier_for(idx, state.world_seed);` once per pundit, per season-end render. This is the
minimal ripple needed for Slice 4 to have a tier value in hand at the point it needs one — no
other behavior changes in this slice.

### 3.2 — Optional, low-cost: display the tier alongside the pundit's byline

**Not requested by Tùng, a Design suggestion only — flag before building:** since the loop
already prints `"{pundit.name} ({pundit.role}):"` (`main.rs:1169`), appending the tier (e.g.
`"Marco Torres (ex-striker, pundit) [Established]:"`) is a one-line change that makes the new
mechanic visible to the player rather than a silent internal multiplier. This is cosmetic and
entirely optional — the multiplier (Slice 4) works identically whether or not this is shown.
**Decide with Tùng before Dev builds it**; it's cheap either way, so this doc doesn't treat it
as load-bearing.

### TDD anchor

- No new pure-logic test needed beyond Slice 2's (this slice is TUI wiring, not new
  computation) — covered by Slice 4's end-to-end test instead, plus a playable-gate check.
- Playable gate: `cargo run -p goat-tui` → play to a season-end → the pundit block renders
  without panicking, same visible comments as before (plus the tier suffix, if 3.2 is
  approved).

**Size: trivial, risk: low.** Pure plumbing — an `.enumerate()` and one function call added to
an existing loop.

---

## Slice 4 — Credibility tier as a Reputation-impact multiplier

### 4.1 — Sentiment classification: reuse the dormant rank-based logic, don't reinvent it

**Decision (item 4):** every pundit comment (praise or criticism) moves player Reputation, by
an amount the pundit's credibility tier multiplies. To do this, the season-end path needs an
actual sentiment (Praise/Neutral/Doubt), which — per "Verified" above — it doesn't compute
today (it always renders `season_reaction` regardless of standing). The praise/neutral/doubt
*threshold logic* already exists, just attached to the wrong (unused) context variant
(`pundits.rs:102-110`: `rank <= 3` → praise, `<= 7` → neutral, else → doubt). This slice
extracts that classification into a small reusable function so both the existing `Pantheon`
template-selection branch and this new reputation-impact path share one source of truth:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PunditSentiment {
    Praise,
    Neutral,
    Doubt,
}

/// Same thresholds `pundit_comment`'s `Pantheon` branch already used — extracted so this
/// slice's Reputation-impact calc and the existing template-selection logic share one
/// source of truth instead of duplicating the rank cutoffs.
pub fn sentiment_from_rank(rank: usize) -> PunditSentiment {
    if rank <= 3 {
        PunditSentiment::Praise
    } else if rank <= 7 {
        PunditSentiment::Neutral
    } else {
        PunditSentiment::Doubt
    }
}
```

`pundit_comment`'s `Pantheon` arm (`pundits.rs:102-110`) is refactored to call
`sentiment_from_rank(*rank)` and match on it for template selection — behavior-identical,
now backed by a named, testable function instead of inline `if`/`else if`.

**The season-end path (`main.rs`'s pundit loop) gains the same classification it never had:**
`let sentiment = sentiment_from_rank(rank);` (using the `rank` value the loop already computes
and currently discards). This is a real, if small, behavior addition beyond pure refactor: it
means the *season_reaction* template could in principle also branch on sentiment later, but
this doc does **not** change `season_reaction`'s template text or add new template variants —
that's a content/writing task, out of scope here (see "Out of scope"). This slice only uses
`sentiment` to drive the Reputation delta below; the printed comment text is unchanged.

### 4.2 — Reputation delta formula: base sentiment delta × tier multiplier

```rust
/// Credibility-tier multiplier on Reputation impact, in permille (1000 = ×1.0) — same
/// permille-arithmetic idiom as `tactical_identity`'s `tactical_nudge` (round-3 §5.4),
/// kept integer, no floats (project-wide constraint).
impl PunditTier {
    pub fn reputation_multiplier_permille(self) -> i32 {
        match self {
            PunditTier::Rookie => 400,       // ×0.4 — a green rookie's take barely registers
            PunditTier::Established => 1000, // ×1.0 — baseline
            PunditTier::Legend => 2200,      // ×2.2 — a veteran's take carries real weight
        }
    }
}

const PUNDIT_BASE_REP_DELTA: i32 = 3; // magnitude of a single pundit's praise/doubt, before tier scaling

/// One pundit's Reputation-impact contribution this season. `Neutral` always contributes 0
/// regardless of tier — there is nothing for a multiplier to scale.
pub fn pundit_reputation_delta(sentiment: PunditSentiment, tier: PunditTier) -> i32 {
    let base = match sentiment {
        PunditSentiment::Praise => PUNDIT_BASE_REP_DELTA,
        PunditSentiment::Neutral => 0,
        PunditSentiment::Doubt => -PUNDIT_BASE_REP_DELTA,
    };
    (base * tier.reputation_multiplier_permille()) / 1000
}
```

Concretely: a Rookie's praise nudges Reputation by `+1` (3 × 0.4 = 1.2, truncated); an
Established pundit's praise/doubt is `±3`; a Legend's praise is `+6` (3 × 2.2 = 6.6,
truncated) and a Legend's doubt is `-6`. This is the literal "a veteran/Legend-tier pundit's
take should move Reputation much more than a Rookie's for the same underlying event"
requirement, expressed as concrete integers.

**Numbers flagged for Tùng, not silently assumed:** `PUNDIT_BASE_REP_DELTA = 3` and the
400/1000/2200 permille multipliers are both Design's own construction — Tùng confirmed the
*mechanism* ("Đúng đó" — yes, that's it, re: tier multiplying comment impact on Reputation)
but did not specify magnitudes. These are first-pass numbers in the same spirit as round-3's
`OUTLIER_CHANCE_PCT`/`TACTICAL_BIAS_MAX_PTS` — adopted so this slice is concretely buildable
and testable, but explicitly needing a `TASK-TUNE` pass once playtested, not locked-in balance.

### 4.3 — Which Reputation facet: Sporting, summed across all 4 pundits, applied once per season

**A real scope decision Tùng did not specify — Design's pick, flagged:** bible §8.2 defines 4
facets; pundit debate is "the voice of the plural pantheon" (§8.1), i.e. commentary on
footballing merit/legacy standing — the closest existing facet is **Sporting**
(`pc_sporting_rep`; drives contract value/transfer interest, §8.2's own table). Character,
Club/Fan, and Marketability are each already driven by other, more specific existing systems
(scandals/discipline for Character, loyalty/tenure for Club/Fan, sponsors for Marketability) —
routing pundit commentary into any of those would overlap with mechanics this doc doesn't
touch. **Design recommendation: Sporting only.** Flag for Tùng: if pundit debate should also
nudge Marketability (pundits shape *public image*, arguably a Marketability signal too) that's
a one-line addition once Marketability's Phase-10 stub (`reputation.rs:14,46`) is unstubbed —
not designed here since that stub is explicitly out of this doc's scope.

The four pundits' individual deltas (4.2) are summed into one total and applied once, at the
same season-end moment they're already computed:

```rust
let total_pundit_rep_delta: i32 = PUNDITS
    .iter()
    .enumerate()
    .map(|(idx, _)| {
        let (_, rank, _) = rankings[PUNDITS[idx].school_idx];
        let tier = tier_for(idx, state.world_seed);
        pundit_reputation_delta(sentiment_from_rank(rank), tier)
    })
    .sum();
```

### 4.4 — Wiring into `reduce`: a new `Intent`, applied after `ApplySeasonEndLegacy`

**Why a second intent, not folding into `ApplySeasonEndLegacy`:** `ApplySeasonEndLegacy`'s
handler (`state.rs:622-623`) **overwrites** `pc_sporting_rep` with the performance-only
`new_sporting_rep` value computed *before* the pundit loop runs (see "Verified" — the pundit
loop needs `rankings`, which is computed from the post-`ApplySeasonEndLegacy` state, since
rankings should reflect this season's now-folded-in evidence). Reordering
`ApplySeasonEndLegacy` to run *after* the pundit loop would require restructuring evidence
computation to work off pre-reduce season-live counters instead of career totals — a bigger,
riskier change than just adding a second, additive intent that runs after:

```rust
/// Apply the season's aggregate pundit-commentary Reputation impact (Design round 6, item
/// 4) — credibility-tier-weighted, computed by the caller from each pundit's rank-derived
/// sentiment (goat-meta::pundits), summed to one delta, applied here as a single clamped
/// addition on top of whatever ApplySeasonEndLegacy already set this season.
ApplyPunditReputationImpact { sporting_rep_delta: i32 },
```

```rust
Intent::ApplyPunditReputationImpact { sporting_rep_delta } => {
    state.pc_sporting_rep = (state.pc_sporting_rep + sporting_rep_delta).clamp(0, 100);
    state
}
```

`run_awards_and_pundits` calls this immediately after the pundit-comment loop, once, with
`total_pundit_rep_delta` (4.3). This keeps every state mutation flowing through
`reduce`/`Intent` (the existing architectural convention — no direct field mutation from the
TUI layer) while resolving the ordering constraint without touching
`ApplySeasonEndLegacy`'s existing 15-field shape at all.

### 4.5 — What this does NOT touch

- `update_sporting_rep`/`update_club_fan_rep` (`reputation.rs:53-82`) are unchanged —
  performance-driven Reputation math stays exactly as-is; this slice adds a second, independent
  pundit-driven adjustment on top, not a replacement.
- `compute_reputation` (`reputation.rs:39-50`, used for display elsewhere) is unchanged — it
  already just reads whatever `pc_sporting_rep` currently holds, so it picks up the
  pundit-adjusted value for free, no changes needed there.
- No mid-season / flashpoint-triggered pundit commentary is added (see "Ground rules" — only
  the one already-live season-end trigger is wired to the multiplier this round).

### TDD anchor

- `sentiment_from_rank_matches_existing_thresholds`: `sentiment_from_rank(1..=3) ==
  Praise`, `(4..=7) == Neutral`, `(8..) == Doubt` — pins the extracted function to the exact
  behavior `pundit_comment`'s `Pantheon` arm already had, a refactor-safety regression guard.
- `pundit_comment_pantheon_arm_unchanged_after_refactor`: for every `rank` value,
  `pundit_comment(..., PunditContext::Pantheon { rank }, ...)` produces byte-identical output
  before/after the 4.1 refactor — zero-behavior-change guarantee for the existing (if
  currently unused) code path.
- `pundit_reputation_delta_scales_with_tier`: for a fixed sentiment (e.g. `Praise`),
  `pundit_reputation_delta(Praise, Legend) > pundit_reputation_delta(Praise, Established) >
  pundit_reputation_delta(Praise, Rookie) > 0` — the literal "Legend moves Reputation more
  than Rookie" requirement, as an ordering assertion (not pinned to the exact magnitudes,
  which are flagged as tunable in 4.2).
- `pundit_reputation_delta_neutral_is_always_zero`: `pundit_reputation_delta(Neutral, tier) ==
  0` for all three tiers — multiplying zero by anything is still zero, worth asserting
  explicitly since it's the "no effect" case a reader might otherwise assume needs special
  casing.
- `pundit_reputation_delta_sign_matches_sentiment`: `Praise` always `> 0`, `Doubt` always `<
  0`, for every tier — the praise-vs-criticism direction guarantee.
- `apply_pundit_reputation_impact_clamps_to_0_100`: seed `pc_sporting_rep` near 0 or 100,
  apply a delta that would overshoot, assert the result stays in `[0, 100]` — same clamp
  discipline every other reputation updater already has (`reputation.rs:62,81`).
- `apply_pundit_reputation_impact_is_additive_not_overwriting`: unlike
  `ApplySeasonEndLegacy`'s `=` assignment, this intent's handler must be `+=`-shaped
  (`state.pc_sporting_rep + delta`, clamped) — a regression test asserting two consecutive
  `ApplyPunditReputationImpact` calls compound rather than the second clobbering the first,
  since that's the exact mistake the 4.4 design note is guarding against.
- Playable gate: `cargo run -p goat-tui` → play a season where the PC finishes clearly inside
  or outside a school's top-7 (forcing a non-Neutral sentiment for at least one pundit) →
  observe `pc_sporting_rep` (visible via the game sheet / legacy screen) shift by a small,
  non-zero amount immediately after the pundit block renders, beyond what
  `update_sporting_rep`'s performance-only formula alone would have produced that season.

**Size: medium, risk: medium.** The formula itself is simple integer arithmetic, but this is
the slice with real new behavior: a new `Intent` variant, a `reduce` arm, a sentiment
classification path that didn't functionally exist before (season-end pundit comments never
branched on sentiment), and an ordering dependency on `ApplySeasonEndLegacy` that must be
gotten right (additive-after, not overwrite-instead) to avoid quietly reintroducing a
d77170b-style "which write wins" bug in a different corner of the same season-end pipeline.
Recommend Dev write the `apply_pundit_reputation_impact_is_additive_not_overwriting` test
first, before wiring the TUI call site, given that exact bug class's precedent in this
codebase.

---

## Out of scope (do not fold into this doc)

- **More pundit personalities or more Pantheon schools.** Explicitly ruled out by Tùng this
  round (Slice 1). `NUM_PUNDITS`/`NUM_SCHOOLS` stay at 4.
- **Real prediction-accuracy tracking / tenure-based credibility growth.** This is the
  eventual intended behavior behind `tier_for` (2.3), explicitly named by Tùng as the "real"
  mechanic he wants, but the exact formula (what counts as "proven right," how tenure accrues,
  whether it needs new per-pundit persisted state — tenure implies *some* notion of "seasons
  since this pundit was introduced," which does not exist anywhere today) is a substantially
  bigger design question than this round's scope. `tier_for`'s function-boundary isolation
  (2.3) exists specifically so this can be designed and implemented later without touching
  `PunditTier` itself, `pundit_reputation_delta`, or the `ApplyPunditReputationImpact` intent —
  only `tier_for`'s body changes.
- **A continuous 0–100 credibility score.** Explicitly rejected by Tùng in favor of discrete
  tiers (Slice 2).
- **New pundit-commentary trigger points** (a presser after a red card, a transfer-saga
  statement — bible §8.7's "media interaction is a flashpoint" framing; `RespondToMedia`
  already exists as a *player*-reaction intent but does not currently invoke any pundit
  commentary). This doc wires the multiplier onto the one trigger that already exists
  (season-end); adding new moments where pundits speak is a separate, UI/flow-shaped task.
- **Marketability facet impact.** Flagged as a possible future extension in 4.3, not built —
  `pc_marketability`'s Phase-10 stub is untouched.
- **Unstubbing `Reputation::marketability`** (`reputation.rs:14,46`) generally — unrelated to
  this doc, not touched.
- **Displaying the credibility tier in any UI beyond the optional one-line season-end suffix**
  (3.2, itself optional and unconfirmed) — no Flutter/`goat-bridge` DTO changes, no dedicated
  "pundit roster" screen.
- **Any change to `season_reaction`'s template text** or new sentiment-branched templates for
  the season-end path — the printed comment text is unchanged by this doc; only the
  Reputation *side effect* of that comment is new.

## Decisions Design made as judgment calls — flag for Tùng's explicit sign-off

1. **Slice 2**: exactly 3 tiers (Rookie/Established/Legend), not 4 — Design's default per the
   task brief's own "3-4 is probably right," using Tùng's literal example names. A 4th tier is
   a cheap addition later if wanted (2.1).
2. **Slice 2**: the 20% / 65% / 15% tier-assignment split in `tier_for` — Design's own numbers,
   not specified by Tùng, first-pass constants needing a `TASK-TUNE` pass (2.3).
3. **Slice 3**: whether to display the tier alongside each pundit's byline in the season-end
   output — a cosmetic, optional addition Design flagged but did not decide (3.2).
4. **Slice 4**: `PUNDIT_BASE_REP_DELTA = 3` and the 400/1000/2200-permille tier multipliers —
   Design's own construction implementing the confirmed *mechanism*, not Tùng-specified
   magnitudes (4.2).
5. **Slice 4**: routing the impact into the **Sporting** facet only, excluding Marketability/
   Character/Club-Fan — Design's pick given the existing facet-to-system mapping, flagged as
   an open question re: Marketability specifically (4.3).

## Definition of done (once Dev implements)

1. `cargo test --workspace` green, including every TDD-anchor test listed per slice above —
   this is also the *first* test coverage `pundits.rs`/`reputation.rs` will have had at all
   (verified: neither file has any `#[test]` today).
2. `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` clean.
3. No new dependencies, no floats in sim state/logic, no unsafe (`goat-meta`'s existing
   `#![forbid(unsafe_code)]` crate attribute, project-wide convention).
4. No `SaveData::VERSION` bump — tier is recomputed on demand, never persisted, per Slice 2's
   ground rule.
5. `Pundit`'s existing `#[derive(Copy)]` and `PUNDITS: [Pundit; NUM_PUNDITS]`'s `const`-ness
   are both unchanged — no repeat of Doc A's `Club` `Copy`→`Clone` migration cost, since
   credibility tier is deliberately never stored on the struct (2.2).
6. Playable gate: `cargo run -p goat-tui` → across at least one season where a pundit's
   sentiment is non-neutral, observe `pc_sporting_rep` move by the pundit-driven delta on top
   of the performance-driven update, in the same season-end render.
7. `main.rs:1184`'s `let _ = rank;` (the currently-discarded rank) is gone, replaced by actual
   use of that value in the sentiment/tier/delta pipeline — a concrete "this dead code is now
   live" marker Dev can grep for to confirm the wiring landed.
