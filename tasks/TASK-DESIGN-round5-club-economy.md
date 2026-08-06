# TASK DESIGN ROUND 5 — AI-run club economy: finances, transfers, managers

**This doc is now a pointer — the full design has been split into 6 self-contained files,
2026-07-22, because the original doc (1315 lines / ~73KB) was too large for one Dev pass — even
bigger than round-4's design, which was split into 4 files the same night for the same reason
("tạo subtask ra để phòng ngắt giữa chừng" — create subtasks specifically so an interruption
mid-implementation doesn't lose everything). Each file below is independently implementable and
carries its own "Verified" grounding excerpt, ground rules, TDD anchors, out-of-scope notes,
relevant "Decisions" items, and Definition of Done — a Dev subagent can pick up any one of them
alone, with no memory of this doc or the original design conversation, and fully implement that
slice group.

**Read and implement in this order:**

1. `tasks/TASK-DESIGN-round5-club-economy-slice1-2-foundation.md` — Club budget (one persisted
   number, additive-contributor income) + market valuation (the shared "what is this player
   worth" formula every later slice reads). No prereq — start here.
2. `tasks/TASK-DESIGN-round5-club-economy-slice3-4-scouting.md` — weakest-position detection +
   gem-hunting target search (both "find a transfer target" logic, sharing the scouting
   machinery). Prereq: slice 1-2.
3. `tasks/TASK-DESIGN-round5-club-economy-slice5-transfers.md` — deterministic bidding-round
   auction + transfer execution. The doc's own most novel/highest-risk piece (a real design
   choice: ascending-round auction), kept isolated. Prereq: slices 1-2 and 3-4.
4. `tasks/TASK-DESIGN-round5-club-economy-slice6-academy.md` — youth-academy investment lane,
   composing with round-3's existing youth intake. Prereq: slices 1-2 and 5.
5. `tasks/TASK-DESIGN-round5-club-economy-slice7-8-managers.md` — manager entity/appointment/
   tactical-identity shift + manager performance/firing/rehire. No prereq from this round's
   other slices (independent subsystem until integration) — can in principle be built any time
   after slice 1-2, but is sequenced here per the implementation order above. Note: the
   `matches_played` field this slice's own Slice 8.3 needs is already included in Slice 7.1's
   `Manager` struct in this file — no later modification of an already-shipped struct required.
6. `tasks/TASK-DESIGN-round5-club-economy-slice9-integration.md` — season-tick wiring into
   `ReplayCache::advance_one_season`, including the `&WorldGenesis` → `&mut WorldGenesis`
   signature-change ripple to every existing caller. Prereq: all 5 slices above — this is the
   integration pass that proves the whole design composes.

Every split file's own "Decisions Design made as judgment calls" section is **not blocking** —
per the original doc's own framing (same as round-3's judgment calls), these are first-pass
numbers flagged for a later `TASK-TUNE` pass once playtested, not items requiring Tùng's
sign-off before Dev starts.

See each split file for the full spec, real code file:line grounding, TDD anchors, and
Definition of Done. This doc itself carries no remaining implementation content — do not add to
it; open a new design round doc instead, the same pattern every prior round in `tasks/` follows.
