# GOAT — web demo

Minimal browser front-end for the deterministic football life-sim, driven by
the `goat-web` crate (`crates/goat-web`) compiled to WebAssembly via
wasm-bindgen. All game logic lives in the Rust core crates; this page is a
thin UI over the JSON-string session API.

## Build

From the repo root:

```sh
wasm-pack build crates/goat-web --target web --out-dir ../../web/pkg
```

The output in `web/pkg/` is git-ignored (as is `web/pkg-node/`, used by the
node smoke test).

## Serve

wasm ES modules don't load over `file://`, so serve the directory:

```sh
cd web && python3 -m http.server 8000
```

Then open <http://localhost:8000>.

## Play

1. Pick a seed, player name, position, then nation → league → club.
2. **Start career** begins season 1.
3. Each round: **Train** (once per week), then **Play Match** (interactive,
   beat-by-beat choices) or **Skip Match** (auto-sim).
4. After 38 rounds: **Season End**, then **Next Season** (resolves
   promotion/relegation for your nation).
5. **Save** stores the career in `localStorage` (base64); **Load** restores it.

## Node smoke test

A headless end-to-end check (no browser needed):

```sh
wasm-pack build crates/goat-web --target nodejs --out-dir ../../web/pkg-node
cd web && node smoke.mjs
```

It creates a game at seed 42, plays one interactive match, sims the rest of
the 38-round season, crosses the season boundary (season end + promotion), and
verifies a save→load roundtrip. See `TEST-LOG.md` for the last captured run.
