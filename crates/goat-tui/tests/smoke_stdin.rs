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
/// tests that need `goat.sav` to live in a scratch directory rather than the
/// crate root, so a pre-seeded save doesn't collide with other tests.
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
        nationality: "Brazilian",
        club: "Riverside Town",
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

    let view = state.players.snapshot(pc);
    let data = goat_save::from_world_state(&state, &view);
    goat_save::save_to_file(&data, dir.join("goat.sav")).expect("save should write");
}

#[test]
fn hard_retirement_age_is_forced_not_offered() {
    use goat_core::tuning::RETIRE_AGE_HARD;

    let dir = std::env::temp_dir().join(format!(
        "goat_tui_smoke_hard_retire_{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    // One week before the hard cap, still under contract — the pre-existing
    // age>=35 && form<40 suggestion wouldn't reliably fire here (form defaults
    // high), and the soft out-of-contract path can't fire either.
    seed_save_at_age_weeks(&dir, RETIRE_AGE_HARD * 52 - 1);

    let script = "L\nF\n1\n"; // load, then advance exactly 1 week — crosses the hard age.
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
        nationality: "Brazilian",
        club: "Riverside Town",
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

    let view = state.players.snapshot(pc);
    let data = goat_save::from_world_state(&state, &view);
    goat_save::save_to_file(&data, dir.join("goat.sav")).expect("save should write");
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
    let script = "L\nG\nG\nQ\nQ\n";
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
