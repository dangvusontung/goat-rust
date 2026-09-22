/// match-batch — headless batch runner for the Match Flow engine.
///
/// Runs N matches in-process (no subprocess spawns) across all 5 positions and
/// opponent strengths 60–95, then prints distribution stats:
///   1. W/D/L split + goals for/against histogram
///   2. player rating histogram
///   3. % of matches where the PC scores 2+ but the team still loses
///   4. average goals per match
///
/// Usage: match-batch [num_matches] [master_seed]
use goat_core::{
    attrs::AttrId,
    generation::{generate_player, CreationChoices, Position},
    roles::RoleId,
    tactical::TacticalProfile,
};
use goat_fixed::Fixed;
use goat_match::{
    beats::ScoreEvent,
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
    let mut total_gf = 0u64;
    let mut total_ga = 0u64;
    let mut rating_sum = 0i64;
    // Per-position W/D/L and avg rating.
    let mut pos_wdl = [[0u64; 3]; 5];
    let mut pos_rating = [0i64; 5];
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
        let aggression = pl.current[AttrId::Aggression as usize]
            .to_int()
            .clamp(1, 99) as u8;
        let match_seed = seed ^ 0xc0ffee;
        let mut rp_rng = GoatRng::new(match_seed ^ 0xBADCAFE);
        let setup = MatchSetup {
            player_role: role,
            player_attrs: pl.current,
            player_familiarity: pl.familiarity,
            own_profile: TacticalProfile::derive(75, 1000, seed),
            opp_profile: TacticalProfile::derive(opp_str, 2000, seed),
            opp_name: "Rivals FC",
            form: Fixed::from_int(65),
            player_aggression: aggression,
            ref_personality: RefPersonality::from_rng(&mut rp_rng),
            dirty_rep: 50,
            player_traits: PlayerTraits::default(),
            staff_mods: goat_core::staff::StaffMods::NEUTRAL,
        };
        let r = auto_play_match(&lib, setup, &mut GoatRng::new(match_seed));

        // PC goals = goals from beats the PC actually acted in (auto commentary
        // goals belong to the team, not the player).
        let pc_goals = r
            .moments
            .iter()
            .filter(|m| m.is_action && matches!(m.goal_event, Some(ScoreEvent::GoalFor)))
            .count() as u64;

        match r.goals_for.cmp(&r.goals_against) {
            std::cmp::Ordering::Greater => {
                wins += 1;
                pos_wdl[pos_idx][0] += 1;
            }
            std::cmp::Ordering::Less => {
                losses += 1;
                pos_wdl[pos_idx][2] += 1;
                if pc_goals >= 2 {
                    brace_losses += 1;
                }
            }
            std::cmp::Ordering::Equal => {
                draws += 1;
                pos_wdl[pos_idx][1] += 1;
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
    println!("══ match-batch: {n} matches (master seed {master_seed:#x}) ══\n");

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
