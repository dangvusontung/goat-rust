# Exact journal checkpoints and runtime storage

Priority: finish medical/history milestone 2 before increasing the deep budget.
Baseline: v12, layout26/SIM12; [medical and codec report](../docs/MEDICAL-JOURNAL.md).

The three-season release diagnostic decodes bytes in 4.301 ms but takes 3.085 s
to reconstruct the NPC population and verify fingerprints. It keeps 364,306 minute
rows and 177,422 credits. File compression leaves runtime Vec journals unchanged.
Its 145,280 KiB peak includes two simulation sessions and diagnostic save clones;
measure the single production session separately before setting a mobile budget.

## Implementation and acceptance

- Measure single-session residency, peak encode/decode allocations and cold replay
  separately at 3/10/20 seasons; compare against the retained path.
- Design an exact completed-season checkpoint or compressed in-memory journal.
  Do not approximate medical chronology or replace dated inputs with career totals.
- Retain stable NPC identity, training/exposure segments, health/energy, last match
  date, form, career counters, membership, manager/market state and ranking history
  wherever needed to replay the next season without replaying the entire career.
  Identify what remains seed-derived before persisting any checkpoint field.
- Preserve zero-minute DNPs, actual fixture date overrides, cross-competition loads,
  scorelines and credit ownership. Past input edits must invalidate the checkpoint
  or explicitly be rejected; they must not silently reuse stale canonical state.
- Compare full replay, retained replay and checkpoint resume at the same date:
  player attributes, career/form fingerprints, training and injury histories,
  promotion/market/coefficient outcomes and the next season's deep fixture results.
- Harden truncated/invalid checkpoint decoding and specify save/SIM compatibility.
- Report native memory and time independently of ARM64 measurements; do not claim
  mobile readiness from desktop results. Keep 150 countries deferred.

NPC return-to-play rotation, fatigue substitutions and discipline remain the next
football behavior milestone after this storage/replay bottleneck is addressed.

## Completion — v13, 2026-10-06

Implemented the compressed in-memory journal alternative: shared lossless codec,
completed-season packed session snapshots, exact streaming invalidation, fixture
metadata dictionary and indexed player minute rows. Full canonical WorldState
vectors and annual cold reconstruction remain; no derived population checkpoint
is persisted. Legacy goldens are preserved. See [implementation and measurements](../docs/CORE-STEPS-2-3.md).
Medical/training histories, career identity, rankings and future fixture replay are
verified against fresh reconstruction. Past minute/credit edits force a rebuild.
3/10/20-season native measurements distinguish one-session/no-codec, codec/save
allocations and cold replay with a second session; ARM64 remains milestone 6.

Subsequent user-approved storage work implements the optional derived-state
checkpoint in layout 28 / unchanged SIM13. The preceding completion describes
the journal-only baseline. See [local resume checkpoint](../docs/LOCAL-RESUME-CHECKPOINT.md)
for 3/10/20-season continuation, file size, resume and memory measurements.
