# Core enhancement roadmap — checkpoint v10, 2026-10-06

Owner priority: complete the simulation core; TUI is a test adapter. The current
baseline has dated seasons, retained replay and detailed NPC fixtures in the PC
league plus five bootstrap-ranked leagues. Save layout 25/SIM10; 591 tests pass.
This roadmap orders future work; it does not claim the milestones are implemented.

| Order | Milestone | Acceptance gate |
| --- | --- | --- |
| 1 | Historical league coefficients | Rank leagues using multi-season continental results, normalize entry counts, use stable ties, refresh the deep set at season preparation and preserve prior fixture outcomes. Document windows and weights before calibration. |
| 2 | Medical continuity and compact history | Preserve dated injury, rehabilitation, fatigue and match congestion across PC/NPC play and season boundaries; reduce old minute/credit journal size without changing replay, DNPs or accounting. Compare long-career save/load and peak memory. |
| 3 | NPC match realism | Score- and fatigue-reactive substitutions, tactical adjustments and competition-specific NPC cards/suspensions; decide and implement named goalkeeper coverage separately from the deferred PC goalkeeper career. Conserve team minutes and verify the shared scoring kernel statistically. |
| 4 | Full competition calendar | Date cups, continental/national games and friendlies; persist tournament progress across June/July and save/load. Temporarily deepen imminent opponents. Verify rest gaps, overlaps and cross-competition suspensions. |
| 5 | Dated roster and market changes | Execute transfers, contracts, youth intake and roster changes at their actual dates. Verify light/deep transitions and that old appearances, health and goal records stay attached to the correct player and club. |
| 6 | Capacity and mobile measurements | Measure cold resume, active weekly progression, annual boundaries, long-career saves and memory at 200k capacity and on native ARM64. Set the deep budget from measurements before integrating 150 countries. |

The 150-country expansion remains a separate [backlog task](../tasks/TASK-CORE-150-COUNTRIES.md).
Renderer enhancements follow core completion. No mobile throughput claim is made
from desktop timing. A 200k benchmark is a capacity check, not an instruction to
increase the live population immediately.

Recommended next coding milestone: **historical league coefficients**, so the deep
set follows accumulated football results rather than only generated strength.
In parallel planning, quantify the journal budget: the current six-league example
stores about 2.6 MiB of minutes per season before credits and scorelines.

Implementation evidence and remaining approximations: [ranked deep leagues](DEEP-LEAGUES.md),
[chronology](SEASON-CHRONOLOGY.md), [deep/light task](../tasks/TASK-CORE-DEEP-LIGHT-SIMULATION.md).
Every milestone retains seeded determinism, integer/fixed-point math, existing
frozen goldens and clear simulation/save compatibility. Future migrations must be
tested explicitly; the current build rejects incompatible older simulation saves.
