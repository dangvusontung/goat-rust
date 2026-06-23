//! Starter beat library — 26 hand-authored beats.
//!
//! All player-facing text lives here; the TUI prints it unchanged.
//! Beat IDs 0–25 are load-bearing: the sim references them by constant index.

use crate::beats::{Beat, BeatChoice, HeadspaceDelta, MatchPhase, Outcome, ScoreEvent};
use goat_core::attrs::AttrId::*;

// ── Beat ID constants ─────────────────────────────────────────────────────────
pub const B_FLANK_RECEIVE: usize = 0;
pub const B_DRIBBLE_CHALLENGE: usize = 1;
pub const B_CROSS_OPPORTUNITY: usize = 2;
pub const B_THROUGH_BALL: usize = 3;
pub const B_SHOT_SETUP: usize = 4;
pub const B_BOX_ENTRY: usize = 5;
pub const B_DEFEND_TRACK: usize = 6;
pub const B_DEFEND_TACKLE: usize = 7;
pub const B_DEFEND_AERIAL: usize = 8;
pub const B_DEFEND_INTERCEPT: usize = 9;
pub const B_ONE_ON_ONE: usize = 10;
pub const B_ONE_ON_ONE_WIDE: usize = 11;
pub const B_FREE_KICK_DIRECT: usize = 12;
pub const B_FREE_KICK_CROSS: usize = 13;
pub const B_CORNER_ATTACK: usize = 14;
pub const B_CORNER_DEFEND: usize = 15;
pub const B_PENALTY: usize = 16;
pub const B_FIND_SPACE: usize = 17;
pub const B_PRESS_TRIGGER: usize = 18;
pub const B_SUPPORT_RUN: usize = 19;
pub const B_HOLD_SHAPE: usize = 20;
pub const B_LAST_MINUTE_CHANCE: usize = 21;
pub const B_LAST_MINUTE_FINISH: usize = 22;
pub const B_CRUCIAL_TACKLE: usize = 23;
pub const B_COMEBACK_MOMENT: usize = 24;
pub const B_WONDER_STRIKE: usize = 25;
/// Frustration-injected beat — never selected by the normal period selector.
/// Only inserted when headspace.frustration > 75.
pub const B_RECKLESS_CHALLENGE: usize = 26;

pub const NUM_BEATS: usize = 27;

macro_rules! hd {
    ($c:expr, $fr:expr, $fl:expr) => {
        HeadspaceDelta {
            confidence: $c,
            frustration: $fr,
            flow: $fl,
        }
    };
}

pub const BEATS: [Beat; NUM_BEATS] = [

// 0 ── FLANK_RECEIVE ──────────────────────────────────────────────────────────
Beat {
    id: B_FLANK_RECEIVE,
    phase: MatchPhase::OpenPlayAttack,
    phase_bias: [3, 3, 2],
    setup: "Ball reaches you on the wing with a yard of space. A defender closes fast.",
    choices: &[
        BeatChoice {
            text: "Drive at him — take him on",
            primary: CloseControl,
            difficulty: 55,
            success: Outcome { text: "You glide past, opening up the flank.", next: Some(B_CROSS_OPPORTUNITY), output_delta: 8, headspace: hd!(5,-2,5), score_event: None, stamina_cost: 5 },
            failure: Outcome { text: "He shepherds you wide and wins it back.", next: None, output_delta: -4, headspace: hd!(-3,5,-3), score_event: None, stamina_cost: 3 },
        },
        BeatChoice {
            text: "Lay it off — keep it simple",
            primary: ShortPassing,
            difficulty: 38,
            success: Outcome { text: "You recycle it smartly, maintaining possession.", next: None, output_delta: 2, headspace: hd!(1,0,0), score_event: None, stamina_cost: 1 },
            failure: Outcome { text: "Your pass is misplaced under pressure.", next: None, output_delta: -5, headspace: hd!(-2,4,-2), score_event: None, stamina_cost: 2 },
        },
        BeatChoice {
            text: "Hit a low cross first time",
            primary: Crossing,
            difficulty: 62,
            success: Outcome { text: "Your early cross whips across the six-yard box.", next: Some(B_BOX_ENTRY), output_delta: 6, headspace: hd!(3,-1,3), score_event: None, stamina_cost: 3 },
            failure: Outcome { text: "The ball goes behind for a goal kick.", next: None, output_delta: -2, headspace: hd!(-1,2,-1), score_event: None, stamina_cost: 2 },
        },
    ],
},

// 1 ── DRIBBLE_CHALLENGE ──────────────────────────────────────────────────────
Beat {
    id: B_DRIBBLE_CHALLENGE,
    phase: MatchPhase::OpenPlayAttack,
    phase_bias: [2, 3, 2],
    setup: "You carry the ball into space. A defender plants his feet — man-marking tight.",
    choices: &[
        BeatChoice {
            text: "Drop your shoulder and burst past",
            primary: Agility,
            difficulty: 58,
            success: Outcome { text: "A sharp change of pace and he's beaten.", next: Some(B_SHOT_SETUP), output_delta: 10, headspace: hd!(6,-2,6), score_event: None, stamina_cost: 6 },
            failure: Outcome { text: "He reads the move and wins the ball.", next: None, output_delta: -5, headspace: hd!(-3,6,-3), score_event: None, stamina_cost: 4 },
        },
        BeatChoice {
            text: "Screen the ball — draw the foul",
            primary: Balance,
            difficulty: 48,
            success: Outcome { text: "You hold him off brilliantly. Free kick won.", next: Some(B_FREE_KICK_DIRECT), output_delta: 5, headspace: hd!(3,-1,2), score_event: None, stamina_cost: 3 },
            failure: Outcome { text: "He strips the ball cleanly — no foul given.", next: None, output_delta: -3, headspace: hd!(-2,4,-2), score_event: None, stamina_cost: 2 },
        },
    ],
},

// 2 ── CROSS_OPPORTUNITY ──────────────────────────────────────────────────────
Beat {
    id: B_CROSS_OPPORTUNITY,
    phase: MatchPhase::OpenPlayAttack,
    phase_bias: [2, 3, 3],
    setup: "You've got space to deliver from the wide area. Runners are making their moves in the box.",
    choices: &[
        BeatChoice {
            text: "Whip a cross to the near post",
            primary: Crossing,
            difficulty: 55,
            success: Outcome { text: "A vicious cross — the keeper can't hold it.", next: Some(B_BOX_ENTRY), output_delta: 8, headspace: hd!(4,-1,4), score_event: None, stamina_cost: 2 },
            failure: Outcome { text: "It sails over everyone — cleared comfortably.", next: None, output_delta: -3, headspace: hd!(-2,3,-2), score_event: None, stamina_cost: 2 },
        },
        BeatChoice {
            text: "Float it to the back post",
            primary: LongPassing,
            difficulty: 50,
            success: Outcome { text: "Perfect delivery — your teammate nods it on.", next: Some(B_CORNER_ATTACK), output_delta: 7, headspace: hd!(3,-1,3), score_event: None, stamina_cost: 2 },
            failure: Outcome { text: "The ball is too long — goal kick.", next: None, output_delta: -2, headspace: hd!(-1,2,-1), score_event: None, stamina_cost: 1 },
        },
        BeatChoice {
            text: "Pull it back for a shot",
            primary: Vision,
            difficulty: 45,
            success: Outcome { text: "You thread it back perfectly — a tap-in opportunity.", next: Some(B_SHOT_SETUP), output_delta: 9, headspace: hd!(4,-2,5), score_event: None, stamina_cost: 2 },
            failure: Outcome { text: "The pass is cut out at the edge of the box.", next: None, output_delta: -2, headspace: hd!(-1,2,-1), score_event: None, stamina_cost: 1 },
        },
    ],
},

// 3 ── THROUGH_BALL ───────────────────────────────────────────────────────────
Beat {
    id: B_THROUGH_BALL,
    phase: MatchPhase::OpenPlayAttack,
    phase_bias: [2, 3, 3],
    setup: "A runner is making a diagonal run in behind. The defensive line is high — a gap is there.",
    choices: &[
        BeatChoice {
            text: "Thread the through ball",
            primary: Vision,
            difficulty: 65,
            success: Outcome { text: "Inch-perfect. Your teammate is clean through.", next: Some(B_ONE_ON_ONE), output_delta: 12, headspace: hd!(7,-2,7), score_event: None, stamina_cost: 2 },
            failure: Outcome { text: "Slightly overhit — the keeper claims it.", next: None, output_delta: -4, headspace: hd!(-3,5,-3), score_event: None, stamina_cost: 1 },
        },
        BeatChoice {
            text: "Hold it and play it safe",
            primary: ShortPassing,
            difficulty: 40,
            success: Outcome { text: "You keep it and the team resets the attack.", next: None, output_delta: 1, headspace: hd!(0,0,0), score_event: None, stamina_cost: 1 },
            failure: Outcome { text: "You hold on too long and lose it under pressure.", next: None, output_delta: -4, headspace: hd!(-2,4,-2), score_event: None, stamina_cost: 2 },
        },
    ],
},

// 4 ── SHOT_SETUP ─────────────────────────────────────────────────────────────
Beat {
    id: B_SHOT_SETUP,
    phase: MatchPhase::OpenPlayAttack,
    phase_bias: [2, 3, 4],
    setup: "You've created a yard of space outside the box. The keeper is set — shoot or hold?",
    choices: &[
        BeatChoice {
            text: "Strike for goal",
            primary: LongShots,
            difficulty: 62,
            success: Outcome { text: "A rasping drive clips the inside of the post — GOAL!", next: None, output_delta: 18, headspace: hd!(10,-5,10), score_event: Some(ScoreEvent::GoalFor), stamina_cost: 4 },
            failure: Outcome { text: "Your shot blazes over the bar.", next: None, output_delta: -4, headspace: hd!(-3,6,-3), score_event: None, stamina_cost: 3 },
        },
        BeatChoice {
            text: "Ride it — wait for the right moment",
            primary: Vision,
            difficulty: 48,
            success: Outcome { text: "You spot a runner and pick him out perfectly.", next: Some(B_BOX_ENTRY), output_delta: 8, headspace: hd!(4,-1,4), score_event: None, stamina_cost: 2 },
            failure: Outcome { text: "You dwell too long and the chance is gone.", next: None, output_delta: -3, headspace: hd!(-2,4,-2), score_event: None, stamina_cost: 1 },
        },
    ],
},

// 5 ── BOX_ENTRY ──────────────────────────────────────────────────────────────
Beat {
    id: B_BOX_ENTRY,
    phase: MatchPhase::OpenPlayAttack,
    phase_bias: [2, 3, 4],
    setup: "The ball has broken inside the box. Bodies everywhere — a half-chance has opened up.",
    choices: &[
        BeatChoice {
            text: "Shoot first time",
            primary: Finishing,
            difficulty: 60,
            success: Outcome { text: "You take it on the half-volley — straight into the corner. GOAL!", next: None, output_delta: 20, headspace: hd!(10,-5,10), score_event: Some(ScoreEvent::GoalFor), stamina_cost: 4 },
            failure: Outcome { text: "Your shot is blocked — deflected wide.", next: Some(B_CORNER_ATTACK), output_delta: -2, headspace: hd!(-2,3,-2), score_event: None, stamina_cost: 3 },
        },
        BeatChoice {
            text: "Take a touch, set yourself",
            primary: BallControl,
            difficulty: 52,
            success: Outcome { text: "Composure — you control it and finish calmly. GOAL!", next: None, output_delta: 18, headspace: hd!(10,-5,10), score_event: Some(ScoreEvent::GoalFor), stamina_cost: 3 },
            failure: Outcome { text: "A touch too many — the chance closes down.", next: None, output_delta: -5, headspace: hd!(-4,7,-4), score_event: None, stamina_cost: 2 },
        },
        BeatChoice {
            text: "Head it away at the back post",
            primary: Heading,
            difficulty: 58,
            success: Outcome { text: "Glancing header at the back post — it nestles in! GOAL!", next: None, output_delta: 18, headspace: hd!(10,-5,10), score_event: Some(ScoreEvent::GoalFor), stamina_cost: 4 },
            failure: Outcome { text: "Your header goes wide of the post.", next: None, output_delta: -3, headspace: hd!(-2,4,-2), score_event: None, stamina_cost: 3 },
        },
    ],
},

// 6 ── DEFEND_TRACK ───────────────────────────────────────────────────────────
Beat {
    id: B_DEFEND_TRACK,
    phase: MatchPhase::OpenPlayDefend,
    phase_bias: [3, 3, 2],
    setup: "Their winger has the ball and is running at pace down your side. You need to deal with this.",
    choices: &[
        BeatChoice {
            text: "Drop back and jockey — don't commit",
            primary: Curve,
            difficulty: 52,
            success: Outcome { text: "You stay patient and force him backwards.", next: None, output_delta: 5, headspace: hd!(3,-1,2), score_event: None, stamina_cost: 4 },
            failure: Outcome { text: "He finds a way past you down the line.", next: Some(B_DEFEND_AERIAL), output_delta: -5, headspace: hd!(-3,6,-3), score_event: None, stamina_cost: 5 },
        },
        BeatChoice {
            text: "Go in hard and win the ball",
            primary: StandingTackle,
            difficulty: 60,
            success: Outcome { text: "Perfectly timed — you take ball and man cleanly.", next: None, output_delta: 8, headspace: hd!(5,-2,5), score_event: None, stamina_cost: 5 },
            failure: Outcome { text: "He sidesteps and you're caught out of position.", next: None, output_delta: -6, headspace: hd!(-4,7,-4), score_event: Some(ScoreEvent::GoalAgainst), stamina_cost: 4 },
        },
    ],
},

// 7 ── DEFEND_TACKLE ──────────────────────────────────────────────────────────
Beat {
    id: B_DEFEND_TACKLE,
    phase: MatchPhase::OpenPlayDefend,
    phase_bias: [2, 3, 2],
    setup: "Their striker receives the ball with his back to goal right in front of you. This is yours.",
    choices: &[
        BeatChoice {
            text: "Slide in for the tackle",
            primary: StandingTackle,
            difficulty: 55,
            success: Outcome { text: "Perfectly timed slide tackle — ball cleared.", next: None, output_delta: 8, headspace: hd!(5,-2,5), score_event: None, stamina_cost: 5 },
            failure: Outcome { text: "He rode the challenge — penalty could be given.", next: None, output_delta: -8, headspace: hd!(-5,8,-5), score_event: None, stamina_cost: 4 },
        },
        BeatChoice {
            text: "Hold your position — mark tight",
            primary: Marking,
            difficulty: 48,
            success: Outcome { text: "You don't give him an inch — he turns the ball over.", next: None, output_delta: 5, headspace: hd!(3,-1,2), score_event: None, stamina_cost: 3 },
            failure: Outcome { text: "He spins sharply and creates space — you're beaten.", next: Some(B_DEFEND_TRACK), output_delta: -4, headspace: hd!(-3,5,-3), score_event: None, stamina_cost: 3 },
        },
    ],
},

// 8 ── DEFEND_AERIAL ──────────────────────────────────────────────────────────
Beat {
    id: B_DEFEND_AERIAL,
    phase: MatchPhase::OpenPlayDefend,
    phase_bias: [2, 2, 3],
    setup: "A cross is swinging in. Their striker is moving to the front post — you must win this.",
    choices: &[
        BeatChoice {
            text: "Time your jump — attack the ball",
            primary: Heading,
            difficulty: 58,
            success: Outcome { text: "Dominant header — you clear it cleanly.", next: None, output_delta: 7, headspace: hd!(4,-2,4), score_event: None, stamina_cost: 5 },
            failure: Outcome { text: "He gets above you — powerful header on goal!", next: None, output_delta: -7, headspace: hd!(-4,7,-4), score_event: Some(ScoreEvent::GoalAgainst), stamina_cost: 4 },
        },
        BeatChoice {
            text: "Hold your ground — block the run",
            primary: Strength,
            difficulty: 50,
            success: Outcome { text: "You use your body well — he can't get a clean run.", next: None, output_delta: 5, headspace: hd!(3,-1,2), score_event: None, stamina_cost: 4 },
            failure: Outcome { text: "He muscles past — glancing header, keeper scrambles.", next: None, output_delta: -4, headspace: hd!(-3,5,-3), score_event: None, stamina_cost: 3 },
        },
    ],
},

// 9 ── DEFEND_INTERCEPT ───────────────────────────────────────────────────────
Beat {
    id: B_DEFEND_INTERCEPT,
    phase: MatchPhase::OpenPlayDefend,
    phase_bias: [3, 3, 2],
    setup: "You read a pass in the channel — step up and intercept, or hold your shape?",
    choices: &[
        BeatChoice {
            text: "Step up and cut it out",
            primary: Interceptions,
            difficulty: 58,
            success: Outcome { text: "You read it perfectly — interception, counter starts.", next: Some(B_THROUGH_BALL), output_delta: 10, headspace: hd!(6,-2,6), score_event: None, stamina_cost: 4 },
            failure: Outcome { text: "Wrong read — you've played them onside. They're through.", next: Some(B_ONE_ON_ONE), output_delta: -8, headspace: hd!(-5,8,-5), score_event: None, stamina_cost: 4 },
        },
        BeatChoice {
            text: "Stay deep — hold the shape",
            primary: Curve,
            difficulty: 42,
            success: Outcome { text: "You hold your line. The ball goes back harmlessly.", next: None, output_delta: 3, headspace: hd!(1,0,1), score_event: None, stamina_cost: 2 },
            failure: Outcome { text: "Hesitation — the pass goes through your legs.", next: None, output_delta: -5, headspace: hd!(-3,5,-3), score_event: None, stamina_cost: 2 },
        },
    ],
},

// 10 ── ONE_ON_ONE ─────────────────────────────────────────────────────────────
Beat {
    id: B_ONE_ON_ONE,
    phase: MatchPhase::OneOnOne,
    phase_bias: [2, 3, 4],
    setup: "You're clean through on goal! Just the keeper. Ice cold or adrenaline?",
    choices: &[
        BeatChoice {
            text: "Chip the keeper",
            primary: Finishing,
            difficulty: 70,
            success: Outcome { text: "Outrageous chip — floats over the keeper and in. GOAL!", next: None, output_delta: 22, headspace: hd!(12,-5,12), score_event: Some(ScoreEvent::GoalFor), stamina_cost: 3 },
            failure: Outcome { text: "The keeper reads it — catches it comfortably.", next: None, output_delta: -6, headspace: hd!(-5,9,-5), score_event: None, stamina_cost: 2 },
        },
        BeatChoice {
            text: "Slot it low and hard to the corner",
            primary: Finishing,
            difficulty: 58,
            success: Outcome { text: "Precise. Low, far corner — GOAL!", next: None, output_delta: 20, headspace: hd!(10,-5,10), score_event: Some(ScoreEvent::GoalFor), stamina_cost: 3 },
            failure: Outcome { text: "Hit it too close to the keeper — he saves!", next: None, output_delta: -7, headspace: hd!(-5,10,-5), score_event: None, stamina_cost: 2 },
        },
        BeatChoice {
            text: "Drive it across goal",
            primary: ShotPower,
            difficulty: 52,
            success: Outcome { text: "Power across the keeper — smacks the net. GOAL!", next: None, output_delta: 20, headspace: hd!(10,-5,10), score_event: Some(ScoreEvent::GoalFor), stamina_cost: 4 },
            failure: Outcome { text: "You overhit it — goes across the face of goal and wide.", next: None, output_delta: -6, headspace: hd!(-4,8,-4), score_event: None, stamina_cost: 3 },
        },
    ],
},

// 11 ── ONE_ON_ONE_WIDE ────────────────────────────────────────────────────────
Beat {
    id: B_ONE_ON_ONE_WIDE,
    phase: MatchPhase::OneOnOne,
    phase_bias: [2, 3, 3],
    setup: "You've burned the full back and you're bearing down on goal from a wide angle.",
    choices: &[
        BeatChoice {
            text: "Cut inside and shoot",
            primary: Agility,
            difficulty: 62,
            success: Outcome { text: "Quick cut, opens up the angle — rifles it into the far post. GOAL!", next: None, output_delta: 20, headspace: hd!(10,-5,10), score_event: Some(ScoreEvent::GoalFor), stamina_cost: 5 },
            failure: Outcome { text: "He recovers and gets a toe in.", next: None, output_delta: -4, headspace: hd!(-3,5,-3), score_event: None, stamina_cost: 4 },
        },
        BeatChoice {
            text: "Pull it back for a tap-in",
            primary: Crossing,
            difficulty: 52,
            success: Outcome { text: "Perfect cutback — someone's on the end of it. GOAL!", next: None, output_delta: 15, headspace: hd!(8,-3,8), score_event: Some(ScoreEvent::GoalFor), stamina_cost: 3 },
            failure: Outcome { text: "Your pull-back is cut out — they clear.", next: None, output_delta: -2, headspace: hd!(-1,2,-1), score_event: None, stamina_cost: 2 },
        },
    ],
},

// 12 ── FREE_KICK_DIRECT ───────────────────────────────────────────────────────
Beat {
    id: B_FREE_KICK_DIRECT,
    phase: MatchPhase::SetPiece,
    phase_bias: [2, 2, 3],
    setup: "Free kick. 25 yards out, central position. The wall is set. This is a real chance.",
    choices: &[
        BeatChoice {
            text: "Curl it over the wall",
            primary: FreeKickAcc,
            difficulty: 65,
            success: Outcome { text: "A perfect curl — dips under the bar. GOAL!", next: None, output_delta: 22, headspace: hd!(10,-5,12), score_event: Some(ScoreEvent::GoalFor), stamina_cost: 3 },
            failure: Outcome { text: "It hits the wall — crosses straight to the goalkeeper.", next: None, output_delta: -4, headspace: hd!(-3,6,-3), score_event: None, stamina_cost: 2 },
        },
        BeatChoice {
            text: "Drive it through the gap in the wall",
            primary: ShotPower,
            difficulty: 62,
            success: Outcome { text: "It goes through the gap and bulges the net — GOAL!", next: None, output_delta: 20, headspace: hd!(10,-5,10), score_event: Some(ScoreEvent::GoalFor), stamina_cost: 3 },
            failure: Outcome { text: "The keeper smothers it — comfortable for him.", next: None, output_delta: -3, headspace: hd!(-2,4,-2), score_event: None, stamina_cost: 2 },
        },
        BeatChoice {
            text: "Chip it to the back post for a header",
            primary: LongPassing,
            difficulty: 52,
            success: Outcome { text: "Floated to the back post — headed in! GOAL!", next: None, output_delta: 15, headspace: hd!(7,-3,7), score_event: Some(ScoreEvent::GoalFor), stamina_cost: 2 },
            failure: Outcome { text: "Nobody connects — keeper takes it.", next: None, output_delta: -2, headspace: hd!(-1,2,-1), score_event: None, stamina_cost: 1 },
        },
    ],
},

// 13 ── FREE_KICK_CROSS ────────────────────────────────────────────────────────
Beat {
    id: B_FREE_KICK_CROSS,
    phase: MatchPhase::SetPiece,
    phase_bias: [2, 3, 2],
    setup: "Free kick wide of the box. Your delivery from set pieces is in the spotlight.",
    choices: &[
        BeatChoice {
            text: "Curl it into the near post area",
            primary: Crossing,
            difficulty: 55,
            success: Outcome { text: "Whipped delivery — flicked on at the near post.", next: Some(B_BOX_ENTRY), output_delta: 8, headspace: hd!(4,-1,4), score_event: None, stamina_cost: 2 },
            failure: Outcome { text: "Overhit — straight to the keeper.", next: None, output_delta: -3, headspace: hd!(-2,3,-2), score_event: None, stamina_cost: 2 },
        },
        BeatChoice {
            text: "Float to the back post",
            primary: FreeKickAcc,
            difficulty: 50,
            success: Outcome { text: "Perfect flight — teammate gets a clean header on target.", next: Some(B_CORNER_ATTACK), output_delta: 8, headspace: hd!(4,-1,4), score_event: None, stamina_cost: 2 },
            failure: Outcome { text: "Ball drifts out for a goal kick.", next: None, output_delta: -2, headspace: hd!(-1,2,-1), score_event: None, stamina_cost: 1 },
        },
    ],
},

// 14 ── CORNER_ATTACK ──────────────────────────────────────────────────────────
Beat {
    id: B_CORNER_ATTACK,
    phase: MatchPhase::SetPiece,
    phase_bias: [2, 3, 3],
    setup: "Corner kick awarded. You're making your run. Make it count.",
    choices: &[
        BeatChoice {
            text: "Attack the near post — flick it on",
            primary: Heading,
            difficulty: 58,
            success: Outcome { text: "Powerful near-post header — GOAL!", next: None, output_delta: 20, headspace: hd!(10,-5,10), score_event: Some(ScoreEvent::GoalFor), stamina_cost: 5 },
            failure: Outcome { text: "Your header goes straight at the keeper.", next: None, output_delta: -2, headspace: hd!(-1,3,-1), score_event: None, stamina_cost: 3 },
        },
        BeatChoice {
            text: "Peel to the edge for a volley",
            primary: Volleys,
            difficulty: 68,
            success: Outcome { text: "Sweet half-volley — rockets into the top corner. GOAL!", next: None, output_delta: 22, headspace: hd!(12,-5,12), score_event: Some(ScoreEvent::GoalFor), stamina_cost: 4 },
            failure: Outcome { text: "Your volley goes skyward into the stands.", next: None, output_delta: -3, headspace: hd!(-2,5,-2), score_event: None, stamina_cost: 3 },
        },
        BeatChoice {
            text: "Stay back — control the second ball",
            primary: AttPositioning,
            difficulty: 42,
            success: Outcome { text: "The ball breaks to you — you keep it alive.", next: Some(B_SHOT_SETUP), output_delta: 4, headspace: hd!(2,-1,2), score_event: None, stamina_cost: 2 },
            failure: Outcome { text: "The clearance flies over your head.", next: None, output_delta: -1, headspace: hd!(0,1,0), score_event: None, stamina_cost: 1 },
        },
    ],
},

// 15 ── CORNER_DEFEND ──────────────────────────────────────────────────────────
Beat {
    id: B_CORNER_DEFEND,
    phase: MatchPhase::SetPiece,
    phase_bias: [2, 2, 3],
    setup: "They've won a corner. You've got a big striker to mark. This is a key defensive moment.",
    choices: &[
        BeatChoice {
            text: "Win the ball in the air",
            primary: Heading,
            difficulty: 58,
            success: Outcome { text: "Authoritative header — you punch it clear.", next: None, output_delta: 8, headspace: hd!(5,-2,5), score_event: None, stamina_cost: 5 },
            failure: Outcome { text: "He gets above you — header rattles the crossbar.", next: None, output_delta: -6, headspace: hd!(-4,7,-4), score_event: None, stamina_cost: 4 },
        },
        BeatChoice {
            text: "Mark your man out of the game",
            primary: Marking,
            difficulty: 52,
            success: Outcome { text: "You hold your man. The delivery goes behind.", next: None, output_delta: 5, headspace: hd!(3,-1,2), score_event: None, stamina_cost: 3 },
            failure: Outcome { text: "He slips your grasp and gets a clean run. They score.", next: None, output_delta: -8, headspace: hd!(-5,8,-5), score_event: Some(ScoreEvent::GoalAgainst), stamina_cost: 3 },
        },
    ],
},

// 16 ── PENALTY ────────────────────────────────────────────────────────────────
Beat {
    id: B_PENALTY,
    phase: MatchPhase::SetPiece,
    phase_bias: [0, 1, 3],
    setup: "PENALTY! You step up. Stadium on its feet. Just you, the ball, and the keeper.",
    choices: &[
        BeatChoice {
            text: "Stutter run — wait for the keeper to dive",
            primary: Composure,
            difficulty: 62,
            success: Outcome { text: "Perfectly placed as the keeper dives the wrong way. GOAL!", next: None, output_delta: 22, headspace: hd!(12,-5,12), score_event: Some(ScoreEvent::GoalFor), stamina_cost: 2 },
            failure: Outcome { text: "He reads you and saves! A moment of agony.", next: None, output_delta: -12, headspace: hd!(-8,15,-8), score_event: None, stamina_cost: 1 },
        },
        BeatChoice {
            text: "Drive it hard and low to the corner",
            primary: Penalties,
            difficulty: 55,
            success: Outcome { text: "Power and placement — the keeper had no chance. GOAL!", next: None, output_delta: 20, headspace: hd!(10,-5,10), score_event: Some(ScoreEvent::GoalFor), stamina_cost: 2 },
            failure: Outcome { text: "It clips the post and stays out. Agonising.", next: None, output_delta: -10, headspace: hd!(-7,12,-7), score_event: None, stamina_cost: 1 },
        },
        BeatChoice {
            text: "Chip it down the middle",
            primary: Finishing,
            difficulty: 70,
            success: Outcome { text: "Cheeky Panenka — rolls in as the keeper dives. GOAL!", next: None, output_delta: 25, headspace: hd!(15,-5,15), score_event: Some(ScoreEvent::GoalFor), stamina_cost: 1 },
            failure: Outcome { text: "The keeper stays put and takes it on the chest.", next: None, output_delta: -12, headspace: hd!(-8,15,-8), score_event: None, stamina_cost: 1 },
        },
    ],
},

// 17 ── FIND_SPACE ─────────────────────────────────────────────────────────────
Beat {
    id: B_FIND_SPACE,
    phase: MatchPhase::Positioning,
    phase_bias: [3, 3, 2],
    setup: "Your team has the ball. You need to find space to receive. Read the defensive shape.",
    choices: &[
        BeatChoice {
            text: "Make an angled run to break the line",
            primary: AttPositioning,
            difficulty: 52,
            success: Outcome { text: "You time the run perfectly — ball played in. In on goal!", next: Some(B_ONE_ON_ONE), output_delta: 10, headspace: hd!(5,-2,5), score_event: None, stamina_cost: 6 },
            failure: Outcome { text: "The run is spotted — you're offside. Flag's up.", next: None, output_delta: -3, headspace: hd!(-2,4,-2), score_event: None, stamina_cost: 4 },
        },
        BeatChoice {
            text: "Drop into the pocket between lines",
            primary: Vision,
            difficulty: 45,
            success: Outcome { text: "You find the half-space. Ball in, you turn and drive.", next: Some(B_DRIBBLE_CHALLENGE), output_delta: 6, headspace: hd!(3,-1,3), score_event: None, stamina_cost: 3 },
            failure: Outcome { text: "You're tracked too closely — no room to turn.", next: None, output_delta: -1, headspace: hd!(0,1,0), score_event: None, stamina_cost: 2 },
        },
    ],
},

// 18 ── PRESS_TRIGGER ──────────────────────────────────────────────────────────
Beat {
    id: B_PRESS_TRIGGER,
    phase: MatchPhase::Positioning,
    phase_bias: [3, 3, 2],
    setup: "Their centre-back has the ball — heavy touch. The press trigger has been hit.",
    choices: &[
        BeatChoice {
            text: "Sprint and press immediately",
            primary: Bravery,
            difficulty: 55,
            success: Outcome { text: "You close him down — panicked clearance, you've won it.", next: Some(B_BOX_ENTRY), output_delta: 8, headspace: hd!(4,-2,4), score_event: None, stamina_cost: 7 },
            failure: Outcome { text: "He steadies and plays through your press.", next: None, output_delta: -2, headspace: hd!(-1,2,-1), score_event: None, stamina_cost: 5 },
        },
        BeatChoice {
            text: "Contain him — cut off the option",
            primary: Curve,
            difficulty: 45,
            success: Outcome { text: "Good shape — you block his lane and he's forced backwards.", next: None, output_delta: 3, headspace: hd!(1,0,1), score_event: None, stamina_cost: 3 },
            failure: Outcome { text: "He finds his man in behind your press.", next: None, output_delta: -3, headspace: hd!(-2,3,-2), score_event: None, stamina_cost: 3 },
        },
    ],
},

// 19 ── SUPPORT_RUN ────────────────────────────────────────────────────────────
Beat {
    id: B_SUPPORT_RUN,
    phase: MatchPhase::Positioning,
    phase_bias: [3, 3, 2],
    setup: "Your teammate is driving forward. Provide the right option — make the run.",
    choices: &[
        BeatChoice {
            text: "Overlap — run beyond him",
            primary: Acceleration,
            difficulty: 52,
            success: Outcome { text: "Perfect overlap. Ball played in, you're one-on-one wide.", next: Some(B_ONE_ON_ONE_WIDE), output_delta: 7, headspace: hd!(4,-1,4), score_event: None, stamina_cost: 7 },
            failure: Outcome { text: "You're flagged offside — no, you aren't — but the moment's gone.", next: None, output_delta: -2, headspace: hd!(-1,2,-1), score_event: None, stamina_cost: 5 },
        },
        BeatChoice {
            text: "Underlap — cut inside to receive",
            primary: Reactions,
            difficulty: 48,
            success: Outcome { text: "You curl infield — ball finds you in the channel.", next: Some(B_SHOT_SETUP), output_delta: 6, headspace: hd!(3,-1,3), score_event: None, stamina_cost: 4 },
            failure: Outcome { text: "He doesn't see you — shoots instead. Blocked.", next: None, output_delta: -1, headspace: hd!(0,1,0), score_event: None, stamina_cost: 3 },
        },
    ],
},

// 20 ── HOLD_SHAPE ─────────────────────────────────────────────────────────────
Beat {
    id: B_HOLD_SHAPE,
    phase: MatchPhase::Positioning,
    phase_bias: [3, 3, 2],
    setup: "They're transitioning quickly. Hold the defensive line or step up?",
    choices: &[
        BeatChoice {
            text: "Maintain the defensive line",
            primary: SlidingTackle,
            difficulty: 45,
            success: Outcome { text: "Good discipline — they're caught offside. Free kick.", next: None, output_delta: 5, headspace: hd!(3,-1,2), score_event: None, stamina_cost: 2 },
            failure: Outcome { text: "You drop too deep — they play someone in behind.", next: Some(B_DEFEND_TRACK), output_delta: -4, headspace: hd!(-3,5,-3), score_event: None, stamina_cost: 2 },
        },
        BeatChoice {
            text: "Step out — look to win the second ball",
            primary: Aggression,
            difficulty: 52,
            success: Outcome { text: "You intercept the second ball brilliantly.", next: Some(B_THROUGH_BALL), output_delta: 8, headspace: hd!(5,-2,5), score_event: None, stamina_cost: 4 },
            failure: Outcome { text: "It breaks behind you — they're in.", next: Some(B_DEFEND_TACKLE), output_delta: -5, headspace: hd!(-3,6,-3), score_event: None, stamina_cost: 3 },
        },
    ],
},

// 21 ── LAST_MINUTE_CHANCE (mini-tree root) ────────────────────────────────────
Beat {
    id: B_LAST_MINUTE_CHANCE,
    phase: MatchPhase::KeyMoment,
    phase_bias: [0, 0, 5],
    setup: "90+2. Your team needs a goal. A long ball breaks to your chest — could be your moment.",
    choices: &[
        BeatChoice {
            text: "Control and turn — go for glory",
            primary: BallControl,
            difficulty: 55,
            success: Outcome { text: "You kill it dead, spin your marker. You're through!", next: Some(B_LAST_MINUTE_FINISH), output_delta: 8, headspace: hd!(8,-3,8), score_event: None, stamina_cost: 5 },
            failure: Outcome { text: "First touch lets you down — defender mops it up. Game over.", next: None, output_delta: -8, headspace: hd!(-8,12,-8), score_event: None, stamina_cost: 3 },
        },
        BeatChoice {
            text: "Flick on first time — hope someone's there",
            primary: Reactions,
            difficulty: 65,
            success: Outcome { text: "Brilliant flick — a teammate is in! Now it's up to him.", next: Some(B_LAST_MINUTE_FINISH), output_delta: 10, headspace: hd!(8,-3,8), score_event: None, stamina_cost: 3 },
            failure: Outcome { text: "The flick goes wide — wasted opportunity.", next: None, output_delta: -6, headspace: hd!(-6,10,-6), score_event: None, stamina_cost: 2 },
        },
    ],
},

// 22 ── LAST_MINUTE_FINISH (mini-tree leaf) ────────────────────────────────────
Beat {
    id: B_LAST_MINUTE_FINISH,
    phase: MatchPhase::KeyMoment,
    phase_bias: [0, 0, 5],
    setup: "Keeper's coming out! The goal is open. One chance to win the game. Composure.",
    choices: &[
        BeatChoice {
            text: "Place it wide of the keeper",
            primary: Composure,
            difficulty: 62,
            success: Outcome { text: "Nerves of steel. Composed finish — GOAL! Wild scenes!", next: None, output_delta: 28, headspace: hd!(15,-5,15), score_event: Some(ScoreEvent::GoalFor), stamina_cost: 2 },
            failure: Outcome { text: "Under the pressure, you scuff it. The keeper gets down to save. Agony.", next: None, output_delta: -12, headspace: hd!(-10,15,-10), score_event: None, stamina_cost: 2 },
        },
        BeatChoice {
            text: "Lash it — maximum power",
            primary: ShotPower,
            difficulty: 55,
            success: Outcome { text: "Hit it before you think! Thunderous. Net ripping. GOAL!", next: None, output_delta: 25, headspace: hd!(13,-5,13), score_event: Some(ScoreEvent::GoalFor), stamina_cost: 3 },
            failure: Outcome { text: "Ballooned over. That's the last kick. Heartbreak.", next: None, output_delta: -10, headspace: hd!(-8,12,-8), score_event: None, stamina_cost: 3 },
        },
    ],
},

// 23 ── CRUCIAL_TACKLE ─────────────────────────────────────────────────────────
Beat {
    id: B_CRUCIAL_TACKLE,
    phase: MatchPhase::KeyMoment,
    phase_bias: [1, 2, 4],
    setup: "They've broken on the counter and your striker is the last defender. This is a match-defining moment.",
    choices: &[
        BeatChoice {
            text: "Slide in — last-ditch tackle",
            primary: StandingTackle,
            difficulty: 65,
            success: Outcome { text: "Perfectly timed — you take the ball cleanly. Magnificent.", next: None, output_delta: 14, headspace: hd!(9,-3,9), score_event: None, stamina_cost: 6 },
            failure: Outcome { text: "He dances past — you're left on the turf. They score.", next: None, output_delta: -10, headspace: hd!(-7,12,-7), score_event: Some(ScoreEvent::GoalAgainst), stamina_cost: 5 },
        },
        BeatChoice {
            text: "Contain — buy time for cover",
            primary: Curve,
            difficulty: 55,
            success: Outcome { text: "Brilliant delaying tactic — cover arrives, danger averted.", next: None, output_delta: 8, headspace: hd!(5,-2,5), score_event: None, stamina_cost: 4 },
            failure: Outcome { text: "He's too quick — shoots before cover gets there.", next: None, output_delta: -7, headspace: hd!(-4,8,-4), score_event: Some(ScoreEvent::GoalAgainst), stamina_cost: 3 },
        },
    ],
},

// 24 ── COMEBACK_MOMENT ────────────────────────────────────────────────────────
Beat {
    id: B_COMEBACK_MOMENT,
    phase: MatchPhase::KeyMoment,
    phase_bias: [1, 3, 4],
    setup: "You've won a free kick in a promising position. The wall is set. Can you make something of it?",
    choices: &[
        BeatChoice {
            text: "Step up and hit it yourself",
            primary: FreeKickAcc,
            difficulty: 68,
            success: Outcome { text: "Curled into the top corner — the comeback starts here! GOAL!", next: None, output_delta: 24, headspace: hd!(12,-5,12), score_event: Some(ScoreEvent::GoalFor), stamina_cost: 3 },
            failure: Outcome { text: "Goes into the wall. Their fans jeer.", next: None, output_delta: -4, headspace: hd!(-4,7,-4), score_event: None, stamina_cost: 2 },
        },
        BeatChoice {
            text: "Dummy run — let someone else take",
            primary: Vision,
            difficulty: 48,
            success: Outcome { text: "The dummy worked — your teammate curls it in. 2-1!", next: None, output_delta: 12, headspace: hd!(7,-3,7), score_event: Some(ScoreEvent::GoalFor), stamina_cost: 2 },
            failure: Outcome { text: "Confusion in the wall. The moment is wasted.", next: None, output_delta: -3, headspace: hd!(-2,4,-2), score_event: None, stamina_cost: 1 },
        },
    ],
},

// 25 ── WONDER_STRIKE ──────────────────────────────────────────────────────────
Beat {
    id: B_WONDER_STRIKE,
    phase: MatchPhase::KeyMoment,
    phase_bias: [1, 2, 3],
    setup: "The ball drops to you 30 yards out, perfectly cushioned. The keeper is slightly off his line. Do you dare?",
    choices: &[
        BeatChoice {
            text: "Audacious lob — first instinct",
            primary: LongShots,
            difficulty: 78,
            success: Outcome { text: "It arcs perfectly, dips under the bar. The crowd is silent... then erupts. GOAL!", next: None, output_delta: 28, headspace: hd!(15,-5,15), score_event: Some(ScoreEvent::GoalFor), stamina_cost: 3 },
            failure: Outcome { text: "It clears the bar by inches. So close.", next: None, output_delta: -4, headspace: hd!(-2,4,-2), score_event: None, stamina_cost: 2 },
        },
        BeatChoice {
            text: "Control first — wait for the right pass",
            primary: Composure,
            difficulty: 42,
            success: Outcome { text: "Patient play — you find a teammate in a great position.", next: Some(B_SHOT_SETUP), output_delta: 6, headspace: hd!(3,-1,3), score_event: None, stamina_cost: 2 },
            failure: Outcome { text: "Caught in two minds — the ball is taken off you.", next: None, output_delta: -4, headspace: hd!(-3,5,-3), score_event: None, stamina_cost: 2 },
        },
    ],
},

// 26 ── RECKLESS_CHALLENGE (frustration-injected; phase_bias all-zero = never normal-selected) ──
Beat {
    id: B_RECKLESS_CHALLENGE,
    phase: MatchPhase::OpenPlayDefend,
    phase_bias: [0, 0, 0],
    setup: "The red mist is descending. You've taken one too many knocks and your blood is up. A loose ball — your chance to make a statement.",
    choices: &[
        BeatChoice {
            text: "Fly in — full commitment, worry about the card later",
            primary: StandingTackle,
            difficulty: 50,
            success: Outcome { text: "You get the ball — but the ref is already reaching into his pocket.", next: None, output_delta: 5, headspace: hd!(-2,-15,3), score_event: None, stamina_cost: 8 },
            failure: Outcome { text: "You miss the ball completely — late, ugly. The ref has no choice.", next: None, output_delta: -8, headspace: hd!(-5,-10,-5), score_event: None, stamina_cost: 8 },
        },
        BeatChoice {
            text: "Late sliding challenge — just a fraction off",
            primary: Aggression,
            difficulty: 45,
            success: Outcome { text: "You get there — borderline, but the ref waves play on.", next: None, output_delta: 3, headspace: hd!(2,-8,2), score_event: None, stamina_cost: 6 },
            failure: Outcome { text: "You clip his heels. The whistle goes immediately.", next: None, output_delta: -6, headspace: hd!(-4,-8,-4), score_event: None, stamina_cost: 6 },
        },
        BeatChoice {
            text: "Pull out — swallow it, stay on the pitch",
            primary: Composure,
            difficulty: 50,
            success: Outcome { text: "You bite your lip and hold back. Smart. The moment passes.", next: None, output_delta: 2, headspace: hd!(3,-20,2), score_event: None, stamina_cost: 2 },
            failure: Outcome { text: "You hesitate and they dance past — but you keep your composure.", next: None, output_delta: -2, headspace: hd!(-1,-15,-1), score_event: None, stamina_cost: 2 },
        },
    ],
},

]; // end BEATS
