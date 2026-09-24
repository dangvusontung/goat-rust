# Round-4 playtest — verifying the round-3 fixes under real play (2026-07-22)

Follow-up to `docs/PLAYTEST-FULLCAREER-2026-07-22.md` (the full-career run that
found the 4 bugs fixed in commits `280ea31..a414fe0`, task spec
`tasks/TASK-PLAYTEST-round1-fixes.md`'s round-3 sibling). All 4 fixes already
have unit/smoke tests passing (`awards_no_overflow.rs`,
`smoke_stdin.rs`'s new retirement/idempotency cases,
`career_sim_verdict.rs`). This round verifies them by **actually playing**
three separate full careers against the real, compiled **debug** build —
debug specifically because that's the build the original overflow crash
(Bug 1) required, and because unit tests use synthetic seed sweeps while
this exercises the same code paths through real interactive input.

career-sim is out of scope for this round (TUI-only real play, per task).

## Method

- Binary: `./target/debug/goat-tui`, built via `cargo build -p goat-tui`
  (plain debug profile — `overflow-checks = true`, no `--release`) against
  `feature/update-v2` at commit `a414fe0` (round-3's last commit).
- Three full careers, played start (character creation) to finish (forced
  retirement), each with a different position / nation / club / seed /
  training routine / intensity:

  | | Persona | Pos | Nation / Division | Club | Seed | Routine | Intensity |
  |---|---|---|---|---|---|---|---|
  | A | Kofi Boateng | ST | England / Premier League | Chelsea | 111111 | Finishing, Shot Power, Composure, Acceleration | High |
  | B | Diego Fontes | CB | Brazil / Série B | Novorizontino | 222222 | Standing Tackle, Marking, Interceptions, Strength | Low |
  | C | Elias Kowalski | CM | England / Championship | Millwall | 333333 | Short Passing, Vision, Ball Control, Stamina | Medium |

- Driven by a throwaway, uncommitted Python/pexpect harness (same spirit as
  the full-career report's "not committed — it's not game code" harness):
  it reads the real pty output reactively and responds to whatever prompt
  actually appears (transfer offers, contract renewals, retirement prompts,
  match beats, the season-end menu), rather than a fixed keystroke script,
  since several of those only appear conditionally. Every run's full
  terminal transcript was captured to a log file as it happened (not
  reconstructed from memory) — quotes below are copy-pasted from those logs.
- Per season (30 rounds, `ROUNDS_PER_SEASON`): every round trained (`W`)
  first; round 1 and round 15 of every season were played **interactively**
  (`P`, real beat-by-beat choices) to sample match beats/output across the
  whole career, not just the start; all other rounds used `K` (auto-skip).
  At round 15 of every season, `T` (table) and `V` (full player sheet, to
  sample the Lifestyle tier) were also checked.
- **At every single season-end menu (`[Y] Next season   [G] Legacy   [Z]
  Save & quit   [Q] Quit`), `G` was pressed twice in a row before `Y`**,
  capturing the Legacy screen's `Goals:/Matches:/Seasons:` line both times —
  this is the direct, repeated stress-test for the round-3 idempotency fix
  (Slice 3), done at all 24 season boundaries per career (72 checks total),
  not just once.
- Transfer offers: always declined (`N`, stay) — kept the run to one club
  per persona so career totals stay simple to audit; not required this
  round (transfer/contract flows are unchanged by round-3). Contract
  renewals: always accepted (`Y`) — deliberately, so a career is never
  forced to retire early via the *soft* age-34-and-out-of-contract path,
  isolating the **hard** `RETIRE_AGE_HARD` (40) enforcement as the only
  possible way any of these three careers could end. The softer
  age-35-and-form-under-40 `[R] Retire now / [C] Continue playing` suggestion
  was set to always decline (`C`) if it ever appeared — it never did, in any
  of the three careers (`grep -c "Retire now"` = 0 for all three transcripts).
- All three careers ran to completion cleanly: 24 completed seasons each,
  age 16 → 40, in ~115-120s of wall-clock play each.

---

## Verification 1 — no crash/panic across the whole career (debug build)

**Confirmed, all 3 careers.** Zero panics, zero "overflow" occurrences, in
any of the three full transcripts (`grep -i "panicked\|overflow"` → no
matches in any log). Each career played 24 seasons and 656-701 matches under
the debug build (`overflow-checks = true`), so `ai_competitor_score`
(`crates/goat-meta/src/awards.rs`) ran the season-end awards hash 23 times
per career (season 2 through season 24) × 2 awards × 8 candidates — the
exact code path and multiplication that overflowed and panicked
deterministically at the end of season 2 pre-fix. All three careers cleared
that exact boundary without incident; verbatim from persona A's transcript
(seasons 2-24 all identical in kind, just different names/numbers):

```
════════════════════════════════════════════╗
║  AWARDS NIGHT — Season 2                     ║
╠══════════════════════════════════════════════╣
║  ★ WON  Player of the Year                   ║
║    Winner: Kofi Boateng                      ║
║    Runner-up: K. Tanaka                      ║
║  ★ WON  Golden Boot                          ║
║    Winner: Kofi Boateng                      ║
║    Runner-up: C. Osei                        ║
╚══
```

— play continued normally afterward every time, all the way through season
24, for all three personas. **This is a stronger test than the unit test's
synthetic 80-season × 6-seed sweep**: it's the actual awards path invoked
from actual interactive TUI input, ~2,036 real matches total across the
three careers, with `overflow-checks` on the whole time. Slice 1 holds.

## Verification 2 — retirement now happens at or before age 40

**Confirmed, all 3 careers — every one retired at exactly age 40, forced,
with no prompt.** None of the three ever saw the softer `[R]/[C]` suggestion
fire at all (age ≥ 35 and form < 40 never coincided for any of them — a
different finding from the pre-fix full-career report, where that gate was
the *only* path and took until age 77). The only retirement path exercised
across all three careers was the unconditional `should_retire()` check
(`age_years >= RETIRE_AGE_HARD`) at the very top of `run_game_loop`'s loop —
confirmed by the transcripts showing **zero** menu, **zero** `[R]/[C]`
prompt, between the last normal input and the `CAREER OVER` screen. Verbatim
from persona A (all three identical in shape):

```
  [Y] Next season   [G] Legacy   [Z] Save & quit   [Q] Quit
  > Y

╔══════════════════════════════════════════════╗
║  CAREER OVER — Kofi Boateng retires at 40            ║
╠══════════════════════════════════════════════╣
║  24 seasons  |  1451 goals  |  701 matches      ║
║  22 league titles  |  14 Player of the Year  ║
║  Reputation: Iconic                              ║
║  Savings: £5920k                               ║
╠══════════════════════════════════════════════╣
```

Pressing `Y` to start season 25 aged the player past 40 mid-`StartSeason`;
the very next loop iteration's `should_retire()` check caught it immediately
and forced `Intent::Retire` — no season-25 gameplay ever happened, no
suggestion prompt, nothing to decline. Same shape for persona B (Diego
Fontes, retires at 40, 24 seasons) and persona C (Elias Kowalski, retires at
40, 24 seasons). This is the exact behaviour Slice 2 was meant to produce:
`RETIRE_AGE_HARD = 40` is no longer dead code — it's a hard, unconditional
ceiling that fired identically for three very different careers (different
position, training focus, and intensity), a **41-season improvement** over
the pre-fix run's age-77 organic retirement.

## Verification 3 — viewing Legacy twice at the same season boundary doesn't change totals

**Confirmed, all 3 careers, all 24 season boundaries each (72/72 checks,
0 mismatches).** At every single season-end menu, `G` was pressed twice in a
row before `Y`; the `Goals:/Matches:/Seasons:` line was captured both times
and compared programmatically. Every single one matched exactly. Sample
(season-2 boundary, all three personas — first press vs. second press,
verbatim numbers):

| Persona | 1st `G` press | 2nd `G` press | Match |
|---|---|---|---|
| A (Kofi Boateng) | Goals: 124  Matches: 58  Seasons: 2 | Goals: 124  Matches: 58  Seasons: 2 | ✅ |
| B (Diego Fontes) | Goals: 108  Matches: 55  Seasons: 2 | Goals: 108  Matches: 55  Seasons: 2 | ✅ |
| C (Elias Kowalski) | Goals: 94  Matches: 53  Seasons: 2 | Goals: 94  Matches: 53  Seasons: 2 | ✅ |

Full verbatim capture of one such screen (persona A, season 2):

```
╔══════════════════════════════════════════════╗
║  LEGACY — Cult Hero                   
╠══════════════════════════════════════════════╣
║  CAREER EVIDENCE                             ║
║  Goals:  124   Matches:   58   Seasons:  2   ║
║  League titles:  2   POTY wins:  1          ║
║  Decisive moments:  0   Clubs:  1           ║
╠══════════════════════════════════════════════╣
```

No double-banking, no re-rolled transfer/contract offers, no unexpected
season-review reprint — pressing `G` twice at the season-end menu is now a
pure read, exactly as Slice 3 intended, confirmed at 72 independent season
boundaries (24 per career × 3 careers), not just the smoke test's single
seeded case.

---

## Persona A — Kofi Boateng (ST, England / Premier League, Chelsea)

- **Start OVR:** 63 (age 16) — `Pac:76 Sho:56 Pas:35 Dri:50 Def:37 Phy:40`
- **Lifestyle:** **Professional** at retirement. Season 1 started Balanced
  (as all careers do); by season 2 it had already drifted to Professional
  and stayed there for the rest of the career (all 24 season-15 samples:
  Professional). Intensity used: **High**, the entire career, unchanged —
  this is the emergent tier the weekly lifestyle nudge produces at fixed
  High intensity, consistent with round-3 Slice 4's description of how the
  nudge behaves (it just wasn't wired into the TUI-side verdict the way
  career-sim's was — this round only observed the TUI's own live tier, which
  isn't affected by Slice 4's fix at all, since that fix was career-sim-only).
- **Training:** High intensity, focus Finishing / Shot Power / Composure /
  Acceleration. OVR climbed from 63 → 86 over seasons 1-13 (age 16 → 28),
  then plateaued at 86 through season 18 (age 33), then declined to 84 by
  season 24 (age 39) — roughly **12-13 seasons of real growth** before
  age-decline caught up with and then outpaced training gains.
- **Peak:** OVR **86**, reached age 28 (season 13) and held through age 33
  (season 18) — `Pac:77-83 Sho:65 Pas:34-35 Dri:54 Def:35-37 Phy:36-40`
  across that plateau.
- **Thành tích / achievements:** 1451 career goals, 701 matches, **22 league
  titles**, **14 Player of the Year** awards, Reputation "**Iconic**". Final
  Pantheon: Trophy Cabinet 73/100 (#4/11), Eye-Test Romantics 58/100
  (#7/11), Stats Purists 88/100 (#1/11), Loyalty Traditionalists 86/100
  (#1/11). No rival ever crystallised ("No rival emerged. You reigned
  alone.").
- **Chỉ số khi giải nghệ / stats at retirement:** final OVR 84 (last sheet,
  age 39) — `Pac:64 Sho:62 Pas:31 Dri:49 Def:31 Phy:28`; retires at age
  **40**; 24 seasons, 1451 goals, 701 matches, 22 titles, 14 POTY; Savings
  £5,920k. **Retirement reason: hard-age cap (`RETIRE_AGE_HARD = 40`)** —
  forced unconditionally, no prompt (see Verification 2).

## Persona B — Diego Fontes (CB, Brazil / Série B, Novorizontino)

- **Start OVR:** 76 (age 16) — `Pac:56 Sho:29 Pas:35 Dri:39 Def:62 Phy:58`
- **Lifestyle:** **Flashy** at retirement. Also drifted away from Balanced
  by season 2 and stayed Flashy for the remaining 23 seasons of samples.
  Intensity used: **Low**, unchanged all career. Notable: Low intensity
  produced the *Flashy* tier here, not the more restrained-sounding tier one
  might guess from "Low effort" — the mechanic is driven by the lifestyle
  nudge formula (intensity + dev-investment), not a direct intensity→tier
  mapping, so this is reported as observed rather than interpreted.
- **Training:** Low intensity, focus Standing Tackle / Marking /
  Interceptions / Strength. Growth was much flatter than the other two
  personas — OVR oscillated in a narrow 76-78 band the *entire* career
  (peak 78 at age 32/season 17, otherwise mostly 76-77), i.e. training
  gains and age-related decline were close to balanced almost immediately;
  there's no multi-season "climb" phase visible the way personas A/C show.
- **Peak:** OVR **78**, age 32 (season 17) — `Pac:49 Sho:29 Pas:34 Dri:36
  Def:70 Phy:55`.
- **Thành tích / achievements:** 1209 career goals, 656 matches, **20
  league titles**, **2 Player of the Year** awards, Reputation "**Cult
  Hero**". Final Pantheon: Trophy Cabinet 73/100 (#4/11), Eye-Test
  Romantics 57/100 (#7/11), Stats Purists 85/100 (#1/11), Loyalty
  Traditionalists 85/100 (#1/11). Rivalry crystallised vs. **F. Oliveira**
  ("Your generation's debate: you vs F. Oliveira").
- **Chỉ số khi giải nghệ / stats at retirement:** final OVR 73 (age 39) —
  `Pac:20 Sho:22 Pas:27 Dri:23 Def:63 Phy:36`; retires at age **40**; 24
  seasons, 1209 goals, 656 matches, 20 titles, 2 POTY; Savings £5,600k.
  **Retirement reason: hard-age cap (`RETIRE_AGE_HARD = 40`)**, same
  unconditional forced transition as persona A.

## Persona C — Elias Kowalski (CM, England / Championship, Millwall)

- **Start OVR:** 75 (age 16) — `Pac:37 Sho:34 Pas:45 Dri:45 Def:41 Phy:53`
- **Lifestyle:** **Balanced** at retirement — never drifted away from the
  starting tier across all 24 season-15 samples. Intensity used:
  **Medium**, unchanged all career — consistent with Medium being the
  "no strong pull either way" middle setting (contrast with A's High →
  Professional and B's Low → Flashy).
- **Training:** Medium intensity, focus Short Passing / Vision / Ball
  Control / Stamina. OVR climbed 75 → 85 over seasons 1-12 (age 16 → 27),
  then oscillated in the low-to-mid 80s (touching 85 again at age 36/season
  21) before declining to 83 by season 24 (age 39) — roughly **11-12
  seasons of real growth** before plateau, very close to persona A's
  timeline despite the lower training intensity.
- **Peak:** OVR **85**, first reached age 27 (season 12), matched again at
  age 36 (season 21) — `Pac:22-37 Sho:32-34 Pas:47-51 Dri:42-46 Def:36-41
  Phy:41-46` across those two peaks (pace had already started its
  age-related fall by the second peak, offset by continued Passing growth
  from the trained routine).
- **Thành tích / achievements:** 1313 career goals, 681 matches, **21
  league titles**, **2 Player of the Year** awards, Reputation "**Iconic**".
  Final Pantheon: Trophy Cabinet 73/100 (#4/11), Eye-Test Romantics 57/100
  (#7/11), Stats Purists 85/100 (#1/11), Loyalty Traditionalists 85/100
  (#1/11). Rivalry crystallised vs. **J. Smith**.
- **Chỉ số khi giải nghệ / stats at retirement:** final OVR 83 (age 39) —
  `Pac:11 Sho:29 Pas:48 Dri:37 Def:32 Phy:38`; retires at age **40**; 24
  seasons, 1313 goals, 681 matches, 21 titles, 2 POTY; Savings £5,330k.
  **Retirement reason: hard-age cap (`RETIRE_AGE_HARD = 40`)**, same
  unconditional forced transition as personas A and B.

---

## Summary

All three round-3 TUI fixes hold under real, extended interactive play —
not just their unit/smoke tests:

1. **No crash across any of the 3 careers** (24 seasons / 656-701 matches
   each, ~2,036 matches total, debug build with `overflow-checks = true`
   throughout). Every career cleared the exact season-2 awards boundary
   that panicked 100% of the time pre-fix, and every season-end boundary
   after it through season 24. Slice 1 confirmed.
2. **Retirement now happens at exactly age 40 for all 3 careers**, forced
   unconditionally with no prompt — a 41-season improvement over the pre-fix
   full-career run's age-77 organic retirement, and the softer `[R]/[C]`
   suggestion never even needed to fire in any of the three runs. Slice 2
   confirmed.
3. **Legacy is idempotent at every season boundary, 72/72 checks across the
   3 careers** (24 per career), each checked by pressing `G` twice in a row
   and diffing the exact `Goals:/Matches:/Seasons:` numbers. Slice 3
   confirmed.
4. career-sim (Slice 4) was out of scope this round per the task — not
   exercised.

**New bugs found: none** that are round-3 regressions or otherwise
actionable gameplay/logic/data bugs. Two things worth a passing note, both
pre-existing/unrelated to round-3 and not being filed as bugs:

- Career goal totals scale very high regardless of position (persona B, a
  CB, finished with 1209 "career goals") — matches the shape already seen
  in `docs/PLAYTEST-FULLCAREER-2026-07-22.md` (a CM there finished with
  2,779), so this looks like existing, intentional match-output abstraction
  rather than something round-3 touched or broke.
- The season-review line's ordinal suffix is always "th" (e.g. "finished
  1th in Premier League") — a text/cosmetic formatting issue, out of scope
  per this project's standing rule.

Lifestyle drift itself (not a round-3 fix, but observed live in the TUI
this round for the first time at this depth) worked exactly as an emergent,
intensity-driven weekly nudge should: three different fixed intensities
held the whole career produced three different, stable tiers by season 2
and never wavered — High → Professional (A), Low → Flashy (B), Medium →
Balanced/unchanged (C).
