//! goat-tui — text-mode renderer for the GOAT simulation.
//!
//! Structure: read state → render → read input → build intent → call reduce().
//! Zero simulation logic, zero randomness, zero game rules here.

use std::cmp::Reverse;
use std::io::{self, BufRead, Write};

use goat_core::{
    attrs::{
        AttrId, ATTR_NAMES, DEFENDING_ATTRS, DRIBBLING_ATTRS, PACE_ATTRS, PASSING_ATTRS,
        PHYSICAL_ATTRS, SHOOTING_ATTRS,
    },
    derive::{derive_attrs, ovr, role_rating},
    generation::{generate_player, CreationChoices},
    player::PlayerView,
    positions::PrimaryPosition,
    roles::RoleId,
    state::{reduce, should_retire, Intent, WorldState},
    week::{DevelopmentEvent, Intensity, Routine},
};
use goat_fixed::Fixed;
use goat_match::{
    beats::ScoreEvent,
    discipline::RefPersonality,
    sim::{
        advance_beat, auto_play_match, start_match, ActiveMatchState, BeatLibrary, MatchResult,
        MatchSetup,
    },
};
use goat_meta::{
    all_rankings, compute_axes, compute_golden_boot, compute_player_of_year, compute_reputation,
    pundit_comment, update_club_fan_rep, update_sporting_rep, LegacyEvidence, PunditContext,
    AXIS_NAMES, PUNDITS, SCHOOLS,
};
use goat_rng::{GoatRng, RngSource};
use goat_save::save::{
    from_world_state, list_slots, load_from_file, save_to_file, slot_path, to_world_state,
    SaveSlotSummary,
};
use goat_traits::PlayerTraits;
use goat_world::{
    fixture_for_round, format_week_header, round_fixtures, round_to_week, sim_team_match,
    world::WorldGenesis, Table, BASE_CAREER_YEAR, CLUBS_PER_DIV, NUM_NATIONS, ROUNDS_PER_SEASON,
    TIERS_PER_NATION,
};

#[path = "orbit_fixtures.rs"]
mod orbit_fixtures;
use orbit_fixtures::build_season_orbit_fixtures;

const SAVE_DIR: &str = "saves";
const NUM_SLOTS: u8 = 9;
const BEATS_JSON: &str = include_str!("../../../beats.json");

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    let mut lines = stdin.lock().lines();

    std::fs::create_dir_all(SAVE_DIR).ok();

    // Try to load beats.json from disk (allows updates without recompile);
    // fall back to the compiled-in copy.
    let beats_json =
        std::fs::read_to_string("beats.json").unwrap_or_else(|_| BEATS_JSON.to_string());
    let beat_lib = match BeatLibrary::load(&beats_json) {
        Ok(lib) => lib,
        Err(e) => {
            eprintln!("Failed to load beat library: {e}");
            std::process::exit(1);
        }
    };

    writeln!(out, "\n╔══════════════════════════════════════════════╗").unwrap();
    writeln!(out, "║         BECOME THE GOAT — v0.3 (TUI)        ║").unwrap();
    writeln!(out, "╚══════════════════════════════════════════════╝\n").unwrap();

    loop {
        writeln!(out, "  [N] New game   [L] Load   [Q] Quit").unwrap();
        write!(out, "  > ").unwrap();
        out.flush().unwrap();

        match lines.next() {
            Some(Ok(l)) => match l.trim().to_ascii_uppercase().as_str() {
                "Q" | "QUIT" => break,
                "N" | "NEW" => run_new_game(&mut lines, &mut out, &beat_lib),
                "L" | "LOAD" => {
                    let slots = list_slots(SAVE_DIR, NUM_SLOTS);
                    render_slot_picker(&mut out, &slots);
                    match prompt_slot_choice(&mut lines, &mut out) {
                        Some(slot) => {
                            if !slots[(slot - 1) as usize].occupied {
                                writeln!(out, "  Slot {slot} is empty.").unwrap();
                            } else {
                                match load_from_file(slot_path(SAVE_DIR, slot)) {
                                    Ok(data) => {
                                        writeln!(out, "  Save loaded.").unwrap();
                                        let world = WorldGenesis::generate(data.world_seed);
                                        let mut state = to_world_state(&data, &world);
                                        // pc_season_fixtures is "generated but
                                        // consistent" (like WorldGenesis itself) — not
                                        // persisted in the save, rebuilt here from the
                                        // loaded world_seed/season/club/division.
                                        state.pc_season_fixtures = build_season_orbit_fixtures(
                                            state.world_seed,
                                            state.season_number,
                                            state.pc_div_idx as usize,
                                            &world.leagues[state.pc_div_idx as usize].clubs,
                                            state.pc_club_idx as usize,
                                        );
                                        run_game_loop(
                                            &mut lines,
                                            &mut out,
                                            state,
                                            &beat_lib,
                                            PlayerTraits::default(),
                                            &world,
                                        );
                                    }
                                    Err(e) => writeln!(out, "  Load failed: {e}").unwrap(),
                                }
                            }
                        }
                        None => writeln!(out, "  Load cancelled.").unwrap(),
                    }
                }
                _ => writeln!(out, "  Unknown command.").unwrap(),
            },
            _ => break,
        }
    }
    writeln!(out, "Goodbye.").unwrap();
}

// ── New game flow ─────────────────────────────────────────────────────────────

fn run_new_game(
    lines: &mut impl Iterator<Item = io::Result<String>>,
    out: &mut impl Write,
    beat_lib: &BeatLibrary,
) {
    writeln!(out, "\n--- CREATE YOUR PLAYER ---").unwrap();
    let name = {
        let s = prompt_or_exit(lines, out, "Player name");
        if s.trim().is_empty() {
            "Unnamed Legend".to_string()
        } else {
            s
        }
    };

    writeln!(out, "\nPick a position:").unwrap();
    for (i, p) in PrimaryPosition::ALL.iter().enumerate() {
        writeln!(out, "  {}. {}", i + 1, p.name()).unwrap();
    }
    let primary_position = loop {
        let s = prompt_or_exit(
            lines,
            out,
            &format!("Choice [1-{}]", PrimaryPosition::ALL.len()),
        );
        if let Ok(n) = s.trim().parse::<usize>() {
            if (1..=PrimaryPosition::ALL.len()).contains(&n) {
                break PrimaryPosition::ALL[n - 1];
            }
        }
        writeln!(
            out,
            "  Please enter a number between 1 and {}.",
            PrimaryPosition::ALL.len()
        )
        .unwrap();
    };

    // The seed must be picked before nation/club selection: club/nation identity is
    // generated from `world_seed` (Design round 2, Doc A), so the world has to exist
    // before the player can browse it.
    let seed = loop {
        let s = prompt_or_exit(lines, out, "Seed (Enter = random)");
        if s.trim().is_empty() {
            use std::time::{SystemTime, UNIX_EPOCH};
            break SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(12345);
        }
        if let Ok(n) = s.trim().parse::<u64>() {
            break n;
        }
        writeln!(out, "  Enter a number or press Enter.").unwrap();
    };

    let world = WorldGenesis::generate(seed);

    // Pick nation
    writeln!(out, "\nPick a nation:").unwrap();
    for (i, n) in world.nations.iter().enumerate() {
        writeln!(out, "  {:2}. {:<16} (stature {})", i + 1, n.name, n.stature).unwrap();
    }
    let nation_idx: usize = loop {
        let s = prompt_or_exit(lines, out, &format!("Choice [1-{NUM_NATIONS}]"));
        if let Ok(n) = s.trim().parse::<usize>() {
            if (1..=NUM_NATIONS).contains(&n) {
                break n - 1;
            }
        }
        writeln!(out, "  Please enter a number between 1 and {NUM_NATIONS}.").unwrap();
    };

    // Pick tier (every nation has TIERS_PER_NATION tiers this round — uniform pyramids).
    writeln!(out, "\nPick a division:").unwrap();
    for tier_idx in 0..TIERS_PER_NATION {
        let league_id = nation_idx * TIERS_PER_NATION + tier_idx;
        writeln!(out, "  {}. {}", tier_idx + 1, world.leagues[league_id].name).unwrap();
    }
    let div_idx: usize = loop {
        let s = prompt_or_exit(lines, out, &format!("Choice [1-{TIERS_PER_NATION}]"));
        if let Ok(n) = s.trim().parse::<usize>() {
            if (1..=TIERS_PER_NATION).contains(&n) {
                break nation_idx * TIERS_PER_NATION + (n - 1);
            }
        }
        writeln!(
            out,
            "  Please enter a number between 1 and {TIERS_PER_NATION}."
        )
        .unwrap();
    };

    // Pick club from that division
    writeln!(out, "\nPick a club:").unwrap();
    let div_club_ids = world.leagues[div_idx].clubs.clone();
    for (i, &cid) in div_club_ids.iter().enumerate() {
        let club = &world.clubs[cid];
        let stars = "*".repeat(((club.strength as usize).div_ceil(20)).clamp(1, 5));
        writeln!(out, "  {:2}. {:<22} {}", i + 1, club.name, stars).unwrap();
    }
    let club_pos: usize = loop {
        let s = prompt_or_exit(lines, out, &format!("Choice [1-{}]", CLUBS_PER_DIV));
        if let Ok(n) = s.trim().parse::<usize>() {
            if (1..=CLUBS_PER_DIV).contains(&n) {
                break n - 1;
            }
        }
        writeln!(out, "  Invalid choice.").unwrap();
    };
    let club_id = div_club_ids[club_pos];

    let nationality = world.nation_name(world.clubs[club_id].nation).to_string();
    let club_name = world.clubs[club_id].name.clone();
    let choices = CreationChoices {
        name,
        primary_position,
        nationality: nationality.clone(),
        club: club_name,
    };

    let mut effective_seed = seed;
    loop {
        // Re-roll regenerates the *player* from a new identity seed but keeps the
        // already-chosen world/nation/club (the world itself only ever regenerates
        // from `world_seed`, which the player picked above and does not re-roll).
        let player = generate_player(effective_seed, &choices);
        // Lifestyle is not yet determined at creation time (every career starts
        // Balanced) — pass the default tier for the preview sheet.
        render_player_sheet(out, &player, &choices, effective_seed, 1);
        writeln!(out, "\n  [S] Start game   [R] Re-roll   [Q] Back").unwrap();
        write!(out, "  > ").unwrap();
        out.flush().unwrap();

        match lines.next() {
            Some(Ok(l)) => match l.trim().to_ascii_uppercase().as_str() {
                "S" | "START" => {
                    // Lifestyle is no longer picked here (bible §8.6) — it emerges over
                    // the career from training intensity, dev investment and sponsor
                    // choices. Every career starts Balanced (score 0).
                    let mut state = WorldState::new();
                    state = reduce(
                        state,
                        Intent::CreatePlayer {
                            seed: effective_seed,
                            choices,
                        },
                        &mut GoatRng::new(0),
                    );
                    state = reduce(
                        state,
                        Intent::InitWorld {
                            // The *world's* seed — must stay `seed` (what `world` was
                            // generated from above), not `effective_seed` (which a re-roll
                            // may have changed): the world was already displayed/chosen
                            // from `seed`, so persisting anything else would regenerate a
                            // different world on next load.
                            world_seed: seed,
                            pc_club_idx: club_id as u16,
                            pc_div_idx: div_idx as u8,
                            facilities_mult: world.clubs[club_id].facilities_mult(),
                            initial_table: Box::new([0u32; 100]),
                        },
                        &mut GoatRng::new(0),
                    );

                    // Seed peer cohort (Phase 9).
                    let peers = build_peer_cohort(effective_seed, &nationality);
                    state = reduce(state, Intent::InitPeers { peers }, &mut GoatRng::new(0));

                    // Roll hidden trait ceilings from creation seed (§A.3 aptitude).
                    // XOR with a domain tag so trait rolls don't alias attribute rolls.
                    let pc_traits = PlayerTraits::roll_from_seed(
                        effective_seed,
                        &mut GoatRng::new(effective_seed ^ 0x7261_6974_0000_0001),
                    );

                    let fixtures = build_season_orbit_fixtures(
                        seed,
                        1,
                        div_idx,
                        &world.leagues[div_idx].clubs,
                        club_id,
                    );
                    state = reduce(
                        state,
                        Intent::StartSeason { fixtures },
                        &mut GoatRng::new(0),
                    );
                    run_game_loop(lines, out, state, beat_lib, pc_traits, &world);
                    return;
                }
                "R" | "REROLL" | "RE-ROLL" => {
                    let mut rng = GoatRng::new(effective_seed);
                    effective_seed = rng.next_u64();
                }
                "Q" | "QUIT" => return,
                // Only an explicit Q/QUIT discards the in-progress character — a
                // blank Enter or any other stray input reprompts instead of
                // silently dropping back to the title screen.
                _ => writeln!(out, "  Please choose S, R, or Q.").unwrap(),
            },
            _ => return,
        }
    }
}

// ── Main game loop ────────────────────────────────────────────────────────────

fn run_game_loop(
    lines: &mut impl Iterator<Item = io::Result<String>>,
    out: &mut impl Write,
    mut state: WorldState,
    beat_lib: &BeatLibrary,
    pc_traits: PlayerTraits,
    world: &WorldGenesis,
) {
    // Season number for which the season-end pipeline (wage collection, awards,
    // legacy accrual, peer batch-tick, transfer window, contract renewal, retirement
    // suggestion) has already run. The end-of-season gate below re-enters every loop
    // iteration while `season_round` stays at the cap (e.g. after a `G` Legacy view,
    // which must be a read-only side trip) — this guard keeps the pipeline itself to
    // exactly one run per season boundary while still letting the post-pipeline menu
    // (and read-only views from it) redisplay freely.
    let mut season_end_done_for: Option<u32> = None;

    loop {
        let pc_id = match state.pc_player_id {
            Some(id) => id,
            None => return,
        };

        // Retirement screen (Phase 10).
        if state.pc_retired {
            let view = state.players.snapshot(pc_id);
            render_retirement_screen(out, &state, &view);
            return;
        }

        // Phase 10: hard retirement enforcement. `should_retire` already encodes the
        // full rule (bible §8.6) — nobody plays past RETIRE_AGE_HARD, full stop,
        // regardless of form. This is unconditional (no [C] to continue), unlike the
        // softer low-form suggestion further below, which still lets the player choose.
        if should_retire(&state) {
            state = reduce(state, Intent::Retire, &mut GoatRng::new(0));
            continue;
        }

        // End-of-season gate
        if state.season_number > 0 && state.season_round >= ROUNDS_PER_SEASON as u32 {
            // Run the season-end pipeline itself at most once per season boundary —
            // re-entering this branch (e.g. via a `G` Legacy read from the menu below)
            // must not re-collect wages, re-roll transfer/contract offers, or
            // double-credit career legacy stats.
            if season_end_done_for != Some(state.season_number) {
                let view = state.players.snapshot(pc_id);
                render_season_review(out, &state, &view, world);

                // Collect annual wage.
                state = reduce(state, Intent::CollectWage, &mut GoatRng::new(0));

                // Awards night + pundit reactions + legacy update.
                state = run_awards_and_pundits(out, state, &view, world);

                // Phase 9: batch-tick peers and check rival crystallisation.
                let season = state.season_number;
                state = reduce(
                    state,
                    Intent::BatchTickPeers { season },
                    &mut GoatRng::new(0),
                );
                if season >= 5 && state.pc_rival_idx.is_none() {
                    if let Some(rival_idx) = find_rival_candidate(&state) {
                        writeln!(out, "\n  *** RIVALRY CRYSTALLISES ***").unwrap();
                        writeln!(
                            out,
                            "  The media has finally named your generation's great debate: {} vs {}.",
                            view.name, state.pc_peers[rival_idx].name
                        )
                        .unwrap();
                        state = reduce(
                            state,
                            Intent::DeclareRival {
                                peer_idx: rival_idx,
                                season,
                            },
                            &mut GoatRng::new(0),
                        );
                    }
                }

                // Phase 8: transfer window and contract renewal.
                state = run_transfer_window(lines, out, state, &view, world);
                if state.pc_contract_seasons_left == 0 {
                    state = run_contract_negotiation(lines, out, state);
                }

                // Phase 10: auto-retirement suggestion.
                let view2 = state.players.snapshot(pc_id);
                let age_years = view2.age_weeks / 52;
                if age_years >= 35 && state.pc_form.to_int() < 40 {
                    writeln!(
                        out,
                        "\n  At {age_years} years old with form {}, the end may be near.",
                        state.pc_form.to_int()
                    )
                    .unwrap();
                    writeln!(out, "  [R] Retire now   [C] Continue playing").unwrap();
                    write!(out, "  > ").unwrap();
                    out.flush().unwrap();
                    if let Some(Ok(l)) = lines.next() {
                        if l.trim().eq_ignore_ascii_case("R") {
                            state = reduce(state, Intent::Retire, &mut GoatRng::new(0));
                            continue;
                        }
                    }
                }

                season_end_done_for = Some(state.season_number);
            }

            writeln!(
                out,
                "\n  [Y] Next season   [G] Legacy   [Z] Save & quit   [Q] Quit"
            )
            .unwrap();
            write!(out, "  > ").unwrap();
            out.flush().unwrap();
            match lines.next() {
                Some(Ok(l)) => match l.trim().to_ascii_uppercase().as_str() {
                    "Y" => {
                        let next_div_idx = state.pc_div_idx as usize;
                        let next_club_id = state.pc_club_idx as usize;
                        let fixtures = build_season_orbit_fixtures(
                            state.world_seed,
                            state.season_number + 1,
                            next_div_idx,
                            &world.leagues[next_div_idx].clubs,
                            next_club_id,
                        );
                        state = reduce(
                            state,
                            Intent::StartSeason { fixtures },
                            &mut GoatRng::new(0),
                        );
                        continue;
                    }
                    "G" => {
                        let ev = build_legacy_evidence(&state);
                        render_legacy_screen(out, &ev, &state);
                        continue;
                    }
                    "Z" => {
                        run_save(lines, out, &state);
                        return;
                    }
                    _ => return,
                },
                _ => return,
            }
        }

        let view = state.players.snapshot(pc_id);
        render_game_sheet(out, &view, &state);

        let has_season = state.season_number > 0;
        if has_season {
            writeln!(
                out,
                "\n  [W] Train  [F] Fast-fwd  [S] Routine  [P] Play match  [K] Skip match"
            )
            .unwrap();
            writeln!(
                out,
                "  [T] Table  [G] Legacy  [U] World  [V] Sheet  [Z] Save  [Q] Quit"
            )
            .unwrap();
        } else {
            writeln!(
                out,
                "\n  [W] Train  [F] Fast-fwd  [S] Routine  [U] World  [V] Sheet  [Q] Quit"
            )
            .unwrap();
        }
        write!(out, "  > ").unwrap();
        out.flush().unwrap();

        let week_rng_seed = (view.age_weeks as u64).wrapping_mul(6364136223846793005);

        match lines.next() {
            Some(Ok(l)) => match l.trim().to_ascii_uppercase().as_str() {
                "Q" | "QUIT" => return,
                "W" => {
                    // The reducer no-ops a second `W` in the same fixture round
                    // (`pc_week_training_done` gate) — snapshot the flag first so we
                    // can tell the player instead of silently redrawing the same state.
                    let already_trained = state.pc_week_training_done;
                    state = reduce(state, Intent::AdvanceWeek, &mut GoatRng::new(week_rng_seed));
                    if already_trained {
                        writeln!(
                            out,
                            "  You've already trained this week — Play or Skip this round's match to continue."
                        )
                        .unwrap();
                    } else {
                        display_events(out, &state.last_week_events);
                        display_flashpoints(out, &state.last_week_flashpoints);
                    }
                }
                "F" => {
                    let n = loop {
                        let s = prompt_or_exit(lines, out, "Advance how many weeks?");
                        if let Ok(n) = s.trim().parse::<u32>() {
                            break n;
                        }
                        writeln!(out, "  Enter a number.").unwrap();
                    };
                    state = reduce(
                        state,
                        Intent::AdvanceWeeks { n },
                        &mut GoatRng::new(week_rng_seed),
                    );
                    display_events(out, &state.last_week_events);
                    display_flashpoints(out, &state.last_week_flashpoints);
                }
                "S" => state = run_set_routine(lines, out, state),
                "P" if has_season => {
                    state = run_next_round(lines, out, state, true, beat_lib, pc_traits, world)
                }
                "K" if has_season => {
                    state = run_next_round(lines, out, state, false, beat_lib, pc_traits, world)
                }
                "T" if has_season => render_table(out, &state, world),
                "U" => render_world_screen(out, &state, world),
                "G" if has_season => {
                    let ev = build_legacy_evidence(&state);
                    render_legacy_screen(out, &ev, &state);
                }
                "Z" => {
                    run_save(lines, out, &state);
                }
                "V" => {
                    let choices = CreationChoices {
                        name: view.name.clone(),
                        primary_position: PrimaryPosition::from_u8(state.pc_position)
                            .unwrap_or(PrimaryPosition::ST),
                        nationality: state.pc_nationality.clone(),
                        club: state.pc_club.clone(),
                    };
                    render_player_sheet(out, &view, &choices, 0, state.pc_lifestyle);
                }
                _ => writeln!(out, "  Unrecognized command.").unwrap(),
            },
            _ => return,
        }
    }
}

// ── Season round ─────────────────────────────────────────────────────────────

fn run_next_round(
    lines: &mut impl Iterator<Item = io::Result<String>>,
    out: &mut impl Write,
    mut state: WorldState,
    play_interactive: bool,
    beat_lib: &BeatLibrary,
    pc_traits: PlayerTraits,
    world: &WorldGenesis,
) -> WorldState {
    let pc_id = match state.pc_player_id {
        Some(id) => id,
        None => return state,
    };
    if state.season_round >= ROUNDS_PER_SEASON as u32 {
        writeln!(out, "  Season is over. Start a new season.").unwrap();
        return state;
    }

    let round = state.season_round as usize;
    let season = state.season_number;
    let div_idx = state.pc_div_idx as usize;
    let pc_club_id = state.pc_club_idx as usize;
    let world_seed = state.world_seed;
    let div_clubs = world.leagues[div_idx].clubs.clone();

    let week_label = format_week_header(BASE_CAREER_YEAR + season - 1, round_to_week(round));
    writeln!(
        out,
        "\n--- ROUND {} / {} · {} ---",
        round + 1,
        ROUNDS_PER_SEASON,
        week_label
    )
    .unwrap();

    // Suspension check: serve ban, auto-skip. `reduce(ApplyRoundResult)` below
    // is the one that actually decrements the ban (bible AC-06) — this branch
    // only skips the PC's personal match participation.
    if state.pc_suspension_weeks > 0 {
        writeln!(
            out,
            "  You are SUSPENDED. {} match(es) remaining after this.",
            state.pc_suspension_weeks - 1
        )
        .unwrap();
        // Still need to sim other matches and advance the round.
        let all_fixtures = round_fixtures(world_seed, season, div_idx, &div_clubs, round);
        let sim_seed = world_seed ^ ((season as u64) << 32) ^ (round as u64) ^ 0xfeed;
        let mut sim_rng = GoatRng::new(sim_seed);
        let mut round_results: Vec<(u8, u8, u32, u32)> = Vec::new();
        for f in &all_fixtures {
            let (gf, ga) = sim_team_match(
                world.clubs[f.home].strength,
                world.clubs[f.away].strength,
                &mut sim_rng,
            );
            let h_pos = club_div_pos_in(&div_clubs, f.home) as u8;
            let a_pos = club_div_pos_in(&div_clubs, f.away) as u8;
            round_results.push((h_pos, a_pos, gf, ga));
        }
        return reduce(
            state,
            Intent::ApplyRoundResult {
                pc_goals: 0,
                pc_output: 0,
                pc_result: 0,
                round_results,
                rest_weeks: goat_world::rest_weeks_after_round(round),
                week_ends: goat_world::week_ends_after_round(round),
            },
            &mut GoatRng::new(0),
        );
    }

    // Find PC's fixture this round.
    let pc_fixture = fixture_for_round(world_seed, season, div_idx, &div_clubs, pc_club_id, round);

    let (pc_goals, pc_output, pc_result, match_result_opt) = match pc_fixture {
        Some(f) => {
            let is_home = f.home == pc_club_id;
            let opp_id = if is_home { f.away } else { f.home };
            let opp = &world.clubs[opp_id];
            let own_str = world.clubs[pc_club_id].strength;
            let view = state.players.snapshot(pc_id);

            let match_seed = world_seed ^ ((season as u64) << 32) ^ (round as u64) ^ 0xc0ffee;
            let mut match_rng = GoatRng::new(match_seed);

            // Ref personality: seeded from match seed (deterministic, not consuming match RNG).
            let ref_personality = {
                let mut rp_rng = GoatRng::new(match_seed ^ 0xBADCAFE);
                RefPersonality::from_rng(&mut rp_rng)
            };

            let make_setup = |view: &goat_core::player::PlayerView| MatchSetup {
                player_role: best_role_for_position(state.pc_position),
                player_attrs: view.current,
                player_familiarity: view.familiarity,
                own_strength: own_str,
                opp_strength: opp.strength,
                opp_name: opp.name.clone(),
                form: state.pc_form,
                player_aggression: view.current[goat_core::attrs::AttrId::Aggression as usize]
                    .to_int()
                    .clamp(1, 99) as u8,
                ref_personality,
                dirty_rep: state.pc_discipline_rep,
                player_traits: pc_traits,
            };

            let result = if play_interactive {
                let mut ms = start_match(beat_lib, make_setup(&view), &mut match_rng);
                while !ms.is_complete {
                    render_beat(out, &ms);
                    if ms.final_result.is_some() {
                        break;
                    }
                    let choice_idx = read_choice(
                        lines,
                        out,
                        ms.current_beat().map(|b| b.choices.len()).unwrap_or(1),
                    );
                    ms = advance_beat(ms, choice_idx, beat_lib, &mut match_rng);
                }
                ms.final_result.unwrap_or_else(|| {
                    auto_play_match(beat_lib, make_setup(&view), &mut GoatRng::new(match_seed))
                })
            } else {
                auto_play_match(beat_lib, make_setup(&view), &mut match_rng)
            };

            render_match_result(out, &result, &opp.name);

            // Show discipline outcome.
            if result.red_card {
                writeln!(out, "  🟥 RED CARD! You'll serve a suspension.").unwrap();
            } else if result.yellow_cards > 0 {
                writeln!(
                    out,
                    "  🟨 Yellow card ({} this season).",
                    state.pc_yellow_cards_season + result.yellow_cards as u32
                )
                .unwrap();
            }

            let pc_goals = result
                .moments
                .iter()
                .filter(|m| matches!(m.goal_event, Some(ScoreEvent::GoalFor)))
                .count() as u32;
            let pc_result: i8 = if result.goals_for > result.goals_against {
                1
            } else if result.goals_for < result.goals_against {
                -1
            } else {
                0
            };

            // Apply match effects (familiarity XP + energy cost).
            state = reduce(
                state,
                Intent::ApplyMatchResult {
                    familiarity_xp: result.familiarity_xp,
                    energy_cost: Fixed::from_int(25),
                    injury_weeks: None,
                },
                &mut GoatRng::new(0),
            );

            // Apply card result (updates suspensions and discipline rep).
            if result.yellow_cards > 0 || result.red_card {
                state = reduce(
                    state,
                    Intent::ApplyCardResult {
                        yellow_cards: result.yellow_cards as u32,
                        red_card: result.red_card,
                    },
                    &mut GoatRng::new(0),
                );
            } else {
                // Clean match: slowly recover dirty rep.
                state.pc_discipline_rep = (state.pc_discipline_rep - 1).max(0);
            }

            (pc_goals, result.player_output, pc_result, Some(result))
        }
        None => (0, 0, 0i8, None),
    };

    // Simulate all other matches in this round.
    let all_fixtures = round_fixtures(world_seed, season, div_idx, &div_clubs, round);
    let sim_seed = world_seed ^ ((season as u64) << 32) ^ (round as u64) ^ 0xfeed;
    let mut sim_rng = GoatRng::new(sim_seed);

    let mut round_results: Vec<(u8, u8, u32, u32)> = Vec::new();
    for f in &all_fixtures {
        let is_pc_match = f.home == pc_club_id || f.away == pc_club_id;
        let (gf, ga) = if is_pc_match {
            if let Some(ref r) = match_result_opt {
                if f.home == pc_club_id {
                    (r.goals_for, r.goals_against)
                } else {
                    (r.goals_against, r.goals_for)
                }
            } else {
                (0, 0)
            }
        } else {
            sim_team_match(
                world.clubs[f.home].strength,
                world.clubs[f.away].strength,
                &mut sim_rng,
            )
        };
        let h_pos = club_div_pos_in(&div_clubs, f.home) as u8;
        let a_pos = club_div_pos_in(&div_clubs, f.away) as u8;
        round_results.push((h_pos, a_pos, gf, ga));
    }

    // Show this round's results.
    writeln!(out, "\n  Round {} results:", round + 1).unwrap();
    for f in &all_fixtures {
        let is_pc = f.home == pc_club_id || f.away == pc_club_id;
        if is_pc {
            continue; // already shown by render_match_result
        }
        let (gf, ga) = round_results
            .iter()
            .find(|&&(h, a, _, _)| {
                h == club_div_pos_in(&div_clubs, f.home) as u8
                    && a == club_div_pos_in(&div_clubs, f.away) as u8
            })
            .map(|&(_, _, gf, ga)| (gf, ga))
            .unwrap_or((0, 0));
        writeln!(
            out,
            "  {:20} {:1}–{:1}  {}",
            world.clubs[f.home].name, gf, ga, world.clubs[f.away].name
        )
        .unwrap();
    }

    state = reduce(
        state,
        Intent::ApplyRoundResult {
            pc_goals,
            pc_output,
            pc_result,
            round_results,
            rest_weeks: goat_world::rest_weeks_after_round(round),
            week_ends: goat_world::week_ends_after_round(round),
        },
        &mut GoatRng::new(0),
    );

    state
}

fn club_div_pos_in(div_clubs: &[usize], club_id: usize) -> usize {
    div_clubs
        .iter()
        .position(|&c| c == club_id)
        .expect("club in division")
}

/// Family-representative match role for a 0..7 PrimaryPosition discriminant
/// (0=ST … 7=CB). The pre-revision 0..2 mapping would deploy a Striker (0) as
/// a CentreBack.
fn best_role_for_position(pc_position: u8) -> RoleId {
    use goat_core::{positions::PrimaryPosition, roles::PositionFamily};
    match PrimaryPosition::from_u8(pc_position)
        .unwrap_or(PrimaryPosition::ST)
        .family()
    {
        PositionFamily::Defender => RoleId::CentreBack,
        PositionFamily::Midfielder => RoleId::CentralMid,
        PositionFamily::Forward => RoleId::CompleteForward,
    }
}

// ── Table display ─────────────────────────────────────────────────────────────

fn render_table(out: &mut impl Write, state: &WorldState, world: &WorldGenesis) {
    let div_idx = state.pc_div_idx as usize;
    let div_club_ids = &world.leagues[div_idx].clubs;
    let table = Table::from_raw(&state.table_raw, div_club_ids);
    let sorted = table.sorted();
    let pc_club_id = state.pc_club_idx as usize;

    writeln!(
        out,
        "\n  {} — Round {}",
        world.leagues[div_idx].name, state.season_round
    )
    .unwrap();
    writeln!(
        out,
        "  {:<22} {:>3} {:>3} {:>3} {:>3} {:>4} {:>4} {:>4}",
        "Club", "Pl", "W", "D", "L", "GF", "GA", "Pts"
    )
    .unwrap();
    writeln!(out, "  {}", "─".repeat(52)).unwrap();
    for e in &sorted {
        let marker = if e.club_id == pc_club_id { "►" } else { " " };
        writeln!(
            out,
            " {}{:<22} {:>3} {:>3} {:>3} {:>3} {:>4} {:>4} {:>4}",
            marker,
            world.clubs[e.club_id].name,
            e.played(),
            e.w,
            e.d,
            e.l,
            e.gf,
            e.ga,
            e.points()
        )
        .unwrap();
    }
}

// ── Season review ─────────────────────────────────────────────────────────────

fn render_season_review(
    out: &mut impl Write,
    state: &WorldState,
    view: &PlayerView,
    world: &WorldGenesis,
) {
    writeln!(out, "\n╔══════════════════════════════════════════════╗").unwrap();
    writeln!(
        out,
        "║  SEASON {} REVIEW                             ║",
        state.season_number
    )
    .unwrap();
    writeln!(out, "╠══════════════════════════════════════════════╣").unwrap();

    let div_idx = state.pc_div_idx as usize;
    let div_club_ids = &world.leagues[div_idx].clubs;
    let table = Table::from_raw(&state.table_raw, div_club_ids);
    let pos = table.position_of(state.pc_club_idx as usize);

    writeln!(
        out,
        "║  {}: finished {}th in {}",
        state.pc_club, pos, world.leagues[div_idx].name
    )
    .unwrap();
    writeln!(
        out,
        "║  Your season: {} matches  {} goals  Output avg: {}",
        state.pc_season_matches,
        state.pc_season_goals,
        if state.pc_season_matches > 0 {
            state.pc_season_output / state.pc_season_matches as i32
        } else {
            0
        }
    )
    .unwrap();
    writeln!(
        out,
        "║  Form: {}  Age: {}y{}w",
        state.pc_form.to_int(),
        view.age_weeks / 52,
        view.age_weeks % 52
    )
    .unwrap();
    writeln!(out, "╠══════════════════════════════════════════════╣").unwrap();
    writeln!(out, "║  FINAL TABLE                                 ║").unwrap();

    let sorted = table.sorted();
    let pc_club_id = state.pc_club_idx as usize;
    for (rank, e) in sorted.iter().enumerate() {
        let marker = if e.club_id == pc_club_id { "►" } else { " " };
        writeln!(
            out,
            "║ {}{:2}. {:<20} {:2}pts {:2}GD",
            marker,
            rank + 1,
            world.clubs[e.club_id].name,
            e.points(),
            e.goal_diff()
        )
        .unwrap();
    }
    writeln!(out, "╚══════════════════════════════════════════════╝").unwrap();
}

// ── Legacy + Pantheon ─────────────────────────────────────────────────────────

fn build_legacy_evidence(state: &WorldState) -> LegacyEvidence {
    LegacyEvidence {
        career_goals: state.pc_career_goals,
        career_matches: state.pc_career_matches,
        career_output_sum: state.pc_career_output_sum,
        best_season_avg_output: state.pc_best_season_avg_output,
        seasons_played: state.pc_seasons_played,
        decisive_moments: state.pc_decisive_moments,
        player_of_year_wins: state.pc_player_of_year_wins,
        league_titles: state.pc_league_titles,
        clubs_served: state.pc_clubs_served,
        longest_club_tenure: state.pc_longest_club_tenure,
        career_standout_matches: state.pc_career_standout_matches,
        career_best_ovr: state.pc_career_best_ovr,
        career_transfer_requests: state.pc_career_transfer_requests,
        career_caps: state.pc_career_caps,
        career_international_goals: state.pc_career_international_goals,
        career_world_cups_won: state.pc_career_world_cups_won,
        career_continental_championships_won: state.pc_career_continental_championships_won,
    }
}

fn render_legacy_screen(out: &mut impl Write, ev: &LegacyEvidence, state: &WorldState) {
    let pc_name = &state.pc_club; // placeholder — use player name
    let axes = compute_axes(ev);
    let rep = compute_reputation(
        state.pc_sporting_rep,
        state.pc_discipline_rep,
        state.pc_club_fan_rep,
    );
    let rankings = all_rankings(ev, &axes);

    writeln!(out, "\n╔══════════════════════════════════════════════╗").unwrap();
    writeln!(
        out,
        "║  LEGACY — {}   {}",
        rep.label(),
        " ".repeat(25_usize.saturating_sub(rep.label().len()))
    )
    .unwrap();
    writeln!(out, "╠══════════════════════════════════════════════╣").unwrap();
    writeln!(out, "║  CAREER EVIDENCE                             ║").unwrap();
    writeln!(
        out,
        "║  Goals: {:4}   Matches: {:4}   Seasons: {:2}   ║",
        ev.career_goals, ev.career_matches, ev.seasons_played
    )
    .unwrap();
    writeln!(
        out,
        "║  League titles: {:2}   POTY wins: {:2}          ║",
        ev.league_titles, ev.player_of_year_wins
    )
    .unwrap();
    writeln!(
        out,
        "║  Decisive moments: {:2}   Clubs: {:2}           ║",
        ev.decisive_moments, ev.clubs_served
    )
    .unwrap();
    writeln!(out, "╠══════════════════════════════════════════════╣").unwrap();
    writeln!(out, "║  LEGACY AXES                                 ║").unwrap();
    let arr = axes.as_array();
    for (i, &name) in AXIS_NAMES.iter().enumerate() {
        let val = arr[i].to_int();
        let bar = "█".repeat((val as usize) / 10).to_string()
            + &"░".repeat(10 - (val as usize / 10).min(10));
        writeln!(out, "║  {:<14} {:>3}  {}  ║", name, val, bar).unwrap();
    }
    writeln!(out, "╠══════════════════════════════════════════════╣").unwrap();
    writeln!(out, "║  REPUTATION                                  ║").unwrap();
    writeln!(
        out,
        "║  Sporting:{:3}  Character:{:3}  Club Fan:{:3}  ║",
        rep.sporting.to_int(),
        rep.character.to_int(),
        rep.club_fan.to_int()
    )
    .unwrap();
    writeln!(out, "╠══════════════════════════════════════════════╣").unwrap();
    writeln!(out, "║  PANTHEON RANKINGS                           ║").unwrap();
    for (i, (score, rank, total)) in rankings.iter().enumerate() {
        writeln!(
            out,
            "║  {:<28} {:>3}/100  #{}/{}  ║",
            SCHOOLS[i].name,
            score.to_int(),
            rank,
            total
        )
        .unwrap();
    }
    writeln!(out, "╚══════════════════════════════════════════════╝").unwrap();
    let _ = pc_name;

    // Career totals/Pantheon scores are batched at season end (docs/CALENDAR.md's
    // season-boundary pipeline) — mid-season they're intentionally frozen, but
    // nothing told the player that (playtest finding).
    if state.season_round < ROUNDS_PER_SEASON as u32 {
        writeln!(
            out,
            "  (Career totals and Pantheon scores update at season end — Round {}/{} so far.)",
            state.season_round, ROUNDS_PER_SEASON
        )
        .unwrap();
    }
}

fn run_awards_and_pundits(
    out: &mut impl Write,
    mut state: WorldState,
    view: &PlayerView,
    world: &WorldGenesis,
) -> WorldState {
    let season = state.season_number;
    let world_seed = state.world_seed;
    let pc_name = &view.name;
    let pc_club = state.pc_club.clone();

    let season_avg = if state.pc_season_matches > 0 {
        state.pc_season_output / state.pc_season_matches as i32
    } else {
        0
    };

    // Compute awards.
    let poty = compute_player_of_year(
        pc_name,
        season_avg,
        state.pc_season_goals,
        season,
        world_seed,
    );
    let boot = compute_golden_boot(pc_name, state.pc_season_goals, season, world_seed);

    // Render awards night.
    writeln!(out, "\n╔══════════════════════════════════════════════╗").unwrap();
    writeln!(
        out,
        "║  AWARDS NIGHT — Season {}                     ║",
        season
    )
    .unwrap();
    writeln!(out, "╠══════════════════════════════════════════════╣").unwrap();

    for award in [&poty, &boot] {
        let marker = if award.pc_won { "★ WON" } else { "  " };
        writeln!(out, "║  {}  {:<28}         ║", marker, award.award_name).unwrap();
        writeln!(out, "║    Winner: {:<34}║", award.winner_name).unwrap();
        if let Some(ref ru) = award.runner_up_name {
            writeln!(out, "║    Runner-up: {:<31}║", ru).unwrap();
        }
    }
    writeln!(out, "╚══════════════════════════════════════════════╝").unwrap();

    let player_of_year_won = poty.pc_won;

    // Compute finish position for legacy/rep update.
    let div_idx = state.pc_div_idx as usize;
    let div_clubs = &world.leagues[div_idx].clubs;
    let table = goat_world::Table::from_raw(&state.table_raw, div_clubs);
    let finish_pos = table.position_of(state.pc_club_idx as usize) as u32;
    let won_title = finish_pos == 1;

    // Update reputation.
    let new_sporting = update_sporting_rep(state.pc_sporting_rep, season_avg, finish_pos);
    let new_club_fan = update_club_fan_rep(
        state.pc_club_fan_rep,
        state.pc_longest_club_tenure,
        season_avg,
        state.pc_clubs_served,
    );

    // Capture before moving state into reduce.
    let s_goals = state.pc_season_goals;
    let s_matches = state.pc_season_matches;
    let s_output = state.pc_season_output;
    let s_standout_matches = state.pc_season_standout_matches;
    let s_transfer_requests = state.pc_season_transfer_requests;

    // Update legacy evidence via intent.
    state = reduce(
        state,
        Intent::ApplySeasonEndLegacy {
            season_goals: s_goals,
            season_matches: s_matches,
            season_output_sum: s_output,
            won_title,
            player_of_year: player_of_year_won,
            finish_position: finish_pos,
            decisive_moments: 0,
            new_sporting_rep: new_sporting,
            new_club_fan_rep: new_club_fan,
            season_standout_matches: s_standout_matches,
            season_transfer_requests: s_transfer_requests,
            // National-team call-ups (Design round 2, Doc B §B.3) aren't wired into the
            // TUI yet — the international-break window is still flavor-text-only (see
            // the calendar-event renderer), so nothing accrues caps this season.
            season_caps: 0,
            season_international_goals: 0,
            // National-team tournaments (Design round 4, Slice 4) are simulated by
            // `goat-world::national_tournament` but not yet dispatched from the live TUI
            // loop (no day-driven "a tournament fixture is due" dispatcher exists yet,
            // same deferred-to-Slice-5 gap as the domestic-cup/continental club
            // competitions) -- so nothing accrues here yet either.
            season_world_cups_won: 0,
            season_continental_championships_won: 0,
        },
        &mut GoatRng::new(0),
    );

    // Pundit reactions.
    let ev = build_legacy_evidence(&state);
    let axes = compute_axes(&ev);
    let rankings = all_rankings(&ev, &axes);

    writeln!(out, "\n  --- THE PUNDITS ---").unwrap();
    for pundit in PUNDITS.iter() {
        let (_, rank, _) = rankings[pundit.school_idx];
        let ctx = PunditContext::Season {
            goals: state.pc_season_goals,
            matches: state.pc_season_matches,
            avg_output: season_avg,
            finish_pos,
        };
        let comment = pundit_comment(pundit, &axes, &ctx, pc_name, &pc_club, season);
        writeln!(out, "\n  {} ({}):", pundit.name, pundit.role).unwrap();
        // Word-wrap at ~60 chars
        let words: Vec<&str> = comment.split_whitespace().collect();
        let mut line = String::from("  \"");
        for word in words {
            if line.len() + word.len() > 62 {
                writeln!(out, "{}\"", line).unwrap();
                line = String::from("   ");
            }
            line.push(' ');
            line.push_str(word);
        }
        if !line.trim().is_empty() {
            writeln!(out, "{}\"", line).unwrap();
        }
        let _ = rank;
    }

    state
}

// ── Phase 8: Transfer window + contract ──────────────────────────────────────

fn generate_transfer_offers(
    state: &WorldState,
    view: &PlayerView,
    world: &WorldGenesis,
) -> Vec<(usize, u8, i64, u32)> {
    // Returns Vec of (club_idx, div_idx, wage_offer, length)
    use goat_rng::RngSource;
    let form = state.pc_form.to_int();
    let age = view.age_weeks / 52;
    // Only generate offers if form > 55 and age < 34
    if form < 55 || age >= 34 {
        return Vec::new();
    }
    let mut rng = GoatRng::new(state.world_seed ^ ((state.season_number as u64) << 32) ^ 0xA11BEEF);
    let n_offers = rng.next_range_u64(0, 2) as usize; // 0-2 offers
    let mut offers = Vec::new();
    let num_leagues = world.leagues.len() as u64;
    for _ in 0..n_offers {
        // Pick a random club from a different division.
        let target_div = ((state.pc_div_idx as u64 + 1 + rng.next_range_u64(0, num_leagues - 2))
            % num_leagues) as usize;
        let league_clubs = &world.leagues[target_div].clubs;
        let club_pos = rng.next_range_u64(0, league_clubs.len() as u64 - 1) as usize;
        let club_id = league_clubs[club_pos];
        let target_strength = world.clubs[club_id].strength;
        let wage_offer =
            state.pc_wage_annual + (target_strength as i64 * 2) + rng.next_range_u64(0, 50) as i64;
        let length = 2 + rng.next_range_u64(0, 2) as u32;
        if club_id != state.pc_club_idx as usize {
            offers.push((club_id, target_div as u8, wage_offer, length));
        }
    }
    offers
}

fn run_transfer_window(
    lines: &mut impl Iterator<Item = io::Result<String>>,
    out: &mut impl Write,
    mut state: WorldState,
    view: &PlayerView,
    world: &WorldGenesis,
) -> WorldState {
    let offers = generate_transfer_offers(&state, view, world);
    if offers.is_empty() {
        // Check if player wants to agitate
        if state.pc_power_ladder > 0 || state.pc_contract_seasons_left == 0 {
            writeln!(out, "\n  No transfer offers this window.").unwrap();
        }
        return state;
    }

    writeln!(out, "\n╔══════════════════════════════════════════════╗").unwrap();
    writeln!(out, "║  TRANSFER WINDOW                             ║").unwrap();
    writeln!(out, "╠══════════════════════════════════════════════╣").unwrap();
    for (i, &(club_id, div_idx, wage, length)) in offers.iter().enumerate() {
        let stars = "*".repeat((world.clubs[club_id].strength as usize / 20).clamp(1, 5));
        writeln!(
            out,
            "║  {}. {:22} ({}) £{}/yr {}yr  ║",
            i + 1,
            world.clubs[club_id].name,
            world.leagues[div_idx as usize].name,
            wage,
            length
        )
        .unwrap();
        let _ = stars;
    }
    writeln!(out, "╚══════════════════════════════════════════════╝").unwrap();
    writeln!(
        out,
        "  [1-{}] Accept offer  [N] Stay  [A] Agitate",
        offers.len()
    )
    .unwrap();
    write!(out, "  > ").unwrap();
    out.flush().unwrap();

    if let Some(Ok(l)) = lines.next() {
        let l = l.trim().to_ascii_uppercase();
        if l == "A" {
            writeln!(out, "  You agitate for a move. Character rep takes a hit.").unwrap();
            state = reduce(state, Intent::AgitateForTransfer, &mut GoatRng::new(0));
        } else if let Ok(n) = l.parse::<usize>() {
            if n >= 1 && n <= offers.len() {
                let (club_id, div_idx, wage, length) = offers[n - 1];
                let club = &world.clubs[club_id];
                writeln!(
                    out,
                    "  TRANSFER COMPLETE: {} → {}",
                    state.pc_club, club.name
                )
                .unwrap();
                let fee_bonus = (world.clubs[state.pc_club_idx as usize].strength as i64) * 3;
                state = reduce(
                    state,
                    Intent::ExecuteTransfer {
                        to_club_idx: club_id as u16,
                        to_div_idx: div_idx,
                        new_wage: wage,
                        new_length: length,
                        new_club_name: club.name.clone(),
                        facilities_mult: club.facilities_mult(),
                        fee_bonus,
                    },
                    &mut GoatRng::new(0),
                );
            }
        }
    }
    state
}

fn run_contract_negotiation(
    lines: &mut impl Iterator<Item = io::Result<String>>,
    out: &mut impl Write,
    mut state: WorldState,
) -> WorldState {
    let new_wage = state.pc_wage_annual + (state.pc_form.to_int() as i64 / 10) * 5;
    let new_length = 2u32;

    writeln!(out, "\n╔══════════════════════════════════════════════╗").unwrap();
    writeln!(out, "║  CONTRACT RENEWAL — {}  ║", state.pc_club).unwrap();
    writeln!(out, "╠══════════════════════════════════════════════╣").unwrap();
    writeln!(
        out,
        "║  Offer: £{}/yr × {} seasons               ║",
        new_wage, new_length
    )
    .unwrap();
    writeln!(out, "╚══════════════════════════════════════════════╝").unwrap();
    writeln!(
        out,
        "  [Y] Accept   [N] Decline (become free agent next window)"
    )
    .unwrap();
    write!(out, "  > ").unwrap();
    out.flush().unwrap();

    if let Some(Ok(l)) = lines.next() {
        if l.trim().eq_ignore_ascii_case("Y") {
            let club_idx = state.pc_club_idx;
            state = reduce(
                state,
                Intent::AcceptContract {
                    new_wage,
                    new_length,
                    new_club_idx: club_idx,
                },
                &mut GoatRng::new(0),
            );
            writeln!(out, "  Contract renewed.").unwrap();
        } else {
            writeln!(out, "  No renewal signed. You're out of contract.").unwrap();
            // Contract stays at 0 — player is "free" but stays until a move happens
        }
    }
    state
}

// ── Phase 9: Peer cohort helpers ──────────────────────────────────────────────

const PEER_NAMES_BY_NATION: &[(&str, &[&str])] = &[
    (
        "England",
        &[
            "J. Smith",
            "T. Williams",
            "O. Brown",
            "L. Taylor",
            "E. Jones",
            "C. Davis",
            "M. Wilson",
            "A. Moore",
        ],
    ),
    (
        "Brazil",
        &[
            "R. Silva",
            "G. Santos",
            "F. Oliveira",
            "M. Souza",
            "L. Costa",
            "P. Ferreira",
            "A. Alves",
            "D. Lima",
        ],
    ),
];

fn build_peer_cohort(world_seed: u64, nationality: &str) -> Vec<goat_core::state::PeerState> {
    use goat_rng::RngSource;
    let names = PEER_NAMES_BY_NATION
        .iter()
        .find(|(n, _)| *n == nationality)
        .map(|(_, names)| *names)
        .unwrap_or(PEER_NAMES_BY_NATION[0].1);

    let mut rng = GoatRng::new(world_seed ^ 0xC0_CA_FE_BE_EF_u64);
    (0..8)
        .map(|i| goat_core::state::PeerState {
            seed: rng.next_u64(),
            name: names[i % names.len()].to_string(),
            nationality: nationality.to_string(),
            career_goals: 0,
            career_matches: 0,
            avg_output: 0,
            titles: 0,
        })
        .collect()
}

fn find_rival_candidate(state: &WorldState) -> Option<usize> {
    let pc_avg = if state.pc_career_matches > 0 {
        (state.pc_career_output_sum / state.pc_career_matches as i64) as u8
    } else {
        0
    };
    state
        .pc_peers
        .iter()
        .enumerate()
        .find(|(_, peer)| {
            peer.career_matches >= 80 && (peer.avg_output as i32 - pc_avg as i32).abs() <= 8
        })
        .map(|(i, _)| i)
}

// ── Phase 10: Retirement ──────────────────────────────────────────────────────

fn render_retirement_screen(out: &mut impl Write, state: &WorldState, view: &PlayerView) {
    let age = view.age_weeks / 52;
    let ev = build_legacy_evidence(state);
    let mut axes = compute_axes(&ev);
    // Phase 10: the off-pitch career (fame, character, money) feeds the Icon axis so the
    // schools' verdict reflects the whole person, not just the football.
    axes.icon = goat_meta::icon_axis(
        state.pc_marketability,
        state.pc_character_rep,
        state.pc_sponsor_tier,
        state.pc_bankrupt,
    );
    let rankings = all_rankings(&ev, &axes);
    let rep = compute_reputation(
        state.pc_sporting_rep,
        state.pc_discipline_rep,
        state.pc_club_fan_rep,
    );

    writeln!(out, "\n╔══════════════════════════════════════════════╗").unwrap();
    writeln!(
        out,
        "║  CAREER OVER — {} retires at {}            ║",
        view.name, age
    )
    .unwrap();
    writeln!(out, "╠══════════════════════════════════════════════╣").unwrap();
    writeln!(
        out,
        "║  {} seasons  |  {} goals  |  {} matches      ║",
        ev.seasons_played, ev.career_goals, ev.career_matches
    )
    .unwrap();
    writeln!(
        out,
        "║  {} league titles  |  {} Player of the Year  ║",
        ev.league_titles, ev.player_of_year_wins
    )
    .unwrap();
    writeln!(
        out,
        "║  Reputation: {}                              ║",
        rep.label()
    )
    .unwrap();
    writeln!(
        out,
        "║  Savings: £{}k                               ║",
        state.pc_savings
    )
    .unwrap();
    writeln!(out, "╠══════════════════════════════════════════════╣").unwrap();
    writeln!(out, "║  THE SCHOOLS DELIVER THEIR FINAL VERDICT     ║").unwrap();
    writeln!(out, "╠══════════════════════════════════════════════╣").unwrap();
    for (i, &(score, rank, total)) in rankings.iter().enumerate() {
        writeln!(
            out,
            "║  {:<28}  {:>3}/100  #{}/{}  ║",
            SCHOOLS[i].name,
            score.to_int(),
            rank,
            total
        )
        .unwrap();
    }
    writeln!(out, "╠══════════════════════════════════════════════╣").unwrap();
    if let Some(rival_idx) = state.pc_rival_idx {
        let rival = &state.pc_peers[rival_idx];
        writeln!(out, "║  Your generation's debate: you vs {}  ║", rival.name).unwrap();
        writeln!(out, "║  The argument will outlast both of you.      ║").unwrap();
    } else {
        writeln!(out, "║  No rival emerged. You reigned alone.        ║").unwrap();
        writeln!(out, "║  Some schools will apply the weak-era note.  ║").unwrap();
    }
    writeln!(out, "╚══════════════════════════════════════════════╝").unwrap();
    writeln!(
        out,
        "\n  Your career has entered the pantheon. The debate is the ending."
    )
    .unwrap();
}

// ── Save / Load ───────────────────────────────────────────────────────────────

/// One line per slot: `"  [3] Alex Turner — S4, age 24"` when occupied,
/// `"  [3] <empty>"` when not.
fn render_slot_picker(out: &mut impl Write, slots: &[SaveSlotSummary]) {
    for s in slots {
        if s.occupied {
            writeln!(
                out,
                "  [{}] {} — S{}, age {}",
                s.slot,
                s.pc_name,
                s.season_number,
                s.pc_age_weeks / 52
            )
            .unwrap();
        } else {
            writeln!(out, "  [{}] <empty>", s.slot).unwrap();
        }
    }
}

/// Reads a single digit `1..=NUM_SLOTS`. `Q`/blank cancels (`None`); anything else
/// reprompts instead of silently dropping back to the caller.
fn prompt_slot_choice(
    lines: &mut impl Iterator<Item = io::Result<String>>,
    out: &mut impl Write,
) -> Option<u8> {
    loop {
        let s = prompt_or_exit(
            lines,
            out,
            &format!("Slot (1-{NUM_SLOTS}, blank/Q to cancel)"),
        );
        let t = s.trim();
        if t.is_empty() || t.eq_ignore_ascii_case("q") {
            return None;
        }
        match t.parse::<u8>() {
            Ok(n) if (1..=NUM_SLOTS).contains(&n) => return Some(n),
            _ => writeln!(
                out,
                "  Please enter a slot number 1-{NUM_SLOTS}, or Q to cancel."
            )
            .unwrap(),
        }
    }
}

fn run_save(
    lines: &mut impl Iterator<Item = io::Result<String>>,
    out: &mut impl Write,
    state: &WorldState,
) {
    let Some(pc_id) = state.pc_player_id else {
        return;
    };

    let slots = list_slots(SAVE_DIR, NUM_SLOTS);
    render_slot_picker(out, &slots);
    let Some(slot) = prompt_slot_choice(lines, out) else {
        writeln!(out, "  Save cancelled.").unwrap();
        return;
    };

    let summary = &slots[(slot - 1) as usize];
    if summary.occupied {
        write!(
            out,
            "  Slot {slot} has a save ({}, S{}). Overwrite? [Y/N] ",
            summary.pc_name, summary.season_number
        )
        .unwrap();
        out.flush().unwrap();
        match lines.next() {
            Some(Ok(l)) if l.trim().eq_ignore_ascii_case("y") => {}
            _ => {
                writeln!(out, "  Save cancelled.").unwrap();
                return;
            }
        }
    }

    let view = state.players.snapshot(pc_id);
    let data = from_world_state(state, &view);
    match save_to_file(&data, slot_path(SAVE_DIR, slot)) {
        Ok(()) => writeln!(out, "  Saved to slot {slot}.").unwrap(),
        Err(e) => writeln!(out, "  Save failed: {e}").unwrap(),
    }
}

// ── Set routine ───────────────────────────────────────────────────────────────

fn run_set_routine(
    lines: &mut impl Iterator<Item = io::Result<String>>,
    out: &mut impl Write,
    state: WorldState,
) -> WorldState {
    writeln!(out, "\n--- SET TRAINING ROUTINE ---").unwrap();
    writeln!(
        out,
        "Pick up to 4 attributes to focus on (by number, comma-separated):"
    )
    .unwrap();
    let attr_list = [
        AttrId::Finishing,
        AttrId::LongShots,
        AttrId::ShotPower,
        AttrId::ShortPassing,
        AttrId::Vision,
        AttrId::Crossing,
        AttrId::CloseControl,
        AttrId::BallControl,
        AttrId::Agility,
        AttrId::StandingTackle,
        AttrId::Marking,
        AttrId::Interceptions,
        AttrId::Strength,
        AttrId::Stamina,
        AttrId::Composure,
        AttrId::Acceleration,
        AttrId::SprintSpeed,
        AttrId::Heading,
    ];
    for (i, a) in attr_list.iter().enumerate() {
        write!(out, "  {:2}. {:<18}", i + 1, ATTR_NAMES[*a as usize]).unwrap();
        if (i + 1) % 3 == 0 {
            writeln!(out).unwrap();
        }
    }
    writeln!(out).unwrap();

    let focus_attrs = {
        let s = prompt_or_exit(lines, out, "Attr numbers (e.g. 1,7,9) or Enter to clear");
        if s.trim().is_empty() {
            vec![]
        } else {
            s.split(',')
                .filter_map(|t| t.trim().parse::<usize>().ok())
                .filter(|&n| n >= 1 && n <= attr_list.len())
                .take(4)
                .map(|n| attr_list[n - 1])
                .collect()
        }
    };

    writeln!(out, "Intensity:  1. Low   2. Medium (default)   3. High").unwrap();
    let intensity_input = prompt_or_exit(lines, out, "Choice [1-3]");
    let intensity = match intensity_input.trim() {
        "1" => Intensity::Low,
        "3" => Intensity::High,
        _ => Intensity::Medium,
    };

    let routine = Routine {
        focus_attrs,
        intensity,
    };
    writeln!(
        out,
        "  Routine set: {} intensity, {} focus attrs.",
        routine.intensity.name(),
        routine.focus_attrs.len()
    )
    .unwrap();
    reduce(state, Intent::SetRoutine { routine }, &mut GoatRng::new(0))
}

// ── Rendering ─────────────────────────────────────────────────────────────────

/// Interior width of the standard box (matching the `╔══...══╗` borders, which
/// are 48 columns total: 1 border char + 46 interior + 1 border char).
const BOX_WIDTH: usize = 46;

/// Truncate `text` to at most `max_chars` *characters* (not bytes — multi-byte
/// glyphs like emoji or accented letters must count as one), breaking at the
/// last word boundary within budget rather than mid-word, and appending `…`
/// when truncation actually occurred.
fn truncate_ellipsis(text: &str, max_chars: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars {
        return text.to_string();
    }
    let budget = max_chars.saturating_sub(1).max(1);
    let mut cut = budget;
    while cut > 0 && !chars[cut - 1].is_whitespace() {
        cut -= 1;
    }
    if cut == 0 {
        cut = budget; // no word boundary within budget — hard cut
    }
    let mut truncated: String = chars[..cut].iter().collect();
    while truncated.ends_with(' ') {
        truncated.pop();
    }
    format!("{truncated}…")
}

/// Write one line of box content, clamping/padding it to the box's fixed
/// interior width (`BOX_WIDTH`) so every line closes with a matching `║` —
/// truncating with `…` at a word boundary when content overflows, padding
/// with spaces when it's short. Counts by `chars()`, not byte length, so
/// emoji/accents don't throw off alignment.
fn box_line(out: &mut impl Write, content: &str) {
    let clamped = truncate_ellipsis(content, BOX_WIDTH);
    let pad = BOX_WIDTH.saturating_sub(clamped.chars().count());
    writeln!(out, "║{clamped}{}║", " ".repeat(pad)).unwrap();
}

/// Wrap pre-formatted `items` across as many box lines as needed (starting
/// with `prefix`, continuation lines indented two spaces) so that a variable
/// number of items — e.g. up to 4 training-focus attrs — never gets silently
/// dropped just to fit one line, the way a hard `.take(3)` would.
fn box_lines_wrapped(out: &mut impl Write, prefix: &str, items: &[String], sep: &str) {
    let mut line = prefix.to_string();
    let mut any_on_line = false;
    for item in items {
        let would_be = line.chars().count()
            + if any_on_line { sep.chars().count() } else { 0 }
            + item.chars().count();
        if any_on_line && would_be > BOX_WIDTH {
            box_line(out, &line);
            line = String::from("  ");
            any_on_line = false;
        }
        if any_on_line {
            line.push_str(sep);
        }
        line.push_str(item);
        any_on_line = true;
    }
    box_line(out, &line);
}

fn render_game_sheet(out: &mut impl Write, view: &PlayerView, state: &WorldState) {
    let age_y = view.age_weeks / 52;
    let age_w = view.age_weeks % 52;
    let energy_pct = view.energy.to_int().clamp(0, 100);
    let energy_bars = (energy_pct / 10) as usize;
    let energy_bar = format!(
        "{}{}",
        "█".repeat(energy_bars),
        "░".repeat(10 - energy_bars.min(10))
    );

    let injury_str = if view.injury_weeks > 0 {
        format!("  ⚠ INJURED ({} wks)", view.injury_weeks)
    } else {
        String::new()
    };

    writeln!(out, "\n╔══════════════════════════════════════════════╗").unwrap();
    box_line(
        out,
        &format!("  {}  Age {}y{}w{injury_str}", view.name, age_y, age_w),
    );
    box_line(out, &format!("  Energy {energy_bar} {energy_pct}%"));

    if state.season_number > 0 {
        let round = state.season_round + 1; // 1-indexed: next round to play, matches the match header
        let total = ROUNDS_PER_SEASON as u32;
        let club_name = &state.pc_club;
        let disc_label = match state.pc_discipline_rep {
            0..=30 => "Clean",
            31..=60 => "Neutral",
            61..=80 => "Combative",
            _ => "Enforcer",
        };
        box_line(
            out,
            &format!(
                "  S{} Round {}/{}  {}",
                state.season_number, round, total, club_name
            ),
        );
        box_line(
            out,
            &format!(
                "  Form:{}  Disc:{} 🟨{} (cards)",
                state.pc_form.to_int(),
                disc_label,
                state.pc_yellow_cards_season,
            ),
        );
        if state.pc_suspension_weeks > 0 {
            box_line(
                out,
                &format!("  SUSPENDED ({} match(es) left)", state.pc_suspension_weeks),
            );
        }
    }

    // Routine summary — wrapped so up to 4 focus attrs never get truncated
    // away just because their names happen to be long.
    let mut routine_items: Vec<String> = if state.pc_routine.focus_attrs.is_empty() {
        vec!["No focus".to_string()]
    } else {
        let n = state.pc_routine.focus_attrs.len();
        state
            .pc_routine
            .focus_attrs
            .iter()
            .enumerate()
            .map(|(idx, &a)| {
                let name = ATTR_NAMES[a as usize]
                    .split_whitespace()
                    .next()
                    .unwrap_or("?");
                if idx + 1 < n {
                    format!("{name},")
                } else {
                    name.to_string()
                }
            })
            .collect()
    };
    routine_items.push(format!("[{}]", state.pc_routine.intensity.name()));
    box_lines_wrapped(out, "  Routine: ", &routine_items, " ");

    // Last week growth. `to_int()` truncates toward zero, which is 0 for
    // essentially every real week (base growth is sub-1.0/week) — display the
    // raw Fixed value as a decimal instead so real growth is visible.
    let had_growth = state.last_week_growth.iter().any(|&g| g != Fixed::ZERO);
    if had_growth {
        let mut v: Vec<_> = state
            .last_week_growth
            .iter()
            .enumerate()
            .filter(|(_, &g)| g > Fixed::ZERO)
            .collect();
        v.sort_by_key(|&(_, &g)| Reverse(g));
        let items: Vec<String> = v
            .into_iter()
            .map(|(i, &g)| {
                format!(
                    "{} +{:.1}",
                    ATTR_NAMES[i].split_whitespace().next().unwrap_or("?"),
                    g.to_raw() as f64 / 1000.0
                )
            })
            .collect();
        box_lines_wrapped(out, "  Last week: ", &items, "  ");
    }

    let fam = derive_attrs(&view.current);
    let player_ovr = ovr(&view.current, view.primary_position);
    let family_items = vec![
        format!("Pac:{}", fam.pace.to_int()),
        format!("Sho:{}", fam.shooting.to_int()),
        format!("Pas:{}", fam.passing.to_int()),
        format!("Dri:{}", fam.dribbling.to_int()),
        format!("Def:{}", fam.defending.to_int()),
        format!("Phy:{}", fam.physical.to_int()),
    ];
    box_lines_wrapped(
        out,
        &format!("  OVR {}  ", player_ovr.to_int()),
        &family_items,
        " ",
    );
    writeln!(out, "╚══════════════════════════════════════════════╝").unwrap();
}

fn render_player_sheet(
    out: &mut impl Write,
    player: &PlayerView,
    choices: &CreationChoices,
    seed: u64,
    lifestyle: u8,
) {
    let cur = &player.current;
    let fam = &player.familiarity;
    let families = derive_attrs(cur);
    let player_ovr = ovr(cur, player.primary_position);

    writeln!(out, "\n╔══════════════════════════════════════════════╗").unwrap();
    writeln!(
        out,
        "║  {:<44}║",
        format!("{}  OVR {}", player.name, player_ovr.to_int())
    )
    .unwrap();
    box_line(out, "  OVR is position-weighted, not a simple avg.");
    writeln!(out, "╠══════════════════════════════════════════════╣").unwrap();
    if seed > 0 {
        writeln!(
            out,
            "║  Position: {:<10}  Seed: {:<16}║",
            choices.primary_position.name(),
            seed
        )
        .unwrap();
        box_line(
            out,
            &format!(
                "  Nationality: {}  Club: {}",
                choices.nationality, choices.club
            ),
        );
    }
    // Lifestyle is a read-only, emergent readout (bible §8.6) — never a menu pick.
    let lifestyle_label = match lifestyle {
        0 => "Professional",
        2 => "Flashy",
        _ => "Balanced",
    };
    writeln!(out, "║  Lifestyle: {:<34}║", lifestyle_label).unwrap();
    writeln!(out, "╠══════════════════════════════════════════════╣").unwrap();
    writeln!(out, "║  ATTRIBUTES (current / potential)            ║").unwrap();
    writeln!(out, "║  ─────────────────────────────────────────── ║").unwrap();

    for (label, family_val, indices) in [
        ("PACE", families.pace, PACE_ATTRS),
        ("SHOOTING", families.shooting, SHOOTING_ATTRS),
        ("PASSING", families.passing, PASSING_ATTRS),
        ("DRIBBLING", families.dribbling, DRIBBLING_ATTRS),
        ("DEFENDING", families.defending, DEFENDING_ATTRS),
        ("PHYSICAL", families.physical, PHYSICAL_ATTRS),
    ] {
        writeln!(
            out,
            "║  {:<10} {:>3}                               ║",
            label,
            family_val.to_int()
        )
        .unwrap();
        for &i in indices {
            writeln!(
                out,
                "║    {:<18} {:>3} / {:>3}               ║",
                ATTR_NAMES[i],
                player.current[i].to_int(),
                player.potential[i].to_int()
            )
            .unwrap();
        }
    }

    writeln!(out, "╠══════════════════════════════════════════════╣").unwrap();
    writeln!(out, "║  ROLES                                       ║").unwrap();
    writeln!(out, "║  ─────────────────────────────────────────── ║").unwrap();

    let mut role_rows: Vec<_> = RoleId::ALL
        .iter()
        .map(|&r| {
            let t = fam[r as usize];
            (r, role_rating(cur, r, t), t)
        })
        .collect();
    role_rows.sort_by_key(|b| Reverse(b.1));

    for (role, rat, tier) in &role_rows {
        writeln!(
            out,
            "║  {:<18} {:>3}  ({:<12})       ║",
            role.name(),
            rat.to_int(),
            tier.name()
        )
        .unwrap();
    }
    writeln!(out, "╚══════════════════════════════════════════════╝").unwrap();
}

/// Phase 9 world screen: the seeded pantheon canon + this career's emergent rival.
/// Pure renderer — all data comes from `goat_world` functions; no sim logic here.
fn render_world_screen(out: &mut impl Write, state: &WorldState, world: &WorldGenesis) {
    use goat_world::history::{backfill_history, great_nation_name};
    use goat_world::rival::{crystallise_rival, RivalVerdict};
    let seed = state.world_seed;

    writeln!(out, "\n╔══════════════════════════════════════════════╗").unwrap();
    writeln!(out, "║  THE WORLD — Pantheon & Your Generation      ║").unwrap();
    writeln!(out, "╚══════════════════════════════════════════════╝").unwrap();

    // Backfilled canon of past greats (pure-derivable from the world seed).
    let hist = backfill_history(seed, 30, world);
    writeln!(out, "\n  PANTHEON — past greats of this universe").unwrap();
    writeln!(
        out,
        "  {:<20} {:<9} {:>4} {:>4}",
        "Name", "Nation", "BdOr", "Peak"
    )
    .unwrap();
    writeln!(out, "  {}", "─".repeat(42)).unwrap();
    for g in hist.canon_ranked().iter().take(6) {
        writeln!(
            out,
            "  {:<20} {:<9} {:>4} {:>4}",
            g.name,
            great_nation_name(g.nationality, world),
            g.ballon_dors,
            g.peak_ovr
        )
        .unwrap();
    }

    // Your generation: batch-tick the cohort up to now, then crystallise.
    let league_clubs = world.static_league_clubs();
    let mut pop = goat_world::population::genesis(seed, world);
    let seasons = state.season_number.max(1);
    for s in 1..=seasons {
        goat_world::batch_tick::batch_tick_season(&mut pop, world, &league_clubs, seed, s, s * 52);
    }
    writeln!(out, "\n  YOUR GENERATION").unwrap();
    match crystallise_rival(&pop, 16 * 52, state.pc_career_goals, state.pc_league_titles) {
        RivalVerdict::Rival {
            name,
            peer_goals,
            peer_titles,
            ..
        } => writeln!(
            out,
            "  Rival: {name} — {peer_goals} goals, {peer_titles} titles. The media frames it.",
        )
        .unwrap(),
        RivalVerdict::WeakEra => writeln!(
            out,
            "  No rival has kept pace — you reign alone (the weak-era asterisk looms).",
        )
        .unwrap(),
    }
}

fn display_flashpoints(
    out: &mut impl Write,
    flashpoints: &[goat_core::calendar_loop::CalendarFlashpoint],
) {
    use goat_calendar::WindowKind;
    for f in flashpoints {
        let (icon, label) = match f.window {
            WindowKind::TransferSummer => ("⇄", "The summer transfer window is open."),
            WindowKind::TransferWinter => ("⇄", "The winter transfer window is open."),
            WindowKind::InternationalBreak => ("✈", "International break — call-ups announced."),
            WindowKind::OffSeason => ("☼", "The off-season has begun."),
        };
        writeln!(out, "  {icon}  CALENDAR: {label}").unwrap();
    }
}

fn display_events(out: &mut impl Write, events: &[DevelopmentEvent]) {
    if events.is_empty() {
        return;
    }
    writeln!(out, "\n  *** EVENTS ***").unwrap();
    for e in events {
        match e {
            DevelopmentEvent::Injury { weeks } => {
                writeln!(out, "  ⚠  INJURY! Out for {weeks} week(s).").unwrap()
            }
            DevelopmentEvent::Illness { weeks } => {
                writeln!(out, "  ⚠  ILLNESS! Reduced capacity for {weeks} week(s).").unwrap()
            }
            DevelopmentEvent::Breakthrough { attr, bonus } => writeln!(
                out,
                "  ★  BREAKTHROUGH! {} +{:.1}",
                ATTR_NAMES[*attr as usize],
                bonus.to_int()
            )
            .unwrap(),
            DevelopmentEvent::FamiliarityUpgrade { role, new_tier } => writeln!(
                out,
                "  ↑  Role familiarity up: {} → {}",
                role.name(),
                new_tier.name()
            )
            .unwrap(),
        }
    }
}

// ── Match beat rendering ──────────────────────────────────────────────────────

fn render_beat(out: &mut impl Write, ms: &ActiveMatchState) {
    let beat = match ms.current_beat() {
        Some(b) => b,
        None => return,
    };
    let min = ms.current_minute();
    let hs = &ms.headspace;
    let stam = ms.stamina.to_int();

    writeln!(out, "\n────────────────────────────────────────────────").unwrap();
    writeln!(
        out,
        " {min:>2}' │ Output: {}/10  Stamina: {stam}  Flow: {}  Nerves: {}",
        ms.player_output / 10,
        hs.flow / 10,
        hs.nerves / 10
    )
    .unwrap();
    writeln!(out, " Score: {}–{}", ms.goals_for, ms.goals_against).unwrap();
    writeln!(out, "────────────────────────────────────────────────").unwrap();
    writeln!(out, " {}", beat.setup).unwrap();
    writeln!(out).unwrap();
    for (i, c) in beat.choices.iter().enumerate() {
        writeln!(out, "  {}. {}", i + 1, c.text).unwrap();
    }
}

fn read_choice(
    lines: &mut impl Iterator<Item = io::Result<String>>,
    out: &mut impl Write,
    n_choices: usize,
) -> usize {
    loop {
        let s = prompt_or_exit(lines, out, "Your choice");
        if let Ok(n) = s.trim().parse::<usize>() {
            if n >= 1 && n <= n_choices {
                return n - 1;
            }
        }
        writeln!(out, "  Enter 1–{n_choices}.").unwrap();
    }
}

fn render_match_result(out: &mut impl Write, result: &MatchResult, opp: &str) {
    let win_str = match result.goals_for.cmp(&result.goals_against) {
        std::cmp::Ordering::Greater => "WIN",
        std::cmp::Ordering::Less => "LOSS",
        std::cmp::Ordering::Equal => "DRAW",
    };
    let stars = "★".repeat((result.player_output / 20 + 1).clamp(1, 5) as usize);
    writeln!(out, "\n╔══════════════════════════════════════════════╗").unwrap();
    writeln!(out, "║  FULL TIME vs {:<32}║", opp).unwrap();
    writeln!(
        out,
        "║  Result: {}–{}  ({win_str})                           ║",
        result.goals_for, result.goals_against
    )
    .unwrap();
    writeln!(
        out,
        "║  Rating: {:>3}/100  {stars:<5}                        ║",
        result.player_output
    )
    .unwrap();
    writeln!(out, "╠══════════════════════════════════════════════╣").unwrap();
    writeln!(out, "║  KEY MOMENTS                                 ║").unwrap();
    for m in result
        .moments
        .iter()
        .filter(|m| m.goal_event.is_some() || m.success)
        .take(5)
    {
        let icon = match m.goal_event {
            Some(ScoreEvent::GoalFor) => "⚽",
            Some(ScoreEvent::GoalAgainst) => "❌",
            None => {
                if m.success {
                    "✓"
                } else {
                    "✗"
                }
            }
        };
        box_line(out, &format!("  {icon} {}'  {}", m.minute, m.outcome_text));
    }
    writeln!(out, "╚══════════════════════════════════════════════╝").unwrap();
}

// ── Utilities ─────────────────────────────────────────────────────────────────

/// Read one line of input, distinguishing a genuine blank Enter (`Some(String::new())`)
/// from stdin EOF (`None`) — the two must never be conflated, or a closed pipe
/// looks identical to "bad input" and a reprompt loop spins forever.
fn prompt(
    lines: &mut impl Iterator<Item = io::Result<String>>,
    out: &mut impl Write,
    label: &str,
) -> Option<String> {
    write!(out, "  {label}: ").unwrap();
    out.flush().unwrap();
    match lines.next() {
        Some(Ok(l)) => Some(l),
        _ => None,
    }
}

/// `prompt()`, treating stdin EOF like an explicit quit: no more input will
/// ever arrive on a closed pipe, so reprompting would hang the process forever.
fn prompt_or_exit(
    lines: &mut impl Iterator<Item = io::Result<String>>,
    out: &mut impl Write,
    label: &str,
) -> String {
    match prompt(lines, out, label) {
        Some(s) => s,
        None => {
            writeln!(out, "\n  No more input — exiting.").unwrap();
            out.flush().unwrap();
            std::process::exit(0);
        }
    }
}
