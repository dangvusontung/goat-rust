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
    generation::{generate_player, CreationChoices, Position},
    player::PlayerView,
    roles::RoleId,
    state::{reduce, Intent, WorldState},
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
use goat_save::save::{from_world_state, load_from_file, save_to_file, to_world_state};
use goat_traits::PlayerTraits;
use goat_world::{
    fixture_for_round, format_week_header, round_fixtures, round_to_week, sim_team_match, Table,
    BASE_CAREER_YEAR, CLUBS, CLUBS_PER_DIV, DIV_CLUBS, DIV_NAMES, DIV_NATIONS, ROUNDS_PER_SEASON,
};

const SAVE_PATH: &str = "goat.sav";
const BEATS_JSON: &str = include_str!("../../../beats.json");

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    let mut lines = stdin.lock().lines();

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
                "L" | "LOAD" => match load_from_file(SAVE_PATH) {
                    Ok(data) => {
                        writeln!(out, "  Save loaded.").unwrap();
                        let state = to_world_state(&data);
                        run_game_loop(
                            &mut lines,
                            &mut out,
                            state,
                            &beat_lib,
                            PlayerTraits::default(),
                        );
                    }
                    Err(e) => writeln!(out, "  Load failed: {e}").unwrap(),
                },
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
        let s = prompt(lines, out, "Player name");
        if s.trim().is_empty() {
            "Unnamed Legend".to_string()
        } else {
            s
        }
    };

    writeln!(
        out,
        "\nPick a position:\n  1. Defender\n  2. Midfielder\n  3. Forward"
    )
    .unwrap();
    let position = loop {
        match prompt(lines, out, "Choice [1-3]").trim() {
            "1" => break Position::Defender,
            "2" => break Position::Midfielder,
            "3" => break Position::Forward,
            _ => writeln!(out, "  Please enter 1, 2 or 3.").unwrap(),
        }
    };

    // Pick nation
    writeln!(out, "\nPick a nation:\n  1. England\n  2. Brazil").unwrap();
    let nation_idx: usize = loop {
        match prompt(lines, out, "Choice [1-2]").trim() {
            "1" => break 0, // England = DIV_ENG_TOP / DIV_ENG_SEC
            "2" => break 2, // Brazil = DIV_BRA_TOP / DIV_BRA_SEC
            _ => writeln!(out, "  Please enter 1 or 2.").unwrap(),
        }
    };

    // Pick division (two per nation)
    let div_a = nation_idx;
    let div_b = nation_idx + 1;
    writeln!(
        out,
        "\nPick a division:\n  1. {} (top)\n  2. {} (second)",
        DIV_NAMES[div_a], DIV_NAMES[div_b]
    )
    .unwrap();
    let div_idx: usize = loop {
        match prompt(lines, out, "Choice [1-2]").trim() {
            "1" => break div_a,
            "2" => break div_b,
            _ => writeln!(out, "  Please enter 1 or 2.").unwrap(),
        }
    };

    // Pick club from that division
    writeln!(out, "\nPick a club:").unwrap();
    let div_club_ids = DIV_CLUBS[div_idx];
    for (i, &cid) in div_club_ids.iter().enumerate() {
        let club = &CLUBS[cid];
        let stars = "*".repeat(((club.strength as usize).div_ceil(20)).clamp(1, 5));
        writeln!(out, "  {:2}. {:<22} {}", i + 1, club.name, stars).unwrap();
    }
    let club_pos: usize = loop {
        let s = prompt(lines, out, &format!("Choice [1-{}]", CLUBS_PER_DIV));
        if let Ok(n) = s.trim().parse::<usize>() {
            if (1..=CLUBS_PER_DIV).contains(&n) {
                break n - 1;
            }
        }
        writeln!(out, "  Invalid choice.").unwrap();
    };
    let club_id = div_club_ids[club_pos];
    let club = &CLUBS[club_id];

    let seed = loop {
        let s = prompt(lines, out, "Seed (Enter = random)");
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

    let nationality = DIV_NATIONS[div_idx].name();
    let choices = CreationChoices {
        name,
        position,
        nationality,
        club: club.name,
    };

    let mut effective_seed = seed;
    loop {
        let player = generate_player(effective_seed, &choices);
        render_player_sheet(out, &player, &choices, effective_seed);
        writeln!(out, "\n  [S] Start game   [R] Re-roll   [Q] Back").unwrap();
        write!(out, "  > ").unwrap();
        out.flush().unwrap();

        match lines.next() {
            Some(Ok(l)) => match l.trim().to_ascii_uppercase().as_str() {
                "S" | "START" => {
                    // Pick lifestyle (Phase 10).
                    writeln!(out, "\n--- LIFESTYLE ---").unwrap();
                    writeln!(
                        out,
                        "  1. Professional — slower decline, better long-term development"
                    )
                    .unwrap();
                    writeln!(out, "  2. Balanced      — the default path").unwrap();
                    writeln!(
                        out,
                        "  3. Flashy        — peak burns brighter but fades faster"
                    )
                    .unwrap();
                    let lifestyle: u8 = match prompt(lines, out, "Choice [1-3]").trim() {
                        "1" => 0,
                        "3" => 2,
                        _ => 1,
                    };

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
                            world_seed: effective_seed,
                            pc_club_idx: club_id as u16,
                            pc_div_idx: div_idx as u8,
                            facilities_mult: club.facilities_mult(),
                            initial_table: Box::new([0u32; 80]),
                        },
                        &mut GoatRng::new(0),
                    );
                    state = reduce(
                        state,
                        Intent::SetLifestyle { lifestyle },
                        &mut GoatRng::new(0),
                    );

                    // Seed peer cohort (Phase 9).
                    let peers = build_peer_cohort(effective_seed, DIV_NATIONS[div_idx].name());
                    state = reduce(state, Intent::InitPeers { peers }, &mut GoatRng::new(0));

                    // Roll hidden trait ceilings from creation seed (§A.3 aptitude).
                    // XOR with a domain tag so trait rolls don't alias attribute rolls.
                    let pc_traits = PlayerTraits::roll_from_seed(
                        effective_seed,
                        &mut GoatRng::new(effective_seed ^ 0x7261_6974_0000_0001),
                    );

                    state = reduce(state, Intent::StartSeason, &mut GoatRng::new(0));
                    run_game_loop(lines, out, state, beat_lib, pc_traits);
                    return;
                }
                "R" | "REROLL" | "RE-ROLL" => {
                    let mut rng = GoatRng::new(effective_seed);
                    effective_seed = rng.next_u64();
                }
                _ => return,
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
) {
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

        // End-of-season gate
        if state.season_number > 0 && state.season_round >= ROUNDS_PER_SEASON as u32 {
            let view = state.players.snapshot(pc_id);
            render_season_review(out, &state, &view);

            // Collect annual wage.
            state = reduce(state, Intent::CollectWage, &mut GoatRng::new(0));

            // Awards night + pundit reactions + legacy update.
            state = run_awards_and_pundits(out, state, &view);

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
            state = run_transfer_window(lines, out, state, &view);
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
                        state = reduce(state, Intent::StartSeason, &mut GoatRng::new(0));
                        continue;
                    }
                    "G" => {
                        let ev = build_legacy_evidence(&state);
                        render_legacy_screen(out, &ev, &state);
                        continue;
                    }
                    "Z" => {
                        run_save(out, &state);
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
                "  [T] Table  [G] Legacy  [V] Sheet  [Z] Save  [Q] Quit"
            )
            .unwrap();
        } else {
            writeln!(
                out,
                "\n  [W] Train  [F] Fast-fwd  [S] Routine  [V] Sheet  [Q] Quit"
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
                    state = reduce(state, Intent::AdvanceWeek, &mut GoatRng::new(week_rng_seed));
                    display_events(out, &state.last_week_events);
                }
                "F" => {
                    let n = loop {
                        let s = prompt(lines, out, "Advance how many weeks?");
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
                }
                "S" => state = run_set_routine(lines, out, state),
                "P" if has_season => {
                    state = run_next_round(lines, out, state, true, beat_lib, pc_traits)
                }
                "K" if has_season => {
                    state = run_next_round(lines, out, state, false, beat_lib, pc_traits)
                }
                "T" if has_season => render_table(out, &state),
                "G" if has_season => {
                    let ev = build_legacy_evidence(&state);
                    render_legacy_screen(out, &ev, &state);
                }
                "Z" => {
                    run_save(out, &state);
                }
                "V" => {
                    let choices = CreationChoices {
                        name: view.name.clone(),
                        position: match state.pc_position {
                            1 => Position::Midfielder,
                            2 => Position::Forward,
                            _ => Position::Defender,
                        },
                        nationality: state.pc_nationality,
                        club: state.pc_club,
                    };
                    render_player_sheet(out, &view, &choices, 0);
                }
                _ => {}
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

    let week_label = format_week_header(BASE_CAREER_YEAR + season - 1, round_to_week(round));
    writeln!(
        out,
        "\n--- ROUND {} / {} · {} ---",
        round + 1,
        ROUNDS_PER_SEASON,
        week_label
    )
    .unwrap();

    // Suspension check: serve ban, auto-skip.
    if state.pc_suspension_weeks > 0 {
        state.pc_suspension_weeks -= 1;
        writeln!(
            out,
            "  You are SUSPENDED. {} match(es) remaining after this.",
            state.pc_suspension_weeks
        )
        .unwrap();
        // Still need to sim other matches and advance the round.
        let all_fixtures = round_fixtures(world_seed, season, div_idx, round);
        let sim_seed = world_seed ^ ((season as u64) << 32) ^ (round as u64) ^ 0xfeed;
        let mut sim_rng = GoatRng::new(sim_seed);
        let mut round_results: Vec<(u8, u8, u32, u32)> = Vec::new();
        for f in &all_fixtures {
            let (gf, ga) =
                sim_team_match(CLUBS[f.home].strength, CLUBS[f.away].strength, &mut sim_rng);
            let h_pos = club_div_pos_in(div_idx, f.home) as u8;
            let a_pos = club_div_pos_in(div_idx, f.away) as u8;
            round_results.push((h_pos, a_pos, gf, ga));
        }
        return reduce(
            state,
            Intent::ApplyRoundResult {
                pc_goals: 0,
                pc_output: 0,
                pc_result: 0,
                round_results,
            },
            &mut GoatRng::new(0),
        );
    }

    // Find PC's fixture this round.
    let pc_fixture = fixture_for_round(world_seed, season, div_idx, pc_club_id, round);

    let (pc_goals, pc_output, pc_result, match_result_opt) = match pc_fixture {
        Some(f) => {
            let is_home = f.home == pc_club_id;
            let opp_id = if is_home { f.away } else { f.home };
            let opp = &CLUBS[opp_id];
            let own_str = CLUBS[pc_club_id].strength;
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
                opp_name: opp.name,
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

            render_match_result(out, &result, opp.name);

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
    let all_fixtures = round_fixtures(world_seed, season, div_idx, round);
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
            sim_team_match(CLUBS[f.home].strength, CLUBS[f.away].strength, &mut sim_rng)
        };
        let h_pos = club_div_pos_in(div_idx, f.home) as u8;
        let a_pos = club_div_pos_in(div_idx, f.away) as u8;
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
                h == club_div_pos_in(div_idx, f.home) as u8
                    && a == club_div_pos_in(div_idx, f.away) as u8
            })
            .map(|&(_, _, gf, ga)| (gf, ga))
            .unwrap_or((0, 0));
        writeln!(
            out,
            "  {:20} {:1}–{:1}  {}",
            CLUBS[f.home].name, gf, ga, CLUBS[f.away].name
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
        },
        &mut GoatRng::new(0),
    );

    state
}

fn club_div_pos_in(div_idx: usize, club_id: usize) -> usize {
    DIV_CLUBS[div_idx]
        .iter()
        .position(|&c| c == club_id)
        .expect("club in division")
}

fn best_role_for_position(pc_position: u8) -> RoleId {
    match pc_position {
        0 => RoleId::CentreBack,
        1 => RoleId::CentralMid,
        _ => RoleId::CompleteForward,
    }
}

// ── Table display ─────────────────────────────────────────────────────────────

fn render_table(out: &mut impl Write, state: &WorldState) {
    let div_idx = state.pc_div_idx as usize;
    let div_club_ids = DIV_CLUBS[div_idx];
    let table = Table::from_raw(&state.table_raw, &div_club_ids);
    let sorted = table.sorted();
    let pc_club_id = state.pc_club_idx as usize;

    writeln!(
        out,
        "\n  {} — Round {}",
        DIV_NAMES[div_idx], state.season_round
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
            CLUBS[e.club_id].name,
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

fn render_season_review(out: &mut impl Write, state: &WorldState, view: &PlayerView) {
    writeln!(out, "\n╔══════════════════════════════════════════════╗").unwrap();
    writeln!(
        out,
        "║  SEASON {} REVIEW                             ║",
        state.season_number
    )
    .unwrap();
    writeln!(out, "╠══════════════════════════════════════════════╣").unwrap();

    let div_idx = state.pc_div_idx as usize;
    let div_club_ids = DIV_CLUBS[div_idx];
    let table = Table::from_raw(&state.table_raw, &div_club_ids);
    let pos = table.position_of(state.pc_club_idx as usize);

    writeln!(
        out,
        "║  {}: finished {}th in {}",
        CLUBS[state.pc_club_idx as usize].name, pos, DIV_NAMES[div_idx]
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
            CLUBS[e.club_id].name,
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
    }
}

fn render_legacy_screen(out: &mut impl Write, ev: &LegacyEvidence, state: &WorldState) {
    let pc_name = CLUBS[state.pc_club_idx as usize].name; // placeholder — use player name
    let axes = compute_axes(ev);
    let rep = compute_reputation(
        state.pc_sporting_rep,
        state.pc_discipline_rep,
        state.pc_club_fan_rep,
    );
    let rankings = all_rankings(&axes);

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
}

fn run_awards_and_pundits(
    out: &mut impl Write,
    mut state: WorldState,
    view: &PlayerView,
) -> WorldState {
    let season = state.season_number;
    let world_seed = state.world_seed;
    let pc_name = &view.name;
    let pc_club = state.pc_club;

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
    let div_clubs = goat_world::DIV_CLUBS[div_idx];
    let table = goat_world::Table::from_raw(&state.table_raw, &div_clubs);
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
        },
        &mut GoatRng::new(0),
    );

    // Pundit reactions.
    let ev = build_legacy_evidence(&state);
    let axes = compute_axes(&ev);
    let rankings = all_rankings(&axes);

    writeln!(out, "\n  --- THE PUNDITS ---").unwrap();
    for pundit in PUNDITS.iter() {
        let (_, rank, _) = rankings[pundit.school_idx];
        let ctx = PunditContext::Season {
            goals: state.pc_season_goals,
            matches: state.pc_season_matches,
            avg_output: season_avg,
            finish_pos,
        };
        let comment = pundit_comment(pundit, &axes, &ctx, pc_name, pc_club, season);
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

fn generate_transfer_offers(state: &WorldState, view: &PlayerView) -> Vec<(usize, u8, i64, u32)> {
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
    for _ in 0..n_offers {
        // Pick a random club from a different division
        let target_div = ((state.pc_div_idx as u64 + 1 + rng.next_range_u64(0, 2)) % 4) as usize;
        let club_pos = rng.next_range_u64(0, (goat_world::CLUBS_PER_DIV - 1) as u64) as usize;
        let club_id = goat_world::DIV_CLUBS[target_div][club_pos];
        let target_strength = CLUBS[club_id].strength;
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
) -> WorldState {
    let offers = generate_transfer_offers(&state, view);
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
        let stars = "*".repeat((CLUBS[club_id].strength as usize / 20).clamp(1, 5));
        writeln!(
            out,
            "║  {}. {:22} ({}) £{}/yr {}yr  ║",
            i + 1,
            CLUBS[club_id].name,
            goat_world::DIV_NAMES[div_idx as usize],
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
                let club = &CLUBS[club_id];
                writeln!(
                    out,
                    "  TRANSFER COMPLETE: {} → {}",
                    state.pc_club, club.name
                )
                .unwrap();
                let fee_bonus = (CLUBS[state.pc_club_idx as usize].strength as i64) * 3;
                state = reduce(
                    state,
                    Intent::ExecuteTransfer {
                        to_club_idx: club_id as u16,
                        to_div_idx: div_idx,
                        new_wage: wage,
                        new_length: length,
                        new_club_name: club.name,
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
    let current_club = &CLUBS[state.pc_club_idx as usize];
    let new_wage = state.pc_wage_annual + (state.pc_form.to_int() as i64 / 10) * 5;
    let new_length = 2u32;

    writeln!(out, "\n╔══════════════════════════════════════════════╗").unwrap();
    writeln!(out, "║  CONTRACT RENEWAL — {}  ║", current_club.name).unwrap();
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

    let nat: &'static str = if nationality == "Brazil" {
        "Brazil"
    } else {
        "England"
    };
    let mut rng = GoatRng::new(world_seed ^ 0xC0_CA_FE_BE_EF_u64);
    (0..8)
        .map(|i| goat_core::state::PeerState {
            seed: rng.next_u64(),
            name: names[i % names.len()].to_string(),
            nationality: nat,
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
    let axes = compute_axes(&ev);
    let rankings = all_rankings(&axes);
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

fn run_save(out: &mut impl Write, state: &WorldState) {
    if let Some(pc_id) = state.pc_player_id {
        let view = state.players.snapshot(pc_id);
        let data = from_world_state(state, &view);
        match save_to_file(&data, SAVE_PATH) {
            Ok(()) => writeln!(out, "  Game saved to {SAVE_PATH}.").unwrap(),
            Err(e) => writeln!(out, "  Save failed: {e}").unwrap(),
        }
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
        let s = prompt(lines, out, "Attr numbers (e.g. 1,7,9) or Enter to clear");
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
    let intensity_input = prompt(lines, out, "Choice [1-3]");
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

fn render_game_sheet(out: &mut impl Write, view: &PlayerView, state: &WorldState) {
    let age_y = view.age_weeks / 52;
    let age_w = view.age_weeks % 52;
    let energy_bars = (view.energy.to_int() / 10) as usize;
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
    writeln!(
        out,
        "║  {}  Age {}y{}w  Energy {}{}",
        view.name, age_y, age_w, energy_bar, injury_str
    )
    .unwrap();

    if state.season_number > 0 {
        let round = state.season_round;
        let total = ROUNDS_PER_SEASON as u32;
        let club_name = CLUBS[state.pc_club_idx as usize].name;
        let susp_str = if state.pc_suspension_weeks > 0 {
            format!("  SUSPENDED({})", state.pc_suspension_weeks)
        } else {
            String::new()
        };
        let disc_label = match state.pc_discipline_rep {
            0..=30 => "Clean",
            31..=60 => "Neutral",
            61..=80 => "Combative",
            _ => "Enforcer",
        };
        writeln!(
            out,
            "║  S{}  Round {}/{}  {}  Form:{}  Disc:{} 🟨{}{}",
            state.season_number,
            round,
            total,
            club_name,
            state.pc_form.to_int(),
            disc_label,
            state.pc_yellow_cards_season,
            susp_str
        )
        .unwrap();
    }

    // Routine summary
    let routine_str = if state.pc_routine.focus_attrs.is_empty() {
        "No focus".to_string()
    } else {
        state
            .pc_routine
            .focus_attrs
            .iter()
            .map(|&a| {
                ATTR_NAMES[a as usize]
                    .split_whitespace()
                    .next()
                    .unwrap_or("?")
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    writeln!(
        out,
        "║  Routine: {} [{}]",
        routine_str,
        state.pc_routine.intensity.name()
    )
    .unwrap();

    // Last week growth
    let had_growth = state.last_week_growth.iter().any(|&g| g != Fixed::ZERO);
    if had_growth {
        let mut v: Vec<_> = state
            .last_week_growth
            .iter()
            .enumerate()
            .filter(|(_, &g)| g > Fixed::ZERO)
            .collect();
        v.sort_by_key(|&(_, &g)| Reverse(g));
        let growth_str = v
            .into_iter()
            .take(3)
            .map(|(i, &g)| {
                format!(
                    "{} +{:.1}",
                    ATTR_NAMES[i].split_whitespace().next().unwrap_or("?"),
                    g.to_int()
                )
            })
            .collect::<Vec<_>>()
            .join("  ");
        writeln!(out, "║  Last week: {growth_str}").unwrap();
    }

    let fam = derive_attrs(&view.current);
    let player_ovr = ovr(&view.current, view.primary_position);
    writeln!(
        out,
        "║  OVR {:<3}  Pac:{:<3} Sho:{:<3} Pas:{:<3} Dri:{:<3} Def:{:<3} Phy:{:<3}",
        player_ovr.to_int(),
        fam.pace.to_int(),
        fam.shooting.to_int(),
        fam.passing.to_int(),
        fam.dribbling.to_int(),
        fam.defending.to_int(),
        fam.physical.to_int()
    )
    .unwrap();
    writeln!(out, "╚══════════════════════════════════════════════╝").unwrap();
}

fn render_player_sheet(
    out: &mut impl Write,
    player: &PlayerView,
    choices: &CreationChoices,
    seed: u64,
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
    writeln!(out, "╠══════════════════════════════════════════════╣").unwrap();
    if seed > 0 {
        writeln!(
            out,
            "║  Position: {:<10}  Seed: {:<16}║",
            choices.position.name(),
            seed
        )
        .unwrap();
        writeln!(
            out,
            "║  Nationality: {:<10}  Club: {:<14}║",
            choices.nationality, choices.club
        )
        .unwrap();
    }
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
        let s = prompt(lines, out, "Your choice");
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
        writeln!(
            out,
            "║  {icon} {}'  {}",
            m.minute,
            m.outcome_text.chars().take(38).collect::<String>()
        )
        .unwrap();
    }
    writeln!(out, "╚══════════════════════════════════════════════╝").unwrap();
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn prompt(
    lines: &mut impl Iterator<Item = io::Result<String>>,
    out: &mut impl Write,
    label: &str,
) -> String {
    write!(out, "  {label}: ").unwrap();
    out.flush().unwrap();
    lines.next().and_then(|r| r.ok()).unwrap_or_default()
}
