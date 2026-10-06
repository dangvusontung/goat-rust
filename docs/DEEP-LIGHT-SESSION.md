# Retained deep/light simulation session — 2026-10-06

Latest implementation: [v10 ranked deep leagues](DEEP-LEAGUES.md) — PC league plus
top 5, named NPC fixture minutes/credits, authoritative standings replay; save 25.

Latest checkpoint (2026-10-06): [implemented v9 season chronology](SEASON-CHRONOLOGY.md),
15 August–30 June, continuous training/health and dated replay; save layout 24.
Legacy factory descriptions below remain applicable to their compatibility paths.

The first implementation step of the approved deep/light plan is complete:
`goat_world::session::SimulationSession` retains one v8 population and its light-world
`ReplayCache` across deep league matches. The TUI game loop owns the session and
passes authoritative orbit-credit and NPC-minute journals into it before selection.
No simulation-version/layout change is needed: this preserves fresh v8 outcomes.

## Synchronization

`population(seed, season, orbit_records, npc_match_loads)` returns the same
population while current-season journals append. It applies only newly appended
credits and loads; player health/attribute projections remain lazy and keep their
numeric caches. Journal prefixes are compared exactly, so a hash collision cannot
silently reuse stale state. The session keeps one journal snapshot, extending only
its new suffix; it does not keep a population for every historical date.

New seeds, a backwards season, deleted/edited/reordered inputs and late observations
or credits belonging to completed seasons trigger a safe canonical rebuild.
`clear()` explicitly discards the session, for example when replacing a career.
Loads for youth not yet created remain pending until seasonal intake creates them.

During the current season, orbit credits update career appearances/goals and form
immediately. The session remembers those columns' original values for touched NPCs.
Before advancing the season, it restores them and lets ReplayCache apply the season's
journal once, alongside the existing batch remainder accounting. This prevents
both duplicate career credits and applying the form EMA twice. Subsequent season
records are then applied incrementally. Minutes have their own idempotent ingestion.

The retained world is the canonical world used by the existing v8 reconstruction,
including market, club budget, managers, intake and promotion processing. It does
not import arbitrary renderer world mutations or new intervention types. Additional
path-dependent inputs must become explicit journals/revision inputs before this
API can safely consume them; previous fresh v8 behavior is preserved here.

## Current depth boundary

The PC fixture still uses the match engine. The current division's other fixtures
use the existing cheaper round simulation. Distant leagues retain seasonal batch
processing, with individual health/development reconstructed on contact. Keeping a
session does not introduce a mandatory weekly tick for every NPC, or a new per-NPC
heap player object. Player contact still uses Population's existing lazy promotion.

This API belongs in the headless core and may be owned by a mobile worker. It is a
mutable single-owner cache, not a shared concurrent world. Save only authoritative
journals; a new session reconstructs from the loaded seed/season/journals.

## Validation

New tests compare retained vs fresh reconstruction for identity/membership,
career fingerprints, form, exposure, attributes, health availability, observed
loads and weekly training/energy history. Cases include repeated reads, journal
appends, consecutive season transitions, changed minutes/rescheduling/DNP,
changed old credits, late completed-season records, backwards time, changed seed,
explicit clear, pending youth observations and backwards attribute reads.
The save-byte round-trip test checks a freshly resumed session's training history
against the original session. The live TUI 38-round save smoke test remains active.

## Remaining work

The rest of the deep/light task is still pending: a headless bounded orbit-selection
policy refinements and full background XI/minute conservation,
cup/national observation integration and a measured 200k capacity scenario.
National synthetic paths and the one-off generation review still use their existing
construction paths. No claim of native Android/iPhone performance is made.

## Recorded release run

Full gate: **574 passed, 0 failed, 1 ignored**; format, Clippy with warnings denied,
career invariants and seed scanner passed. Existing golden values were retained.
[Gate output](experiments/DEEP-LIGHT-SESSION-2026-10-06-gate.txt).

Seed 42, season 5, 32,133 population rows after four seasons of replay. Three
sequential native cloud x86_64 release runs, Rust 1.90.0. Median first session
construction plus one club's 16-player lineup is **2044.234 ms**. The next two
queries per run append one NPC credit and minute observation, advance the queried
week and select the lineup: retained **0.149 ms**, fresh **2037.469 ms**.
Both paths assert identical lineups, OVR sums and career fingerprints.

These timings measure world synchronization and one lineup query, **not the match
engine, a full week for all NPCs, native mobile performance or 200k capacity**.
The append benchmark uses one observed NPC rather than a full squad journal.
Cold resume and season-boundary replay remain substantial; only the repeated
completed-history replay between current-season rounds is eliminated.

Peak child RSS across the example runs is 33,664 KiB (about 32.9 MiB), with both
retained and fresh populations live during comparison; it is not isolated cache
memory. Reproduce with `cargo run --release -p goat-world --example bench_session`.
[Run 1](experiments/DEEP-LIGHT-SESSION-2026-10-06-run1.txt),
[run 2](experiments/DEEP-LIGHT-SESSION-2026-10-06-run2.txt),
[run 3](experiments/DEEP-LIGHT-SESSION-2026-10-06-run3.txt),
[resources](experiments/DEEP-LIGHT-SESSION-2026-10-06-resources.json).

V9 adds `population_dated(seed, base_year, season, records, loads)` and calendar
boundaries as cache identity. Switching years or model rebuilds the population;
legacy `population` retains v8 behavior. Chronology is now implemented rather than
an outstanding cache prerequisite; see the linked v9 measurements above.

V10 `population_deep` also replays detailed fixture scorelines. Sorted load insertions
and additions to an existing league-round credit record are checked as exact
extensions, retaining caches; rewrites/removals rebuild. Identical journal reads
return directly without rebuilding index maps. The dated observation cache retains
prefix state only before both original and rescheduled fixture periods.
