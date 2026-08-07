//! Named recurring pundits — each mapped to a school, each with a personality.
//!
//! Template + slot text lives here; the TUI fills {rank}, {name}, {score} slots
//! and prints without modification. All player-facing text is in this module.

use crate::legacy::LegacyAxes;
use goat_rng::{GoatRng, RngSource};

/// A named pundit character who follows the player's whole career.
#[derive(Debug, Clone, Copy)]
pub struct Pundit {
    pub name: &'static str,
    pub role: &'static str,        // e.g. "former midfielder, Sky analyst"
    pub personality: &'static str, // one-word flavour
    pub school_idx: usize,
    /// Template when PC ranks in top 3 of this pundit's school.
    pub praise: &'static str,
    /// Template when PC ranks 4–7.
    pub neutral: &'static str,
    /// Template when PC ranks outside top 7.
    pub doubt: &'static str,
    /// Template used after a season summary.
    pub season_reaction: &'static str,
}

/// Context used to pick and fill a pundit comment.
#[derive(Debug, Clone)]
pub enum PunditContext {
    /// End of season — show season_reaction template.
    Season {
        goals: u32,
        matches: u32,
        avg_output: i32,
        finish_pos: u32,
    },
    /// Pantheon screen — show praise/neutral/doubt based on rank.
    Pantheon { rank: usize },
    /// Award won.
    AwardWon { award: &'static str },
    /// Award lost.
    AwardLost { award: &'static str, winner: String },
}

/// Slice 1 (Design round 6, revised 2026-08-07): 3 pundits per school × 4
/// schools. `school_idx` is many:1 — ranking sentiment is per-school, so
/// schoolmates always agree on direction; they differ in voice and (via
/// `tier_for`, Slice 2) credibility.
pub const NUM_PUNDITS: usize = 12;

pub const PUNDITS: [Pundit; NUM_PUNDITS] = [
    // ── Trophy Cabinet (school 0) ────────────────────────────────────────────
    Pundit {
        name: "Marco Torres",
        role: "ex-striker, pundit",
        personality: "winner",
        school_idx: 0, // Trophy Cabinet
        praise: "Listen — {goals} goals, a top-half finish. For me, that's the difference between a good player and a champion. If {name} keeps this up, they're entering trophy-cabinet territory.",
        neutral: "{name} had a decent season, {goals} goals. But where are the trophies? I need to see silverware before I start talking about legacies.",
        doubt: "Look, {avg_output}/100 average output is fine. But without trophies it doesn't mean anything. I'm not putting {name} anywhere near the all-time debate yet.",
        season_reaction: "{name} — {goals} goals, position {pos} in the table. The {pos}! We need titles, not mid-table finishes. I want to believe, I do.",
    },
    Pundit {
        name: "Dana Whitmore",
        role: "ex-captain, TV co-commentator",
        personality: "pragmatist",
        school_idx: 0, // Trophy Cabinet
        praise: "I've counted medals my whole career, and {name} is collecting the right kind of seasons. {goals} goals and a real run at it — that's how cabinets get filled.",
        neutral: "Solid season from {name}, no question. But my medal count test is simple: show me the honours list in May. Until then it's promise, not proof.",
        doubt: "I keep hearing about potential with {name}. Potential doesn't weigh anything around your neck. This sport remembers winners, and this season isn't one.",
        season_reaction: "{goals} goals and position {pos}. Decent numbers for {name} — but I judge careers in May, when the medals come out.",
    },
    Pundit {
        name: "Ricky Davenport",
        role: "shock-jock radio host",
        personality: "bombastic",
        school_idx: 0, // Trophy Cabinet
        praise: "FINALLY! {name} gets it — {goals} goals and a title charge! THAT is what I'm talking about! This kid wants to WIN things!",
        neutral: "{name} had moments, sure. But MOMENTS don't fill trophy rooms, people! Call me when there's silverware on the table!",
        doubt: "I'm tired of the hype machine around {name}. No trophies, no legacy, END of conversation. Next caller!",
        season_reaction: "{goals} goals, position {pos} — and ZERO guarantees in May! Wake me when {name} actually WINS something!",
    },
    // ── Eye-Test Romantics (school 1) ────────────────────────────────────────
    Pundit {
        name: "Alice Brennan",
        role: "sports journalist, The Athletic",
        personality: "romantic",
        school_idx: 1, // Eye-Test Romantics
        praise: "There are players who win trophies, and then there are players who leave you reaching for your phone to send that goal clip to your friends at midnight. {name} is the second kind. That's what I'll remember.",
        neutral: "{name} has moments — genuine moments. But they need more of them. Show me that decisive instinct, the ability to lift a team when it matters. Potential is there.",
        doubt: "I watch every game, and I'll be honest: {name} hasn't given me *the* moment yet. Good footballer? Sure. But to be remembered? You need to make people stop breathing for three seconds.",
        season_reaction: "{avg_output} average, {goals} goals this season. The decisive moments — were they there? I need more of those late-match sparks from {name}.",
    },
    Pundit {
        name: "Tomás Reyes",
        role: "freelance football writer",
        personality: "poet",
        school_idx: 1, // Eye-Test Romantics
        praise: "Some seasons are statistics; this one was a verse. {name} played like the pitch was a page and every touch a line worth rereading.",
        neutral: "There are flashes of poetry in {name}'s game — a turn here, a pass there. I'm waiting for the full stanza.",
        doubt: "I keep my notebook open for {name}, and the page stays blank too often. Talent without theatre is just administration.",
        season_reaction: "{goals} goals this season, and a few of them worth framing. {name} is writing something — I just can't tell yet if it's a sonnet or a shopping list.",
    },
    Pundit {
        name: "June Okafor",
        role: "documentary filmmaker",
        personality: "sentimental",
        school_idx: 1, // Eye-Test Romantics
        praise: "I've filmed footballers for twenty years, and I know the ones the camera loves. {name} plays like they know the lens is on them — in the best possible way.",
        neutral: "The footage on {name} is good. Not great. The story is still missing its scene — every legend has one.",
        doubt: "I keep waiting for the frame that defines {name}, and it hasn't come. You can't build a documentary around almost.",
        season_reaction: "{goals} goals, position {pos}. {name}'s season had chapters, but I'm still waiting for the scene that makes the film.",
    },
    // ── Stats Purists (school 2) ─────────────────────────────────────────────
    Pundit {
        name: "Kwame Asante",
        role: "data analyst, The Numbers Don't Lie podcast",
        personality: "purist",
        school_idx: 2, // Stats Purists
        praise: "The data on {name} is compelling. Career output average, match involvement, consistency — these metrics put them in rarified company. The numbers say elite. I trust the numbers.",
        neutral: "{name} averages {avg_output}/100 this season. The trajectory is positive, but the sample size is still limited. Give me 300 games and I'll give you a definitive verdict.",
        doubt: "{goals} goals, {avg_output} average output. Those are fine, not great. Over a full career these numbers need to climb substantially. The stats aren't making a case for a pantheon conversation yet.",
        season_reaction: "Season {season}: {goals} goals, {matches} appearances, {avg_output}/100 average. The baseline is building. The career arc is what matters — single seasons tell you very little.",
    },
    Pundit {
        name: "Ingrid Solberg",
        role: "analytics columnist",
        personality: "methodical",
        school_idx: 2, // Stats Purists
        praise: "Adjusted for minutes and opposition strength, {name}'s output curve is exactly what an elite trajectory looks like. No narrative needed — the trendline speaks.",
        neutral: "{name}'s underlying numbers are stable, not spectacular. I want another season of data before I move them up a tier in my model.",
        doubt: "Strip away the highlight reels and {name}'s season regresses to the mean. My model doesn't see a case here yet.",
        season_reaction: "Season {season} for {name}: {matches} matches, {avg_output}/100 average output, {goals} goals. Logged. The model updates weekly, not emotionally.",
    },
    Pundit {
        name: "Dev Kaminski",
        role: "former quant trader, model-builder",
        personality: "skeptic",
        school_idx: 2, // Stats Purists
        praise: "I short hype for a living, so believe me when I say: {name}'s numbers are genuinely underpriced. This is real production, not variance.",
        neutral: "{name}'s season is within one standard deviation of fine. I'm not buying or selling yet.",
        doubt: "Everyone loves a story stock. {name}'s fundamentals — {goals} goals, {avg_output} average — don't support the valuation.",
        season_reaction: "Season {season}: {goals} goals in {matches} matches, {avg_output}/100. I've seen worse numbers get hyped harder. Show me consistency over volume, {name}.",
    },
    // ── Loyalty Traditionalists (school 3) ───────────────────────────────────
    Pundit {
        name: "Pavel Straka",
        role: "ex-defender, club historian",
        personality: "traditionalist",
        school_idx: 3, // Loyalty Traditionalists
        praise: "What you cannot buy is loyalty. {name} has stayed, performed, become part of the identity of a club. In this era of mercenaries and transfer windows, that means everything to me.",
        neutral: "{name} is a good player. But I look at the loyalty question. How many clubs? A legend is forged over years at one place. I want to see that commitment before I make my judgement.",
        doubt: "{name} moves around too much for my taste. I don't care how many goals you score — if you leave the moment a bigger offer comes in, you're not a legend. You're a hired gun.",
        season_reaction: "{name} at {club} this season — {goals} goals. The connection with the fans, the consistency, the staying power. That's what the game used to be about.",
    },
    Pundit {
        name: "Maggie Calloway",
        role: "supporters' club elder",
        personality: "faithful",
        school_idx: 3, // Loyalty Traditionalists
        praise: "Forty years I've stood on that terrace, and I can tell you: {name} is one of ours. Scores, stays, never looks at the door. That's family.",
        neutral: "{name} seems a good sort, but talk to me in five years. Loyalty isn't a season — it's a habit.",
        doubt: "I've seen a hundred like {name} — here today, agent's phone call tomorrow. We remember the ones who stayed.",
        season_reaction: "{goals} goals for {club} this season. The fans see {name} giving everything. Whether they're one of us — that takes longer to prove.",
    },
    Pundit {
        name: "Viktor Ashby",
        role: "ex-manager, tactics historian",
        personality: "stalwart",
        school_idx: 3, // Loyalty Traditionalists
        praise: "In my day you built a side around players like {name} — because they were THERE, year after year. Continuity wins more than chequebooks do.",
        neutral: "{name} has quality, but a career is a long conversation with one club. I haven't seen that commitment yet.",
        doubt: "Talent is common. Staying power is rare. {name} hasn't shown me the second, and without it the first fades.",
        season_reaction: "{name}, {goals} goals at {club} this season. Ask me again when they've given the club a decade, not a season.",
    },
];

// ── Credibility tiers (Design round 6, Slice 2) ─────────────────────────────

/// A pundit's credibility, currently assigned by `tier_for` — a deliberately
/// simple/placeholder function. The real "grows with tenure and being proven
/// right" mechanic (raised by Tùng, not designed this round) will replace
/// `tier_for`'s body without touching anything that reads `PunditTier` — that
/// is the entire point of keeping this behind one function boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PunditTier {
    Rookie,
    Established,
    Legend,
}

/// Deliberately simple/placeholder tier assignment — a seeded roll per
/// (world_seed, pundit index), deterministic within a save, independent across
/// saves. This is the ONE function the real "grows with tenure / proven-right
/// accuracy" formula replaces later; nothing outside this function should ever
/// compute a tier by any other means. Never stored on the const `Pundit` and
/// never persisted (Slice 2.2) — recomputed on demand, same "generated but
/// consistent" idiom as `WorldGenesis`.
pub fn tier_for(pundit_idx: usize, world_seed: u64) -> PunditTier {
    let mut rng = GoatRng::new(pundit_tier_seed(world_seed, pundit_idx));
    match rng.next_range_u32(0, 99) {
        // TUNABLE placeholder split (Design's pick, flagged for TASK-TUNE):
        // mostly Established, Legend genuinely rare, Rookie a real minority.
        0..=19 => PunditTier::Rookie,       // 20%
        20..=84 => PunditTier::Established, // 65%
        _ => PunditTier::Legend,            // 15%
    }
}

/// Index-keyed XOR-and-multiply — same idiom as `world.rs`'s `club_seed`.
fn pundit_tier_seed(world_seed: u64, pundit_idx: usize) -> u64 {
    world_seed
        ^ (pundit_idx as u64)
            .rotate_left(23)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// Generate a pundit comment for the given context.
///
/// Fills template slots: {name}, {goals}, {matches}, {avg_output}, {pos}, {season},
/// {club}, {award}, {winner}.
pub fn pundit_comment(
    pundit: &Pundit,
    axes: &LegacyAxes,
    ctx: &PunditContext,
    pc_name: &str,
    pc_club: &str,
    current_season: u32,
) -> String {
    let template = match ctx {
        PunditContext::Season { .. } => pundit.season_reaction,
        PunditContext::Pantheon { rank } => {
            if *rank <= 3 {
                pundit.praise
            } else if *rank <= 7 {
                pundit.neutral
            } else {
                pundit.doubt
            }
        }
        PunditContext::AwardWon { .. } => pundit.praise,
        PunditContext::AwardLost { .. } => pundit.neutral,
    };

    let (goals, matches, avg_output, pos) = match ctx {
        PunditContext::Season {
            goals,
            matches,
            avg_output,
            finish_pos,
        } => (*goals, *matches, *avg_output, *finish_pos),
        _ => (0u32, 0u32, axes.output.to_int(), 0u32),
    };

    let award = match ctx {
        PunditContext::AwardWon { award } | PunditContext::AwardLost { award, .. } => award,
        _ => "",
    };
    let winner = match ctx {
        PunditContext::AwardLost { winner, .. } => winner.as_str(),
        _ => "",
    };

    template
        .replace("{name}", pc_name)
        .replace("{goals}", &goals.to_string())
        .replace("{matches}", &matches.to_string())
        .replace("{avg_output}", &avg_output.to_string())
        .replace("{pos}", &pos.to_string())
        .replace("{season}", &current_season.to_string())
        .replace("{club}", pc_club)
        .replace("{award}", award)
        .replace("{winner}", winner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pantheon::NUM_SCHOOLS;

    #[test]
    fn pundit_count_is_twelve() {
        assert_eq!(NUM_PUNDITS, 12);
        assert_eq!(PUNDITS.len(), NUM_PUNDITS);
        assert_eq!(NUM_SCHOOLS, 4);
    }

    #[test]
    fn every_school_has_exactly_three_pundits() {
        for school in 0..NUM_SCHOOLS {
            let count = PUNDITS.iter().filter(|p| p.school_idx == school).count();
            assert_eq!(count, 3, "school {school} must have exactly 3 pundits");
        }
    }

    #[test]
    fn every_pundit_school_idx_in_range() {
        assert!(PUNDITS.iter().all(|p| p.school_idx < NUM_SCHOOLS));
    }

    #[test]
    fn no_two_schoolmates_share_a_personality_string() {
        for school in 0..NUM_SCHOOLS {
            let mut personalities: Vec<&str> = PUNDITS
                .iter()
                .filter(|p| p.school_idx == school)
                .map(|p| p.personality)
                .collect();
            personalities.sort_unstable();
            personalities.dedup();
            assert_eq!(
                personalities.len(),
                3,
                "school {school} has copy-paste personalities"
            );
        }
    }

    #[test]
    fn every_pundit_has_nonempty_content_fields() {
        for p in PUNDITS.iter() {
            assert!(!p.name.is_empty());
            assert!(!p.role.is_empty());
            assert!(!p.praise.is_empty());
            assert!(!p.neutral.is_empty());
            assert!(!p.doubt.is_empty());
            assert!(!p.season_reaction.is_empty());
        }
    }

    // ── Slice 2: PunditTier / tier_for ───────────────────────────────────────

    #[test]
    fn tier_for_is_deterministic_per_seed() {
        for idx in 0..NUM_PUNDITS {
            assert_eq!(tier_for(idx, 42), tier_for(idx, 42));
        }
    }

    #[test]
    fn tier_for_varies_across_pundits_and_seeds() {
        let mut seen = [false; 3];
        for seed in 0..50u64 {
            for idx in 0..NUM_PUNDITS {
                match tier_for(idx, seed) {
                    PunditTier::Rookie => seen[0] = true,
                    PunditTier::Established => seen[1] = true,
                    PunditTier::Legend => seen[2] = true,
                }
            }
        }
        assert!(
            seen[0] && seen[1] && seen[2],
            "all three tiers must be reachable"
        );
    }

    #[test]
    fn tier_for_roughly_matches_declared_split() {
        // 12 pundits × 1,000 seeds = 12,000 rolls; wide tolerance — this pins
        // the SHAPE (mostly Established, rare Legend, minority Rookie), not
        // exact frequencies.
        let (mut rookie, mut established, mut legend) = (0u32, 0u32, 0u32);
        for seed in 0..1_000u64 {
            for idx in 0..NUM_PUNDITS {
                match tier_for(idx, seed) {
                    PunditTier::Rookie => rookie += 1,
                    PunditTier::Established => established += 1,
                    PunditTier::Legend => legend += 1,
                }
            }
        }
        let total = (rookie + established + legend) as f64;
        let r = rookie as f64 / total;
        let e = established as f64 / total;
        let l = legend as f64 / total;
        assert!(
            (0.10..=0.30).contains(&r),
            "rookie share {r:.3} vs declared 0.20"
        );
        assert!(
            (0.55..=0.75).contains(&e),
            "established share {e:.3} vs declared 0.65"
        );
        assert!(
            (0.07..=0.23).contains(&l),
            "legend share {l:.3} vs declared 0.15"
        );
    }
}
