//! Match simulation: beat generation, sequencing, team result, familiarity XP.

use goat_core::attrs::{AttrId, ATTR_NAMES, NUM_ATTRS};
use goat_core::roles::{
    FamiliarityTier, PositionFamily, RoleId, NUM_ROLES, ROLE_POSITION_FAMILY, ROLE_WEIGHT_TABLE,
};
use goat_core::tuning::{FAM_XP_IMP_PER_WEEK, FAM_XP_KEY_PER_WEEK, W_IMP, W_KEY};
use goat_fixed::Fixed;
use goat_rng::RngSource;
use goat_traits::PlayerTraits;

use crate::beats::{
    DisciplineEvent, GeneratedBeat, GeneratedChoice, GeneratedOutcome, HeadspaceDelta, MatchPhase,
    ScoreEvent,
};
use crate::beats_data::{RawBeatLibrary, RawChoice, RawOutcome, RawSituation};
use crate::contest::{auto_pick_generated_choice, resolve_contest};
use crate::discipline::{foul_risk_for, red_mist_roll, resolve_card, RefPersonality};
use crate::headspace::Headspace;

// ── Tuning ────────────────────────────────────────────────────────────────────

const BASE_STAMINA_COST: u8 = 3;
const BEATS_PER_MATCH: usize = 15;
const STARTING_STAMINA: Fixed = Fixed::raw(100_000);
const FAM_MATCH_BONUS: Fixed = Fixed::raw(60);

/// §A.3 position/role bias: a situation whose phase fits the player's position
/// family is boosted; a situation whose phase runs against it is suppressed —
/// proportionally, never to zero, so off-role beats stay reachable as emergent
/// moments (a striker tracking back) rather than routine occurrences.
const ROLE_BIAS_FIT_PCT: i32 = 25;
const ROLE_BIAS_AGAINST_PCT: i32 = -60;

// ── Beat library ──────────────────────────────────────────────────────────────

/// Compiled, ready-to-use beat library loaded from the JSON data file.
///
/// Call `BeatLibrary::load(json_str)` once at startup and pass a reference
/// into every `start_match` call. The library is read-only after construction.
#[derive(Debug, Clone)]
pub struct BeatLibrary {
    raw: RawBeatLibrary,
}

impl BeatLibrary {
    pub fn load(json: &str) -> Result<Self, serde_json::Error> {
        Ok(Self {
            raw: RawBeatLibrary::from_json(json)?,
        })
    }

    /// Generate a full 15-beat match sequence from this library using `rng`.
    ///
    /// Periods: early (beats 0–4), mid (5–9), late (10–13), climax (14).
    /// Each beat = one situation + 2–5 choices (each with a success + failure outcome).
    pub fn generate_match(
        &self,
        rng: &mut impl RngSource,
        player_role: RoleId,
        player_traits: &PlayerTraits,
    ) -> Vec<GeneratedBeat> {
        let family = ROLE_POSITION_FAMILY[player_role as usize];
        let mut beats = Vec::with_capacity(BEATS_PER_MATCH);

        // Five early beats (period 0)
        for _ in 0..5 {
            if let Some(b) = self.pick_beat(0, rng, family, player_traits) {
                beats.push(b);
            }
        }
        // Five mid beats (period 1)
        for _ in 0..5 {
            if let Some(b) = self.pick_beat(1, rng, family, player_traits) {
                beats.push(b);
            }
        }
        // Four late beats (period 2)
        for _ in 0..4 {
            if let Some(b) = self.pick_beat(2, rng, family, player_traits) {
                beats.push(b);
            }
        }
        // Final climax beat — always a key-moment situation; role/traits don't bias tag search
        if let Some(b) = self.pick_beat_by_tag("key", 2, rng) {
            beats.push(b);
        } else if let Some(b) = self.pick_beat(2, rng, family, player_traits) {
            beats.push(b);
        }

        beats
    }

    fn pick_beat(
        &self,
        period: usize,
        rng: &mut impl RngSource,
        family: PositionFamily,
        player_traits: &PlayerTraits,
    ) -> Option<GeneratedBeat> {
        // Beat-summoner hook (§A.2 hook type 2) + position/role bias (§A.3): each
        // situation's effective weight is base + base*trait_bonus_pct/100 +
        // base*role_bonus_pct/100. Traits and role each proportionally scale matching
        // situations without destroying the relative weights of unmatched ones, and
        // role bias never fully zeroes out an off-role situation (floor of 1 when base
        // > 0) — it biases the selector, it does not lock a beat list.
        let weight_for = |s: &RawSituation| -> u32 {
            let base = s.bias[period] as i64;
            if base == 0 {
                return 0;
            }
            let trait_bonus = player_traits.beat_summoner_bonus_pct(&s.tags) as i64;
            let role_bonus = role_bias_pct(family, &s.phase) as i64;
            let w = base + base * trait_bonus / 100 + base * role_bonus / 100;
            w.max(1) as u32
        };
        let total_weight: u32 = self.raw.situations.iter().map(weight_for).sum();
        if total_weight == 0 {
            return None;
        }
        let mut roll = rng.next_range_u64(0, total_weight as u64 - 1) as u32;
        let situation = self.raw.situations.iter().find(|s| {
            let w = weight_for(s);
            if roll < w {
                true
            } else {
                roll -= w;
                false
            }
        })?;
        self.assemble_beat(situation, rng)
    }

    fn pick_beat_by_tag(
        &self,
        tag: &str,
        period: usize,
        rng: &mut impl RngSource,
    ) -> Option<GeneratedBeat> {
        let candidates: Vec<&RawSituation> = self
            .raw
            .situations
            .iter()
            .filter(|s| s.tags.iter().any(|t| t == tag) && s.bias[period] > 0)
            .collect();
        if candidates.is_empty() {
            return None;
        }
        let idx = rng.next_range_u64(0, candidates.len() as u64 - 1) as usize;
        self.assemble_beat(candidates[idx], rng)
    }

    fn assemble_beat(
        &self,
        situation: &RawSituation,
        rng: &mut impl RngSource,
    ) -> Option<GeneratedBeat> {
        // Eligible choices: share at least one tag with the situation.
        let eligible: Vec<&RawChoice> = self
            .raw
            .choices
            .iter()
            .filter(|c| c.tags.iter().any(|t| situation.tags.contains(t)))
            .collect();
        if eligible.is_empty() {
            return None;
        }

        // Pick 2–5 distinct choices.
        let n_choices = (rng.next_range_u64(2, 5) as usize).min(eligible.len());
        let mut choice_pool: Vec<usize> = (0..eligible.len()).collect();
        let mut chosen: Vec<GeneratedChoice> = Vec::with_capacity(n_choices);
        for _ in 0..n_choices {
            if choice_pool.is_empty() {
                break;
            }
            let pick = rng.next_range_u64(0, choice_pool.len() as u64 - 1) as usize;
            let raw_choice = eligible[choice_pool[pick]];
            choice_pool.remove(pick);

            let Some(attr) = parse_attr(&raw_choice.attr) else {
                continue;
            };
            let success = self.pick_outcome(true, rng);
            let failure = self.pick_outcome(false, rng);
            let (Some(success), Some(failure)) = (success, failure) else {
                continue;
            };

            chosen.push(GeneratedChoice {
                text: raw_choice.text.clone(),
                primary: attr,
                difficulty: raw_choice.difficulty,
                success,
                failure,
            });
        }

        if chosen.is_empty() {
            return None;
        }

        Some(GeneratedBeat {
            situation_id: situation.id.clone(),
            phase: parse_phase(&situation.phase),
            setup: situation.setup.clone(),
            choices: chosen,
        })
    }

    fn pick_outcome(&self, success: bool, rng: &mut impl RngSource) -> Option<GeneratedOutcome> {
        let pool: Vec<&RawOutcome> = self
            .raw
            .outcomes
            .iter()
            .filter(|o| {
                o.polarity == "any" || o.polarity == if success { "success" } else { "failure" }
            })
            .collect();
        if pool.is_empty() {
            return None;
        }
        let idx = rng.next_range_u64(0, pool.len() as u64 - 1) as usize;
        let raw = pool[idx];
        Some(GeneratedOutcome {
            text: raw.text.clone(),
            output_delta: raw.output_delta,
            headspace: HeadspaceDelta {
                confidence: raw.confidence,
                frustration: raw.frustration,
                flow: raw.flow,
            },
            score_event: match raw.score_event.as_deref() {
                Some("goal_for") => Some(ScoreEvent::GoalFor),
                Some("goal_against") => Some(ScoreEvent::GoalAgainst),
                _ => None,
            },
            stamina_cost: raw.stamina_cost,
            next_situation: raw.next_situation.clone(),
        })
    }

    /// Build a reckless-challenge beat for frustration injection.
    pub fn reckless_beat(&self, rng: &mut impl RngSource) -> GeneratedBeat {
        let failure = self
            .pick_outcome(false, rng)
            .unwrap_or_else(default_failure);
        GeneratedBeat {
            situation_id: "reckless_challenge".to_string(),
            phase: MatchPhase::OpenPlayDefend,
            setup: "Temper boiling over — you go in recklessly on the ball.".to_string(),
            choices: vec![GeneratedChoice {
                text: "Pull out — stay on your feet".to_string(),
                primary: AttrId::Composure,
                difficulty: 50,
                success: GeneratedOutcome {
                    text: "You check yourself just in time.".to_string(),
                    output_delta: 0,
                    headspace: HeadspaceDelta {
                        confidence: 2,
                        frustration: -5,
                        flow: 0,
                    },
                    score_event: None,
                    stamina_cost: 2,
                    next_situation: None,
                },
                failure,
            }],
        }
    }
}

fn default_failure() -> GeneratedOutcome {
    GeneratedOutcome {
        text: "A reckless challenge — the referee is not happy.".to_string(),
        output_delta: -8,
        headspace: HeadspaceDelta {
            confidence: -5,
            frustration: 10,
            flow: -5,
        },
        score_event: None,
        stamina_cost: 4,
        next_situation: None,
    }
}

// ── Attribute string → AttrId ─────────────────────────────────────────────────

fn parse_attr(s: &str) -> Option<AttrId> {
    // Match against enum variant names (camelCase, no spaces).
    for (i, &name) in ATTR_NAMES.iter().enumerate() {
        // ATTR_NAMES uses spaces; also accept the no-space camelCase form.
        let no_space = name.replace(' ', "");
        if s == name || s == no_space {
            return AttrId::ALL.get(i).copied();
        }
    }
    // Direct enum name fallback (for JSON authored with variant names).
    match s {
        "Acceleration" => Some(AttrId::Acceleration),
        "SprintSpeed" => Some(AttrId::SprintSpeed),
        "Finishing" => Some(AttrId::Finishing),
        "LongShots" => Some(AttrId::LongShots),
        "ShotPower" => Some(AttrId::ShotPower),
        "Volleys" => Some(AttrId::Volleys),
        "Penalties" => Some(AttrId::Penalties),
        "ShortPassing" => Some(AttrId::ShortPassing),
        "LongPassing" => Some(AttrId::LongPassing),
        "Vision" => Some(AttrId::Vision),
        "Crossing" => Some(AttrId::Crossing),
        "FreeKickAcc" => Some(AttrId::FreeKickAcc),
        "CloseControl" => Some(AttrId::CloseControl),
        "Dribbling" => Some(AttrId::CloseControl), // legacy alias
        "BallControl" => Some(AttrId::BallControl),
        "Agility" => Some(AttrId::Agility),
        "Balance" => Some(AttrId::Balance),
        "Reactions" => Some(AttrId::Reactions),
        "StandingTackle" => Some(AttrId::StandingTackle),
        "Tackling" => Some(AttrId::StandingTackle), // legacy alias
        "Marking" => Some(AttrId::Marking),
        "Interceptions" => Some(AttrId::Interceptions),
        "Heading" => Some(AttrId::Heading),
        "Curve" => Some(AttrId::Curve),
        "DefAwareness" => Some(AttrId::Curve), // legacy alias
        "AttPositioning" => Some(AttrId::AttPositioning),
        "Positioning" => Some(AttrId::AttPositioning), // legacy alias
        "Strength" => Some(AttrId::Strength),
        "Stamina" => Some(AttrId::Stamina),
        "Aggression" => Some(AttrId::Aggression),
        "Jumping" => Some(AttrId::Jumping),
        "Composure" => Some(AttrId::Composure),
        "Bravery" => Some(AttrId::Bravery),
        "WorkRate" => Some(AttrId::Bravery), // legacy alias
        "SlidingTackle" => Some(AttrId::SlidingTackle),
        "Concentration" => Some(AttrId::SlidingTackle), // legacy alias
        _ => None,
    }
}

fn parse_phase(s: &str) -> MatchPhase {
    match s {
        "attack" => MatchPhase::OpenPlayAttack,
        "defend" => MatchPhase::OpenPlayDefend,
        "setpiece" => MatchPhase::SetPiece,
        "positioning" => MatchPhase::Positioning,
        "key" => MatchPhase::KeyMoment,
        _ => MatchPhase::OpenPlayAttack,
    }
}

/// Position/role bias (§A.3) for a situation's `phase`, relative to the acting
/// player's position family. Set pieces, positioning, and key moments are shared
/// team-context phases and stay neutral for every family.
fn role_bias_pct(family: PositionFamily, phase: &str) -> i32 {
    match (family, phase) {
        (PositionFamily::Forward, "attack") => ROLE_BIAS_FIT_PCT,
        (PositionFamily::Forward, "defend") => ROLE_BIAS_AGAINST_PCT,
        (PositionFamily::Defender, "defend") => ROLE_BIAS_FIT_PCT,
        (PositionFamily::Defender, "attack") => ROLE_BIAS_AGAINST_PCT,
        _ => 0,
    }
}

// ── Public types ──────────────────────────────────────────────────────────────

/// Everything the match engine needs to run a match.
#[derive(Debug, Clone)]
pub struct MatchSetup {
    pub player_role: RoleId,
    pub player_attrs: [Fixed; NUM_ATTRS],
    pub player_familiarity: [FamiliarityTier; NUM_ROLES],
    pub own_strength: u8,
    pub opp_strength: u8,
    pub opp_name: String,
    pub form: Fixed,
    pub player_aggression: u8,
    pub ref_personality: RefPersonality,
    pub dirty_rep: i32,
    pub player_traits: PlayerTraits,
}

/// Summary of a single beat moment for the post-match recap.
#[derive(Debug, Clone)]
pub struct MomentSummary {
    pub beat_id: String,
    pub minute: u32,
    pub choice_idx: usize,
    pub success: bool,
    pub setup_text: String,
    pub outcome_text: String,
    pub goal_event: Option<ScoreEvent>,
}

/// Final result of a completed match.
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub player_output: i32,
    pub goals_for: u32,
    pub goals_against: u32,
    pub moments: Vec<MomentSummary>,
    pub familiarity_xp: [Fixed; NUM_ROLES],
    pub yellow_cards: u8,
    pub red_card: bool,
}

/// Live match state stored between `MakeMatchChoice` intents.
#[derive(Debug, Clone)]
pub struct ActiveMatchState {
    pub setup: MatchSetup,
    pub beat_idx: usize,
    /// Pre-generated beats for this match.
    pub beats: Vec<GeneratedBeat>,
    pub player_output: i32,
    pub headspace: Headspace,
    pub stamina: Fixed,
    pub goals_for: u32,
    pub goals_against: u32,
    pub yellow_cards: u8,
    pub red_card: bool,
    pub moments: Vec<MomentSummary>,
    pub familiarity_xp: [Fixed; NUM_ROLES],
    pub is_complete: bool,
    pub final_result: Option<MatchResult>,
}

impl ActiveMatchState {
    pub fn current_beat(&self) -> Option<&GeneratedBeat> {
        if self.is_complete {
            return None;
        }
        self.beats.get(self.beat_idx)
    }

    pub fn current_minute(&self) -> u32 {
        if BEATS_PER_MATCH == 0 {
            return 90;
        }
        (self.beat_idx as u32 * 90) / BEATS_PER_MATCH as u32
    }
}

// ── Match initialisation ──────────────────────────────────────────────────────

/// Start a new match: generate beat sequence, initialise headspace from form.
pub fn start_match(
    lib: &BeatLibrary,
    setup: MatchSetup,
    rng: &mut impl RngSource,
) -> ActiveMatchState {
    let headspace = Headspace::from_form(setup.form);
    let beats = lib.generate_match(rng, setup.player_role, &setup.player_traits);
    ActiveMatchState {
        headspace,
        setup,
        beat_idx: 0,
        beats,
        player_output: 50,
        stamina: STARTING_STAMINA,
        goals_for: 0,
        goals_against: 0,
        moments: Vec::new(),
        familiarity_xp: [Fixed::ZERO; NUM_ROLES],
        yellow_cards: 0,
        red_card: false,
        is_complete: false,
        final_result: None,
    }
}

/// Advance one beat with the given choice index.
pub fn advance_beat(
    mut ms: ActiveMatchState,
    choice_idx: usize,
    lib: &BeatLibrary,
    rng: &mut impl RngSource,
) -> ActiveMatchState {
    let beat = match ms.current_beat() {
        Some(b) => b.clone(),
        None => {
            ms.is_complete = true;
            return ms;
        }
    };

    let choice_idx = choice_idx.min(beat.choices.len().saturating_sub(1));
    let choice = &beat.choices[choice_idx];

    let goals_behind = ms.goals_against as i32 - ms.goals_for as i32;
    let minutes_remaining = 90u32.saturating_sub(ms.current_minute());
    let desp_mod = Headspace::desperation_mod(
        goals_behind,
        minutes_remaining,
        ms.setup.player_attrs[AttrId::Composure as usize],
    );

    let success = resolve_contest(
        ms.setup.player_attrs[choice.primary as usize],
        choice.difficulty,
        &ms.headspace,
        ms.stamina,
        desp_mod,
        rng,
    );

    let outcome = if success {
        &choice.success
    } else {
        &choice.failure
    };

    ms.player_output = (ms.player_output + outcome.output_delta as i32).clamp(0, 100);
    ms.headspace.apply(
        &outcome.headspace,
        ms.setup.player_attrs[AttrId::Composure as usize],
    );
    let stamina_cost = Fixed::from_int((BASE_STAMINA_COST + outcome.stamina_cost) as i32);
    ms.stamina = (ms.stamina - stamina_cost).clamp(Fixed::ZERO, STARTING_STAMINA);

    if let Some(ev) = outcome.score_event {
        match ev {
            ScoreEvent::GoalFor => ms.goals_for += 1,
            ScoreEvent::GoalAgainst => ms.goals_against += 1,
        }
    }

    award_familiarity_xp(&mut ms, choice.primary);

    ms.moments.push(MomentSummary {
        beat_id: beat.situation_id.clone(),
        minute: ms.current_minute(),
        choice_idx,
        success,
        setup_text: beat.setup.clone(),
        outcome_text: outcome.text.clone(),
        goal_event: outcome.score_event,
    });

    ms.headspace
        .tick(ms.setup.player_attrs[AttrId::Composure as usize]);

    // ── Discipline ────────────────────────────────────────────────────────────
    if !ms.red_card {
        if let Some(risk) = foul_risk_for_situation(&beat.situation_id, choice_idx) {
            if let Some(card) = resolve_card(
                &risk,
                ms.setup.player_aggression,
                ms.headspace.frustration,
                ms.setup.dirty_rep,
                ms.setup.ref_personality,
                ms.yellow_cards,
                rng,
            ) {
                match card {
                    DisciplineEvent::YellowCard => ms.yellow_cards += 1,
                    DisciplineEvent::RedCard => {
                        ms.red_card = true;
                        finish_match(&mut ms, rng);
                        return ms;
                    }
                }
            }
        }

        if let Some(card) = red_mist_roll(
            ms.headspace.frustration,
            ms.setup.player_aggression,
            ms.setup.dirty_rep,
            ms.setup.ref_personality,
            ms.yellow_cards,
            rng,
        ) {
            match card {
                DisciplineEvent::YellowCard => ms.yellow_cards += 1,
                DisciplineEvent::RedCard => {
                    ms.red_card = true;
                    finish_match(&mut ms, rng);
                    return ms;
                }
            }
        }

        // Frustration injection: insert reckless-challenge beat when temper flares.
        if ms.headspace.frustration > 75 && rng.next_range_u64(0, 9) < 3 {
            let rb = lib.reckless_beat(rng);
            if ms.beat_idx + 1 < ms.beats.len() {
                ms.beats[ms.beat_idx + 1] = rb;
            } else {
                ms.beats.push(rb);
            }
        }
    }

    ms.beat_idx += 1;
    if ms.beat_idx >= ms.beats.len() {
        finish_match(&mut ms, rng);
    }

    ms
}

/// Auto-play an entire match (skip-match path).
pub fn auto_play_match(
    lib: &BeatLibrary,
    setup: MatchSetup,
    rng: &mut impl RngSource,
) -> MatchResult {
    let mut ms = start_match(lib, setup, rng);
    while !ms.is_complete {
        let choice_idx = ms
            .current_beat()
            .map(|b| auto_pick_generated_choice(&b.choices, &ms.setup.player_attrs))
            .unwrap_or(0);
        ms = advance_beat(ms, choice_idx, lib, rng);
    }
    let fallback = build_result(&ms);
    ms.final_result.unwrap_or(fallback)
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn finish_match(ms: &mut ActiveMatchState, rng: &mut impl RngSource) {
    let own = ms.setup.own_strength as u64;
    let opp = ms.setup.opp_strength as u64;
    let player_bonus = (ms.player_output / 10) as u64;

    let own_att = own * 2 + player_bonus;
    let opp_att = opp * 2;
    let for_prob = own_att * 500 / (own_att + opp * 2 + 1);
    let against_prob = opp_att * 500 / (opp_att + own * 2 + 1);

    for _ in 0..5 {
        if rng.next_range_u64(0, 999) < for_prob {
            ms.goals_for += 1;
        }
        if rng.next_range_u64(0, 999) < against_prob {
            ms.goals_against += 1;
        }
    }

    ms.is_complete = true;
    ms.final_result = Some(build_result(ms));
}

fn build_result(ms: &ActiveMatchState) -> MatchResult {
    MatchResult {
        player_output: ms.player_output,
        goals_for: ms.goals_for,
        goals_against: ms.goals_against,
        moments: ms.moments.clone(),
        familiarity_xp: ms.familiarity_xp,
        yellow_cards: ms.yellow_cards,
        red_card: ms.red_card,
    }
}

fn award_familiarity_xp(ms: &mut ActiveMatchState, primary: AttrId) {
    let a = primary as usize;
    for (r, weights) in ROLE_WEIGHT_TABLE.iter().enumerate() {
        let xp = if weights[a] == W_KEY {
            FAM_MATCH_BONUS + FAM_XP_KEY_PER_WEEK
        } else if weights[a] == W_IMP {
            FAM_MATCH_BONUS + FAM_XP_IMP_PER_WEEK
        } else {
            Fixed::ZERO
        };
        if xp != Fixed::ZERO {
            ms.familiarity_xp[r] = ms.familiarity_xp[r] + xp;
        }
    }
}

/// Map situation id + choice index to a foul risk level (delegates to discipline module).
/// Named situations that carry inherent foul risk.
fn foul_risk_for_situation(
    situation_id: &str,
    choice_idx: usize,
) -> Option<crate::discipline::FoulRisk> {
    let legacy_id: usize = match situation_id {
        "defensive_tackle" => 7,
        "crucial_tackle" => 23,
        "reckless_challenge" => 26,
        "aerial_duel" => 8,
        _ => return None,
    };
    foul_risk_for(legacy_id, choice_idx).copied()
}

// ── Stub helpers (kept for goat-tui compatibility) ────────────────────────────

pub const STUB_CLUB_STRENGTHS: [u8; 5] = [30, 40, 50, 60, 70];

pub fn stub_strength(club: &str) -> u8 {
    use goat_core::generation::STUB_CLUBS;
    STUB_CLUBS
        .iter()
        .position(|&c| c == club)
        .map(|i| STUB_CLUB_STRENGTHS[i])
        .unwrap_or(50)
}
