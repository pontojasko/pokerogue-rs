import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const phaserPath = process.argv[2];
if (!phaserPath) throw new Error("Pass Phaser 4.2.1 RandomDataGenerator.js path");
const speedSource = readFileSync(path.join(root, "original/src/utils/speed-order.ts"), "utf8");
const commonSource = readFileSync(path.join(root, "original/src/utils/common.ts"), "utf8");
const moveSource = readFileSync(path.join(root, "original/src/data/moves/move.ts"), "utf8");
const pokemonSource = readFileSync(path.join(root, "original/src/field/pokemon.ts"), "utf8");
const queueSource = readFileSync(path.join(root, "original/src/queues/move-phase-priority-queue.ts"), "utf8");
const sortStart = speedSource.indexOf("export function sortInSpeedOrder");
const shuffleStart = commonSource.indexOf("export function randSeedShuffle");
const shuffleEnd = commonSource.indexOf("\n}\n\nconst FPS", shuffleStart);
const priorityStart = moveSource.indexOf("  getPriority(user: Pokemon, simulated = true): number {");
const priorityEnd = moveSource.indexOf("\n  }\n\n  public getPriorityModifier", priorityStart);
const stageStart = pokemonSource.indexOf("  getStatStageMultiplier(");
const stageEnd = pokemonSource.indexOf("\n  }\n\n  /**", stageStart);
const modifierStart = moveSource.indexOf("  public getPriorityModifier(user: Pokemon, simulated = true): MovePriorityInBracket {");
const modifierEnd = moveSource.indexOf("\n  }\n\n  /**", modifierStart);
const queueSortStart = queueSource.indexOf("  private sortPostSpeed(): void {");
const queueSortEnd = queueSource.indexOf("\n  public cancelMove", queueSortStart);
const helperStart = queueSource.indexOf("function getPriorityModifiersForMP");
const quickAttackLine = moveSource.match(/new AttackMove\(MoveId\.QUICK_ATTACK,[^\n]+/);
const tackleLine = moveSource.match(/new AttackMove\(MoveId\.TACKLE,[^\n]+/);
function movePriority(line) {
  if (!line) throw new Error("Could not find source move definition");
  const args = line[0].slice(line[0].indexOf("(") + 1).replace(/\)\s*[,/].*$/, "").split(",");
  return Number(args[7].trim());
}
const quickAttackPriority = movePriority(quickAttackLine);
const tacklePriority = movePriority(tackleLine);
if ([sortStart, shuffleStart, shuffleEnd, priorityStart, priorityEnd, stageStart, stageEnd,
  modifierStart, modifierEnd, queueSortStart, queueSortEnd, helperStart].some(i => i < 0)) {
  throw new Error("Could not isolate original speed-order/priority methods");
}
const speedFunctions = speedSource.slice(sortStart).replaceAll("export function", "function");
const shuffleFunction = commonSource.slice(shuffleStart, shuffleEnd + 3).replace("export function", "function");
const priorityMethod = moveSource.slice(priorityStart, priorityEnd + 5);
const priorityModifierMethod = moveSource.slice(modifierStart, modifierEnd + 5);
const stageMethod = pokemonSource.slice(stageStart, stageEnd + 5);
const queueSortMethod = queueSource.slice(queueSortStart, queueSortEnd).replace("private sortPostSpeed", "sortPostSpeed");
const priorityHelpers = queueSource.slice(helperStart).replaceAll("function getPriority", "function getPriority");
const tempDir = mkdtempSync(path.join(os.tmpdir(), "pokerogue-action-order-"));
try {
  const tempFile = path.join(tempDir, "oracle.ts");
  const oracle = `
const { createRequire } = require("node:module");
const requireFromProject = createRequire(${JSON.stringify(path.join(root, "package.json"))});
const RandomDataGenerator = requireFromProject(${JSON.stringify(path.resolve(phaserPath))});
const rng = new RandomDataGenerator(["fixture-init"]);
global.Phaser = { Math: { RND: rng } };
global.Stat = { SPD: 5 };
global.ArenaTagSide = { PLAYER: 0, ENEMY: 1, BOTH: 2 };
global.ArenaTagType = { TRICK_ROOM: 0 };
global.BattlerTagType = { BYPASS_SPEED: 0 };
global.MovePriorityInBracket = { FIRST: 2, NORMAL: 1 };
const TempStatStageBoosterModifier = {};
global.globalScene = {};
class BooleanHolder { constructor(value) { this.value = value; } }
class NumberHolder { constructor(value) { this.value = value; } }
const ValueHolder = NumberHolder;
global.applyMoveAttrs = () => {};
global.applyAbAttrs = () => {};
${shuffleFunction}
${speedFunctions}
class MoveOracle { constructor(priority) { this.priority = priority; } ${priorityMethod} ${priorityModifierMethod} }
${priorityHelpers}
class MovePhaseQueueOracle { constructor(queue) { this.queue = queue; } ${queueSortMethod} }
class SpeedPokemonOracle {
  constructor(stage) { this.stage = stage; }
  getStatStage() { return this.stage; }
  isPlayer() { return false; }
  ${stageMethod}
}
function runScenario(spec) {
  const offset = spec.turn * 1000 + 2;
  const seed = Array.from(spec.waveSeed, c => String.fromCharCode(c.charCodeAt(0) + offset)).join("");
  const state = rng.state(); rng.sow([seed]);
  const draws = [];
  const integerInRange = rng.integerInRange.bind(rng);
  rng.integerInRange = (min, max) => { const roll = integerInRange(min, max); draws.push([min, max, roll]); return roll; };
  global.globalScene = {
    currentBattle: { turn: spec.turn },
    waveSeed: spec.waveSeed,
    turnCommandManager: { setOrder: null },
    arena: { applyTags: () => {} },
    applyModifiers: () => {},
    executeWithSeedOffset(fn, seedOffset, seedOverride) {
      const old = rng.state();
      const shifted = Array.from(seedOverride || spec.waveSeed, c => String.fromCharCode(c.charCodeAt(0) + seedOffset)).join("");
      rng.sow([shifted]); fn(); rng.state(old);
    },
  };
  const actors = spec.actors.map((a, i) => ({ ...a, getBattlerIndex: () => i, getTag: () => null,
    getEffectiveStat: () => Math.max(Math.floor(a.speed * new SpeedPokemonOracle(a.stage).getStatStageMultiplier(Stat.SPD)), 1) }));
  const speedOrdered = sortInSpeedOrder(actors);
  const phases = speedOrdered.map(p => ({ actor: p.name, timingModifier: 1, pokemon: p,
    move: { getMove: () => new MoveOracle(p.priority) } }));
  new MovePhaseQueueOracle(phases).sortPostSpeed();
  const ranked = phases.map(p => ({ actor: p.actor, priority: p.move.getMove().priority }));
  rng.integerInRange = integerInRange; rng.state(state);
  return { case: spec.case, effective_speeds: Object.fromEntries(actors.map(a => [a.name, a.getEffectiveStat(Stat.SPD)])),
    priorities: Object.fromEntries(ranked.map(a => [a.actor, a.priority])), order: ranked.map(a => a.actor),
    offset, shuffle_draws: draws };
}
const specs = [
  { case: "faster-player", turn: 1, waveSeed: "OrderParitySeed", actors: [{ name: "player", speed: 12, stage: 0, priority: 0 }, { name: "enemy", speed: 9, stage: 0, priority: 0 }] },
  { case: "slower-player", turn: 1, waveSeed: "OrderParitySeed", actors: [{ name: "player", speed: 7, stage: 0, priority: 0 }, { name: "enemy", speed: 11, stage: 0, priority: 0 }] },
  { case: "priority-over-speed", turn: 1, waveSeed: "OrderParitySeed", actors: [{ name: "player", speed: 7, stage: 0, priority: ${quickAttackPriority} }, { name: "enemy", speed: 11, stage: 0, priority: ${tacklePriority} }] },
  { case: "speed-stages", turn: 1, waveSeed: "OrderParitySeed", actors: [{ name: "player", speed: 9, stage: 1, priority: 0 }, { name: "enemy", speed: 11, stage: -1, priority: 0 }] },
  { case: "equal-speed-priority", turn: 1, waveSeed: "OrderParitySeed", actors: [{ name: "player", speed: 10, stage: 0, priority: 0 }, { name: "enemy", speed: 10, stage: 0, priority: 0 }] },
  { case: "equal-speed-priority-reversed-tie", turn: 1, waveSeed: "OrderTieSeed001", actors: [{ name: "player", speed: 10, stage: 0, priority: 0 }, { name: "enemy", speed: 10, stage: 0, priority: 0 }] },
];
process.stdout.write(JSON.stringify(specs.map(runScenario)));
`;
  writeFileSync(tempFile, oracle);
  const output = execFileSync(process.execPath, ["--experimental-strip-types", tempFile], { encoding: "utf8" });
  const results = JSON.parse(output);
const fixture = [
    "# Executed original sortInSpeedOrder, randSeedShuffle, Pokemon.getStatStageMultiplier, Move.getPriority and MovePhasePriorityQueue.sortPostSpeed under controlled mocks.",
    "# case,player_speed,enemy_speed,player_priority,enemy_priority,order,shuffle_offset,shuffle_roll",
    ...results.map(r => `${r.case},${r.effective_speeds.player},${r.effective_speeds.enemy},${r.priorities.player},${r.priorities.enemy},${r.order.join("|")},${r.offset},${r.shuffle_draws.map(d => d[2]).join("|") || "none"}`),
  ].join("\n") + "\n";
  writeFileSync(path.join(root, "pokerogue-tests/fixtures/action-order.csv"), fixture);
  console.log("Wrote TypeScript action-order fixtures.");
} finally {
  rmSync(tempDir, { recursive: true, force: true });
}
