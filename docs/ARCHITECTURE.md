# Architecture

How Open Scrantic is put together: which module does what, how data flows from the original
files to the screen, and how one frame is produced. For the recovered file formats, VM opcodes,
story rules and deliberate deviations, see [`ENGINE.md`](ENGINE.md); for contributor conventions,
see [`../AGENTS.md`](../AGENTS.md). The same structure drawn as flowcharts and sequence diagrams is in
[`DIAGRAMS.md`](DIAGRAMS.md).

## Principles

- **The disassembly is the ground truth.** Behavior is ported from `SCRANTIC.SCR`, not guessed or
  copied from other reimplementations. Code comments cite original addresses (`seg10:1acb`,
  `DS:1756`).
- **No original data in the binary.** Art, scripts, sounds and every logic table are read at runtime
  from the user's original `SCRANTIC.SCR`, `RESOURCE.MAP` and `RESOURCE.001`. Offsets into the
  executable are validated against known strings, so other builds are rejected.
- **Original arithmetic.** Wrapping `u16`/`i16` math and the original random number generator
  (`engine/rng.rs`) are kept so that sequences match. Do not use another RNG.
- **Untrusted input is an error, not a crash.** The original files come from the user's disk and may
  be truncated or from another build: every read in `assets/` is bounds-checked and returns
  `AssetError`, and decompressed sizes are capped.
- **Deterministic and headless-testable.** The wall clock is injected (`Clock`), time is counted in
  ticks, and the engine never touches the window or the audio device, so it can run without either.

## Data flow

```text
SCRANTIC.SCR ──► assets::exe       ──► sounds, scene catalog, walk routes, RNG seed
RESOURCE.MAP ─┐
RESOURCE.001 ─┴► assets::resource  ──► images, palette, TTM/ADS scripts
                                              │
                    engine: story planner ──► ADS scheduler ──► TTM frames ──► canvas
                    (calendar, chapters)      (z-order, tags)   (draw, sound)    │
                                                                      winit + softbuffer + rodio
```

Three layers, with dependencies pointing downward only:

| Layer     | Modules                                             | Knows about                          |
|-----------|-----------------------------------------------------|--------------------------------------|
| Front end | `main.rs`, `present.rs`, `audio.rs`                 | window, input, sound device, clocks  |
| Engine    | `engine/*`                                          | scripts, story, drawing; no I/O      |
| Assets    | `assets/*`                                          | file formats; no engine state        |

## Module map

| Module                 | Role                                                                                            | Original       |
|------------------------|-------------------------------------------------------------------------------------------------|----------------|
| `assets/mod.rs`        | `ScreensaverData::load`: finds the three files (case-insensitive), builds `Archive` + `ExeData` | —              |
| `assets/exe.rs`        | NE executable parser: `WAVESFXn` sounds, scene catalog, walk tables, RNG seed, whitecaps        | data segment   |
| `assets/resource.rs`   | Dynamix archive: RLE/LZW, 4 bpp BMP/SCR, PAL, TTM (split into frames), ADS                      | —              |
| `engine/mod.rs`        | `Engine`: session planning, calendar/chapters, `step()` and `tick()`                            | `seg4`, `seg2` |
| `engine/ads.rs`        | ADS script VM and the scene scheduler (`AdsRuntime::update`)                                    | `seg10`        |
| `engine/ttm.rs`        | `Scene` records and the frame interpreter `run_frame`                                           | `seg12`        |
| `engine/island.rs`     | Island placement, background composition, waves, whitecaps, clouds, raft, holidays              | `seg3`         |
| `engine/walk.rs`       | Johnny walking between the six island spots                                                     | `seg4`         |
| `engine/gfx.rs`        | 640×480 palette-indexed `Canvas` with the GDI-like primitives the original used                 | GDI calls      |
| `engine/rng.rs`        | Additive lagged generator seeded from the executable's table                                    | `seg4:1e86`    |
| `engine/config.rs`     | `SCRANTIC.INI` read/write with the original keys                                                | —              |
| `present.rs`           | Frame to window pixels: palette lookup, optional dedithering, scaling (pure functions)          | —              |
| `audio.rs`             | One-channel sound playback with `sndPlaySound(SND_ASYNC)` semantics (rodio); optional soft mode | —              |
| `main.rs`              | Window (winit), presenting (softbuffer), clocks, input, saving settings on exit                 | —              |
| `examples/snapshot.rs` | Headless run with a fixed clock that writes PNGs                                                | —              |

## Startup

1. `main` parses the arguments (`--fullscreen`, `--version`, `--help`) and resolves `app_dir()`,
   the directory of the executable. Screensaver files and `SCRANTIC.INI` are always read from there.
2. `ScreensaverData::load` reads the three files, rejects the installer placeholder (a tiny
   `RESOURCE.001`) and builds the `Archive` and `ExeData`. `ExeData::parse` fails on an
   unsupported build.
3. `Config::load` reads `SCRANTIC.INI`; a missing or invalid file is replaced by defaults.
4. `Engine::new` loads the palette (`JOHNCAST.PAL`), Johnny's sprites (`JOHNWALK.BMP`) and the
   island, seeds the RNG (warmed up with the Unix time, as the original does with `time()`), and
   puts the title screen (`INTRO.SCR`) in the front canvas.
5. `Audio::new` opens the default output device; without one the program runs silently.
6. The winit event loop starts. The window is created on `resumed`; the first story step is held
   back by `INTRO_HOLD` (2 s) so the title screen is visible.

## Time: two emulated Windows 3.1 clocks

The original ran two timers, and both are reproduced:

- **Tick, 16 ms** (`TICK_MS`, seg2:16ce): drives script delays, the walk animation, the idle
  pause and the wave/whitecap timers. `Engine::tick` increments `Engine::now` (a wrapping `u32`)
  and decrements those timers.
- **Step, 55 ms** (`STEP_MS`): the story and scheduler step, which composes one frame.

`App::advance` in `main.rs` derives both from wall time with one formula: for every due step it
first runs all ticks up to that step's virtual time, then `Engine::step`. After a long stall
(sleep, debugger) it resumes instead of fast-forwarding. The event loop sleeps with
`ControlFlow::WaitUntil` until the next step. Tests and the snapshot example drive the engine with
the same formula, which is why a run with a fixed clock and seed is reproducible.

## Producing a frame: `Engine::step`

There are three 640×480 canvases of palette indices:

| Canvas  | Content                                                                           |
|---------|-----------------------------------------------------------------------------------|
| `bg`    | persistent background: ocean/night, island, clouds baked in                       |
| `work`  | scratch frame, rebuilt from `bg` on every step                                    |
| `front` | the last presented frame                                                          |

This full recomposition replaces the original's dirty rectangles across three buffers (a
documented deviation). TTM opcode `0x4204` and `LOAD_SCREEN` write into `bg`.

Each step does the following, in order:

1. If the session's plan is exhausted, start a new session (see Story below).
2. Copy `bg` into `work`.
3. `Island::step`: whitecaps, shore waves and the raft, unless the scene has no island or frozen
   waves.
4. Run the current plan entry, which is one of three things:
   - **Walking to the scene's start spot**, when Johnny is not there yet (`Walk::go_to` /
     `Walk::step`).
   - **A standing pause** (ADS id `0xFF`): Johnny idles while a 180–419 tick timer runs.
   - **An ADS scene**: on its first step the ADS script is loaded (`load_ads`), its tag started and
     the TTM drawing offset set; then every step runs `AdsRuntime::update` and checks
     `tag_busy`.
5. When the entry finishes, Johnny's spot and heading become the scene's end values, `plan_idx`
   advances, and story bookkeeping runs.
6. Draw the holiday decoration, queue the chapter chime if the story scene just started.
7. Swap `front` and `work`.

`Engine::frame()` returns the front canvas. `main.rs` converts it to RGB through `Engine::palette`
(`present::plain`, or `present::dedither` with `--dedither`, which replaces each 2×2 checkerboard by
the average of its two colors) and `present::scale` draws it into the window: it scales by
`min(width/640, height/480)` with sharp-bilinear sampling (pixels stay solid; a pixel blends with its
neighbor only within one screen pixel of the edge, so an integer factor is exact replication) and centers it
with black bars. The
initial window is the largest whole multiple of 640×480 that fits in 80% of the screen.

## Story: sessions and plans (`engine/mod.rs`)

A **session** is a list of catalog scene indices (`plan`) that Johnny plays through, then a new
session is planned. `new_session` computes the **day phase** (16 half-hours counted from
`StartTime`: 12 of day, 4 of night), the **holiday** from the date, resets the island
configuration, applies the calendar (`update_day` advances the story chapter once per calendar
day) and calls `plan_session`.

`plan_session` spends a time budget (`PLAN_BUDGET`, 300 duration units):

1. With 10% chance, pick the **chapter-of-the-day scene** (`pick` mask `0x80`).
2. Pick a **final** scene (mask `4`) that is not the last final one.
3. Pick an **opening** scene (mask `1`) when the rules call for one.
4. Fill the remaining budget with **middle** scenes (mask `2`) drawn by weight, repeating the
   repeatable ones.
5. Append the final scene.

`pick` filters the 80-entry catalog by role, day phase, story day and compatibility with the
island layout already decided; every chosen scene calls `Island::apply_flags`, so later picks must
be compatible (`Island::compatible`). `pending[i]` marks scenes whose start has not run yet. A
small state machine (`story_state`) plays the chime when the chapter scene starts and advances the
saved chapter (`Introduction` in the INI) once it completes.

## Scheduling and scripts (`engine/ads.rs`, `engine/ttm.rs`)

Two script languages, one on top of the other:

- **TTM** scripts animate: each file is split into *frames* at `UPDATE` opcodes, and each tag
  (`0x1111`) defines a **scene** starting at a frame.
- **ADS** scripts sequence: they start, stop and combine TTM scenes with conditions, random
  branches and loops.

`AdsRuntime::update` is the scheduler tick (seg10:1acb), in three phases:

1. **Propagate** each tag's state to its scenes (a restarted tag resets them).
2. **Re-run the script of every active tag** from its re-entry point. This is how the original
   works: the whole script is re-evaluated each tick, and block conditions decide what happens.
3. **Execute the current frame of every running scene**, in z-order, through `ttm::run_frame`, and
   advance frames whose delay has elapsed (`Scene::advance`).

`run_frame` interprets one frame's opcodes against an `Env`, the bundle of everything a frame may
touch: the three canvases, the archive, Johnny's sprites, the RNG, the sound queue, the island
offset and the tick counter. The first execution of a frame has side effects (loading images and
screens, sounds, random delays); later executions of the same frame only redraw (`Scene::redraw`).
It returns a `FrameResult` with an optional new delay, a purge flag (end of the scene's frames) and
an optional GOTO. A tag is finished when `tag_busy` is false; scenes that GOTO their own frame are
infinite loops and do not block.

TTM drawing, clip and copy coordinates receive the island offset from `Island::ttm_offset` (island
origin x + 1, plus 272 px for scenes with flag `0x400`). Walk frames use the raw island origin.

## Island and walking

`Island` owns everything ambient: it places the island and decorations within constraints, composes
the background into `bg` at the start of a session (`init`), animates shore waves and ocean
whitecaps every step, bakes clouds into the background, draws the raft (five growth stages by
story day) and the holiday decoration, and redraws the palm trunk in front of Johnny when he walks
behind it (`trunk_overlay`).

`Walk` moves Johnny between six spots using the executable's route table, which holds alternative
paths chosen by percentage and the frame paths of `(sprite, mirror, x, y)`. It turns the shortest
way, picking randomly at 180°, and advances one frame every 4 ticks.

## Sound

Scenes request sounds by `WAVESFXn` number into `Env::sounds`. `Engine::take_sounds` hands the
queue to `main.rs` after each step, which plays them through `Audio` unless `Sounds=0`. Samples
are decoded once at load into `f32` (8-bit, 11,025 Hz, mono).

By default `Audio` plays the samples as they are and dropping the previous `Player` cuts the
previous sound, as `SND_ASYNC` does. With `--soft-audio` (not in the original) each sound is
resampled on first use to the device rate with a low-passed Lanczos-windowed sinc and cached, one
endless mono `Stream` feeds the mixer, and `Audio::play` fades the sound it cuts over 3 ms.

## Persistence

`Config` maps `[ScreenSaver.ScreenAntics]` in `SCRANTIC.INI` to fields: four options
(`Background`, `Clouds`, `Waves`, `Sounds`) and the story state (`NumDays`, `Introduction`, the date
of the last run, `StartTime`). Parsing is strict: any invalid value replaces the whole file with the
defaults. The file is written once, on clean exit (`ApplicationHandler::exiting`). The command-line options
`--background`, `--clouds`, `--waves` and `--sounds` (`0`/`1`) override the four options for one run:
`main.rs` keeps the settings as loaded (`App::saved`) and writes those four back, so an override is never
persisted.

## Testing and debugging

- **Unit tests** cover pure parts (INI parsing, RNG). The tests that need the real files run only
  when `OPEN_SCRANTIC_DATA` points to an install directory, and otherwise pass without doing
  anything. They load every ADS script, run the engine headless until scenes complete, and check the
  day-phase formula against the original.
- **`examples/snapshot.rs`** runs the engine headless with a fixed clock and seed and writes PNGs.
  It can preset the story day, the time of day and a single scene, which makes it the way to verify
  rendering changes.
- **`OPEN_SCRANTIC_DEBUG=1`** prints each session's plan and every finished scene.

## Porting more logic

1. Find the behavior in the disassembly and record its address in a comment.
2. Keep the original's integer widths and wrapping; do not use another RNG.
3. Take time from `Engine::now` / the injected `Clock`, never from the system clock.
4. If you deviate on purpose, document it under *Deliberate differences* in
   [`ENGINE.md`](ENGINE.md).
5. Check the result with the snapshot example, with `cargo clippy --all-targets` at zero warnings.
