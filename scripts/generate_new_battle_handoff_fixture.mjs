#!/usr/bin/env node
// Execute the pinned NewBattlePhase handoff and BattleScene.getNewBattleProps default path.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve } from 'node:path';

const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const phaseSource = readFileSync('original/src/phases/new-battle-phase.ts', 'utf8');
const sceneSource = readFileSync('original/src/battle-scene.ts', 'utf8');
function methodBody(source, marker) {
  const start = source.indexOf(marker); assert.ok(start >= 0, `missing ${marker}`);
  const open = source.indexOf('{', start); let depth = 1, quote = null, escaped = false, line = false, block = false;
  for (let end = open + 1; end < source.length; end++) {
    const c = source[end], n = source[end + 1];
    if (line) { if (c === '\n') line = false; continue; }
    if (block) { if (c === '*' && n === '/') { block = false; end++; } continue; }
    if (quote) { if (escaped) escaped = false; else if (c === '\\') escaped = true; else if (c === quote) quote = null; continue; }
    if (c === '/' && n === '/') { line = true; end++; continue; }
    if (c === '/' && n === '*') { block = true; end++; continue; }
    if (`"'\``.includes(c)) { quote = c; continue; }
    if (c === '{') depth++;
    else if (c === '}' && --depth === 0) return source.slice(open + 1, end);
  }
  throw new Error(`unclosed ${marker}`);
}
const phaseBody = methodBody(phaseSource, 'start() {').replace('super.start();', '');
const phaseMethod = stripTypeScriptTypes(`function phaseStart(){${phaseBody}}`);
const nextPropsBody = methodBody(sceneSource, 'private getNewBattleProps(');
const nextPropsMethod = stripTypeScriptTypes(`function getNewBattleProps(fromSession){${nextPropsBody}}`);
const makeNextProps = new Function('BattleType', 'STARTING_WAVE', 'activeOverrides', `${nextPropsMethod};return getNewBattleProps;`)({ WILD:'WILD' }, 1, {});
const run = wave => {
  const calls = [];
  const scene = { currentBattle: { waveIndex: wave }, phaseManager: { removeAllPhasesOfType: name => calls.push(`remove:${name}`) }, newBattle: () => calls.push('newBattle') };
  const phase = { end: () => calls.push('end') };
  const start = new Function('globalScene', `return (${phaseMethod});`)(scene);
  start.call(phase);
  const next = makeNextProps.call(scene, undefined);
  return `${wave},${next.waveIndex},${next.battleType},${calls.join('|')}`;
};
const rows = [`# source=${pin}; executes NewBattlePhase.start and BattleScene.getNewBattleProps default path`, '# current_wave,next_wave,battle_type,phase_calls'];
for (let wave = 1; wave <= 20; wave++) rows.push(run(wave));
const output = `${rows.join('\n')}\n`;
const path = resolve('pokerogue-tests/fixtures/new-battle-handoff.csv');
if (process.argv.includes('--check')) assert.equal(readFileSync(path, 'utf8'), output);
else writeFileSync(path, output);
console.log('NewBattle handoff source oracle current.');
