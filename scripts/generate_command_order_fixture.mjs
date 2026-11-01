import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const source = readFileSync(path.join(root, "original/src/phases/turn-start-phase.ts"), "utf8");
const match = source.match(/^  private getCommandOrder\(\): BattlerIndex\[\] \{[\s\S]*?^  \}/m);
if (!match) throw new Error("Could not extract TurnStartPhase.getCommandOrder");
const method = match[0].replace("private getCommandOrder(): BattlerIndex[] {", "getCommandOrder() {");
const cases = [
  ["player-switch-vs-enemy-fight", "POKEMON", "FIGHT"],
  ["player-fight-vs-enemy-switch", "FIGHT", "POKEMON"],
  ["fight-vs-fight-stable-field-order", "FIGHT", "FIGHT"],
];
const tempDir = mkdtempSync(path.join(os.tmpdir(), "pokerogue-command-order-"));
try {
  const tempFile = path.join(tempDir, "fixture.ts");
  const test = `
global.Command = { FIGHT: "FIGHT" };
class TurnStartFixture { ${method} }
const cases = ${JSON.stringify(cases)};
for (const [name, playerCommand, enemyCommand] of cases) {
  global.globalScene = {
    currentBattle: { turnCommands: [{ command: playerCommand }, { command: enemyCommand }] },
    getPlayerField: () => [{ getBattlerIndex: () => 0 }],
    getEnemyField: () => [{ getBattlerIndex: () => 1 }],
  };
  const order = new TurnStartFixture().getCommandOrder().map(index => index === 0 ? "player" : "enemy");
  console.log([name, playerCommand, enemyCommand, order.join(">")].join(","));
}
`;
  writeFileSync(tempFile, test);
  const result = execFileSync(process.execPath, ["--experimental-strip-types", tempFile], {
    encoding: "utf8",
  });
  writeFileSync(
    path.join(root, "pokerogue-tests/fixtures/command-order.csv"),
    `# Executed TurnStartPhase.getCommandOrder from original/src/phases/turn-start-phase.ts.\n# case,player command,enemy command,order\n${result}`,
  );
  console.log("Wrote command ordering cases by executing original TurnStartPhase method.");
} finally {
  rmSync(tempDir, { recursive: true, force: true });
}
