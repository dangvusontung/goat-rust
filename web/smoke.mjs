// Node smoke test for goat-web (wasm-pack --target nodejs build in ./pkg-node).
// Plays a full season via train()+skip_match(), exercises the interactive
// match path once, crosses the season boundary, and checks a save→load
// roundtrip. Run from web/:  node smoke.mjs
import * as goat from './pkg-node/goat_web.js';

let failures = 0;
function check(label, cond) {
  console.log(`${cond ? 'PASS' : 'FAIL'}  ${label}`);
  if (!cond) failures++;
}
function parse(json, label) {
  const v = JSON.parse(json);
  if (v && v.error) {
    console.log(`FAIL  ${label}: ${v.error}`);
    failures++;
    process.exit(1);
  }
  return v;
}

const SEED = 42n;

// ── Picker data ─────────────────────────────────────────────────────────────
const nations = parse(goat.get_nations(), 'get_nations');
check('get_nations returns 20 nations', nations.length === 20);
console.log(`  nation[0]: ${nations[0].name} (stature ${nations[0].stature})`);

const leagues = parse(goat.get_leagues(SEED, 0), 'get_leagues');
check('get_leagues returns 3 leagues', leagues.length === 3);
console.log(`  leagues: ${leagues.map((l) => `${l.name} (tier ${l.tier})`).join(', ')}`);

const clubs = parse(goat.get_clubs(SEED, leagues[0].id), 'get_clubs');
check('get_clubs returns 20 clubs', clubs.length === 20);
console.log(`  club[0]: ${clubs[0].name} (strength ${clubs[0].strength})`);

// ── New game (seed 42, ST, first nation, first league, first club) ──────────
let st = parse(goat.new_game(SEED, 'Smoke Test', 0, 0, 0, 0, 2026), 'new_game');
check('new_game starts season 1 round 0', st.season_number === 1 && st.season_round === 0);
console.log(`  ${st.player_name} @ ${st.club_name} — ${st.league_name} (${st.nation_name})`);

// ── Pre-season (Jul-1 anchor, 7-week lead): friendly + rest + train ─────────
check('new game opens in pre-season', st.pre_season === true && st.pre_season_week === 0);
{
  // League play is gated during pre-season.
  const early = JSON.parse(goat.play_match_start());
  check('league match blocked in pre-season', !!early.error);

  // A friendly plays a full beat match but never advances the league round.
  let beat = parse(goat.play_friendly_start(), 'play_friendly_start');
  console.log(`  friendly vs ${beat.opp_name}: ${beat.setup}`);
  let final = null;
  for (let i = 0; i < 64 && !final; i++) {
    const out = parse(goat.play_match_choice(0), 'play_match_choice (friendly)');
    if (out.is_complete) final = out.final;
    else beat = out.next_beat;
  }
  check('friendly completed', final !== null);
  console.log(`  FT: ${final.scoreline}  (${final.rating} · output ${final.output})`);
  const after = parse(goat.state(), 'state after friendly');
  check('friendly left the league round untouched', after.season_round === 0);
  check('still in pre-season after friendly', after.pre_season === true);

  const r = parse(goat.rest_week(), 'rest_week');
  check('rest week ticks one pre-season week', r.state.pre_season_week === 1);

  // Fast-forward is a competition-season tool — pre-season weeks tick one at a time.
  const ff = parse(goat.advance_weeks(2), 'advance_weeks (pre-season)');
  check(
    'advance_weeks defers during pre-season',
    ff.text.includes('Pre-season weeks tick one at a time'),
  );

  // Routine picker data + set (visible in every snapshot afterwards).
  const attrs = parse(goat.get_attrs(), 'get_attrs');
  check('get_attrs returns 30 attributes', attrs.length === 30);
  const sr = parse(goat.set_routine([2, 9, 12, 14, 0, 1], 2), 'set_routine');
  check(
    'routine set with 4-attr cap + intensity',
    sr.state.routine_text.includes('[High]') &&
      (sr.state.routine_text.match(/,/g) || []).length === 3,
  );
  console.log(`  routine: ${sr.state.routine_text}`);

  let t = r;
  while (t.state.pre_season) t = parse(goat.train(), 'train (pre-season)');
  check('pre-season ends after the 7-week lead', t.state.pre_season === false);
  console.log(`  ${t.text.split('\n').pop()}`);
}

// ── Round 1: interactive match (proves the beat loop end-to-end) ────────────
{
  const t = parse(goat.train(), 'train r1');
  check('train round 1 not already-trained', !t.text.includes('already trained'));
  let beat = parse(goat.play_match_start(), 'play_match_start');
  console.log(`  beat 1/${beat.total_beats} vs ${beat.opp_name}: ${beat.setup}`);
  let final = null;
  for (let i = 0; i < 64 && !final; i++) {
    const out = parse(goat.play_match_choice(0), 'play_match_choice');
    if (out.is_complete) final = out.final;
    else beat = out.next_beat;
  }
  check('interactive match completed', final !== null);
  console.log(`  FT: ${final.scoreline}  (${final.rating} · output ${final.output})`);
  console.log(`  PC: ${final.goals}g ${final.assists}a ${final.decisive} decisive ${final.clutch} clutch`);
}

// ── League table top-3 after round 1 ────────────────────────────────────────
st = parse(goat.state(), 'state after r1');
check('round 1 played', st.season_round === 1);
check('table has 20 rows', st.table.length === 20);
console.log('  Table top-3 after round 1:');
for (const row of st.table.slice(0, 3)) {
  console.log(
    `   ${row.position}. ${row.club}  Pld ${row.played} W${row.won} D${row.drawn} L${row.lost} GF${row.goals_for} GA${row.goals_against} Pts ${row.points}${row.is_player_club ? '  <- PC' : ''}`,
  );
}

// ── Auto-advance (in-season): ticks week-by-week in Rust until a match is due
{
  const aa = parse(goat.auto_advance(), 'auto_advance');
  check('auto-advance stops at a due match', aa.stop_reason === 'match_due');
  check(
    'auto-advance reports the next round',
    aa.text.includes('match due: Round 2/38'),
  );
  check('auto-advance does not play the round', aa.state.season_round === 1);
  console.log(`  ${aa.text.split('\n')[0]}`);
}

// ── Rest of the season: train() + skip_match() per round ────────────────────
let alreadyTrainedSeen = false;
while (!st.season_over) {
  const t = parse(goat.train(), 'train');
  if (t.text.includes('already trained')) alreadyTrainedSeen = true;
  const m = parse(goat.skip_match(), 'skip_match');
  st = m.state;
}
check('38 rounds played', st.season_round === 38);
check('season flagged over', st.season_over === true);
check('already-trained guard fired at least once', alreadyTrainedSeen);
console.log(
  `  Season 1 done: ${st.season_goals}g ${st.season_assists}a ${st.season_decisive} decisive ${st.season_clutch} clutch`,
);

// ── Season boundary ──────────────────────────────────────────────────────────
const se = parse(goat.season_end(), 'season_end');
console.log('  season_end:');
for (const line of se.text.split('\n')) console.log(`   ${line}`);

const ns = parse(goat.start_next_season(), 'start_next_season');
console.log('  start_next_season:');
for (const line of ns.text.split('\n')) console.log(`   ${line}`);
check('season 2 started', ns.state.season_number === 2 && ns.state.season_round === 0);
check('promotion events returned', Array.isArray(ns.events) && ns.events.length > 0);

// ── Save → load roundtrip ────────────────────────────────────────────────────
const before = parse(goat.state(), 'state before save');
const bytes = goat.save_game();
check('save_game returns bytes', bytes instanceof Uint8Array && bytes.length > 100);
console.log(`  save size: ${bytes.length} bytes`);
const loaded = parse(goat.load_game(bytes), 'load_game');
const fields = ['player_name', 'club_name', 'league_name', 'nation_name', 'season_number', 'season_round', 'week_label', 'pre_season', 'pre_season_week'];
const mismatch = fields.filter((f) => before[f] !== loaded[f]);
check(`save/load roundtrip preserves ${fields.join(', ')}`, mismatch.length === 0);
if (mismatch.length) console.log(`  mismatched: ${mismatch.join(', ')}`);
check('table survives roundtrip', before.table.length === loaded.table.length);

console.log(failures === 0 ? '\nSMOKE OK' : `\nSMOKE FAILED (${failures})`);
process.exit(failures === 0 ? 0 : 1);
