# TASK DESIGN ROUND 1 — Pantheon signals + save slots (3 parked decisions, resolved)

Prereq: none — this is a design+implementation task on the current `goat-tui`/`goat-core`
build, triggered by 3 decisions Tùng made on 2026-07-22 that had been parked as "open
questions" in `docs/DESIGN-CRITIQUE-2026-07-22.md` (item 1) and
`tasks/TASK-PLAYTEST-round1-fixes.md`'s "Out of scope" list (the save-slots item and the
Legacy/Pantheon architecture item). Read both of those files first, then
`docs/MAIN.md` §8.1 (Legacy & the Pantheon) and §9 (Engineering & Performance Notes — the
"tiny save" principle), then `docs/TECH-REFERENCE-DRAFT.md` §6 (Save format).

## Ground rule for this task

Unlike the two playtest-fix rounds, this task **does** change numbers and add new state
fields — Tùng's decisions explicitly call for that ("a real design+code task, not just
numbers tuning" — decision 2). What stays a hard constraint from prior rounds:

- **No change to Legacy/Pantheon's season-end batch cadence.** `Intent::ApplySeasonEndLegacy`
  remains the only place persistent `pc_career_*`/legacy-evidence counters advance. Every
  new counter this task adds (standout matches, transfer-request count) follows the
  existing `pc_season_X` (live staging, resets every `StartSeason`) → `pc_career_X`
  (persisted, only advances in `ApplySeasonEndLegacy`) two-tier pattern already used by
  `pc_season_goals → pc_career_goals`. This is decision 1, formalized as an implementation
  constraint for the other two slices, not just a doc note.
- **Respect the tiny-save format.** `goat-save::save::SaveData` (`crates/goat-save/src/save.rs`)
  is a flat, versioned, fixed-field binary blob — no full match/season history, only
  scalar counters and the current-season materialized state. New fields append at the end
  with `.unwrap_or(default)` reads so old saves still load (see the existing v7→v8
  `pc_lifestyle_score` precedent at `save.rs:479-480` write side / `save.rs:597-598` read
  side, and its dedicated backward-compat test `old_v7_save_without_lifestyle_score_defaults_to_balanced`
  in `crates/goat-save/tests/save_roundtrip.rs:224-...`). Slice 3 (save slots) needs **zero**
  format changes — it's purely a file-path/directory concern, confirmed by reading
  `save_to_file(data, path: impl AsRef<Path>)` / `load_from_file(path: impl AsRef<Path>)`
  (`save.rs:262-271`), which already take an arbitrary path.

## TDD anchor (applies to every slice)

- Slice 2's new `goat-core` fields/reducer logic get golden/invariant tests in
  `crates/goat-core/` following the frozen-golden-values rule (new fields → new tests).
- Slice 2's new `goat-meta` scoring logic gets new tests in
  `crates/goat-meta/tests/golden_legacy.rs`, alongside (not replacing) the existing golden
  tests there. Note: the existing property-based tests in that file
  (`schools_disagree_on_mixed_profiles`, `loyalty_school_prefers_one_club_legend`,
  `no_career_tops_all_schools`, `early_career_ranks_low`) call `SCHOOLS[i].score(&axes)` —
  this task's `School::score` signature change (see Slice 2) breaks their compilation, not
  just their semantics. Fixing those call sites to pass the new `&LegacyEvidence` argument
  is **required plumbing, not a scope violation** — but the exact numeric assertions in
  `golden_axes_high_output_career` (`compute_axes` output) must NOT change, since
  `compute_axes` itself is untouched by this task (only `School::score` changes what it
  reads).
- Slice 3 gets a new `goat-save` unit test (temp-dir based, no new deps, following the
  `std::env::temp_dir().join(format!("...{}", std::process::id()))` pattern already used in
  `crates/goat-tui/tests/smoke_stdin.rs`) plus scripted-stdin cases appended to the existing
  `crates/goat-tui/tests/smoke_stdin.rs`.

---

## Slice 1 — Close out the Legacy cadence question (trivial, doc-only)

**Decision (verbatim):** "Legacy cadence stays season-end batch ('theo mùa') — do NOT
change this to per-match."

**Verified**: this is the exact question `docs/DESIGN-CRITIQUE-2026-07-22.md` item 1 raises
("Auto-resolve quietly starves the pantheon of the texture it needs") and that
`tasks/TASK-PLAYTEST-round1-fixes.md`'s ground rule explicitly deferred: *"docs/DESIGN-CRITIQUE-2026-07-22.md
item 1 raises the same architectural tension at a design level — that's a separate, open
question, not this task's job."* Code confirms the architecture is already exactly what
Tùng is confirming: `pc_career_goals`/`pc_career_matches`/all other legacy counters only
mutate inside `Intent::ApplySeasonEndLegacy` (`crates/goat-core/src/state.rs:514-551`),
never inside `Intent::ApplyRoundResult` or `Intent::ApplyMatchResult`.

**No code change.** Append a resolution note to item 1 in
`docs/DESIGN-CRITIQUE-2026-07-22.md`:

```markdown
**Resolved 2026-07-22 (Tùng):** cadence stays season-end batch ("theo mùa") — no change to
`Intent::ApplySeasonEndLegacy` firing only at season boundaries. The remaining texture gap
this item raised (skipped matches collapsing to a number with no beat-level narrative
callback) stays open as a *content/beat-authoring* question, not an *architecture* one —
revisit under beat-library volume work, not a state-model change.
```

- TDD: none (doc-only).
- Playable gate: none — `docs/DESIGN-CRITIQUE-2026-07-22.md` item 1 no longer reads as an
  open question when re-read top to bottom.

**Size: trivial** (one doc edit).

---

## Slice 2 — Genuinely distinct signals per Pantheon school

**Decision (verbatim):** "Invest in genuinely distinct signals per Pantheon school...
Loyalty Traditionalists reading years-at-club/transfer-request count directly rather than a
reweighted Output average; Trophy Cabinet reading raw trophy/title counts; Stats Purists
reading raw attribute/goal/assist numbers; Eye-Test Romantics reading match-rating variance
or 'moments' (big-match performances) rather than season averages."

**Verified — what exists today vs. what's missing:**

- `crates/goat-meta/src/pantheon.rs:22-35` — `School::score(&self, axes: &LegacyAxes)`
  computes a single weighted dot-product over the same 8-element `LegacyAxes` array for
  every school (`weights: [i32; 8]` per school, summing to 100). This *is* the "one vector
  reweighted 4 ways" the design critique (item 2) flagged — confirmed by reading the code,
  not just inferring from the critique text.
- `crates/goat-meta/src/legacy.rs:76-118` (`compute_axes`) already derives each of the 8
  axes from *distinct formulas* over `LegacyEvidence`'s raw counters (e.g. `winning` from
  `league_titles`/`decisive_moments`; `loyalty` from `clubs_served`/`longest_club_tenure`) —
  so the axes aren't literally one Output number either. The actual gap is narrower than
  the critique's headline: **Trophy Cabinet** (weights `[35,15,10,5,20,5,5,5]`, dominated by
  the already-raw `winning`/`accolades` axes) and **Loyalty Traditionalists** (weights
  `[15,10,10,15,10,35,3,2]`, dominated by the already-raw `loyalty` axis) are *already*
  fairly close to reading something distinct — the real problem is **Stats Purists**
  (`output`(40%) + `longevity`(20%) = 60%, where `output` is a *smoothed* avg-over-career +
  best-season-avg blend, not a raw number) and **Eye-Test Romantics** (`decisive`(35%),
  which only counts goals/assists in designated cup-final/decisive fixtures — narrower than
  "big-match performances" broadly).
- **Fields that exist and are directly reusable:** `LegacyEvidence.league_titles`,
  `.player_of_year_wins`, `.clubs_served`, `.longest_club_tenure` (all raw counters,
  `crates/goat-meta/src/legacy.rs:10-27`); `PlayerStore::get_current`/`.snapshot()`
  (`crates/goat-core/src/player.rs:90,154`) for live attribute reads; `derive::ovr(current,
  primary_pos)` (`crates/goat-core/src/derive.rs:106-109`) for a talent/ability number that
  is *structurally different* from Output (ability ceiling vs. match performance).
- **Fields that are missing and must be added:**
  1. A raw "standout/big-match performance" counter — nothing in `WorldState` tracks
     individual match quality beyond the season-aggregate `pc_season_output`
     (`crates/goat-core/src/state.rs:55`) and the narrow `pc_decisive_moments`
     (goals/assists in designated decisive fixtures only,
     `crates/goat-core/src/state.rs:72`).
  2. A raw "career-peak talent" number — `ovr()` is computable on demand but nothing
     persists a career-best value; `WorldState` has no such field.
  3. A raw "transfer request count" — `pc_power_ladder` (`state.rs:86`) tracks the *current*
     escalation rung and resets to 0 on every `AcceptContract`/`ExecuteTransfer`
     (`state.rs:561,587`), so it cannot answer "how many times across a whole career did
     this player ask to leave." No cumulative counter exists.
  4. **Explicitly cut from this slice**: true assist tracking. Grepped
     `crates/goat-match/src/sim.rs`'s `MatchResult`/`ActiveMatchState` (`sim.rs:396-422`) —
     there is no assist concept anywhere in the match engine, only `goals_for`/`goals_against`
     at the team level plus a beat-level `ScoreEvent`. Modeling assists would mean new
     beat-authoring/match-engine plumbing (which beats award an assist, to whom), which is a
     match-engine content task, not a legacy-scoring task. Stats Purists gets `career_best_ovr`
     (talent) + existing raw `career_goals`/`career_matches` instead — genuinely distinct
     from Output without inventing a new gameplay concept. Flag assist-tracking as a future
     `TASK-CORE`/match-engine item if Tùng wants it later.
  5. **Explicitly cut from this slice**: true match-rating variance (Welford/statistical
     spread). It would need either per-match history (violates tiny-save) or a cross-season
     running-variance merge (correct but genuinely fiddly in `Fixed`-point and needs its own
     numbers-design pass). "Moments" (a simple standout-match counter, Tùng's own named
     alternative) delivers the same design goal — a countable big-match signal distinct from
     season averages — with none of that complexity.

### 2.1 — New `WorldState` fields (`crates/goat-core/src/state.rs`)

Add, next to the existing Phase 7 legacy-evidence block (`state.rs:66-76`):

```rust
// ── Pantheon raw-signal evidence (Design round 1) ─────────────────────────
/// Cumulative count of matches with pc_output >= STANDOUT_OUTPUT_THRESHOLD, career-wide.
/// Feeds the Eye-Test Romantics school directly (raw "moments", not a season average).
pub pc_career_standout_matches: u32,
/// This season's standout-match count, live — folded into pc_career_standout_matches
/// only at ApplySeasonEndLegacy (mirrors pc_season_goals -> pc_career_goals).
pub pc_season_standout_matches: u32,
/// Career-peak OVR (derive::ovr at the player's primary position), checked once per
/// season end. Feeds Stats Purists directly — a talent/ability number, structurally
/// different from the Output axis (match performance, not attribute ceiling).
pub pc_career_best_ovr: i32,
/// Cumulative count of AgitateForTransfer escalations, career-wide — unlike
/// pc_power_ladder (current rung, resets on contract/transfer), this never resets.
/// Feeds Loyalty Traditionalists directly (raw request count, not a clubs_served penalty).
pub pc_career_transfer_requests: u32,
/// This season's transfer-request count, live — folded into pc_career_transfer_requests
/// only at ApplySeasonEndLegacy.
pub pc_season_transfer_requests: u32,
```

Initialize all 5 to `0` in `WorldState::new()` (`state.rs:160-219`).

**Reducer wiring:**

- `Intent::StartSeason` (`state.rs:776-798`): reset `pc_season_standout_matches = 0` and
  `pc_season_transfer_requests = 0` alongside the existing `pc_season_goals = 0` etc.
- `Intent::ApplyRoundResult` (`state.rs:800-...`): after the existing
  `state.pc_season_output += pc_output;` (`state.rs:837`), add:
  ```rust
  if pc_output >= STANDOUT_OUTPUT_THRESHOLD {
      state.pc_season_standout_matches += 1;
  }
  ```
  (import `STANDOUT_OUTPUT_THRESHOLD` from `tuning.rs`, see 2.2).
- `Intent::AgitateForTransfer` (`state.rs:565-570`): add
  `state.pc_season_transfer_requests += 1;` alongside the existing
  `state.pc_power_ladder = (...).min(3);` line. This is a live, in-season counter (like the
  ladder itself) — only its season-end fold into the *persisted, legacy-facing* field is
  gated to `ApplySeasonEndLegacy`, so the Pantheon screen never shows a mid-season-moving
  number (preserves decision 1 / Slice 2 of `TASK-PLAYTEST-round1-fixes.md`).
- `Intent::ApplySeasonEndLegacy` (`state.rs:514-551`): add two new intent fields
  `season_standout_matches: u32` and `season_transfer_requests: u32` (mirroring the existing
  explicit `season_goals`/`season_matches` pattern — the reducer trusts caller-supplied
  season totals rather than reading `pc_season_*` directly, matching every other field in
  this intent). Fold in:
  ```rust
  state.pc_career_standout_matches += season_standout_matches;
  state.pc_career_transfer_requests += season_transfer_requests;
  // Career-peak OVR: computed here, not staged — a "peak so far" check is naturally
  // season-cadenced, no per-match staging needed.
  if let Some(pc_id) = state.pc_player_id {
      let view = state.players.snapshot(pc_id);
      let current_ovr = crate::derive::ovr(&view.current, state.players.get_primary_position(pc_id)).to_int();
      state.pc_career_best_ovr = state.pc_career_best_ovr.max(current_ovr.clamp(0, 100));
  }
  ```
  (new imports needed in `state.rs`: `crate::derive::ovr` — not currently imported there.)
- Caller side (`crates/goat-tui/src/main.rs` wherever `Intent::ApplySeasonEndLegacy` is
  constructed, and the equivalent in `crates/goat-bridge/src/api.rs`): pass
  `season_standout_matches: state.pc_season_standout_matches` and
  `season_transfer_requests: state.pc_season_transfer_requests` through.

### 2.2 — New tuning constant (`crates/goat-core/src/tuning.rs`)

```rust
/// A single-match output score at/above this counts as a "standout" performance for the
/// Eye-Test Romantics' raw "moments" signal (Design round 1, decision 2). First-pass
/// number — tune later per TASK-TUNE convention, not guessed against real playtest data.
pub const STANDOUT_OUTPUT_THRESHOLD: i32 = 78;
```

### 2.3 — `LegacyEvidence` gets 3 new fields (`crates/goat-meta/src/legacy.rs:8-27`)

```rust
pub career_standout_matches: u32,
pub career_best_ovr: i32,
pub career_transfer_requests: u32,
```

`LegacyEvidence` already derives `Default`, so this is additive and non-breaking for any
test/call site that already uses `..Default::default()`. It is **not** additive for
existing exhaustive struct literals — this breaks compilation (not just semantics) at:

- `crates/goat-tui/src/main.rs:894-907` (`build_legacy_evidence`) — add the 3 new fields,
  reading `state.pc_career_standout_matches`, `state.pc_career_best_ovr`,
  `state.pc_career_transfer_requests`.
- `crates/goat-bridge/src/api.rs:1222-1233` (`get_legacy`'s `LegacyEvidence` literal) — same
  3 fields, same source.
- `crates/goat-meta/tests/golden_legacy.rs:8-49` (4 struct literals:
  `high_output_low_trophies`, `trophy_machine_low_output`, `one_club_legend`, and the
  inline literal in `early_career_ranks_low`) — add `..Default::default()` to each (all 4
  new fields default to 0, which doesn't perturb any *existing* frozen assertion in
  `golden_axes_high_output_career`, since `compute_axes` never reads them).

`compute_axes` itself (`legacy.rs:76-118`) is **unchanged** — it still only produces the 8
`LegacyAxes` values, exactly as before. The 3 new fields are read only by the new
`School::score` logic (2.4), not by axis computation.

### 2.4 — `School` scoring rework (`crates/goat-meta/src/pantheon.rs`)

Replace the pure weighted-dot-product `School::score` with a blend: each school's
`raw_signal` variant reads one new-or-existing *raw* counter directly (0-100, clamped), then
blends it with the existing composite-axes score at a per-school ratio. This keeps the
existing `weights: [i32; 8]` machinery (still meaningful — it's the "everything else" 20-45%
of the score) while giving each school a headline number nothing else shares.

```rust
#[derive(Debug, Clone, Copy)]
pub enum RawSignal {
    /// Composite axes only — already close to a raw trophy/title read (winning +
    /// accolades = 50% of Trophy Cabinet's existing weights), smallest delta of the four.
    None,
    /// Eye-Test Romantics: count of standout/big-match performances (2.1/2.2).
    StandoutMatches,
    /// Stats Purists: career-peak OVR — talent ceiling, not match-performance average.
    BestOvr,
    /// Loyalty Traditionalists: years at club minus transfer-request count, read directly.
    LoyaltyRaw,
}

pub struct School {
    pub name: &'static str,
    pub tagline: &'static str,
    pub weights: [i32; 8],       // unchanged meaning — the composite-axes blend
    pub raw_signal: RawSignal,   // NEW
    pub raw_weight_pct: i32,     // NEW, 0-100: how much raw_signal dominates over composite
}

impl School {
    pub fn score(&self, ev: &LegacyEvidence, axes: &LegacyAxes) -> Fixed {
        let arr = axes.as_array();
        let composite_pct: i32 = self.weights.iter().zip(arr.iter())
            .map(|(&w, ax)| w * ax.to_int()).sum::<i32>() / 100; // unchanged from today

        let raw_pct: i32 = match self.raw_signal {
            RawSignal::None => composite_pct,
            RawSignal::StandoutMatches => (ev.career_standout_matches as i32 * 3).min(100),
            RawSignal::BestOvr => ev.career_best_ovr.clamp(0, 100),
            RawSignal::LoyaltyRaw => ((ev.longest_club_tenure as i32 * 8)
                - (ev.career_transfer_requests as i32 * 15)).clamp(0, 100),
        };

        let blended = (raw_pct * self.raw_weight_pct
            + composite_pct * (100 - self.raw_weight_pct)) / 100;
        Fixed::from_int(blended.clamp(0, 100))
    }
}

pub const SCHOOLS: [School; NUM_SCHOOLS] = [
    School { name: "The Trophy Cabinet", tagline: "...", weights: [35,15,10,5,20,5,5,5],
             raw_signal: RawSignal::None, raw_weight_pct: 0 },
    School { name: "The Eye-Test Romantics", tagline: "...", weights: [10,20,15,5,35,5,7,3],
             raw_signal: RawSignal::StandoutMatches, raw_weight_pct: 65 },
    School { name: "The Stats Purists", tagline: "...", weights: [10,15,40,20,5,5,3,2],
             raw_signal: RawSignal::BestOvr, raw_weight_pct: 65 },
    School { name: "The Loyalty Traditionalists", tagline: "...", weights: [15,10,10,15,10,35,3,2],
             raw_signal: RawSignal::LoyaltyRaw, raw_weight_pct: 55 },
];
```

(keep existing `name`/`tagline`/`weights` strings and numbers verbatim — only the two new
fields and the `score` body change.) `raw_weight_pct` values are a first-pass design choice
(not maxed to 100) so a genuine all-rounder still shows up reasonably across all 4 schools —
only a *lopsided* career (huge standout count but mediocre trophies, say) should swing
hard on one school and not others. Tune later per TASK-TUNE if playtesting shows the blend
too timid or too extreme.

`rank_in_canon`/`all_rankings` (`pantheon.rs:143-165`) both need `ev: &LegacyEvidence`
threaded through as a new first parameter (PC's evidence — used both for `school.score` on
the PC and, per-entry, on each canon great, see 2.5):

```rust
pub fn rank_in_canon(pc_ev: &LegacyEvidence, pc_axes: &LegacyAxes, school_idx: usize) -> (usize, usize) {
    let school = &SCHOOLS[school_idx];
    let pc_score = school.score(pc_ev, pc_axes);
    let rank = 1 + CANON.iter().filter(|g| school.score(&g.evidence, &g.axes) > pc_score).count();
    (rank, NUM_CANON + 1)
}

pub fn all_rankings(pc_ev: &LegacyEvidence, pc_axes: &LegacyAxes) -> [(Fixed, usize, usize); NUM_SCHOOLS] {
    // same shape as today, calls rank_in_canon(pc_ev, pc_axes, i) and SCHOOLS[i].score(pc_ev, pc_axes)
}
```

**Call-site ripple** (all confirmed by grep, all one-line signature updates — pass the
already-in-scope `ev`/`build_legacy_evidence(...)` result alongside `axes`):

- `crates/goat-tui/src/main.rs:917,1084,1360` — `all_rankings(&axes)` → `all_rankings(&ev, &axes)`
  (an `ev`/`build_legacy_evidence(state)` value is already constructed at each of these 3
  call sites before the `all_rankings` call — confirm and reuse, don't reconstruct).
- `crates/goat-bridge/src/api.rs:1235` — same change, `ev` is already in scope (2.3).
- `crates/goat-meta/tests/golden_legacy.rs` — every `SCHOOLS[i].score(&axes)` call (5
  occurrences) becomes `SCHOOLS[i].score(&ev, &axes)`, and `all_rankings(&axes)` (1
  occurrence, `early_career_ranks_low`) becomes `all_rankings(&ev, &axes)`.

### 2.5 — `PastGreat` gets synthetic raw evidence (`crates/goat-meta/src/pantheon.rs:64-139`)

Add an `evidence: LegacyEvidence` field to `PastGreat` (alongside the existing `axes:
LegacyAxes`) and extend the `great!` macro with 4 new trailing params
(`$tenure, $transfer_req, $standout, $best_ovr`) constructing
`LegacyEvidence { longest_club_tenure: $tenure, career_transfer_requests: $transfer_req,
career_standout_matches: $standout, career_best_ovr: $best_ovr, ..Default::default() }`.
Hand-authored to stay consistent with each great's existing flavor-text archetype (doc
comment at `pantheon.rs:96-102`) — verified against the blend formula in 2.4 to confirm it
actually produces divergent school rankings (Andersen/Okonkwo top Loyalty; Pires/Muñoz top
Eye-Test; Cavalcanti/Keane top Stats Purists; existing axis-driven Trophy Cabinet ranking
untouched):

| Great | tenure | transfer_req | standout | best_ovr |
|---|---|---|---|---|
| Cavalcanti | 14 | 2 | 40 | 92 |
| Van der Berg | 16 | 1 | 30 | 88 |
| Dominguez | 4 | 6 | 12 | 78 |
| Petrov | 8 | 3 | 18 | 80 |
| Keane | 10 | 1 | 15 | 90 |
| Nakamura | 12 | 2 | 14 | 87 |
| Pires | 6 | 4 | 48 | 75 |
| Muñoz | 7 | 3 | 40 | 79 |
| Andersen | 15 | 0 | 20 | 76 |
| Okonkwo | 14 | 0 | 22 | 78 |

- TDD: new tests in `crates/goat-meta/tests/golden_legacy.rs` — at minimum,
  `eye_test_school_prefers_moment_makers` (Pires/Muñoz-shaped evidence outranks a
  Keane/Nakamura-shaped one on school idx 1) and
  `stats_purist_school_prefers_high_ovr_over_trophies` (a high-`career_best_ovr`,
  low-`league_titles` evidence outranks a high-`league_titles`, low-`career_best_ovr` one on
  school idx 2), following the existing `schools_disagree_on_mixed_profiles` pattern
  (inequality assertions, not frozen exact values, since these are new behaviors).
- Golden test for the new `WorldState`/reducer wiring: in `crates/goat-core/`'s existing
  test location for `ApplySeasonEndLegacy` (or a new test near it), assert
  `pc_career_standout_matches`/`pc_career_transfer_requests` fold correctly from
  season-staged values and `pc_career_best_ovr` is a running max across two consecutive
  `ApplySeasonEndLegacy` calls (second call with lower current OVR does not decrease it).
- Save round-trip: bump `goat-save::save::VERSION` to `9`, append the 5 new `WorldState`
  fields (3 persisted `pc_career_*`, plus the 2 live `pc_season_*` staging fields — these
  need saving too, exactly like `pc_season_goals` already is, so a mid-season save/load
  doesn't lose in-progress standout/transfer-request counts) to `SaveData`/`to_bytes`/
  `from_bytes` with `.unwrap_or(0)` defaults, plus a new backward-compat test
  `old_v8_save_without_pantheon_signals_defaults_to_zero` mirroring the existing
  `old_v7_save_without_lifestyle_score_defaults_to_balanced` pattern.
- Playable gate: `cargo run -p goat-tui` → play a full season with several high-output
  matches and at least one transfer agitation → `G` (Legacy) → the school rankings/scores
  visibly diverge more than before between a "moments" performance and a "trophies" one
  (qualitative check, not exact-number).

**Known pre-existing quirk, not this slice's job to fix**: `pc_longest_club_tenure`
(`state.rs:76`) currently increments every season regardless of club changes (see
`state.rs:546`, unconditional `+= 1` inside `ApplySeasonEndLegacy`) — its doc comment says
"longest consecutive tenure at one club" but the code tracks total seasons played,
full stop. This task reads that field as-is for the Loyalty raw signal (matching its
*current* actual behavior, not its doc comment's intent) — fixing the semantic drift is a
separate bug, flag for a future round.

**Size: large** (touches `goat-core` state+tuning, `goat-meta` legacy+pantheon+tests,
`goat-tui` main.rs, `goat-bridge` api.rs, `goat-save` format+tests — but every touch point
above is a small, mechanical, fully-specified change; no open design decisions left for Dev
to improvise).

---

## Slice 3 — Multiple save slots + listing

**Decision (verbatim):** "Add multiple save slots + checkpoint support ('nhiều')... Minimum
bar: named/numbered slots instead of one silent overwrite, and a way to list existing
saves. A pre-match checkpoint... is a nice-to-have if it fits cleanly, not a hard
requirement."

**Verified — current state**: exactly one hardcoded path, `const SAVE_PATH: &str =
"goat.sav"` (`crates/goat-tui/src/main.rs:44`). `L`/`LOAD` at the title screen
(`main.rs:78-91`) loads it unconditionally; `Z` (in-game, both the main loop `main.rs:494-496`
and the pre-season menu `main.rs:408-411`) calls `run_save` (`main.rs:1432-1441`), which
always overwrites it with no confirmation — this is exactly Persona 10's "Save Scummer"
complaint in `docs/PERSONA-PLAYTEST-2026-07-22.md`.

**Verified — why this doesn't need a format change**: `save_to_file`/`load_from_file`
(`crates/goat-save/src/save.rs:262-271`) already take `impl AsRef<Path>`, not a fixed
constant — the single-file limitation is entirely a `goat-tui` (and, symmetrically,
`goat-bridge`) call-site choice, not a `goat-save` limitation.

### 3.1 — Slot scheme

Numbered slots (1–9), not free-text-named ones — avoids filename sanitization/path-traversal
surface for a text UI that would otherwise need to validate arbitrary user-typed strings.
Each slot's own save data (`pc_name`, already in `SaveData`) is displayed in the picker, so
the player still sees "their" name against the slot without the TUI needing to manage
free-text filenames.

```rust
// crates/goat-tui/src/main.rs
const SAVE_DIR: &str = "saves";
const NUM_SLOTS: u8 = 9;
```

At `main()` startup (`main.rs:47`), add `std::fs::create_dir_all(SAVE_DIR).ok();` (tolerant
of failure the same way the existing `beats.json` fallback read already is — non-fatal, the
save/load calls will surface their own IO errors if the directory genuinely can't be
created).

### 3.2 — `goat-save` gets a shared slot-listing helper

New, in `crates/goat-save/src/save.rs` (pure save-data logic — no game-loop concerns, so it
belongs in this crate, reusable later by `goat-bridge` for a Flutter multi-slot UI even
though this task doesn't wire that up, see "Out of scope"):

```rust
pub struct SaveSlotSummary {
    pub slot: u8,
    pub occupied: bool,
    /// Empty string / 0 when `occupied` is false.
    pub pc_name: String,
    pub season_number: u32,
    pub pc_age_weeks: u32,
}

pub fn slot_path(dir: impl AsRef<Path>, slot: u8) -> std::path::PathBuf {
    dir.as_ref().join(format!("slot-{slot}.sav"))
}

/// List slots 1..=num_slots in `dir`. Reads every existing file to summarize it — cheap:
/// each save is a few hundred bytes (tiny-save principle), so reading up to `num_slots` of
/// them stays well inside CALENDAR.md's NFR-02 "load under 1s" budget.
pub fn list_slots(dir: impl AsRef<Path>, num_slots: u8) -> Vec<SaveSlotSummary> {
    (1..=num_slots).map(|slot| {
        match load_from_file(slot_path(&dir, slot)) {
            Ok(data) => SaveSlotSummary {
                slot, occupied: true, pc_name: data.pc_name,
                season_number: data.season_number, pc_age_weeks: data.pc_age_weeks,
            },
            Err(_) => SaveSlotSummary {
                slot, occupied: false, pc_name: String::new(),
                season_number: 0, pc_age_weeks: 0,
            },
        }
    }).collect()
}
```

- TDD: new test file or addition to `crates/goat-save/tests/save_roundtrip.rs` —
  `list_slots` on an empty scratch dir returns `NUM_SLOTS` entries all `occupied: false`;
  after writing to `slot_path(dir, 3)`, `list_slots` reports slot 3 occupied with the
  correct `pc_name`/`season_number` and every other slot still unoccupied. Use the existing
  `std::env::temp_dir().join(format!("...{}", std::process::id()))` scratch-dir pattern.

### 3.3 — `goat-tui` UX

New shared render/prompt helpers (near `run_save`, `main.rs:1430-1441`):

- `render_slot_picker(out: &mut impl Write, slots: &[SaveSlotSummary])` — one line per slot,
  e.g. `"  [3] Alex Turner — S4, age 24"` when occupied, `"  [3] <empty>"` when not.
- `prompt_slot_choice(lines, out) -> Option<u8>` — reads a single digit `1..=NUM_SLOTS`,
  `Q`/blank cancels (`None`), reprompts on anything else (reuses the Slice 6-style
  reprompt-on-invalid-input convention from `TASK-PLAYTEST-round1-fixes.md`, not a silent
  `_ => return`).

**Load** (`main.rs:78-91`, the `"L" | "LOAD"` arm): render the picker, prompt for a slot; if
occupied, `load_from_file(slot_path(SAVE_DIR, slot))` → existing flow unchanged; if empty,
print `"  Slot {slot} is empty."` and return to the title menu (no crash, no silent no-op).

**Save** (`run_save`, `main.rs:1432-1441`, called from both `Z` sites): render the picker,
prompt for a slot. If the chosen slot is **occupied**, print
`"  Slot {slot} has a save ({pc_name}, S{season_number}). Overwrite? [Y/N]"` and require an
explicit `Y` before writing — **this is the concrete fix for Persona 10's silent-overwrite
complaint**, not just a side effect of adding slots. If empty, save directly with
`"  Saved to slot {slot}."`.

### 3.4 — Ripple: two existing `smoke_stdin.rs` tests seed `goat.sav` directly

**Verified**: `crates/goat-tui/tests/smoke_stdin.rs` has two test-seed helpers,
`seed_save_at_age_weeks` (`smoke_stdin.rs:292-320`) and `seed_save_at_season_end`
(`smoke_stdin.rs:357-390`), both writing to `dir.join("goat.sav")` and both consumed by
tests that script a bare `"L\n..."` (`hard_retirement_age_is_forced_not_offered`,
`viewing_legacy_twice_at_season_end_does_not_double_credit_career_totals`). Both need two
mechanical updates or they'll fail to compile/pass once Slice 3 lands:
1. `dir.join("goat.sav")` → `goat_save::slot_path(dir.join("saves"), 1)` (create the
   `saves` subdir first, e.g. `std::fs::create_dir_all(dir.join("saves"))`).
2. The scripted input gains a slot-number keystroke after `L`: `"L\n1\n..."` instead of
   `"L\n..."`.

### 3.5 — Checkpoint (nice-to-have, scoped OUT)

Not implemented this round. A true pre-match checkpoint (save → try a beat → reload → try a
different choice) needs either (a) an implicit auto-slot the player doesn't manage, written
before every `P`/`K` round resolution, or (b) exposing the mid-match `ActiveMatchState`
itself to save/load — (b) is a real format change (`ActiveMatchState` has beat-generation
state that isn't in `SaveData` today) and (a) adds a write on every single round regardless
of whether the player ever uses it, for a workflow (save-scumming a specific beat choice)
Tùng flagged as a *complaint to fix* (Persona 10), not a mechanic to actively support.
Slot-based save/load already lets a player manually checkpoint by saving to a spare slot
before `P`/`K` and reloading it if the beat goes badly — good enough for the "nice-to-have,
not required" bar Tùng set. Revisit only if a future playtest round specifically asks for
frictionless mid-match retry.

**Size: medium** (mostly `goat-tui` UX + a small `goat-save` addition + two existing test
updates; no format version bump, no cross-crate ripple beyond the two files above).

---

## Out of scope (do not touch in this task)

- **`goat-bridge`/Flutter multi-slot wiring.** `save_game(path)`/`load_game(path)`
  (`crates/goat-bridge/src/api.rs:1512-1533`) already accept an arbitrary path — the Flutter
  client can already build a multi-slot UI today by choosing its own paths, without any
  core change. Exposing `goat_save::list_slots` as a new FFI function would need a
  `flutter_rust_bridge_codegen` regeneration of `frb_generated.rs` (2606 lines, "do not
  hand-edit" per `CLAUDE.md`) — a toolchain step outside this repo's normal `cargo
  test`/`cargo run` dev loop. `list_slots` is public in `goat-save` specifically so a future
  bridge task can wrap it without re-deriving the logic; this task does not touch
  `goat-bridge` at all for Slice 3.
- **Assist tracking** (Slice 2, cut — see 2.1 item 4). Would require new match-engine/beat
  plumbing, not a legacy-scoring change.
- **True match-rating variance/statistical spread** (Slice 2, cut — see 2.1 item 5).
  "Moments" (standout-match count) delivers the same design goal without a Fixed-point
  variance-merge algorithm.
- **Pre-match checkpoint / mid-match save-scum support** (Slice 3, cut — see 3.5).
- **Fixing `pc_longest_club_tenure`'s semantic drift** (documented as "longest consecutive
  tenure at one club," actually tracks total seasons played regardless of club changes —
  see the "Known pre-existing quirk" note under Slice 2). Read as-is; not this task's bug
  to fix.
- **Any further Pantheon-adjacent design question not named in Tùng's 3 decisions** —
  e.g. `docs/DESIGN-CRITIQUE-2026-07-22.md` items 3 (Output formula inconsistency), 4
  (emergent-rival narrative), and 5 (background-world sameness) remain open, untouched,
  undecided. Do not fold them into this task.
- **Number tuning beyond the first-pass values specified above** (`STANDOUT_OUTPUT_THRESHOLD
  = 78`, `raw_weight_pct` values, the loyalty-raw `*8`/`*15` scale factors, the 10-row
  synthetic-evidence table) — implement exactly as specified; further tuning is
  `TASK-TUNE`'s job once real playtest data exists.

## Definition of done

1. `cargo test --workspace` green, including all pre-existing golden tests unchanged
   (`golden_axes_high_output_career`'s exact numbers untouched) and all newly-added tests
   passing.
2. `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean for
   every file this task touches.
3. `goat-save::save::VERSION` is `9`; a save written by the pre-this-task binary still loads
   cleanly (covered by the new backward-compat test in 2.4's TDD list).
4. `crates/goat-tui/tests/smoke_stdin.rs`'s two existing save-seeding tests updated per 3.4
   and still passing; new Slice 3 slot-picker cases added.
5. Every slice's playable gate works via `cargo run -p goat-tui`.
6. No new dependencies, no floats in sim state/logic, no unsafe.
7. Short summary per slice: what changed, which of Tùng's 3 decisions it resolves.
