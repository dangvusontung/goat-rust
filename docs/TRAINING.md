# Training Subsystem — Core Spec + Claude Code Task

**Project:** BECOME THE GOAT
**Scope:** Headless core. The first content-bearing subsystem — gives the calendar tick something to *do*.
**Status:** Design-locked. Numbers are illustrative placeholders (final tuning deferred per §11).
**Depends on:** `goat-core` (attributes, age curves, potential), `goat-calendar` Phase 1 (the tick loop + Subsystem trait).

---

## Overview

Training is the first subsystem that registers on the calendar's tick loop and produces real state change. Its job, per bible §5.4: push a player's **current → potential**, gated by age curves (§5.1) and paid for in **energy**. It is the mechanical core of the week-as-loop-unit pillar — the player sets a routine once, then the calendar auto-runs it day by day, surfacing a decision only on "big weeks".

This is deliberately the first content subsystem because it's the simplest way to validate the `Subsystem` trait the calendar defines: it reads `DayContext`, mutates player attributes deterministically, costs a resource, and occasionally raises a soft-flashpoint — without touching the beat engine's complexity. If the trait is wrong, training reveals it cheaply.

Training is NOT the match engine and NOT player generation. It only moves existing attributes toward their existing ceilings.

---

## Assumptions

- `[ASSUMED]` Training runs as a per-day tick but the player only *interacts* at the week granularity (set a routine; intervene on big weeks). The subsystem reads the same day-tick as everything else.
- `[ASSUMED]` A "routine" is a standing instruction: which attribute(s) to target, at what intensity. It persists across days until changed. Default routine exists so a never-intervening player still develops.
- `[ASSUMED]` Energy is a per-player resource on a 0–100 fixed-point scale: training spends it, rest recovers it. Low energy reduces gains and raises injury risk (§5.4). `[DECISION NEEDED]` exact curve.
- `[ASSUMED]` Growth is gated by the attribute's age-curve archetype (Physical / Technical / Mental, §5.1): trainability differs (Physical low, Technical high, Mental grows with experience not just training). Current can never exceed potential (§2.4 ceiling clamp — already enforced in goat-core).
- `[ASSUMED]` Facilities/coach multipliers (§4.2) feed in as a development-speed multiplier, but the values come from the club model. For this task, accept a multiplier input and default it to 1.0 — do not build the club model here.
- `[ASSUMED]` Match days are not training days. On a fixture day, training yields no growth (the match handles load); the subsystem either rests or applies match-fatigue, but match logic itself is out of scope.

---

## User Stories

```
US-01: Set a training routine
As a player,
I want to set a standing training routine (target attribute + intensity),
So that my player develops automatically without me touching every day.
Priority: P0 | Size: M

US-02: Develop toward potential
As a player,
I want my trained attributes to rise over time toward their personal ceiling,
So that the talent I was dealt actually turns into ability.
Priority: P0 | Size: M

US-03: Feel the age curve
As a player,
I want physical, technical, and mental attributes to grow at different rates by age,
So that development feels like a real career arc, not a uniform stat-pump.
Priority: P0 | Size: M

US-04: Manage energy / fatigue
As a player,
I want training to cost energy and rest to recover it,
So that overtraining has a real cost (worse gains, higher injury risk).
Priority: P0 | Size: M

US-05: Get interrupted only on big weeks
As a player,
I want the game to surface a training decision only at meaningful moments
(a breakthrough, a dip, returning from injury, a derby week),
So that routine weeks stay silent (manage-by-exception, §2.2).
Priority: P1 | Size: M

US-06 (system): Deterministic development
As the core,
I need development for the same (seed, routine, intensity, days) to be identical,
So that headless career sims are reproducible.
Priority: P0 | Size: S
```

---

## Acceptance Criteria

```
AC-01 for US-02: Growth pushes current toward potential, never past

Scenario: Sustained training on a trainable attribute
  Given a 17-year-old with a technical attribute at 60 current / 85 potential
  And a routine targeting that attribute at moderate intensity
  When the calendar advances one season of non-match days
  Then the attribute's current value increases
  And it never exceeds 85 (potential)
  And on reaching 85 further training yields zero growth

AC-02 for US-03: Age-curve archetypes grow differently

Scenario: Same intensity, three archetypes, young player
  Given a 17-year-old training a Physical, a Technical, and a Mental attribute
    at identical intensity for identical days
  When development is applied
  Then the Technical attribute gains the most from training
  And the Physical attribute gains less (low trainability)
  And the Mental attribute's gain leans on experience/age, not just training input

AC-03 for US-04: Energy gates gains and recovers on rest

Scenario: Training drains energy
  Given a player at 100 energy
  When they train at high intensity for several consecutive non-rest days
  Then energy decreases each training day
  And once energy is low, per-day attribute gain is reduced versus the same training at full energy

Scenario: Rest recovers energy
  Given a player at low energy
  When the calendar advances rest days (no training, no match)
  Then energy increases each rest day toward the cap
  And no attribute growth occurs on pure rest days

AC-04 for US-05: Only big weeks raise a flashpoint

Scenario: Routine week is silent
  Given a standing routine and an ordinary training week with no events
  When the calendar ticks through it
  Then the training subsystem returns Silent for each day
  And the player is not interrupted

Scenario: Breakthrough raises a soft flashpoint
  Given a young player whose attribute crosses a notable threshold (a "breakthrough")
  When that day is ticked
  Then the training subsystem returns a SoftFlashpoint with a payload describing the breakthrough
  And the player is surfaced this per the calendar's flush policy

AC-05 for US-06: Development is deterministic

Scenario: Reproducible season of training
  Given two players from the same seed with the same routine and intensity
  When each is advanced the same number of identical days
  Then their attribute values and energy are identical at the end

NFR-01: No floats
  Given any development or energy calculation
  When inspected
  Then it uses goat-fixed math only — no f32/f64 anywhere in the crate
```

---

## Data Models

```
Entity: TrainingRoutine
- target: AttrTarget            // which attribute(s) the routine pushes
- intensity: Intensity          // Light | Moderate | Hard (illustrative tiers)
Persistence: save (small, per-player-of-interest)
Note: a default routine exists so a non-intervening player still develops

Entity: EnergyState
- value: Fixed                  // 0..100 fixed-point
Persistence: save (orbit players); background players computed on demand (7.1)

Entity: DevelopmentInput        // assembled per training day, fed to the growth fn
- attr_archetype: AgeArchetype  // Physical | Technical | Mental (from goat-core, 5.1)
- age_days: u16
- current: Fixed
- potential: Fixed
- intensity: Intensity
- energy: Fixed
- facility_mult: Fixed          // default 1.0; real value from club model (out of scope)
- rng: RngStream                // injected fork("training") — NEVER the calendar/match stream

Entity: TrainingDayResult       // what the subsystem emits per day (wrapped into DayReport)
- attr_deltas: List<(AttrId, Fixed)>   // growth applied this day
- energy_delta: Fixed
- event: TrainingEvent?         // None on a routine day
Note: maps onto the calendar's DayReport { stop_class, payload, mutations }

Enums:
enum Intensity     { Light, Moderate, Hard }
enum AgeArchetype  { Physical, Technical, Mental }   // mirrors goat-core's curve types
enum TrainingEvent { Breakthrough, FormDip, ReturnFromInjury, Overtrained }
enum AttrTarget    { Single(AttrId), Family(FamilyId) }  // start with Single; Family later
```

---

## Core Loop — Pseudo-code

```
// Registered on the calendar as a Subsystem. Called once per day with DayContext.
impl Subsystem for Training:
  fn on_day(ctx) -> DayReport:
      player = ctx.orbit_player()

      // Match day? Training yields nothing; match handles load (match engine out of scope).
      if ctx.todays_fixtures.involve(player):
          return DayReport.silent()        // match-fatigue applied by match subsystem later

      // Pure rest day (no routine active, or routine = rest)?
      if routine.is_rest() or player.is_injured():
          new_energy = recover_energy(player.energy, ctx.rng)
          return DayReport.silent_with(mutations = [set_energy(new_energy)])

      // Training day.
      input = build_development_input(player, routine, ctx.rng)
      delta = compute_growth(input)          // the heart of it; see below
      energy_delta = spend_energy(input.intensity, input.energy)

      event = detect_event(player, delta)    // breakthrough / overtrained / dip
      stop_class = match event:
          Some(Breakthrough | ReturnFromInjury) => SoftFlashpoint
          Some(Overtrained)                      => SoftFlashpoint
          _                                      => Silent

      return DayReport {
          source: Training,
          stop_class,
          payload: event.map(describe),
          mutations: [apply_attr_delta(delta), apply_energy(energy_delta)],
      }


// The growth function — gated by age archetype, intensity, energy, ceiling.
fn compute_growth(input) -> Fixed:
    if input.current >= input.potential:
        return 0                            // ceiling clamp (2.4). Never exceed potential.

    headroom   = input.potential - input.current
    base       = intensity_factor(input.intensity)         // tuning constant
    trainable  = trainability(input.attr_archetype, input.age_days)   // 5.1 curve
    energy_mod = energy_factor(input.energy)               // low energy -> smaller gains
    noise      = small_seeded_jitter(input.rng)            // deterministic variance

    raw = base * trainable * energy_mod * headroom_scaled(headroom) * noise
    return clamp_to_headroom(raw, headroom)


// Trainability per archetype (bible 5.1):
//   Physical  -> low, declines early with age
//   Technical -> high, broad mid-career plateau
//   Mental    -> grows with EXPERIENCE/age, not just training input
fn trainability(archetype, age_days) -> Fixed:
    match archetype:
        Physical  => physical_curve(age_days)    // peaks early, low ceiling on gains
        Technical => technical_curve(age_days)   // high, slow to decline
        Mental    => mental_curve(age_days)      // appreciates with age
```

All curves and factors are named constants in a `tuning` module — illustrative placeholders, flagged TUNABLE, final values deferred per §11.

---

## Feature Breakdown

### Phase 1 — MVP (single-attribute routine, energy, deterministic growth)

| # | Task | Layer | Notes |
|---|------|-------|-------|
| 1 | `goat-training` crate skeleton (`forbid(unsafe)`, deps: rng/fixed/core/calendar) | Infra | registers as a Subsystem |
| 2 | `TrainingRoutine`, `EnergyState`, `Intensity` models | Domain | Single-attr target first |
| 3 | `tuning` module: intensity factors, energy curve, archetype curves | Domain | all placeholder consts, TUNABLE |
| 4 | `compute_growth()` with ceiling clamp | Domain | fixed-point; never exceed potential |
| 5 | `trainability()` archetype curves (Physical/Technical/Mental) | Domain | bible §5.1 |
| 6 | Energy spend/recover | Domain | drains on train, recovers on rest |
| 7 | `detect_event()` (breakthrough/overtrained) | Domain | raises SoftFlashpoint |
| 8 | `impl Subsystem for Training` (`on_day`) | Domain | the integration point |
| 9 | Golden-seed test: one season of training | Domain | frozen expected values |
| 10 | Determinism test: same seed → identical state | Domain | byte-identical snapshot |

> ⚠️ Task 8 depends on the calendar's `Subsystem` trait being merged. Tasks 4–6 depend on 3. Do task 3 (tuning) right after the skeleton — everything reads from it.

### Phase 2 — Post-MVP (deferred)

| # | Task | Layer | Notes |
|---|------|-------|-------|
| 11 | Family-target routines (train a whole family) | Domain | `AttrTarget::Family` |
| 12 | Facility/coach multiplier wired from club model | Domain | needs club model first |
| 13 | Injury risk from overtraining feeds injury subsystem | Domain | cross-subsystem |
| 14 | Form-dip detection + interaction with season-long form | Domain | needs form model |

---

## Tech Notes & Gotchas

### Determinism
- RNG via `ctx.rng.fork("training")` ONLY. Never the calendar or match stream — sharing would couple training variance to unrelated rolls and break §2.3.
- `grep` gate in DoD: no `f32`/`f64`, no `now()`/`SystemTime`/`chrono` in the crate.
- All growth/energy math in goat-fixed. A single float sneaking into a curve constant will silently desync saves across platforms.

### Correctness
- **Ceiling clamp is sacred (§2.4):** current can NEVER exceed potential. This is already enforced in goat-core — call its clamp, do not re-implement a looser one here. Money/facilities accelerate approach to the ceiling, never lift it.
- **Match days yield no training growth.** Don't double-count load: on a fixture day, training is silent and the (future) match subsystem owns fatigue. For this task, just detect fixture days from `DayContext` and skip growth.
- **Mental attributes are special:** their trainability leans on age/experience, not raw training input. Don't model all three archetypes with one scaled curve — that flattens the late-career reinvention arc (§5.2) the whole design hangs on.

### Scope discipline
- Do NOT build the club model, the injury subsystem, the form model, or the match engine here. Accept their inputs as parameters with sane defaults (facility_mult = 1.0, not injured, no match) and leave hooks.
- Do NOT touch goat-rng, goat-fixed, goat-core, or goat-calendar. If a trait or API seems insufficient, STOP and ask — do not refactor a dependency as a side effect.

### Decisions open
- `[DECISION NEEDED]` Energy curve shape: linear drain/recovery, or diminishing? Affects how punishing overtraining feels.
- `[DECISION NEEDED]` Breakthrough threshold: fixed attribute milestones, or relative-to-potential jumps? Determines how often US-05 flashpoints fire.
- `[DECISION NEEDED]` Does intensity affect injury risk directly, or only via energy? (Cross-subsystem; can defer to Phase 2.)

---

## ⚠️ Risks & Open Questions

- **This task validates the `Subsystem` trait.** If `on_day(ctx) -> DayReport` turns out too thin (e.g. training needs multi-day lookahead, or to read another subsystem's state mid-tick), that's a finding about the *calendar's* design, not training. Surface it loudly rather than hacking around it — it's cheaper to fix the trait now than after three subsystems depend on it.
- **Tuning is unbounded and deferred (§11).** Don't try to make development "feel right" in this task — that needs a prototype and playtesting. Ship defensible placeholder constants, clearly marked TUNABLE, and move on.
- **Cross-subsystem ordering.** Training reads energy; a future injury subsystem also reads/writes energy. The calendar's fixed subsystem order decides who sees what first. Note any ordering assumption training makes so it's explicit when injury lands.
- **Background vs orbit players.** Orbit players store energy/current; background players are formula-driven (§7.1). This task only handles the orbit player. Don't accidentally allocate per-day energy state for the 20-30k population — that's the SoA/perf trap from §9.

---
---

# APPENDIX — `TASK-0X-goat-training.md` (paste-ready for Claude Code)

> Same convention as TASK-01-goat-core: read source-of-truth first, reviewable steps
> with pauses, frozen golden values, determinism non-negotiable.
>
> **Prereqs before pasting:**
> - `goat-core` and `goat-calendar` Phase 1 are merged; `cargo test --workspace` green.
> - The calendar exposes the `Subsystem` trait and `DayContext`.
> - `CLAUDE.md` in repo root; tech doc current.
>
> **Scope:** Phase 1 only (single-attribute routine, energy, deterministic growth).
> Family targets, club multipliers, injury coupling, and form are OUT OF SCOPE.

---

Read CLAUDE.md, then docs/BecomeTheGOAT-RustCore-TechDoc.md (module map + build order),
then design-bible §2.2, §2.4, §5.1, §5.2, §5.4, §9, then the public APIs of
crates/goat-rng, crates/goat-fixed, crates/goat-core, and crates/goat-calendar —
especially goat-core's attribute/age-curve model and goat-calendar's Subsystem trait
and DayContext. Do not write any code until you've read all of them.

If anything here contradicts the tech doc, STOP and flag it — the tech doc wins.

Then build the `goat-training` crate, Phase 1, in these steps — pause after each:

## Step 1 — Crate skeleton + models + tuning
- New workspace member `crates/goat-training` (`#![forbid(unsafe_code)]`, deps limited
  to goat-rng, goat-fixed, goat-core, goat-calendar).
- Models: `TrainingRoutine { target, intensity }`, `EnergyState { value: Fixed }`,
  `Intensity { Light, Moderate, Hard }`. Start with single-attribute targets only.
- A `tuning` module holding ALL magic numbers as named consts: intensity factors,
  energy spend/recover rates, and the three age-archetype trainability curves
  (Physical / Technical / Mental). Every constant documented as TUNABLE placeholder
  per bible §11. No final numbers — defensible placeholders only.
- A default routine so a non-intervening player still develops.

## Step 2 — Growth + energy (the math)
- `compute_growth(input) -> Fixed` per bible §5.4: gated by age-archetype trainability
  (§5.1), intensity, energy, and ceiling. Entirely in goat-fixed.
- The ceiling clamp: current can NEVER exceed potential (§2.4). Call goat-core's
  existing clamp — do not re-implement a looser one.
- `trainability(archetype, age_days)`: three DISTINCT curves. Physical = low, declines
  early. Technical = high, broad plateau. Mental = appreciates with age/experience,
  not just training input (this preserves the late-career reinvention arc §5.2 — do not
  collapse all three into one scaled curve).
- Energy: spend on training (scaled by intensity), recover on rest. Low energy reduces
  per-day gain. No growth on pure rest days.
- Property tests: growth is 0 at the ceiling; growth monotonic in intensity at fixed
  energy/age; energy stays in [0,100]; Technical out-gains Physical at young age,
  same intensity.

## Step 3 — Subsystem impl + golden-seed test
- `impl Subsystem for Training`: `on_day(ctx) -> DayReport`. Match day (fixture in
  ctx involving the orbit player) => Silent, no growth. Rest/injured => recover energy,
  Silent. Training day => compute growth + energy delta, detect event, emit DayReport
  with mutations.
- `detect_event`: at minimum Breakthrough (attribute crosses a notable threshold) and
  Overtrained (trained at low energy) => SoftFlashpoint. Ordinary day => Silent.
  Threshold is a TUNABLE const.
- Golden-seed test (training test #1): a fixed seed + fixed routine + a scripted ~one-
  season sequence of training/rest/match days via the calendar tick → assert the exact
  final attribute values, exact final energy, and the exact set of days that produced a
  SoftFlashpoint. These expected values become FROZEN once I approve.
- Determinism test: run the same sequence twice → byte-identical snapshot.

## Rules reminders (from CLAUDE.md — override convenience)
- No floats in sim. RNG only via `ctx.rng.fork("training")` — never the calendar or
  match stream. No std HashMap iteration feeding results.
- Do NOT touch goat-rng, goat-fixed, goat-core, or goat-calendar. If their API seems
  insufficient, STOP and ask — do not refactor them.
- All pre-existing golden tests stay green at ORIGINAL expected values. Never "fix" a
  failing test by editing the expected value.
- Ceiling clamp uses goat-core's existing function — do not write a looser one.
- Out of scope, do not build: club/facility model, injury subsystem, form model, match
  engine, family-target routines. Accept their inputs as defaulted parameters + hooks.
- `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace` clean before each pause.

At each pause: show me the file tree added, the key type definitions (TrainingRoutine,
EnergyState, DevelopmentInput, the Subsystem impl signature), the tuning constants, and
the test output.

## Definition of done (this task)
1. `cargo test --workspace` green — all pre-existing golden tests at original expected
   values (goat-rng 9, goat-fixed 6, plus goat-core's and goat-calendar's).
2. `cargo fmt --check` and `cargo clippy -D warnings` clean.
3. Deterministic behavior covered by the golden-seed test AND the byte-identical
   determinism test.
4. No new heavy deps, no floats in sim, no unsafe, no I/O, no wall-clock reads.
5. `grep -rn "now()\|SystemTime\|Instant\|chrono\|f32\|f64" crates/goat-training/src`
   returns nothing.
6. Short summary of what changed and which bible/tech-doc sections it implements
   (expected: §2.2, §2.4, §5.1, §5.2, §5.4, §9).