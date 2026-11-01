#!/usr/bin/env node
// Execute pinned getMaxExpLevel and Pokemon.addExp with minimal Classic-mode context.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve } from 'node:path';

const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
function blockAfter(source, at) {
  const open = source.indexOf('{', at); assert.ok(open >= 0);
  let depth = 1, quote = null, escaped = false, line = false, comment = false;
  for (let i = open + 1; i < source.length; i++) {
    const c = source[i], n = source[i + 1];
    if (line) { if (c === '\n') line = false; continue; }
    if (comment) { if (c === '*' && n === '/') { comment = false; i++; } continue; }
    if (quote) { if (escaped) escaped = false; else if (c === '\\') escaped = true; else if (c === quote) quote = null; continue; }
    if (c === '/' && n === '/') { line = true; i++; continue; }
    if (c === '/' && n === '*') { comment = true; i++; continue; }
    if ('"\'`'.includes(c)) quote = c;
    else if (c === '{') depth++;
    else if (c === '}' && --depth === 0) return source.slice(open + 1, i);
  }
  throw new Error('unclosed source block');
}
function arrayLiteral(source, marker) {
  const start = source.indexOf('[', source.indexOf(marker)); assert.ok(start >= 0);
  let depth = 1, quote = null, escaped = false;
  for (let i = start + 1; i < source.length; i++) {
    const c = source[i];
    if (quote) { if (escaped) escaped = false; else if (c === '\\') escaped = true; else if (c === quote) quote = null; continue; }
    if ('"\'`'.includes(c)) quote = c;
    else if (c === '[') depth++;
    else if (c === ']' && --depth === 0) return source.slice(start, i + 1);
  }
  throw new Error('unclosed array');
}
const expSource = readFileSync('original/src/data/exp.ts', 'utf8');
const expLevels = arrayLiteral(expSource, 'const expLevels =');
const getTotalAt = expSource.indexOf('export function getLevelTotalExp');
const getTotalBody = blockAfter(expSource, getTotalAt);
const pokemonSource = readFileSync('original/src/field/pokemon.ts', 'utf8');
const addAt = pokemonSource.indexOf('public addExp('); assert.ok(addAt >= 0);
const addBody = blockAfter(pokemonSource, addAt);
const sceneSource = readFileSync('original/src/battle-scene.ts', 'utf8');
const capAt = sceneSource.indexOf('getMaxExpLevel(ignoreLevelCap = false)'); assert.ok(capAt >= 0);
const capBody = blockAfter(sceneSource, capAt);
const wrapper = stripTypeScriptTypes(`
const expLevels = ${expLevels};
const GrowthRate = { ERRATIC: 0, FAST: 1, MEDIUM_FAST: 2, MEDIUM_SLOW: 3, SLOW: 4, FLUCTUATING: 5 };
function getLevelTotalExp(level, growthRate) { ${getTotalBody} }
class Pokemon {
 constructor(level, exp, growthRate) { this.level=level; this.exp=exp; this.species={growthRate}; }
 addExp(exp, ignoreLevelCap=false) { ${addBody} }
}
class Scene { getMaxExpLevel(ignoreLevelCap=false) { ${capBody} } }
`);
const getCap = new Function('activeOverrides', 'globalScene', `${wrapper}; return (wave, override=0, ignore=false) => { const scene=Object.assign(new Scene(), globalScene, {currentBattle:{waveIndex:wave}}); activeOverrides.LEVEL_CAP_OVERRIDE=override; return scene.getMaxExpLevel(ignore); };`)(
  {}, { currentBattle: null, gameMode: { getWaveForDifficulty: wave => wave } },
);
const runAdd = new Function('globalScene', `${wrapper}; return (level, exp, amount, growth, cap) => { globalScene.getMaxExpLevel=()=>cap; const p=new Pokemon(level, exp, growth); p.addExp(amount); return [p.level,p.exp]; };`)({ getMaxExpLevel: () => 100 });
const rate = { MEDIUM_FAST: 2, MEDIUM_SLOW: 3 };
const rows = [`# source=${pin}; executes original getMaxExpLevel and Pokemon.addExp; Classic difficulty wave mapping`, '# scenario,wave,override,ignore,cap,growth,level,initial_exp,amount,final_level,final_exp'];
for (const [name, wave, override, ignore, growth, level, amountKind] of [
  ['wave-1-cap',1,0,false,'MEDIUM_FAST',5,'cross'],
  ['wave-10-cap',10,0,false,'MEDIUM_SLOW',5,'cross'],
  ['wave-11-cap',11,0,false,'MEDIUM_FAST',5,'cross'],
  ['wave-20-cap',20,0,false,'MEDIUM_SLOW',5,'cross'],
  ['override-cap',11,7,false,'MEDIUM_FAST',5,'cross'],
  ['ignore-cap',11,0,true,'MEDIUM_SLOW',5,'cross'],
  ['at-cap-clips-exp',1,0,false,'MEDIUM_FAST',5,'clip'],
  ['level-100-clips-exp',20,100,false,'MEDIUM_FAST',99,'clip'],
]) {
  const cap = getCap(wave, override, ignore);
  const initial = new Function(`${wrapper}; return getLevelTotalExp;`)()(level, rate[growth]);
  const threshold = new Function(`${wrapper}; return getLevelTotalExp;`)()(Math.min(level + 1, 99), rate[growth]);
  const amount = amountKind === 'clip'
    ? new Function(`${wrapper}; return getLevelTotalExp;`)()(cap, rate[growth])
    : threshold - initial;
  const [finalLevel, finalExp] = runAdd(level, initial, amount, rate[growth], cap);
  rows.push(`${name},${wave},${override},${+ignore},${cap},${growth},${level},${initial},${amount},${finalLevel},${finalExp}`);
}
const output = `${rows.join('\n')}\n`, path = resolve('pokerogue-tests/fixtures/exp-cap.csv');
if (process.argv.includes('--check')) assert.equal(readFileSync(path, 'utf8'), output); else writeFileSync(path, output);
console.log('Experience cap oracle current.');
