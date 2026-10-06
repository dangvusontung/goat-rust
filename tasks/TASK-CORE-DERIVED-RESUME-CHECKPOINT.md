# Follow-up — exact derived-state resume checkpoint

Status: implemented locally in save layout 28 / unchanged SIM13; native 3/10/20
verification and final gate are recorded in [the implementation report](../docs/LOCAL-RESUME-CHECKPOINT.md).
This task was prioritized with the owner's approval after the 20-season journal-only
cold reconstruction took about 176 seconds. One optional current snapshot preserves
canonical journals and existing reconstruction as the fallback/oracle. ARM64 and
200k capacity measurements remain milestone 6 work, not acceptance claims here.

Acceptance specification: one versioned derived population/replay checkpoint at a defined clock boundary.
Keep seed+journal as canonical truth and the existing reconstruction as an oracle.
Persist only state necessary to continue without replaying every elapsed week:
identity/intake/retirement and roster membership, live attributes and clinical state,
energy/return/last-match state, exposure cursor, career/form counters, active-season
credit rollback state, manager/market state and completed league/ranking progression.
Identify fields already persisted or cheaply seed-derived before adding copies.

Bind the checkpoint exactly to simulation model, seed, calendar and its canonical
input prefix. Edited/deleted/reordered past minutes, credit ownership, scorelines or
annual inputs must invalidate it rather than silently continue. A hash alone is not
an exact equality proof; compare canonical encoded inputs or use a representation
that cannot contain an independently inconsistent prefix. Avoid coupling the core
to a renderer or keeping a checkpoint per queried date.

Past-date clinical/training queries can reconstruct the requested player on demand;
they must preserve full history, DNPs, date overrides and retirement boundaries.
Bound all allocations and reject partial, corrupt, mismatched or truncated snapshots.
Specify optional checkpoint/layout compatibility explicitly; a storage-only change
must not quietly change seeded football outputs or alter frozen legacy goldens.

Verify full vs retained vs resumed state at 3/10/20 seasons, including active youth
as well as retired genesis players. Compare attributes/medical state, identities,
manager/budget/membership/ranking, career/form counters, next-season fixtures and
save/load continuation. Measure checkpoint bytes, warm weekly cost, encode/decode,
actual cold resume and separate process memory phases. Then benchmark native ARM64
and 200k capacity under milestone 6; keep 150-country integration deferred.
