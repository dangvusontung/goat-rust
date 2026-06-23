//! In-match headspace: confidence / nerves / frustration / flow.
//!
//! Fast-moving axes that live only for the duration of a match.
//! Composure damps volatility and speeds recovery. Form sets the pre-match baseline.

use crate::beats::HeadspaceDelta;
use goat_fixed::Fixed;

/// In-match mental state (all values ‒50..+50 or 0..100 as noted).
#[derive(Debug, Clone, Copy)]
pub struct Headspace {
    /// Confidence: ‒50 (slump) to +50 (in form). Neutral at 0.
    pub confidence: i32,
    /// Nerves: 0 (calm) to 100 (jittery). Starts elevated, drains naturally.
    pub nerves: i32,
    /// Frustration: 0 to 100. Spikes after failures and missed chances.
    pub frustration: i32,
    /// Flow: 0 to 100. Rises on a run of successes; the "zone" state.
    pub flow: i32,
}

impl Headspace {
    /// Neutral headspace (used for testing and when no form data is available).
    pub fn new() -> Self {
        Self {
            confidence: 0,
            nerves: 40,
            frustration: 0,
            flow: 0,
        }
    }

    /// Initialise headspace from player form (0–100 baseline).
    ///
    /// In-form (form > 60) starts with a confidence cushion and calmer nerves.
    /// Out-of-form (form < 40) starts tentative, nerves higher.
    /// This is the form ↔ headspace interplay from bible §6.2: a good player
    /// can still have a nervy night; an out-of-form player can catch fire.
    pub fn from_form(form: Fixed) -> Self {
        let f = form.to_int().clamp(0, 100);
        let confidence = (f - 50) * 2 / 5; // ‒20..+20 mapped from form range
        let nerves = (60 - f / 3).clamp(20, 65); // 20 (hot streak) to 65 (slump)
        Self {
            confidence: confidence.clamp(-20, 20),
            nerves,
            frustration: 0,
            flow: 0,
        }
    }

    /// Apply beat outcome deltas, damped by composure.
    ///
    /// High composure (close to 99) cuts volatility almost in half.
    pub fn apply(&mut self, delta: &HeadspaceDelta, composure: Fixed) {
        let damp_pct = composure.to_int().clamp(1, 99);
        let damp = |v: i8| -> i32 { v as i32 * (150 - damp_pct / 2) / 100 };

        self.confidence = (self.confidence + damp(delta.confidence)).clamp(-50, 50);
        self.frustration = (self.frustration + damp(delta.frustration)).clamp(0, 100);
        self.flow = (self.flow + damp(delta.flow)).clamp(0, 100);
    }

    /// Natural per-beat drift: nerves drain, frustration decays, flow decays.
    pub fn tick(&mut self, composure: Fixed) {
        let decay = 1 + composure.to_int() / 25; // 1–4 nerves drain per beat
        self.nerves = (self.nerves - decay).clamp(0, 100);
        self.frustration = (self.frustration - 2).clamp(0, 100);
        self.flow = (self.flow - 3).clamp(0, 100);
    }

    /// Net modifier to contest probability (in percentage points, ‒25 to +25).
    pub fn contest_mod(&self) -> i32 {
        let conf_bonus = self.confidence / 5; // ‒10 to +10
        let flow_bonus = self.flow / 10; // 0 to +10
        let frust_pen = -(self.frustration / 10); // 0 to ‒10
        let nerves_pen = -(self.nerves / 20); // 0 to ‒5
        (conf_bonus + flow_bonus + frust_pen + nerves_pen).clamp(-25, 25)
    }

    /// Desperation modifier: combines scoreline deficit + time pressure into a
    /// raw contest modifier. A losing player fights harder but less precisely.
    ///
    /// Positive modifier (up to +8 pp) when behind and time running out.
    /// The player takes risks; Composure dampens the panic component.
    pub fn desperation_mod(goals_behind: i32, minutes_remaining: u32, composure: Fixed) -> i32 {
        if goals_behind <= 0 || minutes_remaining > 30 {
            return 0;
        }
        let deficit = goals_behind.clamp(1, 3);
        let urgency = match minutes_remaining {
            0..=5 => 8,
            6..=15 => 5,
            _ => 2, // 16–30 minutes left
        };
        let raw = deficit * urgency;
        let comp_damp = composure.to_int().clamp(1, 99);
        // High composure halves the panic; low composure amplifies.
        (raw * (100 - comp_damp / 3) / 100).clamp(0, 8)
    }
}

impl Default for Headspace {
    fn default() -> Self {
        Self::new()
    }
}
