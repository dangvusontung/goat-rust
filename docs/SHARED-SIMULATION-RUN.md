# Shared simulation implementation and run — 2026-10-05

Follow-up: [red-card continuation](RED-CARD-CONTINUATION.md) is now implemented
in simulation version 4. The measurements below describe version 3.

This is the first implementation of the owner's agreed improvements 2 and 3:
shared match opportunity/conversion rules and shared PC/NPC development laws.
The TUI remains a test adapter. No dependencies were added.

## What changed

- `goat-core::match_model` separates opportunity creation from conversion.
  Midfield controls base possession; attack versus defense controls conversion.
  Exposure in minutes and available players are explicit inputs. Background
  matches use eighteen five-minute intervals rather than five goal trials.
- The detailed engine's new entry points use that attack kernel and possession
  baseline. They keep authored PC contests and tactical/zone flow. The new path
  does not apply legacy mercy/trailing-side scoring or possession bonuses.
  Actual tick duration, including a shortened final tick, scales opportunities.
- `goat-core::development` supplies the weekly growth and decay calculations
  used by the PC and NPC projection. NPC attributes develop individually with
  physical, technical and mental age curves, facilities, exposure and talent caps.
  Projection splits at age-year boundaries; tests compare it against weekly
  stepping, including the order of growth ceiling and decay.
- Shared populations derive innate talent from player identity. Changing club
  no longer regenerates talent using the new club's tactical philosophy.
  Ranking and lazy promotion use the same projected attributes.
- A date-keyed derived cache stores numeric attributes and OVR, without names
  or full player objects. Repeated ranking at the same date avoids regeneration.
  Cache data is not saved. Population identity remains columnar.

The active TUI, headless match/career harnesses, web and bridge match calls use
the new entry points. Shared population orbit reconstruction and replay-cache
constructors are available; background batch and national matches select the
resolver from the population's model.

## Compatibility and remaining coverage

Legacy public match/population APIs remain available for frozen golden tests and
explicit legacy replay. Their expected values were not rewritten. The save layout
remains 21; `SIM_VERSION` is now **3**, so guarded loading refuses saves from the
previous simulation rather than silently changing their reconstructed universe.
This change does not provide old-save migration.

The models share laws, not identical scorelines: detailed play still includes PC
contests, zone/role decisions, momentum and substitutions; background play uses
team lines and fixed intervals. Detailed match setup currently lacks explicit
home/away context, so its shared automatic attacks use neutral venue. Available
player count is supported by the kernel but detailed calls still supply eleven.
Full-time red-card continuation remains a separate pending fix.

NPC development currently uses a constant expected plan: intensity 1.0, energy
0.9, focus exposure 0.7, healthy exposure 0.9 and the medium training ceiling
0.96. Facilities come from the player's initial/intake club. Transfers do not yet
change historical training exposure. NPCs do not yet receive the PC's sampled
injury history, breakthrough events or familiarity progression. This shares the
growth/decay laws without claiming complete PC/NPC career parity. The daily
`goat-training` subsystem has not been integrated by this change.

Historical backfill and legacy world cup/continental/domestic-cup helper APIs
retain their old resolver. Active adapter-level match calls use the new resolver;
complete competition orchestration parity is still outstanding. The existing
TUI-versus-core economy/replay separation is also unchanged.

## Measurements

Release build, this cloud machine; durations are observations, not portable
performance guarantees. Commands:

```sh
source /workspace/.goat-tools/activate.sh
cargo run --locked --release -p goat-world --example bench_shared
cargo run --locked --release -p goat-tui --bin match-batch -- 10000 42
bash scripts/test.sh
```

| Workload | Legacy | Shared |
| --- | ---: | ---: |
| 100,000 background matches, strength 75 vs 75 | 2.41 ms | 21.23 ms |
| Total goals per background match | 2.511 | 2.830 |
| First scan of 26,835 NPC ratings at week 52 | 0.27 ms | 121.85 ms |
| Repeated scan at the same date | 0.25 ms | 0.13 ms |

Shared background seed 42 at strength 75 vs 75 returns **1–0**, covered by a
new frozen test. Existing legacy seed behavior is preserved independently.

10,000 detailed matches, master seed 42: **45.2% wins, 23.2% draws, 31.6%
losses**, 1.68 goals for and 1.34 against (**3.02 total**); mean rating 59.3.
The harness varies opponents, player quality and positions, so this is not a
controlled equal-team comparison. ST win rate is 55.5%, CB 37.1%; PC opportunity
and role effects need controlled calibration rather than balancing one average.
No real-football dataset was used to validate these distributions.

## Verification

Final `scripts/test.sh` completed successfully: **540 passed, 0 failed,
1 ignored** across workspace test summaries; formatting and Clippy passed.
The seed scanner passed. Seed 42 completed 20 seasons with 730 appearances,
202 goals, one league title and peak OVR 71; attribute invariants held throughout.
These career harness numbers do not certify every live-adapter path.

New tests cover the shared score golden, opportunity causal effects, the
weekly-versus-aggregated development law, talent stability after transfers,
cache/ranking/promotion agreement and automatic-versus-interactive match replay.
The promotion smoke test now checks reported promoted clubs against the new
season table rather than assuming legacy-model winners. Frozen golden
expectations remain unchanged.

## Next core work

1. Continue the match after a PC red card, close PC minutes, prevent further PC
   choices, and apply the manpower change until full time. Verify NPC goals and
   appearances as well as the final clock.
2. Add controlled deep/background experiments with equal teams, venue and
   player absences; compare opportunities, conversion and positional effects.
3. Add dated NPC exposure segments for training facilities, minutes and health
   before claiming shared career behavior. Keep deterministic save/replay and
   measured resource costs as acceptance criteria.

The **150-country expansion** stays deferred in
[TASK-CORE-150-COUNTRIES.md](../tasks/TASK-CORE-150-COUNTRIES.md).
