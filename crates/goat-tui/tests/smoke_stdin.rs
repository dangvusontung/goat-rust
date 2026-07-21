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
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(10);

/// Run `goat-tui` with `input` piped to stdin (then stdin closed, signalling
/// EOF). Returns the captured stdout, or `None` if the process didn't exit
/// within `TIMEOUT` (a hang — the process is killed either way).
fn run_scripted(input: &str) -> Option<String> {
    let mut child: Child = Command::new(env!("CARGO_BIN_EXE_goat-tui"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn goat-tui binary");

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
