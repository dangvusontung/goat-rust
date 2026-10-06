# Task — Expand the shared core world to 150 countries

Status: saved backlog task; not implemented. Owner confirmed **150 countries**
on 2026-10-05. Depends on a shared headless world/career execution path and
compatible statistics/simulation-depth handling. Core realism comes first;
do not prioritize TUI enhancements or implement this by changing a constant.

## Current evidence

Active career model: `WorldGenesis`, 20 countries × 3 tiers × 20 clubs, 38 rounds.
Retained local generator: 50 countries, 159 divisions, 2,544 clubs.
These models coexist after merge; the latter is not the active career loop.
See [audit](../docs/SIMULATION-AUDIT.md) and
[checkpoint](../docs/DEVELOPMENT-STATUS.md).

## Outcome

One shared deterministic world containing exactly 150 unique, stable country
identities. Core consumers use that world: genesis, population, clubs, fixtures,
promotion, transfers, national teams, continental qualification, history,
career transitions, save/load and headless simulation. Adapters display it;
they do not own a second world definition.

## Required design work before implementation

1. Define the 150-country dataset, stable IDs and provenance for relative
   football/economic strength. Inclusion list is unresolved; do not invent an
   owner-approved list or assume all pyramids have identical club counts.
2. Define country-specific tiers, clubs, season calendars, promotion rules,
   international eligibility and confederations. Explicitly document any
   simplifications that remain necessary for the first core implementation.
3. Inventory fixed sizes and narrowing conversions: nation/club/player IDs,
   `NUM_NATIONS`, `TIERS_PER_NATION`, `CLUBS_PER_DIV`, table buffers, division
   indices (including orbit record `u8`), fixture rounds, qualification groups,
   manager pools, name tables, registry and saved membership.
4. Choose migration/version behavior for current saves. Country IDs, club IDs
   and player IDs must not silently change meaning. Preserve old model data or
   reject incompatible saves explicitly; test the chosen policy.
5. Estimate clubs/population/match count from the chosen pyramids; benchmark
   release startup, one season, late-career restore and memory before claiming
   support. Agree practical budgets after measurement rather than inventing them.

## Implementation sequence

- Introduce a shared data-driven country/league registry with stable identities.
- Migrate genesis and lazy population realization to that registry.
- Remove structural assumptions from fixture/table/promotion consumers.
- Adapt market, history and national/continental competitions to the same IDs.
- Integrate shared career/season pipeline and compatibility handling.
- Keep any legacy model only as an explicit versioned compatibility path.

## Acceptance criteria

- Exactly 150 unique countries; all referenced clubs/leagues/players resolve.
- Same seed and decisions reproduce the same world and career.
- Fixture reciprocity, standings and promotion balance hold for representative
  different pyramid shapes, including smaller countries.
- Correct tournament entrants, eligibility and no duplicate qualifications.
- Roundtrip and historical-save policy tested; no integer truncation/ID aliasing.
- Headless multi-season paths exercise transfers, youth intake and retirement
  across country strengths without rebuilding player identities inconsistently.
- Measured runtime/memory and statistical sanity reported against agreed budgets.
- TUI/web/bridge remain adapters; no UI polish is part of this task.
- Golden changes, if required by a new model version, are explicit and reviewed;
  pre-existing frozen outputs are not casually overwritten.

Do not fetch live APIs at runtime or require credentials to simulate the world.
Use offline datasets with documented sources/licensing for authored country data.
