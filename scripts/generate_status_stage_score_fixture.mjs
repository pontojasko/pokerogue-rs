#!/usr/bin/env node
// Execute the pinned StatStageChangeAttr scoring method for imported stage moves.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve } from 'node:path';

const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const source = readFileSync('original/src/data/moves/move.ts', 'utf8');
function bodyAfter(text, marker) {
  const at = text.indexOf(marker); assert.ok(at >= 0, `missing ${marker}`);
  const open = text.indexOf('{', at); let depth = 1, quote = null, escaped = false;
  for (let i = open + 1; i < text.length; i++) {
    const c = text[i];
    if (quote) { if (escaped) escaped = false; else if (c === '\\') escaped = true; else if (c === quote) quote = null; continue; }
    if ('"\'`'.includes(c)) quote = c;
    else if (c === '{') depth++;
    else if (c === '}' && --depth === 0) return text.slice(open + 1, i);
  }
  throw new Error(`unclosed ${marker}`);
}
const cls = bodyAfter(source, 'export class StatStageChangeAttr');
const method = cls.slice(cls.indexOf('  getTargetBenefitScore('));
const scoreBody = bodyAfter(method, 'getTargetBenefitScore(user: Pokemon, target: Pokemon, _move: Move): number {');
const ScoreAttr = new Function('Stat', 'MoveCategory', `return class { constructor(stats, stages) { this.stats=stats; this.stages=stages; this.selfTarget=false; } getLevels() { return this.stages; } getTargetBenefitScore(user,target,_move) { ${scoreBody} } };`)(
  { ATK: 'ATK', DEF: 'DEF', SPATK: 'SPATK', SPDEF: 'SPDEF' }, { PHYSICAL: 'PHYSICAL' });
const rows = [`# source=${pin}; executed StatStageChangeAttr.getTargetBenefitScore; enemy user targets player; status-only moveset`, '# move,stat,target_stage,source_target_score,enemy_ai_score'];
for (const [move, stat] of [['GROWL','ATK'], ['TAIL_WHIP','DEF'], ['LEER','DEF']]) {
  for (const stage of [-6, 0, 5]) {
    const attr = new ScoreAttr([stat], -1);
    const user = { getMoveset: () => [] };
    const target = { getStatStage: () => stage };
    const raw = attr.getTargetBenefitScore(user, target, {});
    const ai = raw === 0 ? -20 : -raw;
    rows.push(`${move},${stat},${stage},${raw},${ai}`);
  }
}
const output = `${rows.join('\n')}\n`, path = resolve('pokerogue-tests/fixtures/status-stage-score.csv');
if (process.argv.includes('--check')) assert.equal(readFileSync(path, 'utf8'), output); else writeFileSync(path, output);
console.log('Status-stage scoring oracle current.');
