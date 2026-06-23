# TASK 02 — Player generation + goat-tui shell

Prereq: Phase 1 done (goat-core domain model + golden test #1 frozen).
Read CLAUDE.md, ROADMAP.md, the tech doc, and the goat-core public API before coding.
Pause for review after each step.

## Step 1 — Generation pipeline (goat-core)
Implement bible §5.3 exactly, as a pure function of (seed, creation choices):
1. Roll the ceiling — overall potential band.
2. Roll role DNA — biased by chosen position.
3. Roll per-attribute potentials — under the ceiling, shaped by role DNA.
4. Set current values at age 16–17: physical attrs start at a high % of potential,
   mental attrs start low (per the age-curve archetypes).
5. Seed familiarity: 1–2 natural roles + adjacent roles at Competent.

Creation choices (bible §4): name, position, nationality, starting club. For this phase,
nationality and club come from a small hardcoded stub list (real world arrives Phase 5);
record the choices in state — their effects land in later phases.

Golden-seed test: fixed seed + fixed choices → assert the exact rolled player
(every attribute current + potential, role DNA, familiarity). Property tests:
two different seeds with identical choices produce different players; current <= potential
everywhere; chosen position's roles are favored in role DNA.

## Step 2 — goat-tui crate
New workspace member, **binary**, plain stdin/stdout, zero new dependencies, zero sim logic.
Structure it as: read core state → render → read input → translate to intent → call core.
A simple screen/menu loop is enough. All strings that describe game facts come from core
data; the TUI only lays them out.

## Step 3 — Wire the playable gate
Flow in `cargo run -p goat-tui`:
- New game → enter name, pick position / nationality / club (stub lists), enter or
  randomize a seed.
- Talent is rolled. Show the player sheet: 6 family values (derived), expandable to the
  30 sub-attributes with current/potential, roles with familiarity tiers and ratings, OVR.
- Allow re-rolling with a new seed to feel the lottery (§2.4).
- TUI smoke test: scripted stdin with fixed seed → assert key stdout fragments.

## Out of scope (do not build yet)
Weeks/training (Phase 3), matches (Phase 4), real world/clubs (Phase 5), saving (Phase 5).

## Rules reminders
No floats in sim; RNG injected only; goat-rng/goat-fixed untouched; all tunables in
`tuning`; pre-existing golden tests stay green with original values.
