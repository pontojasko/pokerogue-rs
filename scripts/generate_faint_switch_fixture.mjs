import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const source = readFileSync(path.join(root, "original/src/phases/faint-phase.ts"), "utf8");
const branchStart = source.lastIndexOf("    if (this.player) {", source.indexOf("const legalPlayerPokemon ="));
const branchEnd = source.indexOf("\n    // in double battles", branchStart);
if (branchStart < 0 || branchEnd < 0) throw new Error("Could not isolate the FaintPhase player replacement branch");
const branch = source.slice(branchStart, branchEnd);
const tempDir = mkdtempSync(path.join(os.tmpdir(), "pokerogue-faint-switch-"));
try {
  const tempFile = path.join(tempDir, "oracle.ts");
  const oracle = `
global.SwitchType = { SWITCH: "SWITCH" };
function run(name, legalPokemon) {
  const calls = [];
  global.globalScene = {
    getPokemonAllowedInBattle: () => legalPokemon,
    currentBattle: { double: false, getBattlerCount: () => 1 },
    phaseManager: {
      unshiftNew: (...args) => calls.push(["unshift", ...args]),
      pushNew: (...args) => calls.push(["push", ...args]),
    },
  };
  const ctx = { player: true, fieldIndex: 0 };
  function extractedBranch() { ${branch} }
  extractedBranch.call(ctx);
  return { name, calls };
}
process.stdout.write(JSON.stringify([
  run("no-legal-reserve", []),
  run("healthy-reserve", [{ isActive: () => false }]),
]));
`;
  writeFileSync(tempFile, oracle);
  const output = execFileSync(process.execPath, ["--experimental-strip-types", tempFile], { encoding: "utf8" });
  const results = JSON.parse(output);
  const rows = [
    "# Executed the extracted player-team decision branch from original FaintPhase.doFaint.",
    "# scenario,queued_operation,phase,switch_type,field_index,modal,do_return",
    ...results.map(r => `${r.name},${r.calls[0][0]},${r.calls[0][1]},${r.calls[0][2] ?? "none"},${r.calls[0][3] ?? "none"},${r.calls[0][4] ?? "none"},${r.calls[0][5] ?? "none"}`),
  ];
  writeFileSync(path.join(root, "pokerogue-tests/fixtures/faint-switch.csv"), rows.join("\n") + "\n");
  console.log("Wrote original FaintPhase replacement fixtures.");
} finally {
  rmSync(tempDir, { recursive: true, force: true });
}
