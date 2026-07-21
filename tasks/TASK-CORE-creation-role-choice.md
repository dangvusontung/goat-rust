> **Status note:** specific-*position* selection (one of the 8 `PrimaryPosition`s —
> ST/W/WM/CAM/CM/DM/FB/CB — instead of the 3 broad families) is now DONE. This file's
> proposal — picking a specific *role* (one of 14 `RoleId`s), a level deeper than
> position — remains a distinct, not-yet-done follow-up.

# TASK CORE — Let the player choose a primary role (not just Def/Mid/Fwd) at creation

Requested by the Flutter client (2026-07-02): the creation flow only offers three
positions, but the game models 14 roles — players want to pick what kind of player
they are.

## Where the limits are today

- `goat-core/src/generation.rs` — `Position` enum has exactly 3 variants
  (Goalkeeper deliberately parked, bible §11). `CreationChoices.position` is the only
  positional input.
- `generate_player` step 3 (`roll_primary_role`) ROLLS the primary role from all 14
  `RoleId`s, weighted 3× toward the chosen position family — the player cannot pick
  Winger vs Poacher vs Trequartista; the dice do.
- The bridge `new_game(position: u8 /* 0..2 */)` mirrors this, and `play_round` /
  `start_interactive_match` hardcode `MatchSetup.player_role` to one role per family
  (CentreBack / CentralMid / CompleteForward) — the generated `primary_position`
  (which OVR uses) is IGNORED by the match engine. Choosing a role is only meaningful
  if this also switches to `view.primary_position`.

## Proposed change

1. **Core:** add `primary_role: Option<RoleId>` to `CreationChoices`. When `Some`,
   `generate_player` skips `roll_primary_role` and uses it directly (position family
   derived via `ROLE_POSITION_FAMILY`, so `position` can be inferred and possibly
   dropped from the UI). When `None`, behavior is byte-identical to today — existing
   `golden_generate` values MUST stay frozen.
2. **Bridge:** `new_game` gains `primary_role: Option<u8>` (RoleId discriminant 0..13);
   `list_roles_for_creation()` or reuse of `get_roles()` metadata (name + family) for
   the picker. Requires `flutter_rust_bridge_codegen generate` (toolchain confirmed
   working, 2.12.0).
3. **Bridge match setup:** use `view.primary_position` for `MatchSetup.player_role`
   instead of the per-family hardcode — in BOTH `play_round` and
   `start_interactive_match`. NOTE: this changes match-sim outputs for existing
   seeds even without role choice, so gate it carefully (it is arguably a separate,
   pre-existing bug).

## Tests

- `golden_generate` unchanged with `primary_role: None`.
- New spec: `primary_role: Some(Winger)` ⇒ familiarity Natural for Winger, Competent
  for the Midfielder/Forward family it belongs to, potentials shaped by Winger key attrs.
- Bridge parity suite (`spec_bridge_parity.rs`) extended: `new_game` with a role
  produces a snapshot whose OVR uses that role.

## Non-goals

- Goalkeeper stays parked (bible §11).
- No UI text for role descriptions in core (see TASK-BRIDGE-refresh D1).
