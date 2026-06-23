//! Named recurring pundits — each mapped to a school, each with a personality.
//!
//! Template + slot text lives here; the TUI fills {rank}, {name}, {score} slots
//! and prints without modification. All player-facing text is in this module.

use crate::legacy::LegacyAxes;

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

pub const NUM_PUNDITS: usize = 4;

pub const PUNDITS: [Pundit; NUM_PUNDITS] = [
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
        name: "Pavel Straka",
        role: "ex-defender, club historian",
        personality: "traditionalist",
        school_idx: 3, // Loyalty Traditionalists
        praise: "What you cannot buy is loyalty. {name} has stayed, performed, become part of the identity of a club. In this era of mercenaries and transfer windows, that means everything to me.",
        neutral: "{name} is a good player. But I look at the loyalty question. How many clubs? A legend is forged over years at one place. I want to see that commitment before I make my judgement.",
        doubt: "{name} moves around too much for my taste. I don't care how many goals you score — if you leave the moment a bigger offer comes in, you're not a legend. You're a hired gun.",
        season_reaction: "{name} at {club} this season — {goals} goals. The connection with the fans, the consistency, the staying power. That's what the game used to be about.",
    },
];

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
