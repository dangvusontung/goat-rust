# Season chronology — owner decision, 2026-10-06

Latest implementation: [v10 ranked deep leagues](DEEP-LEAGUES.md) — PC league plus
top 5, named NPC fixture minutes/credits, authoritative standings replay; save 25.

Status: implemented as simulation version **9**, save layout **24**. New TUI
careers enable the dated model; existing legacy APIs and frozen scenarios remain
available. Incompatible simulation saves are rejected, not silently migrated.

## Competition season and summer interval

A club competition season starts **15 August of year Y** and ends **30 June of
Y+1**, inclusive. This is the season frame, not a requirement that every league's
opening fixture occurs on 15 August. League scheduling must fit inside that frame.
The interval **1 July through 14 August**, inclusive, is 45 days of summer time:
rest/rehabilitation, national-team commitments and club friendlies/preparation.
These activities can consume energy; summer is not an automatic full-health reset.

National matches can also occur during the competition season. A national tournament
may cross 30 June without being restarted or erased. Its own competition identity,
fixtures, suspensions and progress survive the club season boundary. International
and friendly minutes count toward workload/recovery, but not domestic league apps.

## One timeline

Use a single monotonic epoch day for every player, fixture, training/recovery event,
market action and simulation tier. Store/derive explicit start/end dates in the
season frame. Competition-relative round/slot and season IDs do not replace dates.
A rescheduled fixture's actual scheduled date drives its workload and rest gaps.

Training remains a seven-day cadence on that timeline. Weeks may cross a season
boundary; do not discard remaining days or reset the cadence, energy or injuries.
Background simulation and deep simulation must interpret each day identically.
Age progression cannot rely on treating every calendar year as exactly 52 weeks.

Calendar arithmetic must define leap-day handling centrally. The implementation uses deterministic Gregorian date arithmetic, with a 320-day competition season
(321 when it contains 29 February) and the same 45-day summer interval. Gregorian leap-year handling is centralized in `goat-calendar::chronology`.
No runtime system reads wall-clock time. The already saved career_base_year supplies
the absolute-year anchor; preserve the old July-1 career-start epoch where possible.

## Boundary and replay requirements

- Close domestic results and awards once after 30 June. Continue medical and
  training state into July. The next competition season becomes active on 15 August;
  summer preparation may create its schedule earlier without advancing time.
- Attribute a fixture to its explicit competition/season identity; attribute its
  load to the actual day. A calendar season rollover must not double-count either.
- Distinguish the closed domestic season, next planned season and an ongoing
  national tournament instead of forcing all three through one season counter.
- Seasonal light replay must finish only events whose dates have elapsed. Summer
  intake, transfers and preparation need dated events rather than future events
  being applied at a convenient end-of-season batch boundary.
- Cache keys/invalidation use resolved date/season frame and authoritative journal
  inputs. Replace session's `(season - 1) * 364` checks with canonical boundaries.
- Replace workload fixture dates, core season/window mapping, age calculations,
  weekly history, adapter date labels and national/continental scheduling together.
  Audit every `*52`, `*364` and `*365` instead of globally replacing constants.
- Version changed simulation behavior explicitly. Retain frozen v8 factories/tests;
  either provide a tested old-save migration or reject incompatible simulation saves.

## Acceptance scenarios

Check 14/15 August, 30 June/1 July, a week spanning each boundary, multi-year and
leap-year dates, birthday/age transitions, rescheduled fixtures, an injury starting
in June and recovering in July, a July friendly, a national tournament spanning
June/July, and domestic statistics excluding non-league games. Retained/fresh replay,
deep/light contact and save/load must agree at the same epoch day. No drift after
20 seasons, no skipped training days and no duplicate season closing.

The 150-country task must use the agreed shared season frame initially; any future
country-specific season exception requires an explicit design decision.

## Implementation checkpoint

`Chronology` derives civil dates, competition frames and calendar ages from the
saved July-1 epoch and base year. Core date advancement preserves the global
seven-day cadence across July 1, partial periods, injuries and fatigue. Weekly
training injury onset is recorded at period completion, not before it occurs.
Fixture rescheduling retains stable round IDs; played fixture IDs prevent duplicate
PC credit. Save/load persists the calendar mode, reschedules and completed IDs.

Dated population factories and `SimulationSession::population_dated` use the same
frames. Youth intake occurs at the next July 1; retirement follows calendar age.
NPC fixture plans are shared and lazy, with no league doses outside the frame.
Observed summer national/friendly doses still affect health without adding league
appearances. Cache invalidation includes the base year and actual season boundary.

The TUI defaults to this model; `--legacy-calendar` supports frozen scenarios.
Web/bridge new-game adapters remain on legacy factories until separately migrated.
The calendar supports summer activities; a complete dated friendly/national fixture
scheduler is still pending. National tournament progress survives club rollover in
memory, but is not yet persisted. Cup adapter dispatch still uses round slots.
Seasonal market actions remain aggregate passes rather than dated winter transfers.

Current health/training realization is weekly: partial days remain pending until
their period completes. This is not a daily clinical model. Initial NPC birthdays
are anchored to July 1; varied dates of birth remain a realism improvement.
V10 adds permanent league selection and conserved named outfield minutes.
Transient opponent selection and explicit keepers remain pending; chronology
and ranked leagues do not complete the entire deep/light roadmap.

Release measurements (seed 42, year 2023, season 5, 32,131 players): retained
session synchronization plus one lineup had median 0.168 ms; fresh reconstruction
median 2,420.465 ms and cold retained initialization median 2,423.335 ms across
three release runs. Maximum measured process RSS was about 35.8 MiB. These figures are
not full-match timings, a 200k-player benchmark or proof of mobile performance.
Raw measurements: [run 1](experiments/SEASON-CHRONOLOGY-V9-2026-10-06-run1.txt),
[run 2](experiments/SEASON-CHRONOLOGY-V9-2026-10-06-run2.txt),
[run 3](experiments/SEASON-CHRONOLOGY-V9-2026-10-06-run3.txt).
Verification: [workspace gate](experiments/SEASON-CHRONOLOGY-V9-2026-10-06-gate.txt),
including leap years, 20-season dates, partial weeks, injuries, summer non-league
loads, June-30 moved fixtures, cache/fresh equivalence and save/load continuation.

Final workspace gate: **583 passed, 0 failed, 1 ignored**; formatting, Clippy,
career invariants and seed scanner passed. Frozen golden expectations unchanged.
