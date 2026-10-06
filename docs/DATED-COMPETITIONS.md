# Milestone 4 — dated competition design (SIM14)

The owner approved implementation after the local resume checkpoint. Core first;
150 countries, larger permanent deep sets and renderer enhancements stay deferred.
Legacy SIM13 paths and frozen outputs remain available; the new model is explicit.

One Gregorian career clock drives leagues, domestic knockout cups, continental
club groups/two-legged knockouts (single final), national qualifying/finals and
preseason friendlies. Club competitions remain August 15–June 30. National finals
run in July and retain their previous tournament-season identity across July 1.
Qualification reads the previous season's authoritative tables, not future tables.
The existing twenty-country format/slot allocation is retained; this is not a claim
of reproducing every real federation or confederation's bespoke format.

Fixture identities include competition, tournament season, region, stage, round,
leg and slot, independently of rescheduling. Original dates are retained. The
existing importance ladder resolves conflicts; national windows are protected.
A team must have at least three days between kickoffs; a two-leg return remains
after its first leg. Legal overflow fails explicitly instead of silently dropping
a match. New fixtures never move already resolved matches. Domestic draws remain
seeded and continental qualifiers are disjoint across tiers.

Results and current tournament/bracket/group progress are canonical. Reading a
schedule or loading it never resolves matches or rerolls results. Already resolved light results are never rerolled when a club enters deep scope.
New SIM14 light fixtures use a fixture-keyed seeded stream and approximate club
strength; the opt-in model does not promise SIM13 future scores. Selected leagues retain the
same deep budget; cup opponents are temporarily detailed at contact. Match minutes,
DNPs, cards and goal ownership follow actual dates. Extra competition credits do
not subtract from league batch appearances. Retained and reconstructed form must
apply appearances in chronological order.

Competition-scoped discipline uses actual played fixture dates, carries outstanding
red bans into the next edition and resets yellow counts at the configured season
or knockout reset. A league ban cannot be served by a cup/friendly/national game.
The named goalkeeper, substitution and card layer is opt-in for the PC beat engine;
PC dismissal must still leave the match running to 90 minutes. PC output and the
team result remain separate, sharing the existing goal and health kernels.

Layout/model compatibility and measurements are recorded below. Save/load checks
include unfinished groups/brackets and a national tournament continuing through
June/July, plus the next PC fixture after load.

## API and compatibility

Use `Intent::EnableDatedCompetitions` for a fresh dated career. Existing played
SIM13 careers cannot be silently converted/rerolled. `SimulationSession` owns
preparation, advancement, fixture resolution and the June/July transition.
`advance_until_pc_fixture` pauses before the next PC fixture; `match_roster` exposes
only the current two squads; `goat-match::sim::start_match_dated` runs the named
squad adapter. `dated_match_receipt` returns score/goal ownership/minutes/cards/PC
output and XP without new draws. `apply_dated_pc_match` validates and commits the
receipt once. An NPC-only harness can use `advance_competitions` through a date.

The test renderer opts in on new careers with `cargo run -p goat-tui --bin goat-tui
-- --dated-competitions`. Play/auto, save, quit and next season exercise these APIs;
normal legacy testing remains available. Training/UI presentation is unchanged;
the dated test adapter advances PC rest between fixtures, while headless callers
can still choose existing training intents. NPC development remains in the shared
weekly health/development replay.

Layout 29 stores optional canonical competition progress (`COMP`). The local
checkpoint is format 2 / model 14 and binds the entire encoded calendar as well as
canonical journals; corruption or stale binding falls back to reconstruction.
SIM13 files without competition progress are explicitly accepted. Their format-1
checkpoint is discarded and rebuilt once, preserving legacy simulation behavior.
SIM12 and incompatible simulation versions are rejected. A SIM13 trailer cannot
claim a SIM14 competition calendar. Save continuation is exact between fixtures;
mid-beat interactive match snapshots are not introduced by this milestone.

## Remaining approximations

- Federation formats use the existing twenty-country model and stature-based
  qualification slots, with deterministic group/draw rules. Group ties use points,
  goal difference, goals scored and stable ID. Knockout ties use a seeded shootout;
  extra-time play and federation-specific tie-break rules are not modeled.
- Light fixtures use approximate club strength. Permanent detail is the PC league
  and five highest-ranked leagues; national squads and contacted cup opponents
  are detailed temporarily. This does not increase the live population to 200k.
- Transfers, contracts and intake still use the annual machinery. Historically
  serving bans after a club transfer needs affiliation intervals, covered by step 5.
- PC match injuries retain the existing weekly model; the new adapter adds no
  independent match injury/development rolls. NPC per-fixture loads feed the shared
  exposure model. PC cards preserve the beat engine's existing discipline events.
- The headless annual transition accrues personal season evidence and authoritative
  league finish/promotion. It does not add a new award/reputation/contract policy;
  the legacy presentation pipeline remains separate.
- All-country international blackout dates and a three-day kickoff gap are explicit
  scheduling simplifications. Rest is continuous across the summer boundary.

## Verification and native measurements

`./scripts/test.sh` checks format, Clippy with denied warnings, workspace tests,
career invariants and the 50-seed scanner. New coverage includes immutable fixture
identity/rest gaps/resolved-leg timing, qualifier/finals isolation, invalid
calendar members, cancellation of a postponement, contact without rerolling light
results, unavailable keeper fallback, legal substitution limits, PC red-card
continuation, idempotent PC receipts, next PC fixture after save/load and explicit
SIM13 compatibility. Frozen legacy goldens are unchanged.

The seed-42/base-2023 dated harness plays every fixture, checks unique IDs and
three-day rest across all competitions, saves unfinished tournaments, compares
byte-exact derived checkpoints and a freshly reconstructed population/ranking,
then continues July tournaments from both sessions. It additionally compares
attributes and medical state for genesis and the newest intake. The six-season
trace crosses the five-year coefficient window and two World Cup cycles.

The permanent scope remains six leagues. About 25,571–25,581 fixtures resolve per
season (roughly 3,000 detailed); the others retain light team results. Individual
minutes/cards are observed for detailed fixtures. Contact does not retroactively
backfill individual cup histories for old light fixtures. National selection is
at contact; replacement callups and tournament roster locks remain dated-roster
work.

These are native x86_64 Linux / Rust 1.90.0 single-process samples, not ARM64
measurements. The harness keeps warm/resumed/cold sessions, save buffers and
comparison snapshots together: its reported peak is a diagnostic footprint, not
the resident memory of one gameplay session. Three-season validation covers about
76,723 matches versus the older league-focused SIM13 scenario; whole-run time is
not a controlled speed ratio. SIM13 reference: 3-season whole validation 14.013 s,
checkpoint save 22,373,478 B, fresh checkpoint median load 0.167644 s.

Raw evidence:

- [Full quality gate](experiments/DATED-COMPETITIONS-V14-2026-10-06-gate.txt).
- [First 16 weeks, including unfinished groups](experiments/DATED-COMPETITIONS-V14-2026-10-06-native-weeks.txt).
- [Three seasons and summer continuation](experiments/DATED-COMPETITIONS-V14-2026-10-06-native-3.txt).
- [Six seasons, rolling-window and repeated international editions](experiments/DATED-COMPETITIONS-V14-2026-10-06-native-6.txt).

Reproduce after `cargo build --release -p goat-save --example bench_calendar` with
`python3 scripts/measure-native.py OUTPUT target/release/examples/bench_calendar
--years 3` or `--weeks 16`. Six-season runs use `--years 6`. Read the per-week values
separately from the cumulative whole-season timings; historical journal work grows
with career length and remains a profiling target in milestone 6.

| Native sample | Measured value |
| --- | ---: |
| First 16 weeks: progression / total validation | 3.039 s / 3.384 s |
| Weeks 9–16: median / maximum progression | 328.019 ms / 587.764 ms |
| Season progression, years 1 / 2 / 3 | 16.360 / 26.934 / 46.729 s |
| Three-season whole validation | 100.201 s |
| Three-season save / checkpoint | 50,798,436 / 39,705,341 B |
| Three-season checkpoint resume / cold comparison | 0.749 / 2.547 s |
| Three-season diagnostic peak | 430.805 MiB |
| Six-season save / checkpoint | 97,400,923 / 74,930,237 B |
| Six-season checkpoint resume / cold comparison | 1.565 / 17.841 s |
| Six-season diagnostic peak | 855.641 MiB |

All samples exited 0 with exact checkpoint, retained/cold and summer continuation
assertions. The six-season trace precedes the final PC ability-selection refinement
and direct dated-coefficient ingestion; neither changes the autonomous fixture
stream or canonical scores. The final three-season trace matches its fixture,
minute/card and save-byte totals through year 3. Native mobile capacity and a full
long-career memory budget remain milestone 6.

The full gate passed 634 tests, 0 failures and 1 intentional ignored test. A final
[clock regression and PC continuation check](experiments/DATED-COMPETITIONS-V14-2026-10-06-final-targeted.txt)
verifies rejection of unresolved past fixtures and legal forward progression;
Clippy and formatting were checked again. A caller that chooses PC training through
existing intents must process pending world events in order, rather than advancing
the PC past unresolved fixtures and asking the world to rewind.
