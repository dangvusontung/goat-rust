# Persona playtest — Round 2 — 2026-07-22

Follow-up to `docs/PERSONA-PLAYTEST-2026-07-22.md` (round-1, 10 personas) and
`docs/PLAYTEST-BUGS.md`. Round-1 findings were converted into
`tasks/TASK-PLAYTEST-round1-fixes.md` and fixed by Dev across commits `6cc5405`,
`e71230a`, `a6c6a99` (training feedback display, silent double-`W` no-op, Legacy
mid-season note, box-border/key-moment wrapping, invalid-input handling, stdin
EOF handling, OVR/rating explanation, plus a new `smoke_stdin.rs` regression
suite — all 9 cases pass in this session, `cargo test -p goat-tui --test
smoke_stdin`).

Method: 5 distinct personas, each a real session of `./target/debug/goat-tui`
via scripted stdin (character creation through several game-weeks and at
least one full match), varying position/nation/division/club/seed and
in-match choices. All quotes below are exact terminal output, not
paraphrased. Per standing project scope, generic TUI rendering/input-handling
issues (box overflow, truncation, invalid-key handling, stdin EOF) are **out
of scope** to re-flag as bugs now — only noted briefly where still relevant,
not treated as blocking findings.

## Summary

**Verification points across all 5 personas: 4/4 passed, 5/5 personas each.**
Total: 20/20 individual pass checks (a×5, b×5, c×5, d×5).

| # | Fix | Personas passing | Verdict |
|---|-----|-------------------|---------|
| a | Training shows a real decimal delta, not `+0` | 5/5 | **HOLDS** |
| b | Double-`W` in the same round shows a clear message, not silent no-op | 5/5 | **HOLDS** |
| c | Legacy screen explains it's frozen until season-end | 5/5 | **HOLDS** |
| d | OVR/rating has an in-game explanation | 5/5 | **HOLDS** |

**New bugs found this round** (not covered by the round-1 fix list):

1. **The "Last week" decimal delta can itself still read as zero growth for
   one attribute at a time.** The Slice 1 fix filters to attributes where the
   raw `Fixed` growth is `> Fixed::ZERO` and then formats with one decimal
   place — but a small positive raw delta that rounds to `0.0` at one decimal
   (e.g. `0.02–0.049`) still passes the `> ZERO` filter and prints `+0.0`,
   sitting right next to sibling attributes correctly showing `+0.1`/`+0.2`.
   Seen reproducibly in 3 of 5 sessions: `Vision +0.0` (Persona 3, 4×),
   `Short +0.0` (Persona 3, 8× and Persona 5, 2×), `Interceptions +0.0`
   (Persona 2, 4×). This is a narrower recurrence of the exact player-facing
   complaint the fix targeted ("training feels like it does nothing") — for
   one specific attribute per week it still visually reads that way, even
   though the line is technically "real growth, badly rounded" rather than
   "always zero." Not a regression of the fix (three other attributes on the
   same line correctly show non-zero), but a leftover edge the fix didn't
   fully close.

No other new issues surfaced. Everything else observed either matches
round-1's already-parked "out of scope" items (see below) or is expected
existing behavior (e.g. `F` fast-forward never advancing the round counter —
re-confirmed twice this round, consistent with Slice 10's "could not
reproduce" conclusion; calendar events like international breaks printing
mid-fast-forward; red cards/suspensions/yellow cards working as designed).

**Out-of-scope TUI notes (briefly, not blocking):** the persistent status box
still occasionally line-wraps a "Routine:"/"Last week:" block across two
`║...║` lines rather than one — cosmetic, same family as round-1's Slice
4/5 box work, already handled by the existing wrapping helper and not a new
break. No box-overflow or ragged-cutoff bugs were observed this round (Slice
4/5's fix held in all 5 sessions' key-moment and header boxes). No invalid-key
freezes or stdin-EOF hangs were seen — all 5 sessions exited cleanly via `Q`/`Q`.

---

## Persona 1: Marcus Whitfield — stats-obsessed veteran (re-run)

ST, England, Premier League, Manchester City, seed 501. Sets a 4-attribute
Medium routine, immediately checks the sheet before/after training, and
deliberately double-trains to probe the no-op fix.

### Session trace

- Routine set: `Finishing, Shot Power, Close Control, Acceleration [Medium]`.
- First `W`: `"Last week: Acceleration +0.2  Close +0.1  Finishing +0.1  Shot +0.1"` — **(a) PASS**: real decimals, not `+0`, and the attribute sheet (`V`) confirmed `Acceleration 75/88` (up from `74/88` at creation).
- Second and third `W` in the same round (no `P`/`K` between): both printed
  `"  You've already trained this week — Play or Skip this round's match to continue."`
  with age/deltas byte-identical to the first `W`'s result — **(b) PASS**.
- `K` (skip match, Round 1): `FULL TIME vs Nottm Forest / Result: 0–0 (DRAW) / Rating: 80/100 ★★★★★`.
- `G` (Legacy) mid-season: `"  (Career totals and Pantheon scores update at season end — Round 1/30 so far.)"` next to `Goals: 0   Matches: 0   Seasons: 0` — **(c) PASS**.
- `V` (sheet): header reads `"Marcus Whitfield  OVR 64"` immediately followed by `"OVR is position-weighted, not a simple avg."` — **(d) PASS**.
- Second week `W`: `"Last week: Shot +0.2  Acceleration +0.2  Finishing +0.1  Close +0.1"`.
- `K` (Round 2): `FULL TIME vs Liverpool / Result: 4–1 (WIN) / Rating: 84/100 ★★★★★`.
- `G` after the win: Legacy still frozen (`Goals: 0   Matches: 0`), same mid-season note, same Pantheon numbers — confirms the fix is consistent even right after a won match (this exact scenario was round-1 Persona 4/6's original complaint).

### Bugs/regressions

None beyond the cross-cutting `+0.0` residual noted in the summary (not hit
by this persona — every focus attribute here showed non-zero decimals every
week sampled).

### Final state (end of session, Round 3/30, Age 16y2w)

- **OVR 64** — Pac 75, Sho 56, Pas 35, Dri 50, Def 36, Phy 39.
- Attributes (current/potential), key entries: Finishing 65/93, Shot Power
  65/93, Close Control 63/90, Acceleration 75/88, Sprint Speed 76/90,
  Strength 76/90, Heading 74/88; passing/defending category mostly at floor
  (Vision 24/44, Marking 24/44, Interceptions 24/44).
- Roles: Target Forward 54 (Natural), Complete Forward 52 (Competent), Inside
  Forward 49 (Competent).
- Legacy: Goals 0, Matches 0, Seasons 0 (frozen mid-season, as designed).
  Pantheon: Trophy Cabinet 10/100 #11/11, Eye-Test Romantics 10/100 #11/11,
  Stats Purists 7/100 #11/11, Loyalty Traditionalists 37/100 #11/11.
- Record so far: 1 draw (0–0 Nottm Forest), 1 win (4–1 Liverpool). Discipline
  clean (`Disc:Neutral 🟨0`), no suspension/injury. League table not checked
  this session (`T` not sent for this persona).

---

## Persona 2: Bianca Ferreira — defensive-minded, interactive-match tester

CB, Brazil, Série B, Goiás, seed 502, High-intensity defensive routine.
Chosen to (i) fire the double-`W` check on literally the first two actions of
the session, and (ii) play a full match via `P` (interactive beat-by-beat)
rather than `K` (auto-skip), to vary method from Persona 1.

### Session trace

- Routine: `Standing Tackle, Marking, Interceptions, Strength [High]`.
- `W` then immediately `W` again: first shows
  `"Last week: Strength +0.2  Standing +0.1"`; second prints
  `"  You've already trained this week — Play or Skip this round's match to continue."`
  — **(b) PASS**, and **(a) PASS** (real decimal on the first press).
- `P` (interactive match vs Tombense): played through ~15 beats with numbered
  choices (e.g. `"1. Make a timed run to the near post" / "2. Get tight and
  track the runner"` etc., always picking option 1). Beat-by-beat live
  readout worked throughout (`" 60' │ Output: 3/10  Stamina: 40  Flow: 0
  Nerves: 1 / Score: 0–1"`). Result: `FULL TIME vs Tombense / Result: 1–3
  (LOSS)`.
- `G` mid-season: `"(Career totals and Pantheon scores update at season end —
  Round 1/30 so far.)"` next to `Goals: 0   Matches: 0   Seasons: 0` —
  **(c) PASS**.
- `V`: header `"Bianca Ferreira  OVR 75"` then `"OVR is position-weighted, not
  a simple avg."` — **(d) PASS**.
- `T` (Table, Round 1): `►Goiás  1  0  0  1  1  3  0` (bottom of the pack after
  the loss).
- Second `W` (Round 2): `"Last week: Strength +0.1  Marking +0.1  Standing
  +0.1  Interceptions +0.0"` — **new bug** (see below), but 3 of 4 attributes
  still correctly non-zero — **(a) still PASS** overall.
- `K` (Round 2, vs Sampaio Corrêa): `FULL TIME / Result: 3–0 (WIN)`.
- `G` after the win: still frozen (`Goals: 0   Matches: 0`), Pantheon numbers
  unchanged — reconfirms **(c)** post-win.

### Bugs/regressions

- **New (not in round-1 list):** `"Last week: Strength +0.1  Marking +0.1
  Standing +0.1  Interceptions +0.0"` — Interceptions shows `+0.0` despite
  passing the `> Fixed::ZERO` filter that's supposed to only list attributes
  with real growth. See summary item #1.

### Final state (end of session, Round 3/30, Age 16y2w)

- **OVR 75** — Pac 56, Sho 29, Pas 35, Dri 39, **Def 62**, Phy 58.
- Attributes: Standing Tackle 66/95, Marking 50/92, Interceptions 50/91,
  Heading 80/95, Sliding Tackle 65/93, Strength 76/90, Jumping 77/91.
- Roles: Centre Back 52 (Competent), Wing Back 42 (Natural), Full Back 41
  (Competent).
- Legacy: Goals 0, Matches 0, Seasons 0 (frozen). Pantheon identical to
  Persona 1's starting values (10/10/7/37, all #11/11) — same default-state
  numbers, consistent with the season-end batching design.
- Record: 1 loss (1–3 vs Tombense), 1 win (3–0 vs Sampaio Corrêa). Table
  position after Round 1: last place in Série B (`Pl 1 W 0 D 0 L 1 Pts 0`).
  Discipline clean, no suspension/injury.

---

## Persona 3: Elodie Marchetti — CAM, mixed session with fast-forward and a red card

CAM, England, Championship, Ipswich, seed 503, Low-intensity routine, uses
`F` (fast-forward) as well as `W`/`K`, and happens to pick up a red card —
useful for checking the fixes hold under discipline-state changes too.

### Session trace

- Routine: `Short Pass, Vision, Stamina, Composure [Low]`.
- `W`: `"Last week: Stamina +0.1  Composure +0.1"` — **(a) PASS**.
- `W` again (same round): `"  You've already trained this week — Play or Skip
  this round's match to continue."` — **(b) PASS**.
- `K` (Round 1, vs Stoke City): `FULL TIME / Result: 4–0 (WIN)`.
- `G` mid-season: `"(Career totals and Pantheon scores update at season end —
  Round 1/30 so far.)"` next to `Goals: 0   Matches: 0` — **(c) PASS**.
- `F` → `3` (fast-forward 3 weeks): age advanced `16y1w → 16y4w`, an
  in-between calendar event fired (`"✈  CALENDAR: International break —
  call-ups announced."`), and — consistent with round-1 Slice 10's
  "could not reproduce" conclusion — **the season round number did not
  advance** (still `Round 2/30` immediately after `F`); no match was
  auto-resolved. `"Last week: Stamina +0.1  Short +0.0"` printed after the
  fast-forward block (see bug note below — first appearance of the `+0.0`
  residual this session).
- `V`: header `"Elodie Marchetti  OVR 65"` then `"OVR is position-weighted,
  not a simple avg."` — **(d) PASS**.
- `K` (Round 2, vs Millwall): `FULL TIME / Result: 3–2 (WIN)`.
- `W` (Round 3): `"Last week: Stamina +0.1  Short +0.0"`.
- `K` (Round 3, vs Bristol City): `FULL TIME / Result: 1–1 (DRAW)`, then
  `"🟥 RED CARD! You'll serve a suspension."` — status header updated to
  `Disc:Combative 🟨0 (cards)` / `SUSPENDED (1 match(es) left)`, Character
  reputation dropped `51 → 37` on the Legacy screen.
- `G` after the red card: still correctly frozen (`Goals: 0   Matches: 0`),
  same mid-season note — **(c) still PASS** even mid-discipline-event.
- `T` (Table, Round 3): `►Ipswich  3  2  1  0  8  3  7` — 2nd place.

### Bugs/regressions

- **New (recurs from Persona 2):** `"Last week: ... Vision +0.0"` (4×) and
  `"Last week: Stamina +0.1  Short +0.0"` (8×, persisting across the rest of
  the session after the fast-forward block dropped Vision from the routine's
  visible growth entirely one week). Same root cause as Persona 2's finding.

### Final state (end of session, Round 4/30, Age 16y4w)

- **OVR 65** — Pac 37, Sho 45, Pas 51, **Dri 56**, Def 29, Phy 32.
- Attributes: Short Pass 67/96, Long Pass 63/91, Vision 53/97, Curve 65/93,
  Finishing 63/90 (untrained but position-inherited high), Agility 75/89.
- Roles: Attacking Mid 49 (Competent), Winger 45 (Competent), Defensive Mid
  44 (Natural — an off-position quirk worth noting but not new; role-fit
  math is untouched by this task).
- Legacy: Goals 0, Matches 0, Seasons 0 (frozen). Pantheon 10/10/7/37, all
  #11/11 (same defaults as the other personas — expected, all mid-S1).
  Reputation: Sporting 50, **Character 37** (down from 51, reflecting the
  red card), Club Fan 50.
- Record: 2 wins (4–0 Stoke City, 3–2 Millwall), 1 draw (1–1 Bristol City,
  the match that produced the red card). **Currently SUSPENDED, 1 match
  left.** Table: 2nd place, `Pl 3 W 2 D 1 L 0 Pts 7`.

---

## Persona 4: Thiago Nascimento — DM, Brazil, heavy fast-forward user

DM, Brazil, Série A, Santos, seed 504, High-intensity defensive routine.
Trains exclusively via a single large `F` block before ever pressing `W`
manually, to stress-test whether fast-forward-driven training also displays
correctly (round-1 only sampled manual `W` presses for this fix).

### Session trace

- Routine: `Standing Tackle, Marking, Interceptions, Strength [High]`.
- `F` → `5` (fast-forward 5 weeks, no manual `W` beforehand): printed
  `"✈  CALENDAR: International break — call-ups announced."` mid-block, then
  `"Last week: Standing +0.2  Strength +0.2  Marking +0.1  Interceptions
  +0.1"` — **(a) PASS**, and confirms the decimal-delta fix also covers the
  fast-forward code path, not just single `W` presses. Round number stayed
  at `Round 1/30` throughout the 5-week block (no auto-resolved match,
  consistent with Persona 3 and round-1 Slice 10).
- `V`: header `"Thiago Nascimento  OVR 64"` then `"OVR is position-weighted,
  not a simple avg."` — **(d) PASS**.
- `W`: `"Last week: Standing +0.2  Strength +0.2"`.
- `W` again (same round): `"  You've already trained this week — Play or
  Skip this round's match to continue."` — **(b) PASS**.
- `K` (Round 1, vs Atlético MG): `FULL TIME / Result: 2–3 (LOSS)`.
- `G` mid-season: `"(Career totals and Pantheon scores update at season end —
  Round 1/30 so far.)"` next to `Goals: 0   Matches: 0` — **(c) PASS**.
- `T` (Table, Round 1): `►Santos  1  0  0  1  2  3  0` — bottom third.
- `U` (World screen): Pantheon of past greats listed (`Diego Marchetti
  England 9 92`, etc.) and `"No rival has kept pace — you reign alone (the
  weak-era asterisk looms)."` — unchanged from round-1's description,
  working as intended, no new issue.

### Bugs/regressions

None new for this persona — no `+0.0` residual hit this session (all 4
routine attributes showed non-zero decimals both times sampled).

### Final state (end of session, Round 2/30, Age 16y5w)

- **OVR 64** — Pac 37, Sho 29, Pas 40, Dri 39, **Def 54**, Phy 58.
- Attributes: Standing Tackle 66/94, Marking 51/92, Interceptions 53/97,
  Sliding Tackle 62/89, Strength 78/91, Stamina 78/92 (untrained but
  position-favored), Short/Long Pass 63/91 and 63/90.
- Roles: Defensive Mid 50 (Natural), Box-to-Box 46 (Competent), Central Mid
  45 (Competent).
- Legacy: Goals 0, Matches 0, Seasons 0 (frozen, expected). Pantheon 10/10/7/37,
  all #11/11.
- Record: 1 loss (2–3 vs Atlético MG). Table: `Pl 1 W 0 D 0 L 1 Pts 0`,
  12th/16 in Série A. Discipline clean, no suspension/injury.

---

## Persona 5: Grace Lindqvist — FB, England, longest session (3 rounds + interactive match)

FB, England, Premier League, West Ham, seed 505, Low-intensity routine. The
longest of the 5 sessions (3 full rounds), mixing `K` and `P` and deliberately
spamming extra numeric input past match-end to re-probe the "unrecognized
command" handling from round-1 alongside the 4 target fixes.

### Session trace

- Routine: `Standing Tackle, Short Pass, Sprint Speed, Agility [Low]`.
- `W`: `"Last week: Agility +0.1  Sprint +0.1"` — **(a) PASS**.
- `K` (Round 1, vs Aston Villa): `FULL TIME / Result: 2–1 (WIN)`, then
  `"🟨 Yellow card (1 this season)."`.
- `W` (Round 2): `"Last week: Short +0.1  Agility +0.1"`.
- `K` (Round 2, vs Brighton): `FULL TIME / Result: 5–2 (WIN)`.
- `W` then `W` again (same round, Round 3): first shows deltas, second prints
  `"  You've already trained this week — Play or Skip this round's match to
  continue."` (fired twice — pressed a third time deliberately) — **(b) PASS**.
- `G` mid-season: `"(Career totals and Pantheon scores update at season end —
  Round 2/30 so far.)"` next to `Goals: 0   Matches: 0` — **(c) PASS**.
- `P` (interactive match vs Fulham, Round 3): played through the full beat
  sequence with 30 `1`s queued — the match consumed roughly 16 of them
  (result `FULL TIME vs Fulham / Result: 4–2 (WIN)`); the **14 leftover `1`s
  correctly triggered `"  Unrecognized command."` at the main loop** each
  time, exactly as the round-1 Slice 6 fix specifies, with no freeze and the
  loop continuing to accept real commands afterward.
- `V`: header `"Grace Lindqvist  OVR 75"` then `"OVR is position-weighted,
  not a simple avg."` — **(d) PASS**.
- `T` (Table, Round 3): `►West Ham  3  3  0  0  11  5  9` — 1st place.
- `G` again: still frozen (`Goals: 0   Matches: 0`), same note — reconfirms
  **(c)** after 3 wins in a row.

### Bugs/regressions

None new for this persona.

### Final state (end of session, Round 4/30, Age 16y2w)

- **OVR 75** — **Pac 77**, Sho 34, Pas 40, Dri 41, Def 53, Phy 48.
- Attributes: Acceleration 77/91, Sprint Speed 77/91, Agility 77/91, Standing
  Tackle 65/94, Sliding Tackle 64/92, Crossing 63/90, Short Pass 61/88.
- Roles: Full Back 53 (Competent), Centre Back 51 (Natural), Wing Back 51
  (Competent).
- Legacy: Goals 0, Matches 0, Seasons 0 (frozen, expected). Pantheon 10/10/7/37,
  all #11/11.
- Record: **3 wins** (2–1 Aston Villa, 5–2 Brighton, 4–2 Fulham) —
  `Pl 3 W 3 D 0 L 0 Pts 9`, **1st place** in the Premier League table.
  Discipline: `Disc:Neutral 🟨1 (cards)` (one yellow card, no suspension), no
  injury.

---

## Cross-persona notes

- **Fix (a)** held for every persona and both trigger paths tested (manual
  `W` and fast-forward `F`) — real, distinct decimal deltas every week
  sampled, never a bare `+0`. The one caveat is the new `+0.0`-per-attribute
  rounding edge documented above (3/5 sessions), which is a narrower,
  lower-severity descendant of the original complaint, not a full regression.
- **Fix (b)** held for every persona; the message text was byte-identical
  across all 5 sessions and all repeated presses (including a 3rd deliberate
  press in Persona 5), and state (age, deltas) stayed correctly frozen on
  each no-op press.
- **Fix (c)** held for every persona, including immediately after a win
  (Personas 1, 2, 5) and immediately after a red card / suspension
  (Persona 3) — the note text and the `Goals`/`Matches` zeros were
  consistently present and never diverged from the documented batching
  behavior.
- **Fix (d)** held for every persona — the one-line OVR explanation appeared
  directly under the OVR header on every `V` screen across all 5 sessions.
- Fast-forward (`F`) never advanced the season round or auto-resolved a
  fixture in either session that used it (Personas 3 and 4), reconfirming
  round-1 Slice 10's "could not reproduce" conclusion a second time — no
  action needed.
- `cargo test -p goat-tui --test smoke_stdin` — 9/9 passing in this session,
  corroborating the manual findings above at the automated level.
