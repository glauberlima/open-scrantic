const fs = require("fs");
const t = (kind, file, line, scope, description) => ({ kind, file, line, scope, description });
const e = (file, line, description) => ({ file, line, description });
const sev = (l, lr, i, ir) => ({ likelihood: { score: l, reason: lr }, impact: { score: i, reason: ir }, overall_severity: "low" });

const wave = {
  verdict: "confirmed",
  fingerprint: "src/assets/exe.rs:NeFile::wave_resources:aliased-wave-decode-unbounded",
  title: "SCRANTIC.SCR name-table aliasing decodes one small WAVE into unbounded sound memory at startup",
  description: "NeFile::wave_resources decodes the WAVE resource into a fresh Vec<f32> for every RT_NAMETABLE entry named WAVESFX<n>, even when many entries point at the same (type id, resource id), and keeps every copy in ExeData::sounds. A crafted SCRANTIC.SCR with one large WAVE and thousands of ~19-byte name entries allocates about 4 bytes per WAV byte per entry during ScreensaverData::load, before the first frame, ending in an OOM abort or host memory pressure. With the .scr build this repeats on every idle launch.",
  root_cause: "wave_resources has no per-resource or aggregate budget on decoded samples and no deduplication by (type_id, res_id), so decoded size is O(name-table entries x WAV size) instead of O(file size), unlike the 16 MiB MAX_UNPACKED cap resource.rs applies to the other formats.",
  intended_behavior: "Decoded sound memory is bounded by a constant budget (comparable to MAX_UNPACKED) or grows linearly with distinct WAVE bytes; over-budget input is rejected with AssetError::Format like other malformed data.",
  trace: [
    t("entrypoint", "src/main.rs", 307, "main", "ScreensaverData::load_first loads the supplier-controlled SCRANTIC.SCR at startup (also the .scr /s and /p launch paths)."),
    t("propagation", "src/assets/mod.rs", 99, "ScreensaverData::load", "read_ci (fs::read at mod.rs:67) reads the whole file with no size cap; ExeData::parse is called at mod.rs:109."),
    t("propagation", "src/assets/exe.rs", 166, "ExeData::parse", "sounds: ne.wave_resources()? runs unconditionally during parse."),
    t("propagation", "src/assets/exe.rs", 246, "NeFile::wave_resources", "The name-table loop runs once per entry with no entry-count or byte budget."),
    t("sink", "src/assets/exe.rs", 260, "NeFile::wave_resources", "sounds.insert(n, decode_wav(wav)?) re-decodes the aliased resource for each distinct WAVESFX<n> and retains every copy."),
  ],
  evidence: [
    e("src/assets/exe.rs", 258, "resources.get(&(type_id, res_id)) lets any number of names resolve to the same resource; there is no dedup."),
    e("src/assets/exe.rs", 279, "Each decode_wav call collects a new Vec<f32> sized to the data chunk (4 bytes per source byte)."),
    e("src/assets/exe.rs", 37, "ExeData::sounds: HashMap<u32, Vec<f32>> owns each copy for the program lifetime."),
    e("src/audio.rs", 233, "Soft mode (always on in the .scr) resamples and caches per played id, adding a further copy."),
  ],
  conditions: [
    { kind: "user_interaction", description: "The user installs a crafted SCRANTIC.SCR and runs the program, or Windows launches the .scr on idle." },
    { kind: "data_state", description: "The crafted NE passes the build-string checks (ACTIVITY.ADS at DS:0x1681, WAVESFX%d at DS:0x1D56) and the minimal table layout; each aliasing name needs a distinct WAVESFX<n> number." },
  ],
  execution: {
    attacker_perspective: "Supplier of SCRANTIC.SCR who controls all of its bytes and has no other access to the victim.",
    payloads: ["Synthetic 14-segment NE built by build(n, wav_len) in agents/verifier-wave/artifacts/t.rs: resource table shift 4 with a name table (type 0x800F, id 0x8001) and one RIFF/WAVE (type 0x8011, id 0x8002, 64 KiB of 0x80); the name table holds N entries [len, 0x8011, 0x8002, \"\\0WAVESFX<k>\"]."],
    instructions: ["Compile t.rs in sandbox-run.sh against the harness rlib built from the unchanged src/assets, with rustc --edition 2024 -C opt-level=3 -C overflow-checks=off -C panic=abort.", "Run `t 1 65536` and `t 256 65536`; each prints file size, sounds, retained f32 bytes and VmHWM. Stop there and extrapolate retained = N x 4 x WAV size."],
    observed_result: "Independently reproduced by two agents. N=1: 75808 B file, 262064 B retained. N=256: 80032 B file, 67088384 B retained (838x), VmHWM about 67.9 MB. The hunter also measured N=1024: 93120 B file, 268353536 B retained (2882x). Growth is linear in N.",
  },
  remediation: {
    strategy: "Enforce an aggregate decoded-sample budget in wave_resources, checked from the data length before each decode, and return AssetError::Format when exceeded; this keeps ExeData::sounds as HashMap<u32, Vec<f32>> so audio.rs and engine/mod.rs are untouched. Deduplicating aliases would save more memory but needs a shared buffer type and edits to the consumers. Add a regression test with many aliasing WAVESFXn names expecting Err.",
    code_changes: [{ file_name: "src/assets/exe.rs", fixed_code: "const MAX_SOUND_BYTES: usize = 16 << 20; // total WAVE bytes decoded; one f32 per byte\n// in wave_resources, before the loop:\nlet mut budget = MAX_SOUND_BYTES;\n// per WAVESFXn entry, before decode_wav:\nbudget = budget\n    .checked_sub(wav.len())\n    .ok_or_else(|| AssetError::Format(\"NE sound resources too large\".into()))?;\nsounds.insert(n, decode_wav(wav)?);" }],
  },
  severity: sev("low", "The victim must install a crafted SCRANTIC.SCR from a third-party source; there is no network vector, but SECURITY.md lists excessive memory from a malformed SCRANTIC.SCR as in scope.", "low", "Local availability only: startup OOM abort or memory pressure, and a failing .scr on each idle launch until the file is removed. Demonstrated only to about 270 MB and extrapolated; no corruption or code execution."),
  confidence: { score: "high", reason: "Both the hunter and an independent verifier re-read the cited lines and reran the unchanged parser in the release-equivalent profile; growth was linear." },
};

const overlap = {
  verdict: "confirmed",
  fingerprint: "assets/resource:Archive::new:overlapping-entry-copy-amplification",
  title: "Overlapping RESOURCE.MAP entries make Archive::new copy the volume once per entry",
  description: "Archive::new reads only the u32 at map entry+4 as a volume offset; the name and size come from the 17-byte header at that offset in RESOURCE.001. Each entry is bounds-checked alone against the volume and copied with to_vec(). A supplier can place a header every 17 bytes at the start of the volume, each with a distinct name and a size running to the end of the volume, so every header lies inside the data of the earlier entries. Retained memory is then about count x volume length rather than the volume length. Measured with the real Archive::new: a 1 MiB volume plus a 4,117-byte map (512 entries) retained 523,200 KiB, about 511x the volume.",
  root_cause: "Archive::new has no non-overlap check and no running total or cap on copied bytes (MAX_UNPACKED covers only decompression); it copies each in-bounds range into the HashMap with data.to_vec() instead of keeping ranges into the one already-loaded volume buffer.",
  intended_behavior: "Loading the archive uses memory proportional to the size of RESOURCE.001; index entries reference the shared volume buffer, or the loader rejects overlapping entries or an aggregate size above the volume length.",
  trace: [
    t("entrypoint", "src/assets/mod.rs", 100, "ScreensaverData::load", "RESOURCE.MAP and RESOURCE.001 are read whole from the data directory; the only check is volume.len() >= 1024 (mod.rs:102)."),
    t("propagation", "src/assets/mod.rs", 108, "ScreensaverData::load", "Archive::new(&map, &volume) is called with the supplier-controlled bytes."),
    t("propagation", "src/assets/resource.rs", 19, "Archive::new", "count is a u16 from map[19..21] (up to 65535); the offset is the u32 at entry+4 and the name and size come from the header at that offset (resource.rs:22-28)."),
    t("sink", "src/assets/resource.rs", 31, "Archive::new", "files.insert(name, data.to_vec()) allocates a fresh copy of each in-bounds range; distinct names are kept and there is no aggregate cap or overlap rejection."),
  ],
  evidence: [
    e("src/assets/resource.rs", 29, "Each entry's data range is validated only against the volume length (via bytes(), mod.rs:41), never against other entries."),
    e("src/assets/resource.rs", 31, "An owned copy is made per distinct name; a repeated name overwrites and frees the earlier copy, so distinct names are needed, which headers 17 bytes apart provide."),
  ],
  conditions: [
    { kind: "data_state", description: "The supplier controls or replaces RESOURCE.MAP and RESOURCE.001 in the install directory, with 17-byte headers at distinct offsets inside the volume, distinct names, and sizes extending to the end of the volume." },
    { kind: "environmental_dependency", description: "Growth is limited only by host RAM and overcommit; count is a u16, so at most 65535 copies. The extreme was deliberately not run." },
  ],
  execution: {
    attacker_perspective: "A lower-trust supplier of the screensaver data files (for example a tampered or shared data package) that the program loads at start, including unattended .scr launches.",
    payloads: ["RESOURCE.001 of V bytes: for i in 0..N, at offset 17*i write a 13-byte NUL-padded name 'F%05d.TTM', then a u32 LE size = V-17*i-17, and fill the rest with any bytes. RESOURCE.MAP: 21-byte header with u16 LE N at offset 19, then N 8-byte entries whose u32 LE at +4 is 17*i."],
    instructions: ["Build the synthetic map and volume as above (fixture: agents/verifier-overlap/artifacts/t.rs).", "Call the real Archive::new(&map, &volume) inside the sandbox and read VmRSS from /proc/self/status before and after.", "Repeat with V = 64 KiB, 256 KiB, 1 MiB and N = 64 to 512, bounded to 60 s wall."],
    observed_result: "Archive::new returns Ok in every case and retained RSS grows as about N x V. V=64 KiB: N=64 gives 4,116 KiB, N=256 gives 15,920 KiB. V=256 KiB: N=128 gives 32,832 KiB, N=512 gives 129,984 KiB. V=1 MiB: N=128 gives 131,136 KiB, N=256 gives 262,164 KiB, N=512 gives 523,200 KiB (map only 4,117 B, 1.7 s).",
  },
  remediation: {
    strategy: "Do not copy per entry: store (offset, len) ranges into the single owned volume buffer and slice on lookup, or keep the copies but reject overlapping ranges and enforce a running total no greater than volume.len(). Either is applied in Archive::new, the last point that sees both the map and the volume, and bounds retained memory by the volume the user already holds in memory.",
    code_changes: [{ file_name: "src/assets/resource.rs", fixed_code: "// files: HashMap<String, (usize, usize)>  // (start, len) into the retained volume\n// total = total.checked_add(size)?; if total > volume.len() { return Err(AssetError::Format(\"RESOURCE.MAP entries overlap\".into())); }" }],
  },
  severity: sev("low", "The supplier must provide or replace the local RESOURCE.MAP/.001 files; this is a lower trust level than the user but not a remote vector, and the crafted pair is a few KiB.", "low", "Availability only: memory exhaustion of this one process, possibly host OOM-kill or swap. About 511x amplification measured (512 MiB from a 1 MiB volume plus a 4 KiB map); no corruption, code execution or confidentiality loss."),
  confidence: { score: "high", reason: "Reproduced with the unchanged Archive::new from the harness rlib at three volume sizes and several entry counts; growth was linear in N x V." },
};

const stderr = {
  verdict: "confirmed",
  fingerprint: "stderr-unsanitized-data-file-strings",
  title: "Data-file resource names reach the terminal unescaped in warning and error messages",
  description: "Names stored in RESOURCE.001 (TTM text operands and ADS RES-chunk TTM names), RESOURCE.MAP (entry names) and SCRANTIC.SCR (the ADS name table) are decoded with String::from_utf8_lossy, which keeps ESC, BEL and other control bytes. When such a name does not resolve, AssetError::Missing/Format embeds it verbatim, with no escaping and no length cap, and the desktop binary prints it to stderr with eprintln! in a normal run. A supplier of modified data files can make the program emit raw terminal control sequences (screen clear, title change, OSC 52 clipboard write on terminals that allow it) and megabyte-long lines to the terminal that launched it. Impact is limited to terminal output integrity: the victim already runs the supplier's data in-process and the VM gives no code execution. The Windows .scr build has no console and is not affected.",
  root_cause: "Data-derived text is treated as display-safe: resource.rs:27, 44, 129 and 209 and exe.rs:77 decode with from_utf8_lossy; resource.rs:40 builds AssetError::Missing(name.to_string()); the Display impl at assets/mod.rs:24 writes it unmodified; callers print it with eprintln! at ttm.rs:278, ttm.rs:290, engine/mod.rs:459 and cli.rs:62. No control-character filtering or length cap exists on the path.",
  intended_behavior: "Text that comes from the data files is shown only after control characters are escaped and its length is capped, so the files can influence the program's own rendering and nothing else.",
  trace: [
    t("entrypoint", "src/assets/mod.rs", 101, "ScreensaverData::load", "RESOURCE.001 (and RESOURCE.MAP, SCRANTIC.SCR) are read whole from the install directory; the supplier fully controls their bytes."),
    t("propagation", "src/assets/resource.rs", 209, "Ttm::parse", "A TTM opcode with nargs==15 (0xF01F/0xF02F) becomes op.text through from_utf8_lossy; ESC and BEL survive and there is no length bound."),
    t("propagation", "src/assets/resource.rs", 40, "Archive::chunks", "A missing name becomes AssetError::Missing(name.to_string()); Display at assets/mod.rs:24 prints it verbatim."),
    t("sink", "src/engine/ttm.rs", 278, "run_frame 0xF01F error arm", "eprintln!(\"warning: {e}\") runs unconditionally, with no debug gate, and writes the raw bytes to stderr; the same at ttm.rs:290, engine/mod.rs:459 and cli.rs:62."),
  ],
  evidence: [
    e("src/assets/resource.rs", 209, "No control-character filter or length limit on op.text."),
    e("src/assets/resource.rs", 129, "ADS RES TTM names are decoded the same way and reach engine/mod.rs:459 through AdsRuntime::load (ads.rs:56)."),
    e("src/assets/exe.rs", 77, "The SCRANTIC.SCR ADS name table (15 bytes max) is decoded with from_utf8_lossy and used as the archive lookup name."),
    e("src/engine/ttm.rs", 278, "Unconditional eprintln of the data-controlled name."),
  ],
  conditions: [
    { kind: "data_state", description: "The data files are modified so a TTM/ADS resource name or a RESOURCE.MAP entry name contains control bytes and does not resolve (or the map entry is malformed)." },
    { kind: "environmental_dependency", description: "The desktop binary's stderr is attached to a terminal; the .scr build has no console. What the sequences do depends on the terminal emulator." },
    { kind: "user_interaction", description: "The user starts the binary from a shell, as the README instructs for macOS and Linux." },
  ],
  execution: {
    attacker_perspective: "Supplier or redistributor of modified RESOURCE.001, RESOURCE.MAP or SCRANTIC.SCR; no other access.",
    payloads: ["TTM chunk TT3 (RLE literal) holding opcode 0xF02F (or 0xF01F) followed by the bytes 1B 5D 30 3B 54 07 1B 5B 32 4A 00 naming a resource that does not exist", "ADS RES chunk: count=1, id=1, TTM name 1B 5D 30 3B 54 07 1B 5B 32 4A 00"],
    instructions: ["Compile the harness (agents/verifier-stderr/artifacts/evidence-verifier-t.rs) against the unchanged harness rlib inside sandbox-run.sh with -C opt-level=3 -C overflow-checks=off -C panic=abort.", "Run it with each mode argument inside sandbox-run.sh, redirecting stderr to a scratch file, and inspect with od -c; never print the file to a terminal."],
    observed_result: "stderr contained 'warning: missing screensaver file: ' followed by raw 033 ] 0 ; T \\a 033 [ 2 J for the 0xF02F and ADS RES paths. The hunter additionally showed a 1 MiB name emitted as one write of 1,048,612 bytes. The ADS, exe-name and map-name cases used eprintln! calls re-typed in the harness because the real sinks are outside it.",
  },
  remediation: {
    strategy: "Sanitize at the display boundary: escape control and non-printable characters and cap the length inside AssetError's Display impl for the Missing and Format variants, which covers every current and future sink (ttm.rs:278 and 290, engine/mod.rs:459, cli.rs:62, saver.rs:141 and the {name}: ... Format messages). Escaping only in Archive::chunks would leave the Format messages open. Add a regression test that an error built from a name containing \\x1b contains no raw 0x1b.",
    code_changes: [{ file_name: "src/assets/mod.rs", fixed_code: "fn sanitize(s: &str) -> String {\n    let mut out: String = s.chars().take(64).flat_map(char::escape_default).collect();\n    if s.chars().count() > 64 {\n        out.push_str(\"...\");\n    }\n    out\n}\n// in impl fmt::Display for AssetError:\n//   Self::Missing(name) => write!(f, \"missing screensaver file: {}\", sanitize(name)),\n//   Self::Format(msg) => write!(f, \"invalid screensaver data: {}\", sanitize(msg)),\n// (Io keeps its own message)" }],
  },
  severity: sev("low", "Needs modified data files that the victim installs, plus a desktop run from a terminal that interprets the sequences; the .scr build is unaffected.", "low", "Limited to terminal output integrity (spoofed or cleared output, title changes, OSC 52 clipboard write where permitted) and very long lines; no code execution or file access."),
  confidence: { score: "high", reason: "Raw control bytes were reproduced independently on the real TTM 0xF02F sink and on re-typed Display sinks for the other paths, with od -c; the terminal-side effect depends on the emulator and was not exercised." },
};

const dll = {
  verdict: "rejected",
  fingerprint: "dll-search-order:windows-exe-app-dir-shared-with-data-files",
  title: "Windows exe placed beside third-party data files (generic application-directory DLL planting, not project-specific)",
  description: "The candidate said the README tells Windows users to put the .exe in the same folder as the three original data files, so a supplier's archive that also contains a DLL could gain code execution through the Windows application-directory DLL search. The README citations are accurate, but this is the generic rule that an .exe loads non-KnownDLL imports from its own folder first; the project adds no control gap of its own, and anyone who can place a DLL in the folder the user runs the .exe from can already supply an .exe.",
  claimed_root_cause: "Install instructions put the executable's directory (the DLL search location) in the same folder as third-party data files, and the build does not restrict dependent-DLL resolution to System32.",
  trace: [
    t("entrypoint", "README.md", 40, "Get started step 2", "Tells the user to put the downloaded program in the same folder as the three original files."),
    t("propagation", "README.md", 62, "Screensaver install", "The recommended .scr runs from System32, where only an administrator can write; its data lives in %APPDATA%\\OpenScrantic or beside the .scr."),
    t("sink", "build.rs", 9, "main", "The build only embeds resources for the screensaver feature and sets no /DEPENDENTLOADFLAG, which is Windows default behavior rather than a project-introduced flaw."),
  ],
  evidence: [
    e("README.md", 40, "Quote: 'Put it in the same folder as the three original screensaver files'; step 1 sends the user to the project's own release page and nothing tells the user to extract an archive into that folder."),
    e("README.md", 74, "Quote: 'To try it without installing, right-click the .scr and choose Test'; this runs a file the user chose, and the data folder is not necessarily that folder."),
    e("src/windows_ui.rs", 122, "The only LibraryLoader use in src/ is GetModuleHandleW(null()); src/ has no LoadLibrary, SetDllDirectory or GetProcAddress call."),
  ],
  reason: "Rejected as a generic Windows condition without a project-specific control gap. The default desktop search order (already-loaded modules and manifest redirection, KnownDLLs, application directory, System32, Windows directory, current directory, PATH) does resolve non-KnownDLL imports from the application directory first and nothing in the repository changes it, but a supplier who can place a DLL beside the .exe also gets the user to run the project's .exe from the supplier's folder, so the data-file trust boundary adds nothing beyond what a replacement .exe or shortcut would. The dependency sources were not available locally, so nothing is asserted about winit, softbuffer or cpal imports; the unobservable PE import table would not change the verdict. Recorded as hardening: link with /DEPENDENTLOADFLAG:0x800 and advise a dedicated folder.",
};

const all = [wave, overlap, stderr, dll].sort((a, b) => (a.fingerprint < b.fingerprint ? -1 : 1));
fs.writeFileSync("findings.json", JSON.stringify(all, null, 2));
