import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const source = readFileSync(path.join(root, "original/src/field/pokemon.ts"), "utf8");
const match = source.match(/^  setStatStage\(stat: BattleStat, value: number\): void \{[\s\S]*?^  \}/m);
if (!match) throw new Error("Could not extract Pokemon.setStatStage from original/src/field/pokemon.ts");
const tempDir = mkdtempSync(path.join(os.tmpdir(), "pokerogue-stat-stage-"));
try {
  const tempFile = path.join(tempDir, "fixture.ts");
  const test = `const Phaser = { Math: { Clamp: (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v)) } };\nclass PokemonFixture { summonData = { statStages: Array(7).fill(0) }; ${match[0].trim()} }\nconst p = new PokemonFixture();\nfor (const stage of [-7, -6, -1, 0, 6, 7]) { p.summonData.statStages.fill(0); p.setStatStage(6, stage); console.log(stage + "," + p.summonData.statStages[5]); }\n`;
  writeFileSync(tempFile, test);
  const result = execFileSync(process.execPath, [tempFile], { encoding: "utf8" });
  writeFileSync(path.join(root, "pokerogue-tests/fixtures/stat-stages.csv"), `# Executed Pokemon.setStatStage from original/src/field/pokemon.ts.\n# Input accuracy stage, stored stage\n${result}`);
  console.log("Wrote stage-boundary fixtures by executing the original TypeScript setter.");
} finally {
  rmSync(tempDir, { recursive: true, force: true });
}
