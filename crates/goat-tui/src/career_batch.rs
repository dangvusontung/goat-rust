/// career-batch — headless batch runner for full careers (16 → up to 20 seasons).
///
/// Runs N careers in-process (no subprocess spawns), rotating across the 3
/// simmable positions (Forward/Midfielder/Defender), and prints one CSV row
/// per career to stdout. Mirrors `career-sim`'s default simulation path
/// (world genesis → CreatePlayer → InitWorld → 20-season loop → season-end
/// legacy) minus the ASCII portrait rendering.
///
/// Usage: career-batch [num_careers] [master_seed]
///   career-batch 100000 0xBA7C4 > logs/career-batch-100k.csv
use goat_core::{
    attrs::AttrId,
    derive::ovr,
    generation::{CreationChoices, Position},
    state::{reduce, Intent, WorldState},
    week::{Intensity, Routine},
};
use goat_rng::{GoatRng, RngSource};
use goat_world::{
    div_clubs, div_index, facilities_mult, fixture_for_round, round_fixtures, sim_team_match,
    worldgen::generate_world, Table, NATION_ENGLAND, ROUNDS_PER_SEASON,
};

const POSITIONS: [(&str, Position); 3] = [
    ("Forward", Position::Forward),
    ("Midfielder", Position::Midfielder),
    ("Defender", Position::Defender),
];

const INTENSITIES: [(&str, Intensity); 3] = [
    ("Low", Intensity::Low),
    ("Medium", Intensity::Medium),
    ("High", Intensity::High),
];

// (goal_attr, range_max) — same table as career-sim's SimPos::goal_roll.
fn goal_roll(pos: Position) -> (AttrId, u32) {
    match pos {
        Position::Forward => (AttrId::Finishing, 399),
        Position::Midfielder => (AttrId::Finishing, 699),
        Position::Defender => (AttrId::Heading, 1199),
    }
}

fn focus_attrs(pos: Position) -> Vec<AttrId> {
    match pos {
        Position::Forward => vec![
            AttrId::Finishing,
            AttrId::AttPositioning,
            AttrId::ShotPower,
            AttrId::BallControl,
            AttrId::Composure,
        ],
        Position::Midfielder => vec![
            AttrId::ShortPassing,
            AttrId::Stamina,
            AttrId::Vision,
            AttrId::BallControl,
            AttrId::LongPassing,
            AttrId::Composure,
        ],
        Position::Defender => vec![
            AttrId::Marking,
            AttrId::StandingTackle,
            AttrId::Heading,
            AttrId::Strength,
            AttrId::Jumping,
            AttrId::Interceptions,
        ],
    }
}

struct CareerRow {
    seed: u64,
    position: &'static str,
    intensity: &'static str,
    initial_ovr: i32,
    peak_ovr: i32,
    final_ovr: i32,
    ovr_ceiling: i32,
    seasons_played: u32,
    retired_age: u32,
    career_goals: u32,
    career_matches: u32,
    league_titles: u32,
    poty_wins: u32,
    best_season_avg_output: i32,
    sporting_rep: i32,
    club_fan_rep: i32,
    rival_crystallised: bool,
    marketability: i32,
    discipline_rep: i32,
    career_output_sum: i64,
    scouted_avg: i64,
    offer_seasons: u32,
    offers_total: u32,
    offers_wage_avg: i64,
    offers_wage_max: i64,
    season_ovrs: [i32; 20],
}

fn run_one(
    seed: u64,
    pos: Position,
    pos_label: &'static str,
    intensity: Intensity,
    intensity_label: &'static str,
) -> CareerRow {
    let world = generate_world(seed);
    let div_idx = div_index(NATION_ENGLAND, 1);
    let pc_club_id = div_clubs(div_idx)[0];

    let choices = CreationChoices {
        name: "Batch".into(),
        position: pos,
        nationality: "England",
        club: world.clubs[pc_club_id].name.clone(),
    };

    let mut state = WorldState::new();
    state = reduce(
        state,
        Intent::CreatePlayer { seed, choices },
        &mut GoatRng::new(0),
    );
    state = reduce(
        state,
        Intent::InitWorld {
            world_seed: seed,
            pc_club_idx: pc_club_id as u16,
            pc_div_idx: div_idx as u8,
            facilities_mult: facilities_mult(world.clubs[pc_club_id].strength),
            staff_mods: goat_world::staff::club_staff_mods(world.clubs[pc_club_id].strength),
            initial_table: Box::new([0u32; 80]),
        },
        &mut GoatRng::new(0),
    );
    state = reduce(
        state,
        Intent::SetLifestyle { lifestyle: 1 },
        &mut GoatRng::new(0),
    );
    let routine = Routine {
        focus_attrs: focus_attrs(pos),
        intensity,
    };
    state = reduce(state, Intent::SetRoutine { routine }, &mut GoatRng::new(0));

    let pc_id = state.pc_player_id.unwrap();
    let initial_view = state.players.snapshot(pc_id);
    let initial_ovr = ovr(&initial_view.current, initial_view.primary_position).to_int();
    let ovr_ceiling = ovr(&initial_view.potential, initial_view.primary_position).to_int();

    let (goal_attr_id, goal_range_max) = goal_roll(pos);
    let mut peak_ovr = initial_ovr;
    let mut retired_at: u32 = 0;
    let mut season_ovrs = [0i32; 20];
    let mut scouted_sum: i64 = 0;
    let mut scouted_seasons: u32 = 0;
    let mut offer_seasons: u32 = 0;
    let mut offers_total: u32 = 0;
    let mut offers_wage_sum: i64 = 0;
    let mut offers_wage_max: i64 = 0;

    for season in 1u32..=20 {
        state = reduce(state, Intent::StartSeason, &mut GoatRng::new(0));
        let season_div_idx = state.pc_div_idx as usize;
        let season_pc_club = state.pc_club_idx as usize;
        let div_club_ids = div_clubs(season_div_idx);

        for round in 0..ROUNDS_PER_SEASON {
            for t in 0u64..2 {
                let age = state.players.get_age_weeks(pc_id);
                let rng_seed = age as u64 ^ (season as u64 * 0xbeef) ^ (round as u64 * 7) ^ t;
                state = reduce(state, Intent::AdvanceWeek, &mut GoatRng::new(rng_seed));
            }

            let all_fixtures = round_fixtures(seed, season, season_div_idx, round);
            let mut sim_rng = GoatRng::new(seed ^ ((season as u64) << 32) ^ (round as u64));
            let mut round_results: Vec<(u8, u8, u32, u32)> = Vec::new();
            let mut pc_gf = 0u32;
            let mut pc_ga = 0u32;

            for f in &all_fixtures {
                let (gf, ga) = sim_team_match(
                    world.clubs[f.home].strength,
                    world.clubs[f.away].strength,
                    &mut sim_rng,
                );
                let h_pos = div_club_ids.iter().position(|&c| c == f.home).unwrap() as u8;
                let a_pos = div_club_ids.iter().position(|&c| c == f.away).unwrap() as u8;
                round_results.push((h_pos, a_pos, gf, ga));
                if f.home == season_pc_club {
                    pc_gf = gf;
                    pc_ga = ga;
                } else if f.away == season_pc_club {
                    pc_gf = ga;
                    pc_ga = gf;
                }
            }

            let pc_output =
                match fixture_for_round(seed, season, season_div_idx, season_pc_club, round) {
                    Some(_) => {
                        let form_val = state.pc_form.to_int() as u64;
                        let mut out_rng =
                            GoatRng::new(seed ^ (season as u64 * 0xdead) ^ (round as u64));
                        let variance = out_rng.next_range_u64(0, 20) as i32 - 10;
                        (form_val as i32 + variance).clamp(0, 100)
                    }
                    None => 0,
                };
            let pc_result: i8 = if pc_gf > pc_ga {
                1
            } else if pc_gf < pc_ga {
                -1
            } else {
                0
            };

            let goal_attr_val = state
                .players
                .get_current(pc_id, goal_attr_id as usize)
                .to_int() as u32;
            let mut goal_rng =
                GoatRng::new(seed ^ ((season as u64) << 24) ^ (round as u64 * 0x1337));
            let player_goals = (0..pc_gf)
                .filter(|_| goal_rng.next_range_u32(0, goal_range_max) < goal_attr_val)
                .count() as u32;

            state = reduce(
                state,
                Intent::ApplyRoundResult {
                    pc_goals: player_goals,
                    pc_output,
                    pc_result,
                    round_results,
                },
                &mut GoatRng::new(0),
            );
        }

        let view = state.players.snapshot(pc_id);
        let cur_ovr = ovr(&view.current, view.primary_position).to_int();
        if cur_ovr > peak_ovr {
            peak_ovr = cur_ovr;
        }
        season_ovrs[(season - 1) as usize] = cur_ovr;
        let age_years = view.age_weeks / 52;
        let table = Table::from_raw(&state.table_raw, div_club_ids);
        let table_pos = table.position_of(season_pc_club);

        let season_avg = if state.pc_season_matches > 0 {
            state.pc_season_output / state.pc_season_matches as i32
        } else {
            0
        };
        let won_title = table_pos == 1;
        let new_sporting_rep = state.pc_sporting_rep + if season_avg > 60 { 1 } else { 0 };
        let new_club_fan_rep = state.pc_club_fan_rep + 1;
        let season_output_sum = state.pc_season_output;
        let s_goals = state.pc_season_goals;
        let s_matches = state.pc_season_matches;

        // Phase 8 measurement: replicate goat-tui's generate_transfer_offers gate +
        // wage formula exactly (main.rs), observation only — no offer is executed.
        {
            let observed = (state.pc_form.to_int() + season_output_sum) / 2;
            let mut scout_rng = GoatRng::new(seed ^ ((season as u64) << 40) ^ 0xA11BEEF);
            let scouted = goat_world::scout::scout_estimate(observed, &mut scout_rng);
            scouted_sum += scouted as i64;
            scouted_seasons += 1;
            if scouted >= 55 && age_years < 34 {
                let n_offers = scout_rng.next_range_u64(0, 2) as usize;
                if n_offers > 0 {
                    offer_seasons += 1;
                }
                for _ in 0..n_offers {
                    // Merit-based scouting (matches main.rs generate_transfer_offers
                    // after the 2026-09-24 fix): sample 3 candidates, prefer the
                    // closest strength match to `scouted`, 20% runner-up noise.
                    const CANDIDATES: usize = 3;
                    let mut best: Option<(usize, i32)> = None;
                    let mut runner_up: Option<(usize, i32)> = None;
                    for _ in 0..CANDIDATES {
                        let cand_div =
                            ((state.pc_div_idx as u64 + 1 + scout_rng.next_range_u64(0, 2))
                                % goat_world::NUM_DIVISIONS as u64)
                                as usize;
                        let cand_pos = scout_rng
                            .next_range_u64(0, (goat_world::CLUBS_PER_DIV - 1) as u64)
                            as usize;
                        let cand_id = div_clubs(cand_div)[cand_pos];
                        if cand_id == season_pc_club {
                            continue;
                        }
                        let gap = (world.clubs[cand_id].strength as i32 - scouted).abs();
                        match best {
                            Some((_, best_gap)) if gap >= best_gap => {
                                runner_up = Some((cand_id, gap))
                            }
                            _ => {
                                runner_up = best;
                                best = Some((cand_id, gap));
                            }
                        }
                    }
                    let picked = if scout_rng.next_range_u64(0, 99) < 80 {
                        best.or(runner_up)
                    } else {
                        runner_up.or(best)
                    };
                    let Some((club_id, _)) = picked else {
                        continue;
                    };
                    let target_strength = world.clubs[club_id].strength;
                    let wage_offer = (state.pc_wage_annual
                        + (target_strength as i64 * 2)
                        + (scouted as i64 - 50) * 3
                        + scout_rng.next_range_u64(0, 50) as i64)
                        * 100
                        / 100; // agent_q always 0 in this harness (no personal staff hired)
                    offers_total += 1;
                    offers_wage_sum += wage_offer;
                    if wage_offer > offers_wage_max {
                        offers_wage_max = wage_offer;
                    }
                }
            }
        }

        state = reduce(
            state,
            Intent::ApplySeasonEndLegacy {
                season_goals: s_goals,
                season_matches: s_matches,
                season_output_sum,
                won_title,
                player_of_year: season_avg > 75,
                finish_position: table_pos as u32,
                decisive_moments: 0,
                new_sporting_rep,
                new_club_fan_rep,
            },
            &mut GoatRng::new(0),
        );

        retired_at = age_years;
        if age_years >= 35 && cur_ovr < 50 {
            break;
        }
    }

    let final_view = state.players.snapshot(pc_id);
    let final_ovr = ovr(&final_view.current, final_view.primary_position).to_int();

    CareerRow {
        seed,
        position: pos_label,
        intensity: intensity_label,
        initial_ovr,
        peak_ovr,
        final_ovr,
        ovr_ceiling,
        seasons_played: state.pc_seasons_played,
        retired_age: retired_at,
        career_goals: state.pc_career_goals,
        career_matches: state.pc_career_matches,
        league_titles: state.pc_league_titles,
        poty_wins: state.pc_player_of_year_wins,
        best_season_avg_output: state.pc_best_season_avg_output,
        sporting_rep: state.pc_sporting_rep,
        club_fan_rep: state.pc_club_fan_rep,
        rival_crystallised: state.pc_rival_idx.is_some(),
        marketability: state.pc_marketability,
        discipline_rep: state.pc_discipline_rep,
        career_output_sum: state.pc_career_output_sum,
        scouted_avg: if scouted_seasons > 0 {
            scouted_sum / scouted_seasons as i64
        } else {
            0
        },
        offer_seasons,
        offers_total,
        offers_wage_avg: if offers_total > 0 {
            offers_wage_sum / offers_total as i64
        } else {
            0
        },
        offers_wage_max,
        season_ovrs,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(100_000);
    let master_seed: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0xBA7C4);
    let season_curve_path = args
        .get(3)
        .cloned()
        .unwrap_or_else(|| "logs/season_curve.csv".to_string());

    println!(
        "seed,position,intensity,initial_ovr,peak_ovr,final_ovr,ovr_ceiling,seasons_played,retired_age,\
career_goals,career_matches,league_titles,poty_wins,best_season_avg_output,sporting_rep,\
club_fan_rep,rival_crystallised,marketability,discipline_rep,career_output_sum,\
scouted_avg,offer_seasons,offers_total,offers_wage_avg,offers_wage_max"
    );

    // [position][intensity][season-1] -> (sum_ovr, count)
    let mut season_sum = [[[0i64; 20]; 3]; 3];
    let mut season_count = [[[0u64; 20]; 3]; 3];

    for i in 0..n {
        let pos_idx = ((i / 3) % 3) as usize;
        let int_idx = (i % 3) as usize;
        let (pos_label, pos) = POSITIONS[pos_idx];
        let (int_label, intensity) = INTENSITIES[int_idx];
        let seed = master_seed ^ i;
        let row = run_one(seed, pos, pos_label, intensity, int_label);
        for (s, &ovr_val) in row.season_ovrs.iter().enumerate() {
            if ovr_val > 0 {
                season_sum[pos_idx][int_idx][s] += ovr_val as i64;
                season_count[pos_idx][int_idx][s] += 1;
            }
        }
        println!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            row.seed,
            row.position,
            row.intensity,
            row.initial_ovr,
            row.peak_ovr,
            row.final_ovr,
            row.ovr_ceiling,
            row.seasons_played,
            row.retired_age,
            row.career_goals,
            row.career_matches,
            row.league_titles,
            row.poty_wins,
            row.best_season_avg_output,
            row.sporting_rep,
            row.club_fan_rep,
            row.rival_crystallised,
            row.marketability,
            row.discipline_rep,
            row.career_output_sum,
            row.scouted_avg,
            row.offer_seasons,
            row.offers_total,
            row.offers_wage_avg,
            row.offers_wage_max,
        );
    }

    use std::io::Write;
    let mut f = std::fs::File::create(&season_curve_path)
        .unwrap_or_else(|e| panic!("cannot create {season_curve_path}: {e}"));
    writeln!(f, "position,intensity,season,avg_ovr,n").unwrap();
    for (pos_idx, (pos_label, _)) in POSITIONS.iter().enumerate() {
        for (int_idx, (int_label, _)) in INTENSITIES.iter().enumerate() {
            for s in 0..20 {
                let cnt = season_count[pos_idx][int_idx][s];
                if cnt == 0 {
                    continue;
                }
                let avg = season_sum[pos_idx][int_idx][s] as f64 / cnt as f64;
                writeln!(f, "{pos_label},{int_label},{},{avg:.3},{cnt}", s + 1).unwrap();
            }
        }
    }
}
