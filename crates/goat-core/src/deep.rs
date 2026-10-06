//! Path-dependent tier decisions and detailed fixture outcomes, independent of rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepScope {
    pub season: u32,
    pub epoch_day: u32,
    pub pc_league: u32,
    pub pc_club: u32,
    pub leagues: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeepFixtureResult {
    pub season: u32,
    pub round: u32,
    pub league: u32,
    pub epoch_day: u32,
    pub home: u32,
    pub away: u32,
    pub home_goals: u32,
    pub away_goals: u32,
}
impl DeepFixtureResult {
    pub fn key(self) -> (u32, u32, u32, u32, u32) {
        (self.season, self.league, self.round, self.home, self.away)
    }
}
