# Core simulation audit

Reviewed 2026-10-05 against checkout `c78b04b` plus the uncommitted baseline
test/Clippy fixes. This is an implementation audit and development guide,
not a claim that the model has been validated against real football data.

Subsequent implementation: [shared simulation run](SHARED-SIMULATION-RUN.md)
records the new model, coverage and remaining gaps; findings below describe the
audited baseline rather than the new entry points.

The subsequent [red-card fix](RED-CARD-CONTINUATION.md) now continues the
current shared engine to full time after dismissal.

## Direction confirmed by the owner

Prioritize the most realistic practical **core simulation**. The TUI is a
testing adapter, not the product to polish now. Complete shared simulation
behavior before enhancing presentation. The 150-country expansion is saved
as [a separate task](../tasks/TASK-CORE-150-COUNTRIES.md); it is not implemented
in this audit and does not take priority over fixing causal inconsistencies.

Realism should mean coherent causes and believable distributions: minutes,
ability, tactics, opposition, workload and circumstances affect outcomes;
the same football world remains consistent when observed at different levels
of detail. The existing semantic-zone/beat approach can support this without
a coordinate-based physics engine. Do not silently replace that design.

Evidence labels used below:

- **Implemented:** directly inspected code, with path and function supplied.
- **Measured:** a command actually executed or an explicit analytical result.
- **Risk:** a likely consequence needing a focused experiment, not a proven bug.
- **Proposed:** future behavior or validation, not an existing capability.

Existing design sources: [Bible](DESIGN_BIBLE.md), [match appendix](MATCH.md),
[rating appendix](PLAYER_RATING.md), [calendar](CALENDAR.md),
[training](TRAINING.md), [traits](DESIGN_BIBLE_APP_A.md).
The rating appendix supersedes the Bible's old best-role definition of OVR.
Historical task checkboxes and tuning logs are not authoritative evidence of
current integration after the September merge.

## 1. Execution paths and ownership

| Path | What executes | Important limitation |
|---|---|---|
| Player league match | TUI constructs real population squads, manager selection and match setup; `goat-match` resolves play; reducer banks results | Significant simulation orchestration remains in the renderer |
| Academy/friendly/several cup paths | Same match engine with synthetic squad sheets at multiple call sites | Named real league opponents and synthetic opponents are not equivalent evidence |
| Background league | Fixtures plus `sim_team_match`, then aggregate NPC credits | Different scoring distribution and individual development from deep matches |
| World replay | `ReplayCache::advance_one_season` runs economic passes, matches, manager evaluation, intake and promotion | A full pipeline existing does not prove the live career invokes it |
| Orbit population rebuild | `orbit::rebuild_population`: genesis, NPC credits, orbit-aware batch tick, youth intake | It does not call the full ReplayCache economy/transfer/manager pipeline |
| `career-batch` | Reducer plus synthetic player output (`form ± 10`) and coarse team results | Cannot validate actual match behavior, selection or substitution effects |
| `match-batch` | Controlled match setups with stub squads and injected star players | No real season, transfer history or live manager selection |
| Web/bridge | Separate application/session orchestration around shared crates | Passing adapter tests does not establish parity across every subsystem |

Sources: [TUI](../crates/goat-tui/src/main.rs) `run_next_round`,
`build_orbit_record`, `generate_transfer_offers`, `run_awards_and_pundits`;
[orbit](../crates/goat-world/src/orbit.rs) `rebuild_population`;
[replay](../crates/goat-world/src/promotion.rs) `advance_one_season`;
[career batch](../crates/goat-tui/src/career_batch.rs).

**Architectural priority:** introduce one headless career/match/season
orchestration boundary. It may live in a dedicated application crate to avoid
`goat-core`/`goat-world` dependency cycles. The TUI, web, bridge and measurement
tools should submit decisions to it and render its events. Pure component
libraries remain useful; a green unit suite is not a substitute for this boundary.
See [the proposed task](../tasks/TASK-CORE-SHARED-SIMULATION.md).

## 2. Player creation, ability and development

**Implemented:** 30 current/potential attributes; position-shaped potential;
role familiarity; seeded ceiling/spikiness; tactical bias for lazy population
realization; debut OVR cap 65. Position rating blends a weighted attribute
average with the highest key attribute, then familiarity. Role rating is a
separate lens used by match logic. OVR describes the primary position, not
an empirical probability of winning or a universal football ability scale.

Sources: [generation](../crates/goat-core/src/generation.rs)
`generate_player_biased_with_ceiling`,
[ratings](../crates/goat-core/src/derive.rs) `position_rating`, `role_rating`, `ovr`.

**Canonical live training:** [week.rs](../crates/goat-core/src/week.rs)
`advance_week` advances age, handles injury recovery, downgrades intensity
when exhausted, applies energy cost/recovery, rolls injury, then grows focused
attributes and familiarity, applies decay and possibly a breakthrough.

Growth is approximately:

```
base growth × age-archetype rate × intensity × energy × facilities/staff/lifestyle
  + bounded random variance
```

It is capped per attribute per week and by effective potential. Training
intensity and lifestyle also reduce the reachable ceiling, not just growth
speed. Physical training gains stop after 27; technical/mental gains continue
at slower age-dependent rates. This is a game rule, not measured physiology.

**Separate model:** `goat-training` implements daily growth and its own
subsystem/golden tests. It is not used by the TUI's training path. The original
TASK-3.5 wanted consolidation, while current calendar integration explicitly
preserves weekly growth. Reconcile intent before adopting either model.

**Background players:** [population](../crates/goat-world/src/population.rs)
`development_fraction` uses one curve: 60% at age 16, rising to 100% around 25,
flat through 31, then declining. `promote` applies that same fraction to every
potential attribute. This differs from the PC's physical/technical/mental curves,
workload and facilities-driven training. Coarse `current_ovr` also need not equal
the rating computed from the lazily realized full attribute set.

**Risks to investigate:**

1. Injured branches return before passive decay in both training and rest weeks.
   Absence should not accidentally protect an older player from aging, and a
   long injury currently has no explicit skill/conditioning regression model.
2. `Routine.focus_attrs` is a vector with no hard limit; the loop awards growth
   independently per entry while session energy cost is flat. Check whether
   duplicate or excessive focus entries can create extra growth through core
   callers. Define a training-time budget and validate input at the shared boundary.
3. Mental development is mainly focus-driven. The design's experience-driven
   reinvention needs evidence that playing meaningful minutes changes development.
4. A changed intensity/lifestyle ceiling may clamp an already-earned attribute
   down; assess whether this discontinuity reflects the intended sustained process.
5. Realizing a player reads the current club's tactical identity. Test continuity
   across transfers/manager changes so a lazy read does not regenerate innate talent.

**Proposed validation:** cohorts across position, talent and age; matched players
with different minutes/training/rest/facilities; growth and injury exposure per
1,000 hours; peak-age distributions by attribute type; return-from-injury paths;
no state jump simply because the player becomes observed.

## 3. Match flow and individual performance

**Implemented:** [sim.rs](../crates/goat-match/src/sim.rs) runs 3–8 minute ticks,
possession and semantic zones, role-dependent involvement, authored situations,
contests and short automatic chains. A match normally ends at 90 minutes.
Attack/defense/midfield profiles, score state and momentum influence events.
Individual output starts around neutral and receives tapered outcome deltas.

[contest.rs](../crates/goat-match/src/contest.rs) resolves:

```
base probability = clamp(50 + (attribute − difficulty) × 50 / 99, 5, 95)
final probability = clamp(base + headspace + stamina + context, 3, 97)
```

Difficulty blends opponent team-line quality and an individual's counter-stat.
Headspace includes confidence, nerves, frustration and flow, with composure
moderating changes. Automatic play chooses the most favorable attribute-minus-
difficulty choice, not a full expected-value decision over risk/reward.

Background commentary goals use attack/(attack+defense+1), modified by lead,
trailing/late-game boosts and response surge. Authored choices may directly
produce goals. These mechanisms are calibrated by internal output targets,
not a measured shot-location/shot-quality or xG model.

**Realism gaps:**

- Role-zone pulls change possession opportunities: forwards often pull play to
  their attack, defenders to opposition attack. This can couple team results to
  the selected PC position independently of underlying squad strength.
- Comeback, mercy and late-level boosts are strong outcome-shaping assumptions.
  Plausible average goals alone does not validate score-state behavior.
- Player stamina starts at a match-local constant and is depleted by resolved
  choices. Investigate inactive/bench minutes and workload transfer between
  career energy and match stamina; NPC conditioning is less detailed.
- No dedicated keeper attributes/decision model, shot ledger or calibrated
  finishing-versus-saving model is evident in the inspected path. Goalkeeper
  *career* remains parked; team-level keeper influence is a separate realism question.
- Tick sampling and short chain limits are acceptable abstractions only if
  timing, chance creation and event distributions are calibrated across roles.
- Real home advantage is explicit in the coarse result model; equivalent treatment
  in the deep path needs verification rather than assumption.

**Proposed validation:** home/away swaps, equal-team role swaps, attack/defense
perturbations, chance quality/conversion, timing by score state, clean sheets,
draws, high-score tails, goals/assists by position, and performance in defeat.
Use paired setups and many seeds; control player quality and teammates before
attributing a positional gap to bias. Preserve believable variance.

## 4. Discipline, substitutions and selection

**Implemented:** [discipline](../crates/goat-match/src/discipline.rs) combines
choice foul risk, referee style, dirty reputation, aggression and frustration.
The reducer has competition-scoped suspension handling.

**High-priority code finding:** `sim::apply_card` calls `finalize(ms)` immediately
on a PC red card. This completes the *whole match*, not only the PC's involvement.
Real football continues with the dismissed player absent and the team short-handed.
This is confirmed by source inspection; a focused reproduction and regression
test should precede a change. Do not silently alter frozen results during this audit.

**Implemented selection:** [population](../crates/goat-world/src/population.rs)
`pc_selection_score` includes ability, form, trust, fit, hype, wage/appeal,
rebellion and energy. NPCs use a simpler path. Manager effects can explain some
unequal treatment; such asymmetry should be explicit and measured.

**Implemented substitutions:** PC sub-on/off uses manager-context rules and a
separate RNG stream; opponent substitutions are trailing-only, capped at two,
with coarse same-position replacements. Replacement players affect matchup
contests and credits, but the inspected opponent swap does not recompute the
team's tactical profile. Other own-team NPC rotation is limited.

Partial-match output is blended toward 50 by minutes played. This mixes
performance quality with participation; separate per-minute quality from
career contribution before using one rating to drive form and legacy.

**Proposed validation:** red-card continuation to full time; ten-man effects;
legal substitution limits/windows; no dismissed player returning; formation
constraints; no unavailable starter; minutes bounded by actual entry/exit;
replacement quality affects the appropriate team lines; equivalent NPC/PC
events obey the same accounting rules.

## 5. Background matches, statistics and continuity

**Implemented:** [season](../crates/goat-world/src/season.rs) `sim_team_match`
gives each side five independent scoring trials. Home attack is `2×strength+5`;
other attack/defense terms are `2×strength`. Each trial's probability is about
half its attack share. Consequently each team can score at most five goals.

**Analytical measurement:** equal strength 75 produces trial probabilities
0.253 home and 0.249 away, expected total 2.51 goals, and a 0–0 probability
about 5.56%. This describes the implemented formula, not a real-data target.
Deep-match samples below average about 3 goals and can exceed five per side.
Those setups differ, so this does not prove an equal-input discrepancy; it
does expose distinct model supports and motivates matched experiments.

[batch_tick](../crates/goat-world/src/batch_tick.rs) credits starters 30
appearances and fringe players eight, then shares goals by position × current
OVR. Integer division can leave team goals unallocated. Aggregate appearances
do not carry the full minutes/availability/match participation ledger.
Orbit overlays avoid double-counting some already-detailed statistics.

[orbit](../crates/goat-world/src/orbit.rs) derives NPC ratings from goals,
assists and team result. It stores appearances/goals and changes form; there
is no equivalent background season assists accumulation evident in this path.
The result term risks rating defenders primarily through team results rather
than prevented danger. Inspect full data flow before expanding the schema.

**Proposed invariants:** player goals reconcile to team goals (including own
goals/unknown attribution if modeled); assists require eligible events; one
appearance per participant; NPC and PC minutes have the same semantics; league
points and goals balance; orbit-to-background transition never erases history;
observation depth changes cost, not the statistical meaning of a career.

## 6. Calendar, competitions and season boundaries

**Implemented:** [calendar bridge](../crates/goat-core/src/calendar_loop.rs)
advances seven day ticks using a forked calendar RNG and surfaces windows.
Weekly training remains separate. Pre-season lasts seven weeks; the league
has a 38-round grid with rest and double-fixture weeks. Career base year is
read outside the core and saved. Internal year length is fixed at 365 days;
age uses weeks and a 52-week seasonal cap.

Competition modules implement league/cup/continental/national progression,
but multiple dispatchers remain in TUI code. A calendar unit test validating
fixture conflict does not prove the renderer dispatch respects the same result.

**Risks:** check the 364-day age year versus 365-day calendar at repeated
season transitions; early exits/event pauses; fixture congestion and recovery;
injury/suspension scope; two matches in one week; season-end idempotence; national
selection and player strength on synthetic versus real squad paths.

**Proposed:** one authoritative elapsed-day clock, with explicit derived age,
training/workload exposure and a fixed-order season pipeline. Changing the
existing time model needs compatibility tests and explicit design decisions.

## 7. Clubs, scouting, contracts and player economy

**Implemented AI club economy:** [economy](../crates/goat-world/src/economy.rs)
uses strength/tier income and estimated squad wages; budgets can be negative.
[scouting](../crates/goat-world/src/scouting.rs) searches bounded candidate
lists for upgrades or young high-upside players. It directly reads potential
for gem scores: AI hidden knowledge differs from noisy PC-facing scouting.
[transfers](../crates/goat-world/src/transfers.rs) resolves bids/auctions and
conserves buyer/seller fee money. Academy investment and manager replacement
run in the replay pipeline.

**PC market:** offer generation, matching clubs to scouted level, wage/fee terms
and negotiation remain in TUI. `scout_estimate` adds usually ±8 noise, with a
10% wider ±20 branch. Season-average output fixed the former saturation bug.
There is no measured uncertainty reduction with repeated observation in this
function, nor a unified contract ledger connecting both sides of the market.

**Player life/economy:** reducer intents cover wages, sponsors, development
investment, business returns, upkeep, relationship changes and reputation.
An intent existing does not establish that autonomous core events invoke it.
Inspect wages and settlement for double income before changing economy formulas.

**Realism priorities:** common contracts and transfer identities; team need,
playing-time promises and affordability; uncertain scouting rather than
omniscient potential; country/tier economics; cash-flow conservation; insolvency
and turnover; development investment with diminishing returns; explicit generated
life-event sources. Do not promise real-world financial accuracy from invented
£k constants without calibration.

## 8. Awards, reputation, rivalry and legacy

**Implemented:** [awards](../crates/goat-meta/src/awards.rs) uses a seeded list
of generated competitors, explicitly not actual population season statistics.
Awards are reproducible but may contradict the league's real scorer/performer.
Replace the synthetic candidate source with the common statistics ledger before
using awards as strong evidence of career realism.

[legacy](../crates/goat-meta/src/legacy.rs) builds bounded axes with hand-set
thresholds: five titles saturate the title contribution; 300 appearances
saturate longevity; ten decisive moments saturate that axis. `compute_axes`
returns neutral Icon/head-to-head values; some callers substitute richer data.
Those adapters and their evidence sources need parity checks.

[rival](../crates/goat-world/src/rival.rs) compares goals + 50×titles in a
birth-age cohort, with minimum 150 appearances and a 70% keeping-pace bar.
This favors goal accumulation and trophies; role, competition strength and
minutes context need consideration before interpreting “weak era.”

**Proposed:** actual award candidates; position/competition-aware contribution;
long-career discrimination without early saturation; consistent reputation
updates from events; rivals drawn from the same evolving records and relevant
football context. Pantheon disagreement is a design feature, not a realism bug.

## 9. New diagnostic measurements

Executed 2026-10-05 with Rust 1.90.0, debug build, master seed 763844 (`0xBA7C4`),
10,000 matches each. Harness uses five role groups, controlled opponents and
synthetic squads; 2,500 matches inject a stronger PC. Both commands exited zero.

```sh
cargo run --locked -p goat-tui --bin match-batch -- 10000 763844
cargo run --locked -p goat-tui --bin match-batch -- 10000 763844 --m1-squad
```

| Metric | Default profile | Squad-derived own profile |
|---|---:|---:|
| W / D / L (%) | 38.9 / 24.8 / 36.2 | 35.5 / 24.8 / 39.8 |
| Mean total goals | 2.99 | 3.00 |
| Mean PC rating | 59.2 | 59.3 |
| Clean sheets (%) | 16.7 | 14.8 |
| ST win / loss (%) | 50.9 / 24.5 | 46.7 / 27.8 |
| CB win / loss (%) | 27.3 / 48.5 | 25.1 / 51.6 |
| Rating ≥70 in defeat (% of all matches) | 3.84 | 4.21 |

Rounding means percentages need not sum to 100. These are diagnostic samples,
not empirical football validation or paired role-isolation experiments. The
squad option changes the own profile, not a whole live-world season. The
positional gap persists and merits controlled causal tests; it is not proof
that a centre-back career is necessarily unfair in the live game.

Raw outputs in this environment: `/workspace/.goat-tools/audit-match-default.log`
and `audit-match-squad.log`. The commands and summarized results above are
retained here so the findings do not depend on those local files surviving.

Earlier stabilization: 532 tests passed, no failures, one ignored;
formatting, Clippy, career invariants and scanner passed. One real TUI auto-play
season tested all 38 rounds and persisted NPC credits/legacy. That is readiness
evidence, not validation of all realism questions described here.

## 10. Prioritized core backlog and acceptance evidence

| Priority | Work | Evidence required before declaring it complete |
|---|---|---|
| P0 | Shared headless match/career/season orchestration | Adapters/harnesses call one path; scripted season parity and replay/save continuity |
| P0 | Red-card continuation and participation ledger | Full-time completion, unavailable-player exclusion, consistent minutes/goals/assists |
| P1 | One statistics source for awards, rivals, form and legacy | Winners and comparisons reference actual eligible records; no synthetic replacements |
| P1 | Compatible background/deep simulation | Equal-input and observation-transition experiments; reconciled team/player totals |
| P1 | Development/workload/injury coherence | Cohort curves, exposure-aware injuries, bounded training, transfer/lazy-realization continuity |
| P1 | Common transfer/economy pipeline | Actual squads evolve in careers; contracts, budgets and scouting reconcile across adapters |
| P2 | Tactical/role/substitution realism and calibrated distributions | Controlled positional ablations, profile updates, home advantage, score-state tests |
| Later | 150-country expansion | Stable identity/migration, representative pyramids, tournaments and measured resource cost |
| Later | Presentation enhancements | Core decisions/events already exposed independently of TUI |

Build correctness, statistical calibration and product playtesting are separate
gates. Deterministic output can be consistently unrealistic; realistic-looking
averages can hide broken accounting or impossible edge cases.

For calibration, choose dated real datasets and define competition scope,
season, units, exposure, sample sizes, missing-data handling and acceptable
uncertainty before setting numeric targets. No external dataset was fetched
for this audit. Use football reference distributions plus paired intervention
tests; report failures and uncertainty, not only a global goal average.

The next coding milestone is the shared-core task, beginning with extraction
that preserves behavior. Keep the red-card correction as a separate reviewed
behavior change. Do not rewrite frozen expected values just to make a changed
simulation pass. This audit changes documentation only.
