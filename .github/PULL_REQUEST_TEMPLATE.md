## What and why

<!-- One subject per pull request. Link the issue if there is one. -->

## How it was checked against the original

<!-- A disassembly address (seg10:1acb, DS:1756), a snapshot image, or a video. -->

## Checklist

- [ ] `cargo clippy --all-targets` has no warnings
- [ ] `cargo fmt --check` passes
- [ ] `OPEN_SCRANTIC_DATA=/path/to/install cargo test` passes
- [ ] If `src/saver.rs` or `src/windows_ui.rs` changed: clippy and tests pass with `--features screensaver` (Windows)
- [ ] The commit messages follow Conventional Commits and are in en-US
- [ ] No screensaver data (`SCRANTIC.SCR`, `RESOURCE.MAP`, `RESOURCE.001`) is included
- [ ] Ported logic cites the original address in a comment
- [ ] A deliberate deviation from the original is recorded in `docs/ENGINE.md`
