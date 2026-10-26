# Leads awaiting validation

No `needs_validation` record remains in `findings.json`. Items outside the source that the owner has to settle:

- **Dependency advisories:** run `cargo audit` (or `cargo deny check advisories`) on `Cargo.lock` with a current
  advisory database. It could not be done offline, and no specific advisory is suspected.
- **Windows binary import table (optional):** run `dumpbin /imports` on the `.exe` and `.scr` and list DLLs outside
  KnownDLLs. The DLL lead was rejected as a generic Windows condition; this would only decide whether to apply
  `/DEPENDENTLOADFLAG:0x800` as hardening.
