# Core enhancement roadmap — checkpoint v13, 2026-10-06

Owner priority: complete the simulation core; TUI is a test adapter. The current
baseline has dated seasons, retained replay and detailed NPC fixtures in the PC
league plus five ranked leagues. Historical continental coefficients now replace
the bootstrap gradually over five seasons. Save layout 28/SIM13; 625 workspace tests pass. Medical continuity,
lossless save/runtime journal storage and reactive autonomous NPC league matches
are implemented. Milestones 1–3 are complete within the documented model scope;
calendar integration and the remaining football approximations are listed in
[the v13 report](CORE-STEPS-2-3.md).

| Order | Milestone | Acceptance gate |
| --- | --- | --- |
| 1 | Historical league coefficients — implemented v11 | Rank leagues using multi-season continental results, normalize entry counts, use stable ties, refresh the deep set at season preparation and preserve prior fixture outcomes. Document windows and weights before calibration. |
| 2 | Medical continuity and compact history — implemented v13 | Preserve dated injury, rehabilitation, fatigue and match congestion across PC/NPC play and season boundaries; reduce old minute/credit journal size without changing replay, DNPs or accounting. Compare long-career save/load and peak memory. |
| 3 | NPC match realism — implemented v13 | Score- and fatigue-reactive substitutions, tactical adjustments and competition-specific NPC cards/suspensions; decide and implement named goalkeeper coverage separately from the deferred PC goalkeeper career. Conserve team minutes and verify the shared scoring kernel statistically. |
| 4 | Full competition calendar | Date cups, continental/national games and friendlies; persist tournament progress across June/July and save/load. Temporarily deepen imminent opponents. Verify rest gaps, overlaps and cross-competition suspensions. |
| 5 | Dated roster and market changes | Execute transfers, contracts, youth intake and roster changes at their actual dates. Verify light/deep transitions and that old appearances, health and goal records stay attached to the correct player and club. |
| 6 | Capacity and mobile measurements | Measure cold resume, active weekly progression, annual boundaries, long-career saves and memory at 200k capacity and on native ARM64. Set the deep budget from measurements before integrating 150 countries. |

The 150-country expansion remains a separate [backlog task](../tasks/TASK-CORE-150-COUNTRIES.md).
Renderer enhancements follow core completion. No mobile throughput claim is made
from desktop timing. A 200k benchmark is a capacity check, not an instruction to
increase the live population immediately.

Recommended next coding milestone: **[4 — full competition calendar](../tasks/TASK-CORE-DATED-COMPETITIONS.md)**. Persist
actual cup/continental/national fixtures and tournament progression through summer,
then connect competition-specific NPC bans and minute observations to those dates.
Integrate reactive NPC/keeper behavior into the interactive PC match adapter and
temporarily deepen imminent opponents. Do not expand the deep budget before the
capacity/ARM64 measurements in milestone 6.

[The runtime journal task](../tasks/TASK-CORE-JOURNAL-CHECKPOINTS.md) is fulfilled by
lossless completed-season cache compression and exact streaming comparison. An optional
[local resume checkpoint](LOCAL-RESUME-CHECKPOINT.md) now persists annual derived
state in layout 28 without changing SIM13. The authoritative WorldState journal
vectors remain expanded; capacity and native ARM64 gates remain open.

Historical coefficients are [implemented in v11](LEAGUE-COEFFICIENTS.md).
Qualification slot allocation is still stature-based; interactive PC cup results,
live roster-based continental strength and geographic competition pools remain
calendar/persistence work. The current coefficient uses the generated batch cup
engine, with distinct entrants across its three tiers.

Implementation evidence and remaining approximations: [ranked deep leagues](DEEP-LEAGUES.md),
[chronology](SEASON-CHRONOLOGY.md), [deep/light task](../tasks/TASK-CORE-DEEP-LIGHT-SIMULATION.md).
Every milestone retains seeded determinism, integer/fixed-point math, existing
frozen goldens and clear simulation/save compatibility. Future migrations must be
tested explicitly; the current build rejects incompatible older simulation saves.
