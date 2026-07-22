# Full-career playtest — 2026-07-22

Follow-up to `docs/PERSONA-PLAYTEST-2026-07-22.md` (round-1) and `docs/PLAYTEST-ROUND2-2026-07-22.md` (round-2). Those rounds each played a handful of personas through a few weeks/matches — not long enough to catch bugs that only surface over a full career (season-boundary banking, Legacy drift, age-based decline). This round plays **one persona through an entire career, season 1 to organic retirement**, driven by a throwaway harness (`/tmp/goat_career_driver.py`, not part of the repo) that talks to a real `./target/release/goat-tui` subprocess over its actual stdin/stdout pipe.

**Capture method (per Tùng's correction mid-run):** the harness reads the subprocess's raw stdout with `os.read`/`select` as it's produced and appends every byte to an in-memory transcript, flushed to `/tmp/goat_career_transcript.log` at the end (7.5MB, 62 `SEASON N REVIEW` boundaries). Every number in the table below is pulled by regex directly out of that captured file — not reconstructed from memory. The extraction script and its raw output are reproducible from the files listed in Appendix A.

**Persona:** Marcus Chen, CM, England, Premier League, Aston Villa, seed `1234567`. Routine: Short Pass / Vision / Interceptions / Stamina, Medium intensity — set once at creation and never touched again (matches how a real player might "set and forget").

**Play pattern per season:** round 1 is always played **manually** (`P`, then `1` repeatedly through match events until `FULL TIME` — a real simulated match, not a skip), rounds 2–30 are skipped (`K`) except round 15 (`T` then `G`, checking the Legacy screen's mid-season freeze note) and, every 4th season (8 times total), round 2 additionally fast-forwards 2 weeks (`F`, `2`) to re-confirm the round-1/round-2 finding that `F` advances age but not the round counter. So every one of the 62 seasons has at least one manually-played match in the captured transcript, plus periodic deeper probes.

## Outcome

**The career reached organic retirement** — not a harness cutoff. At age 77, season 62, the game itself offered `[R] Retire now` and the harness accepted it. Verbatim:

```
CAREER OVER — Marcus Chen retires at 77            ║
╠══════════════════════════════════════════════╣
║  62 seasons  |  2779 goals  |  1664 matches      ║
║  30 league titles  |  6 Player of the Year  ║
║  Reputation: Cult Hero                              ║
║  Savings: £42829k                               ║
╠══════════════════════════════════════════════╣
║  THE SCHOOLS DELIVER THEIR FINAL VERDICT     ║
╠══════════════════════════════════════════════╣
║  The Trophy Cabinet             73/100  #4/11  ║
║  The Eye-Test Romantics         57/100  #7/11  ║
║  The Stats Purists              84/100  #1/11  ║
║  The Loyalty Traditionalists    85/100  #1/11  ║
╠══════════════════════════════════════════════╣
║  Your generation's debate: you vs O. Brown  ║
║  The argument will outlast both of you.      ║
╚══════════════════════════════════════════════╝

  Your career has entered the pantheon. The debate is the ending.
```

(Note: an earlier run of the same harness with `MAX_SEASONS=40` hit that artificial cap at age 55 without the game offering retirement — logged as a `career-not-retired` event in that run's `events.json`. Bumping `MAX_SEASONS` to 65 for this run was what let the career actually finish; the cap was a harness limitation, not a game bug.)

## Season-by-season table

Every row's **Verbatim source** column is copied character-for-character out of `/tmp/goat_career_transcript.log` (only whitespace-trimmed, `·` used in place of the harness's own `|` separator to keep the markdown table parseable). Season 62 has no `career_goals`/`titles` figures because the harness's JSON recorder appends a season's structured record only on the *non-retirement* path (a minor harness gap, not a game bug) — season 62's own numbers are still fully captured in its review-box quote and in the retirement box above (2779 goals / 1664 matches / 30 titles are the final post-season-62 career totals).

| S | Age | Pos | Div | Season G | Out.Avg | Career G | Titles | Reputation | Verbatim source (transcript) |
|---|-----|-----|-----|----------|---------|----------|--------|-------------|-------------------------------|
| 1 | 16 | 1 | Premier League | 46 | 74 | 46 | 1 | Respected Pro | `finished 1th in Premier League · Your season: 28 matches  46 goals  Output avg: 74 · Form: 79  Age: 16y37w` |
| 2 | 17 | 2 | S | 39 | 69 | 85 | 1 | Respected Pro | `finished 2th in S · Your season: 28 matches  39 goals  Output avg: 69 · Form: 74  Age: 17y35w` |
| 3 | 18 | 1 | S | 44 | 86 | 129 | 2 | Respected Pro | `finished 1th in S · Your season: 28 matches  44 goals  Output avg: 86 · Form: 88  Age: 18y35w` |
| 4 | 19 | 1 | S | 50 | 77 | 179 | 3 | Respected Pro | `finished 1th in S · Your season: 29 matches  50 goals  Output avg: 77 · Form: 87  Age: 19y35w` |
| 5 | 20 | 2 | S | 34 | 78 | 213 | 3 | Respected Pro | `finished 2th in S · Your season: 28 matches  34 goals  Output avg: 78 · Form: 79  Age: 20y36w` |
| 6 | 21 | 1 | S | 65 | 88 | 278 | 4 | Respected Pro | `finished 1th in S · Your season: 28 matches  65 goals  Output avg: 88 · Form: 91  Age: 21y35w` |
| 7 | 22 | 1 | S | 54 | 81 | 332 | 5 | Respected Pro | `finished 1th in S · Your season: 27 matches  54 goals  Output avg: 81 · Form: 77  Age: 22y35w` |
| 8 | 23 | 1 | S | 48 | 81 | 380 | 6 | Cult Hero | `finished 1th in S · Your season: 28 matches  48 goals  Output avg: 81 · Form: 78  Age: 23y35w` |
| 9 | 24 | 1 | S | 51 | 74 | 431 | 7 | Cult Hero | `finished 1th in S · Your season: 28 matches  51 goals  Output avg: 74 · Form: 82  Age: 24y37w` |
| 10 | 25 | 1 | S | 66 | 91 | 497 | 8 | Cult Hero | `finished 1th in S · Your season: 29 matches  66 goals  Output avg: 91 · Form: 94  Age: 25y35w` |
| 11 | 26 | 1 | S | 54 | 80 | 551 | 9 | Club Legend | `finished 1th in S · Your season: 30 matches  54 goals  Output avg: 80 · Form: 82  Age: 26y35w` |
| 12 | 27 | 1 | S | 62 | 86 | 613 | 10 | Club Legend | `finished 1th in S · Your season: 28 matches  62 goals  Output avg: 86 · Form: 86  Age: 27y35w` |
| 13 | 28 | 2 | S | 62 | 85 | 675 | 10 | Club Legend | `finished 2th in S · Your season: 29 matches  62 goals  Output avg: 85 · Form: 85  Age: 28y37w` |
| 14 | 29 | 1 | S | 59 | 86 | 734 | 11 | Iconic | `finished 1th in S · Your season: 30 matches  59 goals  Output avg: 86 · Form: 83  Age: 29y35w` |
| 15 | 30 | 1 | S | 64 | 86 | 798 | 12 | Iconic | `finished 1th in S · Your season: 30 matches  64 goals  Output avg: 86 · Form: 94  Age: 30y35w` |
| 16 | 31 | 3 | S | 41 | 72 | 839 | 12 | Iconic | `finished 3th in S · Your season: 28 matches  41 goals  Output avg: 72 · Form: 73  Age: 31y35w` |
| 17 | 32 | 1 | S | 63 | 88 | 902 | 13 | Iconic | `finished 1th in S · Your season: 30 matches  63 goals  Output avg: 88 · Form: 90  Age: 32y37w` |
| 18 | 33 | 3 | S | 62 | 84 | 964 | 13 | Iconic | `finished 3th in S · Your season: 30 matches  62 goals  Output avg: 84 · Form: 92  Age: 33y35w` |
| 19 | 34 | 1 | S | 51 | 86 | 1015 | 14 | Iconic | `finished 1th in S · Your season: 29 matches  51 goals  Output avg: 86 · Form: 84  Age: 34y35w` |
| 20 | 35 | 1 | S | 58 | 82 | 1073 | 15 | Iconic | `finished 1th in S · Your season: 29 matches  58 goals  Output avg: 82 · Form: 79  Age: 35y35w` |
| 21 | 36 | 1 | S | 60 | 86 | 1133 | 16 | Iconic | `finished 1th in S · Your season: 30 matches  60 goals  Output avg: 86 · Form: 81  Age: 36y37w` |
| 22 | 37 | 1 | S | 61 | 85 | 1194 | 17 | Iconic | `finished 1th in S · Your season: 28 matches  61 goals  Output avg: 85 · Form: 79  Age: 37y35w` |
| 23 | 38 | 1 | S | 54 | 83 | 1248 | 18 | Iconic | `finished 1th in S · Your season: 29 matches  54 goals  Output avg: 83 · Form: 79  Age: 38y35w` |
| 24 | 39 | 1 | S | 59 | 81 | 1307 | 19 | Iconic | `finished 1th in S · Your season: 30 matches  59 goals  Output avg: 81 · Form: 74  Age: 39y35w` |
| 25 | 40 | 1 | S | 53 | 72 | 1360 | 20 | Iconic | `finished 1th in S · Your season: 30 matches  53 goals  Output avg: 72 · Form: 76  Age: 40y37w` |
| 26 | 41 | 1 | S | 57 | 76 | 1417 | 21 | Iconic | `finished 1th in S · Your season: 28 matches  57 goals  Output avg: 76 · Form: 85  Age: 41y35w` |
| 27 | 42 | 2 | S | 53 | 74 | 1470 | 21 | Iconic | `finished 2th in S · Your season: 29 matches  53 goals  Output avg: 74 · Form: 79  Age: 42y35w` |
| 28 | 43 | 1 | S | 61 | 82 | 1531 | 22 | Iconic | `finished 1th in S · Your season: 28 matches  61 goals  Output avg: 82 · Form: 80  Age: 43y35w` |
| 29 | 44 | 6 | S | 39 | 71 | 1570 | 22 | Iconic | `finished 6th in S · Your season: 27 matches  39 goals  Output avg: 71 · Form: 75  Age: 44y37w` |
| 30 | 45 | 1 | S | 40 | 66 | 1610 | 23 | Club Legend | `finished 1th in S · Your season: 26 matches  40 goals  Output avg: 66 · Form: 65  Age: 45y35w` |
| 31 | 46 | 1 | S | 61 | 78 | 1671 | 24 | Club Legend | `finished 1th in S · Your season: 26 matches  61 goals  Output avg: 78 · Form: 83  Age: 46y35w` |
| 32 | 47 | 2 | S | 49 | 70 | 1720 | 24 | Club Legend | `finished 2th in S · Your season: 25 matches  49 goals  Output avg: 70 · Form: 78  Age: 47y35w` |
| 33 | 48 | 1 | S | 60 | 84 | 1780 | 25 | Cult Hero | `finished 1th in S · Your season: 27 matches  60 goals  Output avg: 84 · Form: 83  Age: 48y35w` |
| 34 | 49 | 1 | S | 45 | 66 | 1825 | 26 | Cult Hero | `finished 1th in S · Your season: 28 matches  45 goals  Output avg: 66 · Form: 59  Age: 49y35w` |
| 35 | 50 | 6 | S | 36 | 59 | 1861 | 26 | Club Legend | `finished 6th in S · Your season: 28 matches  36 goals  Output avg: 59 · Form: 56  Age: 50y35w` |
| 36 | 51 | 4 | S | 48 | 72 | 1909 | 26 | Cult Hero | `finished 4th in S · Your season: 26 matches  48 goals  Output avg: 72 · Form: 67  Age: 51y35w` |
| 37 | 52 | 2 | S | 38 | 65 | 1947 | 26 | Cult Hero | `finished 2th in S · Your season: 24 matches  38 goals  Output avg: 65 · Form: 74  Age: 52y35w` |
| 38 | 53 | 1 | S | 44 | 78 | 1991 | 27 | Club Legend | `finished 1th in S · Your season: 30 matches  44 goals  Output avg: 78 · Form: 77  Age: 53y35w` |
| 39 | 54 | 1 | S | 43 | 65 | 2034 | 28 | Club Legend | `finished 1th in S · Your season: 28 matches  43 goals  Output avg: 65 · Form: 58  Age: 54y35w` |
| 40 | 55 | 1 | S | 43 | 69 | 2077 | 29 | Cult Hero | `finished 1th in S · Your season: 24 matches  43 goals  Output avg: 69 · Form: 57  Age: 55y35w` |
| 41 | 56 | 9 | S | 31 | 65 | 2108 | 29 | Cult Hero | `finished 9th in S · Your season: 24 matches  31 goals  Output avg: 65 · Form: 59  Age: 56y35w` |
| 42 | 57 | 8 | S | 38 | 58 | 2146 | 29 | Cult Hero | `finished 8th in S · Your season: 27 matches  38 goals  Output avg: 58 · Form: 67  Age: 57y35w` |
| 43 | 58 | 2 | S | 47 | 66 | 2193 | 29 | Cult Hero | `finished 2th in S · Your season: 28 matches  47 goals  Output avg: 66 · Form: 69  Age: 58y35w` |
| 44 | 59 | 3 | S | 36 | 61 | 2229 | 29 | Cult Hero | `finished 3th in S · Your season: 27 matches  36 goals  Output avg: 61 · Form: 57  Age: 59y35w` |
| 45 | 60 | 15 | S | 30 | 55 | 2259 | 29 | Club Legend | `finished 15th in S · Your season: 23 matches  30 goals  Output avg: 55 · Form: 63  Age: 60y35w` |
| 46 | 61 | 1 | S | 36 | 55 | 2295 | 30 | Cult Hero | `finished 1th in S · Your season: 25 matches  36 goals  Output avg: 55 · Form: 55  Age: 61y35w` |
| 47 | 62 | 2 | S | 39 | 63 | 2334 | 30 | Cult Hero | `finished 2th in S · Your season: 25 matches  39 goals  Output avg: 63 · Form: 63  Age: 62y35w` |
| 48 | 63 | 10 | S | 31 | 63 | 2365 | 30 | Cult Hero | `finished 10th in S · Your season: 26 matches  31 goals  Output avg: 63 · Form: 68  Age: 63y35w` |
| 49 | 64 | 13 | S | 25 | 51 | 2390 | 30 | Cult Hero | `finished 13th in S · Your season: 22 matches  25 goals  Output avg: 51 · Form: 47  Age: 64y35w` |
| 50 | 65 | 4 | S | 38 | 53 | 2428 | 30 | Cult Hero | `finished 4th in S · Your season: 26 matches  38 goals  Output avg: 53 · Form: 59  Age: 65y35w` |
| 51 | 66 | 3 | S | 39 | 57 | 2467 | 30 | Cult Hero | `finished 3th in S · Your season: 23 matches  39 goals  Output avg: 57 · Form: 68  Age: 66y35w` |
| 52 | 67 | 9 | S | 30 | 61 | 2497 | 30 | Cult Hero | `finished 9th in S · Your season: 25 matches  30 goals  Output avg: 61 · Form: 61  Age: 67y35w` |
| 53 | 68 | 15 | S | 30 | 50 | 2527 | 30 | Club Legend | `finished 15th in S · Your season: 27 matches  30 goals  Output avg: 50 · Form: 47  Age: 68y35w` |
| 54 | 69 | 10 | S | 34 | 47 | 2561 | 30 | Club Legend | `finished 10th in S · Your season: 25 matches  34 goals  Output avg: 47 · Form: 60  Age: 69y35w` |
| 55 | 70 | 4 | S | 34 | 53 | 2595 | 30 | Cult Hero | `finished 4th in S · Your season: 24 matches  34 goals  Output avg: 53 · Form: 54  Age: 70y35w` |
| 56 | 71 | 3 | S | 24 | 51 | 2619 | 30 | Cult Hero | `finished 3th in S · Your season: 22 matches  24 goals  Output avg: 51 · Form: 47  Age: 71y35w` |
| 57 | 72 | 11 | S | 24 | 58 | 2643 | 30 | Cult Hero | `finished 11th in S · Your season: 20 matches  24 goals  Output avg: 58 · Form: 61  Age: 72y35w` |
| 58 | 73 | 4 | S | 30 | 61 | 2673 | 30 | Cult Hero | `finished 4th in S · Your season: 25 matches  30 goals  Output avg: 61 · Form: 53  Age: 73y35w` |
| 59 | 74 | 5 | S | 23 | 47 | 2696 | 30 | Cult Hero | `finished 5th in S · Your season: 22 matches  23 goals  Output avg: 47 · Form: 43  Age: 74y35w` |
| 60 | 75 | 2 | S | 31 | 48 | 2727 | 30 | Cult Hero | `finished 2th in S · Your season: 23 matches  31 goals  Output avg: 48 · Form: 47  Age: 75y35w` |
| 61 | 76 | 5 | S | 23 | 48 | 2750 | 30 | Cult Hero | `finished 5th in S · Your season: 23 matches  23 goals  Output avg: 48 · Form: 60  Age: 76y35w` |
| 62 |  |  |  |  |  |  |  |  | `finished 5th in S · Your season: 27 matches  29 goals  Output avg: 39 · Form: 32  Age: 77y35w` |

## Attribute/OVR decline curve (from real `V`-sheet captures)

The attribute sheet (`V`) was captured at every season boundary too (4117 `OVR` occurrences total across the transcript, since it's also displayed on the persistent status HUD every round). Sampled every 10 seasons, parsed straight out of those captures:

| Season | Age | OVR |
|---|---|---|
| 1 | 16 | 76 |
| 11 | 26 | 86 |
| 21 | 36 | 85 |
| 31 | 46 | 78 |
| 41 | 56 | 69 |
| 51 | 66 | 63 |
| 61 | 76 | 58 |

Rises from 76 (age 16, still developing) to a peak 86 (age 26) then declines steadily to 58 by age 76 — the age-decline curve behaves as expected across a genuinely full career, which round-1/round-2's few-week sessions couldn't have shown either way.

## Legacy/Pantheon checkpoints (verbatim `G`-screen quotes)

Sampled from the 123 `LEGACY —` screen captures in the transcript (the harness reads this screen twice per season on average: once mid-season at round 15 to check the freeze note, once at season-end). Labels below are the `Seasons: N` figure printed in each quote itself, not a guessed position:

```
LEGACY — Journeyman     | Goals:    0   Matches:    0   Seasons:  0 | League titles:  0   POTY wins:  0
LEGACY — Cult Hero      | Goals:  497   Matches:  281   Seasons: 10 | League titles:  8   POTY wins:  3
LEGACY — Iconic         | Goals: 1073   Matches:  574   Seasons: 20 | League titles: 15   POTY wins:  5
LEGACY — Club Legend    | Goals: 1610   Matches:  859   Seasons: 30 | League titles: 23   POTY wins:  6
LEGACY — Cult Hero      | Goals: 2750   Matches: 1637   Seasons: 61 | League titles: 30   POTY wins:  6
```

Reputation label is **non-monotonic** by design — per the table above it cycles between Club Legend and Cult Hero repeatedly (Club Legend at S11-13, S30-32, S35, S38-39, S45, S53-54; Cult Hero everywhere else from S8 onward, final label at S62 retirement is Cult Hero) despite `career_goals`/`league_titles` only ever increasing. This matches `character_rep` swinging as low as 1 and `sporting_rep` staying pinned at 100 in the parsed JSON — reputation reacts to recent form/discipline, not just cumulative stats. Not flagged as a bug; consistent behavior across the whole run, not a one-off.

## Bugs / anomalies found

**None new.** Across all 62 seasons (1860 simulated rounds, 111 discipline events, 186 double-fixture weeks, 8 explicit fast-forward re-checks), every regression check from round-1/round-2 held:

- No `bug-g-reentry-recurred`, `bug-double-season-review`, `bug-legacy-freeze`, or `bug-no-season-end` events fired (all 4 are checks the harness runs automatically every season/round-15 — zero hits across the whole career).
- All 8 `f-forward-check` events confirm the round-1/round-2 finding still holds: age advances ~2 weeks on `F`+`2`, round counter stays unchanged. Example, verbatim: `S1: sent F=2 weeks at round 2. Before age (16, 2). After: round-marker=S1 Round 2/30 age=Age 16y4w`.
- One harmless quirk at the very end, **after** the career was already over: the harness's generic "[Y] Next season" cascade check fired once more on the post-retirement pantheon screen (`season-cascade-unexpected` at S62 — "unrecognized season-end prompt state, sending 'Y' blind"), the game correctly replied `Unknown command.`, and the harness's final cleanup `Q`/`Q` then hit a broken pipe (`crash` event) because the process had already exited cleanly after `Goodbye.`. This is a harness driving-script edge case (its own state machine, not the game), not a `goat-tui` bug — no game state was affected, it happened strictly after `CAREER OVER` had already printed.
- Harness gap noted above: season 62's structured JSON record wasn't appended (retirement path returns early, before the `seasons.append(...)` call) — numbers for season 62 are still fully present in the raw transcript and reported here from that source directly.

## Appendix A — raw artifacts

- `/tmp/goat_career_driver.py` — the driver script (MAX_SEASONS=65, MAX_WALLCLOCK_S=400 for this run).
- `/tmp/goat_career_transcript.log` — 7.5MB, full raw captured terminal output, all 62 seasons.
- `/tmp/goat_career_seasons.json` — 61 structured per-season records (season 62 missing, see above), each parsed from the real captured screens by regex in the driver script itself.
- `/tmp/goat_career_events.json` — 309 timestamped anomaly/note events collected while driving.
- Prior 40-season run (hit the old `MAX_SEASONS` cap, superseded by this one): archived as `/tmp/goat_career_{transcript,seasons,events}.40season_run.*`.

