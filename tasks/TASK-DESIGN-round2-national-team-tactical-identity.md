# TASK DESIGN ROUND 2, DOC B — National-team layer + per-team tactical identity

Prereq: none strictly required, but **pairs naturally with**
`tasks/TASK-DESIGN-round2-world-genesis-scaleup.md` (Doc A) — see "Relationship to Doc A"
below. This is item 4 of the scope Tùng approved verbally on 2026-07-22, written up
separately because it is the largest single item and touches a completely different part of
the codebase than items 1-3.

Read first: `docs/MAIN.md` §4.1 (Nationality as the difficulty/story dial), §5.2 (Roles &
Multi-Role — the `role_rating`/familiarity-tier machinery this task must extend, not
replace), §7.4 (The Emergent Rival — the closest existing precedent for "a generated axis
checked against the player, non-blocking"), `crates/goat-core/src/roles.rs` (role/familiarity
definitions), `crates/goat-core/src/derive.rs` (`role_rating`, `ovr` — the existing
weighting-lens pattern this task's design leans on), `crates/goat-meta/src/pantheon.rs`
(`School::score` — the *second* existing instance of the same weighting-lens pattern, see
below), `docs/CALENDAR.md` (international-break windows — the existing timing hook this task
plugs into).

## Ground rules for this task

- **No change to player generation, attribute storage, or `PlayerStore`'s SoA columns**
  (`crates/goat-core/src/player.rs:20-41`) beyond what's explicitly proposed in B.2. The
  scope brief is explicit: "do NOT redesign player-population storage, only club/league/
  nation structure" (that line refers to Doc A, but the same discipline applies here —
  this is a new subsystem layered on top of existing per-player data, not a rework of it).
- **Non-blocking, per the decision's own wording.** "This should affect call-up/starting-XI
  likelihood, not hard-block anything (a bad fit sits on the bench sometimes, isn't banned
  from the sport)." Every mechanism below is a *weight on a probability*, never an `if fit ==
  Awkward { return None }` hard gate.
- **Reuse the existing role/familiarity substrate; don't invent a parallel one.** See B.1 —
  this is the single biggest design decision in this doc and it's made by recognizing an
  existing pattern already used twice in the codebase, not by designing from scratch.

## Relationship to Doc A

This task does **not** require Doc A to be built first — a national team can be generated for
whatever `Nation` set exists today (currently 2: England, Brazil) with zero dependency on
Doc A's club/league scale-up. But it is **much more interesting** once Doc A ships (~20
nations to have call-up rivalries and international honors across, instead of 2) — bible
§4.1's whole "powerhouse vs. minnow, one-of-many vs. national-god" framing needs more than 2
nations to read as the intended difficulty *spectrum* rather than a binary switch. Sequence
suggestion for Tùng: **either order is technically safe to build in, but shipping Doc A first
makes this doc's playable gates land better** (a national-team call-up screen against 20
generated nations is a much stronger demo than against England/Brazil only). Not a hard
blocker either way — flagging the interaction, not creating a false dependency.

## Verified: what exists today vs. what's completely new

**Calendar has the timing hook, nothing behind it.**
`crates/goat-calendar/src/clock.rs:34` / `crates/goat-core/src/calendar_loop.rs:39` —
`WindowKind::InternationalBreak` exists and fires on schedule. `crates/goat-tui/src/main.rs:1891`
— the *only* code that reacts to it is a flavor-text line,
`("✈", "International break — call-ups announced.")`, with **no actual call-up, no squad, no
selection logic behind it**. `crates/goat-calendar/src/subsystem.rs:24,35` —
`SubsystemId::International` exists as an enum variant with a doc comment ("Must interrupt
immediately: match day, transfer offer, serious injury, call-up") but grepping the whole
`goat-calendar`/`goat-core`/`goat-tui` tree found no call-up selection code anywhere. This
confirms the task brief's framing exactly: the *slot* for this feature exists in the calendar
architecture; nothing has ever been built to fill it.

**Zero national-team data model exists.** No `NationalTeam` type, no per-nation squad concept,
no international-caps/honors counter anywhere in `crates/goat-core` or `crates/goat-meta`.
`crates/goat-meta/src/legacy.rs:9-33`'s `LegacyEvidence` (the struct that feeds the whole
pantheon-ranking system, `TASK-DESIGN-round1-pantheon-saves.md`'s subject) has **no
international-honors field at all** — no caps, no "world cups/continental titles won," no
international-goals counter. This is a real, separate gap from this task's named scope (item 4
only asked for the tactical-identity/selection layer, not legacy wiring) but it means bible
§4.1's headline promise — "winning a World Cup... is borderline impossible [for a minnow]...
the legacy case has to be built almost entirely at club level" — has **no mechanical
consequence today even for a powerhouse nation's player**, since nothing tracks international
honors as legacy evidence at all. **Flagged explicitly in "Decisions needed," item 3 below** —
this task's scope brief doesn't ask for legacy wiring, but shipping call-ups with zero legacy
consequence would leave bible §4.1's central promise still entirely unconnected to the
Pantheon after this task lands.

## B.1 — The core design: reuse the existing "shared substrate + reweighting lens" pattern

**This is the load-bearing recognition this whole doc is built on.** The codebase already has
this *exact* shape, twice:

1. `role_rating(current_attrs, role, familiarity)` (`crates/goat-core/src/derive.rs:43-61`) —
   one shared 30-attribute substrate, reweighted per-role by `ROLE_WEIGHT_TABLE`
   (`roles.rs:161-...`), scaled by a familiarity multiplier.
2. `School::score(ev, axes)` (per `TASK-DESIGN-round1-pantheon-saves.md` §2.4,
   `crates/goat-meta/src/pantheon.rs`) — one shared 8-axis substrate (`LegacyAxes`),
   reweighted per-school by `weights: [i32; 8]`, blended with a raw-signal component.

**A team's tactical identity is a third instance of the same pattern: one shared 14-role
substrate (the player's per-role `role_rating`, already computed, already trained via
`familiarity_xp`), reweighted per-team by a new generated `[i32; NUM_ROLES]` vector.** A
player's "fit" for a given team is not a new attribute or a new trained value — it's **an
existing role rating, checked through a different lens**, exactly mirroring how the same
8 `LegacyAxes` read differently through the Trophy Cabinet's lens vs. the Eye-Test Romantics'.

Concretely:

```rust
// crates/goat-core/src/tactical_identity.rs (new)

/// A team's (club's or national team's) style bias over the 14 outfield roles — how much
/// that team's system rewards each role, independent of any specific player.
/// Seed-derived at genesis, analogous to Club::strength (world.rs:49) — a static per-team
/// number, not a trained/mutable one (see B.1's "does this train?" note below).
pub struct TacticalIdentity {
    pub role_weight: [Fixed; NUM_ROLES], // sums to a fixed total, mirrors ROLE_WEIGHT_TABLE's shape
}

/// A player's fit for a specific team's tactical identity, in the *same* 4-tier vocabulary
/// as role familiarity (Natural/Competent/Unconvincing/Awkward) — deliberately reusing
/// FamiliarityTier rather than inventing a parallel tier enum, so the existing UI/select
/// logic (bench-likelihood, etc.) has one vocabulary, not two.
pub fn team_fit(
    current: &[Fixed; NUM_ATTRS],
    familiarity: &[FamiliarityTier; NUM_ROLES],
    identity: &TacticalIdentity,
) -> FamiliarityTier {
    // Best-role-under-this-lens score, mirroring derive::ovr's "best role rating" pattern
    // (derive.rs:111) but weighted by `identity.role_weight` instead of a neutral pick.
    let best = RoleId::ALL.iter().map(|&r| {
        role_rating(current, r, familiarity[r as usize]) * identity.role_weight[r as usize]
    }).fold(Fixed::ZERO, Fixed::max);
    // Bucket `best` into the 4-tier vocabulary via thresholds (new tuning constants,
    // TEAM_FIT_NATURAL_PCT / _COMPETENT_PCT / _UNCONVINCING_PCT, first-pass numbers —
    // needs its own TASK-TUNE pass once playtested, same convention as every other
    // first-pass threshold in this codebase).
    bucket_into_tier(best)
}
```

**This resolves the "does tactical fit train over time?" question the original scope
description left ambiguous** ("a new data axis... analogous to how strength rating works
today" reads as static, but "checked per-role... a player can be Natural at club but Awkward
for national team" sounds like it wants per-team-trained state). Under this design: **nothing
new trains.** A player is naturally "Natural at club, Awkward for country" purely because
their *already-trained* per-role familiarity (e.g., Natural at CentralMid from years at their
club) gets read through two different `TacticalIdentity` lenses (club's role-weight vector vs.
country's) — if the national team's system favors a role the player hasn't trained, they come
out Awkward under that lens even though CentralMid-at-club stays Natural. No new persisted
per-player-per-team state, no new `PlayerStore` columns, fully consistent with the "don't
redesign player-population storage" ground rule.

**Still needs Tùng's confirmation, not a silent Design pick**: this reading (fit = existing
familiarity reweighted, no new training) is Design's recommended interpretation of an
ambiguous brief line, and it's a genuinely good architectural fit (reuses two established
patterns, zero new persisted state) — but if Tùng's actual intent was "a player should be able
to specifically work on / develop chemistry with the national team over repeated call-ups,"
that's a different, bigger feature (would need new persisted per-player-per-scope state,
closer to a second familiarity-XP system) that this design does not deliver. **Flag and
confirm before Dev starts** — this is exactly the kind of interpretation call the task brief
asked not to be silently resolved.

## B.2 — Generating `TacticalIdentity` for clubs and national teams

- **Clubs**: one `TacticalIdentity` generated per club at genesis, seed-derived
  (`GoatRng::new(world_seed ^ club_seed(id))`, same pattern as every other per-club generated
  value in Doc A). This is a **new** per-club field, additive to `Club`
  (`crates/goat-world/src/world.rs:42-50`) — small, `[Fixed; 14]`, cheap even at Doc A's
  proposed ~960-club scale, not a SoA concern (14 small values × ~1,000 clubs is nowhere near
  the 20-30k-player-population scale the SoA mandate is about).
- **National teams**: one `TacticalIdentity` per nation, generated the same way, seed-derived
  from `world_seed ^ nation_seed(id)`. A `NationalTeam` type is new — minimally
  `{ nation_id, tactical_identity: TacticalIdentity }`; **does a national team need a squad
  concept at all**, or is call-up/selection resolved by querying the whole *eligible*
  population (all players of that nationality above some quality bar) on demand each window,
  with no persisted roster? **Recommendation: no persisted roster** — squad selection is
  recomputed each international window from (eligible-by-nationality population × current
  attributes × team_fit), mirroring the bible's "background growth is formula-driven, computed
  on demand" principle (§7.1) rather than maintaining a 23-man national-squad list that needs
  updating every time a player's form/attributes change. Flag for confirmation since a
  persisted squad list is also a legitimate simpler-sounding alternative, but it's *more* new
  state, not less, for no clear gameplay benefit given nothing here needs squad continuity
  week to week the way a club squad does (a club fields the same players repeatedly; a
  national call-up is inherently a fresh selection each window under this task's own framing).

## B.3 — Call-up and starting-XI selection logic

Two separate probability weightings, both driven by `team_fit` (B.1) but at different
decision points:

1. **Call-up** (does the PC get selected for the squad this international window at all): a
   base probability from the PC's overall quality (existing `ovr()`/attributes) modulated by
   `team_fit` against the national team's identity — a Natural fit raises call-up odds, an
   Awkward one lowers them but never zeroes them out (per the ground rule). This is the hook
   for the existing `WindowKind::InternationalBreak` calendar event
   (`calendar_loop.rs:39`/`main.rs:1891`) — replace the current flavor-only string with an
   actual roll.
2. **Starting XI** (if called up, does the PC start or sit): a second, independent roll,
   same `team_fit` input, modulating minutes/start-likelihood the same way `role_rating` +
   familiarity already modulate club-level selection implicitly (there's no explicit "starting
   XI" mechanic at club level either today — the match engine works at beat/`PositionFamily`
   granularity per `crates/goat-match/src/sim.rs:356`'s `role_bias_pct`, not an explicit
   11-man selection list — so this may be the **first** explicit "am I starting" roll in the
   codebase, worth noting as new ground, not an extension of an existing club-side mechanic).

**Underspecified — needs Tùng's numbers, not a Design guess**: the actual probability curve
(how much does Natural vs. Awkward move call-up odds — a 2x swing? 5x? capped floor/ceiling?).
Design recommends first-pass placeholder constants in a new `tuning.rs` block (following the
existing "first-pass number, tune later per TASK-TUNE convention" precedent used throughout
`TASK-DESIGN-round1-pantheon-saves.md` §2.2), **not** guessed-and-shipped-as-final — flag this
explicitly as needing a TASK-TUNE follow-up once playtested, same as every other first-pass
number in this codebase's history.

## B.4 — The bigger, adjacent question: does this task track international honors at all?

**Verified gap (see above): `LegacyEvidence` has zero international-caps/honors fields
today.** This task's named scope (item 4) is specifically the call-up/tactical-fit/selection
layer — it does not explicitly ask for legacy-system wiring. But shipping call-ups with
**zero persistent consequence** (no caps counter, no "won a major international tournament"
flag anywhere) means:

- Bible §4.1's central nationality-dial promise ("a minnow nation... can become a national
  god, but winning a World Cup... is borderline impossible... the legacy case has to be built
  almost entirely at club level") stays **entirely unconnected to the Pantheon** even after
  this task ships — a player could get capped 100 times for their national team and it would
  have zero effect on any `School::score` ranking, since no evidence field exists to read.
- This isn't a bug this task introduces — it's a pre-existing gap this task's scope, as
  written, does not close.

**Flag explicitly for Tùng: is a minimal `career_caps: u32` (and maybe
`international_honors: u32`, if actual tournament simulation is in scope — see next
paragraph) addition to `LegacyEvidence` in scope for this round, or is that a deliberate
follow-up task once the selection layer proves out?** Recommendation: at minimum, add
`career_caps`/`career_international_goals` as new `LegacyEvidence` raw counters (mechanically
identical to `TASK-DESIGN-round1-pantheon-saves.md`'s Slice 2 pattern — new counter, new
`pc_season_X`/`pc_career_X` pair, folded at `ApplySeasonEndLegacy`, no new school-weighting
logic required this round) even if full tournament simulation is deferred — capped appearances
alone is a meaningful, cheap legacy signal and a natural `Stats Purists`/`Trophy Cabinet`-
adjacent raw counter, without committing to the much bigger "simulate an actual World Cup"
scope below.

**Separately, and much bigger: does this task include simulating actual international
tournament *outcomes* (a World Cup / continental championship the national team can win or
lose), or is it purely a call-up/selection/tactical-fit layer with no simulated competitive
result?** The scope brief as given (item 4, verbatim) only describes call-up/starting-XI
likelihood — it does not mention simulating tournament results. **Design's reading: tournament
simulation is explicitly OUT of this round's scope** (it would need its own fixture/knockout-
bracket engine, an entirely different shape from the existing league-table `Table`
machinery in `season.rs`, since tournaments are single/double-elimination, not round-robin) —
but this should be **stated as an explicit scoping decision, confirmed by Tùng**, not
inferred, since bible §4.1 leans hard on "winning a World Cup" as a concrete story beat that a
call-up-only layer does not deliver. If Tùng wants that too, it's a third, even-bigger doc on
top of this one — **do not fold it in here without an explicit decision**, since it roughly
doubles this doc's scope again.

## Out of scope (do not fold into this task)

- **Actual international tournament/match simulation** (World Cup, continental championship
  brackets) — see B.4. Flagged as a probable future task, not this round's job, pending
  confirmation.
- **A trained, per-team, per-player "chemistry" axis** that improves with repeated call-ups —
  see B.1's flagged alternative interpretation; this design's default (no new training, fit is
  a reweighted-existing-familiarity read) does not include this. If Tùng wants literal
  training, that's a different, bigger design (new persisted state, new XP mechanic).
- **A persisted national-squad roster list** — see B.2; this design recomputes eligibility/
  selection fresh each international window instead.
- **Player-initiated nationality switching / dual-nationality mechanics** — nothing in the
  approved scope mentions this; nationality stays a creation-time-only choice (bible §4).
- **Youth/age-group national teams** — scope says "each national team," read as singular
  senior team per nation; youth internationals are a separate, unscoped idea.
- **Rewiring `Table`/`season.rs`'s league machinery** — this task's `TacticalIdentity` is
  additive data on `Club`/a new `NationalTeam`, it does not touch league-table simulation
  (that's Doc A's territory, and this task has no dependency on Doc A's internals, only on
  `Nation` existing as an identifier, which it already does today).
- **PC Reputation as a factor in call-up likelihood — approved by Tùng (2026-07-22, "Ok" at the
  point where this was proposed) but not designed in this doc; flagged here so the decision
  isn't lost.** Tùng's framing: high Reputation (bible §8.2, `crates/goat-meta/src/
  reputation.rs`) should make national-team call-up more likely independent of raw role-fit
  numbers alone. This doc's call-up/starting-XI weighting (B.1/B.3) is role-fit-only as
  designed; a future round should add a Reputation term alongside fit when weighting call-up
  probability — not designed here, just recorded as a real requirement.

## Decisions Design needs from Tùng before Dev starts (collected from above)

1. **B.1**: confirm the "fit = existing role familiarity reweighted per-team, nothing new
   trains" interpretation, vs. a literal new trained per-player-per-team-scope axis (bigger,
   different feature). **Recommendation: the reweighted-existing-data reading** — reuses two
   already-established codebase patterns, adds zero new persisted per-player state.
2. **B.2**: confirm "no persisted national-squad roster, recomputed fresh each call-up window"
   vs. a persisted 23-man squad list maintained across windows. **Recommendation: no
   persisted roster.**
3. **B.4 (the big one)**: is any legacy-system wiring (`career_caps` at minimum) in scope this
   round, given the call-up layer otherwise has zero persistent consequence and leaves bible
   §4.1's nationality-dial promise still unconnected to the Pantheon? **Recommendation: yes,
   at minimum `career_caps`/`career_international_goals` as new raw `LegacyEvidence` counters**
   (cheap, mechanical, follows the exact TASK-DESIGN-round1 Slice 2 precedent) — but full
   tournament simulation (World Cups actually won/lost) is a separate, much bigger,
   **explicitly out-of-scope-pending-confirmation** question.
4. **B.3**: first-pass call-up/starting-XI probability curve numbers — Design proposes
   placeholder constants + a TASK-TUNE follow-up, not final numbers guessed now.
5. **Sequencing vs. Doc A**: confirm whether Tùng wants Doc A (world genesis scale-up) shipped
   first for a stronger playable gate here, or is fine building this against the current
   2-nation world with Doc A following later. Not a blocker either way — just a sequencing
   preference to state.

## Definition of done (once Dev implements)

1. `cargo test --workspace` green, including new tests for `team_fit`/`TacticalIdentity`
   generation (determinism per seed, `Natural`-fit and `Awkward`-fit cases both reachable,
   following the existing `derive.rs:150-165` role-rating test-pattern precedent) and, if B.4
   item 1 is confirmed in scope, a `LegacyEvidence`-folding golden test mirroring
   `TASK-DESIGN-round1-pantheon-saves.md`'s Slice 2 TDD anchor exactly.
2. `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` clean.
3. No new dependencies, no floats in sim state/logic, no unsafe.
4. Playable gate: `cargo run -p goat-tui` → advance to an international-break calendar window
   → an actual call-up/no-call-up outcome is shown (not the current flavor-only string) →
   if called up, a starting/bench outcome is shown → (if B.4 item 1 is in scope) a capped
   appearance increments a visible legacy counter reachable from the existing Legacy/[G]
   screen.
5. If `LegacyEvidence` gains new fields (B.4), `goat-save::save::VERSION` bumps with a
   backward-compat test, following the exact v8→v9 precedent from
   `TASK-DESIGN-round1-pantheon-saves.md`.
