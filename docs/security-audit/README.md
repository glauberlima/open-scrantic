# Security audit

Security audits of this repository, run with the methodology of
[cloudflare/security-audit-skill](https://github.com/cloudflare/security-audit-skill).

| Run | Audited revision | Profile | Status |
|---|---|---|---|
| [run-1](run-1/REPORT.md) | `60e8e5e` (clean tree) | standard | **incomplete**: 3 confirmed low-severity findings, 6 coverage units deferred |

Start with [`run-1/REPORT.md`](run-1/REPORT.md) for the findings and [`run-1/NEXT-STEPS.md`](run-1/NEXT-STEPS.md)
for what remains to be done.

## Layout of a run

| Path | Content |
|---|---|
| `REPORT.md`, `FINDINGS-DETAIL.md`, `NEEDS-VALIDATION.md` | Human-readable reports derived from the final records |
| `findings.json` | Final records (`confirmed`, `rejected`), valid against the skill's `report-schema.json` |
| `coverage-ledger.json` | One entry per coverage unit with status, reviewed paths and checks |
| `run-metadata.json`, `architecture.md` | Run parameters and the reconnaissance summary |
| `candidates/`, `results/` | Raw hunter candidates and per-hunter structured results |
| `agents/*/artifacts/` | Promoted local evidence (fixtures and their output) |
| `harness/`, `bin/`, `final/` | Tooling used to run and consolidate the audit |
| `hunter-brief.md` | The hunter prompt material used for every hunter |

## Caveats

- The tooling was written for the Linux cloud sandbox of the first run. `harness/lib.rs` includes the sources through
  absolute `#[path]` attributes (`/home/user/open-scrantic/...`), `bin/sandbox-run.sh` needs `unshare`, `prlimit`
  and root, and two files mention session-specific scratch paths. Adapt the paths before reusing it.
- The fixtures under `agents/*/artifacts/` are synthetic and contain no original screensaver data.
- The findings describe local denial-of-service and terminal-output issues triggered by malformed data files. If you
  consider that too much detail for a public branch, keep this branch unmerged or move the audit to a private place.
