# AGENTS.md

Guidance for AI coding agents (and human contributors) working in this repository.

## What this is

A reverse-engineered Rust reimplementation of *Screen Antics: Johnny Castaway* (Sierra/Dynamix, 1992, Win16).
The binary holds only the engine logic. At runtime it reads the **original installed screensaver files**
from a directory: `SCRANTIC.SCR` (the original NE executable), `RESOURCE.MAP` and `RESOURCE.001`.
Never commit screensaver data (the three files are git-ignored).

Behavior was recovered by disassembling `SCRANTIC.SCR`; the disassembly is the ground truth, and
third-party reimplementations (jc_reborn, castaway) are only hints. `docs/ENGINE.md` records the
recovered formats, VM semantics, story rules, timing and the deliberate deviations from the
original — read it before changing engine behavior. Code comments cite original addresses
(`seg10:1acb`, `DS:1756`); keep that convention when porting more logic.

## Commands

```sh
cargo build --release                                   # optimized binary: target/release/open-scrantic
cp SCRANTIC.SCR RESOURCE.MAP RESOURCE.001 target/release/ && target/release/open-scrantic [--fullscreen] [--dedither] [--soft-audio]   # screensaver files + SCRANTIC.INI live next to the binary; Esc exits
cargo clippy --all-targets                              # must stay at zero warnings
cargo fmt --check                                       # max_width = 120
OPEN_SCRANTIC_DATA=/path/to/install cargo test          # data-dependent tests silently pass without it
OPEN_SCRANTIC_DATA=/path/to/install cargo test headless_run_completes_scenes   # single test
OPEN_SCRANTIC_DEBUG=1 target/release/open-scrantic              # prints each session's plan and scene completions
cargo run --release --example snapshot -- INSTALL_DIR OUT_DIR [STEPS] [EVERY] [HHMM] [SEED] [STORY_DAY] [SCENE|list]
```

The `snapshot` example runs the engine headless with a fixed clock and writes PNGs; it is the way
to verify rendering changes (e.g. `STORY_DAY` presets a story chapter, `HHMM` picks day/night). `SCENE` forces one catalog scene as the whole session (`list` prints the
catalog with indices); story scenes have `day` > 0 and some need a late `HHMM` (their phase range).
`DEDITHER=1` applies `--dedither` to the PNGs and `SIZE=WxH` scales them as a window of that size would.

`clippy::pedantic` is deliberately not enabled: many `as` casts intentionally
reproduce the original 16-bit arithmetic (wrapping `u16`/`i16`, unsigned `rand() % n`); do not
"fix" them without checking the original semantics.

## Development notes

- macOS: if sound keeps playing with no `open-scrantic` process running, the host of a `.saver` preview may still
  be alive: `legacyScreenSaver` (`pgrep -fl legacyScreenSaver`). `pkill -f legacyScreenSaver` is safe in
  development; macOS starts it again on demand and nothing is lost.

## Architecture

A longer write-up (startup, clocks, frame pipeline, story planning, scheduler) is in
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md); the summary below is what to know before editing.

Crate is a library (`src/lib.rs`) plus the fullscreen binary (`src/main.rs`) and the `snapshot` example.
`present.rs` (frame to window pixels, optional dedithering) and `audio.rs` (rodio, optional soft mode)
are presentation only: neither is in the original and both default to its behavior.

Per-module notes: [src/assets/CLAUDE.md](src/assets/CLAUDE.md) and [src/engine/CLAUDE.md](src/engine/CLAUDE.md).

Key cross-cutting model:
- **Rendering**: three canvases — `bg` (persistent background), `work` (rebuilt from `bg` every
  step, then island ambience, scenes, walk and holiday drawn on top) and `front` (last presented
  frame). This full-redraw replaces the original's dirty-rectangle buffers (documented deviation).
  TTM `0x4204` and `LOAD_SCREEN` write into `bg`.
- **Timing**: two emulated Windows 3.1 clocks — `tick()` every 16 ms (script delays, walk/idle/wave
  timers) and `step()` every 55 ms (`STEP_MS`). `main.rs` derives both from wall time; tests and the
  snapshot example drive them deterministically with the same formula.
- **Coordinates**: TTM drawing gets the island offset from `Island::ttm_offset` (origin x + 1, plus
  272 px for scenes with flag `0x400`); walk frames use the raw island origin.
- **Persistence**: `engine/config.rs` reads/writes `SCRANTIC.INI` with the original keys next to the
  executable (`app_dir()`); a missing or invalid file is recreated with defaults; saved on clean exit only.

## Conventions

- Everything in the repo (code, comments, docs, commit messages) is en-US.
- Commits follow Conventional Commits.
