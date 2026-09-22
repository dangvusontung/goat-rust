//! Match simulation: the Match Flow engine.
//!
//! A match is no longer a pre-generated sequence of 15 beats. It is a live flow:
//! each tick advances the clock, decides possession from both teams' tactical
//! profiles, drifts the ball between pitch zones, and — when the PC's role zone
//! is involved — stops to offer the player (or the auto-picker) a choice.
//! Outcomes carry transitions (possession / zone / momentum / chain) that shape
//! the next tick, so the match unfolds from the two teams' stats, not from a
//! static script.

use goat_core::attrs::{
    AttrId, ATTR_NAMES, DEFENDING_ATTRS, DRIBBLING_ATTRS, NUM_ATTRS, PASSING_ATTRS, SHOOTING_ATTRS,
};
use goat_core::derive::role_rating;
use goat_core::roles::{
    FamiliarityTier, PitchZone, PositionFamily, RoleId, NUM_ROLES, ROLE_POSITION_FAMILY,
    ROLE_WEIGHT_TABLE, ROLE_ZONE,
};
use goat_core::tactical::{TacticalProfile, TacticalStyle};
use goat_core::tuning::{FAM_XP_IMP_PER_WEEK, FAM_XP_KEY_PER_WEEK, W_IMP, W_KEY};
use goat_fixed::Fixed;
use goat_rng::RngSource;
use goat_traits::PlayerTraits;

use crate::beats::{
    DisciplineEvent, GeneratedBeat, GeneratedChoice, GeneratedOutcome, HeadspaceDelta, Possession,
    ScoreEvent,
};
use crate::beats_data::{RawAction, RawBeatLibrary, RawOutcome, RawSituation};
use crate::contest::{auto_pick_generated_choice, resolve_contest};
use crate::discipline::{red_mist_roll, resolve_card, FoulRisk, RefPersonality};
use crate::headspace::Headspace;

// ── Tuning ────────────────────────────────────────────────────────────────────

const BASE_STAMINA_COST: u8 = 3;
const STARTING_STAMINA: Fixed = Fixed::raw(100_000);
const FAM_MATCH_BONUS: Fixed = Fixed::raw(60);

/// Minutes advanced per flow tick (inclusive range) — match length is dynamic.
const TICK_MIN_MINUTES: u32 = 3;
const TICK_MAX_MINUTES: u32 = 8;
const FULL_TIME: u32 = 90;
/// Minute from which the "late" context activates.
const LATE_MINUTE: u32 = 75;

/// Momentum bounds (user spec: −100..=100, reset to 0 at kickoff).
const MOMENTUM_MAX: i32 = 100;
/// Momentum decays by 1/8th toward zero each tick.
const MOMENTUM_DECAY_DIV: i32 = 8;
/// Momentum swing when an auto-resolved (non-player) beat produces a goal.
const MOMENTUM_GOAL: i32 = 40;
/// `momentum / MOMENTUM_SHARE_DIV` feeds the possession share (±10 pp max).
/// Deliberately weak: the PC's hot streak should not single-handedly tilt the
/// whole match — that is what keeps "starred in defeat" possible.
const MOMENTUM_SHARE_DIV: i32 = 10;
/// PC contest outcomes move match momentum at 1/PC_MOMENTUM_DIV strength —
/// one player shouldn't swing the whole match's momentum on his own.
const PC_MOMENTUM_DIV: i32 = 2;
/// `momentum / MOMENTUM_CONTEST_DIV` feeds the contest roll (±5 pp max).
/// Deliberately weak: match momentum is a TEAM state — letting it dominate the
/// PC's contest roll would couple his output to the scoreline and kill
/// "starred in defeat" (a star on a battered team still plays his own game).
const MOMENTUM_CONTEST_DIV: i32 = 20;

/// `(own.midfield − opp.midfield) / MIDFIELD_SHARE_DIV` tilts possession (±24 pp max).
const MIDFIELD_SHARE_DIV: i32 = 2;
/// Possession share clamps (percent).
const SHARE_MIN: i32 = 10;
const SHARE_MAX: i32 = 90;

/// Base percent chance the possessing side advances one zone per tick, adjusted
/// by `(attack − defense) / 2` of the two relevant lines. On failure the ball
/// stays put — zones are sticky so play can build.
const ZONE_ADVANCE_BASE: i32 = 55;

/// Player-involvement chance per tick: base + quality bonus (better players
/// find the ball — `role_rating` driven).
const INVOLVE_BASE: u32 = 35;
/// Star funnel: when trailing, everything goes through the protagonist —
/// chasing teams lean on their star (+involvement, +zone pull). This is what
/// lets a PC rack up a big rating in a losing effort.
const TRAIL_INVOLVE_BONUS: u32 = 20;
const TRAIL_ZONE_PULL_BONUS: u32 = 15;
/// When the PC is involved, the camera (and the ball) comes to HIS zone this
/// share of the time — a star forward is found in attack even when his team is
/// pinned back. Without this pull, global zone time dominates and protagonists
/// starve.
const ZONE_PULL_PCT: u32 = 55;
/// When a defender's zone is pulled, possession flips to the opposition this
/// share of the time (he is mostly involved to stop them, not to attack).
const DEF_PULL_OPP_PCT: u32 = 70;
/// `(role_rating − 50) / 2`, clamped to this cap — better players find the ball.
const INVOLVE_QUALITY_CAP: i32 = 20;

/// Max auto-resolved chain beats in a single tick before control returns
/// to the player (user spec: 3).
const CHAIN_MAX: u8 = 3;

/// Auto-beat goal probability = AUTO_GOAL_SCALE × att / (att + def + 1), percent.
const AUTO_GOAL_SCALE: u32 = 33;
/// Mercy rule: each goal of lead divides the attacking side's goal chance by
/// (100 + MERCY_DAMPEN)/100 — a team 3 up scores at ~36% of its base rate, so
/// 8-0 blowouts become rare without a hard cap.
const MERCY_DAMPEN: u32 = 60;
/// Trailing boost: each goal BEHIND multiplies the attacking side's goal chance
/// by (100 + TRAIL_BOOST)/100 — teams chasing the game throw bodies forward and
/// score more often. Caps at +3 goals behind; this is also what lets a PC's
/// goals coexist with a team loss (starred-in-defeat).
const TRAIL_BOOST: u32 = 90;
/// Late-game push: from LATE_MINUTE on, the trailing side's goal chance gets an
/// extra (100 + LATE_TRAIL_BOOST)/100 multiplier per goal behind (same cap) —
/// real goals cluster in the final 15 minutes as the losing side goes all-in.
const LATE_TRAIL_BOOST: u32 = 90;
/// Level-late boost: when the score is tied from LATE_MINUTE on, both sides
/// chase the winner — converts passive draws into decisive endings.
const LATE_LEVEL_BOOST: u32 = 300;
/// Score effect on possession: each goal behind adds TRAIL_SHARE_PCT pp to the
/// trailing side's share (capped at TRAIL_SHARE_GOALS goals).
const TRAIL_SHARE_PCT: i32 = 6;
const TRAIL_SHARE_GOALS: i32 = 3;
/// Response surge: in the minutes right after conceding, the stung side pours
/// forward (real matches cluster goals shortly after a goal). For
/// RESPONSE_TICKS ticks the conceding side gets +RESPONSE_SHARE_PCT possession
/// and a (100 + RESPONSE_GOAL_BOOST)/100 goal-chance multiplier. This is also a
/// decoupler: a PC goal invites an opposition reply, so a big PC game can still
/// end in defeat ("starred in defeat").
const RESPONSE_TICKS: u8 = 2;
const RESPONSE_GOAL_BOOST: u64 = 40;
const RESPONSE_SHARE_PCT: i32 = 8;
/// PC contest outcomes: goal outcomes are dropped from the pool entirely once
/// the scoring side leads by this much (text stays coherent — no disallowed
/// "GOAL!" lines).
const MERCY_LEAD: u32 = 4;

/// Style-match: each shared style tag contributes
/// `(own_style + opp_style) / STYLE_MATCH_DIVISOR` percent to a situation's weight.
const STYLE_MATCH_DIVISOR: u64 = 10;
const STYLE_MATCH_MAX: u64 = 40;

/// Opponent-stat difficulty scaling: `(opp_stat − 50) / OPP_DIFFICULTY_DIV`.
const OPP_DIFFICULTY_DIV: i32 = 2;

/// Frustration above this can force a reckless defend beat on the next tick.
const RECKLESS_FRUSTRATION: i32 = 75;

// ── Beat library ──────────────────────────────────────────────────────────────

/// Compiled, ready-to-use beat library loaded from the JSON data file.
///
/// Call `BeatLibrary::load(json_str)` once at startup and pass a reference
/// into every `start_match` call. The library is read-only after construction.
#[derive(Debug, Clone)]
pub struct BeatLibrary {
    raw: RawBeatLibrary,
}

/// Scoreline context for situation filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rel {
    Leading,
    Trailing,
    Level,
}

/// Match-context flags for one pick (by value — no borrows, no leaks).
#[derive(Debug, Clone, Copy)]
struct MatchContext {
    rel: Rel,
    late: bool,
    /// Late and within one goal — the clutch context.
    key: bool,
}

impl MatchContext {
    fn matches(&self, contexts: &[String]) -> bool {
        contexts.is_empty()
            || contexts.iter().any(|c| {
                c == "any"
                    || (c == "late" && self.late)
                    || (c == "key" && self.key)
                    || (c == "leading" && self.rel == Rel::Leading)
                    || (c == "trailing" && self.rel == Rel::Trailing)
                    || (c == "level" && self.rel == Rel::Level)
            })
    }
}

/// Lens for one pick: what the flow currently looks like.
struct FlowLens<'a> {
    zone: PitchZone,
    side: Possession,
    context: MatchContext,
    own: &'a TacticalProfile,
    opp: &'a TacticalProfile,
    traits: &'a PlayerTraits,
    setpiece_bonus: i32,
}

impl BeatLibrary {
    pub fn load(json: &str) -> Result<Self, serde_json::Error> {
        Ok(Self {
            raw: RawBeatLibrary::from_json(json)?,
        })
    }

    /// Weight of a situation under the current lens: base weight × style match
    /// (both teams' profiles) × trait bonus. Zero when the situation doesn't fit
    /// the zone / side / context — that is the filter.
    fn situation_weight(&self, s: &RawSituation, lens: &FlowLens) -> u64 {
        let base = s.weight as u64;
        if base == 0 {
            return 0;
        }
        if let Some(z) = parse_zone(&s.zone) {
            if z != lens.zone {
                return 0;
            }
        }
        if let Some(side) = parse_side(&s.side) {
            if side != lens.side {
                return 0;
            }
        }
        if !lens.context.matches(&s.context) {
            return 0;
        }

        let style_match: u64 = s
            .style
            .iter()
            .filter_map(|t| parse_style(t))
            .map(|st| (lens.own.style(st) as u64 + lens.opp.style(st) as u64) / STYLE_MATCH_DIVISOR)
            .sum::<u64>()
            .min(STYLE_MATCH_MAX);
        // Beat-summoner traits hook onto the situation's style tags.
        let trait_bonus = lens.traits.beat_summoner_bonus_pct(&s.style) as u64;

        base * (100 + style_match) * (100 + trait_bonus)
    }

    /// Weighted pick of a commentary situation under the lens.
    fn pick_situation<'a>(
        &'a self,
        lens: &FlowLens,
        rng: &mut impl RngSource,
    ) -> Option<&'a RawSituation> {
        let total: u64 = self
            .raw
            .situations
            .iter()
            .map(|s| self.situation_weight(s, lens))
            .sum();
        if total == 0 {
            return None;
        }
        let mut roll = rng.next_range_u64(0, total - 1);
        self.raw.situations.iter().find(|s| {
            let w = self.situation_weight(s, lens);
            if roll < w {
                true
            } else {
                roll -= w;
                false
            }
        })
    }

    /// Pick an outcome for a contest result on the given side. Polarity-matched
    /// outcomes are preferred; "any" outcomes fill in when a pool is empty.
    /// Goal outcomes are suppressed once the scoring side leads by MERCY_LEAD.
    fn pick_outcome(
        &self,
        success: bool,
        side: Possession,
        suppress_for: bool,
        suppress_against: bool,
        rng: &mut impl RngSource,
    ) -> Option<GeneratedOutcome> {
        let side_str = side_str(side);
        let polarity = if success { "success" } else { "failure" };
        let allowed = |o: &&RawOutcome| {
            !o.auto_commentary
                && match o.score_event.as_deref() {
                    Some("goal_for") => !suppress_for,
                    Some("goal_against") => !suppress_against,
                    _ => true,
                }
        };
        // Prefer polarity-matched outcomes; "any" outcomes are a fallback pool so
        // contest results stay specific ("buried it", not "the crowd roars").
        let pool: Vec<&RawOutcome> = self
            .raw
            .outcomes
            .iter()
            .filter(|o| {
                allowed(o) && o.polarity == polarity && (o.side == side_str || o.side == "any")
            })
            .collect();
        let pool: Vec<&RawOutcome> = if pool.is_empty() {
            self.raw
                .outcomes
                .iter()
                .filter(|o| {
                    allowed(o) && o.polarity == "any" && (o.side == side_str || o.side == "any")
                })
                .collect()
        } else {
            pool
        };
        if pool.is_empty() {
            return None;
        }
        let raw = pool[rng.next_range_u64(0, pool.len() as u64 - 1) as usize];
        Some(convert_outcome(raw))
    }

    /// Build an interactive beat for the PC under the current lens: commentary
    /// situation + 2–4 actions filtered by zone / role family / side.
    /// Goal outcomes are suppressed per the mercy flags (see pick_outcome).
    fn build_beat(
        &self,
        lens: &FlowLens,
        role: RoleId,
        opp: &TacticalProfile,
        suppress_for: bool,
        suppress_against: bool,
        rng: &mut impl RngSource,
    ) -> Option<GeneratedBeat> {
        let situation = self.pick_situation(lens, rng)?;

        let family = role_family_str(ROLE_POSITION_FAMILY[role as usize]);
        let zone_str = zone_str(lens.zone);
        let side = side_str(lens.side);
        let eligible: Vec<&RawAction> = self
            .raw
            .actions
            .iter()
            .filter(|a| {
                a.side == side
                    && a.roles.iter().any(|r| r == family)
                    && a.zones.iter().any(|z| z == zone_str || z == "any")
            })
            .collect();
        if eligible.is_empty() {
            return None;
        }

        let n = (rng.next_range_u64(2, 4) as usize).min(eligible.len());
        let mut pool: Vec<usize> = (0..eligible.len()).collect();
        let mut choices: Vec<GeneratedChoice> = Vec::with_capacity(n);
        for _ in 0..n {
            if pool.is_empty() {
                break;
            }
            let pick = rng.next_range_u64(0, pool.len() as u64 - 1) as usize;
            let raw_action = eligible[pool[pick]];
            pool.remove(pick);

            let Some(attr) = parse_attr(&raw_action.attr) else {
                continue;
            };
            let (Some(success), Some(failure)) = (
                self.pick_outcome(true, lens.side, suppress_for, suppress_against, rng),
                self.pick_outcome(false, lens.side, suppress_for, suppress_against, rng),
            ) else {
                continue;
            };
            choices.push(GeneratedChoice {
                text: raw_action.text.clone(),
                primary: attr,
                difficulty: scaled_difficulty(
                    raw_action.difficulty,
                    attr,
                    opp,
                    lens.setpiece_bonus,
                ),
                foul_chance: raw_action.foul_chance,
                foul_serious: raw_action.foul_serious,
                success,
                failure,
            });
        }
        if choices.is_empty() {
            return None;
        }

        Some(GeneratedBeat {
            situation_id: situation.id.clone(),
            setup: situation.text.clone(),
            zone: lens.zone,
            side: lens.side,
            choices,
        })
    }

    /// Pick a goal commentary outcome for an auto-resolved score event.
    fn pick_goal_text(&self, event: ScoreEvent, rng: &mut impl RngSource) -> Option<String> {
        let (ev, side) = match event {
            ScoreEvent::GoalFor => ("goal_for", "attack"),
            ScoreEvent::GoalAgainst => ("goal_against", "defend"),
        };
        let pool: Vec<&RawOutcome> = self
            .raw
            .outcomes
            .iter()
            .filter(|o| {
                o.auto_commentary
                    && o.score_event.as_deref() == Some(ev)
                    && (o.side == side || o.side == "any")
            })
            .collect();
        if pool.is_empty() {
            return None;
        }
        Some(
            pool[rng.next_range_u64(0, pool.len() as u64 - 1) as usize]
                .text
                .clone(),
        )
    }
}

// ── Public types ──────────────────────────────────────────────────────────────

/// Everything the match engine needs to run a match.
#[derive(Debug, Clone)]
pub struct MatchSetup {
    pub player_role: RoleId,
    pub player_attrs: [Fixed; NUM_ATTRS],
    pub player_familiarity: [FamiliarityTier; NUM_ROLES],
    /// Tactical profile of the PC's team — drives possession, zone drift, situation weights.
    pub own_profile: TacticalProfile,
    /// Tactical profile of the opposition — drives the same, plus contest difficulty.
    pub opp_profile: TacticalProfile,
    pub opp_name: &'static str,
    pub form: Fixed,
    pub player_aggression: u8,
    pub ref_personality: RefPersonality,
    pub dirty_rep: i32,
    pub player_traits: PlayerTraits,
    /// Club-staff effects on this match (stamina, headspace, set pieces).
    /// `StaffMods::NEUTRAL` = no staff influence.
    pub staff_mods: goat_core::staff::StaffMods,
}

/// Summary of a single flow moment for the commentary feed / post-match recap.
#[derive(Debug, Clone)]
pub struct MomentSummary {
    pub beat_id: String,
    pub minute: u32,
    pub choice_idx: usize,
    pub success: bool,
    pub setup_text: String,
    pub outcome_text: String,
    pub goal_event: Option<ScoreEvent>,
    /// True when the PC made (or auto-picked) a choice; false = pure commentary.
    pub is_action: bool,
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

/// Live match state, advanced tick by tick through the flow.
#[derive(Debug, Clone)]
pub struct ActiveMatchState {
    pub setup: MatchSetup,
    pub minute: u32,
    pub possession: Possession,
    pub zone: PitchZone,
    pub momentum: i32,
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
    /// Beat waiting for a player decision; `None` while the flow auto-runs.
    current: Option<GeneratedBeat>,
    /// Frustration flag: next tick forces a reckless defend beat.
    force_reckless: bool,
    /// Response surge: ticks remaining of the conceding side's post-goal push.
    response_ticks: u8,
    response_side: Possession,
}

impl ActiveMatchState {
    pub fn current_beat(&self) -> Option<&GeneratedBeat> {
        self.current.as_ref()
    }

    pub fn current_minute(&self) -> u32 {
        self.minute
    }
}

// ── Match initialisation ──────────────────────────────────────────────────────

/// Start a new match: initialise the flow at kickoff and run it until the first
/// player decision (or full time).
pub fn start_match(
    lib: &BeatLibrary,
    setup: MatchSetup,
    rng: &mut impl RngSource,
) -> ActiveMatchState {
    let headspace = Headspace::from_form(setup.form);
    // Kickoff possession: the midfield battle decides.
    let share = possession_share(setup.own_profile.midfield, setup.opp_profile.midfield, 0, 0);
    let possession = if rng.next_range_u64(1, 100) <= share {
        Possession::Own
    } else {
        Possession::Opp
    };
    let mut ms = ActiveMatchState {
        headspace,
        setup,
        minute: 0,
        possession,
        zone: PitchZone::Midfield,
        momentum: 0,
        player_output: 50,
        stamina: STARTING_STAMINA,
        goals_for: 0,
        goals_against: 0,
        yellow_cards: 0,
        red_card: false,
        moments: Vec::new(),
        familiarity_xp: [Fixed::ZERO; NUM_ROLES],
        is_complete: false,
        final_result: None,
        current: None,
        force_reckless: false,
        response_ticks: 0,
        response_side: Possession::Opp,
    };
    run_until_decision(&mut ms, lib, rng);
    ms
}

/// Advance the flow with the player's choice for the pending beat, then run on
/// until the next decision point or full time.
pub fn advance_beat(
    mut ms: ActiveMatchState,
    choice_idx: usize,
    lib: &BeatLibrary,
    rng: &mut impl RngSource,
) -> ActiveMatchState {
    let Some(beat) = ms.current.take() else {
        run_until_decision(&mut ms, lib, rng);
        return ms;
    };

    let choice_idx = choice_idx.min(beat.choices.len().saturating_sub(1));
    resolve_choice(&mut ms, &beat, choice_idx, lib, rng, 0);
    if ms.is_complete {
        return ms; // red card during resolution
    }

    // Red mist: extreme frustration can produce a card without any tactical choice.
    if !ms.red_card {
        if let Some(card) = red_mist_roll(
            ms.headspace.frustration,
            ms.setup.player_aggression,
            ms.setup.dirty_rep,
            ms.setup.ref_personality,
            ms.yellow_cards,
            rng,
        ) {
            apply_card(&mut ms, card);
            if ms.is_complete {
                return ms;
            }
        }
        // Frustration injection: temper forces a reckless defend beat next tick.
        if ms.headspace.frustration > RECKLESS_FRUSTRATION && rng.next_range_u64(0, 9) < 3 {
            ms.force_reckless = true;
        }
    }

    ms.headspace
        .tick(ms.setup.player_attrs[AttrId::Composure as usize]);

    run_until_decision(&mut ms, lib, rng);
    ms
}

/// Auto-play an entire match (skip-match / idle path).
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

// ── The flow ──────────────────────────────────────────────────────────────────

/// Run tick after tick until the PC is involved (a beat waits for a decision)
/// or the match ends.
fn run_until_decision(ms: &mut ActiveMatchState, lib: &BeatLibrary, rng: &mut impl RngSource) {
    while !ms.is_complete && ms.current.is_none() {
        tick(ms, lib, rng);
    }
}

fn tick(ms: &mut ActiveMatchState, lib: &BeatLibrary, rng: &mut impl RngSource) {
    // 1. Clock.
    ms.minute = (ms.minute + rng.next_range_u32(TICK_MIN_MINUTES, TICK_MAX_MINUTES)).min(FULL_TIME);

    // 2. Momentum decays toward zero; the conceding side's surge winds down.
    ms.momentum -= ms.momentum / MOMENTUM_DECAY_DIV;
    ms.response_ticks = ms.response_ticks.saturating_sub(1);

    // 3. Possession battle.
    let mut share = possession_share(
        ms.setup.own_profile.midfield,
        ms.setup.opp_profile.midfield,
        ms.momentum,
        ms.goals_for as i32 - ms.goals_against as i32,
    ) as i32;
    if ms.response_ticks > 0 {
        share += match ms.response_side {
            Possession::Own => RESPONSE_SHARE_PCT,
            Possession::Opp => -RESPONSE_SHARE_PCT,
        };
    }
    let share = share.clamp(SHARE_MIN, SHARE_MAX) as u64;
    ms.possession = if rng.next_range_u64(1, 100) <= share {
        Possession::Own
    } else {
        Possession::Opp
    };

    // 4. Zone drift for the possessing side.
    drift_zone(ms, rng);

    // 5. Frustration override: a reckless beat hijacks the tick.
    if ms.force_reckless {
        ms.force_reckless = false;
        ms.possession = Possession::Opp;
        ms.zone = PitchZone::Defense;
        let lens = make_lens(ms);
        let (sf, sa) = mercy_flags(ms);
        if let Some(beat) = lib.build_beat(
            &lens,
            ms.setup.player_role,
            &ms.setup.opp_profile,
            sf,
            sa,
            rng,
        ) {
            ms.current = Some(beat);
            return;
        }
    }

    // 6. Player involvement — the camera (and the ball) comes to the protagonist.
    if involved(ms, rng) {
        let role = ms.setup.player_role;
        // Pull the action to his zone most of the time; possession follows the
        // role's natural side (a forward is found up front, a defender is mostly
        // found stopping the opposition). When chasing the game, the team looks
        // for him even more.
        let pull = ZONE_PULL_PCT
            + if ms.goals_for < ms.goals_against {
                TRAIL_ZONE_PULL_BONUS
            } else {
                0
            };
        if rng.next_range_u64(1, 100) <= pull as u64 {
            ms.zone = ROLE_ZONE[role as usize];
            match ROLE_POSITION_FAMILY[role as usize] {
                PositionFamily::Forward => ms.possession = Possession::Own,
                PositionFamily::Defender => {
                    if rng.next_range_u64(1, 100) <= DEF_PULL_OPP_PCT as u64 {
                        ms.possession = Possession::Opp;
                    }
                }
                PositionFamily::Midfielder => {}
            }
        }
        let lens = make_lens(ms);
        let (sf, sa) = mercy_flags(ms);
        if let Some(beat) = lib.build_beat(
            &lens,
            ms.setup.player_role,
            &ms.setup.opp_profile,
            sf,
            sa,
            rng,
        ) {
            ms.current = Some(beat);
            return;
        }
    }

    // 7. The world moves on without the PC: commentary + possible goal.
    auto_beat(ms, lib, rng);

    if ms.minute >= FULL_TIME {
        finalize(ms);
    }
}

/// Mercy flags for beat building: suppress goal outcomes for a side already
/// leading by MERCY_LEAD or more.
fn mercy_flags(ms: &ActiveMatchState) -> (bool, bool) {
    (
        ms.goals_for >= ms.goals_against + MERCY_LEAD,
        ms.goals_against >= ms.goals_for + MERCY_LEAD,
    )
}

/// A goal was just scored: the conceding side surges for the next few ticks.
fn set_response_surge(ms: &mut ActiveMatchState, conceding_side: Possession) {
    ms.response_side = conceding_side;
    ms.response_ticks = RESPONSE_TICKS;
}

/// Soft-capped rating movement: deltas shrink as output nears the 0/100 rails,
/// so ratings taper off instead of piling up at the ceiling (the 90–100 bucket
/// was overweight when strong PCs kept adding full deltas at 95+). The factor
/// never reaches zero — a strong PC can still grind out a 90+ rating, just
/// slower.
fn apply_output_delta(output: i32, delta: i32) -> i32 {
    let scaled = if delta >= 0 {
        delta * (200 - output) / 200
    } else {
        delta * (100 + output) / 200
    };
    // Keep at least ±1 of movement so a beat never feels like a no-op.
    let scaled = if delta != 0 && scaled == 0 {
        delta.signum()
    } else {
        scaled
    };
    (output + scaled).clamp(0, 100)
}

/// Possession share for the own team, in percent. Includes the score effect:
/// the trailing side sees more of the ball (desperation football), capped at
/// ±TRAIL_SHARE_GOALS goals of difference.
fn possession_share(own_mid: u8, opp_mid: u8, momentum: i32, goal_diff: i32) -> u64 {
    let trail = (-goal_diff).clamp(-TRAIL_SHARE_GOALS, TRAIL_SHARE_GOALS) * TRAIL_SHARE_PCT;
    let share = 50
        + (own_mid as i32 - opp_mid as i32) / MIDFIELD_SHARE_DIV
        + momentum / MOMENTUM_SHARE_DIV
        + trail;
    share.clamp(SHARE_MIN, SHARE_MAX) as u64
}

/// Move the ball one step toward the possessing side's attack (or back).
fn drift_zone(ms: &mut ActiveMatchState, rng: &mut impl RngSource) {
    let (att, def) = match ms.possession {
        Possession::Own => (ms.setup.own_profile.attack, ms.setup.opp_profile.defense),
        Possession::Opp => (ms.setup.opp_profile.attack, ms.setup.own_profile.defense),
    };
    let advance_pct = ZONE_ADVANCE_BASE + (att as i32 - def as i32) / 2;
    let advance = rng.next_range_u64(1, 100) <= advance_pct.clamp(5, 95) as u64;

    ms.zone = match (ms.possession, ms.zone, advance) {
        // Own ball: push toward their third; zones are sticky on failure.
        (Possession::Own, PitchZone::Defense, true) => PitchZone::Midfield,
        (Possession::Own, PitchZone::Midfield, true) => wide_or_central(&ms.setup.own_profile, rng),
        (Possession::Own, PitchZone::AttackWide, true) => PitchZone::AttackCentral,
        (Possession::Own, PitchZone::AttackCentral, true) => PitchZone::AttackCentral,
        (Possession::Own, _, false) => ms.zone,
        // Opp ball (their attack is our third): press into our defense.
        (Possession::Opp, PitchZone::Midfield, true) => PitchZone::Defense,
        (Possession::Opp, PitchZone::AttackWide | PitchZone::AttackCentral, true) => {
            PitchZone::Defense
        }
        (Possession::Opp, _, false) => ms.zone,
        // Opp already deep: stays camped regardless.
        (Possession::Opp, PitchZone::Defense, true) => PitchZone::Defense,
    };
}

/// Wing-play weight decides whether an attack develops wide or centrally.
fn wide_or_central(profile: &TacticalProfile, rng: &mut impl RngSource) -> PitchZone {
    let wide_pct =
        (profile.wing_play as u64 * 100) / (profile.wing_play + profile.possession + 1) as u64;
    if rng.next_range_u64(1, 100) <= wide_pct {
        PitchZone::AttackWide
    } else {
        PitchZone::AttackCentral
    }
}

/// Does the PC get into this tick's action? Base rate plus quality — better
/// players find the ball (stars bend the game toward them). When the team is
/// trailing, the ball is funnelled to the protagonist.
fn involved(ms: &ActiveMatchState, rng: &mut impl RngSource) -> bool {
    let role = ms.setup.player_role;
    let rating = role_rating(
        &ms.setup.player_attrs,
        role,
        ms.setup.player_familiarity[role as usize],
    )
    .to_int();
    let mut chance =
        INVOLVE_BASE.saturating_add(((rating - 50) / 2).clamp(0, INVOLVE_QUALITY_CAP) as u32);
    if ms.goals_for < ms.goals_against {
        chance = chance.saturating_add(TRAIL_INVOLVE_BONUS);
    }
    rng.next_range_u64(1, 100) <= chance as u64
}

/// Auto-resolve a tick the PC is not involved in: commentary, plus a goal roll
/// when the possessing side is deep in attacking territory.
fn auto_beat(ms: &mut ActiveMatchState, lib: &BeatLibrary, rng: &mut impl RngSource) {
    let lens = make_lens(ms);
    let situation = lib.pick_situation(&lens, rng);
    let mut text = situation.map(|s| s.text.clone()).unwrap_or_default();
    let beat_id = situation.map(|s| s.id.clone()).unwrap_or_default();

    let attacking = matches!(
        (ms.possession, ms.zone),
        (Possession::Own, PitchZone::AttackWide)
            | (Possession::Own, PitchZone::AttackCentral)
            | (Possession::Opp, PitchZone::Defense)
    );
    let mut goal_event = None;
    if attacking {
        let (att, def) = match ms.possession {
            Possession::Own => (ms.setup.own_profile.attack, ms.setup.opp_profile.defense),
            Possession::Opp => (ms.setup.opp_profile.attack, ms.setup.own_profile.defense),
        };
        let p = AUTO_GOAL_SCALE as u64 * att as u64 / (att as u64 + def as u64 + 1);
        // Mercy rule: the side already leading gets its goal chance dampened,
        // the side chasing gets a boost (desperation football).
        let diff = match ms.possession {
            Possession::Own => ms.goals_for as i64 - ms.goals_against as i64,
            Possession::Opp => ms.goals_against as i64 - ms.goals_for as i64,
        };
        let p = if diff > 0 {
            p * 100 / (100 + diff as u64 * MERCY_DAMPEN as u64)
        } else if diff < 0 {
            let behind = (-diff).min(3) as u64;
            let mut boost = behind * TRAIL_BOOST as u64;
            if ms.minute >= LATE_MINUTE {
                boost += behind * LATE_TRAIL_BOOST as u64;
            }
            p * (100 + boost) / 100
        } else if ms.minute >= LATE_MINUTE {
            // Level late: both sides chase the winner instead of settling.
            p * (100 + LATE_LEVEL_BOOST as u64) / 100
        } else {
            p
        };
        // Response surge: the side that just conceded is stung into a reply.
        let p = if ms.response_ticks > 0 && ms.possession == ms.response_side {
            p * (100 + RESPONSE_GOAL_BOOST) / 100
        } else {
            p
        };
        if rng.next_range_u64(1, 100) <= p.max(1) {
            let ev = match ms.possession {
                Possession::Own => ScoreEvent::GoalFor,
                Possession::Opp => ScoreEvent::GoalAgainst,
            };
            match ev {
                ScoreEvent::GoalFor => {
                    ms.goals_for += 1;
                    ms.momentum += MOMENTUM_GOAL;
                }
                ScoreEvent::GoalAgainst => {
                    ms.goals_against += 1;
                    ms.momentum -= MOMENTUM_GOAL;
                }
            }
            ms.momentum = ms.momentum.clamp(-MOMENTUM_MAX, MOMENTUM_MAX);
            // Kickoff: the conceding side restarts with the ball — and surges.
            ms.possession = match ev {
                ScoreEvent::GoalFor => Possession::Opp,
                ScoreEvent::GoalAgainst => Possession::Own,
            };
            ms.zone = PitchZone::Midfield;
            set_response_surge(ms, ms.possession);
            if let Some(t) = lib.pick_goal_text(ev, rng) {
                text = t;
            }
            goal_event = Some(ev);
        }
    }

    ms.moments.push(MomentSummary {
        beat_id,
        minute: ms.minute,
        choice_idx: 0,
        success: false,
        setup_text: text.clone(),
        outcome_text: text,
        goal_event,
        is_action: false,
    });
}

/// Resolve one choice (player-made or auto-picked in a chain): contest, outcome
/// application, discipline, then any chained follow-up beats.
fn resolve_choice(
    ms: &mut ActiveMatchState,
    beat: &GeneratedBeat,
    choice_idx: usize,
    lib: &BeatLibrary,
    rng: &mut impl RngSource,
    chain_depth: u8,
) {
    let choice = &beat.choices[choice_idx.min(beat.choices.len() - 1)];

    let goals_behind = ms.goals_against as i32 - ms.goals_for as i32;
    let minutes_remaining = FULL_TIME.saturating_sub(ms.minute);
    let desp_mod = Headspace::desperation_mod(
        goals_behind,
        minutes_remaining,
        ms.setup.player_attrs[AttrId::Composure as usize],
    );
    let context_mod = desp_mod + ms.momentum / MOMENTUM_CONTEST_DIV;

    let success = resolve_contest(
        ms.setup.player_attrs[choice.primary as usize],
        choice.difficulty,
        &ms.headspace,
        ms.stamina,
        context_mod,
        rng,
    );
    let outcome = if success {
        &choice.success
    } else {
        &choice.failure
    };

    ms.player_output = apply_output_delta(ms.player_output, outcome.output_delta as i32);
    {
        let mods = &ms.setup.staff_mods;
        let d = &outcome.headspace;
        // Club psychologist: amplify the good days, take the edge off the bad ones.
        let scaled = crate::beats::HeadspaceDelta {
            confidence: if d.confidence > 0 {
                d.confidence as i32 * mods.headspace_gain_pct / 1000
            } else {
                d.confidence as i32
            } as i8,
            frustration: if d.frustration > 0 {
                d.frustration as i32 * mods.frustration_gain_pct / 1000
            } else {
                d.frustration as i32
            } as i8,
            flow: if d.flow > 0 {
                d.flow as i32 * mods.headspace_gain_pct / 1000
            } else {
                d.flow as i32
            } as i8,
        };
        ms.headspace
            .apply(&scaled, ms.setup.player_attrs[AttrId::Composure as usize]);
    }
    let stamina_cost = Fixed::from_int(
        (BASE_STAMINA_COST + outcome.stamina_cost) as i32 * ms.setup.staff_mods.stamina_cost_pct
            / 1000,
    );
    ms.stamina = (ms.stamina - stamina_cost).clamp(Fixed::ZERO, STARTING_STAMINA);

    if let Some(ev) = outcome.score_event {
        match ev {
            ScoreEvent::GoalFor => ms.goals_for += 1,
            ScoreEvent::GoalAgainst => ms.goals_against += 1,
        }
        // Kickoff: the side that conceded restarts with the ball (real-football
        // rule — and a natural decoupler: PC goals hand the initiative to the
        // opposition, so a great PC game can still end in defeat).
        ms.possession = match ev {
            ScoreEvent::GoalFor => Possession::Opp,
            ScoreEvent::GoalAgainst => Possession::Own,
        };
        ms.zone = PitchZone::Midfield;
        set_response_surge(ms, ms.possession);
    }

    // The PC is 1 of 11 players: his outcomes move match momentum at half
    // strength, so a hot streak helps but doesn't run the whole game.
    ms.momentum = (ms.momentum + outcome.momentum_delta as i32 / PC_MOMENTUM_DIV)
        .clamp(-MOMENTUM_MAX, MOMENTUM_MAX);
    if let Some(p) = outcome.possession_to {
        ms.possession = p;
    }
    if let Some(z) = outcome.zone_to {
        ms.zone = z;
    }

    award_familiarity_xp(ms, choice.primary);

    ms.moments.push(MomentSummary {
        beat_id: beat.situation_id.clone(),
        minute: ms.minute,
        choice_idx,
        success,
        setup_text: beat.setup.clone(),
        outcome_text: outcome.text.clone(),
        goal_event: outcome.score_event,
        is_action: true,
    });

    // Discipline: foul review from the action's own foul data.
    if !ms.red_card && choice.foul_chance > 0 {
        let risk = FoulRisk {
            foul_chance: choice.foul_chance,
            serious: choice.foul_serious,
        };
        if let Some(card) = resolve_card(
            &risk,
            ms.setup.player_aggression,
            ms.headspace.frustration,
            ms.setup.dirty_rep,
            ms.setup.ref_personality,
            ms.yellow_cards,
            rng,
        ) {
            apply_card(ms, card);
            if ms.is_complete {
                return;
            }
        }
    }

    // Chain: the outcome springs an immediate follow-up beat (auto-resolved,
    // capped at CHAIN_MAX per tick so the player always gets control back).
    if let Some(chain_side) = outcome.chain {
        if chain_depth < CHAIN_MAX && !ms.red_card && ms.minute < FULL_TIME {
            ms.possession = chain_side;
            let lens = make_lens(ms);
            let (sf, sa) = mercy_flags(ms);
            if let Some(next) = lib.build_beat(
                &lens,
                ms.setup.player_role,
                &ms.setup.opp_profile,
                sf,
                sa,
                rng,
            ) {
                let idx = auto_pick_generated_choice(&next.choices, &ms.setup.player_attrs);
                resolve_choice(ms, &next, idx, lib, rng, chain_depth + 1);
            }
        }
    }
}

fn apply_card(ms: &mut ActiveMatchState, card: DisciplineEvent) {
    match card {
        DisciplineEvent::YellowCard => ms.yellow_cards += 1,
        DisciplineEvent::RedCard => {
            ms.red_card = true;
            finalize(ms);
        }
    }
}

fn finalize(ms: &mut ActiveMatchState) {
    ms.is_complete = true;
    ms.current = None;
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

// ── Lens & parsing helpers ────────────────────────────────────────────────────

fn make_lens(ms: &ActiveMatchState) -> FlowLens<'_> {
    let late = ms.minute >= LATE_MINUTE;
    let rel = match ms.goals_for.cmp(&ms.goals_against) {
        std::cmp::Ordering::Greater => Rel::Leading,
        std::cmp::Ordering::Less => Rel::Trailing,
        std::cmp::Ordering::Equal => Rel::Level,
    };
    let key = late && (ms.goals_for as i32 - ms.goals_against as i32).abs() <= 1;
    FlowLens {
        zone: ms.zone,
        side: ms.possession,
        context: MatchContext { rel, late, key },
        own: &ms.setup.own_profile,
        opp: &ms.setup.opp_profile,
        traits: &ms.setup.player_traits,
        setpiece_bonus: ms.setup.staff_mods.setpiece_bonus,
    }
}

fn parse_zone(s: &str) -> Option<PitchZone> {
    match s {
        "defense" => Some(PitchZone::Defense),
        "midfield" => Some(PitchZone::Midfield),
        "attack_wide" => Some(PitchZone::AttackWide),
        "attack_central" => Some(PitchZone::AttackCentral),
        _ => None, // "any"
    }
}

fn zone_str(z: PitchZone) -> &'static str {
    match z {
        PitchZone::Defense => "defense",
        PitchZone::Midfield => "midfield",
        PitchZone::AttackWide => "attack_wide",
        PitchZone::AttackCentral => "attack_central",
    }
}

fn parse_side(s: &str) -> Option<Possession> {
    match s {
        "attack" => Some(Possession::Own),
        "defend" => Some(Possession::Opp),
        _ => None, // "any"
    }
}

fn side_str(p: Possession) -> &'static str {
    match p {
        Possession::Own => "attack",
        Possession::Opp => "defend",
    }
}

fn parse_style(s: &str) -> Option<TacticalStyle> {
    match s {
        "pressing" => Some(TacticalStyle::Pressing),
        "possession" => Some(TacticalStyle::Possession),
        "counter" => Some(TacticalStyle::Counter),
        "wing_play" => Some(TacticalStyle::WingPlay),
        _ => None,
    }
}

fn role_family_str(f: PositionFamily) -> &'static str {
    match f {
        PositionFamily::Defender => "defender",
        PositionFamily::Midfielder => "midfielder",
        PositionFamily::Forward => "forward",
    }
}

/// Contest difficulty scaled by the opponent's relevant line stat:
/// attacking attrs test against their defense, defensive attrs against their
/// attack, everything else against the midpoint. Set-piece attrs (FreeKickAcc,
/// Heading) get a flat reduction from the set-piece coach.
fn scaled_difficulty(base: u8, attr: AttrId, opp: &TacticalProfile, setpiece_bonus: i32) -> u8 {
    let idx = attr as usize;
    let stat = if DEFENDING_ATTRS.contains(&idx) {
        opp.attack as i32
    } else if SHOOTING_ATTRS.contains(&idx)
        || PASSING_ATTRS.contains(&idx)
        || DRIBBLING_ATTRS.contains(&idx)
    {
        opp.defense as i32
    } else {
        (opp.attack as i32 + opp.defense as i32) / 2
    };
    let sp = if attr == AttrId::FreeKickAcc || attr == AttrId::Heading {
        setpiece_bonus
    } else {
        0
    };
    (base as i32 + (stat - 50) / OPP_DIFFICULTY_DIV - sp).clamp(1, 99) as u8
}

fn convert_outcome(raw: &RawOutcome) -> GeneratedOutcome {
    GeneratedOutcome {
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
        momentum_delta: raw.momentum_delta,
        possession_to: raw.possession_to.as_deref().and_then(|s| match s {
            "own" => Some(Possession::Own),
            "opp" => Some(Possession::Opp),
            _ => None,
        }),
        zone_to: raw.zone_to.as_deref().and_then(parse_zone),
        chain: raw.chain.as_deref().and_then(parse_side),
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
        "AttPositioning" => Some(AttrId::AttPositioning),
        "Positioning" => Some(AttrId::AttPositioning), // legacy alias
        "Strength" => Some(AttrId::Strength),
        "Stamina" => Some(AttrId::Stamina),
        "Aggression" => Some(AttrId::Aggression),
        "Jumping" => Some(AttrId::Jumping),
        "Composure" => Some(AttrId::Composure),
        "Bravery" => Some(AttrId::Bravery),
        "SlidingTackle" => Some(AttrId::SlidingTackle),
        _ => None,
    }
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
