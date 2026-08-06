# TASK PLAYTEST ROUND 3 — Fix findings from the full-career playtest + career-sim audit

Prereq: none — bug-fix task on the current `goat-tui`/`goat-core` build, no new mechanics,
no new numbers beyond what's already defined in `tuning.rs`. Read CLAUDE.md first. Source:
`docs/PLAYTEST-FULLCAREER-2026-07-22.md` (62-season single-career run, age 16→77) plus a
same-day interactive audit of `career-sim` (see Slice 4).

## Ground rule for this task

Every item below was independently verified by reading the relevant code and/or
reproducing it against a real build before being included. Do not redesign the retirement
or lifestyle systems — the constants and mechanics already encode the right intent
(`RETIRE_AGE_HARD = 40`, `DECLINE_LIFESTYLE_PRO/BALANCED/FLASHY`); the bugs are that the
TUI/harness don't correctly wire to or report on what's already defined.

## Slices

### Slice 1 (CRITICAL) — debug-build crash in awards hashing, ~season 2+

`cargo build`/`cargo run` (non-release) panics on a `u64` overflow inside the
end-of-season awards hash in `awards.rs:51`. Only reproduces in debug (overflow checks
on); `--release` silently wraps and survives. No prior playtest round hit this because
none had played past season 1 before. Fix the overflow (wrapping/checked arithmetic as
appropriate, not just silencing it) so debug builds are usable for a full career — debug
is what any interactive dev/playtest session runs by default.

- Verified: reproduced directly, full-career report §"Bug 1" has the panic trace.
- TDD anchor: a `goat-core`/`goat-tui` test that runs season-end award hashing across
  enough seasons/seeds in a debug-equivalent (overflow-checked) build to have caught this
  before it shipped.

### Slice 2 (HIGH) — retirement is unreachable in a normal high-performing career

`RETIRE_AGE_HARD = 40` (`crates/goat-core/src/tuning.rs`) is dead code as far as the TUI
is concerned. The only retirement path reachable through the TUI's main loop only offers
`[R] Retire now` when **both** age ≥ 35 **and** form < 40 — and form rarely drops that low
for a well-managed player. In the full-career run, that gate didn't fire until age 77,
season 62. `should_retire()` in `goat-core/src/state.rs` already exists and is exercised
by `spec_phase10_retire.rs`/`spec_phase10_longhorizon.rs`, but nothing in the TUI's main
loop calls it or enforces the hard cap. Wire the existing hard-age enforcement into the
TUI's week loop so a career cannot continue past `RETIRE_AGE_HARD` regardless of form.

- Verified: full-career report §"Bug 2", quotes the exact prompt logic and the age-77
  first-fire.
- TDD anchor: scripted-stdin smoke test (extend `crates/goat-tui/tests/smoke_stdin.rs`)
  asserting retirement is forced by/before `RETIRE_AGE_HARD`, not just offered.

### Slice 3 (HIGH) — viewing Legacy at the end-of-season menu re-runs the season-end pipeline

Selecting the Legacy screen from the end-of-season menu re-triggers the full season-end
pipeline (banking, offers, contract rolls) as a side effect of a **read-only view action**,
double-crediting salary/career stats and re-rolling transfer/contract offers each time the
screen is opened. The season-end pipeline must be idempotent per season boundary (run
once, on the actual transition intent) — viewing Legacy must never re-invoke it.

- Verified: full-career report §"Bug 3", reproduced by opening Legacy twice at the same
  season boundary and diffing career totals.
- TDD anchor: golden/invariant test asserting `Intent::ApplySeasonEndLegacy` (or
  equivalent) totals are unchanged by repeated `ViewLegacy`-style read intents at the same
  season boundary.

### Slice 4 (MEDIUM) — `career-sim --lifestyle` verdict label doesn't reflect the simulated outcome

`crates/goat-tui/src/career_sim.rs:1394` prints the "Lifestyle:" line in the CAREER
VERDICT box straight from the CLI's `--lifestyle` arg (the initial seed value), not from
the actual `state.pc_lifestyle` tier after simulation. Since lifestyle is an emergent,
weekly-nudged readout (`apply_lifestyle_weekly_nudges` in `goat-core/src/state.rs`,
driven by training intensity/dev-investment, threshold `LIFESTYLE_TIER_THRESHOLD = 0.333`),
the tier can and does drift away from the CLI seed within the first few seasons — but the
verdict box keeps showing the stale starting label for the whole 20-season run, actively
misleading anyone using this harness to reason about lifestyle behavior. Print the actual
final `state.pc_lifestyle` tier instead (and consider also printing the tier at a couple
of season checkpoints, since it's not static).

- Verified: ran `career-sim` with all 3 `--lifestyle` values at fixed `--intensity high`
  (2026-07-22) — STAT DEVELOPMENT and CAREER VERDICT numbers were byte-identical across
  all three runs except injured-weeks, because intensity's weekly nudge pulls the real
  tier to the same value regardless of the CLI seed; the mismatched labels made this look
  like 3 different lifestyles when it was 1.
- Not urgent for the shipped game (TUI doesn't have this bug — lifestyle there really is
  emergent-only, no misleading label); this is a dev-tooling accuracy fix, sequence last.
- Follow-up (not this task, flag for Design): once Slice 4 is fixed, a real controlled
  lifestyle-vs-decline comparison (isolate decline_mult from intensity's own growth-rate
  effect) is still an open verification gap — full-career only ever observed one drifting
  tier.

## Out of scope (per standing project rule)

TUI rendering/input-handling cosmetics (box overflow, text truncation, invalid-key
handling, stdin EOF) are out of scope — see prior round's scope note. None of the 4 slices
above are TUI cosmetics; all are gameplay/data-correctness or dev-tooling-accuracy bugs.
