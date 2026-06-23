//! Team-strength match simulation and league table helpers.

use crate::world::{ClubId, CLUBS_PER_DIV};
use goat_rng::RngSource;

/// Simulate a single match between two clubs by strength.
/// Returns `(goals_for_home, goals_for_away)`.
pub fn sim_team_match(home_str: u8, away_str: u8, rng: &mut impl RngSource) -> (u32, u32) {
    // Each team gets 5 "shots". P(goal per shot) scaled by att / (att + def + 1).
    // Home advantage: +5 to effective home attack.
    let home_att = home_str as u64 * 2 + 5;
    let away_att = away_str as u64 * 2;
    let home_def = home_str as u64 * 2;
    let away_def = away_str as u64 * 2;

    let for_prob = home_att * 500 / (home_att + away_def + 1);
    let against_prob = away_att * 500 / (away_att + home_def + 1);

    let mut gf = 0u32;
    let mut ga = 0u32;
    for _ in 0..5 {
        if rng.next_range_u64(0, 999) < for_prob {
            gf += 1;
        }
        if rng.next_range_u64(0, 999) < against_prob {
            ga += 1;
        }
    }
    (gf, ga)
}

/// League table entry for a single club.
#[derive(Debug, Clone, Copy, Default)]
pub struct TableEntry {
    pub club_id: ClubId,
    pub w: u32,
    pub d: u32,
    pub l: u32,
    pub gf: u32,
    pub ga: u32,
}

impl TableEntry {
    pub fn points(self) -> u32 {
        self.w * 3 + self.d
    }
    pub fn played(self) -> u32 {
        self.w + self.d + self.l
    }
    pub fn goal_diff(self) -> i32 {
        self.gf as i32 - self.ga as i32
    }
}

/// Full division table (16 entries).
#[derive(Debug, Clone)]
pub struct Table {
    pub entries: [TableEntry; CLUBS_PER_DIV],
}

impl Table {
    pub fn new(div_club_ids: &[ClubId; CLUBS_PER_DIV]) -> Self {
        let mut entries = [TableEntry::default(); CLUBS_PER_DIV];
        for (i, &id) in div_club_ids.iter().enumerate() {
            entries[i].club_id = id;
        }
        Self { entries }
    }

    /// Apply a match result (home_club vs away_club, goals scored).
    pub fn apply_result(&mut self, home: ClubId, away: ClubId, home_gf: u32, home_ga: u32) {
        let record_for = |entry: &mut TableEntry, gf: u32, ga: u32| {
            entry.gf += gf;
            entry.ga += ga;
            if gf > ga {
                entry.w += 1;
            } else if gf == ga {
                entry.d += 1;
            } else {
                entry.l += 1;
            }
        };
        for e in &mut self.entries {
            if e.club_id == home {
                record_for(e, home_gf, home_ga);
            } else if e.club_id == away {
                record_for(e, home_ga, home_gf);
            }
        }
    }

    /// Return entries sorted: points desc, goal diff desc, gf desc.
    pub fn sorted(&self) -> Vec<&TableEntry> {
        let mut v: Vec<&TableEntry> = self.entries.iter().collect();
        v.sort_by(|a, b| {
            b.points()
                .cmp(&a.points())
                .then(b.goal_diff().cmp(&a.goal_diff()))
                .then(b.gf.cmp(&a.gf))
        });
        v
    }

    /// Position (1-based) of a club in the sorted table.
    pub fn position_of(&self, club_id: ClubId) -> usize {
        self.sorted()
            .iter()
            .position(|e| e.club_id == club_id)
            .unwrap_or(0)
            + 1
    }

    /// Serialise to `[u32; 5 * CLUBS_PER_DIV]` (w, d, l, gf, ga per club, in original order).
    pub fn to_raw(&self) -> [u32; 5 * CLUBS_PER_DIV] {
        let mut out = [0u32; 5 * CLUBS_PER_DIV];
        for (i, e) in self.entries.iter().enumerate() {
            out[i] = e.w;
            out[CLUBS_PER_DIV + i] = e.d;
            out[CLUBS_PER_DIV * 2 + i] = e.l;
            out[CLUBS_PER_DIV * 3 + i] = e.gf;
            out[CLUBS_PER_DIV * 4 + i] = e.ga;
        }
        out
    }

    /// Restore from `to_raw` output, with the same club_id ordering.
    pub fn from_raw(
        raw: &[u32; 5 * CLUBS_PER_DIV],
        div_club_ids: &[ClubId; CLUBS_PER_DIV],
    ) -> Self {
        let mut t = Self::new(div_club_ids);
        for (i, e) in t.entries.iter_mut().enumerate() {
            e.w = raw[i];
            e.d = raw[CLUBS_PER_DIV + i];
            e.l = raw[CLUBS_PER_DIV * 2 + i];
            e.gf = raw[CLUBS_PER_DIV * 3 + i];
            e.ga = raw[CLUBS_PER_DIV * 4 + i];
        }
        t
    }
}
