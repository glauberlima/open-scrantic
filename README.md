<div align="center">

# 🏝️ Open Scrantic

### Johnny Castaway, the classic 1992 screensaver, running natively on Windows, macOS and Linux.

<img src="docs/images/open-scrantic-logo.png" alt="Open Scrantic — a castaway with a spyglass on a tropical island" width="240">

[![CI](https://img.shields.io/github/actions/workflow/status/glauberlima/open-scrantic/ci.yml?branch=main&style=flat-square&label=ci)](https://github.com/glauberlima/open-scrantic/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/glauberlima/open-scrantic?style=flat-square)](https://github.com/glauberlima/open-scrantic/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-green?style=flat-square)](LICENSE)

[▶ Watch a 30-second demo](docs/media/open-scrantic-demo.mp4)

</div>

Open Scrantic brings the original _Screen Antics: Johnny Castaway_ screensaver (Sierra/Dynamix, 1992)
back to modern computers: a free, open-source Rust reimplementation of the classic Windows 3.1
screensaver. Watch Johnny go about his days on his tiny island while a story unfolds from
one day to the next, with the same behavior as the original.

[Get started](#get-started) · [Settings](#settings) · [FAQ](#faq) · [Why](#why-this-exists) ·
[For developers](#for-developers) · [AI disclosure](#ai-disclosure) · [Legal](#legal-notice)

> [!IMPORTANT]
> You need your own copy of the original screensaver files. This project does not include them
> (see [What you need](#what-you-need)).

## Get started

1. **Download** the program for your system from the
   [latest release](https://github.com/glauberlima/open-scrantic/releases/latest):

   | System  | File                            |
   | ------- | ------------------------------- |
   | Windows | `open-scrantic-windows-x64.exe` |
   | macOS   | `open-scrantic-macos`           |
   | Linux   | `open-scrantic-linux-x64`       |

2. **Put it in the same folder** as the three original screensaver files (`SCRANTIC.SCR`, `RESOURCE.MAP`
   and `RESOURCE.001`).
3. **Run it.** It opens in a window; press **Esc** to exit. Start it with `--fullscreen` to fill the
   screen. **F12** saves a screenshot (`-raw` as drawn, `-view` as shown) next to the executable.

<details>
<summary>Your system refuses to open the file?</summary>

The program is not code-signed, so your system may warn about it.

- **Windows:** in the SmartScreen window, click **More info**, then **Run anyway**.
- **macOS:** open Terminal in the folder and run
  `chmod +x open-scrantic-macos && xattr -d com.apple.quarantine open-scrantic-macos`, then
  `./open-scrantic-macos`.
- **Linux:** run `chmod +x open-scrantic-linux-x64`, then `./open-scrantic-linux-x64`. If it complains
  about a missing sound library, install ALSA (`libasound2` on Debian and Ubuntu).

</details>

## What you need

The three files from the original _Screen Antics: Johnny Castaway_ installation (Windows 3.1), all in
one folder next to the program:

| File           | Size      | What it is                           |
| -------------- | --------- | ------------------------------------ |
| `SCRANTIC.SCR` | 295,952   | the original screensaver program     |
| `RESOURCE.MAP` | 1,461     | index of the screensaver's resources |
| `RESOURCE.001` | 1,175,645 | the images and animation scripts     |

<details>
<summary>Check that your files are the right ones</summary>

| File           | SHA-256                                                            |
| -------------- | ------------------------------------------------------------------ |
| `SCRANTIC.SCR` | `7811b526b36bba2f08326d7a680f2f063190b4278e5a36393fde5a02080b0262` |
| `RESOURCE.MAP` | `3d9ec330aab96bbe5a44ce34f5945703862e82b195088590b7adfef5d7345da7` |
| `RESOURCE.001` | `df9c2213f7c0abacf4e302cb53a476f9f220579c07ba350b167e351eed548eae` |

Run `shasum -a 256 SCRANTIC.SCR RESOURCE.MAP RESOURCE.001` (macOS) or `sha256sum` (Linux) and compare.
On Windows, use `certutil -hashfile <file> SHA256`.

</details>

## Settings

Options and story progress are stored in `SCRANTIC.INI`, next to the program. It is created on the
first run and rewritten when you exit normally, so edit it only while the program is closed. Keys are
case-insensitive.

**Options** (`0` = off, `1` = on; default `1`):

| Key          | Effect when off                   |
| ------------ | --------------------------------- |
| `Background` | no island or ocean background     |
| `Clouds`     | no clouds in the sky              |
| `Waves`      | no ocean whitecaps                |
| `Sounds`     | sound effects muted               |

Each option can also be set for one run with `--background=N`, `--clouds=N`, `--waves=N` or `--sounds=N`
(`N` is `0` or `1`). The command line wins over the file, and such a run never writes the value back to it.

To start over, delete `SCRANTIC.INI`. If it contains an invalid value, it is replaced by the
defaults, story progress included.

<details>
<summary>Story state (advanced)</summary>

Maintained by the program; edit only to force a chapter or time of day.

| Key                                         | Default | Meaning                                                         |
| ------------------------------------------- | ------- | --------------------------------------------------------------- |
| `NumDays`                                   | `0`     | story chapter of the day, 1–11; advances once per calendar day  |
| `Introduction`                              | `0`     | next chapter to show; the chapter plays when it is `<= NumDays` |
| `CurrentYear`, `CurrentMonth`, `CurrentDay` | `0`     | date of the last run, used to detect a new day                  |
| `StartTime`                                 | `900`   | start of the 16-half-hour day/night cycle, as `HHMM`            |

An out-of-range `NumDays` (below 2 or above 11) restarts the story at chapter 1 on the next day
change.

</details>

## FAQ

**Why do I need the original files?**
The art, animation scripts and sounds are copyrighted by their owners. This project reimplements
only the engine and reads those assets when it runs.

**Does it work with any build of the screensaver?**
No. The loader checks known strings in `SCRANTIC.SCR` and rejects other builds of the executable.

**Where is my progress saved?**
In `SCRANTIC.INI` (see [Settings](#settings)).

## Why this exists

Because it's cool, and because it's nostalgic. Johnny Castaway takes me back to my childhood, to
simpler times, and I wanted it to keep running on the computers we use today, for me and for anyone
else who remembers it.

## About the project

Open Scrantic is a reverse-engineered reimplementation of _Screen Antics: Johnny
Castaway_ (Sierra/Dynamix, 1992), rebuilt from the disassembly of the original Windows 3.1
screensaver. The program contains only the engine logic.

```text
SCRANTIC.SCR ──► assets::exe       ──► sounds, scene catalog, walk routes, RNG seed
RESOURCE.MAP ─┐
RESOURCE.001 ─┴► assets::resource  ──► images, palette, TTM/ADS scripts
                                              │
                    engine: story planner ──► ADS scheduler ──► TTM frames ──► canvas
                    (calendar, chapters)      (z-order, tags)   (draw, sound)    │
                                                                      winit + softbuffer + rodio
```

The disassembly is the ground truth for behavior, including the original 16-bit arithmetic and random
number generator. Recovered formats, VM semantics, story rules, timing and the deliberate deviations
(for example, full-frame redraw instead of dirty rectangles) are recorded in
[`docs/ENGINE.md`](docs/ENGINE.md). The original settings dialog is not reimplemented; options live
in [`SCRANTIC.INI`](#settings).

## For developers

### Build from source

Requires Rust 1.99.1 (pinned in `rust-toolchain.toml`). On Linux, also install the ALSA headers and
`pkg-config` (`libasound2-dev pkg-config` on Debian and Ubuntu).

```sh
cargo build --release
cp /path/to/install/{SCRANTIC.SCR,RESOURCE.MAP,RESOURCE.001} target/release/
target/release/open-scrantic
```

The screensaver files and `SCRANTIC.INI` are always read from the directory of the executable.

| Option          | Effect                                                       |
| --------------- | ------------------------------------------------------------ |
| `--fullscreen`  | start fullscreen instead of windowed                         |
| `--dedither`    | blend the 1992 checkerboard dithering into solid colors      |
| `--soft-audio`  | improve the sounds: resample, hide the 8-bit hiss, fade cuts |
| `--background=N`, `--clouds=N`, `--waves=N`, `--sounds=N` | override that INI option for this run (`0` or `1`) |
| `-V, --version` | print the version and exit                                   |
| `-h, --help`    | print the usage and exit                                     |

The release profile is optimized and portable (fat LTO, a single codegen unit, `panic = "abort"`,
stripped symbols). For a binary tuned to the build machine only (it may not run on older CPUs):

```sh
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

### Development

```sh
OPEN_SCRANTIC_DEBUG=1 target/release/open-scrantic              # prints each session's plan
cargo run --release --example snapshot -- /path/to/install /tmp/out 2000 100 1000 42   # headless PNG render
```

Architecture (the full write-up, with the data flow and how a frame is produced, is in
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)):

- `src/assets/` — reading the original files: the Dynamix resource archive (RLE/LZW, BMP, SCR,
  PAL, TTM, ADS) and the NE executable (`WAVESFXn` sounds and data-segment tables).
- `src/engine/` — the engine: TTM VM (`ttm.rs`), ADS VM and scene scheduler (`ads.rs`), island
  and ambient animation (`island.rs`), walking (`walk.rs`), story planning (`mod.rs`), the original
  random number generator (`rng.rs`), drawing primitives (`gfx.rs`), INI settings (`config.rs`).
- `src/main.rs` — window (winit + softbuffer), clocks and input; `src/present.rs` — frame to window
  pixels (dedithering, scaling); `src/audio.rs` — sound (rodio).

See [`AGENTS.md`](AGENTS.md) for the full architecture notes and conventions.

### Contributing

Run these before sending changes; warnings fail the build (`Cargo.toml` denies them):

```sh
cargo clippy --all-targets
cargo fmt --check
OPEN_SCRANTIC_DATA=/path/to/install cargo test
```

Commits follow [Conventional Commits](https://www.conventionalcommits.org/). Never commit screensaver
data. When porting more logic, cite the original address in the comment (`seg10:1acb`, `DS:1756`),
as existing code does.

### Releases

CI (format, clippy, release build on Linux, macOS and Windows) runs on every pull request and push to
`main`. A release is cut manually from the **Release** workflow (Actions tab): it derives the next
version from the Conventional Commits since the last tag (git-cliff), builds the three binaries,
scans them with VirusTotal, commits the updated `CHANGELOG.md`, tags the version and publishes the
GitHub release with the binaries and their SHA-256 checksums. If no commit since the last tag
deserves a release (`feat`, `fix`, `perf`, `refactor`, `docs`), nothing is published.

## AI disclosure

Generative AI was used throughout the development of Open Scrantic. The project was built with
[Claude Code](https://claude.com/claude-code) (Anthropic) together with
[REA: Reverse Engineer Anything](https://rea.tools/), a reverse-engineering toolkit
exposed to the model. Together they were used to disassemble and decompile the original
`SCRANTIC.SCR`, recover the resource formats and script semantics, and write the Rust code, tests and
documentation. The developer directed the work and reviewed and integrated the AI-assisted results,
checking the behavior against the original screensaver.

## Legal notice

_Screen Antics: Johnny Castaway_ is a trademark and copyright of its respective owners (Sierra /
Dynamix). This is an independent, non-commercial interoperability project, not affiliated with or
endorsed by them. It does not include or distribute any original screensaver data.

## License

Source code under the [MIT License](LICENSE).

<div align="center">

Made in Campinas, Brazil 🇧🇷 with ❤️, [Claude Code](https://claude.com/claude-code) and
[REA](https://rea.tools/)

</div>
