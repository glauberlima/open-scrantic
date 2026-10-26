# Hunter brief (verbatim methodology sections + run-specific execution instructions)

#### Core hunting method — include in every hunter prompt

```text
## Defensive vulnerability-finding method

Your goal is to find source-grounded security invariant failures and the smallest fix,
not to expand harm beyond the boundary result. Stay within source review and bounded local execution.
Do not contact deployed endpoints, provider APIs, registries, identity systems,
message brokers, shared services, or other users. Use local dummy data only.

READ THE CODE AT DEPTH. Follow each assigned input through parsing, identity,
authorization, normalization, state, derived copies, and the final sink. Read sibling,
legacy, batch, retry, cancellation, migration, and error paths that produce the same
effect. Compare sibling controls for equivalence, not only presence, and compare what
one component guarantees with what the next component assumes.

WORK FROM A CONCRETE INVARIANT:
1. Name the lower-trust principal and starting capability.
2. Name the accepted value, action, state transition, or resource selector.
3. Locate the control that should reject, bind, isolate, limit, or revoke it.
4. Trace the exact source path after that decision.
5. Stop at the smallest affected dummy record, wrong return value, process-integrity
   effect, or locally observable shared-resource effect.
6. State a source-level change and regression case that enforce the invariant.

DEPTH BOUND: trace only paths that can reach your assigned boundary or whose
guarantees that boundary relies on. Stop a line of investigation as soon as the
invariant is settled either way, and record the result in your structured output —
a covered, candidate, or blocked disposition, or an `uncovered` entry — instead of
continuing to search.

TEST SAD PATHS AND DISAGREEMENTS. Check absent, empty, zero, negative, maximum,
over-limit, duplicate, mixed encoding, stale, revoked, reordered, concurrent,
partially migrated, failed dependency, and rollback state only where the interface
accepts them. Compare canonicalization and units at every parser or policy handoff.
For multi-step issues, treat each output as a prerequisite and do not assume a later
boundary. If any prerequisite is not established, record a blocker.

When a proposed high or critical candidate reveals a reusable root cause, search paths
owned by the assigned coverage IDs for lexical, structural, and logical variants.
Consolidate the same root cause, but establish each variant's conditions and impact
independently. Do not investigate peer-owned units. Return a variant with no current
coverage unit as `uncovered`.

USE THE NARROWEST LOCAL CHECK THAT SETTLES THE CLAIM. Target-controlled builds,
tests, processes, browsers, emulators, fuzzers, and fixture processing may run only
inside the parent-approved OS-enforced sandbox. It must disable external networking,
start from an empty allowlisted environment, expose target and tools read-only, permit
writes only to your scratch directory, and apply low CPU, memory, process, file-size,
disk, and wall-clock limits. Isolated loopback is allowed only for a local fixture.
If any control is unavailable, do not execute: return needs_validation with that exact
blocker. Prefer an existing unit test, minimal function harness, dummy-tenant service
call, small malformed fixture, deterministic race schedule, or locally rendered policy.
Do not install or fetch tools.

Record the exact input, command, limits, and minimum result. For the environment,
record only allowlisted variable names and safe non-secret values needed to reproduce
the check. Never capture the ambient environment, inherited variables, credentials,
authentication state, or unrelated host paths. The target-controlled process writes
only in scratch. After the sandbox and all its processes terminate, only trusted
parent-side code may promote predeclared scratch-relative files, following the
promotion procedure block included verbatim in this prompt. You and target code never
write retained artifacts. If promotion is unavailable or fails for decisive evidence,
return needs_validation with the exact promotion blocker.
Never stress availability, invoke a live target, use a real credential, publish an
artifact, or continue past the minimum observed effect.

A deployment, browser, provider, broker, OS, proxy, package, secret, or identity fact
outside source is not proof either way. If one such fact is decisive, return a
needs_validation record with the exact missing observation and safe owner-observed check.
```

#### Promotion procedure — copy this promotion procedure verbatim into every hunter prompt

```text
Artifact promotion procedure (trusted parent-side code only):
Reference only for you: the parent performs these steps; you never perform them.

Before execution, the parent opens and retains trusted, non-inheritable directory
descriptors for the agent's scratch/ and artifacts/ roots, and records an allowlist
of expected scratch-relative artifact files plus explicit per-file and cumulative
byte limits. Never pass those descriptors to the agent or sandbox. After the sandbox
and all its processes terminate, trusted parent-side code promotes each allowlisted
file separately:

1. Validate the declared relative path: reject absolute, empty, `.`, `..`, or
   symlinked components.
2. Walk each parent component from the retained scratch-root descriptor with
   no-follow directory-relative operations; never reopen by path.
3. Open the leaf no-follow and nonblocking.
4. Verify with `fstat` that it is a regular file with link count exactly one and
   within the recorded per-file and cumulative byte limits.
5. Enforce those limits again while reading from that descriptor.
6. Copy exactly the verified size, repeat `fstat`, and reject a changed identity,
   type, link count, or size.
7. For the destination, walk every parent component from the retained
   artifacts-root descriptor with no-follow directory-relative operations; require
   each existing component to be a real directory, and create any missing directory
   exclusively before reopening and verifying it no-follow.
8. Create the leaf exclusively without following links, verify that the opened
   destination is a regular file with link count exactly one, and copy from the
   verified source descriptor without reopening either path.
9. Use equivalent race-safe APIs on non-POSIX systems.
10. Never recursively copy or glob scratch, extract an archive into artifacts, or
    open or promote a symlink, FIFO, socket, device, directory, hard-linked file,
    changing file, or file that exceeds its bound.
11. If any check is unavailable, cannot be enforced, or fails, discard the scratch
    entry; if it is decisive evidence, retain `needs_validation` with the exact
    promotion blocker.
```

#### Core validation rules — include in every hunter prompt

```text
## Candidate gate

1. A candidate needs a complete repository-relative source trace and evidence for the
   claimed root cause, including the strongest source-visible control.
2. A proposed confirmed record needs a bounded local observed result, meaningful impact
   across a stated boundary, complete conditions, and no visible preventing layer.
3. Do not strengthen a crash into code execution, ordinary work into shared availability,
   or a same-principal action into privilege gain.
4. If a required fact is not source-visible or locally observable, use
   needs_validation. Name exact blockers; do not give it severity or speculative completion.
5. A missing best practice with no affected principal/resource is excluded or hardening,
   not a finding. A candidate disproved by source is not needs_validation.
6. Use the same source-derived fingerprint for the same root cause in every state.
   It must match `^[A-Za-z0-9][A-Za-z0-9._:/@+-]*$` and must not include a line,
   wave, agent, severity, or verdict.
7. Return an empty candidate array when nothing survives these gates.
```

## Structured hunter result

Return exactly one JSON object, with no surrounding prose:

```json
{
  "units": [
    {
      "coverage_id": "one assigned ID",
      "disposition": "covered|candidate|blocked",
      "reviewed_paths": ["repo/relative/path"],
      "checks": [
        {
          "agent_id": "canonical owner of this check",
          "reviewed_paths": ["repo/relative/path owned by this check"],
          "invariant": "specific control checked for this unit",
          "method": "source|local",
          "result": "what source or the bounded check established",
          "artifact": "agents/<agent-id>/artifacts/file for local, null for source"
        }
      ],
      "candidate_fingerprints": [],
      "unresolved": []
    }
  ],
  "candidates": [],
  "hardening": ["concrete non-finding note"],
  "uncovered": [
    {
      "surface": "...",
      "boundary": "...",
      "subsystem": "...",
      "attack_class": "...",
      "starting_paths": ["repo/relative/path"],
      "reason": "why this needs its own deterministic coverage unit"
    }
  ]
}
```

Each `candidates` entry is schema-shaped except that it uses `proposed_verdict` in place of `verdict`:

- `proposed_verdict: "confirmed"`: include every field required by the `confirmed` branch of `report-schema.json` other than `verdict`: `fingerprint`, title, description, `root_cause`, `intended_behavior`, ordered `trace`, `evidence`, `conditions`, target-neutral `execution`, `remediation`, `severity`, and `confidence`. The execution instructions describe only the bounded local check already performed. `payloads` holds the minimum test input, fixture, or native invocation. `observed_result` records actual local output. Overall severity must not exceed observed impact.
- `proposed_verdict: "needs_validation"`: include every field required by that schema branch other than `verdict`: `fingerprint`, title, description, `claimed_root_cause`, ordered `trace`, `evidence`, nonempty `blockers`, and `validation_plan` with at least one applicable `local` or `deployment` step. Do not invent an inapplicable context. Do not include severity, execution, remediation, reason, or a confirmed `root_cause`. `deployment` is an owner-observed check, not a request to probe a live target.

Every assigned coverage ID appears exactly once in `units`. A `covered` unit needs an owner, nonempty `reviewed_paths` and `checks`, no unresolved fact, and no candidate. A `candidate` unit has the same owned evidence and is the only state that carries linked fingerprints. A `blocked` unit is an owned partial review with nonempty paths, checks, and unresolved facts but no fingerprint. All source paths are repository-relative, never absolute or traversal paths. A trace with several entries begins at `entrypoint`, ends at `sink`, and labels intermediate steps `propagation`. Every check has its own canonical lowercase `agent_id` and nonempty `reviewed_paths`; the unit-level list is exactly the union of those owned paths. A `source` check uses `artifact: null`. A `local` check uses one successfully parent-promoted regular file beneath `agents/<check.agent_id>/artifacts/`; this permits a verifier to add independently owned evidence without taking ownership from the hunter. Never link scratch, an output-root file, or another check owner's artifact.

## Run-specific local execution (parent-approved OS-enforced sandbox)

- Output dir: /tmp/claude-0/-home-user-open-scrantic/fbbdca3d-d67e-504e-bff1-344795dee824/scratchpad/security-audit/run-1. Target (read-only): /home/user/open-scrantic.
- Sandbox: `/tmp/claude-0/-home-user-open-scrantic/fbbdca3d-d67e-504e-bff1-344795dee824/scratchpad/security-audit/run-1/bin/sandbox-run.sh <your-scratch-dir> <wall-seconds> <absolute-cmd> [args...]` — new net/mount/pid namespaces (no network), every mount read-only except your scratch dir, `env -i` with PATH=/usr/bin:/bin and HOME=TMPDIR=<scratch>, prlimit address space 2 GiB, CPU 60 s, file size 64 MiB, 64 processes, 256 fds, SIGKILL at the wall limit. Use wall-seconds <= 120. Run EVERY compile and EVERY execution of target-derived code through it.
- The full crate cannot be built (deps not cached; fetching is prohibited). Use the harness rlib `/tmp/claude-0/-home-user-open-scrantic/fbbdca3d-d67e-504e-bff1-344795dee824/scratchpad/security-audit/run-1/build/libscrantic_harness.rlib` (crate name `scrantic_harness`), compiled inside the sandbox from the target's UNCHANGED `src/assets/{mod,exe,resource}.rs` and `src/engine/{ads,gfx,rng,ttm}.rs` with opt-level=3, overflow-checks=off, debug-assertions=off (= the release profile), plus `engine::Env` copied verbatim from `src/engine/mod.rs:47-57`. Module paths: `scrantic_harness::assets::{resource::{Archive,decompress,Ttm,Op,...}, exe::ExeData, ScreensaverData}`, `scrantic_harness::engine::{ads::AdsRuntime, ttm::{run_frame,TtmCtx,Scene}, gfx::Canvas, rng::Rng, Env}`. Only `pub` items are reachable; build fixtures through public APIs (e.g. synthesize RESOURCE.MAP/.001 bytes and call `Archive::new`, then `archive.ttm(..)`, `AdsRuntime::load`).
- Compile a test program: `/tmp/claude-0/-home-user-open-scrantic/fbbdca3d-d67e-504e-bff1-344795dee824/scratchpad/security-audit/run-1/bin/sandbox-run.sh <scratch> 120 /root/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc --edition 2024 -C opt-level=3 -C overflow-checks=off -C panic=abort -L /tmp/claude-0/-home-user-open-scrantic/fbbdca3d-d67e-504e-bff1-344795dee824/scratchpad/security-audit/run-1/build --extern scrantic_harness=/tmp/claude-0/-home-user-open-scrantic/fbbdca3d-d67e-504e-bff1-344795dee824/scratchpad/security-audit/run-1/build/libscrantic_harness.rlib -o <scratch>/t <scratch>/t.rs` (use panic=abort to match the release profile). Then run `/tmp/claude-0/-home-user-open-scrantic/fbbdca3d-d67e-504e-bff1-344795dee824/scratchpad/security-audit/run-1/bin/sandbox-run.sh <scratch> <secs> <scratch>/t` and redirect its output to a file inside scratch via a wrapper only if needed (e.g. `/bin/sh -c '<scratch>/t > <scratch>/out.txt 2>&1; echo exit=$? >> <scratch>/out.txt'`).
- Write ONLY inside your scratch dir. To have a local result used as evidence, list the scratch-relative files in your result (field `artifact` = `agents/<your-id>/artifacts/<same relative path>`); the parent promotes them with the procedure above (per-file limit 1 MiB, cumulative 4 MiB). Keep evidence files small text files (source + output log).
- Never install, fetch, or contact any network service. Never edit target source or any file outside your scratch dir.
- The `confirmed` and `needs_validation` record branches you must follow are in `/tmp/claude-0/-home-user-open-scrantic/fbbdca3d-d67e-504e-bff1-344795dee824/scratchpad/skill/skills/security-audit/report-schema.json` — read that file before writing candidates.
