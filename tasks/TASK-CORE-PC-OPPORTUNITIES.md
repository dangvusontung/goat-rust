# Task — PC/NPC opportunity accounting and conversion

Status: implemented in simulation version 5, 2026-10-05.
Report: [PC-NPC-OPPORTUNITIES.md](../docs/PC-NPC-OPPORTUNITIES.md).
NPC policy/quality approximations and remaining differences are documented there.
Evidence: [MATCH-CALIBRATION.md](../docs/MATCH-CALIBRATION.md) and its raw CSV.

## Problem

The shared kernel currently governs automatic NPC attacks, while PC choices
apply authored goal events through a separate contest path. Equal-profile,
equal-attribute synthetic teams win 38.9% in the NPC observer, versus 62.9%
with a PC striker. PC participation and its decision policy are not equivalent
to NPC participation, so identify and expose causal components before tuning.

## Implementation scope

1. Define a headless opportunity record with possession side, elapsed exposure,
   semantic danger/quality, actors and available players. Use fixed-point units.
2. Allocate PC participation within team opportunity flow; protagonist selection
   must not manufacture additional scoring exposure. Preserve off-ball choices,
   defensive choices and authored semantic consequences.
3. Connect decision contests and conversion coherently. Successful build-up is
   not automatically a goal; genuine finishing choices retain matching outcome
   text. Audit `beats.json` contracts before changing event semantics.
4. Model the NPC decision policy or label its approximation explicitly. Keep
   opportunity creation, opportunity quality and goal conversion distinguishable.
5. Extend observations to count PC and NPC opportunities without double-counting
   chained actions or mixing authored stakes with comparable-quality shots.
6. Re-run the existing equal-team, venue, manpower, role and quality ablations.
   Add policy/zone-pull/chain ablations to explain remaining participation shifts.

## Acceptance

- Team opportunities, minutes, goals and credits reconcile across both paths.
- Role and individual quality influence identifiable causes; unexplained large
  changes from the protagonist marker are measured and resolved or documented.
- Red cards still close PC minutes and finish with NPCs; substitutions and
  off-pitch periods preserve accounting.
- Deterministic replay and fixed-point math hold, with no extra renderer rules.
- Existing frozen goldens retain an explicit legacy path. Changed production
  outcomes receive new goldens and a simulation-version bump, without changing
  old expected values to pass.
- Report comparative measurements, uncertainty and remaining model differences.
  Real-football calibration requires a separately specified dataset/competition.

Do not tune global NPC scoring to hide PC-specific differences. Do not replace
the semantic beat engine with coordinate physics. TUI presentation and
150-country expansion are outside this task.
