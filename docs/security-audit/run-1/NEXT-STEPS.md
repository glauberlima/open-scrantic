# Next steps

What remains after audit run 1, in the order I would do it. Nothing in `src/` has been changed yet: this branch only
adds documentation and audit artifacts.

## 0. Pick up

```sh
git fetch origin && git checkout security-audit/run-1
```

Read [`REPORT.md`](REPORT.md) first. The audited revision is `60e8e5e`; `main` has since moved only in `README.md`.
Work on the fixes in a new branch from `main` (the audit artifacts do not need to be merged).

## 1. Fix the three confirmed findings

Each fix goes in its own commit (Conventional Commits, en-US), with a regression test that **fails before and passes
after**. The tests use synthetic fixtures built in the test itself, so they run without the original data files.

| # | Finding | Files | Suggested commit | Regression test |
|---|---|---|---|---|
| 1 | `WAVESFX` aliasing | `src/assets/exe.rs` | `fix(assets): bound the decoded sound memory` | NE with one WAVE and many aliasing `WAVESFXn` names must return `Err`; the builder is in `agents/verifier-wave/artifacts/t.rs` |
| 2 | Overlapping `RESOURCE.MAP` entries | `src/assets/resource.rs` | `fix(assets): index archive entries without copying them` | map with a header every 17 bytes in a 1 MiB volume must not retain more than the volume; fixture in `agents/verifier-overlap/artifacts/t.rs` |
| 3 | Raw names on the terminal | `src/assets/mod.rs` (`Display` for `AssetError`) | `fix(assets): escape data-file text in error messages` | an error built from a name containing `\x1b` must contain no raw `0x1b`; the cases are in `agents/verifier-stderr/artifacts/evidence-verifier-t.rs` |

The suggested code is in section 2 of `REPORT.md` and in `findings.json` (`remediation.code_changes`). It is a
sketch, not a patch: check variable names and types in context.

Acceptance criteria:

- The three tests fail on `main` and pass on the fix.
- The genuine files are still accepted unchanged: 80 catalog scenes, 23 sounds, 35 paths (see
  `loads_installed_game` in `src/assets/mod.rs`).
- Finding 1: the budget must not reject the real `SCRANTIC.SCR`. The genuine sounds are far below 16 MiB, but
  confirm with the real file.
- Finding 2: if you choose the "reject overlaps / total <= volume" variant instead of storing ranges, confirm that
  the genuine map does not repeat an offset.

## 2. Verify with the original files (needs your machine)

These steps need `SCRANTIC.SCR`, `RESOURCE.MAP` and `RESOURCE.001`, which are not in the repository.

```sh
cargo fmt --check
cargo clippy --all-targets            # must stay at zero warnings
OPEN_SCRANTIC_DATA=/path/to/install cargo test
OPEN_SCRANTIC_DEBUG=1 cargo run --release   # sessions still plan and scenes still complete
cargo run --release --example snapshot -- /path/to/install /tmp/after 600 100
```

Run the snapshot example on `main` and on the fix and compare the PNGs; the rendering must be identical. On Windows
also run `cargo clippy --all-targets --features screensaver` and `cargo test --features screensaver`.

## 3. Check dependencies

```sh
cargo install cargo-audit && cargo audit
```

The audit could not do this offline. Record the result here or open an issue for any advisory that applies to a
locked version.

## 4. Second audit pass on the deferred units

Six units were deferred (see `coverage-ledger.json`, status `deferred`; `unresolved` says why). In priority order:

1. **LZW decoder** (`src/assets/resource.rs`, `lzw`). A first review called it covered; the coverage critic disputed
   a premise about how the first code of a stream is handled. Add a test that feeds many malformed and random
   streams through `decompress` and checks that it always terminates in bounded time and memory, then settle the
   disputed premise from the source.
2. **TTM drawing cost** (`src/engine/gfx.rs`, `rect`/`ellipse`; `src/engine/ttm.rs`). The loops cover the whole
   unclipped rectangle before clipping. Measure the cost for large `i16` arguments with scaled-down inputs and, if
   it is real, clip to the canvas before looping.
3. **ADS VM** (`src/engine/ads.rs`): recursion depth and termination of `run_block`, `exec`, `skip_block`,
   `eval_cond`, `random`; `panic = "abort"` makes a stack overflow fatal.
4. **Engine planning** (`src/engine/mod.rs`, `walk.rs`, `island.rs`) with an empty or FINAL-less scene catalog,
   weights summing to zero or above 65535, and route lists that never reach the target.
5. **Aggregate decoded memory** when an ADS RES list names the same TTM many times and when `0xF01F`/`0xF02F` run
   repeatedly (unit proposed by the critic, never assigned).

To rerun the audit tooling, read the caveats in [`../README.md`](../README.md): the harness uses absolute paths and
the sandbox runner needs Linux with `unshare`, `prlimit` and root.

## 5. Hardening backlog (optional, none of it is a vulnerability)

See section 5 of `REPORT.md`. The most useful items: `persist-credentials: false` and a protected environment on the
release publish job, bumping the version before building so `--version` matches the tag, `cargo build --locked` in
CI and release, and a size cap in `read_ci`.

## 6. When done

- Update `SECURITY.md` if the scope statement changes.
- The release workflow generates the changelog from the commit messages, so use `fix(...)` types for the three fixes.
- Delete this branch or move `docs/security-audit/` somewhere private if you do not want the finding details public.
