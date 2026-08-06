//! Design round 4, Slice 5 §5.3 — congestion-score sanity check across the full
//! competition set.
//!
//! `congestion_score` (a 10-day-window fixture count, `engine.rs`) and
//! `SOFT_FLUSH_THRESHOLD` (the soft-flashpoint-batching threshold, `tuning.rs`) were
//! both written and tuned back when the only orbit competition was the league. Now
//! that a PC's club can genuinely be in League + Domestic Cup + a continental tier
//! simultaneously, with the PC personally possibly holding a national-team call-up on
//! top, this is the point where fixture density is highest anywhere in the game.
//!
//! This is a MECHANICAL sanity check only (the count scales correctly, the constant's
//! current behavior under that density is visible) — not a feel/tuning verdict. Per
//! the doc: "Not a new number to invent here" — a real playtesting pass belongs to a
//! later round; this just proves the existing machinery doesn't silently break or cap
//! under the new, denser fixture load.

use std::cell::RefCell;
use std::rc::Rc;

use goat_calendar::{
    should_flush_soft, CalendarEngine, Competition, CompetitionKind, DayContext, DayReport,
    Fixture, FixtureImportance, GameClock, Season, StopClass, Subsystem, SubsystemId,
};
use goat_rng::RngSource;

const LEAGUE: u32 = 1;
const DOMESTIC_CUP: u32 = 2;
const CONTINENTAL_TIER1: u32 = 3;
const WORLD_CUP: u32 = 4;

fn season() -> Season {
    Season {
        id: 1,
        start_day: 0,
        end_day: 364,
        windows: vec![],
        competition_ids: vec![LEAGUE, DOMESTIC_CUP, CONTINENTAL_TIER1, WORLD_CUP],
    }
}

fn competitions() -> Vec<Competition> {
    vec![
        Competition {
            id: LEAGUE,
            kind: CompetitionKind::League,
            priority: 100,
            is_orbit: true,
        },
        Competition {
            id: DOMESTIC_CUP,
            kind: CompetitionKind::DomesticCup,
            priority: 50,
            is_orbit: true,
        },
        Competition {
            id: CONTINENTAL_TIER1,
            kind: CompetitionKind::ContinentalTier1,
            priority: 70,
            is_orbit: true,
        },
        Competition {
            id: WORLD_CUP,
            kind: CompetitionKind::WorldCup,
            priority: 90,
            is_orbit: true,
        },
    ]
}

fn fixture(id: u64, competition_id: u32, day: u32, importance: FixtureImportance) -> Fixture {
    Fixture {
        id,
        competition_id,
        scheduled_day: day,
        original_day: day,
        is_orbit: true,
        importance,
        leg_for_id: None,
    }
}

/// Every orbit competition in the game landing inside the same 10-day window — the
/// densest realistic load (League + Domestic Cup + one continental tier + a
/// national-team fixture), spread 2 days apart so no two clash on the same day.
fn max_density_fixtures() -> Vec<Fixture> {
    vec![
        fixture(1, LEAGUE, 0, FixtureImportance::League),
        fixture(2, DOMESTIC_CUP, 2, FixtureImportance::DomesticCupLate),
        fixture(3, CONTINENTAL_TIER1, 4, FixtureImportance::ContinentalTier1),
        // National-team fixtures carry no FixtureImportance ladder entry (deliberately
        // excluded — an international window excludes club fixtures rather than
        // winning a same-day priority contest against them); any ladder value is fine
        // for a fixture that never actually clashes same-day with a club fixture.
        fixture(4, WORLD_CUP, 6, FixtureImportance::League),
    ]
}

struct CongestionRecorder(Rc<RefCell<Vec<u32>>>);

impl Subsystem for CongestionRecorder {
    fn on_day(&mut self, ctx: &DayContext, _rng: &mut dyn RngSource) -> DayReport {
        self.0.borrow_mut().push(ctx.congestion_score);
        DayReport::silent(SubsystemId::Media)
    }
    fn id(&self) -> SubsystemId {
        SubsystemId::Media
    }
}

/// `congestion_score` must count every orbit competition's fixture landing in the
/// window, not just the league's — the whole point of Slice 5 densifying the fixture
/// load is that this count now reflects a real multi-competition season, not a
/// leftover single-competition assumption.
#[test]
fn congestion_score_counts_every_orbit_competitions_fixtures_in_the_window() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut engine = CalendarEngine::new(1, season(), max_density_fixtures(), competitions());
    engine.register(Box::new(CongestionRecorder(log.clone())));

    engine.advance_bounded(1, false); // tick day 0 only

    let day0_score = log.borrow()[0];
    assert_eq!(
        day0_score, 4,
        "day 0's congestion_score must see all 4 orbit competitions' fixtures within \
         the 10-day window, not just the league's — got {day0_score}"
    );
}

/// A single-competition (league-only) season is the Phase-1 baseline this constant was
/// originally tuned against — confirms the max-density case above is a genuine
/// increase, not noise from a differently-shaped fixture list.
#[test]
fn congestion_score_is_higher_under_slice5_density_than_league_only() {
    let league_only_log = Rc::new(RefCell::new(Vec::new()));
    let mut league_only_engine = CalendarEngine::new(
        1,
        season(),
        vec![fixture(1, LEAGUE, 0, FixtureImportance::League)],
        competitions(),
    );
    league_only_engine.register(Box::new(CongestionRecorder(league_only_log.clone())));
    league_only_engine.advance_bounded(1, false);

    let dense_log = Rc::new(RefCell::new(Vec::new()));
    let mut dense_engine = CalendarEngine::new(1, season(), max_density_fixtures(), competitions());
    dense_engine.register(Box::new(CongestionRecorder(dense_log.clone())));
    dense_engine.advance_bounded(1, false);

    assert!(
        dense_log.borrow()[0] > league_only_log.borrow()[0],
        "Slice 5's combined fixture load must score strictly higher congestion than \
         the league-only Phase-1 baseline"
    );
}

/// `SOFT_FLUSH_THRESHOLD` (currently 3) was tuned back when 3 buffered soft events was
/// a lot — a plausible proxy for "several weeks of quiet". With Slice 5's density, 3
/// events can now arrive from 3 DIFFERENT competitions inside one busy week rather than
/// from several quiet weeks, so `should_flush_soft` fires sooner (in wall-clock terms)
/// than the constant's original intent. This is flagged, not fixed — the doc is
/// explicit that this constant is a placeholder for a later real playtesting pass, not
/// a new number to invent here.
#[test]
fn soft_flush_threshold_can_fire_from_one_busy_multi_competition_week_alone() {
    let clock = GameClock::new(1);
    let three_events_one_week = vec![
        DayReport {
            source: SubsystemId::Media,
            stop_class: StopClass::SoftFlashpoint,
            payload: None,
            mutations: vec![],
        };
        3
    ];
    assert!(
        should_flush_soft(&three_events_one_week, &clock),
        "3 buffered soft events must still flush at the current threshold — this is \
         the mechanical fact this slice's §5.3 note is about: with League + Domestic \
         Cup + a continental tier all live at once, those 3 events can now come from a \
         single busy week rather than several quiet ones. TASK-TUNE: revisit \
         SOFT_FLUSH_THRESHOLD's feel against a real multi-competition playtest, not \
         here."
    );
}
