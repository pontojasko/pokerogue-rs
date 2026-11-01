#!/usr/bin/env node
// Run pinned getNewModifierTypeOption with the source-executed wave-one common pool weights.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const source = readFileSync(resolve(root, 'original/src/modifier/modifier-type.ts'), 'utf8');
const poolFixture = readFileSync(resolve(root, 'pokerogue-tests/fixtures/modifier-pools.csv'), 'utf8');
const weightsFixture = readFileSync(resolve(root, 'pokerogue-tests/fixtures/common-rewards.csv'), 'utf8');
function body(text, marker) {
  const at = text.indexOf(marker); assert.ok(at >= 0, `missing ${marker}`);
  const open = text.indexOf('{', at); let depth = 1, quote = null, escaped = false;
  for (let i = open + 1; i < text.length; i++) {
    const ch = text[i];
    if (quote) { if (escaped) escaped = false; else if (ch === '\\') escaped = true; else if (ch === quote) quote = null; }
    else if (ch === '"' || ch === "'" || ch === '`') quote = ch;
    else if (ch === '{') depth++;
    else if (ch === '}' && --depth === 0) return text.slice(open + 1, i);
  }
  throw new Error(`unterminated ${marker}`);
}

class Alea {
  constructor(seed) {
    this.c = 1; this.n = 0xefc8249d;
    this.s0 = this.hash(' '); this.s1 = this.hash(' '); this.s2 = this.hash(' ');
    this.s0 = (this.s0 - this.hash(seed) + 1) % 1;
    this.s1 = (this.s1 - this.hash(seed) + 1) % 1;
    this.s2 = (this.s2 - this.hash(seed) + 1) % 1;
  }
  hash(text) {
    for (let i = 0; i < text.length; i++) {
      this.n += text.charCodeAt(i); let h = 0.02519603282416938 * this.n;
      this.n = Math.trunc(h) >>> 0; h -= this.n; h *= this.n;
      this.n = Math.trunc(h) >>> 0; h -= this.n; this.n += h * 4294967296;
    }
    return (this.n >>> 0) / 4294967296;
  }
  rnd() {
    const t = 2091639 * this.s0 + this.c * (1 / 4294967296);
    this.c = Math.trunc(t); this.s0 = this.s1; this.s1 = this.s2; this.s2 = t - this.c; return this.s2;
  }
  frac() { const first = this.rnd(); return first + Math.floor(this.rnd() * 2097152) * (1 / 9007199254740992); }
  int(range) { return Math.floor(this.frac() * range); }
}
const shiftSeed = (seed, wave) => Array.from(seed, (c) => String.fromCharCode(c.charCodeAt(0) + wave)).join('');
const commonWeightRow = weightsFixture.split(/\r?\n/).find((line) => line.startsWith('common,1,1,5,-,'));
assert.ok(commonWeightRow, 'source-executed common pool scenario missing');
const weights = commonWeightRow.split(',')[6].split('|').map(Number);
const ids = poolFixture.split(/\r?\n/).filter((line) => line.startsWith('modifier,COMMON,'))
  .map((line) => line.split(',')[4]);
assert.equal(ids.length, weights.length);
const thresholdMap = {}; let total = 0;
weights.forEach((weight, index) => { if (weight) { total += weight; thresholdMap[total] = index; } });
const cumulative = Object.fromEntries(Object.entries(thresholdMap));
const groupOf = (id) => ({ POKEBALL: 'pb', LURE: 'lure', POTION: 'potion', SUPER_POTION: 'potion', BERRY: 'berry', TM_COMMON: 'tm' })[id];
const pool = Array.from({ length: 5 }, (_, tier) => tier === 0
  ? ids.map((id) => ({ modifierType: { id, name: id, group: groupOf(id), tier } }))
  : [{ modifierType: { id: `UNUSED_${tier}`, name: `UNUSED_${tier}`, group: `UNUSED_${tier}`, tier } }]);
const thresholds = { 0: cumulative, 1: { 1: 0 }, 2: { 1: 0 }, 3: { 1: 0 }, 4: { 1: 0 } };
const marker = 'function getNewModifierTypeOption(';
let implementation = body(source, marker)
  .replace('let thresholds: object;', 'let thresholds;')
  .replace('let tier: ModifierTier;', 'let tier;')
  .replace('let modifierType: ModifierType | null', 'let modifierType')
  .replace('let index: number | undefined;', 'let index;')
  .replace('(modifierType as ModifierTypeGenerator)', 'modifierType')
  .replace('.at(-1)!', '.at(-1)')
  .replace('modifierType as ModifierType', 'modifierType')
  .replace('upgradeCount!', 'upgradeCount');
const generated = new Function(
  'ModifierPoolType', 'ModifierTier', 'getModifierPoolForType', 'modifierPoolThresholds', 'enemyModifierPoolThresholds',
  'enemyBuffModifierPoolThresholds', 'dailyStarterModifierPoolThresholds', 'getPartyLuckValue', 'randSeedInt',
  'ModifierTypeGenerator', 'ModifierTypeOption', 'ignoredPoolIndexes',
  `return function getNewModifierTypeOption(party,poolType,tier,upgradeCount,retryCount=0,allowLuckUpgrades=true) { ${implementation} };`,
);
const retryBody = body(source, 'function getModifierTypeOptionWithRetry(').replace('return candidate!;', 'return candidate;');
const makeRetry = new Function('BooleanHolder', 'ChallengeType', 'applyChallenges', 'getNewModifierTypeOption', 'ModifierPoolType',
  `return function(existingOptions,retryCount,party,tier,allowLuckUpgrades) { ${retryBody} };`);

const runSeed = 'MiniRunSeed0000';
const rows = [`# source=${pin}; executed getNewModifierTypeOption/getModifierTypeOptionWithRetry; source COMMON weights; natural PLAYER candidate and 3-option offer`];
for (const wave of [1]) {
  const seed = shiftSeed(runSeed, wave); const rng = new Alea(seed); const draws = [];
  const randSeedInt = (range, min = 0) => { const value = min + rng.int(range); draws.push({ range, min, value }); return value; };
  class ModifierTypeGenerator {}
  class ModifierTypeOption { constructor(type, upgrades) { this.type = type; this.upgradeCount = upgrades; } }
  const invoke = generated(
    { PLAYER: 0, WILD: 1, TRAINER: 2, ENEMY_BUFF: 3, DAILY_STARTER: 4 },
    { COMMON: 0, GREAT: 1, ULTRA: 2, ROGUE: 3, MASTER: 4 },
    () => pool, thresholds, {}, {}, {}, () => 0, randSeedInt, ModifierTypeGenerator, ModifierTypeOption,
    { 0: [], 1: [], 2: [], 3: [], 4: [] },
  );
  const oldLog = console.log; console.log = () => {};
  let option;
  try { option = invoke([], 0, undefined, undefined, 0, true); }
  finally { console.log = oldLog; }
  assert.equal(option.type.id, 'POKEBALL', 'scenario must choose a non-generated common modifier');
  rows.push(`candidate,${wave},${runSeed},${draws.map((d) => `${d.range}:${d.min}:${d.value}`).join('|')},${option.type.id},${option.upgradeCount},0,${ids.indexOf(option.type.id)}`);
}
const generatedIds = new Set(['TEMP_STAT_STAGE_BOOSTER', 'BERRY', 'TM_COMMON']);
let foundOffer = false;
for (let seedIndex = 0; seedIndex < 256 && !foundOffer; seedIndex++) {
 for (const wave of [1,2,3,4,5,6,7,8,9,10]) {
  const offerRunSeed = seedIndex === 0 ? runSeed : `NaturalOfferSeed${String(seedIndex).padStart(4, '0')}`;
  const seed = shiftSeed(offerRunSeed, wave); const rng = new Alea(seed); const draws = [];
  const randSeedInt = (range, min = 0) => { const value = min + rng.int(range); draws.push({ range, min, value }); return value; };
  class ModifierTypeGenerator {}
  class ModifierTypeOption { constructor(type, upgrades) { this.type = type; this.upgradeCount = upgrades; } }
  const newCandidate = generated(
    { PLAYER: 0, WILD: 1, TRAINER: 2, ENEMY_BUFF: 3, DAILY_STARTER: 4 },
    { COMMON: 0, GREAT: 1, ULTRA: 2, ROGUE: 3, MASTER: 4 },
    () => pool, thresholds, {}, {}, {}, () => 0, randSeedInt, ModifierTypeGenerator, ModifierTypeOption,
    { 0: [], 1: [], 2: [], 3: [], 4: [] },
  );
  const getOption = makeRetry(class { constructor(value) { this.value = value; } }, { WAVE_REWARD: 0 }, () => {}, newCandidate, { PLAYER: 0 });
  const options = [];
  const oldLog = console.log; console.log = () => {};
  try { for (let i = 0; i < 3; i++) options.push(getOption(options, 15, [], undefined, true)); }
  finally { console.log = oldLog; }
  if (options.some((option) => generatedIds.has(option.type.id) || option.type.id.startsWith('UNUSED_'))) continue;
  rows.push(`offer,${wave},${offerRunSeed},${options.map((option) => `${option.type.id}:${option.upgradeCount}:${option.type.tier}:${ids.indexOf(option.type.id)}`).join('|')},${draws.map((d) => `${d.range}:${d.min}:${d.value}`).join('|')}`);
  foundOffer = true; break;
 }
}
assert.ok(foundOffer, 'no deterministic offer avoiding unmodeled generated/higher-tier entries was found');
const output = `${rows.join('\n')}\n`;
const path = resolve(root, 'pokerogue-tests/fixtures/natural-reward-candidate.csv');
if (process.argv.includes('--check')) assert.equal(readFileSync(path, 'utf8'), output);
else writeFileSync(path, output);
console.log('Natural reward candidate oracle current.');
