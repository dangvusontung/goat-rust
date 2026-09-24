# TASK — BL5.2: Decisive moments (dynamic detection, v1)

Prereq: none. This is v1 of a staged rollout — v2 (league-table-aware match
tension) is explicitly parked, see "Out of scope" below.

Read first: `crates/goat-match/src/sim.rs` (`MomentSummary`, `ActiveMatchState`,
`advance_beat`, `finish_match`), `crates/goat-match/src/beats.rs` (`ScoreEvent`,
`BeatChoice`'s `success`/`failure` `Outcome`s), `crates/goat-core/src/state.rs`
(`pc_decisive_moments`, `Intent::ApplySeasonEndLegacy`'s `decisive_moments: u32`
param, its handler around line 792), `crates/goat-meta/src/legacy.rs`
(`LegacyEvidence::decisive_moments`, the 3 sites it feeds), `crates/goat-calendar/
src/clock.rs` (`FixtureImportance`), `crates/goat-tui/src/orbit_fixtures.rs`
(where fixtures — and today, always `FixtureImportance::League` — get built).

## Origin

Raised by Tùng 2026-07-22 (round2 doc's "Parked for a future design round"),
picked up 2026-08-03 same day as BL5.1. Verified then: `pc_decisive_moments`
and `ApplySeasonEndLegacy`'s `decisive_moments` param already exist and already
feed `legacy.rs`'s Legacy formula (3 sites, all `... + decisive_moments × k`,
`.clamp`/`.min(100)`) — every call site just hardcodes `decisive_moments: 0`
today (confirmed: `state.rs:1623` is one such site). This task wires it to a
real, computed value.

**This doc is the product of an extended live design conversation, not a
first-pass guess** — several early ideas were explored and explicitly reversed;
the "Decision" section below is the final, settled shape. Notable reversals,
recorded so Kimi doesn't re-litigate them:
- First proposal: detect decisive moments via a curated `"key"` tag in
  `beats.json` (7 existing situations: `penalty`, `last_minute_chance`,
  `comeback_moment`, `wonder_strike`, `one_on_one`, `one_on_one_wide`,
  `crucial_tackle`). **Rejected** — verified these 7 skew 6-attacking/1-defensive,
  which would systematically under-count decisive moments for CB/DM/CM players
  (they'd usually face situations they're statistically weak at). Authoring 3
  new defend-flavored `"key"` situations was drafted as a fix, then **also
  rejected** in favor of the dynamic approach below, which needs no new content
  and has no position bias by construction.
- A "committing a professional foul always counts as decisive" branch was
  proposed for `crucial_tackle`, then **rejected**: verified in `advance_beat`
  that contest resolution (which sets `goal_event`) runs *before* the separate,
  independent foul/card roll — so "fouled and still conceded" is a real
  reachable state. Fix: gate purely on `goal_event`, not on whether a card was
  given.

## Verified: current state (2026-08-03)

- `MomentSummary` (`crates/goat-match/src/sim.rs`) currently has: `beat_id`,
  `minute`, `choice_idx`, `success`, `setup_text`, `outcome_text`, `goal_event:
  Option<ScoreEvent>`. It does **not** currently record the live score
  (`goals_for`/`goals_against`) at the moment the beat resolved — needed for
  v1, see Decision below.
- Each `BeatChoice` has independent `success: Outcome` and `failure: Outcome`
  branches, each with its own `score_event: Option<ScoreEvent>` — i.e., whether
  a given beat is "stakes-bearing" (could plausibly end in a goal either way)
  is already knowable per-choice from existing data, with no need for a
  curated tag.
- `advance_beat`'s order of operations: contest resolves (sets `success` and
  therefore `goal_event`) **first**; the foul/card roll (`foul_risk_for_situation`
  → `resolve_card`) happens **after**, independently. A red card at a beat whose
  contest still failed (conceding a goal) is a real, reachable combination —
  do not treat "a card was given" as itself evidence of a successful defensive
  stand.
- `FixtureImportance` (`goat-calendar::clock`) already has the full ordinal
  scale needed: `DeadRubber, League, Derby, ContinentalTier3, DomesticCupEarly,
  ContinentalTier2, DomesticCupLate, ContinentalTier1, DomesticCupFinal,
  ContinentalTier1Final`. `pc_result` (win/draw/loss, already threaded through
  `Intent::ApplyRoundResult`) is the other input v1 needs.
- **Not yet wired**: `crates/goat-tui/src/orbit_fixtures.rs` hardcodes every
  league fixture as `importance: FixtureImportance::League` (line ~43) — no
  distinction today between an ordinary league match and one with real stakes.
  v1 only needs League vs. Cup vs. Continental vs. their respective Finals
  (data already available at fixture-build time from which competition/round
  it is) — NOT the dynamic table-standings tension described below.

## Decision (confirmed scope for v1)

### 1. Per-moment "decisive candidate" detection — pure function, `goat-match`

Add 2 fields to `MomentSummary`: `goals_for_before: u8`, `goals_against_before:
u8` — the live score **immediately before** this beat's own outcome is applied
(so a beat that itself scores the tying goal is judged against the score it
broke, not the score it created). Populate these in `advance_beat` at the same
point `MomentSummary` is currently constructed, from `ms.goals_for`/
`ms.goals_against` before this beat's `score_event` is applied to them.

A beat is a decisive candidate — implement as one pure function taking a
`&MomentSummary`, e.g. `fn is_decisive(m: &MomentSummary) -> bool` in
`goat-match` — when **all** of:
1. Either the taken choice's `success` or `failure` outcome carries a
   `score_event` (i.e., this was a stakes-bearing beat — confirm this is
   knowable from `MomentSummary` alone, or thread through one more field if
   not; flag if it turns out `MomentSummary` needs the *choice's* branch
   score-events, not just the resolved one, to answer this).
2. `m.minute >= 80` (late-game; exact cutoff is a placeholder — pick a named
   `tuning` constant, not a magic number, per this project's convention).
3. Score closeness at that moment: `(m.goals_for_before as i32 -
   m.goals_against_before as i32).abs() <= 1`.
4. The actual outcome either scored (`m.goal_event` is `GoalFor`/`AssistFor`
   and `m.success`) or prevented a threatened concession (the beat's failure
   branch carries `GoalAgainst`, but `m.goal_event` is not `GoalAgainst` —
   i.e. it didn't happen this time). Whether a card was given on this beat is
   irrelevant to this check (see the rejected-foul-branch note above).

**Verified against two concrete scenarios Tùng supplied — both must pass**:
- Trailing 0-1 at minute 82, score twice to win 2-1: both goals count (each
  judged against the score *before* it — 0-1 then 1-1, both within the
  closeness threshold).
- Leading 1-0, a last-ditch tackle at minute ~85 draws a red card but the
  defense holds (no goal conceded): counts, regardless of the card.

Unit-test this as a pure golden-seed-style test in `crates/goat-match/tests/`
with hand-built `MomentSummary` fixtures — no live match, world, or save
needed. This mirrors how `resolve_contest`/`injury_prob` are already tested in
isolation.

### 2. Aggregation into `pc_decisive_moments` / Legacy weighting — `goat-core`

Per match, count decisive candidates from its `moments: Vec<MomentSummary>`
(already collected in `MatchResult`), weight each by a **static** match-value
coefficient:
- `FixtureImportance` → numeric coefficient. Placeholder scale (confirm/adjust
  with Tùng, or pick sane defaults and flag for sign-off): `DeadRubber=0.5,
  League=1.0, Derby=1.2, ContinentalTier3=1.3, DomesticCupEarly=1.3,
  ContinentalTier2=1.5, DomesticCupLate=1.6, ContinentalTier1=1.8,
  DomesticCupFinal=2.0, ContinentalTier1Final=2.0`.
- Result multiplier: win=1.0, draw=0.5, loss=0.0 (a decisive moment in a match
  you still lost contributes nothing to this axis — open question flagged to
  Tùng earlier, this is the "lean simple" default; flag if this feels wrong
  once implemented against a real career-sim run).

This produces a weighted contribution added into `pc_decisive_moments` (still
a single accumulating `u32`-shaped value — round the weighted sum sensibly,
document the rounding rule). Wire this through `Intent::ApplyRoundResult`
(where `pc_goals`/`pc_assists` already flow through per round) similarly to
those, then into `Intent::ApplySeasonEndLegacy`'s existing `decisive_moments`
param exactly like BL5.1 wired `pc_assists` — same shape, same call sites
(`main.rs`, `career_sim.rs`, `goat-bridge`).

### 3. `orbit_fixtures.rs`: real (static) `FixtureImportance`

Fix the hardcoded `FixtureImportance::League` to reflect which competition/
round a fixture actually belongs to (League vs. Domestic Cup vs. Continental,
and their Final rounds) using data already available when fixtures are built.
Do **not** attempt any table-standings-based tension detection here (see Out
of scope).

## Explicitly out of scope (v1) — parked for later rounds, not dropped

- **v2 — League-table-aware match tension.** Tùng's ask: an ordinary League
  fixture late in a still-live title race or relegation battle should carry a
  higher coefficient than one that's mathematically dead, even though both are
  `FixtureImportance::League` today. This needs live table-standings data
  (points, gap to top/bottom) from `goat-world`'s season/table system — a real
  sub-system, not a formula tweak. Do not attempt in v1.
- **"Clutch index" as a new, separate per-match stat.** Tùng raised this as a
  possibly-distinct concept from the weighted `pc_decisive_moments` accumulator
  above (open questions never fully resolved in the design conversation:
  is it a new accumulating stat or per-match-only display value?). **Not part
  of v1** — v1 only wires the existing `pc_decisive_moments`/Legacy path. If
  this is wanted later, it needs its own design round.
- **3 new `"key"`-tagged `beats.json` situations for position balance** — this
  was the position-bias fix originally proposed, superseded by the dynamic
  per-moment formula (§1 above), which has no position bias by construction.
  Do not add new beats.json content for this task.
- No changes to `resolve_contest`, `advance_beat`'s core contest/foul logic,
  match resolution math, or any existing golden test's frozen values.

## Definition of done

Follow `CLAUDE.md`'s standing Definition of Done:
1. `cargo test --workspace` green, all pre-existing golden tests unchanged
   (including the same pre-existing `smoke_stdin` baseline failures noted in
   the last two tasks — not this task's concern, don't fix them).
2. `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D
   warnings` clean.
3. New golden-seed-style tests for `is_decisive` (or equivalent) covering
   both of Tùng's verified scenarios above, plus at least one negative case
   (blowout scoreline, or too-early minute) proving it correctly does NOT
   count.
4. Playable gate: state the exact `cargo run -p goat-tui`/`career-sim` flow
   showing a nonzero `pc_decisive_moments` after a match with a late, close
   goal — mirror how BL5.1 verified assists appear via `career-sim
   --match-beats`.
5. No new dependencies, no floats in sim (fixed-point/integer math only, per
   existing convention), no unsafe, no I/O in core.
6. Short summary: what changed, and explicitly restate what was deliberately
   left for v2 (don't let it read as "done" when the table-tension/clutch-index
   ideas are still open).

If anything above turns out to be wrong once you're reading the real code —
especially point 1's "score_event knowable from `MomentSummary` alone" claim,
which may need an extra field — stop and report back rather than improvising.
This doc reflects a long back-and-forth design conversation on 2026-08-03; it
is more likely to have a subtle gap than the shorter BL5.1/auto-advance docs
were.
