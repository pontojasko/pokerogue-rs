import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
const root = new URL('../', import.meta.url);
const source = readFileSync(new URL('original/src/modifier/modifier.ts', root), 'utf8');
function body(marker) {
  const at = source.indexOf(marker); assert.ok(at >= 0, `missing ${marker}`);
  const open = source.indexOf('{', at); let depth = 1;
  for (let i = open + 1; i < source.length; i++) {
    if (source[i] === '{') depth++;
    if (source[i] === '}' && --depth === 0) return source.slice(open + 1, i);
  }
  throw new Error(`unterminated ${marker}`);
}
const lapseSource = stripTypeScriptTypes(`function lapse(..._args: unknown[]): boolean {${body('public lapse(..._args: unknown[]): boolean {')}}`);
const resetSource = stripTypeScriptTypes(`function resetBattleCount(): void {${body('resetBattleCount(): void {')}}`);
const lapse = new Function(`return (${lapseSource});`)();
const reset = new Function(`return (${resetSource});`)();
const rows = ['# source=7c0881255af5cd51992e513ff93c2f71adbe40e0; executes LapsingPersistentModifier.lapse/resetBattleCount; BattleEndPhase lapse point source-inspected'];
for (const initial of [10, 6, 2, 1]) {
  const state = { battleCount: initial, maxBattles: 10 };
  const active = lapse.call(state);
  rows.push(`lapse,${initial},${state.battleCount},${Number(active)}`);
}
const refreshed = { battleCount: 6, maxBattles: 10 };
reset.call(refreshed);
rows.push(`refresh,6,${refreshed.battleCount}`);
const output = `${rows.join('\n')}\n`;
const path = new URL('pokerogue-tests/fixtures/lure.csv', root);
if (process.argv.includes('--check')) assert.equal(readFileSync(path, 'utf8'), output);
else writeFileSync(path, output);
console.log('Lure lapse oracle verified.');
