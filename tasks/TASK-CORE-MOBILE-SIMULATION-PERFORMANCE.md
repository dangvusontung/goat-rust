# Task — Mobile simulation performance and session replay cache

Status: next, after v7 individual NPC training/health.
Context: [measured core costs](../docs/DEVELOPMENT-HISTORY.md).

Retain the current lived population/replay cache across match rounds instead of rebuilding
all completed seasons for every match. Key it by world seed, model version, season
and orbit/intervention revision. Apply newly recorded orbit events once; invalidate
and reconstruct correctly if a loaded save or earlier input changes. Keep core
logic independent of the test TUI and put mobile calls on a worker thread.

Compare cached vs fresh replay for attributes, club membership, workload history,
career credits and form across save/load, transfers, season boundaries and changed
records. Avoid keeping an unbounded cache of dates or a heap player per NPC.

Measure release native ARM64 on a midrange Android device and an iPhone when
hardware/build support is available: cold resume, one-week progression, full match,
one-season advance, peak RAM and sustained performance. Cloud v6 measurements are
not device certification. Preserve deterministic numeric outputs and existing
frozen compatibility APIs. No new dependencies or renderer work is required for
the core cache. The 150-country task remains separate.

V7 cache state includes energy, remaining injury duration and rolling availability
mask as well as attributes. Test those fields and individual injury/training
history when reusing or rebuilding the session cache. See [v7 measurements](../docs/NPC-INDIVIDUAL-LIFE.md).
