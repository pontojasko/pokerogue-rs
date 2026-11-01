#!/usr/bin/env node
// Execute the pinned VictoryPhase.start method with a minimal, recording scene.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve } from 'node:path';
import { SourcePhaseTree, sourcePhaseManagerMethods } from './lib/source_phase_tree.mjs';

const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const source = readFileSync('original/src/phases/victory-phase.ts', 'utf8');
const marker = 'public override start(): void';
const start = source.indexOf(marker);
assert.ok(start >= 0, 'VictoryPhase.start missing');
const open = source.indexOf('{', start);
let depth = 1, quote = null, escaped = false, line = false, block = false, end = open + 1;
for (; end < source.length && depth; end++) {
  const c = source[end], n = source[end + 1];
  if (line) { if (c === '\n') line = false; continue; }
  if (block) { if (c === '*' && n === '/') { block = false; end++; } continue; }
  if (quote) { if (escaped) escaped = false; else if (c === '\\') escaped = true; else if (c === quote) quote = null; continue; }
  if (c === '/' && n === '/') { line = true; end++; continue; }
  if (c === '/' && n === '*') { block = true; end++; continue; }
  if (`"'\``.includes(c)) { quote = c; continue; }
  if (c === '{') depth++;
  else if (c === '}' && --depth === 0) break;
}
assert.equal(depth, 0, 'VictoryPhase.start body did not close');
let body = source.slice(open + 1, end).replace('super.start();', '');
const method = stripTypeScriptTypes(`function start(){${body}}`);
const phaseNames = [
  'BattleEndPhase', 'TrainerVictoryPhase', 'EggLapsePhase', 'ModifierRewardPhase',
  'SelectModifierPhase', 'SelectBiomePhase', 'NewBattlePhase', 'GameOverPhase',
  'AddEnemyBuffModifierPhase',
];
const modifierIds = [
  'LOCK_CAPSULE', 'EXP_CHARM', 'SUPER_EXP_CHARM', 'GOLDEN_POKEBALL',
  'VOUCHER_PLUS', 'VOUCHER_PREMIUM',
];
const makeEnum = names => new Proxy(Object.fromEntries(names.map(name => [name, name])), {
  get: (obj, key) => obj[key] ?? String(key),
});
const run = wave => {
  const tree = new SourcePhaseTree();
  const endTrace = [];
  class MessagePhase { constructor(message) { this.phaseName='MessagePhase'; this.message=message; } is(type){ return this.phaseName===type; } }
  const phaseManager = Object.assign({
    phaseQueue: tree,
    checkDynamic: phase => phase,
    create: (phaseName, ...args) => ({ phaseName, args, is(type){ return this.phaseName===type; } }),
    hasPhaseOfType: () => false,
  }, sourcePhaseManagerMethods(MessagePhase));
  const currentBattle = { waveIndex: wave, battleType: 'WILD', isBattleMysteryEncounter: () => false };
  const gameMode = {
    isEndless: false, isClassic: true, isDaily: false, hasRandomBiomes: false,
    isWaveFinal: () => false, getFixedBattle: () => undefined, getClearScoreBonus: () => 0,
  };
  const scene = {
    currentBattle, gameMode, phaseManager,
    gameData: { gameStats: { pokemonDefeated: 0 } },
    getEnemyParty: () => [], applyPartyExp: () => {}, modifiers: [], score: 0, offsetGym: false,
    getParty: () => [], isNewBiome: () => false, updateScoreText: () => {},
  };
  const context = { getPokemon: () => ({ getExpValue: () => 100 }), end: () => endTrace.push('END') };
  const startPhase = new Function('globalScene', 'timedEventManager', 'handleMysteryEncounterVictory', 'modifierTypes', 'BattleType', 'ClassicFixedBossWaves', 'ModifierTier', `return (${method});`)(
    scene, { getFixedBattleEventRewards: () => [] }, () => {}, makeEnum(modifierIds), { WILD: 'WILD', TRAINER: 'TRAINER', CLEAR: 'CLEAR' }, { EVIL_BOSS_2: 165 }, makeEnum(['COMMON','GREAT','ULTRA','ROGUE','MASTER']),
  );
  startPhase.call(context);
  const queue = [];
  let phase;
  while ((phase = tree.getNextPhase())) {
    const arg = phase.args[0];
    queue.push(arg === undefined ? phase.phaseName : `${phase.phaseName}:${typeof arg === 'string' ? arg : JSON.stringify(arg)}`);
  }
  return [...queue, ...endTrace].join('|');
};

const rows = [`# source=${pin}; executes VictoryPhase.start, original PhaseManager.pushNew/pushPhase methods and PhaseTree for Classic wild victory; constructors/dynamic scheduling stubbed`, '# wave,queued_phases'];
for (let wave = 1; wave <= 20; wave++) rows.push(`${wave},${run(wave)}`);
const output = `${rows.join('\n')}\n`;
const fixture = resolve('pokerogue-tests/fixtures/victory-phase.csv');
if (process.argv.includes('--check')) assert.equal(readFileSync(fixture, 'utf8'), output);
else writeFileSync(fixture, output);
console.log('VictoryPhase source oracle current.');
