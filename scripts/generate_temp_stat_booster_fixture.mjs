#!/usr/bin/env node
// Execute pinned TempStatStageBoosterModifier.apply and lapse/reset methods.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const source = readFileSync(resolve(root, 'original/src/modifier/modifier.ts'), 'utf8');
function body(marker) {
  const at = source.indexOf(marker); assert.ok(at >= 0, `missing ${marker}`);
  const open = source.indexOf('{', at); let depth = 1;
  for (let i = open + 1; i < source.length; i++) {
    if (source[i] === '{') depth++;
    else if (source[i] === '}' && --depth === 0) return source.slice(open + 1, i);
  }
  throw new Error(`unterminated ${marker}`);
}
const apply = new Function('statLevel', `const that=this; ${body('override apply(_tempBattleStat: TempBattleStat, statLevel: NumberHolder): boolean {')} `);
const lapse = new Function(`const that=this; ${body('public lapse(..._args: unknown[]): boolean {')}`);
const reset = new Function(`const that=this; ${body('resetBattleCount(): void {')}`);
const rows = [`# source=${pin}; executes TempStatStageBoosterModifier.apply and LapsingPersistentModifier lapse/reset`];
for (const [stat, boost] of [['ATK', 0.2], ['DEF', 0.2], ['SPATK', 0.2], ['SPDEF', 0.2], ['SPD', 0.2], ['ACC', 1]]) {
  const holder = { value: 2 };
  const applied = apply.call({ boost }, holder);
  const modifier = { battleCount: 2, maxBattles: 5 };
  const oneRemains = lapse.call(modifier);
  const expires = lapse.call(modifier);
  modifier.battleCount = 1;
  reset.call(modifier);
  rows.push(`${stat},${boost},2,${holder.value},${applied},${oneRemains},${expires},${modifier.battleCount}`);
}
const output = `${rows.join('\n')}\n`;
const path = resolve(root, 'pokerogue-tests/fixtures/temp-stat-booster.csv');
if (process.argv.includes('--check')) assert.equal(readFileSync(path, 'utf8'), output); else writeFileSync(path, output);
console.log('Temp stat booster source fixture current.');
