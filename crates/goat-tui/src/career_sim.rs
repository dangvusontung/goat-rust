/// career-sim — headless 20-season career simulation with full ASCII career portrait.
///
/// Usage:
///   career-sim                          — auto-scans for a star Forward seed
///   career-sim --seed N                 — run with specific seed N
///   career-sim --position midfielder    — sim as Midfielder (forward/midfielder/defender)
///   career-sim --scan                   — print top 20 seeds ranked by key attr potential
use goat_core::{
    attrs::{AttrId, NUM_ATTRS},
    calendar_loop::LEAGUE_COMPETITION_ID,
    derive::ovr,
    generation::{generate_player, CreationChoices},
    positions::{PrimaryPosition, POSITION_WEIGHT_TABLE},
    roles::RoleId,
    state::{reduce, Intent, WorldState},
    week::{Intensity, Routine},
};
use goat_fixed::Fixed;
use goat_match::{
    discipline::RefPersonality,
    sim::{auto_play_match, BeatLibrary, MatchSetup},
};
use goat_rng::{GoatRng, RngSource};
use goat_traits::PlayerTraits;

const BEATS_JSON: &str = include_str!("../../../beats.json");
use goat_world::{
    fixture_for_round, rest_weeks_after_round, round_fixtures, sim_team_match,
    week_ends_after_round, world::WorldGenesis, Table, BASE_CAREER_YEAR, ROUNDS_PER_SEASON,
};

#[path = "orbit_fixtures.rs"]
mod orbit_fixtures;
use orbit_fixtures::build_season_orbit_fixtures;

// ── Position selector ─────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum SimPos {
    Forward,
    Midfielder,
    Defender,
}

impl SimPos {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "forward" | "fw" | "st" => Some(Self::Forward),
            "midfielder" | "mid" | "cm" => Some(Self::Midfielder),
            "defender" | "def" | "cb" => Some(Self::Defender),
            _ => None,
        }
    }

    /// Maps to the same specific position the old 3-way `default_primary()` picked,
    /// so career-sim's byte-identical goldens are preserved.
    fn to_position(self) -> PrimaryPosition {
        match self {
            Self::Forward => PrimaryPosition::ST,
            Self::Midfielder => PrimaryPosition::CM,
            Self::Defender => PrimaryPosition::CB,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Forward => "Forward",
            Self::Midfielder => "Midfielder",
            Self::Defender => "Defender",
        }
    }

    fn focus_attrs(self) -> Vec<AttrId> {
        match self {
            // ST: all attrs with position weight > 0 (Key + Imp + all Sec).
            // Growth is independent per attr so adding Sec costs Key/Imp nothing.
            Self::Forward => vec![
                // Key (w=3)
                AttrId::Finishing,
                AttrId::AttPositioning,
                // Imp (w=2)
                AttrId::ShotPower,
                AttrId::BallControl,
                AttrId::Composure,
                // Sec (w=1)
                AttrId::Reactions,
                AttrId::Acceleration,
                AttrId::SprintSpeed,
                AttrId::Heading,
                AttrId::Strength,
                AttrId::Volleys,
                AttrId::CloseControl,
                AttrId::ShortPassing,
                AttrId::Penalties,
            ],
            // CM: all attrs with position weight > 0.
            Self::Midfielder => vec![
                // Key (w=3)
                AttrId::ShortPassing,
                AttrId::Stamina,
                // Imp (w=2)
                AttrId::Vision,
                AttrId::BallControl,
                AttrId::LongPassing,
                AttrId::Composure,
                // Sec (w=1)
                AttrId::Interceptions,
                AttrId::StandingTackle,
                AttrId::Reactions,
                AttrId::Strength,
                AttrId::LongShots,
                AttrId::Aggression,
            ],
            // CB: all attrs with position weight > 0 (note: SlidingTackle is Imp w=2).
            Self::Defender => vec![
                // Key (w=3)
                AttrId::Marking,
                AttrId::StandingTackle,
                AttrId::Heading,
                // Imp (w=2)
                AttrId::Strength,
                AttrId::Jumping,
                AttrId::Interceptions,
                AttrId::SlidingTackle,
                // Sec (w=1)
                AttrId::Composure,
                AttrId::Bravery,
                AttrId::Reactions,
                AttrId::Aggression,
                AttrId::ShortPassing,
                AttrId::Acceleration,
            ],
        }
    }

    // 5 display attrs shown in the attribute timeline and season log.
    fn display_attrs(self) -> [(&'static str, &'static str, AttrId); 5] {
        // (long label for timeline, short 4-char label for log, AttrId)
        match self {
            Self::Forward => [
                ("Finishing", "Fin ", AttrId::Finishing),
                ("Att.Pos.", "Att ", AttrId::AttPositioning),
                ("BallCtrl", "BCo ", AttrId::BallControl),
                ("Composure", "Com ", AttrId::Composure),
                ("Acceleratn", "Acc ", AttrId::Acceleration),
            ],
            Self::Midfielder => [
                ("Sh.Pass", "ShP ", AttrId::ShortPassing),
                ("Vision", "Vis ", AttrId::Vision),
                ("BallCtrl", "BCo ", AttrId::BallControl),
                ("Composure", "Com ", AttrId::Composure),
                ("Stamina", "Sta ", AttrId::Stamina),
            ],
            Self::Defender => [
                ("Marking", "Mrk ", AttrId::Marking),
                ("Heading", "Hea ", AttrId::Heading),
                ("Strength", "Str ", AttrId::Strength),
                ("Jumping", "Jmp ", AttrId::Jumping),
                ("Acceleratn", "Acc ", AttrId::Acceleration),
            ],
        }
    }

    // (goal_attr, range_max) where probability = attr_val / (range_max + 1).
    // FW Fin 90 → ~22%  MF Fin 70 → ~10%  CB Hea 80 → ~7%
    fn goal_roll(self) -> (AttrId, u32) {
        match self {
            Self::Forward => (AttrId::Finishing, 399),
            Self::Midfielder => (AttrId::Finishing, 699),
            Self::Defender => (AttrId::Heading, 1199),
        }
    }

    // (key1, key2, support, min1, min2, min_sup) for star-seed scan.
    fn scan_criteria(self) -> (AttrId, AttrId, AttrId, i32, i32, i32) {
        match self {
            Self::Forward => (
                AttrId::Finishing,
                AttrId::AttPositioning,
                AttrId::BallControl,
                80,
                80,
                62,
            ),
            Self::Midfielder => (
                AttrId::ShortPassing,
                AttrId::Vision,
                AttrId::BallControl,
                80,
                78,
                70,
            ),
            Self::Defender => (
                AttrId::Marking,
                AttrId::Heading,
                AttrId::Strength,
                78,
                75,
                72,
            ),
        }
    }
}

// ── Season snapshot ───────────────────────────────────────────────────────────

struct SeasonSnap {
    year: u32,
    age: u32,
    ovr: i32,
    attrs: [i32; 5], // position-specific display attrs
    form: i32,
    table_pos: usize,
    goals: u32,
    assists: u32,
    matches: u32,
}

// ── Star-seed scan ────────────────────────────────────────────────────────────

fn scan_star_seed(limit: u64, sim_pos: SimPos) -> Option<u64> {
    let (k1, k2, sup, min1, min2, min_sup) = sim_pos.scan_criteria();
    let choices = CreationChoices {
        name: "Prospect".into(),
        primary_position: sim_pos.to_position(),
        nationality: "England".to_string(),
        club: "Leeds United".to_string(),
    };
    let mut best: Option<(u64, i32)> = None;
    for seed in 0..limit {
        let p = generate_player(seed, &choices);
        let v1 = p.potential[k1 as usize].to_int();
        let v2 = p.potential[k2 as usize].to_int();
        let vs = p.potential[sup as usize].to_int();
        if v1 >= min1 && v2 >= min2 && vs >= min_sup {
            let score = v1 + v2 + vs;
            if best.is_none_or(|(_, s)| score > s) {
                best = Some((seed, score));
            }
        }
    }
    best.map(|(seed, _)| seed)
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Parse --position (default: Forward)
    let sim_pos: SimPos = args
        .iter()
        .position(|a| a == "--position")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| SimPos::from_str(s))
        .unwrap_or(SimPos::Forward);

    let display_attrs = sim_pos.display_attrs();

    // --lifestyle <professional|balanced|flashy> (default: balanced=1)
    let lifestyle: u8 = args
        .iter()
        .position(|a| a == "--lifestyle")
        .and_then(|i| args.get(i + 1))
        .map(|s| match s.to_lowercase().as_str() {
            "professional" | "pro" | "0" => 0,
            "flashy" | "toxic" | "2" => 2,
            _ => 1,
        })
        .unwrap_or(1);

    // --intensity <low|medium|high> (default: high)
    let cli_intensity: Intensity = args
        .iter()
        .position(|a| a == "--intensity")
        .and_then(|i| args.get(i + 1))
        .map(|s| match s.to_lowercase().as_str() {
            "low" => Intensity::Low,
            "medium" | "med" => Intensity::Medium,
            _ => Intensity::High,
        })
        .unwrap_or(Intensity::High);

    // --match-beats [seed] [opp_str] — play ONE match and print the beat-by-beat narrative.
    if args.iter().any(|a| a == "--match-beats") {
        let p = args.iter().position(|a| a == "--match-beats").unwrap();
        let seed: u64 = args.get(p + 1).and_then(|s| s.parse().ok()).unwrap_or(7);
        let opp_str: u8 = args.get(p + 2).and_then(|s| s.parse().ok()).unwrap_or(78);
        use goat_match::beats::ScoreEvent;

        let lib = BeatLibrary::load(BEATS_JSON).expect("beats.json must parse");
        let choices = CreationChoices {
            name: "Striker".into(),
            primary_position: PrimaryPosition::ST,
            nationality: "Brazilian".to_string(),
            club: "Riverside Town".to_string(),
        };
        let pl = generate_player(seed, &choices);
        let aggression = pl.current[AttrId::Aggression as usize]
            .to_int()
            .clamp(1, 99) as u8;
        let match_seed = seed ^ 0xc0ffee;
        let mut rp_rng = GoatRng::new(match_seed ^ 0xBADCAFE);
        let setup = MatchSetup {
            player_role: RoleId::CompleteForward,
            player_attrs: pl.current,
            player_familiarity: pl.familiarity,
            own_strength: 75,
            opp_strength: opp_str,
            opp_name: "Rivals FC".to_string(),
            form: Fixed::from_int(65),
            player_aggression: aggression,
            ref_personality: RefPersonality::from_rng(&mut rp_rng),
            dirty_rep: 50,
            player_traits: PlayerTraits::default(),
        };
        let r = auto_play_match(&lib, setup, &mut GoatRng::new(match_seed));

        println!(
            "MATCH — Striker (OVR {}) vs Rivals FC (str {opp_str})   seed {seed}\n",
            ovr(&pl.current, pl.primary_position).to_int()
        );
        for m in &r.moments {
            let tag = match m.goal_event {
                Some(ScoreEvent::GoalFor) => " ⚽ GOAL!",
                Some(ScoreEvent::AssistFor) => " 🅰 ASSIST!",
                Some(ScoreEvent::GoalAgainst) => " (they score)",
                _ if m.success => " ✓",
                _ => " ✗",
            };
            println!("  {:>2}'  {}", m.minute, m.setup_text);
            println!("        → {}{}", m.outcome_text, tag);
            println!();
        }
        let res = match r.goals_for.cmp(&r.goals_against) {
            std::cmp::Ordering::Greater => "WIN",
            std::cmp::Ordering::Less => "LOSS",
            std::cmp::Ordering::Equal => "DRAW",
        };
        let cards = if r.red_card {
            format!("{}Y + RED", r.yellow_cards)
        } else {
            format!("{}Y", r.yellow_cards)
        };
        let player_goals = r
            .moments
            .iter()
            .filter(|m| matches!(m.goal_event, Some(ScoreEvent::GoalFor)))
            .count();
        let player_assists = r
            .moments
            .iter()
            .filter(|m| matches!(m.goal_event, Some(ScoreEvent::AssistFor)))
            .count();
        println!("  {}", "─".repeat(52));
        println!(
            "  FULL TIME  {}-{}  {res}   |   your rating {}   goals {player_goals}   assists {player_assists}   cards {cards}",
            r.goals_for, r.goals_against, r.player_output
        );
        return;
    }

    // --season-beats [seed] — simulate ONE full season where the PC's matches are played
    // through the beat engine (not the team-strength shortcut). Match-by-match log.
    if args.iter().any(|a| a == "--season-beats") {
        let p = args.iter().position(|a| a == "--season-beats").unwrap();
        let seed: u64 = args.get(p + 1).and_then(|s| s.parse().ok()).unwrap_or(7);
        use goat_match::beats::ScoreEvent;

        let lib = BeatLibrary::load(BEATS_JSON).expect("beats.json must parse");
        let world = WorldGenesis::generate(seed);
        // Second tier of the first generated nation — analogous to the old hardcoded
        // England Championship slot (DIV_ENG_SEC).
        let div_idx = 1;
        let div_clubs = world.leagues[div_idx].clubs.clone();
        let pc_club_id = div_clubs[0];
        let choices = CreationChoices {
            name: "Tung".into(),
            primary_position: PrimaryPosition::ST,
            nationality: "England".to_string(),
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
                facilities_mult: world.clubs[pc_club_id].facilities_mult(),
                initial_table: Box::new([0u32; 100]),
            },
            &mut GoatRng::new(0),
        );
        let routine = Routine {
            focus_attrs: SimPos::Forward.focus_attrs(),
            intensity: Intensity::High,
        };
        state = reduce(state, Intent::SetRoutine { routine }, &mut GoatRng::new(0));
        let fixtures = build_season_orbit_fixtures(seed, 1, div_idx, &div_clubs, pc_club_id);
        state = reduce(
            state,
            Intent::StartSeason { fixtures },
            &mut GoatRng::new(0),
        );
        let pc_id = state.pc_player_id.unwrap();

        println!(
            "BEAT SEASON — {} (seed {seed})\n",
            world.clubs[pc_club_id].name
        );
        println!("  Rd  Opponent           Score  Res  Out  Gls  Cards");
        println!("  {}", "─".repeat(52));

        let (mut tot_out, mut tot_goals, mut tot_assists, mut min_o, mut max_o) =
            (0i64, 0u32, 0u32, 100i32, 0i32);
        let (mut w, mut d, mut l, mut yel, mut red, mut played) = (0, 0, 0, 0u32, 0u32, 0u32);

        for round in 0..ROUNDS_PER_SEASON {
            for t in 0u64..2 {
                let age = state.players.get_age_weeks(pc_id);
                let rng_seed = age as u64 ^ (round as u64 * 7) ^ t;
                state = reduce(state, Intent::AdvanceWeek, &mut GoatRng::new(rng_seed));
            }

            let all_fixtures = round_fixtures(seed, 1, div_idx, &div_clubs, round);
            let pc_fix = all_fixtures
                .iter()
                .find(|f| f.home == pc_club_id || f.away == pc_club_id);
            let Some(fx) = pc_fix else { continue };
            let opp = if fx.home == pc_club_id {
                fx.away
            } else {
                fx.home
            };

            let view = state.players.snapshot(pc_id);
            let aggression = view.current[AttrId::Aggression as usize]
                .to_int()
                .clamp(1, 99) as u8;
            let match_seed = seed ^ ((round as u64) << 16) ^ 0xc0ffee;
            let mut rp_rng = GoatRng::new(match_seed ^ 0xBADCAFE);
            let setup = MatchSetup {
                player_role: RoleId::CompleteForward,
                player_attrs: view.current,
                player_familiarity: view.familiarity,
                own_strength: world.clubs[pc_club_id].strength,
                opp_strength: world.clubs[opp].strength,
                opp_name: world.clubs[opp].name.clone(),
                form: state.pc_form,
                player_aggression: aggression,
                ref_personality: RefPersonality::from_rng(&mut rp_rng),
                dirty_rep: state.pc_discipline_rep,
                player_traits: PlayerTraits::default(),
            };
            let r = auto_play_match(&lib, setup, &mut GoatRng::new(match_seed));

            state = reduce(
                state,
                Intent::ApplyMatchResult {
                    familiarity_xp: r.familiarity_xp,
                    energy_cost: Fixed::from_int(25),
                    injury_weeks: None,
                },
                &mut GoatRng::new(0),
            );
            if r.yellow_cards > 0 || r.red_card {
                state = reduce(
                    state,
                    Intent::ApplyCardResult {
                        competition_id: LEAGUE_COMPETITION_ID,
                        yellow_cards: r.yellow_cards as u32,
                        red_card: r.red_card,
                    },
                    &mut GoatRng::new(0),
                );
            }

            let goals = r
                .moments
                .iter()
                .filter(|m| matches!(m.goal_event, Some(ScoreEvent::GoalFor)))
                .count() as u32;
            let assists = r
                .moments
                .iter()
                .filter(|m| matches!(m.goal_event, Some(ScoreEvent::AssistFor)))
                .count() as u32;
            let (gf, ga) = (r.goals_for, r.goals_against);
            let res_int: i8 = match gf.cmp(&ga) {
                std::cmp::Ordering::Greater => 1,
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
            };
            let res = match res_int {
                1 => {
                    w += 1;
                    "W"
                }
                -1 => {
                    l += 1;
                    "L"
                }
                _ => {
                    d += 1;
                    "D"
                }
            };

            // Feed the league table: PC's real result + the rest sim'd by strength.
            let mut sim_rng = GoatRng::new(seed ^ (round as u64) ^ 0xfeed);
            let mut round_results: Vec<(u8, u8, u32, u32)> = Vec::new();
            for f in &all_fixtures {
                let (rgf, rga) = if f.home == pc_club_id {
                    (gf, ga)
                } else if f.away == pc_club_id {
                    (ga, gf)
                } else {
                    sim_team_match(
                        world.clubs[f.home].strength,
                        world.clubs[f.away].strength,
                        &mut sim_rng,
                    )
                };
                let h = div_clubs.iter().position(|&c| c == f.home).unwrap() as u8;
                let a = div_clubs.iter().position(|&c| c == f.away).unwrap() as u8;
                round_results.push((h, a, rgf, rga));
            }
            state = reduce(
                state,
                Intent::ApplyRoundResult {
                    competition_id: LEAGUE_COMPETITION_ID,
                    pc_goals: goals,
                    pc_assists: assists,
                    pc_output: r.player_output,
                    pc_result: res_int,
                    round_results,
                    rest_weeks: rest_weeks_after_round(round),
                    week_ends: week_ends_after_round(round),
                },
                &mut GoatRng::new(0),
            );

            let cards = if r.red_card {
                format!("{}Y 🟥", r.yellow_cards)
            } else if r.yellow_cards > 0 {
                format!("{}Y", r.yellow_cards)
            } else {
                "—".into()
            };
            println!(
                "  {:>2}  {:<18} {:>2}-{:<2}  {:^3}  {:>3}  {:>3}  {}",
                round + 1,
                world.clubs[opp].name,
                gf,
                ga,
                res,
                r.player_output,
                goals,
                cards
            );

            played += 1;
            tot_out += r.player_output as i64;
            tot_goals += goals;
            tot_assists += assists;
            min_o = min_o.min(r.player_output);
            max_o = max_o.max(r.player_output);
            yel += r.yellow_cards as u32;
            red += u32::from(r.red_card);
        }

        let table = Table::from_raw(&state.table_raw, &div_clubs);
        let pos = table.position_of(pc_club_id);
        let avg = if played > 0 {
            tot_out / played as i64
        } else {
            0
        };
        println!("  {}", "─".repeat(52));
        println!("\n  SEASON SUMMARY");
        println!(
            "  Played {played}  W{w} D{d} L{l}  |  Goals {tot_goals}  Assists {tot_assists}  |  League position {pos}/{}",
            div_clubs.len()
        );
        println!("  Output: avg {avg}  min {min_o}  max {max_o}   Cards: {yel}Y {red}R");

        // ── Final league table (the record table) ────────────────────────────
        println!("\n  FINAL TABLE — {}", world.leagues[div_idx].name);
        println!(
            "  {:<3} {:<18} {:>2} {:>2} {:>2} {:>2} {:>3} {:>3} {:>4} {:>3}",
            "#", "Club", "P", "W", "D", "L", "GF", "GA", "GD", "Pts"
        );
        println!("  {}", "─".repeat(54));
        for (i, e) in table.sorted().iter().enumerate() {
            let mark = if e.club_id == pc_club_id { "▶" } else { " " };
            println!(
                "{}{:>2} {:<18} {:>2} {:>2} {:>2} {:>2} {:>3} {:>3} {:>+4} {:>3}",
                mark,
                i + 1,
                world.clubs[e.club_id].name,
                e.played(),
                e.w,
                e.d,
                e.l,
                e.gf,
                e.ga,
                e.goal_diff(),
                e.points()
            );
        }
        println!(
            "\n  (matches played beat-by-beat through the engine; output is the player's own\n   game, the scoreline is the team's — they can diverge.)"
        );
        return;
    }

    // --match-sim [N] [seed] — run N beat-engine matches and analyse output vs result.
    if args.iter().any(|a| a == "--match-sim") {
        let pos = args.iter().position(|a| a == "--match-sim").unwrap();
        let n: u32 = args
            .get(pos + 1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(100);
        let seed: u64 = args.get(pos + 2).and_then(|s| s.parse().ok()).unwrap_or(7);

        let lib = BeatLibrary::load(BEATS_JSON).expect("beats.json must parse");
        let choices = CreationChoices {
            name: "Striker".into(),
            primary_position: PrimaryPosition::ST,
            nationality: "Brazilian".to_string(),
            club: "Riverside Town".to_string(),
        };
        let p = generate_player(seed, &choices);
        let player_ovr = ovr(&p.current, p.primary_position).to_int();
        let aggression = p.current[AttrId::Aggression as usize].to_int().clamp(1, 99) as u8;
        let own_strength = 70u8;

        // Accumulators.
        let (mut sum_out, mut min_out, mut max_out) = (0i64, 100i32, 0i32);
        let mut buckets = [0u32; 4]; // <40, 40-59, 60-79, 80-100
        let (mut w, mut d, mut l) = (0u32, 0u32, 0u32);
        let (mut gf_sum, mut ga_sum, mut yellows, mut reds) = (0u64, 0u64, 0u64, 0u32);
        let (mut out_w, mut out_d, mut out_l) = (0i64, 0i64, 0i64);
        let (mut starred_in_defeat, mut carried_to_win) = (0u32, 0u32);

        for i in 0..n {
            let match_seed = seed ^ (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let mut rng = GoatRng::new(match_seed);
            // Opponents span the full strength spectrum so results vary.
            let opp_strength = (45 + rng.next_range_u32(0, 45)) as u8;
            let mut rp_rng = GoatRng::new(match_seed ^ 0xBADCAFE);
            let setup = MatchSetup {
                player_role: RoleId::CompleteForward,
                player_attrs: p.current,
                player_familiarity: p.familiarity,
                own_strength,
                opp_strength,
                opp_name: "Rivals FC".to_string(),
                form: Fixed::from_int(60),
                player_aggression: aggression,
                ref_personality: RefPersonality::from_rng(&mut rp_rng),
                dirty_rep: 50,
                player_traits: PlayerTraits::default(),
            };
            let r = auto_play_match(&lib, setup, &mut GoatRng::new(match_seed));

            let out = r.player_output;
            sum_out += out as i64;
            min_out = min_out.min(out);
            max_out = max_out.max(out);
            buckets[match out {
                o if o < 40 => 0,
                o if o < 60 => 1,
                o if o < 80 => 2,
                _ => 3,
            }] += 1;
            gf_sum += r.goals_for as u64;
            ga_sum += r.goals_against as u64;
            yellows += r.yellow_cards as u64;
            reds += u32::from(r.red_card);

            match r.goals_for.cmp(&r.goals_against) {
                std::cmp::Ordering::Greater => {
                    w += 1;
                    out_w += out as i64;
                    if out <= 45 {
                        carried_to_win += 1;
                    }
                }
                std::cmp::Ordering::Equal => {
                    d += 1;
                    out_d += out as i64;
                }
                std::cmp::Ordering::Less => {
                    l += 1;
                    out_l += out as i64;
                    if out >= 70 {
                        starred_in_defeat += 1;
                    }
                }
            }
        }

        let nf = n as i64;
        let avg = |s: i64, c: u32| if c > 0 { s / c as i64 } else { 0 };
        println!("MATCHSIM player_ovr={player_ovr} matches={n} own_str={own_strength}");
        println!(
            "OUTPUT min={min_out} max={max_out} avg={}  buckets <40={} 40-59={} 60-79={} 80+={}",
            sum_out / nf,
            buckets[0],
            buckets[1],
            buckets[2],
            buckets[3]
        );
        println!("RESULTS W={w} D={d} L={l}");
        println!(
            "TEAMGOALS avg_for={} avg_against={}  cards yellow={yellows} red={reds}",
            gf_sum / n as u64,
            ga_sum / n as u64
        );
        println!(
            "OUTPUT_BY_RESULT W={} D={} L={}",
            avg(out_w, w),
            avg(out_d, d),
            avg(out_l, l)
        );
        println!(
            "DECOUPLING starred_in_defeat(out>=70 & L)={starred_in_defeat}  carried_to_win(out<=45 & W)={carried_to_win}"
        );
        return;
    }

    // --genesis [seed] — dump the SoA population fingerprint (Phase 9 Slice 9A.1 gate).
    if args.iter().any(|a| a == "--genesis") {
        let seed: u64 = args
            .iter()
            .position(|a| a == "--genesis")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(42);
        let world = WorldGenesis::generate(seed);
        let pop = goat_world::population::genesis(seed, &world);
        println!(
            "GENESIS seed={seed}  players={}  fingerprint=0x{:016x}",
            pop.len(),
            pop.fingerprint()
        );
        return;
    }

    // --history [seed] — dump the seeded canon of past greats (Phase 9 Slice 9A.4 gate).
    if args.iter().any(|a| a == "--history") {
        let seed: u64 = args
            .iter()
            .position(|a| a == "--history")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(42);
        let world = WorldGenesis::generate(seed);
        let h = goat_world::history::backfill_history(seed, 30, &world);
        println!("PANTHEON CANON (seed {seed}, 30 backfilled seasons)");
        println!(
            "  {:<22} {:<10} {:>4}  {:>4}",
            "Name", "Nation", "BdOr", "Peak"
        );
        println!("  {}", "─".repeat(46));
        for g in h.canon_ranked().iter().take(8) {
            println!(
                "  {:<22} {:<10} {:>4}  {:>4}",
                g.name,
                goat_world::history::great_nation_name(g.nationality, &world),
                g.ballon_dors,
                g.peak_ovr
            );
        }
        return;
    }

    // --rival [seed] — crystallise the emergent rival vs a representative PC (Slice 9A.5).
    if args.iter().any(|a| a == "--rival") {
        let seed: u64 = args
            .iter()
            .position(|a| a == "--rival")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(42);
        let world = WorldGenesis::generate(seed);
        let league_clubs = world.static_league_clubs();
        let mut pop = goat_world::population::genesis(seed, &world);
        for s in 1..=14u32 {
            goat_world::batch_tick::batch_tick_season(
                &mut pop,
                &world,
                &league_clubs,
                seed,
                s,
                s * 52,
            );
        }
        // A representative mid-tier PC career to set the bar.
        match goat_world::rival::crystallise_rival(&pop, 16 * 52, 200, 5) {
            goat_world::rival::RivalVerdict::Rival {
                name,
                peer_goals,
                peer_titles,
                ..
            } => println!(
                "RIVAL (seed {seed}): your generation produced {name} — {peer_goals} goals, {peer_titles} titles. The media frames the rivalry.",
            ),
            goat_world::rival::RivalVerdict::WeakEra => println!(
                "RIVAL (seed {seed}): nobody kept pace. You reign alone — the schools apply the weak-era asterisk.",
            ),
        }
        return;
    }

    // --world-sim [seed] [seasons] — simulate the WHOLE world headless and summarise.
    if args.iter().any(|a| a == "--world-sim") {
        let pos = args.iter().position(|a| a == "--world-sim").unwrap();
        let seed: u64 = args.get(pos + 1).and_then(|s| s.parse().ok()).unwrap_or(42);
        let seasons: u32 = args.get(pos + 2).and_then(|s| s.parse().ok()).unwrap_or(20);
        // The PC career the rivalry is measured against (sets the "keep pace" bar). A
        // GOAT-tier PC makes the weak-era asterisk reachable when the cohort is thin.
        let pc_goals: u32 = args
            .get(pos + 3)
            .and_then(|s| s.parse().ok())
            .unwrap_or(300);
        let pc_titles: u32 = args.get(pos + 4).and_then(|s| s.parse().ok()).unwrap_or(8);

        let world = WorldGenesis::generate(seed);
        let league_clubs = world.static_league_clubs();
        let mut pop = goat_world::population::genesis(seed, &world);
        for s in 1..=seasons {
            goat_world::batch_tick::batch_tick_season(
                &mut pop,
                &world,
                &league_clubs,
                seed,
                s,
                s * 52,
            );
        }

        // All-time top scorer of the run.
        let top = (0..pop.len()).max_by_key(|&i| pop.career_goals[i]).unwrap();
        // Most-titled club (sum of its squad's titles).
        let mut club_titles = vec![0u32; world.clubs.len()];
        for i in 0..pop.len() {
            club_titles[pop.club[i] as usize] += pop.career_titles[i];
        }
        let top_club = (0..world.clubs.len())
            .max_by_key(|&c| club_titles[c])
            .unwrap();
        // Pantheon GOAT from the backfilled canon.
        let hist = goat_world::history::backfill_history(seed, 30, &world);
        let goat = hist.canon_ranked()[0];
        // Emergent rival vs a representative mid-tier PC.
        let rival = match goat_world::rival::crystallise_rival(&pop, 16 * 52, pc_goals, pc_titles) {
            goat_world::rival::RivalVerdict::Rival {
                name, peer_goals, ..
            } => {
                format!("RIVAL name=\"{name}\" goals={peer_goals}")
            }
            goat_world::rival::RivalVerdict::WeakEra => "WEAKERA".to_string(),
        };

        println!("WORLD seed={seed} players={} seasons={seasons}", pop.len());
        println!(
            "WORLD topscorer=\"{}\" goals={} club=\"{}\"",
            goat_world::history::name_from_seed(pop.seed[top]),
            pop.career_goals[top],
            world.clubs[pop.club[top] as usize].name
        );
        println!(
            "WORLD mosttitles_club=\"{}\" titles={}",
            world.clubs[top_club].name, club_titles[top_club]
        );
        println!(
            "WORLD pantheon_goat=\"{}\" ballondors={} peak={}",
            goat.name, goat.ballon_dors, goat.peak_ovr
        );
        println!("WORLD rival={rival}");
        return;
    }

    // --scan mode
    if args.iter().any(|a| a == "--scan") {
        let (k1, k2, sup, ..) = sim_pos.scan_criteria();
        let d = display_attrs;
        println!(
            "{:>8}  {:>5}  {:>5}  {:>5}  note",
            "Seed",
            d[0].1.trim(),
            d[1].1.trim(),
            d[2].1.trim()
        );
        println!("{}", "─".repeat(40));
        let choices = CreationChoices {
            name: "X".into(),
            primary_position: sim_pos.to_position(),
            nationality: "England".to_string(),
            club: "Leeds United".to_string(),
        };
        let (_, _, _, min1, min2, min_sup) = sim_pos.scan_criteria();
        let mut hits: Vec<(u64, i32, i32, i32, i32)> = Vec::new();
        for seed in 0u64..500 {
            let p = generate_player(seed, &choices);
            let v1 = p.potential[k1 as usize].to_int();
            let v2 = p.potential[k2 as usize].to_int();
            let vs = p.potential[sup as usize].to_int();
            if v1 >= min1 && v2 >= min2 && vs >= min_sup {
                hits.push((seed, v1, v2, vs, v1 + v2 + vs));
            }
        }
        hits.sort_by_key(|h| -h.4);
        for (seed, v1, v2, vs, _) in hits.iter().take(20) {
            let star = if *v1 >= 90 && *v2 >= 90 {
                " ★★★"
            } else if *v1 >= 85 {
                " ★★"
            } else {
                " ★"
            };
            println!("{seed:>8}  {v1:>5}  {v2:>5}  {vs:>5}{star}");
        }
        return;
    }

    // --seed N or auto-scan
    let seed: u64 = if let Some(pos) = args.iter().position(|a| a == "--seed") {
        args.get(pos + 1).and_then(|s| s.parse().ok()).unwrap_or(42)
    } else {
        print!("  Scanning for a star {}", sim_pos.label());
        let found = scan_star_seed(500, sim_pos);
        println!(" → found seed {:?}", found);
        found.unwrap_or(42)
    };

    let world = WorldGenesis::generate(seed);
    // Second tier of the first generated nation — analogous to the old hardcoded
    // England Championship slot (DIV_ENG_SEC).
    let div_idx = 1;
    let pc_club_id = world.leagues[div_idx].clubs[0];

    let choices = CreationChoices {
        name: "Tung".into(),
        primary_position: sim_pos.to_position(),
        nationality: "England".to_string(),
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
            facilities_mult: world.clubs[pc_club_id].facilities_mult(),
            initial_table: Box::new([0u32; 100]),
        },
        &mut GoatRng::new(0),
    );

    // Lifestyle is now a derived readout (bible §8.6), not a settable intent — this
    // harness still wants to force a starting tier for long-horizon comparisons, so it
    // seeds the underlying score directly at the extremes/centre of the tier band.
    state.pc_lifestyle_score = match lifestyle {
        0 => Fixed::raw(-1_000),
        2 => Fixed::raw(1_000),
        _ => Fixed::ZERO,
    };
    state.pc_lifestyle = lifestyle.min(2);

    let routine = Routine {
        focus_attrs: sim_pos.focus_attrs(),
        intensity: cli_intensity,
    };
    state = reduce(state, Intent::SetRoutine { routine }, &mut GoatRng::new(0));

    let pc_id = state.pc_player_id.unwrap();
    let initial_view = state.players.snapshot(pc_id);
    let initial_ovr = ovr(&initial_view.current, initial_view.primary_position).to_int();

    // Track per-attr peaks across all seasons for the stat development panel.
    let mut peak_attrs: Vec<i32> = (0..NUM_ATTRS)
        .map(|a| initial_view.current[a].to_int())
        .collect();

    let (goal_attr_id, goal_range_max) = sim_pos.goal_roll();

    let mut snaps: Vec<SeasonSnap> = Vec::new();
    let mut retired_at: Option<u32> = None;
    let mut injured_weeks: u32 = 0; // weeks spent injured across the career

    // ── 20-season simulation ───────────────────────────────────────────────────
    for season in 1u32..=20 {
        // pc_div_idx/pc_club_idx already reflect the club for the season about to
        // start (set by the previous iteration's promotion/relegation, or by InitWorld
        // for season 1) — compute the fixtures before StartSeason resets season state.
        let season_div_idx = state.pc_div_idx as usize;
        let season_pc_club = state.pc_club_idx as usize;
        let div_clubs = world.leagues[season_div_idx].clubs.clone();
        let fixtures =
            build_season_orbit_fixtures(seed, season, season_div_idx, &div_clubs, season_pc_club);
        state = reduce(
            state,
            Intent::StartSeason { fixtures },
            &mut GoatRng::new(0),
        );

        for round in 0..ROUNDS_PER_SEASON {
            for t in 0u64..2 {
                let age = state.players.get_age_weeks(pc_id);
                let rng_seed = age as u64 ^ (season as u64 * 0xbeef) ^ (round as u64 * 7) ^ t;
                state = reduce(state, Intent::AdvanceWeek, &mut GoatRng::new(rng_seed));
                if state.players.get_injury_weeks(pc_id) > 0 {
                    injured_weeks += 1;
                }
            }

            let all_fixtures = round_fixtures(seed, season, season_div_idx, &div_clubs, round);
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
                let h_pos = div_clubs.iter().position(|&c| c == f.home).unwrap() as u8;
                let a_pos = div_clubs.iter().position(|&c| c == f.away).unwrap() as u8;
                round_results.push((h_pos, a_pos, gf, ga));
                if f.home == season_pc_club {
                    pc_gf = gf;
                    pc_ga = ga;
                } else if f.away == season_pc_club {
                    pc_gf = ga;
                    pc_ga = gf;
                }
            }

            let pc_output = match fixture_for_round(
                seed,
                season,
                season_div_idx,
                &div_clubs,
                season_pc_club,
                round,
            ) {
                Some(_) => {
                    let form_val = state.pc_form.to_int() as u64;
                    let mut out_rng =
                        GoatRng::new(seed ^ (season as u64 * 0xdead) ^ (round as u64));
                    let variance = out_rng.next_range_u64(0, 20) as i32 - 10;
                    ((form_val as i32 + variance).clamp(0, 100)) as i32
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
                    competition_id: LEAGUE_COMPETITION_ID,
                    pc_goals: player_goals,
                    // Batch path rolls goals from attrs without beat moments, so
                    // there is no assist source here — assists stay 0 in this mode.
                    pc_assists: 0,
                    pc_output,
                    pc_result,
                    round_results,
                    rest_weeks: rest_weeks_after_round(round),
                    week_ends: week_ends_after_round(round),
                },
                &mut GoatRng::new(0),
            );
        }

        let view = state.players.snapshot(pc_id);
        let cur_ovr = ovr(&view.current, view.primary_position).to_int();
        let age_years = view.age_weeks / 52;
        let table = Table::from_raw(&state.table_raw, &div_clubs);
        let table_pos = table.position_of(season_pc_club);

        // Update per-attr peaks.
        for (a, peak) in peak_attrs.iter_mut().enumerate() {
            let cur = view.current[a].to_int();
            if cur > *peak {
                *peak = cur;
            }
        }

        let snap_attrs: [i32; 5] =
            std::array::from_fn(|i| view.current[display_attrs[i].2 as usize].to_int());

        snaps.push(SeasonSnap {
            year: BASE_CAREER_YEAR + season - 1,
            age: age_years,
            ovr: cur_ovr,
            attrs: snap_attrs,
            form: state.pc_form.to_int(),
            table_pos,
            goals: state.pc_season_goals,
            assists: state.pc_season_assists,
            matches: state.pc_season_matches,
        });

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
        let s_assists = state.pc_season_assists;
        let s_matches = state.pc_season_matches;
        let s_standout_matches = state.pc_season_standout_matches;
        let s_transfer_requests = state.pc_season_transfer_requests;

        state = reduce(
            state,
            Intent::ApplySeasonEndLegacy {
                season_goals: s_goals,
                season_assists: s_assists,
                season_matches: s_matches,
                season_output_sum,
                won_title,
                player_of_year: season_avg > 75,
                finish_position: table_pos as u32,
                decisive_moments: 0,
                new_sporting_rep,
                new_club_fan_rep,
                season_standout_matches: s_standout_matches,
                season_transfer_requests: s_transfer_requests,
                season_caps: 0,
                season_international_goals: 0,
                season_world_cups_won: 0,
                season_continental_championships_won: 0,
            },
            &mut GoatRng::new(0),
        );

        if age_years >= 35 && cur_ovr < 50 {
            retired_at = Some(age_years);
            break;
        }
    }

    // Verify invariants
    let mut all_ok = true;
    for a in 0..NUM_ATTRS {
        let cur = state.players.get_current(pc_id, a);
        let pot = state.players.get_potential(pc_id, a);
        if cur > pot || cur < Fixed::MIN_ATTR {
            all_ok = false;
        }
    }

    let final_view = state.players.snapshot(pc_id);
    let n = snaps.len();

    // ══════════════════════════════════════════════════════════════════════════
    // RENDER FULL CAREER PORTRAIT
    // ══════════════════════════════════════════════════════════════════════════

    let w = 82;
    let bar = "═".repeat(w);

    println!("\n╔{bar}╗");
    println!("║{:^width$}║", "CAREER PORTRAIT", width = w);
    println!(
        "║{:^width$}║",
        format!(
            "{}  ·  {}  ·  {}  ·  Seed {seed}",
            final_view.name,
            sim_pos.label(),
            world.clubs[pc_club_id].name
        ),
        width = w
    );
    println!("╚{bar}╝");

    // ── 1. OVR arc ────────────────────────────────────────────────────────────
    let ovr_vals: Vec<i32> = snaps.iter().map(|s| s.ovr).collect();
    let ovr_max = *ovr_vals.iter().max().unwrap_or(&50);
    let ovr_min = initial_ovr.min(*ovr_vals.iter().min().unwrap_or(&30));
    let chart_h = 8usize;
    let col_w = 4usize;

    println!(
        "\n  OVR PROGRESSION  ({} → peak {} → {})",
        initial_ovr,
        ovr_max,
        snaps.last().map_or(0, |s| s.ovr)
    );

    let phase_label = |age: u32| -> &'static str {
        if age <= 21 {
            "◆"
        } else if age <= 27 {
            "▲"
        } else if age <= 31 {
            "★"
        } else {
            "▼"
        }
    };

    for row in (0..=chart_h).rev() {
        let val = ovr_min + (row as i32) * (ovr_max - ovr_min + 2) / chart_h as i32;
        print!("  {:>2} │", val);
        for s in &snaps {
            let filled = (s.ovr - ovr_min) * chart_h as i32 / (ovr_max - ovr_min + 2);
            let ch = if filled as usize >= row {
                phase_label(s.age)
            } else {
                " "
            };
            print!("{ch:<col_w$}", ch = ch, col_w = col_w);
        }
        println!();
    }
    print!("     └");
    for _ in &snaps {
        print!("{}", "─".repeat(col_w));
    }
    println!();
    print!("      ");
    for s in &snaps {
        print!("{:<col_w$}", s.age, col_w = col_w);
    }
    println!("  age");
    println!("      ◆ youth (≤21)  ▲ rising (22–27)  ★ peak (28–31)  ▼ decline (32+)");

    // ── 2. Attribute timeline bars ────────────────────────────────────────────
    println!("\n  ATTRIBUTE TIMELINE  (each season → bar = value/99)");
    println!(
        "  {:<12}  0         20        40        60        80        99",
        ""
    );
    println!(
        "  {:<12}  ·─────────·─────────·─────────·─────────·─────────·",
        ""
    );

    for (attr_idx, (long_label, _, attr_id)) in display_attrs.iter().enumerate() {
        let pot = final_view.potential[*attr_id as usize].to_int();
        let mut prev = 0i32;
        for (i, s) in snaps.iter().enumerate() {
            let val = s.attrs[attr_idx];
            let bar_len = (val * 50 / 99) as usize;
            let age_m = phase_label(s.age);
            if i == 0 || val != prev {
                println!(
                    "  {long_label:<12}  {}{} {val:>2}  {age_m} {}",
                    "█".repeat(bar_len),
                    "░".repeat(50 - bar_len),
                    s.age
                );
                prev = val;
            } else if i == n - 1 {
                println!(
                    "  {:<12}  {}{} {val:>2}  {age_m} {} (final)",
                    " ".repeat(12),
                    "█".repeat(bar_len),
                    "░".repeat(50 - bar_len),
                    s.age
                );
            }
        }
        println!(
            "  {:<12}  ·─────────·─────────·─────────·─────────·─────────· ceiling: {pot}",
            " ".repeat(12)
        );
    }

    // ── 3. Form track ─────────────────────────────────────────────────────────
    println!("\n  FORM TRACK  (season average — higher = stronger performances)");
    let form_max = snaps.iter().map(|s| s.form).max().unwrap_or(70);
    for row in [form_max, form_max * 3 / 4, form_max / 2, form_max / 4, 0].iter() {
        print!("  {:>3} │", row);
        for s in &snaps {
            let marker = if s.form >= *row { "▓" } else { " " };
            print!("{:<4}", marker);
        }
        println!();
    }
    print!("      └");
    for _ in &snaps {
        print!("────");
    }
    println!();
    print!("      ");
    for s in &snaps {
        print!("{:<4}", s.age);
    }
    println!("  age");

    // ── 4. League position ────────────────────────────────────────────────────
    println!(
        "\n  LEAGUE POSITION  (1=champion, {}=bottom, ★=title)",
        goat_world::CLUBS_PER_DIV
    );
    for pos in [1usize, 5, 10, 15, goat_world::CLUBS_PER_DIV] {
        print!("  {:>2} │", pos);
        for s in &snaps {
            let marker = if s.table_pos == pos {
                if pos == 1 {
                    "★"
                } else {
                    "●"
                }
            } else if s.table_pos <= pos && s.table_pos > pos.saturating_sub(3) {
                "·"
            } else {
                " "
            };
            print!("{:<4}", marker);
        }
        println!();
    }
    print!("      └");
    for _ in &snaps {
        print!("────");
    }
    println!();
    print!("      ");
    for s in &snaps {
        print!("{:<4}", s.age);
    }
    println!("  age");

    // ── 5. Season-by-season table ─────────────────────────────────────────────
    println!("\n  SEASON LOG");
    print!("  {:>4}  {:>3}  {:>3}", "Year", "Age", "OVR");
    for (_, short, _) in &display_attrs {
        print!("  {:>4}", short.trim());
    }
    println!(
        "  {:>4}  {:>3}  {:>5}  {:>5}  {:>5}",
        "Form", "Pos", "Match", "Goals", "Ast"
    );
    println!("  {}", "─".repeat(74));

    for s in &snaps {
        let phase = phase_label(s.age);
        print!("  {:>4}  {:>3}  {:>3}", s.year, s.age, s.ovr);
        for &val in &s.attrs {
            print!("  {:>4}", val);
        }
        println!(
            "  {:>4}  {:>3}  {:>5}  {:>5}  {:>5}  {}",
            s.form, s.table_pos, s.matches, s.goals, s.assists, phase
        );
    }

    // ── 6. Stat development panel ────────────────────────────────────────────
    println!("\n  STAT DEVELOPMENT  (each trained attr: start → peak → final / ceiling)");
    println!(
        "  {:<16}  {:<3}  {:>5}  {:>5}  {:>5}  {:>7}  Arc",
        "Attribute", "W", "Start", "Peak", "Final", "Ceiling"
    );
    println!("  {}", "─".repeat(90));

    let focus = sim_pos.focus_attrs();
    let pos_weights = &POSITION_WEIGHT_TABLE[initial_view.primary_position as usize];

    for &attr_id in &focus {
        let a = attr_id as usize;
        let w = pos_weights[a];
        let tier = match w {
            3 => "Key",
            2 => "Imp",
            _ => "Sec",
        };
        let start = initial_view.current[a].to_int();
        let peak = peak_attrs[a];
        let final_val = final_view.current[a].to_int();
        let ceiling = final_view.potential[a].to_int();
        let status = if final_val >= ceiling {
            "✓ ceiling"
        } else if final_val < peak {
            "↓ declined"
        } else {
            "→ growing"
        };
        // Bar: ▓ = retained at final, ░ = lost from peak, · = below ceiling
        let bar_w = 40usize;
        let b_final = (final_val as usize * bar_w / 99).min(bar_w);
        let b_peak = (peak as usize * bar_w / 99).min(bar_w);
        let b_ceil = (ceiling as usize * bar_w / 99).min(bar_w);
        let bar: String = (0..bar_w)
            .map(|i| {
                if i < b_final {
                    '▓'
                } else if i < b_peak {
                    '░'
                }
                // peak that was lost (physical decay)
                else if i < b_ceil {
                    '·'
                }
                // potential not yet reached
                else {
                    ' '
                }
            })
            .collect();
        println!(
            "  {:<16}  {:<3}  {:>5}  {:>5}  {:>5} /{:>4}  [{}]  {}",
            format!("{:?}", attr_id),
            tier,
            start,
            peak,
            final_val,
            ceiling,
            bar,
            status
        );
    }

    println!();
    println!("\n╔{bar}╗");
    println!("║{:^width$}║", "CAREER VERDICT", width = w);
    println!("╠{bar}╣");

    let peak_ovr_season = snaps.iter().max_by_key(|s| s.ovr).unwrap();
    let best_form_season = snaps.iter().max_by_key(|s| s.form).unwrap();
    let titles: usize = snaps.iter().filter(|s| s.table_pos == 1).count();
    let career_goals: u32 = snaps.iter().map(|s| s.goals).sum();
    let career_assists: u32 = snaps.iter().map(|s| s.assists).sum();
    let career_apps: u32 = snaps.iter().map(|s| s.matches).sum();

    println!(
        "║  {:76} ║",
        format!(
            "Seasons played : {}  (retired age {})",
            n,
            retired_at.unwrap_or(snaps.last().map_or(0, |s| s.age))
        )
    );
    println!(
        "║  {:76} ║",
        format!(
            "Career apps    : {}  Goals: {}  Assists: {}",
            career_apps, career_goals, career_assists
        )
    );
    // Lifestyle is an emergent, weekly-nudged readout (bible §8.6) — report the actual
    // final tier (`state.pc_lifestyle`), not the CLI `--lifestyle` seed, which can and
    // does drift away within the first few seasons.
    let lifestyle_label = match state.pc_lifestyle {
        0 => "Professional",
        2 => "Flashy",
        _ => "Balanced",
    };
    println!(
        "║  {:76} ║",
        format!(
            "Lifestyle      : {}  Intensity: {}  Injured weeks: {}",
            lifestyle_label,
            cli_intensity.name(),
            injured_weeks
        )
    );
    println!(
        "║  {:76} ║",
        format!(
            "League titles  : {}  (positions: {})",
            titles,
            snaps
                .iter()
                .map(|s| s.table_pos.to_string())
                .collect::<Vec<_>>()
                .join("-")
        )
    );
    println!(
        "║  {:76} ║",
        format!(
            "Peak OVR       : {} (age {}, season {})",
            peak_ovr_season.ovr, peak_ovr_season.age, peak_ovr_season.year
        )
    );
    println!(
        "║  {:76} ║",
        format!(
            "Best form      : {} (age {}, season {})",
            best_form_season.form, best_form_season.age, best_form_season.year
        )
    );
    let ceiling_line: String = display_attrs
        .iter()
        .map(|(_, short, attr_id)| {
            format!(
                "{} {}",
                short.trim(),
                final_view.potential[*attr_id as usize].to_int()
            )
        })
        .collect::<Vec<_>>()
        .join("  ");
    println!(
        "║  {:76} ║",
        format!("Attribute ceiling reached: {}", ceiling_line)
    );
    println!(
        "║  {:76} ║",
        format!(
            "Invariants OK  : {}",
            if all_ok {
                "YES — cur ≤ pot, cur ≥ min throughout"
            } else {
                "VIOLATION DETECTED"
            }
        )
    );
    println!("╚{bar}╝");
}
