#!/usr/bin/env node
// Execute the pinned one-usable-move AI path and stat-stage multiplier method.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve } from 'node:path';

const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const source = readFileSync('original/src/field/pokemon.ts', 'utf8');
const moveSource = readFileSync('original/src/data/moves/move.ts', 'utf8');
function bodyAfter(text, marker) {
  const at = text.indexOf(marker); assert.ok(at >= 0, `missing ${marker}`);
  const open = text.indexOf('{', at); let depth = 1, quote = null, escaped = false, line = false, comment = false;
  for (let i = open + 1; i < text.length; i++) {
    const c = text[i], n = text[i + 1];
    if (line) { if (c === '\n') line = false; continue; }
    if (comment) { if (c === '*' && n === '/') { comment = false; i++; } continue; }
    if (quote) { if (escaped) escaped = false; else if (c === '\\') escaped = true; else if (c === quote) quote = null; continue; }
    if (c === '/' && n === '/') { line = true; i++; continue; }
    if (c === '/' && n === '*') { comment = true; i++; continue; }
    if ('"\'`'.includes(c)) quote = c;
    else if (c === '{') depth++;
    else if (c === '}' && --depth === 0) return text.slice(open + 1, i);
  }
  throw new Error(`unclosed source body ${marker}`);
}

const moveBody = stripTypeScriptTypes(`function getNextMove() {${bodyAfter(source, 'getNextMove(): TurnMove {')}}`);
const getNextMove = new Function('MoveUseMode', `${moveBody}; return getNextMove;`)({ NORMAL: 0 });
const mock = {
  summonData: { moveQueue: [] },
  getMoveQueue: () => [],
  getMoveset: () => [{ moveId: 'HARDEN', isUsable: () => [true] }],
  getNextTargets: () => [2],
};
const selected = getNextMove.call(mock);
assert.equal(selected.move, 'HARDEN');

assert.match(moveSource, /new SelfStatusMove\(MoveId\.HARDEN, PokemonType\.NORMAL, -1, 30, -1, 0, 1\)\s*\/\/\s*\.attr\(StatStageChangeAttr, \[Stat\.DEF\], 1, true\)/);
const attrClass = bodyAfter(moveSource, 'export class StatStageChangeAttr');
const changeBody = bodyAfter(attrClass, 'apply(user: Pokemon, target: Pokemon, move: Move, args?: any[]): boolean {');
const groupSource = readFileSync('original/src/utils/stat-change.ts', 'utf8');
const groupBody = stripTypeScriptTypes(`function groupStatChange(stats, stages) {${bodyAfter(groupSource, 'export function groupStatChange(')}}`);
const groupStatChange = new Function(`${groupBody}; return groupStatChange;`)();
const applyChange = new Function('globalScene', 'groupStatChange', `
  class MoveEffectAttr { constructor(selfTarget) { this.selfTarget = selfTarget; } apply() { return true; } }
  class StatStageChangeAttr extends MoveEffectAttr {
    constructor() { super(true); this.stats = ['DEF']; this.stages = 1; this.options = { condition: () => true }; }
    getMoveChance() { return -1; }
    getLevels() { return this.stages; }
    apply(user, target, move, args) { ${changeBody} }
  }
  return new StatStageChangeAttr();
`)(
  { phaseManager: { unshiftNew: (phase, data) => { globalThis.phaseEvent = { phase, battlerIndex: data.battlerIndex, changes: data.changes }; } } },
  groupStatChange,
);
let sourceRandomCalls = 0;
const applied = applyChange.apply(
  { getBattlerIndex: () => 1, randBattleSeedInt: () => { sourceRandomCalls++; return 0; } },
  { getBattlerIndex: () => 2 },
  { id: 'HARDEN' },
);
assert.equal(applied, true);
assert.deepEqual(globalThis.phaseEvent, { phase: 'StatStageChangePhase', battlerIndex: 1, changes: [{ stat: 'DEF', stages: 1 }] });
assert.equal(sourceRandomCalls, 0);

const stageBody = stripTypeScriptTypes(`function getStatStageMultiplier(stat, opponent, move, ignoreOppAbility = false, isCritical = false, simulated = true, ignoreHeldItems = false) {${bodyAfter(source, 'getStatStageMultiplier(\n    stat: EffectiveStat')}}`);
const getStatStageMultiplier = new Function(
  'NumberHolder', 'BooleanHolder', `${stageBody}; return getStatStageMultiplier;`,
)(class NumberHolder { constructor(value) { this.value = value; } }, class BooleanHolder { constructor(value) { this.value = value; } });
const rows = [`# source=${pin}; executed Pokemon.getNextMove, StatStageChangeAttr.apply, groupStatChange, getStatStageMultiplier; mocked phase queue; no modifiers/abilities/opponent`,
  `move,${selected.move},${selected.targets.join('|')},${selected.useMode}`,
  `effect,HARDEN,${globalThis.phaseEvent.phase},${globalThis.phaseEvent.battlerIndex},${globalThis.phaseEvent.changes[0].stat},${globalThis.phaseEvent.changes[0].stages},${sourceRandomCalls}`];
for (const stage of [-6, -2, -1, 0, 1, 2, 6]) {
  const value = getStatStageMultiplier.call({ getStatStage: () => stage }, 2, undefined, undefined, false, false, true, true);
  rows.push(`stage,${stage},${value}`);
}
const fixture = resolve('pokerogue-tests/fixtures/harden.csv'), output = `${rows.join('\n')}\n`;
if (process.argv.includes('--check')) assert.equal(readFileSync(fixture, 'utf8'), output); else writeFileSync(fixture, output);
console.log('Harden oracle current.');
