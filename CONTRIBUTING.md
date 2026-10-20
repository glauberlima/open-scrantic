# Contributing

Thanks for helping with Open Scrantic. This project reimplements *Screen Antics: Johnny Castaway* from the
disassembly of the original `SCRANTIC.SCR`, so changes are judged against the original's behavior.

Be kind: see the [Code of Conduct](CODE_OF_CONDUCT.md).

## Before you start

- Read [`docs/ENGINE.md`](docs/ENGINE.md) (recovered formats, VM semantics, story rules and the deliberate
  deviations from the original) and [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md); the same structure drawn
  is in [`docs/DIAGRAMS.md`](docs/DIAGRAMS.md). [`AGENTS.md`](AGENTS.md) has the commands and conventions.
- The disassembly is the ground truth. Other reimplementations (jc_reborn, castaway) are only hints.
- **Never commit screensaver data.** `SCRANTIC.SCR`, `RESOURCE.MAP` and `RESOURCE.001` are git-ignored. The
  program reads them from the directory of the executable at run time.

## Checks to run

Warnings fail the build (`Cargo.toml` denies them):

```sh
cargo clippy --all-targets
cargo fmt --check
OPEN_SCRANTIC_DATA=/path/to/install cargo test
```

The tests that need the real files pass without doing anything when `OPEN_SCRANTIC_DATA` is not set, so set it
to a directory with the three files before trusting a green run.

## Porting more logic

1. Find the behavior in the disassembly and record its address in a comment (`seg10:1acb`, `DS:1756`).
2. Keep the original's integer widths and wrapping, and its random number generator (`engine/rng.rs`).
3. Take time from `Engine::now` or the injected `Clock`, never from the system clock.
4. If you deviate on purpose, document it under *Deliberate differences* in `docs/ENGINE.md`.

## Checking a rendering change

`examples/snapshot.rs` runs the engine headless with a fixed clock and writes PNGs, which makes a change
reproducible:

```sh
cargo run --release --example snapshot -- INSTALL_DIR OUT_DIR [STEPS] [EVERY] [HHMM] [SEED] [STORY_DAY] [SCENE|list]
```

`DATE=MMDD` picks the calendar day (and so the holiday decoration), `DEDITHER=1` and `SIZE=WxH` mimic the
window. See `AGENTS.md` for details.

## Commits and pull requests

- Commits follow [Conventional Commits](https://www.conventionalcommits.org/) (`feat`, `fix`, `docs`, ...).
  The release is derived from them (git-cliff), so the type matters.
- Everything in the repo, code and comments and docs and commit messages, is en-US.
- Keep a pull request to one subject, and say how you checked it against the original.

## Reporting a bug

Open an issue with the form. It asks for `open-scrantic --version`, your operating system, the flags you
used, and, for a story or scene problem, the output of `OPEN_SCRANTIC_DEBUG=1 target/release/open-scrantic`.
Report security problems privately, as described in [`SECURITY.md`](SECURITY.md).
