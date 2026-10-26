# open-scrantic — architecture for security audit run 1

## Product, principals, resources

open-scrantic is a Rust (edition 2024) reimplementation of the 1992 Win16 screensaver *Screen Antics: Johnny Castaway*. It ships as a desktop binary (Linux/macOS/Windows, `src/main.rs` + `src/cli.rs`) and as a Windows `.scr` (`--features screensaver`, `src/saver.rs`, `src/windows_ui.rs`). The program contains only engine logic; at runtime it reads three **third-party data files** the user obtains separately (in practice from abandonware archives or old CDs): `SCRANTIC.SCR` (Win16 NE executable), `RESOURCE.MAP`, `RESOURCE.001`, plus its own `SCRANTIC.INI`.

Principals:
- **Data-file supplier (lower trust)**: whoever authored or redistributes the three original files. Fully controls their bytes. `SECURITY.md` explicitly states "a crash or excessive memory use triggered by a malformed `SCRANTIC.SCR`, `RESOURCE.MAP`, `RESOURCE.001` or `SCRANTIC.INI`" is in scope.
- **Local user / victim**: runs the binary or has Windows launch the `.scr` automatically on idle (`/s`), as preview (`/p HWND`) in the Screen Saver Settings dialog, or configure (`/c`).
- **Repository contributors / PR authors (lower trust)** vs **maintainer + release workflow** (contents: write, VirusTotal API secret).

Protected resources: integrity and availability of the user's process and host (memory, CPU, the unattended screensaver session, files written in the settings directory), the release artifacts and `main` branch, the `VIRUSTOTAL_API_KEY` secret.

No network code, no IPC servers, no authentication. `unsafe` is forbidden crate-wide except `src/windows_ui.rs`.

## Comparable baseline

Comparable reimplementations named in the docs (jc_reborn, castaway) parse the same Dynamix formats; they are hints only. Parsers of untrusted legacy game data (ScummVM-like engines) conventionally treat malformed data as a robustness/security boundary — consistent with this project's own `SECURITY.md` and its tests `corrupt_input_is_an_error_not_a_panic` / `truncated_installs_are_errors_not_panics` (`src/assets/mod.rs:172-207`).

## Stack and build limits

Rust 1.99.1 pinned (`rust-toolchain.toml`); local toolchain is 1.97.0. Deps: chrono, rodio (cpal), softbuffer, winit, windows-sys (Windows), embed-resource (Windows build). Release profile: `panic = "abort"`, overflow checks off. Dependencies are not cached locally; fetching is prohibited, so the full binary is not built. A harness rlib (`<output-dir>/build/libscrantic_harness.rlib`) compiles `src/assets/{mod,exe,resource}.rs` and `src/engine/{ads,gfx,rng,ttm}.rs` unchanged, plus a verbatim copy of `Env`. Sandbox runner: `<output-dir>/bin/sandbox-run.sh <scratch> <secs> <cmd...>`.

## Entry surfaces and source-to-sink paths

1. **Data files** — `src/lib.rs:data_dirs/settings_dir` → `src/assets/mod.rs:read_ci/locate/load` (whole file `fs::read`, no size cap).
   - `RESOURCE.MAP`/`.001` → `Archive::new` (`resource.rs:15`) copies every entry → `chunks` → `bmp/scr/palette/ttm/ads` → `decompress` (`rle`, `lzw`, cap `MAX_UNPACKED` 16 MiB) → `unpack_4bpp` → `Ttm::parse` / `Ads`.
   - `SCRANTIC.SCR` → `ExeData::parse` (`exe.rs:422`) → `NeFile::segment`, scene catalog, walk paths, routes, RNG table, `wave_resources` → `decode_wav` → `audio.rs:resample` (Soft mode, always on in `.scr`).
   - Bytecode execution: ADS VM (`engine/ads.rs`: recursive `run_block`/`exec`/`skip_block`/`eval_cond`/`random`) schedules TTM scenes → `ttm.rs:run_frame` → `gfx.rs` primitives (`rect`, `ellipse`, `line`, `sprite`, `copy_rect_from`, `save_rect/restore_rect`, `load_picture`) with i16 script arguments; `0xF01F/0xF02F` load resources by script-supplied name; `0xC051` queues sound ids.
   - Engine planning (`engine/mod.rs`) indexes `scenes`, `pending`, `plan` with catalog-derived values; `walk.rs`, `island.rs` index EXE tables.
2. **`SCRANTIC.INI`** — `config.rs:load/parse/save` (strict integer parse, rewrite with defaults on error, `fs::write` on exit).
3. **CLI** — `cli.rs:parse_args` (flags only); `.scr` `saver.rs:parse` (`/s /c[:hwnd] /p hwnd /a`) → `windows_ui::settings_dialog`, `preview_attributes` (`with_parent_window` on caller HWND), `already_running` (named mutex `Local\OpenScranticSaver`).
4. **Files written**: `SCRANTIC.INI` in settings dir, F12 screenshots in exe dir, `%APPDATA%\OpenScrantic` creation.
5. **Env**: `OPEN_SCRANTIC_DEBUG`, `APPDATA`.
6. **CI/release** — `.github/workflows/ci.yml` (PR-triggered, read-only token, `Swatinem/rust-cache`), `release.yml` (`workflow_dispatch` on main, git-cliff version from commit messages, `publish` job with `contents: write` and default persisted checkout credentials, VirusTotal secret, sed/perl edits of Cargo files), `scripts/virustotal-scan.sh`, `build.rs` (embed-resource on Windows), `Cargo.lock`, `dependabot.yml`.

## Trust boundaries and strongest controls

- Data file → process memory/CPU: bounds-checked `bytes()` helper (`assets/mod.rs:41`), `MAX_UNPACKED` (`resource.rs:8`), decompressed-size equality check, `unpack_4bpp` cap, `.get()`-based indexing in VMs, `forbid(unsafe_code)`, `panic = "abort"` (any panic is a process abort).
- Data file → unattended `.scr` session: single-instance mutex; input guard ends run.
- INI → engine state: strict range-checked parser.
- PR author → CI: `permissions: contents: read`, `persist-credentials: false`, SHA-pinned actions; fork PRs get no secrets.
- Maintainer → release: `workflow_dispatch` restricted to `refs/heads/main`; publish runs only after builds succeed.

## Starting paths

`src/assets/`, `src/engine/`, `src/audio.rs`, `src/lib.rs`, `src/main.rs`, `src/cli.rs`, `src/saver.rs`, `src/windows_ui.rs`, `src/present.rs`, `examples/snapshot.rs`, `.github/workflows/`, `scripts/`, `build.rs`, `resources/`, `Cargo.toml`, `Cargo.lock`.

## Prior coverage

None: this is the first run; no prior ledger or findings exist.

## Companion selection

- `MEMORY-SAFETY-AND-BINARY.md`: binary-format parsers (NE, Dynamix archive, LZW/RLE), bytecode VMs, integer casts reproducing 16-bit arithmetic, the `unsafe` Win32 FFI module.
- `RESOURCE-EXHAUSTION-AND-AVAILABILITY.md`: decompression/aliasing amplification, superlinear VM evaluation, reachable fatal errors (`panic = "abort"`) from data supplied by a different principal than the victim.
- `SUPPLY-CHAIN-AND-RELEASE.md`: GitHub Actions CI/release workflows, unsigned release binaries, Cargo dependencies, build script.
- `DESKTOP-MOBILE-AND-LOCAL-IPC.md`: Windows `.scr` launch modes and HWND arguments, named mutex, local file writes and search paths.
- Not selected: `WEB-PROTOCOL-AND-AUTH.md`, `CLIENT-SIDE.md`, `AI-AND-LLM.md`, `CLOUD-AND-DEPLOYMENT.md`, `PROTOCOLS-RPC-AND-MESSAGING.md`, `DATA-ISOLATION-AND-LIFECYCLE.md` — no HTTP, browser, model, cloud, RPC/broker or multi-tenant store surface exists in source.
