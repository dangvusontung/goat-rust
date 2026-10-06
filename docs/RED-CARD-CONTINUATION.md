# PC red-card continuation — 2026-10-05

The current shared match engine now completes the match after the player is
sent off. This addresses the full-time continuation bug identified in the
[simulation audit](SIMULATION-AUDIT.md).

Dismissal closes the PC's on-pitch minutes immediately, disables further PC
decisions and substitution re-entry, and removes the PC from the starting
sheet without replacing him. NPC commentary and goal attribution continue
until minute 90. Their goals contribute to the final team result; the PC's
individual minutes remain the dismissal time rather than the final clock.

The reduced side uses ten players for shared opportunity creation and 10/11
of its midfield/defensive coverage for possession and conversion. These are
explicit first-order manpower effects, not calibrated tactical adjustments.
The engine does not yet model a manager reorganizing the formation after a
red card, extra time, added time or NPC dismissals.

This behavior applies to shared entry points already used by the active
adapters. Explicit legacy match APIs retain the old stopping behavior so the
frozen legacy goldens remain unchanged. Save layout stays 21; simulation version
is now **4** because the continued goals change replay outcomes. Guarded loading
refuses previous simulation saves; migration is not included.

The regression fixture exercises 256 aggressive-player seeds and requires
multiple actual dismissals before minute 80. It checks final time, closed PC
minutes, absence of later PC actions, a ten-player sheet, possible later NPC
goals, a complete goal-credit ledger and an idempotent completed match.
A new seed-3 golden freezes dismissal at minute **27**, final score **2–2** and
PC output **60**. Existing legacy expected values are untouched.

Validation commands:

```sh
source /workspace/.goat-tools/activate.sh
cargo test --locked -p goat-match --test golden_match shared_dismissal
bash scripts/test.sh
cargo run --locked --release -p goat-tui --bin match-batch -- 10000 42
```

Final validation: **541 passed, 0 failed, 1 ignored**; formatting, Clippy,
seed-42 20-season career invariants and the seed scanner all passed.
The repeated 10,000-match release sample (master seed 42) averaged **3.06**
goals per match: 1.69 for, 1.36 against; W/D/L 45.2%/22.9%/31.9%.
Values are rounded independently. The prior version averaged 3.02 goals.
These samples establish regression observations, not real-football calibration.

Next: controlled calibration of deep/background matches, with matched team
strength, venue and player availability; then dated NPC training/health exposure.
The 150-country task remains deferred.
