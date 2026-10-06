# Task — Mobile simulation performance and session replay cache

Status: retained core session and live league integration implemented; native ARM64
measurement and additional intervention integration remain pending.
Report: [session implementation](../docs/DEEP-LIGHT-SESSION.md).
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

V8 adds the last played match day, season appearance count and sparse observed
fixture minutes to cached/replayed state. Include moved fixtures and zero-minute
DNP overrides in cache revision tests. See [v8 report](../docs/NPC-FIXTURE-WORKLOAD.md).

Approved tiering direction: [deep/light simulation](TASK-CORE-DEEP-LIGHT-SIMULATION.md).
The retained session cache is its first implementation step; detailed candidate
inspection must not force full-world weekly simulation.

V9 checkpoint: retained session cache now keys on calendar base year; tests cover
calendar age, fixture reschedules, completed IDs and partial-period save/load.
Native ARM64 and 200k capacity measurements remain pending. See
[season chronology](../docs/SEASON-CHRONOLOGY.md) for the current desktop benchmark.

V10 measured six selected leagues/2,280 detailed NPC fixtures: native release
round median 27.782 ms, season 1.070 s, annual boundary 0.773 s and peak process
RSS about 35.0 MiB. This is a 20-country desktop scenario; 200k and native ARM64
budgets remain unmeasured. DNP/minute journal compaction is now a concrete follow-up.
See [ranked deep leagues](../docs/DEEP-LEAGUES.md).
