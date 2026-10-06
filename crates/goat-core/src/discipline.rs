//! Competition-scoped card history. Fixture dates, not elapsed weeks, serve bans.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NpcCardEvent {
    pub season: u32,
    pub competition_id: u32,
    pub pop_idx: u32,
    pub fixture_id: u64,
    pub epoch_day: u32,
    pub minute: u8,
    /// 0 yellow; 1 second-yellow dismissal; 2 direct red.
    pub kind: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisciplineRules {
    pub yellow_threshold: u32,
    pub direct_red_games: u32,
}
impl DisciplineRules {
    pub fn for_competition(competition: u32) -> Self {
        Self {
            yellow_threshold: if competition == crate::calendar_loop::LEAGUE_COMPETITION_ID {
                5
            } else {
                2
            },
            direct_red_games: 3,
        }
    }
}
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct DisciplineStatus {
    pub yellows: u32,
    pub ban_games: u32,
}

/// `events` is the chronological history for one player.
/// `fixtures` contains this player's club fixture dates in this competition,
/// including DNPs and light fixtures, sorted by date. Actual date overrides belong in this list.
pub fn status(
    events: &[NpcCardEvent],
    season: u32,
    day: u32,
    competition: u32,
    fixtures: &[u32],
) -> DisciplineStatus {
    status_with_rules(
        events,
        season,
        day,
        competition,
        fixtures,
        DisciplineRules::for_competition(competition),
        None,
    )
}
/// Versioned competition policy; yellow resets never erase already earned bans.
#[allow(clippy::too_many_arguments)]
pub fn status_with_rules(
    events: &[NpcCardEvent],
    season: u32,
    day: u32,
    competition: u32,
    fixtures: &[u32],
    rules: DisciplineRules,
    yellow_reset_day: Option<u32>,
) -> DisciplineStatus {
    let mut result = DisciplineStatus::default();
    let mut last_day = None;
    let mut last_season = 0;
    let mut last_yellow_fixture = None;
    let mut reset_applied = false;
    for event in events
        .iter()
        .filter(|e| e.competition_id == competition && e.epoch_day < day)
    {
        if let Some(previous) = last_day {
            let served = fixtures
                .partition_point(|&d| d < event.epoch_day)
                .saturating_sub(fixtures.partition_point(|&d| d <= previous))
                as u32;
            result.ban_games = result.ban_games.saturating_sub(served);
        }
        if event.season != last_season {
            result.yellows = 0;
            last_season = event.season;
            last_yellow_fixture = None;
        }
        if !reset_applied && yellow_reset_day.is_some_and(|reset| event.epoch_day >= reset) {
            result.yellows = 0;
            last_yellow_fixture = None;
            reset_applied = true;
        }
        match event.kind {
            0 => {
                result.yellows += 1;
                last_yellow_fixture = Some(event.fixture_id);
                if result.yellows % rules.yellow_threshold == 0 {
                    result.ban_games += 1;
                }
            }
            1 => {
                if last_yellow_fixture == Some(event.fixture_id) && result.yellows > 0 {
                    if result.yellows % rules.yellow_threshold == 0 {
                        result.ban_games = result.ban_games.saturating_sub(1);
                    }
                    result.yellows -= 1;
                }
                last_yellow_fixture = None;
                result.ban_games += 1;
            }
            2 => result.ban_games += rules.direct_red_games,
            _ => {}
        }
        last_day = Some(event.epoch_day);
    }
    if let Some(previous) = last_day {
        let served = fixtures
            .partition_point(|&d| d < day)
            .saturating_sub(fixtures.partition_point(|&d| d <= previous))
            as u32;
        result.ban_games = result.ban_games.saturating_sub(served);
    }
    if yellow_reset_day.is_some_and(|reset| day >= reset) && !reset_applied {
        result.yellows = 0;
    }
    if last_season != season {
        result.yellows = 0;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn card(day: u32, competition: u32, kind: u8) -> NpcCardEvent {
        NpcCardEvent {
            season: 1,
            competition_id: competition,
            pop_idx: 1,
            fixture_id: day as u64,
            epoch_day: day,
            minute: 35,
            kind,
        }
    }
    #[test]
    fn league_and_cup_bans_are_separate_and_dnp_fixtures_serve_them() {
        let events = [card(10, 1, 2), card(12, 2, 0), card(13, 2, 0)];
        let fixtures = [17, 24, 31, 38];
        assert_eq!(status(&events, 1, 17, 1, &fixtures).ban_games, 3);
        assert_eq!(status(&events, 1, 24, 1, &fixtures).ban_games, 2);
        assert_eq!(status(&events, 1, 38, 1, &fixtures).ban_games, 0);
        assert_eq!(status(&events, 1, 17, 2, &[]).ban_games, 1);
        assert_eq!(status(&events, 2, 17, 1, &fixtures).yellows, 0);
        assert_eq!(status(&events, 2, 17, 1, &fixtures).ban_games, 3);
    }
    #[test]
    fn same_fixture_cards_do_not_serve_a_new_ban_and_five_yellows_trigger_it() {
        let events: Vec<_> = [1, 8, 15, 22, 29]
            .into_iter()
            .map(|day| card(day, 1, 0))
            .collect();
        assert_eq!(
            status(&events, 1, 36, 1, &[1, 8, 15, 22, 29, 36]).ban_games,
            1
        );
        assert_eq!(
            status(&events, 1, 43, 1, &[1, 8, 15, 22, 29, 36]).ban_games,
            0
        );
        let two = [card(1, 1, 0), card(1, 1, 1)];
        assert_eq!(status(&two, 1, 8, 1, &[1, 8]).ban_games, 1);
    }
    #[test]
    fn second_yellow_rescinds_the_match_caution_instead_of_double_banning() {
        let mut events: Vec<_> = [1, 8, 15, 22, 29]
            .into_iter()
            .map(|d| card(d, 1, 0))
            .collect();
        events.push(card(29, 1, 1));
        let at_next = status(&events, 1, 36, 1, &[1, 8, 15, 22, 29, 36, 43]);
        assert_eq!(at_next.yellows, 4);
        assert_eq!(at_next.ban_games, 1);
        assert_eq!(
            status(&events, 1, 43, 1, &[1, 8, 15, 22, 29, 36, 43]).ban_games,
            0
        );
    }
    #[test]
    fn summer_does_not_serve_a_ban_and_rescheduled_matches_do() {
        let events = [card(100, 1, 2)];
        assert_eq!(status(&events, 2, 190, 1, &[]).ban_games, 3);
        assert_eq!(
            status(&events, 2, 220, 1, &[190, 200, 215, 220]).ban_games,
            0
        );
        assert_eq!(
            status(&events, 2, 210, 1, &[190, 200, 215, 220]).ban_games,
            1
        );
    }
}
