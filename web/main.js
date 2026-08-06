// GOAT web demo — vanilla ES module driving the goat-web wasm build.
import init, * as goat from './pkg/goat_web.js';

const $ = (id) => document.getElementById(id);
const parse = (json) => {
  const v = JSON.parse(json);
  if (v && v.error) throw new Error(v.error);
  return v;
};

function log(msg) {
  $('log').textContent = msg + '\n\n' + $('log').textContent;
}

function b64encode(bytes) {
  let bin = '';
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin);
}
function b64decode(b64) {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

function renderState(s) {
  $('status').textContent =
    `${s.player_name} — ${s.club_name} (${s.league_name}, ${s.nation_name})\n` +
    `Age ${s.age_years} · Energy ${s.energy}% · Form ${s.form}\n` +
    `Season ${s.season_number} · Round ${s.season_round}/${s.rounds_per_season} · ${s.week_label}\n` +
    `Season: ${s.season_goals}g ${s.season_assists}a · ${s.season_decisive} decisive · ${s.season_clutch} clutch` +
    (s.trained_this_week ? ' · trained ✓' : '') +
    (s.season_over ? '\nSEASON OVER — use Season End / Next Season.' : '');
  const tbody = $('table').querySelector('tbody');
  tbody.innerHTML = '';
  for (const r of s.table) {
    const tr = document.createElement('tr');
    if (r.is_player_club) tr.className = 'pc';
    for (const v of [r.position, r.club, r.played, r.won, r.drawn, r.lost, r.goals_for, r.goals_against, r.points]) {
      const td = document.createElement('td');
      td.textContent = v;
      tr.appendChild(td);
    }
    tbody.appendChild(tr);
  }
}

function showBeat(b) {
  $('match').classList.remove('hidden');
  $('beat').textContent =
    `vs ${b.opp_name} — ${b.goals_for}–${b.goals_against} · ${b.minute}' · beat ${b.beat_number}/${b.total_beats}\n\n${b.setup}`;
  const box = $('choices');
  box.innerHTML = '';
  for (const c of b.choices) {
    const btn = document.createElement('button');
    btn.textContent = c.text;
    btn.onclick = () => choose(c.index);
    box.appendChild(btn);
  }
}

function choose(idx) {
  const out = parse(goat.play_match_choice(idx));
  if (out.is_complete) {
    const f = out.final;
    $('match').classList.add('hidden');
    log(`FT: ${f.scoreline}  (${f.rating} · output ${f.output})\n` +
        `You: ${f.goals}g ${f.assists}a · ${f.decisive} decisive · ${f.clutch} clutch\n` +
        f.moments.join('\n'));
    renderState(out.state);
  } else {
    showBeat(out.next_beat);
    $('beat').textContent =
      `${out.success ? '✓' : '✗'} ${out.outcome_text}\n\n` + $('beat').textContent;
  }
}

async function main() {
  await init();

  // ── Setup screen ──────────────────────────────────────────────────────────
  // Seed is never player-facing: rolled randomly per page load. `?seed=` is a
  // dev-only override for reproducible testing, not surfaced in the UI.
  const devSeed = new URLSearchParams(location.search).get('seed');
  $('seed').value = devSeed || BigInt(Math.floor(Math.random() * 2 ** 32)).toString();
  const seed = () => BigInt($('seed').value);
  const nations = parse(goat.get_nations());
  const nationSel = $('nation');
  nationSel.innerHTML = '';
  for (const n of nations) {
    const o = document.createElement('option');
    o.value = n.id;
    o.textContent = `${n.name} (stature ${n.stature})`;
    nationSel.appendChild(o);
  }

  function fillLeagues() {
    const leagues = parse(goat.get_leagues(seed(), Number(nationSel.value)));
    const leagueSel = $('league');
    leagueSel.innerHTML = '';
    for (const l of leagues) {
      const o = document.createElement('option');
      o.value = l.id;
      o.textContent = `${l.name} (tier ${l.tier})`;
      leagueSel.appendChild(o);
    }
    fillClubs();
  }
  function fillClubs() {
    const clubs = parse(goat.get_clubs(seed(), Number($('league').value)));
    const clubSel = $('club');
    clubSel.innerHTML = '';
    clubs.forEach((c, i) => {
      const o = document.createElement('option');
      o.value = i; // club_idx within the league
      o.textContent = `${c.name} (strength ${c.strength})`;
      clubSel.appendChild(o);
    });
  }
  nationSel.onchange = fillLeagues;
  $('league').onchange = fillClubs;
  fillLeagues();

  $('start').onclick = () => {
    try {
      const s = parse(goat.new_game(
        seed(),
        $('pname').value,
        Number($('position').value),
        Number(nationSel.value),
        $('league').selectedIndex,
        Number($('club').value),
      ));
      $('setup').classList.add('hidden');
      $('game').classList.remove('hidden');
      renderState(s);
      log(`Career started: ${s.player_name} at ${s.club_name}.`);
    } catch (e) {
      $('setup-log').classList.remove('hidden');
      $('setup-log').textContent = String(e);
    }
  };

  // ── Game screen ───────────────────────────────────────────────────────────
  $('train').onclick = () => {
    const r = parse(goat.train());
    log(r.text);
    renderState(r.state);
  };
  $('skip').onclick = () => {
    const r = parse(goat.skip_match());
    log(r.text);
    renderState(r.state);
  };
  $('play').onclick = () => {
    try {
      showBeat(parse(goat.play_match_start()));
    } catch (e) {
      log(String(e));
    }
  };
  $('season-end').onclick = () => {
    try {
      const r = parse(goat.season_end());
      log(r.text);
      renderState(r.state);
    } catch (e) {
      log(String(e));
    }
  };
  $('next-season').onclick = () => {
    try {
      const r = parse(goat.start_next_season());
      log(r.text);
      renderState(r.state);
    } catch (e) {
      log(String(e));
    }
  };
  $('save').onclick = () => {
    const bytes = goat.save_game();
    if (!bytes.length) return log('Nothing to save.');
    localStorage.setItem('goat-save', b64encode(bytes));
    log(`Saved ${bytes.length} bytes to localStorage.`);
  };
  $('load').onclick = () => {
    const b64 = localStorage.getItem('goat-save');
    if (!b64) return log('No save in localStorage.');
    try {
      const s = parse(goat.load_game(b64decode(b64)));
      $('setup').classList.add('hidden');
      $('game').classList.remove('hidden');
      renderState(s);
      log('Save loaded.');
    } catch (e) {
      log(String(e));
    }
  };
}

main().catch((e) => {
  document.body.insertAdjacentHTML('beforeend', `<pre>init failed: ${e}</pre>`);
});
