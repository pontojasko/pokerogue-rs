#!/usr/bin/env node
// Execute the pinned LevelUpPhase.end queue insertion for ordinary level moves.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { SourcePhaseTree, extractSourceBody, sourcePhaseManagerMethods } from './lib/source_phase_tree.mjs';
import { stripTypeScriptTypes } from 'node:module';

const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const source = readFileSync('original/src/phases/level-up-phase.ts', 'utf8');
const marker = 'public override end(): void';
const start = source.indexOf(marker);
assert.ok(start >= 0, 'LevelUpPhase.end missing');
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
assert.equal(depth, 0, 'LevelUpPhase.end body did not close');
const body = source.slice(open + 1, end).replace('super.end();', '');
const method = stripTypeScriptTypes(`function end(){${body}}`);
const sourceMoves = [
  [7, 'LEECH_SEED', 0],
  [10, 'GROWTH', 0],
  [13, 'POISON_POWDER', 0],
  [13, 'SLEEP_POWDER', 0],
];
const expDataSource = readFileSync('original/src/data/exp.ts', 'utf8');
const expLevelsStart = expDataSource.indexOf('[', expDataSource.indexOf('const expLevels ='));
assert.ok(expLevelsStart >= 0, 'expLevels table missing');
let expDepth = 1, expEnd = expLevelsStart + 1;
for (; expEnd < expDataSource.length && expDepth; expEnd++) {
  if (expDataSource[expEnd] === '[') expDepth++;
  else if (expDataSource[expEnd] === ']' && --expDepth === 0) break;
}
assert.equal(expDepth, 0, 'expLevels table did not close');
const expLevels = expDataSource.slice(expLevelsStart, expEnd + 1);
const totalExpBody = extractSourceBody(expDataSource, 'export function getLevelTotalExp');
const pokemonSource = readFileSync('original/src/field/pokemon.ts', 'utf8');
const addExpBody = extractSourceBody(pokemonSource, 'public addExp(');
const totalExpCode = stripTypeScriptTypes(`const expLevels=${expLevels}; function getLevelTotalExp(level,growthRate){${totalExpBody}}`);
const totalExp = new Function('GrowthRate', `${totalExpCode}; return getLevelTotalExp;`)({
  ERRATIC: 0, FAST: 1, MEDIUM_FAST: 2, MEDIUM_SLOW: 3, SLOW: 4, FLUCTUATING: 5,
});
const queued = [];
const prevoMessages = [];
const phaseTree = new SourcePhaseTree();
class MessagePhase { constructor(message, callbackDelay, prompt, promptDelay) { this.phaseName='MessagePhase'; this.message=message; this.args=[callbackDelay,prompt,promptDelay]; prevoMessages.push(message); } is(type){ return this.phaseName===type; } }
const makePhaseManager = tree => Object.assign({
  phaseQueue: tree,
  checkDynamic: phase => phase,
  create: (phaseName, ...args) => ({ phaseName, args, slot: args[0], move: args[1], is(type){ return this.phaseName===type; } }),
  pushPhase: phase => tree.pushPhase(phase),
}, sourcePhaseManagerMethods(MessagePhase));
const phase = {
  lastLevel: 5,
  partyMemberIndex: 0,
  pokemon: { pauseEvolutions: true },
  getPokemon: () => ({ getLevelMoves: () => sourceMoves }),
  getEvolution: () => null,
};
const endPhase = new Function('globalScene', 'getBaseLearnableMoveSource', 'LearnableMoveSource', 'getPokemonNameWithAffix', 'allMoves', 'i18next', `return (${method});`)(
  { phaseManager: makePhaseManager(phaseTree) },
  source => source - (source % 2),
  { PREVO: 6 },
  () => '',
  {},
  { t: () => '' },
);
endPhase.call(phase);
let queuedPhase;
while ((queuedPhase = phaseTree.getNextPhase())) {
  if (queuedPhase.phaseName === 'LearnMovePhase') queued.push(queuedPhase.move);
}
assert.deepEqual(queued, ['LEECH_SEED', 'GROWTH', 'POISON_POWDER', 'SLEEP_POWDER']);
const prevoPhase = {
  lastLevel: 14,
  partyMemberIndex: 0,
  pokemon: { pauseEvolutions: true },
  getPokemon: () => ({ getLevelMoves: () => [[15, 'BUG_BITE', 6]] }),
  getEvolution: () => null,
};
const prevoTree = new SourcePhaseTree();
const prevoManager = makePhaseManager(prevoTree);
const prevoEnd = new Function('globalScene', 'getBaseLearnableMoveSource', 'LearnableMoveSource', 'getPokemonNameWithAffix', 'allMoves', 'i18next', `return (${method});`)(
  { phaseManager: Object.assign(prevoManager, { unshiftNew: () => assert.fail('prevo move must not become LearnMovePhase') }) },
  source => source - (source % 2),
  { PREVO: 6 },
  () => 'Silcoon',
  { BUG_BITE: { name: 'Bug Bite' } },
  { t: (key, options) => `${key}:${options.moveName}` },
);
prevoEnd.call(prevoPhase);
assert.deepEqual(prevoMessages, ['battle:prevoMovePrompt:Bug Bite']);
const queuedPrevo = prevoTree.getNextPhase();
assert.equal(queuedPrevo.phaseName, 'MessagePhase');
assert.equal(queuedPrevo.message, prevoMessages[0]);

const nestedTree = new SourcePhaseTree();
const nestedManager = makePhaseManager(nestedTree);
const expFixture = readFileSync('pokerogue-tests/fixtures/party-exp.csv', 'utf8')
  .split('\n').find(line => line.startsWith('exp-share-five-stacks-two-participants,'));
assert.ok(expFixture, 'two-participant EXP phase fixture missing');
const expSlots = expFixture.split(',')[4].split('|').map(entry => Number(entry.split(':')[0])).slice(0, 2);
for (const slot of expSlots) {
  nestedTree.pushPhase({ phaseName: 'ExpPhase', slot, is(type){ return this.phaseName===type; } });
}
const expSource = readFileSync('original/src/phases/exp-phase.ts', 'utf8');
const expBody = extractSourceBody(expSource, 'public override start(): void').replace('super.start();', '');
const makeExpStart = new Function(
  'globalScene', 'i18next', 'ValueHolder', 'ExpBoosterModifier', 'getPokemonNameWithAffix',
  `return function(){${expBody}}`,
);
class ValueHolder { constructor(value){ this.value=value; } }
class ExpBoosterModifier {}
const names = ['Bulbasaur', 'Silcoon'];
const nestedTrace = [];
let next;
while ((next = nestedTree.getNextPhase())) {
  if (next.phaseName === 'ExpPhase') {
    const slot = next.slot;
    const currentLevel = slot === 0 ? 12 : 14;
    const nextLevel = currentLevel + 1;
    const currentExp = totalExp(currentLevel, 3);
    const expValue = totalExp(nextLevel, 3) - currentExp;
    const globalSceneForPokemon = { getMaxExpLevel: () => 100 };
    const addExp = new Function('globalScene', 'getLevelTotalExp', `return function(exp,ignoreLevelCap=false){${addExpBody}};`)(
      globalSceneForPokemon, totalExp,
    );
    const pokemon = {
      name: names[slot], level: currentLevel, exp: currentExp, species: { growthRate: 3 },
      addExp,
      showExpGain(){ return Promise.resolve(); },
    };
    const globalScene = {
      applyModifiers(){}, phaseManager: nestedManager,
      ui: { showText: (_text, _delay, callback) => callback() },
    };
    const start = makeExpStart(globalScene, { t: () => '' }, ValueHolder, ExpBoosterModifier, mon => mon.name);
    start.call({
      partyMemberIndex: slot, expValue, getPlayerPokemon: () => pokemon, end(){},
    });
  } else if (next.phaseName === 'LevelUpPhase') {
    const slot = next.args[0];
    const sourceRows = slot === 0
      ? [[13, 'POISON_POWDER', 0], [13, 'SLEEP_POWDER', 0]]
      : [[15, 'BUG_BITE', 6]];
    const runEnd = new Function('globalScene', 'getBaseLearnableMoveSource', 'LearnableMoveSource', 'getPokemonNameWithAffix', 'allMoves', 'i18next', `return (${method});`)(
      { phaseManager: nestedManager },
      source => source - (source % 2),
      { PREVO: 6 },
      pokemon => names[pokemon.partyMemberIndex],
      { BUG_BITE: { name: 'Bug Bite' } },
      { t: (_key, options) => `${options.pokemonName}:${options.moveName}` },
    );
    runEnd.call({
      lastLevel: next.args[1], partyMemberIndex: slot, pokemon: { pauseEvolutions: true, partyMemberIndex: slot },
      getPokemon: () => ({ getLevelMoves: () => sourceRows }), getEvolution: () => null,
    });
  } else if (next.phaseName === 'LearnMovePhase') {
    nestedTrace.push(`learn:${next.slot}:${next.move}`);
  } else if (next.phaseName === 'MessagePhase') {
    nestedTrace.push(`message:${next.message}`);
  }
}
assert.deepEqual(nestedTrace, [
  'learn:0:POISON_POWDER', 'learn:0:SLEEP_POWDER', 'message:Silcoon:Bug Bite',
]);
const output = `# source=${pin}; executes ExpPhase.start, LevelUpPhase.end, PhaseManager queue methods and original PhaseTree FIFO; source applyPartyExp supplies EXP phase order\n# last_level,learnable_moves_in_source_order,phase_tree_learn_order,prevolution_message,two_member_nested_phase_order\n5,LEECH_SEED|GROWTH|POISON_POWDER|SLEEP_POWDER,${queued.join('|')},${prevoMessages[0]},${nestedTrace.join('|')}\n`;
const path = resolve('pokerogue-tests/fixtures/level-up-move-queue.csv');
if (process.argv.includes('--check')) assert.equal(readFileSync(path, 'utf8'), output);
else writeFileSync(path, output);
console.log('LevelUpPhase move queue fixture current.');
