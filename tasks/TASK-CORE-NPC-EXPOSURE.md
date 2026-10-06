# Task — Dated PC/NPC training and health history

Status: implemented in simulation version 6; PC factual ledgers and NPC dated expected exposure.
Report: [development history, validation and performance](../docs/DEVELOPMENT-HISTORY.md).
Context: [PC-NPC-OPPORTUNITIES.md](../docs/PC-NPC-OPPORTUNITIES.md).

Replace the constant NPC expected plan with dated exposure, without rerolling
talent or projecting the player's whole past using today's club conditions.

1. Store compact columnar exposure segments for facility/training changes,
   workload, energy and health. Derive deterministic segments from season/market
   replay where possible; persist only genuinely path-dependent inputs.
2. Apply the shared weekly growth/decay laws over those segments, splitting age
   boundaries and preserving fixed-point rounding, ceilings and decline order.
3. Use the PC's injury-risk/durability primitives for background expectations or
   seeded individual injuries; label approximations and keep RNG streams stable.
4. Transfers and facility upgrades affect subsequent exposure, not innate
   potential or previously accrued development. Youth intake uses its own epoch.
5. Ranking and lazy promotion read the same attributes. Cache invalidation must
   include exposure revision as well as date, not silently reuse a stale OVR.
6. Test replay/save reconstruction, transfer/facility interventions, injury
   recovery, youth ages and long careers. Compare segmented aggregation with a
   weekly reference and measure cold/warm costs at the current 26,000+ scale.

No full heap player object per NPC, no renderer rules, no new dependencies
without authorization. Preserve frozen legacy tests with an explicit compatible
path. Bump simulation version for changed outcomes; change binary layout only if
new path-dependent saved fields are actually necessary. Report remaining
approximations and measured performance. The 150-country task stays separate.

User-expanded scope: PC weekly training/rest history, actual match energy workload
and injury/recovery events persist in layout 22. NPC history remains expected
exposure derived from replay, not diagnosed individual episodes. See report for
season-clock, policy, path-dependent intervention and mobile limitations.

Enhancement completed 2026-10-06: [v7 individual NPC weeks](../docs/NPC-INDIVIDUAL-LIFE.md)
replace expected post-entry health with seeded episodes and concrete drills;
lineups, appearance quotas and goal attribution use simulated health. Earlier
expected factories remain available. Workload is still an appearance proxy;
fixture-specific load, NPC breakthroughs/familiarity and saved override editing
remain outside this implementation.
