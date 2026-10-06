# Task — One headless simulation path for matches and careers

Status: proposed next coding milestone, 2026-10-05.
Direction: core realism first; TUI is a test adapter. Read
[SIMULATION-AUDIT.md](../docs/SIMULATION-AUDIT.md), the Bible and relevant
calendar/match specs before changing behavior. Existing checkout is isolated;
do not create a worktree unless requested.

## Problem

Real squad setup, manager relations, participation accounting, awards,
transfer offers and competition dispatch are partly in `goat-tui/main.rs`.
Other adapters/harnesses run different paths. `career-batch` uses synthetic
output and orbit reconstruction omits the full replay economy pipeline.
Core completion therefore requires shared orchestration, not TUI polish.

## First slice: behavior-preserving extraction

1. Inventory calls and state ownership. Choose a headless application crate
   depending on component libraries if necessary to avoid circular dependencies.
2. Define explicit decisions, events, RNG streams and state inputs for one
   league match, including roster setup, manager selection and result banking.
3. Extract that orchestration without changing constants, draw order or formulas.
   Keep presentation and human input in the TUI; core returns structured events.
4. Make a headless season runner use the same boundary. Collect minutes,
   bench/start rate, manager trust, team results, PC/NPC contributions and
   resource usage as measurement outputs, not mutable simulation inputs.
5. Route the TUI through the boundary and demonstrate parity for representative
   seeds. Migrate other adapters incrementally with an explicit coverage matrix.

## Following slices, kept distinct

- Shared participation/statistics ledger and full-time red-card continuation.
- Actual-record award/rival candidates rather than seeded substitute competitors.
- Unified calendar/season/market/replay pipeline and save continuity.
- Matched deep/background and development realism experiments from the audit.

Do not fix every formula while extracting: first establish trustworthy causal
measurements and preserve golden behavior. Behavior changes get their own
tests, explicit rationale and compatibility review.

## Acceptance criteria for the first slice

- Headless core runs a complete league season using actual match setup, without
  executing the TUI binary or synthesizing `form ± 10` performance.
- TUI and headless runner produce identical state/events for equivalent seed
  and decisions; compare accounting as well as scorelines.
- Training/rest, injury/DNP, bench/substitution, cards and season-end banking
  are represented in scenarios; no zero-test or skipped-path readiness claim.
- No new simulation logic remains in extracted renderer sections.
- Save/load and deterministic replay preserve individual identities and records.
- Frozen goldens remain unchanged; workspace tests, formatting and Clippy pass.
- Document remaining adapter differences. No TUI visual enhancement is required.
