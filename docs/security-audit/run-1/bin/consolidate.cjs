// Parent consolidation: node consolidate.cjs <ledger.json> <agent_id> <result.json>
const fs = require("fs"), path = require("path");
const [ledgerPath, agent, resPath] = process.argv.slice(2);
const L = JSON.parse(fs.readFileSync(ledgerPath, "utf8"));
const R = JSON.parse(fs.readFileSync(resPath, "utf8"));
const out = path.dirname(ledgerPath);
const errs = [];
const owned = L.filter((u) => u.agent_id === agent && u.status === "in_progress");
const seen = new Set();
for (const r of R.units) {
  const u = L.find((x) => x.coverage_id === r.coverage_id);
  if (!u || u.agent_id !== agent) { errs.push("unknown/unowned " + r.coverage_id); continue; }
  if (seen.has(r.coverage_id)) { errs.push("duplicate " + r.coverage_id); continue; }
  seen.add(r.coverage_id);
  for (const c of r.checks) {
    if (c.method === "source" && c.artifact !== null) errs.push("source check with artifact in " + u.key);
    if (c.method === "local") {
      const p = path.join(out, c.artifact || "");
      if (!c.artifact || !c.artifact.startsWith(`agents/${c.agent_id}/artifacts/`) || !fs.existsSync(p) || !fs.lstatSync(p).isFile())
        errs.push(`unpromoted artifact ${c.artifact} in ${u.key}`);
    }
  }
  u.status = r.disposition;
  u.reviewed_paths = [...new Set(r.checks.flatMap((c) => c.reviewed_paths))].sort();
  u.local_checks = r.checks;
  u.result_fingerprints = r.candidate_fingerprints || [];
  u.unresolved = r.unresolved || [];
  u.parent_hardening = R.hardening || [];
}
for (const u of owned) if (!seen.has(u.coverage_id)) errs.push("missing unit " + u.key);
for (const c of R.candidates || []) fs.writeFileSync(path.join(out, "candidates", c.fingerprint.replace(/[^A-Za-z0-9._-]/g, "_") + ".json"), JSON.stringify({ hunter: agent, ...c }, null, 2));
fs.writeFileSync(path.join(out, "results", agent + ".uncovered.json"), JSON.stringify(R.uncovered || [], null, 2));
fs.writeFileSync(ledgerPath, JSON.stringify(L, null, 2));
console.log(errs.length ? "ERRORS:\n" + errs.join("\n") : `ok: ${seen.size} units, ${(R.candidates || []).length} candidates, ${(R.uncovered || []).length} uncovered`);
