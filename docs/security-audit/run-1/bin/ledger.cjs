// Parent-only ledger tool. Usage: node ledger.cjs seed <units.json> <ledger.json>
//                                 node ledger.cjs assign <ledger.json> <agent_id> <id-prefix>...
const fs = require("fs");
const enc = (s) => Array.from(Buffer.from(s.normalize("NFC"), "utf8"))
  .map((b) => /[A-Za-z0-9\-._~]/.test(String.fromCharCode(b)) ? String.fromCharCode(b) : "%" + b.toString(16).toUpperCase().padStart(2, "0")).join("");
const id = (r) => ["surface", "boundary", "subsystem", "attack_class"].map((k) => enc(r[k])).join("::") + (r.lifecycle ? "::" + enc(r.lifecycle) : "");
const [cmd, a, b, ...rest] = process.argv.slice(2);
if (cmd === "seed") {
  const units = JSON.parse(fs.readFileSync(a, "utf8")).map((u) => ({
    coverage_id: id(u.canonical_refs), canonical_refs: u.canonical_refs,
    surface: u.surface, boundary: u.boundary, subsystem: u.subsystem, attack_class: u.attack_class,
    starting_paths: u.starting_paths, ordinary_attack_class_block: u.ordinary_attack_class_block,
    selected_companion_blocks: u.selected_companion_blocks, excluded_blocks: u.excluded_blocks,
    prior_status: "none", attempts: [], wave: 1, status: "planned", agent_id: null,
    reviewed_paths: [], local_checks: [], result_fingerprints: [], unresolved: [], key: u.key,
  }));
  units.sort((x, y) => (x.coverage_id < y.coverage_id ? -1 : 1));
  fs.writeFileSync(b, JSON.stringify(units, null, 2));
} else if (cmd === "assign") {
  const L = JSON.parse(fs.readFileSync(a, "utf8"));
  for (const u of L) if (rest.includes(u.key)) { u.status = "in_progress"; u.agent_id = b; }
  fs.writeFileSync(a, JSON.stringify(L, null, 2));
}
