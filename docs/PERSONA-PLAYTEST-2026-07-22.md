# Persona playtest — 2026-07-22
Started: Wed Jul 22 02:17:16 AM +07 2026

Method: 10 distinct personas, each played a real session of `./target/debug/goat-tui` via scripted stdin (character creation through several game-weeks and at least one full match), varying position/nation/division/club/seed and in-match choices to match how that persona would actually behave. All quotes below are exact terminal output, not paraphrased. This supersedes an earlier same-day 3-persona pass (stats veteran / narrative fan / efficiency optimizer) — those three are re-run here from fresh sessions for continuity, plus 7 new personas.

## Persona 1: Marcus "The Numbers" Whitfield — stats-obsessed veteran
ST, England, Manchester City, seed 100. Scrutinizes the attribute sheet before and after every action.

### Liked
- The attribute sheet is genuinely deep: 18 sub-attributes across 6 categories, each shown as `current / potential` (e.g. `Finishing 65 / 94`), plus a 14-entry Roles table with a numeric fit score and a Natural/Competent/Awkward descriptor. Real legible depth.
- Character creation exposes a raw `Seed (Enter = random):` field — visible proof the sim is fully seeded.

### Disliked / friction
- **OVR isn't derivable from the numbers shown.** Sheet showed `Pac:76 Sho:56 Pas:35 Dri:50 Def:37 Phy:39` with headline `OVR 64`. A simple average of those six category numbers is ~48.8 — nowhere near 64. Nothing on any screen explains the (presumably position-weighted) formula that gets from the visible categories to the headline number.
- **Training produced zero visible effect over 3 full weeks.** Set routine to Finishing/Shot Power/Close Control/Acceleration at Medium intensity (`Routine set: Medium intensity, 4 focus attrs.`), then trained (`W`) three weeks in a row. Every week: `Last week: Acceleration +0  Close +0  Finishing +0`. Re-checked the full sheet (`V`) before and after — every one of the 4 targeted attributes (Finishing 65, Shot Power 63, Close Control 61, Acceleration 76) was byte-identical, before and after.
- The Roles leaderboard order flipped (Target Forward overtook Complete Forward for #1) between two `V` checks with no training on any relevant attribute in between — small stat noise reorders the one screen this persona treats as authoritative, with no explanation offered.

## Persona 2: Camila Alameda — narrative/RPG fan
CAM, Brazil, Atlético MG (Série A). Reads every line of match prose, picks the more dramatic/flavorful option, checks lore screens (World/Legacy) between matches.

### Liked
- Picking nation = Brazil doesn't just reskin colors — it swaps in an entirely correct Brazilian league structure (`Série A (top)` / `Série B (second)`, clubs Flamengo/Palmeiras/Corinthians/São Paulo/etc.). Real worldbuilding payoff for someone playing for immersion.
- Match prose has genuine tension in places: *"Injury time. Everything still to play for. One last chance drops to you."*, *"The offside trap is beaten. It's you against the goalkeeper."*
- The World screen's framing is a nice long-arc narrative hook: a Pantheon of past greats (`Goran Petrov  England  9  97`) followed by *"No rival has kept pace — you reign alone (the weak-era asterisk looms)."* — a genuinely evocative piece of meta-narrative once you find it.

### Disliked / friction
- **Every match's key-moment recap is truncated mid-sentence** by the fixed-width box, cutting off exactly the emotional payoff line: `✓ 0'  Your run is timed to perfection — you`, `✓ 84'  Your early cross whips across the six-`.
- **The "key moment" line pool feels shallow.** Across only 2 matches in this session, lines like *"Your run is timed to perfection — you..."* and *"The ball hits the back of the net. Get..."* each showed up more than once verbatim (confirmed by grepping across other personas' matches too — see cross-cutting themes). For someone playing specifically for story variety, repeats surface almost immediately.
- **Decisions don't always match the position fantasy.** Playing CAM, several prompts were pure last-line-defender scenarios (*"Their winger makes a sharp run behind you. Stay or go?"* with tackle-style options 1-5) rather than something CAM-flavored — breaks the "this is my player's story" feeling.
- Character creation has no personalized intro/bio line — after picking name, position, nation, division, club and seed, the game jumps straight to the numeric sheet with no narrated acknowledgement of the specific combination chosen.

## Persona 3: Deja "Quickie" Okoye — efficiency optimizer
CM, England, Manchester United. Blasts through fast-forward, avoids menus, minimal clicks per session.

### Liked
- `F` (fast-forward) skips N weeks in a single keystroke + one number entry, no per-week confirmation.
- `K` (skip match) auto-resolves a full match with the exact same rich FT summary as manually playing — the "fast path" isn't a degraded one.

### Disliked / friction
- **Fast-forward silently auto-plays any match inside the skipped range, with no warning, and the result can be bad.** `F` → `10` ran straight through Round 1's fixture and the log came back with: `FULL TIME vs Brentford / Result: 0–4 (LOSS) / Rating: 0/100 ★` plus `🟥 RED CARD! You'll serve a suspension.` — discovered only after the fact, buried in the results dump. An optimizer using fast-forward to skip *training* weeks has no way to know a fixture was about to be auto-resolved on their behalf.
- Injuries interrupt the middle of a fast-forward block with no checkpoint: `*** EVENTS *** ⚠ INJURY! Out for 6 week(s).` just prints mid-block and advancement continues; there's no "stop and let you decide" moment.
- Ending up suspended (`Disc:Combative 🟨0 SUSPENDED(1)`) after an auto-resolved match the player never saw is a rough outcome to receive with zero agency.
- Pressing `W` more than once in the same fixture round is a silent no-op (identical screen redraws, no "already trained this week" message) — wastes keystrokes with no signal that anything failed to register.

## Persona 4: Harold Voss — skeptical pragmatist
CB, England, Everton. Doesn't buy that "no win condition" is meaningful; goes looking for evidence the Legacy/Pantheon system is actually wired up.

### Liked
- The Legacy screen's `PANTHEON RANKINGS` — four named judge archetypes (`The Trophy Cabinet`, `The Eye-Test Romantics`, `The Stats Purists`, `The Loyalty Traditionalists`), each scored 0-100 with a rank — is a legitimately interesting reframe of "no single win condition" as "several audiences judging your career differently." Worth surfacing more, because it's currently the strongest answer the game has to "what's the point."

### Disliked / friction
- **That same screen visibly does not react to actual play.** Checked Legacy pre-match: `Goals: 0  Matches: 0  Seasons: 0`, Pantheon scores `10/100 #11/11`, `10/100 #11/11`, `7/100 #11/11`, `37/100 #11/11`. Then played and **won** a match 4–3 (`FULL TIME vs Arsenal / Result: 4–3 (WIN)`). Checked Legacy again: `Goals: 0  Matches: 0`, and all four Pantheon scores/ranks byte-for-byte identical to before the win. For a persona already doubting the game has a point, watching a won match move nothing on the one screen billed as "the point" reads as confirmation, not rebuttal.
- Nothing in-game explains what the four Pantheon judges actually reward or how to move their scores — they read as flavor labels bolted onto a static number rather than a legible goal system.

## Persona 5: Priya "TAS" Nandakumar — speedrunner
DM, England, Manchester City, **fixed seed 777**. Tests reproducibility and looks for a deterministic optimal route.

### Liked
- **Confirmed full determinism.** Ran the identical input script twice against seed 777; `diff` on the two full session transcripts returned no differences at all — same rolled attributes, same match events, same everything. The seed field is a real, working reproducibility guarantee, exactly what a route-planning/TAS-style player needs.
- Fast, dense feedback loop for iterating routes: single-letter commands, no confirmation dialogs standing between decisions.

### Disliked / friction
- Same invisible-training problem as Persona 1: a High-intensity, 4-attribute routine (Strength/Stamina/Acceleration/Sprint Speed) produced `Last week: Sprint +0  Acceleration +0  Strength +0` every single week for 4 weeks — no signal to optimize against, and no visible difference between High and Low/Medium intensity to even compare.
- **The "Last week" delta line doesn't match the routine's own header.** With `Routine: Strength, Stamina, Acceleration, Sprint [High]` active, the delta line only ever lists 3 of the 4 declared attributes — Stamina never appears in `Last week: ...` even though it's explicitly one of the 4 selected focus attributes. For someone trying to reverse-engineer the training formula from observed deltas, the displayed data doesn't even match the declared inputs.
- Numeric prompts and the in-match choice menu loop forever (never exit) if stdin runs out mid-prompt instead of failing fast — relevant for anyone trying to script/automate route-testing against this binary.

## Persona 6: Jimmy "Geordie" Redknapp — traditional football purist
W, England, Newcastle. Cares about playing time and turning out for the shirt over trophies; checks the league table often.

### Liked
- The league table updates promptly and correctly: Round 0 all-zeros, Round 2 showed Newcastle 2 games unbeaten, `Pl 2 W 2 D 0 L 0 Pts 6`, sitting 2nd. Solid, legible context.
- Two matches played produced two different, non-copy-pasted results (4–3 wins over Brighton then Fulham) rather than reruns of the same script.

### Disliked / friction
- **There's no personal "appearances/minutes played" counter anywhere in the main loop.** The only proxy is the league Round number, which is a *fixture* counter, not a personal appearance count — Persona 3's session showed a round can pass with the player suspended and NOT playing, with nothing distinguishing "rounds I actually played" from "rounds I sat out." For someone who explicitly values playing time over trophies, there's no single number that answers "how many games have I actually played."
- Compounding that: Legacy's `Matches: 0` not updating after a played match (independently hit by Persona 4) means even the one screen that *should* have this number doesn't show it.

## Persona 7: Tyler "One-Tap" Reyes — casual, short attention span
WM, Brazil, Chapecoense (Série B). Wants a short session, picks the first listed option every time, easily overwhelmed by dense text.

### Liked
- The core loop really is just single-letter presses; a full match resolves via a handful of `1` taps, so an end-to-end session genuinely can be a few minutes.
- The full-time card is scannable at a glance the first time you see it: `Result: 1–1 (DRAW) / Rating: 40/100 ★★★` — no digging required for the headline.

### Disliked / friction
- **The very first thing shown, before you've done anything, is an 18-line attribute wall** (6 categories, sub-stats each with a `current/potential` pair). For a "just let me play" player this is a lot of unrequested density up front.
- **Repeatedly pressing `1` after the match ended produced ~8 back-to-back, pixel-identical redraws of the same status box, with zero feedback** — no "invalid command," no acknowledgement of any kind. To a casual player mashing a button, this reads as "did this freeze?" rather than "your input was a no-op."
- Always taking option 1 produced a middling result (draw, `40/100`) with no signposting anywhere that option order isn't difficulty- or safety-ordered — a satisficing player has no cue that "always pick the top option" is a weak heuristic here.

## Persona 8: Ben "Theorycraft" Adeyemi — min-maxer / build theorycrafter
CAM, England, Arsenal. Re-rolls repeatedly comparing role-fit numbers, retunes training routines to compare deltas.

### Liked
- Re-roll (`R`) is instant and free before starting, and the Roles table (14 roles, numeric fit + Natural/Competent/Awkward tag) is exactly the side-by-side comparison surface a min-maxer wants for deciding whether to keep or re-roll a build.

### Disliked / friction
- **Found a real trap doing exactly what this persona naturally does.** Repro: `N` → name → position → nation → division → club → blank seed → `R` (re-roll) → *blank Enter* → `S`. The blank Enter at the `[S] Start game  [R] Re-roll  [Q] Back` prompt silently behaves like `Q` — it discards the whole in-progress character and drops back to the title screen, with no confirmation. The following `S` then lands on the *title* menu instead of starting the game. A min-maxer who re-rolls several times while spamming Enter to advance between comparisons will lose their work with zero warning.
- **Changing the training routine mid-session desyncs the status box.** Switched routine from Vision/Crossing/Short Pass/Ball Control to Finishing/Long Shots/Shot Power/Close Control, then advanced a week. The header then showed `Routine: Finishing, Long, Shot, Close` directly above `Last week: Ball +0  Short +0  Crossing +0` — the routine name and the delta line one row below it reference two completely different sets of attributes. For someone trying to attribute cause → effect precisely, this is actively misleading, not just cosmetic.
- Same "+0 regardless of routine/intensity" training result every other numbers-focused persona hit.

## Persona 9: Grace Lindqvist — accessibility-minded player
FB, England, West Ham. Pays attention to whether meaning is carried redundantly in text, not just color/emoji/glyphs.

### Liked
- Match rating is doubled up as both number and stars (`Rating: 40/100 ★★★`) — redundant coding done correctly; it still works with stars or color stripped out.

### Disliked / friction
- **Energy is rendered only as a 10-character block-glyph bar** — `Energy █████████░` / `Energy ██████░░░░` — in both the persistent header and the full sheet (`V`). There is no numeric percentage anywhere. It's impossible to tell if "6 blocks" is 55% or 64%, and a screen reader has literally nothing to announce beyond "some blocks."
- **Discipline status is a single emoji plus a bare count with no label**: `Disc:Neutral 🟨0`. Nothing nearby says what the number counts (cards this match? this season? career?) — the meaning lives entirely in an icon + an unlabeled digit.

## Persona 10: Oskar "Save Scummer" Talvela — completionist / save-scummer
CB, Brazil, São Paulo. Saves often, tries to load and replay, wants to explore alternate choice branches.

### Liked
- Save/Load round-tripped correctly end to end: saved mid-week (`Game saved to goat.sav.`), quit to the title screen, chose Load, and got the exact prior Age/Round/Form/OVR state back (`Save loaded.`). The core promise works.

### Disliked / friction
- **Exactly one save slot, silently overwritten every time.** `Z` always prints the same message (`Game saved to goat.sav.`) with no filename prompt, no "overwrite existing save?" confirmation, and no way to see a list of past saves. There's no way to keep a "before this match" checkpoint alongside a "current" save to compare how a different in-match choice would have played out — which is the entire point of this playstyle.
- **Load is only reachable from the title screen.** Typing `L` mid-session, inside the main `W/F/S/P/...` loop, does nothing — it's silently swallowed exactly like any other invalid main-loop letter (see cross-cutting theme below). Reloading always costs a full Quit → title → Load round trip, even when the player already knows they want to rewind right now.

## Cross-cutting themes
(Patterns that 3+ personas independently hit, unprompted)

1. **Training feedback is invisible.** Every persona who set a routine and checked the "Last week" delta (Whitfield/#1, TAS/#5, MinMax/#8) saw `+0` on every tracked attribute, every week sampled, regardless of Low/Medium/High intensity or which attributes were targeted — and the underlying attribute sheet was pixel-identical before and after multiple weeks of training. Whether this is a display bug or a genuinely near-zero weekly progression rate, from the player's chair training currently reads as a decorative menu with no felt effect.

2. **OVR can't be derived from the numbers the game shows you.** Multiple independent builds (#1: categories 76/56/35/50/37/39 → OVR 64; separately observed categories 37/35/46/44/41/54 → OVR 75 on another build) have a headline OVR far from a simple average of the six displayed category scores, with no screen anywhere explaining the (apparently position-weighted) formula. Every numbers-oriented persona (stats veteran, speedrunner, min-maxer) flagged this as an opacity/trust problem independently.

3. **The Legacy/Pantheon meta-progression doesn't visibly react to a played match.** Two personas (Doubter/#4, Geordie/#6) checked "Matches" and Pantheon scores before and after playing and winning a full match in the same session — every number was identical. This is the screen most likely to answer "why keep playing," and right now it looks frozen regardless of what you just did.

4. **Match-end "key moment" text is truncated by the fixed-width box on nearly every match sampled** (#1, #2, #4, #6, #9 all show lines cut off mid-word: `...you thump it into th`, `...six-`, `...just enough to dive`). Purely cosmetic, but it's the single most-repeated visual bug across the whole playtest, and it always clips the emotional payoff line of the match.

5. **Invalid/no-op input is handled inconsistently and sometimes dangerously.** Numeric prompts (position, seed, weeks-to-advance) loop with a visible error message (`Please enter a number.`, `Enter 1–2.`) on bad input. But at the main game loop, a stray letter is silently swallowed with **no message at all** (screen just redraws identically — hit independently by Optimizer/#3, Casual/#7 and Scummer/#10), while a **blank Enter at the character-confirm screen (`[S] Start / [R] Re-roll / [Q] Back`) silently discards the in-progress character and returns to the title with zero confirmation** (hit by MinMax/#8 while doing exactly what that persona naturally does — re-rolling and pressing Enter between comparisons). The same "did nothing happen" input gets two very different real behaviors depending on which screen you're on, and neither tells you which one just occurred.

6. **Match-scenario/key-moment text isn't filtered by player position** — a CAM (#2) got pure last-line-defender decision prompts, and this echoes the same content-pool/tagging gap noted in the earlier same-day pass for a striker being asked to hold an offside line. Worth fixing alongside item 4 since both live in the same text-generation path.

Completed: Wed Jul 22 02:26:31 AM +07 2026
