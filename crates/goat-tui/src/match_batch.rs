/// match-batch — headless batch runner for the Match Flow engine.
///
/// Runs N matches in-process (no subprocess spawns) across all 5 positions and
/// opponent strengths 60–95, then prints distribution stats:
///   1. W/D/L split + goals for/against histogram
///   2. player rating histogram
///   3. % of matches where the PC scores 2+ but the team still loses
///   4. average goals per match
///
/// Usage: match-batch [num_matches] [master_seed] [--m1-squad]
///
/// `--m1-squad` (measurement tool only, no game-logic change): derive
/// `own_profile` from the squad average WITH the PC occupying his position
/// slot — the M1 live-loop mechanism — instead of the fixed
/// `TacticalProfile::derive(75, …)`. Tests how much of the per-position
/// W/D/L gap (CBs lose far more than STs) comes from the PC's real attrs
/// lifting his own line vs. the position-independent fixed profile.
use goat_core::{
    attrs::{AttrId, NUM_ATTRS},
    generation::{generate_player, CreationChoices, Position},
    roles::RoleId,
    tactical::TacticalProfile,
};
use goat_fixed::Fixed;
use goat_match::{
    discipline::RefPersonality,
    sim::{auto_play_match, BeatLibrary, MatchSetup},
};
use goat_rng::{GoatRng, RngSource};
use goat_traits::PlayerTraits;

const BEATS_JSON: &str = include_str!("../../../beats.json");

const POSITIONS: [(&str, Position, RoleId); 5] = [
    ("ST", Position::Forward, RoleId::CompleteForward),
    ("W", Position::Midfielder, RoleId::Winger),
    ("CAM", Position::Midfielder, RoleId::AttackingMid),
    ("CM", Position::Midfielder, RoleId::CentralMid),
    ("CB", Position::Defender, RoleId::CentreBack),
];

/// Per-match record kept for outlier hunting (top scorelines / extreme ratings).
#[derive(Clone)]
struct Outlier {
    seed: u64,
    pos_idx: usize,
    opp_str: u8,
    goals_for: u32,
    goals_against: u32,
    rating: i32,
    pc_goals: u64,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(100_000);
    let master_seed: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0xBA7C4);
    let m1_squad = args.iter().any(|a| a == "--m1-squad");
    let lib = BeatLibrary::load(BEATS_JSON).expect("beats.json must parse");

    let mut rng = GoatRng::new(master_seed);

    let mut wins = 0u64;
    let mut draws = 0u64;
    let mut losses = 0u64;
    let mut gf_hist = [0u64; 11]; // 0..=9, 10 = "10+"
    let mut ga_hist = [0u64; 11];
    let mut rating_hist = [0u64; 10]; // buckets 0-9 .. 90-100
    let mut pc_goals_hist = [0u64; 6]; // 0,1,2,3,4,5+
    let mut brace_losses = 0u64; // PC 2+ goals, team loses
    let mut clean_sheets = 0u64; // goals_against == 0
                                 // Danger-man duels: matches with a danger man faced, duel totals,
                                 // and the Tùng scenario — keeping him quiet in a defeat.
    let mut danger_matches = 0u64;
    let mut danger_won = 0u64;
    let mut danger_lost = 0u64;
    let mut quiet_in_defeat = 0u64; // >=3 duels, won >= 60%, team lost
    let mut exposed_in_defeat = 0u64; // >=3 duels, lost >= 60%, team lost
                                      // Decoupling: output rating vs team result (match-sim.sh definitions).
    let mut starred_70 = 0u64; // rating >= 70, team loses
    let mut starred_80 = 0u64; // rating >= 80, team loses
    let mut carried_45 = 0u64; // rating <= 45, team wins
    let mut total_gf = 0u64;
    let mut total_ga = 0u64;
    let mut rating_sum = 0i64;
    // Per-position W/D/L and avg rating.
    let mut pos_wdl = [[0u64; 3]; 5];
    let mut pos_rating = [0i64; 5];
    // Star-band view: strong PCs (role rating >= 65) experience the match as the
    // protagonist of a real career — decoupling/rating issues hide in the
    // population average, so track them separately.
    let mut star_n = 0u64;
    let mut star_rating_hist = [0u64; 10];
    let mut star_rating_100 = 0u64;
    let mut star_starred_70 = 0u64;
    let mut star_losses = 0u64;
    let mut role_rating_hist = [0u64; 10];
    let mut outliers: Vec<Outlier> = Vec::new();

    for i in 0..n {
        let seed = rng.next_u64() ^ (i.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let pos_idx = rng.next_range_u64(0, 4) as usize;
        let opp_str = rng.next_range_u64(60, 95) as u8;
        let (_, position, role) = POSITIONS[pos_idx];

        let choices = CreationChoices {
            name: "Batch".into(),
            position,
            nationality: "Brazilian",
            club: "Riverside Town".into(),
        };
        let pl = generate_player(seed, &choices);
        // Star injection: 1 in 4 matches is played by a peak-career PC (attr
        // floor 75), modelling the actual game scenario — the PC grows into a
        // star. Fresh-gen players cap at role rating ~60, which would otherwise
        // wash star-player effects out of the stats.
        let mut attrs = pl.current;
        if i % 4 == 3 {
            for a in attrs.iter_mut() {
                *a = (*a).max(Fixed::from_int(75));
            }
        }
        let role_rating =
            goat_core::derive::role_rating(&attrs, role, pl.familiarity[role as usize]).to_int();
        role_rating_hist[(role_rating.clamp(0, 99) as usize) / 10] += 1;
        let aggression = attrs[AttrId::Aggression as usize].to_int().clamp(1, 99) as u8;
        let match_seed = seed ^ 0xc0ffee;
        let mut rp_rng = GoatRng::new(match_seed ^ 0xBADCAFE);
        let own_squad = goat_match::squad::SquadSheet::stub(75, seed ^ 0x5A04_0001, (4, 3, 3));
        // M1 squad-derived profile (measurement variant): the PC replaces the
        // first squad player of HIS position group, so his real (position-
        // shaped) attrs lift his own line of the team average — exactly what
        // `Population::squad_avg_attrs(.., extra = PC)` + `from_squad` do live.
        let own_profile = if m1_squad {
            let pc_group = match position {
                Position::Defender => 0u8,
                Position::Midfielder => 1,
                Position::Forward => 2,
            };
            let mut sums = [0i64; NUM_ATTRS];
            let mut cnt = 0i64;
            let mut pc_placed = false;
            for p in &own_squad.players {
                let src = if !pc_placed && p.position == pc_group {
                    pc_placed = true;
                    &attrs
                } else {
                    &p.attrs
                };
                for (a, s) in sums.iter_mut().enumerate() {
                    *s += src[a].to_raw() as i64;
                }
                cnt += 1;
            }
            let mut avg = [Fixed::ZERO; NUM_ATTRS];
            for (a, v) in avg.iter_mut().enumerate() {
                *v = Fixed::raw((sums[a] / cnt.max(1)) as i32);
            }
            TacticalProfile::from_squad(&avg, 1000, seed)
        } else {
            TacticalProfile::derive(75, 1000, seed)
        };
        let setup = MatchSetup {
            player_role: role,
            player_attrs: attrs,
            player_familiarity: pl.familiarity,
            own_profile,
            opp_profile: TacticalProfile::derive(opp_str, 2000, seed),
            opp_name: "Rivals FC",
            form: Fixed::from_int(65),
            player_aggression: aggression,
            ref_personality: RefPersonality::from_rng(&mut rp_rng),
            dirty_rep: 50,
            player_traits: PlayerTraits::default(),
            staff_mods: goat_core::staff::StaffMods::NEUTRAL,
            own_squad,
            opp_squad: goat_match::squad::SquadSheet::stub(opp_str, seed ^ 0x5A04_0002, (4, 3, 3)),
            sub_context: None,
        };
        let r = auto_play_match(&lib, setup, &mut GoatRng::new(match_seed));

        // PC goals = goals CREDITED to the PC (PA2 M3) — team goals finished by
        // a teammate (incl. off the PC's delivery = his assists) don't count.
        let pc_goals = r
            .goal_credits
            .iter()
            .filter(|c| {
                c.event == goat_match::beats::ScoreEvent::GoalFor
                    && c.scorer == goat_match::beats::GoalActor::Pc
            })
            .count() as u64;

        match r.goals_for.cmp(&r.goals_against) {
            std::cmp::Ordering::Greater => {
                wins += 1;
                pos_wdl[pos_idx][0] += 1;
                if r.player_output <= 45 {
                    carried_45 += 1;
                }
            }
            std::cmp::Ordering::Less => {
                losses += 1;
                pos_wdl[pos_idx][2] += 1;
                if pc_goals >= 2 {
                    brace_losses += 1;
                }
                if r.player_output >= 70 {
                    starred_70 += 1;
                }
                if r.player_output >= 80 {
                    starred_80 += 1;
                }
            }
            std::cmp::Ordering::Equal => {
                draws += 1;
                pos_wdl[pos_idx][1] += 1;
            }
        }
        if r.goals_against == 0 {
            clean_sheets += 1;
        }
        let dd_w = r.danger_duels_won as u64;
        let dd_l = r.danger_duels_lost as u64;
        let dd = dd_w + dd_l;
        if dd > 0 {
            danger_matches += 1;
            danger_won += dd_w;
            danger_lost += dd_l;
            if dd >= 3 && r.goals_for < r.goals_against {
                if dd_w * 5 >= dd * 3 {
                    quiet_in_defeat += 1;
                }
                if dd_l * 5 >= dd * 3 {
                    exposed_in_defeat += 1;
                }
            }
        }
        if role_rating >= 65 {
            star_n += 1;
            star_rating_hist[(r.player_output.clamp(0, 99) as usize) / 10] += 1;
            if r.player_output == 100 {
                star_rating_100 += 1;
            }
            if r.goals_for < r.goals_against {
                star_losses += 1;
                if r.player_output >= 70 {
                    star_starred_70 += 1;
                }
            }
        }
        gf_hist[(r.goals_for as usize).min(10)] += 1;
        ga_hist[(r.goals_against as usize).min(10)] += 1;
        rating_hist[(r.player_output.clamp(0, 99) as usize) / 10] += 1;
        pc_goals_hist[(pc_goals as usize).min(5)] += 1;
        total_gf += r.goals_for as u64;
        total_ga += r.goals_against as u64;
        rating_sum += r.player_output as i64;
        pos_rating[pos_idx] += r.player_output as i64;
        outliers.push(Outlier {
            seed,
            pos_idx,
            opp_str,
            goals_for: r.goals_for,
            goals_against: r.goals_against,
            rating: r.player_output,
            pc_goals,
        });
    }

    let nf = n as f64;
    println!(
        "══ match-batch: {n} matches (master seed {master_seed:#x}{}) ══\n",
        if m1_squad {
            ", M1 squad-derived own_profile"
        } else {
            ""
        }
    );

    println!("── Result split ──");
    println!(
        "  W {:>6} ({:>5.1}%)   D {:>6} ({:>5.1}%)   L {:>6} ({:>5.1}%)",
        wins,
        wins as f64 / nf * 100.0,
        draws,
        draws as f64 / nf * 100.0,
        losses,
        losses as f64 / nf * 100.0,
    );

    println!("\n── Goals for (team) ──");
    print_hist(
        &gf_hist,
        n,
        &["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10+"],
    );
    println!("\n── Goals against (team) ──");
    print_hist(
        &ga_hist,
        n,
        &["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10+"],
    );

    println!("\n── Player rating (output 0–100) ──");
    print_hist(
        &rating_hist,
        n,
        &[
            "0-9", "10-19", "20-29", "30-39", "40-49", "50-59", "60-69", "70-79", "80-89", "90-100",
        ],
    );
    println!("  mean rating {:.1}", rating_sum as f64 / nf);

    println!("\n── PC goals per match ──");
    print_hist(&pc_goals_hist, n, &["0", "1", "2", "3", "4", "5+"]);
    println!(
        "  PC scored 2+ but team LOST: {} matches ({:.2}%)",
        brace_losses,
        brace_losses as f64 / nf * 100.0
    );

    println!("\n── Decoupling (output rating vs team result) ──");
    println!(
        "  starred in defeat (rating>=70 & L): {} ({:.2}%)   (rating>=80 & L): {} ({:.2}%)",
        starred_70,
        starred_70 as f64 / nf * 100.0,
        starred_80,
        starred_80 as f64 / nf * 100.0,
    );
    println!(
        "  carried to a win (rating<=45 & W): {} ({:.2}%)",
        carried_45,
        carried_45 as f64 / nf * 100.0,
    );
    if losses > 0 {
        println!(
            "  share of losses with rating>=70: {:.1}%",
            starred_70 as f64 / losses as f64 * 100.0
        );
    }
    println!(
        "  clean sheets (goals_against == 0): {} ({:.1}%)",
        clean_sheets,
        clean_sheets as f64 / nf * 100.0
    );
    println!(
        "  danger man faced in {} matches ({:.1}%)",
        danger_matches,
        danger_matches as f64 / nf * 100.0
    );
    if danger_won + danger_lost > 0 {
        println!(
            "  danger-man duel win rate: {:.1}% ({} won / {} lost)",
            danger_won as f64 / (danger_won + danger_lost) as f64 * 100.0,
            danger_won,
            danger_lost,
        );
    }
    println!(
        "  kept the danger man quiet in DEFEAT (>=3 duels, >=60% won): {} ({:.2}%)",
        quiet_in_defeat,
        quiet_in_defeat as f64 / nf * 100.0
    );
    println!(
        "  run ragged by the danger man in DEFEAT (>=3 duels, >=60% lost): {} ({:.2}%)",
        exposed_in_defeat,
        exposed_in_defeat as f64 / nf * 100.0
    );

    println!("\n── PC role rating (quality spread of generated players) ──");
    print_hist(
        &role_rating_hist,
        n,
        &[
            "0-9", "10-19", "20-29", "30-39", "40-49", "50-59", "60-69", "70-79", "80-89", "90-100",
        ],
    );

    if star_n > 0 {
        println!("\n── Star band (role rating >= 65, n={star_n}) ──");
        print_hist(
            &star_rating_hist,
            star_n,
            &[
                "0-9", "10-19", "20-29", "30-39", "40-49", "50-59", "60-69", "70-79", "80-89",
                "90-100",
            ],
        );
        let sf = star_n as f64;
        println!(
            "  of which rating == 100 (clamp pile): {} ({:.2}%)",
            star_rating_100,
            star_rating_100 as f64 / sf * 100.0,
        );
        println!(
            "  starred in defeat (rating>=70 & L): {} ({:.2}%)   share of losses: {:.1}%",
            star_starred_70,
            star_starred_70 as f64 / sf * 100.0,
            if star_losses > 0 {
                star_starred_70 as f64 / star_losses as f64 * 100.0
            } else {
                0.0
            },
        );
    }

    println!("\n── Goals per match ──");
    println!(
        "  for {:.2}   against {:.2}   total {:.2}",
        total_gf as f64 / nf,
        total_ga as f64 / nf,
        (total_gf + total_ga) as f64 / nf
    );

    println!("\n── Per-position ──");
    println!("  pos    W%     D%     L%   avg rating");
    for (i, (name, _, _)) in POSITIONS.iter().enumerate() {
        let cnt: u64 = pos_wdl[i].iter().sum();
        if cnt == 0 {
            continue;
        }
        let cf = cnt as f64;
        println!(
            "  {name:<4} {:>5.1}  {:>5.1}  {:>5.1}  {:>7.1}",
            pos_wdl[i][0] as f64 / cf * 100.0,
            pos_wdl[i][1] as f64 / cf * 100.0,
            pos_wdl[i][2] as f64 / cf * 100.0,
            pos_rating[i] as f64 / cf,
        );
    }

    println!("\n── Outliers ──");
    let show = |title: &str, rows: &[&Outlier]| {
        println!("  {title}");
        for o in rows {
            let pos = POSITIONS[o.pos_idx].0;
            let res = match o.goals_for.cmp(&o.goals_against) {
                std::cmp::Ordering::Greater => "W",
                std::cmp::Ordering::Less => "L",
                std::cmp::Ordering::Equal => "D",
            };
            println!(
                "    seed {:>16}  {pos:<3} vs str {:>2}  {}-{} {res}  rating {:>3}  pc_goals {}",
                o.seed, o.opp_str, o.goals_for, o.goals_against, o.rating, o.pc_goals
            );
        }
    };
    let mut by_total_goals: Vec<&Outlier> = outliers.iter().collect();
    by_total_goals.sort_by_key(|o| std::cmp::Reverse(o.goals_for + o.goals_against));
    show("highest-scoring matches:", &by_total_goals[..5]);
    let mut by_margin: Vec<&Outlier> = outliers.iter().collect();
    by_margin.sort_by_key(|o| std::cmp::Reverse(o.goals_for.abs_diff(o.goals_against)));
    show("biggest margins:", &by_margin[..5]);
    let mut by_rating_desc: Vec<&Outlier> = outliers.iter().collect();
    by_rating_desc.sort_by_key(|o| std::cmp::Reverse(o.rating));
    show("highest ratings:", &by_rating_desc[..5]);
    let mut by_rating_asc = by_rating_desc;
    by_rating_asc.sort_by_key(|o| o.rating);
    show("lowest ratings:", &by_rating_asc[..5]);
}

fn print_hist(hist: &[u64], total: u64, labels: &[&str]) {
    let tf = total as f64;
    for (i, &c) in hist.iter().enumerate() {
        if c == 0 {
            continue;
        }
        let pct = c as f64 / tf * 100.0;
        let bar = "█".repeat((pct * 1.5) as usize);
        println!("  {:>6} {:>7} ({:>5.1}%) {}", labels[i], c, pct, bar);
    }
}
