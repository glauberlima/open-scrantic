# Security audit: open-scrantic (run 1)

**Profile:** `standard`, scope: the whole repository. **Audited revision:** `60e8e5e` (clean tree; `main` has since
moved to `8c04961`, which only changes `README.md`). **Budget:** 16 of 30 agent invocations used. **Execution:**
source review and local checks inside a sandbox (no network, read-only mounts except the agent scratch directory,
empty environment, CPU/memory/process/time limits). **Prior runs:** none.

> **This is a partial pass (`run_status: incomplete`).** Six coverage units were deferred and the final clean
> coverage critic did not run. No "no vulnerabilities" claim holds for the areas listed in section 6.

## 1. Security posture

The program has no network code, authentication or command execution, and `unsafe` exists only in the Win32 module,
which was reviewed without findings. The real attack surface is the three original data files, which come from third
parties, and `SCRANTIC.INI`. The parsers use checked access (`bytes()`, `.get()`), and 200,000 random mutations of
the NE header produced no panic or hang. The confirmed findings are all **low-severity availability or
terminal-output-integrity issues** that need the user to install tampered data files.

## 2. Confirmed findings

| Severity | Title | Boundary | Observed result |
|---|---|---|---|
| Low | `WAVESFX` aliasing in `SCRANTIC.SCR` decodes one WAVE many times | `.SCR` supplier to process memory | an 80 KB file retained 67 MB (N=256), linear in N |
| Low | Overlapping `RESOURCE.MAP` entries copy the volume once per entry | `.MAP`/`.001` supplier to process memory | 1 MiB volume + 4 KB map retained 523,200 KiB (~511x) |
| Low | Resource names from the data files reach the terminal unescaped | data supplier to the user's terminal | raw ESC/BEL bytes on stderr; a 1 MiB name gives a 1 MiB line |

Each was found by one agent, reproduced by a second, and checked by a third.

### 2.1 `src/assets/exe.rs:246-260`: `WAVESFX` aliasing

- **Invariant:** decoded memory should be bounded by the size of the file.
- **Root cause:** `wave_resources` calls `decode_wav` for every name-table entry, without deduplicating by
  `(type_id, res_id)` and without a budget. Each copy stays in `ExeData::sounds` (4 bytes per WAV byte).
- **Attack:** a `SCRANTIC.SCR` with one large WAVE and thousands of 19-byte names allocates all of it in
  `ScreensaverData::load`, before the first frame. The `.scr` repeats this on every idle launch, and its always-on
  soft audio mode adds one more resampled copy per played id.
- **Fix (minimal, keeps `HashMap<u32, Vec<f32>>`):**

```rust
const MAX_SOUND_BYTES: usize = 16 << 20; // total WAVE bytes decoded
// in wave_resources, before the loop:
let mut budget = MAX_SOUND_BYTES;
// per WAVESFXn entry, before decode_wav:
budget = budget
    .checked_sub(wav.len())
    .ok_or_else(|| AssetError::Format("NE sound resources too large".into()))?;
sounds.insert(n, decode_wav(wav)?);
```

Regression test: an NE with one WAVE and many aliasing `WAVESFXn` names must return `Err`. The genuine sounds total
far less than 16 MiB.

### 2.2 `src/assets/resource.rs:21-31`: overlapping entries

- **Root cause:** `Archive::new` reads only the `u32` at `entry+4` as the offset; the name and size come from the
  17-byte header at that offset. Each entry is validated alone and copied with `to_vec()`. Headers every 17 bytes,
  with distinct names and sizes running to the end of the volume, all lie inside the data of earlier entries.
- **Preferred fix:** do not copy per entry. Keep `(start, len)` ranges into the single volume buffer and slice on
  lookup. Alternative: reject overlaps and require `total <= volume.len()` (small risk of double-counting a genuine
  map that repeats an offset).

```rust
// files: HashMap<String, (usize, usize)>  // (start, len) into the retained volume
// in Archive::new, instead of data.to_vec():
files.insert(name, (offset + 17, size));
```

Regression test: the 17-byte-header fixture must not retain more than `volume.len()`.

### 2.3 `src/assets/mod.rs:24`: raw names on the terminal

- **Root cause:** names decoded with `String::from_utf8_lossy` keep ESC and BEL, end up in `AssetError`, and are
  printed by `eprintln!` at `ttm.rs:278`, `ttm.rs:290`, `engine/mod.rs:459` and `cli.rs:62` with no escaping or
  length cap, in a normal (non-debug) run.
- **Impact:** clearing or spoofing the screen, changing the window title and writing the clipboard through OSC 52 in
  terminals that allow it. The Windows `.scr` has no console and is not affected. No code execution.
- **Fix at the display boundary (covers every sink, including the `Format` messages):**

```rust
fn sanitize(s: &str) -> String {
    let mut out: String = s.chars().take(64).flat_map(char::escape_default).collect();
    if s.chars().count() > 64 { out.push_str("..."); }
    out
}
// in impl fmt::Display for AssetError:
Self::Missing(name) => write!(f, "missing screensaver file: {}", sanitize(name)),
Self::Format(msg)   => write!(f, "invalid screensaver data: {}", sanitize(msg)),
```

Escaping only in `Archive::chunks` would leave the `Format` messages open.

## 3. Leads needing validation

None remain in `findings.json`. The Windows DLL lead was rejected (section 4). The dependency advisory check is a
coverage gap (section 6), not a lead with a severity.

## 4. Rejected candidate

**`dll-search-order:windows-exe-app-dir-shared-with-data-files`.** The README tells users to put the `.exe` next to
the data files, and the build does not restrict DLL resolution. But loading non-KnownDLL imports from the
application directory is the default Windows rule for every downloaded `.exe`; whoever can plant a DLL there can
also get the user to run an `.exe` of their own; and the recommended `.scr` install runs from `System32`. The
dependency sources were not available, so nothing is claimed about their imports, which would not change the
verdict. Kept as hardening.

## 5. Hardening notes and positive patterns

**Hardening (no boundary violated):**

- `release.yml`: `persist-credentials: false` on the publish checkout; a protected environment or tag rules;
  validate `TAG` against `^v[0-9]+\.[0-9]+\.[0-9]+$` and pass `VERSION` to `sed`/`perl` through the environment;
  `git push --atomic`; bump the version before building (the `v0.4.0` binaries report `0.3.0`); build provenance
  attestations.
- CI and release: `--locked`. `scripts/virustotal-scan.sh`: drop `-L` from requests that carry `x-apikey`.
- `/DEPENDENTLOADFLAG:0x800` in `build.rs`, and advise a dedicated folder for the `.exe`.
- Cap file sizes in `read_ci` (and require a regular file) and for `SCRANTIC.INI`; `create_new` for screenshots and
  an atomic INI write.
- `preview_attributes` should check `IsWindow(parent)`; `decode_wav` is only safe on 64-bit targets (no 32-bit
  target is published); `Ttm::parse` expands each opcode to ~40 bytes before truncating to `pages`.
- `.gitignore` matches the data-file names case-sensitively while the loader is case-insensitive;
  `examples/snapshot.rs` accepts `SIZE=0x0` (developer tool).

**Positive patterns:** checked access in every parser; 16 MiB decompression cap; `forbid(unsafe_code)` outside the
Win32 module; panics abort; CI actions pinned by SHA with `permissions: contents: read` and
`persist-credentials: false`; no secrets in the tree or in the 50-commit history.

## 6. Coverage

Final ledger (20 units): **9 covered, 4 with a decided candidate, 6 deferred, 1 blocked.**

**Not completed (deferred):**

- TTM and ADS interpreters: CPU cost of the drawing opcodes (`rect` and `ellipse` loop over the whole unclipped
  rectangle before clipping), integer behavior, recursion depth. Partial source review found no out-of-bounds
  index, but nothing was executed.
- Engine planning with an empty or malformed scene catalog, and route lists.
- LZW decoder: a first review marked it covered and the coverage critic disputed a premise of that review about the
  handling of the first code. It is **unresolved and unverified**; treat it as the top priority of the next run.
- Aggregate decoded memory when ADS RES lists and TTM bitmap/screen loads repeat (unit proposed by the critic,
  never assigned).

**Blocked:** advisories for the 241 locked dependencies (no network or advisory database). Run `cargo audit`.

**Deviations from the method:** the four reconnaissance agents were replaced by the coordinator reading the code
(~5.5k lines). The full crate could not be built (dependencies not cached, fetching prohibited), so the checks used
a harness that includes the dependency-free modules unchanged. The sinks in `cli.rs`, `engine/mod.rs` and
`audio.rs` were verified by reading the source, with the messages re-typed in the harness.
