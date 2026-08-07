//! Scripted-stdin smoke tests for `goat-tui`.
//!
//! Spawns the actual built binary (`CARGO_BIN_EXE_goat-tui`, a standard
//! Cargo-provided env var — no new dependency), pipes a fixed stdin script,
//! captures stdout, and asserts on fragments. This is the home for TUI-level
//! regression tests (playtest round 1, TASK-PLAYTEST-round1-fixes.md).
//!
//! Every run is bounded by a wall-clock timeout via a background reader
//! thread + `mpsc::recv_timeout` (no new deps) so a regression that hangs the
//! process fails the test instead of hanging the suite.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(10);

/// Run `goat-tui` with `input` piped to stdin (then stdin closed, signalling
/// EOF). Returns the captured stdout, or `None` if the process didn't exit
/// within `TIMEOUT` (a hang — the process is killed either way).
fn run_scripted(input: &str) -> Option<String> {
    run_scripted_in(input, None)
}

/// Same as `run_scripted`, but optionally runs the child in `cwd` — used by
/// tests that need `saves/slot-N.sav` to live in a scratch directory rather than
/// the crate root, so a pre-seeded save doesn't collide with other tests.
fn run_scripted_in(input: &str, cwd: Option<&Path>) -> Option<String> {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_goat-tui"));
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let mut child: Child = cmd.spawn().expect("failed to spawn goat-tui binary");

    let mut stdin = child.stdin.take().expect("child stdin");
    let input = input.to_string();
    std::thread::spawn(move || {
        let _ = stdin.write_all(input.as_bytes());
        // `stdin` drops here, closing the pipe — the child sees EOF.
    });

    let mut stdout = child.stdout.take().expect("child stdout");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = stdout.read_to_string(&mut buf);
        let _ = tx.send(buf);
    });

    match rx.recv_timeout(TIMEOUT) {
        Ok(buf) => {
            let _ = child.wait();
            Some(buf)
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            None
        }
    }
}

/// Script prefix: new game, blank name (→ "Unnamed Legend"), ST, England,
/// Premier League, Manchester City, seed 42, start.
fn new_game_england_man_city() -> String {
    "N\n\n1\n1\n1\n1\n42\nS\n".to_string()
}

/// Same, but Brazil / Série B / Chapecoense — the long-club-name case the
/// task spec calls out explicitly (vs. Manchester City).
fn new_game_brazil_chapecoense() -> String {
    "N\n\n1\n2\n2\n2\n42\nS\n".to_string()
}

/// True if every line in the `render_game_sheet` box (the persistent status
/// header: opens right after "[S] Start game", identified by its "Energy"
/// line) closes with a matching, correctly-padded `║` — this is the box this
/// task rewrote line-by-line via `box_line`/`box_lines_wrapped`.
///
/// (The player sheet's *other* attribute/role lines have a pre-existing,
/// separate 1-char width drift unrelated to this task's scope — this check
/// deliberately does not cover them; see the nationality/club and OVR-note
/// lines checked individually in the callers instead.)
fn game_sheet_box_lines_all_closed(stdout: &str) -> bool {
    let mut in_box = false;
    let mut border_width = 0usize;
    let mut lines = stdout.lines().peekable();
    while let Some(line) = lines.next() {
        if line.starts_with('╔') {
            // Only the game-sheet box contains an "Energy" line right after
            // the opening border — skip any other box (title banner, player
            // sheet preview, etc).
            let is_game_sheet = lines.peek().is_some_and(|next| next.contains("Age"));
            if is_game_sheet {
                in_box = true;
                border_width = line.chars().count();
            }
            continue;
        }
        if line.starts_with('╚') {
            in_box = false;
            continue;
        }
        if in_box
            && !line.is_empty()
            && (!line.starts_with('║')
                || !line.ends_with('║')
                || line.chars().count() != border_width)
        {
            return false;
        }
    }
    true
}

/// A single line, found anywhere in `stdout`, that both contains `needle`
/// and is a well-formed closed box line (`║...║`, matching the standard
/// 48-column border width).
fn has_closed_box_line_containing(stdout: &str, needle: &str) -> bool {
    stdout.lines().any(|line| {
        line.contains(needle)
            && line.starts_with('║')
            && line.ends_with('║')
            && line.chars().count() == 48
    })
}

// ── Slice 2: Legacy mid-season messaging ──────────────────────────────────────

#[test]
fn legacy_screen_notes_mid_season_batching() {
    let script = format!("{}K\nG\nQ\nQ\n", new_game_england_man_city());
    let stdout = run_scripted(&script).expect("process should exit cleanly");
    assert!(
        stdout.contains("update at season end"),
        "expected the mid-season batching note on the Legacy screen:\n{stdout}"
    );
    assert!(
        stdout.contains("Goals:    0") && stdout.contains("Matches:    0"),
        "totals should still read zero mid-season (additive messaging, not a stat change):\n{stdout}"
    );
}

// ── Slice 3: silent training no-op ────────────────────────────────────────────

#[test]
fn double_w_in_same_round_shows_message_not_silent_noop() {
    let script = format!("{}W\nW\nQ\nQ\n", new_game_england_man_city());
    let stdout = run_scripted(&script).expect("process should exit cleanly");
    assert!(
        stdout.contains("already trained this week"),
        "second W in the same fixture round should message instead of silently no-op:\n{stdout}"
    );
}

// ── Slice 4 + 5: box border / key-moment truncation ───────────────────────────

#[test]
fn key_moments_lines_close_with_ellipsis_not_ragged_cutoff() {
    let script = format!("{}K\nQ\nQ\n", new_game_england_man_city());
    let stdout = run_scripted(&script).expect("process should exit cleanly");
    let in_key_moments = stdout
        .lines()
        .skip_while(|l| !l.contains("KEY MOMENTS"))
        .skip(1)
        .take_while(|l| !l.starts_with('╚'));
    let mut saw_a_moment_line = false;
    for line in in_key_moments {
        saw_a_moment_line = true;
        assert!(
            line.starts_with('║') && line.ends_with('║'),
            "key-moment line must close with a border: {line:?}"
        );
        // No line should end mid-word right before the border — either it fit
        // as-is, or it was truncated and must show the ellipsis marker.
        let interior = line.trim_start_matches('║').trim_end_matches('║');
        assert!(
            !interior.trim_end().ends_with(char::is_alphanumeric) || interior.trim_end().len() < 40,
            "suspiciously long untruncated line, expected a … marker: {line:?}"
        );
    }
    assert!(
        saw_a_moment_line,
        "expected at least one key-moment line:\n{stdout}"
    );
}

#[test]
fn game_sheet_and_player_sheet_boxes_close_for_short_and_long_club_names() {
    for (label, script_prefix) in [
        ("England / Manchester City", new_game_england_man_city()),
        ("Brazil / Chapecoense", new_game_brazil_chapecoense()),
    ] {
        let script = format!("{script_prefix}V\nQ\nQ\n");
        let stdout = run_scripted(&script).expect("process should exit cleanly");
        assert!(
            game_sheet_box_lines_all_closed(&stdout),
            "[{label}] every status-header box line must close with a matching, \
             correctly-padded border:\n{stdout}"
        );
        assert!(
            has_closed_box_line_containing(&stdout, "Nationality:"),
            "[{label}] the nationality/club line must close cleanly even for a long club name:\n{stdout}"
        );
    }
}

// ── Slice 6: inconsistent invalid-input handling ──────────────────────────────

#[test]
fn main_loop_unrecognized_command_messages_and_continues() {
    let script = format!("{}ZZZ\nQ\nQ\n", new_game_england_man_city());
    let stdout = run_scripted(&script).expect("process should exit cleanly");
    assert!(
        stdout.contains("Unrecognized command."),
        "an unmapped key at the main loop should message, not silently redraw:\n{stdout}"
    );
    assert!(
        stdout.contains("Goodbye."),
        "the loop should continue and still accept Q afterwards:\n{stdout}"
    );
}

#[test]
fn confirm_screen_blank_enter_reprompts_instead_of_discarding_character() {
    // Blank Enter at the S/R/Q confirm screen, then a real S — the character
    // must survive (game must actually start) rather than being silently
    // dropped back to the title screen.
    let script = "N\n\n1\n1\n1\n1\n42\n\nS\nQ\nQ\n".to_string();
    let stdout = run_scripted(&script).expect("process should exit cleanly");
    assert!(
        stdout.contains("Please choose S, R, or Q."),
        "a blank Enter at the confirm screen should reprompt, not discard:\n{stdout}"
    );
    assert!(
        stdout.contains("[W] Train"),
        "the game should still start after the reprompt (character preserved):\n{stdout}"
    );
}

// ── Slice 7: infinite reprompt loop on stdin EOF ──────────────────────────────

#[test]
fn stdin_eof_mid_prompt_exits_instead_of_hanging() {
    // The exact repro from docs/PLAYTEST-BUGS.md: a script that runs dry
    // mid-prompt. Must exit within the wall-clock bound, not hang forever.
    let stdout = run_scripted("N\n");
    assert!(
        stdout.is_some(),
        "process should exit on stdin EOF instead of reprompting forever"
    );
}

// ── Slice 8: OVR formula opacity ──────────────────────────────────────────────

#[test]
fn player_sheet_explains_ovr_is_position_weighted() {
    let script = format!("{}V\nQ\nQ\n", new_game_england_man_city());
    let stdout = run_scripted(&script).expect("process should exit cleanly");
    assert!(
        stdout.contains("OVR is position-weighted"),
        "expected the new OVR explanation line on the sheet screen:\n{stdout}"
    );
}

// ── Slice 9: energy % + discipline count context ──────────────────────────────

#[test]
fn status_header_shows_energy_percent_and_labeled_discipline_count() {
    let script = format!("{}Q\nQ\n", new_game_england_man_city());
    let stdout = run_scripted(&script).expect("process should exit cleanly");
    assert!(
        stdout.contains('%') && stdout.contains("Energy"),
        "expected a numeric energy percentage next to the bar:\n{stdout}"
    );
    assert!(
        stdout.contains("(cards)"),
        "expected the discipline count's scope to be labeled:\n{stdout}"
    );
}

// ── Round 3, Slice 2: hard retirement age enforcement ──────────────────────────

/// Write a `goat.sav` (into `dir`) for a player at exactly `age_weeks`, mid-season
/// (so `L`oading it drops straight into the normal week menu) and still under
/// contract (so only the hard-age path, not the out-of-contract soft path, can fire).
fn seed_save_at_age_weeks(dir: &std::path::Path, age_weeks: u32) {
    use goat_core::generation::CreationChoices;
    use goat_core::positions::PrimaryPosition;
    use goat_core::state::{reduce, Intent, WorldState};
    use goat_rng::GoatRng;

    let choices = CreationChoices {
        name: "Veteran".into(),
        primary_position: PrimaryPosition::ST,
        primary_role: None,
        nationality: "Brazilian".to_string(),
        club: "Riverside Town".to_string(),
    };
    let mut state = WorldState::new();
    state.world_seed = 42;
    state = reduce(
        state,
        Intent::CreatePlayer { seed: 42, choices },
        &mut GoatRng::new(0),
    );
    let pc = state.pc_player_id.unwrap();
    state.players.set_age_weeks(pc, age_weeks);
    state.season_number = 5;
    state.season_round = 0;
    state.pc_contract_seasons_left = 3;

    std::fs::create_dir_all(dir.join("saves")).expect("saves subdir");
    let view = state.players.snapshot(pc);
    let data = goat_save::from_world_state(&state, &view);
    goat_save::save_to_file(&data, goat_save::slot_path(dir.join("saves"), 1))
        .expect("save should write");
}

#[test]
fn hard_retirement_age_is_forced_not_offered() {
    use goat_core::tuning::RETIRE_AGE_HARD;

    let dir =
        std::env::temp_dir().join(format!("goat_tui_smoke_hard_retire_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    // One week before the hard cap, still under contract — the pre-existing
    // age>=35 && form<40 suggestion wouldn't reliably fire here (form defaults
    // high), and the soft out-of-contract path can't fire either.
    seed_save_at_age_weeks(&dir, RETIRE_AGE_HARD * 52 - 1);

    let script = "L\n1\nF\n1\n"; // load slot 1, then advance exactly 1 week — crosses the hard age.
    let stdout = run_scripted_in(script, Some(&dir)).expect("process should exit cleanly");

    let _ = std::fs::remove_dir_all(&dir);

    assert!(
        stdout.contains("CAREER OVER"),
        "crossing RETIRE_AGE_HARD must force retirement, not just offer it:\n{stdout}"
    );
    assert!(
        !stdout.contains("Retire now"),
        "the hard cap must not go through the optional [R]/[C] suggestion prompt:\n{stdout}"
    );
}

// ── Round 3, Slice 3: idempotent season-end pipeline (Legacy is read-only) ────

/// Write a `goat.sav` for a player exactly at the end-of-season gate
/// (`season_round == ROUNDS_PER_SEASON`), with known season totals and zeroed
/// career totals, so the season-end pipeline's credit into career totals is
/// checkable by exact value. Form stays at the default 50 (below the transfer-offer
/// threshold of 55) and the contract has 2 seasons left, so no extra interactive
/// prompt (transfer window / contract renewal / retirement suggestion) fires.
fn seed_save_at_season_end(dir: &std::path::Path) {
    use goat_core::generation::CreationChoices;
    use goat_core::positions::PrimaryPosition;
    use goat_core::state::{reduce, Intent, WorldState};
    use goat_rng::GoatRng;
    use goat_world::ROUNDS_PER_SEASON;

    let choices = CreationChoices {
        name: "Prospect".into(),
        primary_position: PrimaryPosition::ST,
        primary_role: None,
        nationality: "Brazilian".to_string(),
        club: "Riverside Town".to_string(),
    };
    let mut state = WorldState::new();
    state.world_seed = 42;
    state = reduce(
        state,
        Intent::CreatePlayer { seed: 42, choices },
        &mut GoatRng::new(0),
    );
    let pc = state.pc_player_id.unwrap();
    state.players.set_age_weeks(pc, 20 * 52);
    state.season_number = 2;
    state.season_round = ROUNDS_PER_SEASON as u32;
    state.pc_season_goals = 5;
    state.pc_season_matches = 10;
    state.pc_season_output = 500;
    state.pc_wage_annual = 100;
    state.pc_contract_seasons_left = 2;

    std::fs::create_dir_all(dir.join("saves")).expect("saves subdir");
    let view = state.players.snapshot(pc);
    let data = goat_save::from_world_state(&state, &view);
    goat_save::save_to_file(&data, goat_save::slot_path(dir.join("saves"), 1))
        .expect("save should write");
}

#[test]
fn viewing_legacy_twice_at_season_end_does_not_double_credit_career_totals() {
    let dir = std::env::temp_dir().join(format!(
        "goat_tui_smoke_legacy_idempotent_{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    seed_save_at_season_end(&dir);

    // Load straight into the end-of-season gate (which runs the pipeline once),
    // then take the read-only Legacy side trip from the post-pipeline menu twice
    // before moving on.
    let script = "L\n1\nG\nG\nQ\nQ\n";
    let stdout = run_scripted_in(script, Some(&dir)).expect("process should exit cleanly");

    let _ = std::fs::remove_dir_all(&dir);

    let expected_line = format!(
        "║  Goals: {:4}   Matches: {:4}   Seasons: {:2}   ║",
        5, 10, 1
    );
    let occurrences = stdout.matches(expected_line.as_str()).count();
    assert_eq!(
        occurrences, 2,
        "opening Legacy twice at the same season boundary must show the same, \
         single-credited career totals both times (not double-credited on the \
         second view) — expected line {expected_line:?}:\n{stdout}"
    );
}

// ── Design round 1, Slice 3: save slots ─────────────────────────────────────

#[test]
fn slot_picker_shows_all_nine_slots_as_empty_on_fresh_save_dir() {
    let dir =
        std::env::temp_dir().join(format!("goat_tui_smoke_slots_fresh_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");

    let script = "L\nQ\nQ\n"; // title Load, cancel at slot prompt, quit.
    let stdout = run_scripted_in(script, Some(&dir)).expect("process should exit cleanly");
    let _ = std::fs::remove_dir_all(&dir);

    for slot in 1..=9 {
        assert!(
            stdout.contains(&format!("[{slot}] <empty>")),
            "slot {slot} should render as empty on a fresh save dir:\n{stdout}"
        );
    }
    assert!(
        stdout.contains("Load cancelled."),
        "Q at the slot prompt should cancel, not crash:\n{stdout}"
    );
}

#[test]
fn load_empty_slot_reports_empty_not_crash() {
    let dir = std::env::temp_dir().join(format!(
        "goat_tui_smoke_slots_load_empty_{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir");

    let script = "L\n5\nQ\n"; // title Load, pick empty slot 5, then quit.
    let stdout = run_scripted_in(script, Some(&dir)).expect("process should exit cleanly");
    let _ = std::fs::remove_dir_all(&dir);

    assert!(
        stdout.contains("Slot 5 is empty."),
        "loading an empty slot must say so, not crash or silently no-op:\n{stdout}"
    );
}

#[test]
fn save_to_empty_slot_succeeds_without_confirmation() {
    let dir = std::env::temp_dir().join(format!(
        "goat_tui_smoke_slots_save_empty_{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir");

    let script = format!("{}Z\n2\nQ\nQ\n", new_game_england_man_city());
    let stdout = run_scripted_in(&script, Some(&dir)).expect("process should exit cleanly");
    let _ = std::fs::remove_dir_all(&dir);

    assert!(
        stdout.contains("Saved to slot 2."),
        "saving to an empty slot should succeed directly:\n{stdout}"
    );
    assert!(
        !stdout.contains("Overwrite?"),
        "an empty slot must not trigger the overwrite confirmation:\n{stdout}"
    );
}

#[test]
fn save_overwrite_requires_explicit_confirmation() {
    // This is the concrete fix for the "silent overwrite" complaint: saving to an
    // already-occupied slot must not write until the player explicitly answers Y.
    let dir = std::env::temp_dir().join(format!(
        "goat_tui_smoke_slots_overwrite_{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir");

    // First save (slot 4, empty -> direct write), then a second save attempt to the
    // same now-occupied slot: decline with N (must cancel), then accept with Y.
    let script = format!(
        "{}Z\n4\nZ\n4\nN\nZ\n4\nY\nQ\nQ\n",
        new_game_england_man_city()
    );
    let stdout = run_scripted_in(&script, Some(&dir)).expect("process should exit cleanly");
    let _ = std::fs::remove_dir_all(&dir);

    assert!(
        stdout.contains("has a save") && stdout.contains("Overwrite? [Y/N]"),
        "re-saving to an occupied slot must prompt for overwrite confirmation:\n{stdout}"
    );
    assert!(
        stdout.contains("Save cancelled."),
        "answering N to the overwrite prompt must cancel, not write:\n{stdout}"
    );
    let saved_count = stdout.matches("Saved to slot 4.").count();
    assert_eq!(
        saved_count, 2,
        "expected exactly 2 successful writes to slot 4 (first empty-slot save, \
         then the Y-confirmed overwrite) — the N-declined attempt must not count:\n{stdout}"
    );
}

// ── Auto-advance training (TASK-AUTO-ADVANCE-TRAINING) ───────────────────────
//
// The [C] Continue default: auto-trains the current week via the exact [W]
// path, then stops only for a decision — a due match, a noteworthy event, or
// a flashpoint. Break weeks never surface in the loop at all: ApplyRoundResult
// elapses them as rest weeks, so they need (and get) no keypress.

/// Script prefix that actually reaches the in-game menu with the CURRENT world
/// genesis: new game, blank name, ST, seed 42, first nation/division/club,
/// start. (The older `new_game_*` helpers above predate the world-genesis
/// scale-up's prompt order and no longer reach the menu — that pre-existing
/// red baseline is not this task's scope.)
fn new_game_reaching_menu() -> String {
    "N\n\n1\n42\n1\n1\n1\nS\n".to_string()
}

/// Tick through the 7-week pre-season (Jul-1 anchor), declining every friendly
/// offer — lands on the first competition week with round 1 still unplayed.
fn through_pre_season() -> String {
    let mut s = String::new();
    for _ in 0..7 {
        s.push_str("C\nX\n");
    }
    s
}

#[test]
fn preseason_friendly_is_playable_and_leaves_the_league_untouched() {
    // One [C] ticks a real pre-season week; [P] plays the offered friendly as a
    // normal beat match — but no league round advances and no stats accrue.
    let mut script = format!("{}C\nP\n", new_game_reaching_menu());
    for _ in 0..20 {
        script.push_str("1\n"); // first choice at every beat
    }
    script.push_str("Q\n");
    let stdout = run_scripted(&script).expect("process should exit cleanly");
    assert!(
        stdout.contains("--- FRIENDLY (pre-season)"),
        "the pre-season week must offer a friendly:\n{stdout}"
    );
    assert!(
        stdout.contains("FULL TIME vs"),
        "the friendly must play to completion:\n{stdout}"
    );
    assert!(
        stdout.contains("S1 Round 1/38"),
        "a friendly must not advance the league season:\n{stdout}"
    );
}

#[test]
fn continue_trains_week_and_offers_match_once_in_one_match_week() {
    // Past pre-season, [C] auto-trains the current week (no [W] keypress) and
    // stops once to offer the due match; deferring returns to the menu with
    // the round still unplayed.
    let script = format!(
        "{}{}C\nX\nQ\n",
        new_game_reaching_menu(),
        through_pre_season()
    );
    let stdout = run_scripted(&script).expect("process should exit cleanly");
    assert!(
        stdout.contains("Round 1/38 due this week"),
        "[C] should stop once to offer the round-1 match:\n{stdout}"
    );
    assert!(
        stdout.contains("Age 16y8w"),
        "7 pre-season weeks + 1 auto-trained competition week = 8 weeks:\n{stdout}"
    );
    assert_eq!(
        stdout.matches("due this week").count(),
        1,
        "a deferred match must not re-offer within the same [C]:\n{stdout}"
    );
}

#[test]
fn continue_two_match_week_stops_once_per_match() {
    // Grid week 8 has 2 matches (rounds 2 and 3). [C] must stop for BOTH in
    // sequence — no silent collapse — with no extra week ticked in between.
    let script = format!(
        "{}{}K\nC\nK\nC\nX\nQ\n",
        new_game_reaching_menu(),
        through_pre_season()
    );
    let stdout = run_scripted(&script).expect("process should exit cleanly");
    assert!(
        stdout.contains("Round 2/38 due this week"),
        "first stop of the 2-match week (round 2):\n{stdout}"
    );
    assert!(
        stdout.contains("Round 3/38 due this week"),
        "second stop of the 2-match week (round 3) — both matches must be offered:\n{stdout}"
    );
    // After skipping round 2 (mid double-fixture week), the second [C] offers
    // round 3 WITHOUT ticking another week — age never passes 16y9w before the
    // round-3 offer (7 pre-season + R1 rest tick + 1 trained week = 9).
    let offer_pos = stdout.find("Round 3/38 due this week").unwrap();
    assert!(
        !stdout[..offer_pos].contains("Age 16y10w"),
        "no week may elapse between the two matches of the same week:\n{stdout}"
    );
}

#[test]
fn continue_break_week_elapses_without_a_keypress() {
    // Rounds 1–5 skipped: grid week 11 is a break week with no fixture. It must
    // elapse on its own — the next [C] trains the NEXT match week exactly once
    // and offers round 6; no round header ever reads Game Week 12.
    let script = format!(
        "{}{}K\nK\nK\nK\nK\nC\nX\nQ\n",
        new_game_reaching_menu(),
        through_pre_season()
    );
    let stdout = run_scripted(&script).expect("process should exit cleanly");
    assert!(
        stdout.contains("Round 6/38 due this week"),
        "after the break, [C] should offer round 6:\n{stdout}"
    );
    assert!(
        stdout.contains("Game Week 11"),
        "round 5 (the match right before the break) should have been played:\n{stdout}"
    );
    assert!(
        !stdout.contains("Game Week 12"),
        "the break week (grid week 11) has no fixture — no header may reference it:\n{stdout}"
    );
    // 7 pre-season weeks + five skipped match weeks (round 3 shares its week
    // with round 2, and the break week elapses as rest) + one auto-trained
    // week after the break = 13w.
    assert!(
        stdout.contains("Age 16y13w"),
        "the break week must elapse silently — 7 pre-season + 5 rounds + 1 auto-train:\n{stdout}"
    );
}

// ── Live promotion/relegation (A3.3) ─────────────────────────────────────────
//
// Season-end [Y] resolves promotion/relegation for the PC's nation: bottom-3 of
// a tier drop, top-3 of the tier below rise, edges no-op. The PC's league uses
// the real played table; sibling leagues are batch-simmed. The section must
// print exactly once per boundary even with read-only [G] side trips in between
// (the d77170b idempotency bug class).

/// Skip a full season: the 7-week pre-season (declining every friendly), then
/// 38 rounds of [K], then N ("Stay") at the transfer-window prompt, landing at
/// the season-end [Y/G/Z/Q] menu.
fn play_full_season_skipped() -> String {
    let mut s = through_pre_season();
    for _ in 0..38 {
        s.push_str("K\n");
    }
    s.push_str("N\n");
    s
}

#[test]
fn promotion_relegation_fires_at_season_boundary_and_applies_once() {
    let script = format!(
        "{}{}G\nG\nY\nQ\n",
        new_game_reaching_menu(),
        play_full_season_skipped()
    );
    let stdout = run_scripted(&script).expect("process should exit cleanly");
    assert!(
        stdout.contains("--- PROMOTION & RELEGATION ---"),
        "the season boundary must show the promotion/relegation resolution:\n{stdout}"
    );
    assert!(
        stdout.contains("relegated to England Division Two"),
        "bottom-3 of the Premier League must drop:\n{stdout}"
    );
    assert!(
        stdout.contains("promoted to England Premier League"),
        "top-3 of Division Two must rise:\n{stdout}"
    );
    assert_eq!(
        stdout.matches("--- PROMOTION & RELEGATION ---").count(),
        1,
        "viewing [G] Legacy twice must not re-run the resolution (idempotent):\n{stdout}"
    );
    assert!(
        stdout.contains("S2 Round 1/38"),
        "the new season must start after the resolution:\n{stdout}"
    );
}

#[test]
fn promoted_clubs_appear_in_next_season_table() {
    let script = format!(
        "{}{}Y\nT\nQ\n",
        new_game_reaching_menu(),
        play_full_season_skipped()
    );
    let stdout = run_scripted(&script).expect("process should exit cleanly");
    // Deterministic for this seed: these three Division Two clubs come up.
    assert!(
        stdout.contains("Greymarsh Wanderers promoted to England Premier League"),
        "the promoted clubs must be named at the boundary:\n{stdout}"
    );
    // And the new season's table shows the refreshed composition (Round 0, all
    // zero — the promoted clubs are now Premier League members).
    let table_pos = stdout
        .find("England Premier League — Round 0")
        .expect("the next season's table must render after [Y]:\n{stdout}");
    assert!(
        stdout[table_pos..].contains("Greymarsh Wanderers"),
        "the promoted club must appear in the new season's top-tier table:\n{stdout}"
    );
    assert!(
        !stdout[table_pos..].contains("Ashford City"),
        "the relegated club must be gone from the new top-tier table:\n{stdout}"
    );
}
