# Shared PC/NPC opportunities — simulation version 5

Implemented 2026-10-05 from
[TASK-CORE-PC-OPPORTUNITIES.md](../tasks/TASK-CORE-PC-OPPORTUNITIES.md).
The version-4 comparison exposed a large participation effect: an equally
rated protagonist striker raised the controlled team's winning share from
38.9% in the NPC observer to 62.9%. Version 5 allocates opportunities to teams
before choosing who resolves them.

## Behavior and accounting

Each elapsed interval reserves one opportunity record and one conversion draw.
The record contains its owning side, minutes, available players, whether an
opening was created and fixed-point quality. PC involvement and camera/role
selection happen afterwards. They can change execution and opportunity quality,
but do not allocate additional attacking minutes or scoring openings.

Authored scoring choices require an existing opening on the matching side.
Non-scoring build-up/off-ball choices remain available. If no eligible choice
remains, the flow continues with NPCs. Finishing or preventing a goal consumes
the reserved opportunity whether it scores or not. Chained actions cannot spend
it again. A turnover during build-up cancels that opening; a counterattack must
get an opportunity in a subsequent interval. A goal preserves kickoff ownership
instead of letting a following authored transition overwrite it.

Execution and conversion are distinct:

- Skill contests govern the execution component of Output/headspace/stamina.
- PC finishing and defensive contests use the same attack-versus-defense
  conversion law as NPC finishing. Selected individual attributes, pressure,
  headspace and execution affect the quality of the allocated opening.
- A well-executed shot/delivery can fail to become a goal. Baked non-goal
  templates describe it without displaying the authored goal text. Actual
  goals retain their scoring text and scorer/assist credits.

The new pressure multiplier uses 0.5% per effective point, bounded to 0.5–1.5.
Execution adjusts quality by ±0.2; combined quality can span 0.4–1.8 before the
kernel's bound. These are explicit starting assumptions, not coefficients
validated against football data. Existing global NPC creation/conversion
constants were not changed to compensate for the PC path.

`MatchResult` exposes team observations and resolved interval records. A
record's actor identifies the resolver; `goal_credits` holds the actual scorer
and assist identities. `execution_success` distinguishes finishing contests
from NPC resolution or cancelled opportunities. Created opportunities are
not synonymous with shots: a turnover can cancel an opening before a shot.

The ledger guarantees one record per elapsed interval, positive interval
durations, 90 total minutes, goals bounded by created opportunities and exact
agreement with the final score and goal-credit ledger. Substitution keeps
eleven players; dismissal closes PC minutes and subsequent intervals use ten.
There are no additional zero-minute scoring ticks after the final decision.

## Integration and compatibility

Active TUI, web, bridge and headless detailed-match calls now use
`start_match_unified` / `auto_play_match_unified`. The contextual entry point
supports policy, role-zone and chain ablations. Aggregate team matches retain
their existing shared kernel; it is the same creation/conversion law used by
the opportunity allocator.

Legacy APIs and version-4 `*_shared` entry points remain explicit compatibility
paths for existing frozen tests. Their expectations were not rewritten. The
new version-5 golden at seed 42 gives score **1–0**, Output **37**, 16 intervals,
created opportunities **3/6** and attacking exposure **42/48 minutes** in the
golden fixture. This differs from the controlled flat-60 fixture below.

Save layout stays 21; `SIM_VERSION` is **5**. Guarded loading refuses earlier
simulation saves; migration is not provided. Opportunity telemetry is derived,
not added to saves. No dependencies or renderer simulation rules were added.

## Controlled run and ablations

Two disjoint blocks, seeds 0–9,999 and 1,000,000–1,009,999, give **20,000
fixtures per condition**. Eight scenarios compare aggregate/detailed NPC
resolution; seven PC cases include three ablations: **460,000 fixtures**.
The fixed inputs and limitations are described in
[MATCH-CALIBRATION.md](MATCH-CALIBRATION.md).

```sh
source /workspace/.goat-tools/activate.sh
cargo run --locked --release -p goat-match --example calibrate_match -- 10000 0
cargo run --locked --release -p goat-match --example calibrate_match -- 10000 1000000
# Explicit v4 reproduction, excluding the v5-only ablations:
cargo run --locked --release -p goat-match --example calibrate_match -- 10000 0 --v4
```

[Raw version-5 CSV](experiments/PC-NPC-OPPORTUNITIES-V5-2026-10-05.csv)
includes both seed blocks and the full ledger measurements. Aggregate rows
do not populate the detailed-only ledger columns; zero there means unavailable.

| Fixed-profile scenario | v4 win % | v5 win % | v5 goals for / against |
| --- | ---: | ---: | ---: |
| Detailed NPC observer, equal neutral teams | 38.9 | 38.4 | 1.391 / 1.393 |
| PC striker, attributes 60 | 62.9 | 35.8 | 1.228 / 1.317 |
| PC centre-back, attributes 60 | 42.7 | 38.8 | 1.258 / 1.168 |
| PC central midfielder, attributes 60 | 51.0 | 38.4 | 1.245 / 1.168 |
| PC striker, attributes 90 | 84.6 | 40.5 | 1.371 / 1.254 |

Both NPC and PC paths allocate around **5.6 openings per side**. Every detailed
condition reconciles **90 minutes**; the mean is about 16.8 variable-length
intervals. Individual quality increases conversion and involvement within that
team budget rather than generating a separate supply of goals.

| PC-60 striker ablation | Win % | PC actions / match |
| --- | ---: | ---: |
| Default | 35.8 | 6.99 |
| No forced role-zone camera pull | 38.0 | 9.73 |
| No chains | 36.4 | 4.91 |
| First available choice | 35.4 | 6.95 |

The remaining default striker disadvantage has identifiable policy components.
PC actions cancel about **0.88 openings/match** through turnover transitions;
the NPC approximation does not run that build-up policy. Default zone pulls
also influence which authored actions are available. This is not a claim of
complete PC/NPC policy parity. Chains and choice-policy differences here are
small relative to sampling uncertainty, so do not infer a definitive ranking.
For winning shares around 40% at n=20,000, an individual normal-approximation
95% sampling interval is roughly ±0.7 percentage points; this is not a paired
effect interval or model/data uncertainty estimate.

The heterogeneous production `match-batch 10000 42` run gives **2.50 total
goals/match**, W/D/L **34.6%/24.1%/41.3%**, mean Output **57.2**. Its inputs vary
players/opponents and differ from the controlled fixtures. Neither run has
been fitted to a real competition dataset.

## Verification and performance

Full quality gate: **548 passed, 0 failed, 1 ignored**; formatting, Clippy,
career attribute invariants and the seed scanner passed. New tests exercise
ledger reconciliation across roles/chains, live/automatic replay, dismissal
continuation, good execution without a goal, late injury-return cameos and
manager hooks. Existing legacy/v4 goldens remain unchanged. The career-sim
attribute harness does not certify every live orchestration path; the match
regressions and TUI smoke season independently exercise the new match engine.

A warm release comparison used 40,000 matched fixture inputs per run, three
runs per model, with compilation excluded and ablations disabled. Median
wall times on this cloud machine were **1.13 s for v4** and **1.10 s for v5**.
This mixed aggregate/NPC/PC workload showed no material performance regression;
it is not a production-scale latency or memory guarantee.

## Remaining core work

NPCs still use average team finishing rather than the PC's full decision policy.
Opportunity quality is a scalar semantic approximation; it does not model shot
coordinates, body orientation, goalkeeper actions or a full tactical response
to a dismissal. Counterattacks within one interval, chance transfers and
position-dependent execution/selection deserve explicit future modeling.
Venue support exists in the context API, but default detailed adapters still
use neutral venue until fixture context is wired through shared orchestration.

Next: [dated NPC training/health exposure](../tasks/TASK-CORE-NPC-EXPOSURE.md),
while keeping deterministic reconstruction and measuring resource costs. Keep the 150-country task separate.

Latest development update: [version 6 dated PC/NPC training and health](DEVELOPMENT-HISTORY.md).
The match experiment and version-5 save discussion above remain historical.
