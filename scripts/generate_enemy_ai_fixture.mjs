import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const phaserPath = process.argv[2];
if (!phaserPath) throw new Error("Pass Phaser 4.2.1 RandomDataGenerator.js path");
const pokemonSource = readFileSync(path.join(root, "original/src/field/pokemon.ts"), "utf8");
const moveSource = readFileSync(path.join(root, "original/src/data/moves/move.ts"), "utf8");
const aiStart = pokemonSource.indexOf("  getNextMove(): TurnMove {");
const aiEnd = pokemonSource.indexOf("\n  }\n\n  /**\n   * Determines the Pokemon the given move", aiStart);
if (aiStart < 0 || aiEnd < 0) throw new Error("Could not isolate Pokemon.getNextMove");
const aiMethod = pokemonSource.slice(aiStart, aiEnd + 5).replace("  getNextMove(): TurnMove {", "  getNextMove() {");
const baseStart = pokemonSource.indexOf("  getBaseDamage({");
const baseEnd = pokemonSource.indexOf("\n  }\n\n  /** Determine the STAB multiplier", baseStart);
const attackStart = pokemonSource.indexOf("  getAttackDamage({");
const formulaStart = pokemonSource.indexOf("    damage.value = toDmgValue(", attackStart);
const formulaEnd = pokemonSource.indexOf("\n    );", formulaStart);
if (baseStart < 0 || baseEnd < 0 || attackStart < 0 || formulaStart < 0 || formulaEnd < 0) {
  throw new Error("Could not isolate simulated TypeScript damage calculation");
}
const baseDamageMethod = pokemonSource.slice(baseStart, baseEnd + 5);
const damageFormula = pokemonSource.slice(formulaStart, formulaEnd + 7);
const attackClass = moveSource.indexOf("export class AttackMove extends Move {");
const scoreStart = moveSource.indexOf(
  "  getTargetBenefitScore(user: Pokemon, target: Pokemon, move: Move): number {",
  attackClass,
);
const scoreEnd = moveSource.indexOf("\n  }\n}\n\nexport class StatusMove", scoreStart);
if (attackClass < 0 || scoreStart < 0 || scoreEnd < 0) {
  throw new Error("Could not isolate AttackMove.getTargetBenefitScore");
}
const scoreMethod = moveSource.slice(scoreStart, scoreEnd + 5);
const tempDir = mkdtempSync(path.join(os.tmpdir(), "pokerogue-enemy-ai-"));
try {
  const tempFile = path.join(tempDir, "oracle.ts");
  const test = `
const { createRequire } = require("node:module");
const requireFromProject = createRequire(${JSON.stringify(path.join(root, "package.json"))});
const RandomDataGenerator = requireFromProject(${JSON.stringify(path.resolve(phaserPath))});
global.AiType = { RANDOM: 0, SMART_RANDOM: 1, SMART: 2 };
global.MoveUseMode = { NORMAL: 0, IGNORE_PP: 1 };
global.MoveTarget = { ATTACKER: "attacker" };
global.MoveCategory = { STATUS: "status", PHYSICAL: "physical", SPECIAL: "special" };
global.BattlerTagType = { ALWAYS_CRIT: "always-crit" };
global.MoveId = { SUCKER_PUNCH: "sucker", UPPER_HAND: "upper", THUNDERCLAP: "thunderclap", STRUGGLE: "struggle" };
global.BattlerIndex = { ENEMY: 2 };
global.EncoreTag = class {};
global.NumberHolder = class { constructor(value) { this.value = value; } };
global.ValueHolder = global.NumberHolder;
global.Stat = { ATK: "atk", SPATK: "spatk", DEF: "def", SPDEF: "spdef" };
global.applyMoveAttrs = () => {};
global.getMoveTargets = () => ({ targets: [0], multiple: false });
function toDmgValue(value) { return Math.max(Math.floor(value), 1); }
class PokemonDamageOracle { ${baseDamageMethod} }
class BaseMove { getTargetBenefitScore() { return 0; } }
class AttackMoveOracle extends BaseMove {
  constructor(id, name, type, category, power, effectiveness) {
    super(); Object.assign(this, { moveId: id, id, name, type, category, power, effectiveness });
    this.accuracy = 100; this.moveTarget = "near-other"; this.isUnimplemented = false;
  }
  getUserBenefitScore() { return 0; }
  getMove() { return this; }
  getName() { return this.name; }
  isUsable() { return [true, ""]; }
  is(kind) { return kind === "AttackMove"; }
  hasAttr() { return false; }
  applyConditions() { return true; }
  calculateEffectivePower() { return this.power; }
  calculateBattlePower() { return this.power; }
  getMoveEffectiveness() { return this.effectiveness; }
  ${scoreMethod}
}
const ember = new AttackMoveOracle("ember", "Ember", "fire", "special", 40, 2);
const tackle = new AttackMoveOracle("tackle", "Tackle", "normal", "physical", 40, 1);
const counter = new AttackMoveOracle("counter", "Counter", "fighting", "physical", -1, 1);
counter.moveTarget = MoveTarget.ATTACKER;
const counterAsOpponent = new AttackMoveOracle("counter", "Counter", "fighting", "physical", -1, 1);
const wrappers = [ember, tackle].map(move => ({ moveId: move.moveId, getMove: () => move,
  getName: () => move.name, isUsable: () => [true, ""] }));
const counterWrappers = [counter, tackle].map(move => ({ moveId: move.moveId, getMove: () => move,
  getName: () => move.name, isUsable: () => [true, ""] }));
const counterControlWrappers = [counterAsOpponent, tackle].map(move => ({ moveId: move.moveId, getMove: () => move,
  getName: () => move.name, isUsable: () => [true, ""] }));
class EnemyAiFixture { ${aiMethod} }
function runScenario(name, targetHp, moves = wrappers, seed = "ParitySeed000000") {
  const shift = Array.from(seed, c => String.fromCharCode(c.charCodeAt(0) + 64)).join("");
  const rng = new RandomDataGenerator(["fixture-init"]); rng.sow([shift]);
  const rngCalls = [], sourceLogs = [], damageByMove = {};
  global.console = { log(...args) { sourceLogs.push(args); }, error: console.error };
  const target = {
    hp: targetHp, waveData: { abilityRevealed: false }, isPlayer: () => true, getBattlerIndex: () => 0,
    getAlly: () => null,
    getEffectiveStat(stat) { return stat === Stat.DEF ? 9 : 11; },
    getBaseDamage(params) { return new PokemonDamageOracle().getBaseDamage.call(this, params); },
    getAttackTypeEffectiveness: (_type, options) => options.move.effectiveness,
    getMoveEffectiveness: (_user, move) => move.effectiveness,
    getAttackDamage({ source, move, simulated = true }) {
      const baseDamage = this.getBaseDamage({ source, move, moveCategory: move.category, simulated });
      const damage = { value: 0 }, targetMultiplier = 1, multiStrikeEnhancementMultiplier = { value: 1 };
      const weatherDamageMultiplier = 1, glaiveRushMultiplier = { value: 1 }, criticalMultiplier = { value: 1 };
      const randomMultiplier = 1, stabMultiplier = source.isOfType(move.type) ? 1.5 : 1;
      const typeMultiplier = move.effectiveness, burnMultiplier = 1, screenMultiplier = { value: 1 };
      const hitsTagMultiplier = { value: 1 };
      ${damageFormula}
      damageByMove[move.id] = damage.value;
      return { damage: damage.value };
    },
  };
  const user = {
    level: 5, summonData: { moveQueue: [] }, aiType: AiType.SMART_RANDOM,
    getMoveQueue: () => [], getMoveset: () => moves, getTag: () => null,
    isPlayer: () => false, isOfType: type => type === "fire", getNextTargets: () => [0],
    getEffectiveStat: stat => stat === Stat.SPATK ? 11 : 10,
  };
  global.globalScene = {
    getField: () => [target], arena: { isMoveWeatherCancelled: () => false, isMoveTerrainCancelled: () => false },
    randBattleSeedInt(range) { const value = rng.integerInRange(0, range - 1); rngCalls.push([range, value]); return value; },
  };
  const result = new EnemyAiFixture().getNextMove.call(user);
  const candidates = sourceLogs.find(([label]) => label === "Move Pool:")[1];
  const scores = sourceLogs.find(([label]) => label === "Move Scores:")[1];
  return { name, seed, targetHp, damages: damageByMove,
    koMoves: Object.entries(damageByMove).filter(([, damage]) => damage >= targetHp).map(([move]) => move),
    candidates, scores: Object.fromEntries(candidates.map((move, index) => [move, scores[index]])),
    rngCalls, chosen: result.move };
}
const scenarios = [runScenario("full-hp-none-ko", 19), runScenario("above-ember-boundary", 16),
  runScenario("ember-exact-ko-boundary", 15), runScenario("both-exact-ko-boundary", 5)];
const eligibility = [
  runScenario("attacker-target-counter-excluded", 1, counterWrappers, "CounterSeed00000"),
  runScenario("counter-mistarget-control", 1, counterControlWrappers, "CounterSeed00000"),
];
process.stdout.write(JSON.stringify({ scenarios, eligibility }));
`;
  writeFileSync(tempFile, test);
  const result = execFileSync(process.execPath, ["--experimental-strip-types", tempFile], {
    encoding: "utf8",
  });
  const { scenarios: oracles, eligibility } = JSON.parse(result);
  const base = oracles[0];
  const aiFixture = [
    "# Executed extracted Pokemon.getNextMove and AttackMove.getTargetBenefitScore from pinned TypeScript, using Phaser 4.2.1 RNG.",
    "# seed,enemy,enemy_stats,active_target,target_hp,target_types,moves,scores,rng_calls,chosen",
    `ParitySeed000000,Charmander,ATK10|SPATK11,Bulbasaur,19,GRASS|POISON,Ember|Tackle,Ember:${base.scores.Ember}|Tackle:${base.scores.Tackle},${base.rngCalls.map(([range, value]) => `${range}:${value}`).join("|")},${base.chosen}`,
  ].join("\n") + "\n";
  const koFixture = [
    "# Executed extracted original AI prefilter and damage base/final formulas with controlled no-ability/no-attribute battlers.",
    "# scenario,seed,target_hp,damages,ko_moves,candidates,scores,rng_calls,chosen",
    ...oracles.map(o => `${o.name},${o.seed},${o.targetHp},${Object.entries(o.damages).map(([m, d]) => `${m}:${d}`).join("|")},${o.koMoves.join("|") || "none"},${o.candidates.join("|")},${Object.entries(o.scores).map(([m, s]) => `${m}:${s}`).join("|")},${o.rngCalls.map(([range, value]) => `${range}:${value}`).join("|") || "none"},${o.chosen}`),
  ].join("\n") + "\n";
  writeFileSync(path.join(root, "pokerogue-tests/fixtures/enemy-ai.csv"), aiFixture);
  writeFileSync(path.join(root, "pokerogue-tests/fixtures/enemy-ko.csv"), koFixture);
  const eligibilityFixture = [
    "# Executed original getNextMove with Counter (MoveTarget.ATTACKER) and a counterfactual ordinary-target control.",
    "# scenario,seed,target_hp,damages,ko_moves,candidates,scores,rng_calls,chosen",
    ...eligibility.map(o => `${o.name},${o.seed},${o.targetHp},${Object.entries(o.damages).map(([m, d]) => `${m}:${d}`).join("|")},${o.koMoves.join("|") || "none"},${o.candidates.join("|")},${Object.entries(o.scores).map(([m, s]) => `${m}:${s}`).join("|")},${o.rngCalls.map(([range, value]) => `${range}:${value}`).join("|") || "none"},${o.chosen}`),
  ].join("\n") + "\n";
  writeFileSync(path.join(root, "pokerogue-tests/fixtures/enemy-ko-target.csv"), eligibilityFixture);
  console.log("Wrote original SMART_RANDOM and KO-prefilter fixtures with Phaser RNG.");
} finally {
  rmSync(tempDir, { recursive: true, force: true });
}
