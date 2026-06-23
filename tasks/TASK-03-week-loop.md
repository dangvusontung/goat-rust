# TASK 03 — The week loop: training, energy, growth, events

Prereq: Phase 2 playable. Read CLAUDE.md + bible §5.4 + tech doc before coding.
Pause for review after each step.

## Step 1 — The week tick (goat-core)
The week is the core loop unit. A week intent advances state:
- **Training routine:** target attributes at chosen intensity. Intensity costs energy;
  gains are gated by the attribute's age-curve archetype (physical/technical/mental
  trainability per bible §5.1) and capped at potential. Playing a role grows its
  familiarity faster than training it; adjacent roles convert faster than distant.
- **Energy/fatigue:** tired players gain less and injure more; rest recovers energy.
- **Facilities/coaches multiplier:** a per-club development multiplier (stub values on
  the stub clubs from Phase 2) — this is the develop-vs-minutes dial's first half.
- **Age curves over time:** advancing weeks/seasons ages the player; physical declines
  early and hard, technical plateaus, mental keeps growing with experience.

## Step 2 — Random development events, by exception
Injuries, illness, breakthroughs, form spikes — rolled through injected RNG inside the
week tick. Injury risk scales with fatigue, intensity, age. Injured weeks tick down
automatically. Events surface as **exceptions**: the week tick returns a list of
noteworthy events; quiet weeks return none. Never spam.

## Step 3 — Routine + manage-by-exception in the TUI
- Set a weekly routine once; then "advance week" repeats it silently.
- "Advance N weeks / to next event" fast-forward: auto-runs and stops only when an
  exception fires (injury, breakthrough, milestone) — pillar §2.2 made playable.
- Intervene screen on big weeks: change routine, rest, push intensity.
- Show energy, sharpness, recent growth deltas on the player sheet.

## Tests
- Golden: fixed seed, fixed routine, N weeks → exact attribute/energy state.
- Property: never exceed potential; energy stays in bounds; old players' physical
  attrs trend down while mental can still rise; resting recovers energy.
- Long-horizon: simulate 16→38 years of weeks headless — no panics, invariants hold,
  the career-arc shape (rise, peak, decline) is visible in family values.
- TUI smoke test updated for the new flow.

## Out of scope
Matches (Phase 4), minutes/form from matches (Phases 4–5), loans (Phase 8),
money-bought trainers (Phase 10).
