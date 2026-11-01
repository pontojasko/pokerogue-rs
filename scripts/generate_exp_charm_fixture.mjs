import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const root = resolve(import.meta.dirname, '..');
const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const victory = readFileSync(resolve(root, 'original/src/phases/victory-phase.ts'), 'utf8').replace(/\/\/.*$/gm, '');
const modifier = readFileSync(resolve(root, 'original/src/modifier/modifier.ts'), 'utf8').replace(/\/\/.*$/gm, '');
const modifierTypes = readFileSync(resolve(root, 'original/src/modifier/modifier-type.ts'), 'utf8');
const boostPercent = Number(modifierTypes.match(/EXP_CHARM: \(\) => new ExpBoosterModifierType\([^\n]*, (\d+)\)/)?.[1]);
assert.equal(boostPercent, 25, 'EXP_CHARM percentage changed in the pinned source');
const superBoostPercent = Number(modifierTypes.match(/SUPER_EXP_CHARM: \(\) => new ExpBoosterModifierType\([^\n]*, (\d+)\)/)?.[1]);
assert.equal(superBoostPercent, 60, 'SUPER_EXP_CHARM percentage changed in the pinned source');
const selectCondition = victory.match(/if\s*\((currentWaveIndex % 10)\)/)?.[1];
const specialCondition = victory.match(/if\s*\((gameMode\.isClassic && currentWaveIndex === 10)\)/)?.[1];
const periodicCondition = victory.match(/if\s*\((currentWaveIndex <= 750 && \(currentWaveIndex <= 500 \|\| currentWaveIndex % 30 === superExpWave\))\)/)?.[1];
const superCharmCondition = victory.match(/currentWaveIndex % 30 !== superExpWave \|\| currentWaveIndex > 250/)?.[0];
assert.ok(selectCondition && specialCondition && periodicCondition && superCharmCondition, 'missing VictoryPhase reward schedule conditions');
function body(source, signature) {
  const start = source.indexOf(signature);
  assert.notEqual(start, -1, `missing method ${signature}`);
  const open = source.indexOf('{', start);
  let depth = 0;
  for (let i = open; i < source.length; i++) {
    if (source[i] === '{') depth++;
    if (source[i] === '}' && --depth === 0) return source.slice(open + 1, i);
  }
  throw new Error(`unclosed method ${signature}`);
}
const applyBody = body(modifier, 'override apply(boost: NumberHolder): boolean');
const expCharmClass = modifier.slice(
  modifier.indexOf('export class ExpBoosterModifier'),
  modifier.indexOf('export class PokemonExpBoosterModifier'),
);
const maxBody = body(expCharmClass, 'getMaxStackCount(_forThreshold?: boolean): number');
const sourceApply = new Function('boost', 'stack', `return function() { ${applyBody} };`);
const sourceMax = new Function('stack', `return function() { ${maxBody} };`);
const isSelectWave = new Function('currentWaveIndex', `return Boolean(${selectCondition});`);
const isSpecialWave = new Function('gameMode', 'currentWaveIndex', `return Boolean(${specialCondition});`);
const hasPeriodicReward = new Function('currentWaveIndex', 'superExpWave', `return Boolean(${periodicCondition});`);
const isNormalCharm = new Function('currentWaveIndex', 'superExpWave', `return Boolean(${superCharmCondition});`);
const rows = [`# source=${pin}; extracted and executed VictoryPhase selection/fixed reward conditions plus ExpBoosterModifier.apply/getMaxStackCount; Classic waves 1-20`];
let normalStacks = 0, superStacks = 0;
for (let wave = 1; wave <= 20; wave++) {
  const naturalOffer = isSelectWave(wave);
  const special = !naturalOffer && isSpecialWave({ isClassic: true }, wave) ? 1 : 0;
  const periodic = !naturalOffer && hasPeriodicReward(wave, 20);
  const periodicNormal = periodic && isNormalCharm(wave, 20) ? 1 : 0;
  const periodicSuper = periodic && !periodicNormal ? 1 : 0;
  const normalGranted = special + periodicNormal;
  const superGranted = periodicSuper;
  const input = 257;
  const holder = { value: input };
  sourceApply(holder, normalStacks).call({ boostMultiplier: 0.25, getStackCount: () => normalStacks });
  sourceApply(holder, superStacks).call({ boostMultiplier: 0.6, getStackCount: () => superStacks });
  const normalCap = sourceMax(normalStacks).call({ boostMultiplier: 0.25 });
  const superCap = sourceMax(superStacks).call({ boostMultiplier: 0.6 });
  rows.push(`schedule,${wave},${Number(naturalOffer)},${normalGranted},${superGranted},${normalStacks},${superStacks},${boostPercent},${superBoostPercent},${input},${holder.value},${normalCap},${superCap}`);
  normalStacks = Math.min(normalStacks + normalGranted, 99);
  superStacks = Math.min(superStacks + superGranted, 30);
}
const path = resolve(root, 'pokerogue-tests/fixtures/exp-charm.csv');
const output = `${rows.join('\n')}\n`;
if (process.argv.includes('--check')) {
  assert.equal(readFileSync(path, 'utf8'), output, 'EXP Charm fixture stale');
  console.log('EXP Charm oracle fixture is current.');
} else {
  writeFileSync(path, output);
  console.log('Wrote EXP Charm source-executed fixture.');
}
