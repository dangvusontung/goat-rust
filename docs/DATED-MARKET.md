# Dated rosters and market — SIM15

Core milestone 5 connects roster churn to the dated competitions clock. Fresh
`EnableDatedCompetitions` careers enable this model. The TUI remains a thin test
adapter (`cargo run -p goat-tui -- --dated-competitions`). No dependencies were added.

## Calendar policy

| Date | Operation |
| --- | --- |
| July 1 | Renew expiring NPC contracts; admit the preceding academy cohort; open summer budget; weakest-position auction |
| August 31 | Summer gem auction; academy investment; close registrations |
| January 1 | Open winter budget; weakest-position auction |
| January 31 | Winter gem auction; academy investment; close registrations |
| June 30 | Close actual league tables, coefficients, manager review and promotion/relegation |

Windows include their closing date. This generic federation policy uses Gregorian
calendar dates, including leap years; competition dates remain August 15–June 30.
A fixture on an event date uses the updated squad. Sparse progression executes
all intervening market events in order; repeated reads do not execute them again.

The existing two auction lanes now use shared-model ability/age at the event week,
rather than both using the annual boundary. Transfer applications have stable player-index
order, conserve fee money, cannot spend beyond the buyer's current budget and
cannot drain a seller below eleven active players. Keeper identities are protected
from these outfield auction lanes until a keeper-specific recruitment policy exists.

## Identity and historical registration

`Population::club_at(player, day)` answers historical registration, including
`None` before an academy player's entry. `club_history` contains sparse changes;
the original club and contract expiry are columnar arrays. Player IDs, genomes,
career accumulators, medical observations and training records never move or reset.

Match rosters and suspension service use the club registered on each fixture date.
Actual detailed appearances/goals remain canonical match credits. Completed light
games assign a cheap deterministic participation/scoring projection to the squad
registered on that date. They do not manufacture an observed minute journal.
League tables consume actual calendar results, with no extra annual league roll or
synthetic appearance remainder applied on top of detailed matches. League title
credits use the registration at June 30.

Club facilities change future training exposure at the next full week, because the
existing development/medical model still ticks weekly. This can delay the training
condition change by up to six days; registration itself changes on the actual day.
Dated-market league workload IDs are world-unique: changing leagues cannot merge
two separate games that share a round number. A derived club fixture index is joined
to registration intervals for planned load and refreshes between market events during
cold replay. Speculative future medical views are invalidated after a move or a
reschedule. An exact date binding avoids rebuilding the index on unchanged reads.
Suspension checks first select only relevant competitions and registered clubs,
then resolve historical affiliation; this optimization preserves the same outcomes.

## Contracts and PC acceptance

NPC contracts have seeded one-to-four-year terms, ending at July 1 (June 30 is
covered). Signing a transfer starts a new term. `contract_end_day` exposes expiry.
The initial NPC policy automatically renews an expiring active player's contract.
This is a closed-roster baseline, **not** a Bosman/free-agent negotiation model.

PC contract acceptance stores an absolute expiry in competition progress. PC
transfers outside the windows are rejected. Headless adapters should use
`SimulationSession::transfer_pc_on_date` to validate the club, derive the live league,
facilities and staff, and refresh the PC league plus five ranked deep leagues.
Existing match outcomes remain untouched. PC registration history also survives
save/load and supplies suspension service. Annual wage settlement occurs once
when the dated season closes; contract years remaining are derived from expiry at
July 1. Offer generation and negotiation retain existing meta policies.

## Persistence and compatibility

Save layout **30**, simulation **15**, derived checkpoint format **3** / model **15**.
Registration/economy/intake are reconstructed from seed, dated clock and canonical
match journals; the optional local checkpoint retains the exact derived cache.
PC registration/contract decisions are path-dependent and remain in the canonical
competition block.

Layout29/SIM14 dated saves migrate with `market_enabled = false`: their annual
market/intake behavior remains intact. SIM13 legacy saves without dated competitions
remain accepted. Older checkpoint formats are discarded and rebuilt once. A save
cannot claim SIM14 while containing the new dated-market policy.

## Remaining approximations

- Four auction batches stand in for continuous negotiations inside each window.
- All nations share these window dates; loans, registration quotas, work permits,
  release clauses, player vetoes and contract wage bargaining remain absent.
- NPC expiry automatically renews; free agents and Bosman moves need a separate policy.
- Keeper recruitment is protected rather than simulated in outfield auctions.
- Light participation/goals are projections; light cup loads/cards and daily training
  remain outside this model. Permanent deep scope still follows six-league policy.
- No expansion to 150 countries or 200k live NPCs; those remain separate tasks.
- Mobile/ARM64 capacity validation remains milestone 6.

## Validation

The seeded 42/base2023 headless run completes **three seasons** and 16 weeks in
separate runs. Retained and cold reconstruction agree for every player's identity,
career totals, form, complete registration history, contract expiry and exposure
history. Selected medical/attribute probes also agree. Saved checkpoints restore
with zero rebuilds and byte-identical cache continuation; June/July continuation
matches on both sessions. Fixtures retain unique IDs and minimum rest gaps.

Raw evidence: [three-season trace](experiments/DATED-MARKET-V15-2026-10-06-native-3.txt)
and [weekly trace](experiments/DATED-MARKET-V15-2026-10-06-native-weeks.txt).

| Measurement | 16-week run | After season 3 |
| --- | ---: | ---: |
| Save bytes | 14,093,577 | 48,580,425 |
| Optional checkpoint bytes | 11,993,293 | 37,375,137 |
| Checkpoint restore | 0.161 s | 0.615 s |
| Cold replay | 0.391 s | 3.293 s |
| Diagnostic peak RSS | 110,912 KiB | 422,476 KiB |

Season progression: 16.350 s, 27.332 s and 48.839 s. Weekly windows 9–16:
median 264 ms, maximum 508 ms; one window had no fixtures. The three-season run
has 76,723 resolved fixtures, 9,046 detailed fixtures and 484,285 observed loads.
After season 3, 12 national fixtures remain pending for July as intended.

These are x86_64 Linux / Rust1.90 diagnostic runs on the cloud instance, while
workspace validation also ran. They are not isolated throughput measurements or
mobile/ARM64 certification. Peak memory includes simultaneous retained/resumed/cold
populations and save buffers, not a single gameplay session. No direct speed ratio
against SIM14 is claimed: NPC registrations, workload identities and outputs differ.

`./scripts/test.sh` passes: **644 tests passed, 0 failed, 1 intentionally ignored**;
fmt, Clippy with warnings denied, career invariants and the 50-seed scanner pass.
[Final gate output](experiments/DATED-MARKET-V15-2026-10-06-gate.txt).
Targeted coverage includes exact leap-year window dates, split progression, historical
medical continuity, July entry, PC window/contract/deep scope, distinct transferred
loads, speculative-view invalidation and layout29 migration. Frozen legacy goldens
retain their original expected values.
