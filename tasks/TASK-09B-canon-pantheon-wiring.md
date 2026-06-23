# TASK 09B — Wire the seeded canon into the Phase-7 pantheon (deferred follow-up to 9A.4)

Prereq: Phase 9 done (9A.1–9A.6). Read CLAUDE.md + bible §7 (pantheon/schools).
Slice 9A.4 produces a **seeded, derivable** canon of past greats in `goat-world::history`
(`backfill_history` → `History::canon_ranked()`), but `goat-meta`'s pantheon still ranks a
**hardcoded** `const CANON: [PastGreat; 10]` of `&'static str` greats. This task makes the
legacy screen rank the *seed-specific* greats so every universe has its own pantheon.

## Why it was deferred
`PastGreat` uses `&'static str` (compile-time). Consuming a runtime seeded canon needs an
owned-data refactor of `goat-meta`, which would move the **frozen `golden_legacy` values**.
That's a legitimate change for new behavior — but it must be done deliberately, with the
new goldens re-approved, not as a drive-by during Phase 9.

## Architecture
- Refactor `goat_meta::pantheon::PastGreat` to owned data: `name: String`, `era: String`,
  `nationality: String` (or keep `&'static str` for the handcrafted defaults via a `Cow`).
- Change `rank_in_canon` / `all_rankings` to take a **runtime canon slice** (`&[PastGreat]`)
  instead of reading the `const CANON`. Keep the handcrafted `CANON` as the *default* used
  when no seeded canon is supplied (so existing tests/paths still work).
- Add a mapper in `goat-meta` (or `goat-tui`): `HistoricGreat → PastGreat`, deriving
  `LegacyAxes` from the backfilled record:
  - `winning` ← titles/era dominance, `accolades` ← `ballon_dors`, `output` ← `peak_ovr`,
    `longevity` ← `final_year - debut_year`, others ← sensible stubs/derivations.
- `goat-tui` legacy screen passes `backfill_history(world_seed, N).canon_ranked()` (mapped)
  into `all_rankings` so the PC is ranked against the *seeded* greats.

## TDD anchors
- `seeded_canon_maps_to_valid_axes` — every mapped `PastGreat` has axes in range.
- `rank_in_canon_accepts_runtime_canon` — ranking against a supplied canon matches the old
  behavior when given the default `CANON`.
- Golden: PC ranking against a fixed seed's canon is stable (new frozen values).

## ⏸ Pause
The existing `golden_legacy` tests assert rankings against the static `CANON`. If the wiring
changes their inputs, **re-freeze with explicit user approval** (per CLAUDE.md). Prefer
keeping the default-`CANON` path byte-identical so those goldens DON'T move, and add NEW
goldens for the seeded path.

## Definition of done
1. Legacy screen ranks the PC against the seed's backfilled greats (named, seed-specific).
2. Default-canon path unchanged → `golden_legacy` frozen values intact (or re-approved).
3. New seeded-canon goldens frozen.
4. `cargo test --workspace` green; `fmt --check` + `clippy -D warnings` clean on touched crates.
5. No floats in sim, no unsafe, no I/O in core, no logic in TUI.

## Out of scope
History browser UI, multi-era canon depth, the relationship web (Phase 10 / parked).
