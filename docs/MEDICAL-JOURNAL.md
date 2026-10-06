Latest: [v13 runtime compression and NPC return-load integration](CORE-STEPS-2-3.md)
completes the runtime follow-up. The measurements and remaining-task statements
below describe the v12 historical checkpoint.

# Medical continuity and lossless journals — v12, 2026-10-06

The dated PC/NPC simulation already carries training, energy and injuries through
June/July and annual replay. This change corrects recovery reporting and reduces
the serialized match journal without deleting the inputs needed to reproduce it.

## Medical dates

PC rest recovery now records the endpoint of the elapsed health week, matching
the recovery event in development history. Previously it used the adapter clock,
which can be ahead while older weeks are being caught up. Training recovery already
used the elapsed endpoint. The legacy undated rest path remains unchanged.

NPC injury episodes distinguish expected recovery from observed recovery. A return
is recorded only after replay actually sees remaining injury weeks reach zero.
An injury still active at the queried date, or when simulation stops at retirement,
does not get a fabricated completed recovery.

`WorldState::pc_medical_status()` and `Population::medical_status()` expose the same
read-only phase: injured, returning, or healthy, along with energy and the last
observed return week. The two-week returning window preserves the existing PC cameo
context and now lives in core rather than being calculated in the TUI. A future
return date is not treated as a completed recovery. A new injury takes priority.
NPC status requires the individually sampled model; legacy expectation-only
factories and dates outside simulated player life do not report a diagnosis. NPC detailed history is an on-demand readout
for players of interest, not a per-player squad-selection loop.

The returning flag is advisory for NPCs. It does not yet impose NPC minute limits,
introduce rehabilitation exercises, model tissue-specific injuries or recurrence,
or change injury rolls and attribute growth. Weekly recovery resolution remains.
Those behavior changes need separate calibration and belong with squad selection
and injury realism; the current flag is not a clinical fitness guarantee.

## Compact save layout 26

All NPC workload rows remain present, including zero-minute DNPs. A dictionary
stores each `(competition, fixture ID, actual date)` once. Rows encode a dictionary
reference, a signed player-index delta and minutes as bounded integer varints.
Using the actual date in the key preserves overrides even if one fixture has
multiple recorded dates. Original row order is restored exactly; no sorting,
merging, appearance inference or RNG calls occur in the codec.

Orbit credits also use player-index deltas, preserving their original order,
goals, assists and signed result. The delta uses wrapping u32 arithmetic plus
zigzag encoding, so descending indices and the entire u32 identity range round-trip.
The decoder rejects oversized counts, missing dictionary references, overflowing
or noncanonical integers, invalid minutes and truncated rows before using them.
No new dependencies were added.

The old layout reader remains available for explicit layout-only inspection.
Normal loading still enforces simulation compatibility: SIM12 is required because
the PC recovery-date correction can affect its existing return cameo context.
Older SIM11 saves are rejected; there is no automatic career migration.

This compacts files, not the runtime `Vec` journals. Past workload and credit rows
cannot simply be deleted or replaced by career totals: medical replay and future
form/growth depend on when and how those events happened. Runtime compaction needs
an exact checkpoint or compressed in-memory schedule and remains a capacity task.

## Verification

Tests cover cross-season NPC injuries, active versus observed recovery, retirement
before recovery, PC clock catch-up, return-window boundaries, nonmonotonic and
maximum identities, multiple competitions, fixture dates, DNPs, malformed compact
data and legacy layout inspection. A multi-season save/load test compares PC
medical state, NPC career fingerprints, full training histories, medical readouts
and subsequent PC recovery.

[Workspace gate](experiments/MEDICAL-JOURNAL-V12-2026-10-06-gate.txt).
The release diagnostic uses actual seeded NPC fixture journals over three seasons;
it reports serialized size separately from cold replay and verifies retained versus
loaded replay. It does not run the PC beat engine or establish mobile throughput.


## Run report

Full workspace gate: **605 passed, 0 failed, 1 ignored**. Formatter, Clippy, career
invariants and seed scanner passed. The release diagnostic was also built and run;
a final example-only correction initialized the PC identity required by the save
API and passed focused Clippy/build/run checks.

| Completed seasons | Minute rows | Credit rows | Old journal bytes | Compact journal bytes | Whole save bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 | 113,088 | 59,139 | 3,131,057 | 583,442 | 657,429 |
| 2 | 234,536 | 118,308 | 6,462,956 | 1,198,883 | 1,345,874 |
| 3 | 364,306 | 177,422 | 9,994,198 | 1,845,282 | 2,065,277 |

The three-season minute/credit payload shrank **81.5%**. The old payload size is
computed using the previous layout's exact field widths for the same rows; this
is not a second simulation run under different football rules. Round-trip equality
and cold/retained career, medical and training-history equality were asserted.

One release x86_64 Linux sample: third-season encode 12.656 ms, decode 4.301 ms,
cold reconstruct-and-compare 3.085438 s. The diagnostic's peak RSS was 145,280 KiB
(about 142 MiB), including two sessions, save-data/base clones, decoded rows and
codec buffers; this is not the memory footprint of a single gameplay session.
The row counts and file size still grow with career length. Three seasons are a
medium-term replay check, not a retirement-length or mobile capacity result.
[Raw run](experiments/MEDICAL-JOURNAL-V12-2026-10-06-native.txt).

This result makes [exact runtime journal/checkpoints](../tasks/TASK-CORE-JOURNAL-CHECKPOINTS.md)
the recommended next optimization: the bottleneck is historical replay rather
than compact byte decoding. Keep the current deep budget while addressing it.
