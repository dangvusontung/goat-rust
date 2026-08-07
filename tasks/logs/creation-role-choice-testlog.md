# TASK-CORE-creation-role-choice — test log (2026-08-07T19:59:30+07:00)

## goat-core (generation: primary_role + default_role)
```
test generation::tests::all_attrs_in_valid_range ... ok
test generation::tests::chosen_role_is_natural_and_shapes_familiarity ... ok
test generation::tests::current_never_exceeds_potential ... ok
test generation::tests::different_seeds_produce_different_players ... ok
test generation::tests::chosen_position_favored_in_familiarity ... ok
test generation::tests::durability_is_deterministic_per_seed ... ok
test generation::tests::forward_key_attrs_outrank_zero_weight_attrs ... ok
test generation::tests::same_seed_same_player ... ok
test generation::tests::durability_varies_across_seeds ... ok
test generation::tests::none_role_path_is_unchanged_by_the_role_feature ... ok
test generation::tests::tactical_bias_shifts_technical_club_toward_technical_attrs ... ok
test generation::tests::primary_position_set_correctly ... ok
test generation::tests::winger_position_reachable_directly_and_shapes_generation ... ok
test generation::tests::unbiased_path_is_byte_identical ... ok
test generation::tests::tactical_bias_is_bounded ... ok
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 40 filtered out; finished in 0.01s
```

## golden_generate (frozen, primary_role: None path)
```
test golden_seed_12345_forward ... ok
test golden_seed_777_defender ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## goat-bridge (parity + role choice)
```
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test new_game_with_role_choice_matches_direct ... ok
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.35s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## codegen regen (pinned 2.9.0)
```
$ cd app && flutter_rust_bridge_codegen generate  → Done! (new_game primary_role: Option<u8> + list_roles_for_creation into frb_generated.rs + Dart bindings)
```

## workspace
```
suites ok: 39
only failure: smoke_stdin 10 pre-existing (baseline diff: BASELINE_UNCHANGED)
```

## fmt/clippy
```
fmt clean
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.09s
```

## SEED-OUTPUT CHANGE NOTICE
Match-sim output changes for existing seeds when the PC position is NOT
ST/CM/CB (those three map to the same role as the old family hardcode).
W/WM now play as Winger, CAM as AttackingMid, DM as DefensiveMid, FB as
FullBack — the match engine previously played every Midfielder as CentralMid
and every Forward as CompleteForward regardless of specific position.
