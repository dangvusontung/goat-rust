# League coefficients — implementation specification, 2026-10-06

Later update: [v12 medical/save changes](MEDICAL-JOURNAL.md) uses layout26/SIM12.
The compatibility and measurements below describe the v11 checkpoint.

The v11 dated deep model ranks associations over the last five completed seasons.
Each continental match awards 2 points for a win, 1 for a draw; penalty-shootout
winners receive advancement credit but the drawn leg stays a draw. Group-entry
bonuses are 4/2/1 for the three competition tiers; each knockout advancement adds 1.
Sum club points and divide by the association's actual entrants across all three
competitions, using integer thousandths. A club enters at most one tier per season.

Sum five annual normalized coefficients. For missing years, use one fifth of the
seeded stature bootstrap per year; after five completed seasons the prior is gone.
Top-division league score is the association sum; lower divisions retain the existing
20,000-per-tier penalty. Ties use stable league ID. First-season bootstrap selection
is unchanged. Freeze the table at preparation and change the selected top five only
at the next season; transferring the PC still changes its mandatory league immediately.
These weights are a documented simulation approximation, not a claim to implement
the latest real UEFA regulations.

Domestic final tables determine distinct continental entrants. Seeded continental
results use the existing batch competition engine; their streams do not consume
match, training or market RNG. Only completed seasons contribute. Ranking history
is small, derived in replay and not serialized. Old compatibility factories remain
available; changed deep behavior gets a new simulation version.

Current continental qualification slot counts remain stature-based; dynamic slot
allocation and authoritative replay of interactive PC continental results are separate
calendar/persistence work. This milestone must not claim those are implemented.

Compatibility: save layout remains 25, simulation version is 11. SIM10 saves are
rejected rather than silently replayed under changed season selection. Explicit
legacy factories and their frozen expectations remain unchanged.

Continental approximation: the batch engine uses generated club strength, groups
play three games per club, and all generated nations share this continental pool.
The weakest six nations currently have no continental slots and receive zero annual
points. These limitations matter when calibrating coefficients; this is not yet a
live roster-based, geographically partitioned continental calendar. A knockout bye
adds no match or advancement points because no tie was played.

Validation: [full workspace gate](experiments/LEAGUE-COEFFICIENTS-V11-2026-10-06-gate.txt).
Regression coverage checks 144 distinct entrants, drawn two-leg shootouts, entry
normalization, consecutive annual recording, five-year expiry, bootstrap removal,
result-based ranking with stable ties and a mandatory lower-division PC league,
and deterministic ranked replay alongside unchanged compatibility factories.


Run report: 597 tests passed, 0 failed, 1 ignored; formatter, Clippy, career
invariants and seed scanner passed. Three release x86_64 Linux runs of the existing
six-league diagnostic yielded median 28.118 ms per round, 1.076421 s for 2,280 NPC
fixtures, and 0.774997 s at the full annual boundary (including coefficients,
markets, promotion and intake). Native child-process peak RSS across these runs
was 33,320 KiB; the resource counter is cumulative across child processes. Fixture
and minute-row counts stayed at 2,280 and 113,088; next-year population was 29,235.
The v10 reference annual boundary was 0.773192 s; this sample does not establish
an isolated coefficient cost or a statistically significant slowdown. The diagnostic
excludes the PC beat engine and I/O, and does not measure mobile throughput.
[Raw native runs](experiments/LEAGUE-COEFFICIENTS-V11-2026-10-06-native.txt).
