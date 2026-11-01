#!/usr/bin/env node
// Execute the pinned Arena.randomSpecies and RNG helper bodies in a controlled Town scenario.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const original = resolve(root, 'original/src');
const arenaSource = readFileSync(resolve(original, 'field/arena.ts'), 'utf8');
const commonSource = readFileSync(resolve(original, 'utils/common.ts'), 'utf8');
const dataRows = readFileSync(resolve(root, 'pokerogue-tests/fixtures/imported-demo-data.csv'), 'utf8')
  .split(/\r?\n/).filter((line) => line && !line.startsWith('#')).map((line) => line.split(','));
const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const tierNames = ['COMMON', 'UNCOMMON', 'RARE', 'SUPER_RARE', 'ULTRA_RARE', 'BOSS', 'BOSS_RARE', 'BOSS_SUPER_RARE', 'BOSS_ULTRA_RARE'];
const timeNames = ['ALL', 'DAWN', 'DAY', 'DUSK', 'NIGHT'];

function delimited(source, open, left = '{', right = '}') {
  let depth = 0;
  let quote = null;
  let escaped = false;
  for (let i = open; i < source.length; i++) {
    const ch = source[i];
    if (quote) {
      if (escaped) escaped = false;
      else if (ch === '\\') escaped = true;
      else if (ch === quote) quote = null;
    } else if (ch === '"' || ch === "'" || ch === '`') quote = ch;
    else if (ch === left) depth++;
    else if (ch === right && --depth === 0) return source.slice(open + 1, i);
  }
  throw new Error(`unterminated source block ${left}${right}`);
}

function methodBody(source, marker) {
  const at = source.indexOf(marker);
  if (at < 0) throw new Error(`source method missing: ${marker}`);
  const open = source.indexOf('{', at);
  return delimited(source, open);
}

function helperBody(source, marker) {
  const at = source.indexOf(marker);
  if (at < 0) throw new Error(`source helper missing: ${marker}`);
  return methodBody(source, marker);
}

function uint32(value) {
  return Math.trunc(value) >>> 0;
}

class PhaserAlea {
  constructor(seed) {
    this.c = 1;
    this.n = 0xefc8249d;
    this.s0 = this.hash(' ');
    this.s1 = this.hash(' ');
    this.s2 = this.hash(' ');
    this.s0 = (this.s0 - this.hash(seed) + 1) % 1;
    this.s1 = (this.s1 - this.hash(seed) + 1) % 1;
    this.s2 = (this.s2 - this.hash(seed) + 1) % 1;
  }

  hash(text) {
    for (let i = 0; i < text.length; i++) {
      this.n += text.charCodeAt(i);
      let h = 0.02519603282416938 * this.n;
      this.n = uint32(h);
      h -= this.n;
      h *= this.n;
      this.n = uint32(h);
      h -= this.n;
      this.n += h * 4294967296;
    }
    return uint32(this.n) / 4294967296;
  }

  rnd() {
    const t = 2091639 * this.s0 + this.c * (1 / 4294967296);
    this.c = Math.trunc(t);
    this.s0 = this.s1;
    this.s1 = this.s2;
    this.s2 = t - this.c;
    return this.s2;
  }

  frac() {
    const first = this.rnd();
    const lowBits = Math.floor(this.rnd() * 2097152);
    return first + lowBits * (1 / 9007199254740992);
  }

  integerInRange(min, max) {
    return Math.floor(this.frac() * (max - min + 1) + min);
  }
}

function makePhaser(seed, calls, prefixes = { frac: 'F', integer: 'I', real: 'R' }) {
  const rng = new PhaserAlea(seed);
  const phaser = { Math: {
    Clamp: (value, min, max) => Math.min(Math.max(value, min), max),
    RND: {
      frac: () => { const value = rng.frac(); calls.push(`${prefixes.frac}:${value}`); return value; },
      realInRange: (min, max) => { const value = min + rng.frac() * (max - min); calls.push(`${prefixes.real}:${value}`); return value; },
      integerInRange: (min, max) => { const value = rng.integerInRange(min, max); calls.push(`${prefixes.integer}:${max-min+1}:${min}:${value}`); return value; },
      pick: items => items[phaser.Math.RND.integerInRange(0, items.length - 1)],
    },
  } };
  return phaser;
}

const shiftSeed = (seed, shift) => Array.from(seed, (ch) => String.fromCharCode(ch.charCodeAt(0) + shift)).join('');
const officialRngRows = readFileSync(resolve(root, 'pokerogue-tests/fixtures/phaser-rng.csv'), 'utf8')
  .split(/\r?\n/).filter((line) => line && !line.startsWith('#')).map((line) => line.split(',').map(Number))
  .filter(([turn]) => turn === 1);
const checkRng = new PhaserAlea(shiftSeed('ParitySeed000000', 1 << 6));
for (const [, range, min, expected] of officialRngRows) {
  assert.equal(checkRng.integerInRange(min, min + range - 1), expected, 'Phaser Alea harness must match official fixture');
}

const pools = new Map();
const speciesByNumericId = new Map();
const speciesIdNumeric = new Map();
for (const row of dataRows) {
  if (row[0] === 'species') {
    speciesByNumericId.set(Number(row[2]), { speciesId: Number(row[2]), name: row[3] });
    speciesIdNumeric.set(row[1], Number(row[2]));
  }
  if (row[0] === 'pool_bucket') {
    pools.set(`${row[2]}/${row[4]}`, row[7] ? row[7].split('|').map((entry) => Number(entry.split(':')[1])) : []);
  }
}
const currentTime = 'DAY';
const sourcePoolData = Object.fromEntries(tierNames.map((tier, tierId) => [tierId,
  Object.fromEntries(timeNames.map((time, timeId) => [timeId - 1, pools.get(`${tier}/${time}`) ?? []]))]));
const updatePoolsBody = methodBody(arenaSource, 'public updatePoolsForTimeOfDay(')
  .replace(/\s*tier satisfies `\$\{BiomePoolTier\}`;/, '')
  .replace('{} as Writable<ArenaPokemonPools>', '{}');
const updatePoolsForTime = new Function('allBiomes', 'TimeOfDay',
  `return function() { ${updatePoolsBody} };`)(
  { get: () => ({ pokemonPool: sourcePoolData }) },
  { ALL: -1, DAWN: 0, DAY: 1, DUSK: 2, NIGHT: 3 },
);
const poolsByTime = new Map();
for (const time of ['DAWN', 'DAY', 'DUSK', 'NIGHT']) {
  const arenaForTime = { biomeId: 'TOWN', lastTimeOfDay: null,
    getTimeOfDay: () => ({ DAWN: 0, DAY: 1, DUSK: 2, NIGHT: 3 })[time], pokemonPool: {} };
  updatePoolsForTime.call(arenaForTime);
  poolsByTime.set(time, arenaForTime.pokemonPool);
}
const pokemonPool = poolsByTime.get(currentTime);
const speciesIdNames = Object.fromEntries([...speciesIdNumeric].map(([name, id]) => [id, name]));
const tierEnum = {};
tierNames.forEach((name, id) => { tierEnum[name] = id; tierEnum[id] = name; });
const biomeEnum = { TOWN: 1, END: 999 };
const timeTierFn = (name) => new Function('BiomePoolTier', `return function(tierValue) { ${methodBody(arenaSource, `private ${name}(`)} };`)(tierEnum);
const generateNonBossBiomeTier = timeTierFn('generateNonBossBiomeTier');
const generateBossBiomeTier = timeTierFn('generateBossBiomeTier');
const enumShim = { BiomePoolTier: tierEnum, BiomeId: biomeEnum };
const tierMethodBody = methodBody(arenaSource, 'public randomSpecies(')
  .replace('let tier: BiomePoolTier;', 'let tier;')
  .replace('let species: PokemonSpecies;', 'let species;');
const randSeedIntBody = helperBody(commonSource, 'export function randSeedInt(');
const randSeedItemBody = helperBody(commonSource, 'export function randSeedItem<');
const seed = 'TownWaveOneParitySeed';
const rng = new PhaserAlea(seed);
const rngCalls = [];
const Phaser = { Math: { RND: {
  integerInRange: (min, max) => {
    const value = rng.integerInRange(min, max);
    rngCalls.push({ min, max, value });
    return value;
  },
  pick: (items) => items[Phaser.Math.RND.integerInRange(0, items.length - 1)],
} } };
const randSeedInt = new Function('Phaser', `return function randSeedInt(range, min = 0) { ${randSeedIntBody} };`)(Phaser);
const randSeedItem = new Function('Phaser', `return function randSeedItem(items) { ${randSeedItemBody} };`)(Phaser);
const scene = {
  gameMode: {
    getOverrideSpecies: () => null,
    isClassic: false,
    isWaveFinal: () => false,
    getWaveForDifficulty: (wave) => wave,
  },
  getEncounterBossSegments: () => 0,
  randomSpecies: () => { throw new Error('unexpected pool fallback'); },
};
const registry = {
  getSpecies: (id) => {
    const base = speciesByNumericId.get(Number(id));
    if (!base) throw new Error(`species not imported: ${id}`);
    return { ...base, getWildSpeciesForLevel: () => Number(id) };
  },
};
const tierMethod = new Function(
  'BiomePoolTier', 'BiomeId', 'globalScene', 'getDailyForcedWaveBiomePoolTier', 'randSeedInt', 'randSeedItem', 'speciesDataRegistry',
  `return function randomSpecies(waveIndex, level, attempt = 0, luckValue = 0, isBoss) { ${tierMethodBody} };`,
)(tierEnum, biomeEnum, scene, () => null, randSeedInt, randSeedItem, registry);
const arena = {
  biomeId: biomeEnum.TOWN,
  pokemonPool,
  generateBossBiomeTier,
  generateNonBossBiomeTier,
  checkLegendBST: () => false,
};
const oldLog = console.log;
console.log = () => {};
let result;
try {
  result = tierMethod.call(arena, 1, 2);
} finally {
  console.log = oldLog;
}

const tierDraw = rngCalls[0].value;
const tier = generateNonBossBiomeTier(tierDraw);
const candidates = pokemonPool[tier];
const indexDraw = candidates.length > 1 ? rngCalls[1]?.value : null;
const chosenIndex = indexDraw ?? 0;
const chosenNumericId = candidates[chosenIndex];
assert.equal(Number(result.speciesId), chosenNumericId, 'executed Arena.randomSpecies result must match captured draws');
assert.equal(rngCalls.length, candidates.length > 1 ? 2 : 1);
function bandRows(method, max, boss) {
  const fn = method;
  const values = Array.from({ length: max }, (_, i) => fn(i));
  const rows = [];
  let start = 0;
  for (let i = 1; i <= values.length; i++) {
    if (i === values.length || values[i] !== values[start]) {
      rows.push(`weight,${boss ? 'BOSS' : 'NON_BOSS'},${tierNames[values[start]]},${values[start]},${max},${start},${i - 1},${i - start}`);
      start = i;
    }
  }
  return rows;
}
const poolIds = (tier, time) => pools.get(`${tier}/${time}`) ?? [];
const combined = pokemonPool[tier];
const rows = [
  `scenario,${pin},${seed},1,2,TOWN,${currentTime},NON_BOSS,0,512,${tierDraw},${tierNames[tier]},${tier},${candidates.length},${candidates.map((id) => `${speciesIdNames[id]}:${id}`).join('|')},${chosenIndex},${speciesIdNames[chosenNumericId]},${chosenNumericId},${rngCalls.map(({ min, max, value }) => `${max - min + 1}:${min}:${value}`).join('|')}`,
  ...['DAWN', 'DAY', 'DUSK', 'NIGHT'].flatMap((time) => tierNames.map((name, tierId) => {
    const all = poolIds(name, 'ALL');
    const timed = poolIds(name, time);
    const merged = poolsByTime.get(time)[tierId];
    return `time_pool,${time},${tierId},${all.map((id) => `${speciesIdNames[id]}:${id}`).join('|')},${time},${timed.map((id) => `${speciesIdNames[id]}:${id}`).join('|')},COMBINED,${merged.map((id) => `${speciesIdNames[id]}:${id}`).join('|')}`;
  })),
  ...bandRows(generateNonBossBiomeTier, 512, false),
  ...bandRows(generateBossBiomeTier, 64, true),
];

// A second, shared-stream fixture executes Battle.getLevelForWave before Arena.randomSpecies,
// matching NewBattle's level preparation followed by EncounterPhase's wild generation.
const battleSource = readFileSync(resolve(original, 'battle.ts'), 'utf8');
const pokemonSource = readFileSync(resolve(original, 'field/pokemon.ts'), 'utf8');
const entrySeed = 'TownWaveEntrySeed0001';
const entryWaveSeed = shiftSeed(entrySeed, 1);
const levelConstructorCalls = [];
const arenaCalls = [];
const entryLevelPhaser = makePhaser(shiftSeed(entryWaveSeed, 1 << 3), levelConstructorCalls, { frac: 'F', integer: 'C', real: 'R' });
const entryArenaPhaser = makePhaser(entryWaveSeed, arenaCalls, { frac: 'unused', integer: 'I', real: 'unused' });
const randSeedIntFor = Phaser => new Function('Phaser', `return function(range,min=0){${randSeedIntBody}};`)(Phaser);
const randomStringBody = helperBody(commonSource, 'export function randomString(');
const makeRandomString = randSeedInt => new Function('randSeedInt', `return function(length,seeded=false){${randomStringBody}};`)(randSeedInt);
const entryRandSeedIntForConstructor = randSeedIntFor(entryLevelPhaser);
const entryBattleSeed = makeRandomString(entryRandSeedIntForConstructor)(16, true);
assert.equal(entryBattleSeed.length, 16);
const entryRandSeedFloat = new Function('Phaser',
  `return function randSeedFloat() { ${helperBody(commonSource, 'export function randSeedFloat(')} };`)(entryLevelPhaser);
const gaussBody = methodBody(battleSource, 'randSeedGaussForLevel(value:');
const gauss = new Function('randSeedFloat', `return function(value) { ${gaussBody} };`)(entryRandSeedFloat);
const levelBody = methodBody(battleSource, 'getLevelForWave(): number');
const getLevelForWave = new Function('Phaser', 'isDailyFinalBoss', 'DAILY_BOSS_LEVEL',
  `return function() { ${levelBody} };`)(entryLevelPhaser, () => false, 200);
const battleMock = { waveIndex: 1, gameMode: {
  getWaveForDifficulty: (wave) => wave,
  isBoss: () => false,
  isWaveFinal: () => false,
}, randSeedGaussForLevel: gauss };
const entryLevel = getLevelForWave.call(battleMock);
const entryRandSeedInt = randSeedIntFor(entryArenaPhaser);
const entryRandSeedItem = new Function('Phaser',
  `return function(items) { ${randSeedItemBody} };`)(entryArenaPhaser);
const entryTierMethod = new Function(
  'BiomePoolTier', 'BiomeId', 'globalScene', 'getDailyForcedWaveBiomePoolTier', 'randSeedInt', 'randSeedItem', 'speciesDataRegistry',
  `return function(waveIndex, level, attempt = 0, luckValue = 0, isBoss) { ${tierMethodBody} };`,
)(tierEnum, biomeEnum, scene, () => null, entryRandSeedInt, entryRandSeedItem, registry);
const entryArena = {
  biomeId: biomeEnum.TOWN,
  pokemonPool: poolsByTime.get(currentTime),
  generateBossBiomeTier,
  generateNonBossBiomeTier,
  checkLegendBST: () => false,
};
let entryResult;
console.log = () => {};
try {
  entryResult = entryTierMethod.call(entryArena, 1, entryLevel);
} finally {
  console.log = oldLog;
}
const entryCalls = [...levelConstructorCalls, ...arenaCalls];
const entryTierRoll = Number(entryCalls.find((call) => call.startsWith('I:'))?.split(':')[3]);
const entryTier = generateNonBossBiomeTier(entryTierRoll);
const entryCandidates = entryArena.pokemonPool[entryTier];
const entrySpeciesId = Number(entryResult.speciesId);
const entryMoveRows = dataRows.filter((row) => row[0] === 'level_move' && Number(row[3]) <= entryLevel && Number(row[2]) === entrySpeciesId);
const entryMoveIds = [...new Set(entryMoveRows.map((row) => Number(row[5])))];
const entryMoves = entryMoveIds.map((id) => dataRows.find((row) => row[0] === 'move' && Number(row[2]) === id)?.[1]).filter(Boolean);
const entrySpeciesRow = dataRows.find((row) => row[0] === 'species' && Number(row[2]) === entrySpeciesId);
const statBody = methodBody(pokemonSource, 'calculateStats(): void');
class NumberHolder { constructor(value) { this.value = value; } }
const calculateStats = new Function(
  'PERMANENT_STATS', 'Stat', 'NumberHolder', 'globalScene', 'PokemonIncrementingStatModifier',
  'PokemonNatureWeightModifier', 'getNatureStatMultiplier', 'AbilityId', 'Phaser',
  `return function() { ${statBody} };`,
)([0, 1, 2, 3, 4, 5], { HP: 0 }, NumberHolder,
  { applyModifier: () => {} }, {}, {}, () => 1, { WONDER_GUARD: 0 }, entryArenaPhaser);
const statHolder = {
  level: entryLevel,
  ivs: [0, 0, 0, 0, 0, 0],
  hp: undefined,
  stats: undefined,
  calculateBaseStats: () => entrySpeciesRow.slice(6, 12).map(Number),
  isPlayer: () => false,
  hasAbility: () => false,
  getNature: () => 0,
  setStat: (stat, value) => { statHolder.stats[stat] = value; },
};
calculateStats.call(statHolder);
const expValueBody = methodBody(pokemonSource.replace(/\/\/.*$/gm, ''), 'getExpValue(): number');
const getExpValue = new Function(`return function() { ${expValueBody} };`)();
const enemyExpValue = getExpValue.call({ level: entryLevel,
  getSpeciesForm: () => ({ getBaseExp: () => Number(entrySpeciesRow[12]) }) });
rows.push(`entry_scenario,${pin},${entrySeed},1,${entryLevel},TOWN,${currentTime},${entryTierRoll},${tierNames[entryTier]},${entrySpeciesRow[1]},${entrySpeciesId},${entryCandidates.indexOf(entrySpeciesId)},${entryMoves.join('|')},${entryMoves.slice(0, 2).join('|')},${statHolder.stats.join('|')},${entryCalls.join('|')},${enemyExpValue},${entryBattleSeed}`);
for (let wave = 1; wave <= 20; wave++) {
  const runSeed = 'MiniRunSeed0000';
  const runWaveSeed = shiftSeed(runSeed, wave);
  const runLevelCalls = [];
  const runArenaCalls = [];
  const runLevelPhaser = makePhaser(shiftSeed(runWaveSeed, wave << 3), runLevelCalls, { frac: 'F', integer: 'C', real: 'R' });
  const runArenaPhaser = makePhaser(runWaveSeed, runArenaCalls, { frac: 'unused', integer: 'I', real: 'unused' });
  const runConstructorRandInt = randSeedIntFor(runLevelPhaser);
  const runBattleSeed = makeRandomString(runConstructorRandInt)(16, true);
  const runFloat = new Function('Phaser', `return function() { ${helperBody(commonSource, 'export function randSeedFloat(')} };`)(runLevelPhaser);
  const runGauss = new Function('randSeedFloat', `return function(value) { ${gaussBody} };`)(runFloat);
  const runGetLevel = new Function('Phaser', 'isDailyFinalBoss', 'DAILY_BOSS_LEVEL', `return function() { ${levelBody} };`)(runLevelPhaser, () => false, 200);
  const runBattle = { waveIndex: wave, gameMode: {
    getWaveForDifficulty: (index) => index, isBoss: () => wave % 10 === 0, isWaveFinal: () => wave === 200,
  }, randSeedGaussForLevel: runGauss };
  const level = runGetLevel.call(runBattle);
  const runInt = randSeedIntFor(runArenaPhaser);
  const runItem = new Function('Phaser', `return function(items) { ${randSeedItemBody} };`)(runArenaPhaser);
  const runRandomSpecies = new Function(
    'BiomePoolTier', 'BiomeId', 'globalScene', 'getDailyForcedWaveBiomePoolTier', 'randSeedInt', 'randSeedItem', 'speciesDataRegistry',
    `return function(waveIndex, level, attempt = 0, luckValue = 0, isBoss) { ${tierMethodBody} };`,
  )(tierEnum, biomeEnum, scene, () => null, runInt, runItem, registry);
  const runArena = { biomeId: biomeEnum.TOWN, pokemonPool: poolsByTime.get(currentTime),
    generateBossBiomeTier, generateNonBossBiomeTier, checkLegendBST: () => false };
  console.log = () => {};
  let species;
  try { species = runRandomSpecies.call(runArena, wave, level, 0, 0, wave % 10 === 0); }
  finally { console.log = oldLog; }
  const runCalls = [...runLevelCalls, ...runArenaCalls];
  const tierRoll = Number(runCalls.find((call) => call.startsWith('I:'))?.split(':')[3]);
  const tierId = generateNonBossBiomeTier(tierRoll);
  const chosen = Number(species.speciesId);
  const pool = runArena.pokemonPool[tierId];
  const speciesRow = dataRows.find((row) => row[0] === 'species' && Number(row[2]) === chosen);
  const learned = [...new Set(dataRows.filter((row) => row[0] === 'level_move' && Number(row[2]) === chosen && Number(row[3]) <= level).map((row) => Number(row[5])))];
  const moveIds = learned.map((id) => dataRows.find((row) => row[0] === 'move' && Number(row[2]) === id)?.[1]).filter(Boolean);
  const statCalc = new Function('PERMANENT_STATS', 'Stat', 'NumberHolder', 'globalScene',
    'PokemonIncrementingStatModifier', 'PokemonNatureWeightModifier', 'getNatureStatMultiplier', 'AbilityId', 'Phaser',
    `return function() { ${statBody} };`)([0,1,2,3,4,5], {HP:0}, NumberHolder, {applyModifier:()=>{}}, {}, {}, ()=>1, {WONDER_GUARD:0}, runArenaPhaser);
  const stats = { level, ivs:[0,0,0,0,0,0], hp:undefined, stats:undefined,
    calculateBaseStats:()=>speciesRow.slice(6,12).map(Number), isPlayer:()=>false,
    hasAbility:()=>false, getNature:()=>0, setStat:(stat,value)=>{stats.stats[stat]=value;} };
  statCalc.call(stats);
  const exp = getExpValue.call({level, getSpeciesForm:()=>({getBaseExp:()=>Number(speciesRow[12])})});
  rows.push(`run_wave,${pin},${runSeed},${wave},${level},${currentTime},${tierRoll},${tierNames[tierId]},${speciesRow[1]},${chosen},${pool.indexOf(chosen)},${moveIds.join('|')},${moveIds.slice(0,2).join('|')},${stats.stats.join('|')},${runCalls.join('|')},${exp},${runBattleSeed}`);
}
const outputPath = resolve(root, 'pokerogue-tests/fixtures/town-encounter.csv');
const output = `# Generated by executing pinned Battle.getLevelForWave, Pokemon.calculateStats, Arena.randomSpecies, updatePoolsForTimeOfDay, tier methods and RNG helpers with a Phaser-Alea harness verified against phaser-rng.csv.\n${rows.join('\n')}\n`;
if (process.argv.includes('--check')) {
  assert.equal(readFileSync(outputPath, 'utf8'), output, 'Town encounter fixture is stale');
  console.log('Town encounter oracle fixture is current.');
} else {
  writeFileSync(outputPath, output);
  console.log('Wrote deterministic Town Arena.randomSpecies fixture.');
}
