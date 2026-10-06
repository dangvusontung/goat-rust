# Controlled match comparison — 2026-10-05

This report describes v4. The subsequent [v5 opportunity implementation](PC-NPC-OPPORTUNITIES.md)
contains updated outcomes and raw measurements. To reproduce v4, append `--v4`
to the commands below.

The shared NPC kernel behaves similarly in aggregate and detailed resolution.
The largest measured difference appears when the protagonist participates:
an equally rated PC striker gives the controlled team a much larger winning
share than the all-NPC observer. Changing global NPC scoring constants would
not address that difference.

## Method and reproducibility

Run **20,000 fixtures per condition**, in two disjoint seed blocks: 0–9,999
and 1,000,000–1,009,999. Eight scenarios compare two engines, and four scenarios
isolate PC participation: **400,000 fixtures** in the saved sample.

```sh
source /workspace/.goat-tools/activate.sh
cargo run --locked --release -p goat-match --example calibrate_match -- 10000 0
cargo run --locked --release -p goat-match --example calibrate_match -- 10000 1000000
```

[Raw CSV](experiments/MATCH-CALIBRATION-2026-10-05.csv) contains both blocks,
including seed ranges, team goals, W/D/L, NPC chances, NPC goals, NPC attacking
minutes, PC actions, red-card share and authored scoring stakes.

The neutral reference has attack/midfield/defense 60 versus 60, flat NPC
attributes 60, neutral tactical weights, no bench substitutions and a neutral
venue. Each intervention changes one team line, venue or initial player count.
The ten-player experiment starts short-handed at kickoff; it does not model
a timed dismissal. The PC experiment uses flat attributes, Natural familiarity,
neutral form/staff/traits and automatic decisions. Team profiles stay fixed at
60 even for the PC-90 ablation, deliberately isolating the individual input.
These are synthetic diagnostic inputs, not realistic generated squads.

The detailed observer replaces the protagonist slot with an NPC and disables
PC decisions, substitutions and PC discipline. It uses the actual detailed
flow, including variable ticks, zone transitions, commentary RNG and momentum.
Both engines receive the same seed and input per fixture, but consume RNG
differently, so individual scorelines are not expected to match.

## Results

Means below combine both equally sized blocks. Goals are per match; winning
share is relative to the first team. Values are rounded independently.

| Intervention | Aggregate goals for / against | Detailed NPC goals for / against | Aggregate win % | Detailed NPC win % |
| --- | ---: | ---: | ---: | ---: |
| Equal, neutral | 1.371 / 1.386 | 1.383 / 1.369 | 37.5 | 38.9 |
| First team at home | 1.427 / 1.386 | 1.427 / 1.367 | 38.9 | 39.9 |
| First team away | 1.371 / 1.441 | 1.382 / 1.415 | 36.4 | 38.0 |
| First team's attack 90 | 1.649 / 1.386 | 1.680 / 1.354 | 44.4 | 46.2 |
| First team's defense 90 | 1.371 / 1.110 | 1.390 / 1.096 | 43.5 | 44.5 |
| First team's midfield 90 | 1.784 / 0.973 | 1.808 / 0.946 | 57.1 | 58.3 |
| First team has ten players | 1.170 / 1.543 | 1.164 / 1.546 | 29.5 | 30.3 |
| Opposition has ten players | 1.531 / 1.177 | 1.540 / 1.173 | 46.1 | 46.7 |

Home advantage increases opportunity creation in both engines. Stronger attack
increases conversion; stronger defense reduces opposition conversion; stronger
midfield changes opportunity share. Missing players reduce opportunities and
defensive coverage. All measured interventions act in the expected direction.

Equal neutral teams create about **5.6 NPC chances per side**, converting about
**24.7%**. Their total goal means differ by only **0.005 goals/match**. The
detailed observer nevertheless draws less often: **23.4% versus 25.1%**.
Momentum, tick granularity and draw-stream differences remain candidates for
an ablation; similar means do not establish distributional equivalence.

| Participation, fixed team profiles | Goals for / against | Win % | PC actions per match |
| --- | ---: | ---: | ---: |
| All NPCs, detailed observer | 1.383 / 1.369 | 38.9 | 0 |
| PC striker, attributes 60 | 2.164 / 1.028 | 62.9 | 10.15 |
| PC centre-back, attributes 60 | 1.568 / 1.377 | 42.7 | 10.86 |
| PC central midfielder, attributes 60 | 1.829 / 1.246 | 51.0 | 10.69 |
| PC striker, attributes 90 | 3.078 / 0.809 | 84.6 | 14.28 |

The PC-60 striker result repeats in both blocks (62.48% and 63.28% wins).
The NPC observer is 38.83% and 38.90%. The large difference persists across
blocks; these repetitions do not replace a formal uncertainty analysis or
real-football validation.

For the PC-60 striker, NPC automatic attacks cover only about **52 of 90
minutes**. Authored PC actions replace the remaining flow and supply around
**1.38 additional team goals** beyond the 0.79 automatic NPC goals for his
side. NPC-only opportunity counters therefore intentionally exclude PC actions.
The CSV's authored scoring-stake counts are opportunities for a choice branch
to score/concede, not comparable-quality shots or measured expected goals.

## Interpretation and implementation

Source inspection identifies different laws for the two action paths:
`auto_beat` uses shared chance creation/conversion; `resolve_choice` applies an
authored score event directly when its contest succeeds/fails. Involvement,
role-zone pulls, trailing-side focus, chains and the automatic choice policy
also affect how often PC scoring stakes appear. This experiment exposes their
combined effect; it does not prove a single cause or that all PC advantage is
unjustified. NPCs do not currently run the same decision policy as the PC.

Added core capabilities:

- Explicit aggregate `MatchContext` with home/away/neutral venue and initial
  manpower, preserving the old shared default wrapper and frozen seed score.
- Numeric `MatchObservations` for NPC chances, goals and attacking minutes.
- Detailed observer entry points with matched venue/manpower inputs, and
  observation counters on detailed results without extra RNG draws.
- One shared manpower-coverage helper used by both resolutions.

The active adapters' default behavior remains neutral venue in detailed play.
The contextual observer API enables measurement; this step does not wire venue
through every competition adapter or change scoring constants. Save simulation
version stays **4**, and the added observations are not persisted.

Validation: **543 passed, 0 failed, 1 ignored**, plus formatting, Clippy,
20-season career invariants and the seed scanner. New tests check observer
replay, full 90-minute exposure, no protagonist events, goal accounting and
venue/manpower effects. Legacy and version-4 dismissal goldens still pass.

## Next implementation

Implement a shared opportunity record/budget for authored PC and automatic NPC
actions, with explicit opportunity quality and goal conversion. Keep semantic
choices and their consequence text consistent; do not halve PC scoring or
raise NPC scoring merely to match one aggregate mean. The detailed task is
[TASK-CORE-PC-OPPORTUNITIES.md](../tasks/TASK-CORE-PC-OPPORTUNITIES.md).

Dated NPC training/health exposure and the deferred 150-country expansion
remain subsequent tasks. No external football dataset was fetched in this run.
