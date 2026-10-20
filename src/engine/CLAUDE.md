# engine/

- **`engine/mod.rs` (`Engine`)** — the story driver: session planning (`plan_session`/`pick`),
  calendar/chapter logic, and `step()`, which composes one frame. `tick()` is the separate 16 ms
  timer. `Env` bundles everything a TTM frame may touch.
- **`engine/ads.rs`** — ADS block-structured script VM plus the scene scheduler (`update`): every
  step it propagates tag state to scenes, re-runs every active tag's script from the start, then
  executes the current frame of every running scene in z-order and advances frames whose delay
  elapsed. A tag is finished when `tag_busy` is false (self-looping scenes don't block).
- **`engine/ttm.rs`** — scene records and `run_frame`. A frame's first execution has side effects
  (loads, sounds, random delays); later executions of the same frame only redraw (`Scene::redraw`).
- **`engine/island.rs`** — background composition and ambience (placement constraints, shore
  waves, whitecaps, clouds baked into the background, raft, holiday decorations).
  **`walk.rs`** — Johnny walking between six spots using EXE route tables.
  **`rng.rs`** — the original additive generator; use it (not another RNG) for any randomness in the engine.
