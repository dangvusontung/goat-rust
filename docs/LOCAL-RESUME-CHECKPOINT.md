# Local resume checkpoint — save layout 28 / SIM13

Subsequent [SIM14 dated competitions](DATED-COMPETITIONS.md) use layout 29 and
checkpoint format 2, including exact calendar binding. The layout-28/SIM13 figures
below remain the historical league-only baseline.

User-approved storage work after the 20-season v13 cold reconstruction took
175.884 seconds. One optional derived-state checkpoint is stored in the local
save. The seed and complete journals remain canonical. Football, development,
health and ranking rules stay SIM13; this does not change frozen match outputs.

## What is persisted

The checkpoint contains the session's full SoA population: identity, youth intake,
retirement, attributes, potentials, keeper proxies, career/form counters, exposure
links, minute dictionary/indices, development caches and clinical/availability state.
It also contains annual replay progress, budgets, academy boosts, manager state,
league membership and ranking history; mutable generated-world budgets, academy
levels and tactics; completed-season compressed session journals; active-season
journals and original career counters needed for rollback. Static generated world
metadata is reconstructed from the seed. Cache allocation capacity is not persisted.

This is one current resume snapshot, not a snapshot for every historical date.
Training and medical history remain queryable through the original dated projection
and journals. The TUI only calls the save/load API; it owns no simulation logic.
The separate `goat-training` crate is not newly integrated by this storage change.

## Binding, validation and recovery

Cache format 1 includes model 13, seed, calendar year, season/day, realistic-mode
flag, scopes, scorelines, cards and losslessly encoded canonical minute/credit inputs.
Load checks their **full exact equality**, including ordering and ownership; it does
not substitute a hash comparison for journal equality. It also checks checksum,
framing, trailing bytes, vector alignment, indices, exposure links, fixture indices,
league/manager assignments and replay progress before installing the cache.

The 64-bit FNV checksum detects accidental corruption; it is not cryptographic
protection against a deliberately forged file. Valid data is bounded to 256 MiB
of checkpoint payload. Decoders check length against available bytes and grow
collections as elements decode rather than reserving from untrusted counts. A
separate 1 GiB aggregate collection-storage accounting budget includes conservative
vector growth and map-node overhead; this is a decoder safeguard, not a mobile RAM
target or a bound on the complete game process.
The producer currently checks the payload limit after serialization.

A missing, unsupported, corrupt or mismatched optional checkpoint is discarded;
normal seed-and-journal reconstruction still works. A malformed/truncated save
container is rejected rather than loaded partially. Save layout 27 with the same
SIM13 remains accepted. Older incompatible simulation versions remain rejected.
Save layout 28 without a checkpoint also remains valid.

Local saves are encoded first, written to a new sibling temporary file, synced,
then atomically renamed over the destination. Failures preserve the previous save;
a temporary file that was not created by this call is never deleted. This is an
atomic replacement, not a claim of directory durability across power loss.

`from_world_state_with_session` exports a synchronized checkpoint;
`session_from_save` restores it or returns a fresh replay session. Plain
`from_world_state` still creates a journal-only save. The first export from an old
save/fresh session must reconstruct derived state once. A clock or annual-phase
change requiring synchronization can also trigger existing replay behavior; the
checkpoint does not change that model to make every possible save instantaneous.

## Verification and measurements

Focused tests compare complete serialized cached state before/after resume and
then verify attributes, keeper/form counters, medical/training/injury history,
youth, rankings, next fixtures and the next annual boundary against fresh replay.
Edited inputs and malformed cache/container data exercise rejection and recovery.
New tests freeze primitive encoding without changing legacy goldens. Changes to
the serialized field order or representation must bump the checkpoint format; an
unsupported optional cache then falls back to canonical reconstruction.

The native benchmark executes real rounds in six deep leagues, seed 42, base year
2023, with the PC league 59. At 3/10/20 seasons it writes and reads actual files,
requires zero replay rebuilds on first query, checks the whole cache byte-for-byte,
compares fresh replay, and verifies the following season's first fixture. Disk
write timings include encoding and file sync. Load timings include disk read,
save decode, world generation, cache validation/restoration and first population
query. Fresh-process loads do not flush the operating system file cache; they
measure session-cold resume, not guaranteed cold physical-storage reads.
Measurements are cloud samples on x86_64; native ARM64 and 200k capacity
remain separate work, and expanded canonical journals still cost RAM.

Reproduce:

```sh
source /workspace/.goat-tools/activate.sh
cargo build --release -p goat-save --example bench_checkpoint
python3 scripts/measure-native.py docs/experiments/checkpoint-native.txt \
  target/release/examples/bench_checkpoint --years 20
# Independently measure one loaded game without diagnostic duplicate sessions:
python3 scripts/measure-native.py docs/experiments/checkpoint-load.txt \
  target/release/examples/bench_checkpoint --load /tmp/goat-checkpoints/seed-42-season-20.gsav
```

The workspace gates overlap parts of the long career run. Cumulative whole-run
wall time includes diagnostics and that overlap, so it is not used to claim
simulation throughput. Independent load-only runs with the final binary measure
resume latency and memory after both simulation and workspace gates finish.

## Actual results — 2026-10-06

MB below is decimal; MiB is binary. Export and write columns are single diagnostic
samples. Resume columns use three separate fresh processes with the final decoder,
after all long-running simulation and quality gates finished.

| Seasons | Journal-only save MB | Checkpoint MB | Full save MB | Export ms | Atomic encode/write ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| 3 | 2.75 | 19.62 | 22.37 | 153.277 | 28.650 |
| 10 | 10.61 | 51.42 | 62.03 | 476.655 | 112.627 |
| 20 | 23.90 | 107.57 | 131.47 | 1055.494 | 307.937 |

| Seasons | Fresh seed replay s | Final fresh-process load median s (3 samples) | Loaded resident MiB | Load peak MiB (max of 3) |
| --- | ---: | ---: | ---: | ---: |
| 3 | 1.530 | 0.168 | 38.46 | 66.23 |
| 10 | 43.517 | 0.574 | 95.15 | 189.67 |
| 20 | 177.271 | 1.347 | 199.89 | 408.19 |

All three saved snapshots were byte-exact after restoration; every fresh-process
load used the checkpoint with **zero rebuilds**. Fresh replay matched identity,
career/form/keeper counters, full sampled attributes and medical/training/injury
histories, active youth and ranking. The following season's first fixture matched
scorelines, cards, all minute rows and all credits at every milestone, including
season 21 after the 20-season save.

The 20-season file is 131,472,911 bytes (125.38 MiB), about 5.5 times the
journal-only save. Fresh-process resume drops the roughly 177-second reconstruction
to about 1.3 seconds on this cloud host. This is a load-time improvement, not faster
football physics. Export from the already synchronized 20-season session took
about 1.06 seconds, followed by about 0.31 seconds for encoding and atomic write.

At 20 seasons the long-running warm process retained about 389.73 MiB, whereas
freshly restored game-only processes retained about 200 MiB after releasing save
buffers; allocator capacity and historical query/cache patterns differ. Load still
peaks around 408 MiB. The diagnostic process reached 867.02 MiB with duplicate
saved data and simulation sessions; that is not game-only resident memory. Expanded
canonical journals and overlapping decode/state buffers remain optimization targets.
Native ARM64 measurements are required before mobile acceptance.

Final gate: **625 passed, 0 failed, 1 ignored**; format, warnings-denied Clippy,
career invariants and seed scanner all passed. No legacy frozen goldens changed.
Changes are local; this work has not been committed or pushed.

Evidence: [continuous native run](experiments/LOCAL-CHECKPOINT-2026-10-06-native.txt),
[final full gate](experiments/LOCAL-CHECKPOINT-2026-10-06-final-gate.txt),
[structured metrics and three load samples per milestone](experiments/LOCAL-CHECKPOINT-2026-10-06-metrics.json),
[20-season load sample](experiments/LOCAL-CHECKPOINT-2026-10-06-load-20-1.txt).

Next realism milestone remains [4 — dated competition calendar](../tasks/TASK-CORE-DATED-COMPETITIONS.md).
The [150-country expansion](../tasks/TASK-CORE-150-COUNTRIES.md) stays in the backlog.
