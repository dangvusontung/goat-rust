# TASK DESIGN ROUND 4 — Competitions: domestic cup, continental club tiers, World Cup & continental championships

**This doc is now a pointer — the full design has been split into 4 self-contained files,
2026-07-22, after Tùng resolved all 10 "[DECISION NEEDED]" items from the original design
conversation.** The split was explicit, at Tùng's request ("tạo subtask ra để phòng ngắt giữa
chừng" — create subtasks specifically so an interruption mid-implementation doesn't lose
everything), the same lesson an earlier round learned the hard way when one large uninterrupted
Dev pass needed several messy fixup sessions. Each file below is independently implementable and
carries its own "Verified" grounding excerpt, TDD anchors, out-of-scope notes, and Definition of
Done — a Dev subagent can pick up any one of them alone, with no memory of this doc or the
original design conversation, and fully implement that slice.

**Read and implement in this order:**

1. `tasks/TASK-DESIGN-round4-competitions-slice1-foundation.md` — `Competition` entity,
   `FixtureImportance` ladder, wiring real fixtures through `goat-calendar`'s existing Phase 1
   scaffold, and same-day conflict resolution. The foundation every other slice schedules its
   fixtures through. No prereq — start here.
2. `tasks/TASK-DESIGN-round4-competitions-slice2-3-club-cups.md` — domestic cup
   (single-elimination, tier-staggered entry, round-by-round random redraw) + continental club
   competitions (3 tiers, stature-ranked qualification, group-stage-then-knockout). Prereq:
   slice 1.
3. `tasks/TASK-DESIGN-round4-competitions-slice4-national-teams.md` — World Cup + continental
   championships (real-world 4-year/2-year-stagger cadence, a new off-season tournament window
   reusing the already-declared-but-unused `WindowKind::OffSeason`, a qualifying campaign, and a
   group-stage-then-knockout finals tournament). Prereq: slice 1 (hard), slices 2-3 (soft —
   reuses their bracket/draw pattern, though this slice's own knockout never needs bye-handling).
4. `tasks/TASK-DESIGN-round4-competitions-slice5-integration.md` — `SuspensionLedger` scoping
   (replacing the single global `pc_suspension_weeks` scalar) and final conflict-resolution/
   congestion sanity-check across all 7 competition kinds. Prereq: all 3 slices above — this is
   the integration pass that proves the whole design composes.

**What changed from the original design conversation, in one paragraph each:** the
`FixtureImportance` ladder, domestic cup bracket shape, and continental qualification pool
(Tier-1-domestic-only) were all confirmed exactly as originally proposed. The continental
slot-count formula was **replaced** (not confirmed) — the original 4-band table summed to 67
Tier-1-continental slots across 20 nations, rejected as too generous; the final design is a
per-nation-rank taper, independent per tier, tapering to exactly 0 for the weakest 6 of 20
nations, hitting exactly 32/48/64 total slots for Tier-1/2/3. The fixture format for both
continental club tiers and national-team tournaments was changed from the original
single-elimination recommendation to **group-stage-then-knockout** (Tùng's explicit call) — a
genuinely new piece of design work, detailed in slices 2-3 and 4 respectively (4-team groups
throughout, chosen to divide the 32/48/64 continental totals evenly, and extended to an 8-nation/
2-group national tournament for consistency). Season-1's coincidental alignment with a World-Cup
year needs no special-casing either way — the international cycle is independent of career start
by design, not something to force. The off-season tournament window turned out to already have a
home: `WindowKind::OffSeason` was declared and rendered in the TUI but never actually constructed
anywhere — slice 4 wires it up rather than adding a 5th window kind. The qualifying-campaign size
(6 matches/cycle) and the two new `LegacyEvidence` counters
(`career_world_cups_won`/`career_continental_championships_won`, mirroring the existing
`career_caps` precedent) were both confirmed exactly as proposed.

See each split file for the full spec, real code file:line grounding, TDD anchors, and
Definition of Done. This doc itself carries no remaining implementation content — do not add to
it; open a new design round doc instead, the same pattern every prior round in `tasks/` follows.
