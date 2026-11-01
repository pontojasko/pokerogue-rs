#!/usr/bin/env node
// Deterministically extract the prototype's content records from the pinned TS source.
import { readFileSync, readdirSync, writeFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const sourceRoot = resolve(root, 'original/src');
const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const wantedSpecies = ['BULBASAUR', 'CHARMANDER'];
const wantedMoves = ['VINE_WHIP', 'TACKLE', 'EMBER', 'SAND_ATTACK', 'COUNTER', 'QUICK_ATTACK', 'STRUGGLE'];
const speciesDir = resolve(sourceRoot, 'data/balance/species');
const speciesSources = readdirSync(speciesDir)
  .filter((file) => /^generation-\d+\.ts$/.test(file))
  .sort()
  .map((file) => readFileSync(resolve(speciesDir, file), 'utf8'));
const moveSource = readFileSync(resolve(sourceRoot, 'data/moves/move.ts'), 'utf8');
const townSource = readFileSync(resolve(sourceRoot, 'data/balance/biomes/town.ts'), 'utf8');
const speciesEnumSource = readFileSync(resolve(sourceRoot, 'enums/species-id.ts'), 'utf8');
const moveEnumSource = readFileSync(resolve(sourceRoot, 'enums/move-id.ts'), 'utf8');
const modifierTierEnumSource = readFileSync(resolve(sourceRoot, 'enums/modifier-tier.ts'), 'utf8');
const tmPoolTierSource = readFileSync(resolve(sourceRoot, 'data/balance/tm-pool-tiers.ts'), 'utf8');
const poolTierEnumSource = readFileSync(resolve(sourceRoot, 'enums/biome-pool-tier.ts'), 'utf8');
const timeOfDayEnumSource = readFileSync(resolve(sourceRoot, 'enums/time-of-day.ts'), 'utf8');
const arenaSource = readFileSync(resolve(sourceRoot, 'field/arena.ts'), 'utf8');
const expSource = readFileSync(resolve(sourceRoot, 'data/exp.ts'), 'utf8');
const poolTiers = ['COMMON', 'UNCOMMON', 'RARE', 'SUPER_RARE', 'ULTRA_RARE', 'BOSS', 'BOSS_RARE', 'BOSS_SUPER_RARE', 'BOSS_ULTRA_RARE'];
const timeBuckets = ['ALL', 'DAWN', 'DAY', 'DUSK', 'NIGHT'];

function enumIds(source, enumName) {
  const enumAt = source.indexOf(`export enum ${enumName}`);
  if (enumAt < 0) throw new Error(`${enumName} enum missing`);
  const bodyOpen = source.indexOf('{', enumAt);
  const body = balancedBlock(source, bodyOpen).replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/.*$/gm, '');
  const ids = new Map();
  let current = -1;
  for (const raw of body.split(',')) {
    const token = raw.trim();
    if (!token) continue;
    const match = token.match(/^([A-Z0-9_]+)(?:\s*=\s*(-?\d+))?$/);
    if (!match) throw new Error(`unsupported ${enumName} entry: ${token}`);
    current = match[2] === undefined ? current + 1 : Number(match[2]);
    ids.set(match[1], current);
  }
  return ids;
}

const speciesNumericIds = enumIds(speciesEnumSource, 'SpeciesId');
const moveNumericIds = enumIds(moveEnumSource, 'MoveId');
const modifierTierIds = enumIds(modifierTierEnumSource, 'ModifierTier');
const poolTierIds = enumIds(poolTierEnumSource, 'BiomePoolTier');
const timeOfDayIds = enumIds(timeOfDayEnumSource, 'TimeOfDay');
const growthRateIds = enumIds(expSource, 'GrowthRate');
const requiredId = (ids, id, kind) => {
  const numericId = ids.get(id);
  if (numericId === undefined) throw new Error(`${kind} numeric ID missing: ${id}`);
  return numericId;
};

function delimitedContents(source, open, left, right) {
  if (source[open] !== left) throw new Error(`expected ${left} at ${open}`);
  let depth = 0;
  let quote = null;
  let escaped = false;
  for (let i = open; i < source.length; i++) {
    const ch = source[i];
    if (quote) {
      if (escaped) escaped = false;
      else if (ch === '\\') escaped = true;
      else if (ch === quote) quote = null;
      continue;
    }
    if (ch === '"' || ch === "'" || ch === '`') quote = ch;
    else if (ch === left) depth++;
    else if (ch === right && --depth === 0) return source.slice(open + 1, i);
  }
  throw new Error(`unterminated TypeScript ${left}${right} block`);
}

function balancedBlock(source, open) {
  return delimitedContents(source, open, '{', '}');
}

function townEncounterPools() {
  const pokemonAt = townSource.indexOf('const pokemonPool:');
  const pokemonOpen = townSource.indexOf('{', pokemonAt);
  const pokemon = balancedBlock(townSource, pokemonOpen);
  const pools = [];
  for (const tierMatch of pokemon.matchAll(/\[BiomePoolTier\.([A-Z_]+)\]:\s*\{/g)) {
    const tier = tierMatch[1];
    const tierOpen = pokemon.indexOf('{', tierMatch.index);
    const tierBody = balancedBlock(pokemon, tierOpen);
    for (const timeMatch of tierBody.matchAll(/\[TimeOfDay\.([A-Z_]+)\]:\s*\[/g)) {
      const time = timeMatch[1];
      const listOpen = tierBody.indexOf('[', tierBody.indexOf(':', timeMatch.index) + 1);
      const list = delimitedContents(tierBody, listOpen, '[', ']');
      const speciesIds = [...list.matchAll(/SpeciesId\.([A-Z0-9_]+)/g)].map((m) => m[1]);
      pools.push({ tier, time, speciesIds });
    }
  }
  const expectedCount = poolTiers.length * timeBuckets.length;
  if (pools.length !== expectedCount) throw new Error(`Town pool buckets: expected ${expectedCount}, parsed ${pools.length}`);
  for (const tier of poolTiers) {
    for (const time of timeBuckets) {
      if (!pools.some((pool) => pool.tier === tier && pool.time === time)) {
        throw new Error(`Town pool bucket missing: ${tier}/${time}`);
      }
    }
  }
  return pools;
}

function sourceMethodBody(methodName) {
  const at = arenaSource.search(new RegExp(`(?:private|public) ${methodName}\\(`));
  if (at < 0) throw new Error(`Arena.${methodName} missing`);
  const open = arenaSource.indexOf('{', at);
  return balancedBlock(arenaSource, open);
}

function tierWeights(methodName, rollMax, isBoss) {
  const body = sourceMethodBody(methodName);
  const enumShim = Object.fromEntries(poolTiers.map((tier) => [tier, tier]));
  const original = new Function('BiomePoolTier', `return (tierValue) => { ${body} };`)(enumShim);
  const bands = [];
  let start = 0;
  let currentTier = original(0);
  for (let roll = 1; roll <= rollMax; roll++) {
    const nextTier = roll < rollMax ? original(roll) : null;
    if (nextTier !== currentTier) {
      bands.push({ is_boss: isBoss, tier: currentTier, roll_max: rollMax, min_roll: start, max_roll: roll - 1, weight: roll - start });
      start = roll;
      currentTier = nextTier;
    }
  }
  return bands;
}

function sourceTimePoolRule() {
  const body = sourceMethodBody('updatePoolsForTimeOfDay');
  const allAt = body.indexOf('pool[TimeOfDay.ALL]');
  const currentAt = body.indexOf('pool[timeOfDay]');
  if (allAt < 0 || currentAt < 0) throw new Error('Town time-pool merge condition was not recognized');
  return { always_bucket: 'ALL', current_bucket: 'timeOfDay', all_first: allAt < currentAt };
}

function sourceTierSelectionRule(normalRollMax, bossRollMax) {
  const override = arenaSource.match(/const overrideSpecies = ([^;]+);/);
  if (!override) throw new Error('Override-species condition was not recognized');
  const overrideAt = arenaSource.indexOf(override[0]);
  if (!/^\s*if \(overrideSpecies\)\s*\{\s*return overrideSpecies;/.test(arenaSource.slice(overrideAt + override[0].length))) {
    throw new Error('Override species no longer bypasses biome tier selection as expected');
  }
  const source = arenaSource.match(/const forcedTier = (getDailyForcedWaveBiomePoolTier\(waveIndex\));/);
  if (!source) throw new Error('Daily forced-tier source condition was not recognized');
  const forcedAt = arenaSource.indexOf(source[0]);
  const remainder = arenaSource.slice(forcedAt + source[0].length);
  if (!/^\s*if \(forcedTier === null\)/.test(remainder)) {
    throw new Error('Daily forced tier no longer guards random tier selection as expected');
  }
  const elseBranch = remainder.match(/\}\s*else\s*\{\s*tier = forcedTier;/);
  if (!elseBranch) throw new Error('Daily forced-tier override branch was not recognized');
  const luck = arenaSource.match(/const luckModifier = ([^;]+);/);
  if (!luck) throw new Error('Luck adjustment formula was not recognized');
  if (!arenaSource.includes('randSeedInt(rollMax - luckModifier)')) {
    throw new Error('Luck modifier no longer adjusts the tier roll bound as expected');
  }
  return {
    normal_roll_max: normalRollMax,
    boss_roll_max: bossRollMax,
    boss_table_condition: 'isBossSpecies',
    override_species_source: override[1],
    override_species_bypasses_tier: true,
    forced_tier_source: source[1],
    forced_tier_skips_roll: true,
    luck_modifier_formula: luck[1],
    effective_roll_max_formula: 'rollMax - luckModifier',
  };
}

function speciesRecord(id) {
  const assignmentForId = `SpeciesId.${id}] =`;
  const source = speciesSources.find((text) => text.includes(assignmentForId));
  if (!source) throw new Error(`species declaration missing: ${id}`);
  const at = source.indexOf(assignmentForId);
  const outerOpen = source.indexOf('{', at + assignmentForId.length);
  const outer = balancedBlock(source, outerOpen);
  const ctor = 'species: new PokemonSpecies({';
  const ctorAt = outer.indexOf(ctor);
  if (ctorAt < 0) throw new Error(`PokemonSpecies record missing: ${id}`);
  const objectOpen = outer.indexOf('{', ctorAt + ctor.length - 1);
  const body = balancedBlock(outer, objectOpen);
  const get = (key) => {
    const match = body.match(new RegExp(`\\b${key}:\\s*([^,\\n]+)`));
    if (!match) throw new Error(`${id}.${key} missing`);
    return match[1].trim();
  };
  const type = (key) => {
    const value = get(key);
    return value === 'null' ? null : value.replace('PokemonType.', '');
  };
  const levelMovesMatch = outer.match(/levelMoves:\s*\[/);
  if (!levelMovesMatch) throw new Error(`${id}.levelMoves missing`);
  const levelMovesOpen = outer.indexOf('[', levelMovesMatch.index);
  const levelMovesSource = delimitedContents(outer, levelMovesOpen, '[', ']');
  const levelMoves = [...levelMovesSource.matchAll(/\[\s*(\d+)\s*,\s*MoveId\.([A-Z0-9_]+)\s*\]/g)]
    .map((m) => ({ level: Number(m[1]), move_id: m[2] }));
  const tmMatch = outer.match(/\btms:\s*\[/);
  const tmMoves = tmMatch
    ? [...delimitedContents(outer, outer.indexOf('[', tmMatch.index), '[', ']').matchAll(/MoveId\.([A-Z0-9_]+)/g)].map((m) => m[1])
    : [];
  const evolutionMatch = outer.match(/\bevolutions:\s*\[/);
  const evolutionSource = evolutionMatch
    ? delimitedContents(outer, outer.indexOf('[', evolutionMatch.index), '[', ']')
    : '';
  const evolutions = [];
  for (const match of evolutionSource.matchAll(/new SpeciesEvolution\(\{/g)) {
    const open = evolutionSource.indexOf('{', match.index);
    const evolution = balancedBlock(evolutionSource, open);
    if (/evoFormKey:\s*SpeciesFormKey\.MEGA(?:_[A-Z]+)?\b/.test(evolution)) continue;
    const target = evolution.match(/\bspeciesId:\s*SpeciesId\.([A-Z0-9_]+)/);
    if (target) evolutions.push(target[1]);
  }
  return {
    id,
    numeric_id: requiredId(speciesNumericIds, id, 'SpeciesId'),
    name: id.toLowerCase().replace(/(^|_)([a-z])/g, (_, p, c) => `${p ? ' ' : ''}${c.toUpperCase()}`),
    type1: type('type1'), type2: type('type2'),
    hp: Number(get('baseHp')), attack: Number(get('baseAtk')),
    defense: Number(get('baseDef')), special_attack: Number(get('baseSpatk')),
    special_defense: Number(get('baseSpdef')), speed: Number(get('baseSpd')),
    base_exp: Number(get('baseExp')), growth_rate: get('growthRate').replace('GrowthRate.', ''),
    catch_rate: Number(get('catchRate')), base_friendship: Number(get('baseFriendship')),
    levelMoves: levelMoves.map((move) => ({ ...move, numeric_id: requiredId(moveNumericIds, move.move_id, 'MoveId') })),
    tmMoves: [...new Set(tmMoves)],
    evolutions,
  };
}

function originalExpThresholds() {
  const match = expSource.match(/const expLevels\s*=\s*\[/);
  if (!match) throw new Error('original expLevels table missing');
  const open = expSource.indexOf('[', match.index);
  const expLevels = new Function(`return [${delimitedContents(expSource, open, '[', ']')}];`)();
  const growthRate = Object.fromEntries([...growthRateIds].map(([name, value]) => [name, value]));
  const at = expSource.indexOf('export function getLevelTotalExp(');
  if (at < 0) throw new Error('getLevelTotalExp source missing');
  const body = balancedBlock(expSource, expSource.indexOf('{', at)).replace(/:\s*number\b/g, '');
  const getLevelTotalExp = new Function('expLevels', 'GrowthRate',
    `return function getLevelTotalExp(level, growthRate) { ${body} };`)(expLevels, growthRate);
  return [...growthRateIds].sort((a, b) => a[1] - b[1]).map(([name, id]) => ({
    name, id, values: Array.from({ length: 100 }, (_, index) => getLevelTotalExp(index + 1, id)),
  }));
}

function callArguments(source, start) {
  let depth = 1;
  let quote = null;
  let escaped = false;
  const args = [];
  let argStart = start;
  for (let i = start; i < source.length; i++) {
    const ch = source[i];
    if (quote) {
      if (escaped) escaped = false;
      else if (ch === '\\') escaped = true;
      else if (ch === quote) quote = null;
      continue;
    }
    if (ch === '"' || ch === "'" || ch === '`') quote = ch;
    else if (ch === '(') depth++;
    else if (ch === ')') {
      depth--;
      if (depth === 0) {
        args.push(source.slice(argStart, i).trim());
        return args;
      }
    } else if (ch === ',' && depth === 1) {
      args.push(source.slice(argStart, i).trim());
      argStart = i + 1;
    }
  }
  throw new Error('unterminated move constructor');
}

function moveRecord(id) {
  const re = new RegExp(`new (AttackMove|ChargingAttackMove|StatusMove|SelfStatusMove|ChargingSelfStatusMove)\\(MoveId\\.${id},`);
  const match = re.exec(moveSource);
  if (!match) throw new Error(`move declaration missing: ${id}`);
  const open = moveSource.indexOf('(', match.index);
  const args = callArguments(moveSource, open + 1);
  const nextMoveAt = moveSource.indexOf('\n    new ', match.index + match[0].length);
  const declarationTail = moveSource.slice(match.index, nextMoveAt < 0 ? moveSource.length : nextMoveAt);
  const attack = match[1].includes('AttackMove');
  const typeIndex = 1;
  return {
    id,
    numeric_id: requiredId(moveNumericIds, id, 'MoveId'),
    name: id.toLowerCase().replace(/(^|_)([a-z])/g, (_, p, c) => `${p ? ' ' : ''}${c.toUpperCase()}`),
    type: args[typeIndex].replace('PokemonType.', ''),
    special: attack && args[2] === 'MoveCategory.SPECIAL',
    status: !attack,
    power: attack ? Number(args[3]) : 0,
    accuracy: Number(args[attack ? 4 : 2]),
    pp: Number(args[attack ? 5 : 3]),
    priority: Number(args[attack ? 7 : 5]),
    is_unimplemented: declarationTail.includes('.unimplemented()'),
  };
}

const encounterPools = townEncounterPools();
const expThresholds = originalExpThresholds();
const encounterPool = encounterPools.find((pool) => pool.tier === 'COMMON' && pool.time === 'ALL').speciesIds;
const importedSpeciesIds = [...new Set([...wantedSpecies, ...encounterPools.flatMap((pool) => pool.speciesIds)])];
const species = importedSpeciesIds.map(speciesRecord);
const importedSpeciesSet = new Set(importedSpeciesIds);
const prevolutionRows = [];
for (const parent of species) {
  for (const childId of parent.evolutions) {
    if (importedSpeciesSet.has(childId)) {
      prevolutionRows.push({
        species_numeric_id: requiredId(speciesNumericIds, childId, 'SpeciesId'),
        prevolution_numeric_id: parent.numeric_id,
      });
    }
  }
}
const learnsetMoveIds = species.flatMap((record) => record.levelMoves.map((move) => move.move_id));
const tmMoveIds = species.flatMap((record) => record.tmMoves);
const importedMoveIds = [...new Set([...wantedMoves, ...learnsetMoveIds, ...tmMoveIds])];
const moves = importedMoveIds.map(moveRecord);
const importedTmMoveSet = new Set(tmMoveIds);
const tmTierRows = [...tmPoolTierSource.matchAll(/\[MoveId\.([A-Z0-9_]+)\]:\s*ModifierTier\.([A-Z0-9_]+)/g)]
  .filter((match) => importedTmMoveSet.has(match[1]))
  .map((match) => ({ move_id: match[1], move_numeric_id: requiredId(moveNumericIds, match[1], 'MoveId'), tier: match[2], tier_id: requiredId(modifierTierIds, match[2], 'ModifierTier') }));
const tmTierById = new Map(tmTierRows.map((row) => [row.move_id, row]));
for (const id of tmMoveIds) if (!tmTierById.has(id)) throw new Error(`TM tier missing: ${id}`);
const rollMaxMatch = arenaSource.match(/const rollMax = isBossSpecies \? (\d+) : (\d+);/);
if (!rollMaxMatch) throw new Error('Arena.randomSpecies rollMax condition was not recognized');
const bossRollMax = Number(rollMaxMatch[1]);
const nonBossRollMax = Number(rollMaxMatch[2]);
const tierWeightRows = [
  ...tierWeights('generateNonBossBiomeTier', nonBossRollMax, false),
  ...tierWeights('generateBossBiomeTier', bossRollMax, true),
].map((row) => ({ ...row, tier_id: requiredId(poolTierIds, row.tier, 'BiomePoolTier') }));
const timePoolRule = sourceTimePoolRule();
const tierSelectionRule = sourceTierSelectionRule(nonBossRollMax, bossRollMax);
const rustType = (t) => t === null ? 'None' : `Some(Element::${toPascal(t)})`;
const toPascal = (s) => s.toLowerCase().split('_').map((x) => x[0].toUpperCase() + x.slice(1)).join('');
const lines = [
  `// @generated by scripts/import_demo_data.mjs from PokéRogue ${pin}; do not edit.`,
  'use super::{',
  '    Element, GrowthRate, ImportedEncounterPool, ImportedLevelMove, ImportedMove, ImportedSpecies,',
  '    ImportedSpeciesPrevolution, ImportedSpeciesTm, ImportedTierSelectionRule, ImportedTierWeight,',
  '    ImportedTimePoolRule, ImportedTmTier,',
  '};',
  '#[rustfmt::skip]',
  'pub const IMPORTED_SPECIES: &[ImportedSpecies] = &[',
  ...species.map((s) => `    ImportedSpecies { id: "${s.id}", numeric_id: ${s.numeric_id}, name: "${s.name}", type1: Element::${toPascal(s.type1)}, type2: ${rustType(s.type2)}, base_hp: ${s.hp}, base_attack: ${s.attack}, base_defense: ${s.defense}, base_special_attack: ${s.special_attack}, base_special_defense: ${s.special_defense}, base_speed: ${s.speed}, base_exp: ${s.base_exp}, growth_rate: GrowthRate::${toPascal(s.growth_rate)}, catch_rate: ${s.catch_rate}, base_friendship: ${s.base_friendship} },`),
  '];',
  '#[rustfmt::skip]',
  'pub const IMPORTED_SPECIES_TMS: &[ImportedSpeciesTm] = &[',
  ...species.flatMap((s) => s.tmMoves.map((id, index) => `    ImportedSpeciesTm { species_numeric_id: ${s.numeric_id}, move_numeric_id: ${requiredId(moveNumericIds, id, 'MoveId')}, order: ${index} },`)),
  '];',
  '#[rustfmt::skip]',
  'pub const IMPORTED_SPECIES_PREVOLUTION: &[ImportedSpeciesPrevolution] = &[',
  ...prevolutionRows.map((row) => `    ImportedSpeciesPrevolution { species_numeric_id: ${row.species_numeric_id}, prevolution_numeric_id: ${row.prevolution_numeric_id} },`),
  '];',
  '#[rustfmt::skip]',
  'pub const IMPORTED_TM_TIERS: &[ImportedTmTier] = &[',
  ...tmTierRows.map((row) => `    ImportedTmTier { move_numeric_id: ${row.move_numeric_id}, tier_id: ${row.tier_id} },`),
  '];',
  '#[rustfmt::skip]',
  'pub const EXP_THRESHOLDS: [[u32; 100]; 6] = [',
  ...expThresholds.map((row) => `    [${row.values.join(', ')}],`),
  '];',
  '#[rustfmt::skip]',
  'pub const TOWN_ENCOUNTER_POOLS: &[ImportedEncounterPool] = &[',
  ...encounterPools.map((pool) => `    ImportedEncounterPool { tier_id: ${requiredId(poolTierIds, pool.tier, 'BiomePoolTier')}, tier: "${pool.tier}", time_id: ${requiredId(timeOfDayIds, pool.time, 'TimeOfDay')}, time_of_day: "${pool.time}", species: &[${pool.speciesIds.map((id) => `(${requiredId(speciesNumericIds, id, 'SpeciesId')}, "${id}")`).join(', ')}] },`),
  '];',
  '#[rustfmt::skip]',
  'pub const TOWN_COMMON_ALL_POOL: &[(u16, &str)] = &[',
  ...encounterPool.map((id) => `    (${requiredId(speciesNumericIds, id, 'SpeciesId')}, "${id}"),`),
  '];',
  '#[rustfmt::skip]',
  `pub const TOWN_TIME_POOL_RULE: ImportedTimePoolRule = ImportedTimePoolRule { always_include_bucket: "${timePoolRule.always_bucket}", current_time_bucket: "${timePoolRule.current_bucket}", always_bucket_first: ${timePoolRule.all_first} };`,
  '#[rustfmt::skip]',
  `pub const TOWN_TIER_SELECTION_RULE: ImportedTierSelectionRule = ImportedTierSelectionRule { normal_roll_max: ${tierSelectionRule.normal_roll_max}, boss_roll_max: ${tierSelectionRule.boss_roll_max}, boss_table_condition: "${tierSelectionRule.boss_table_condition}", override_species_source: "${tierSelectionRule.override_species_source}", override_species_bypasses_tier: ${tierSelectionRule.override_species_bypasses_tier}, forced_tier_source: "${tierSelectionRule.forced_tier_source}", forced_tier_skips_roll: ${tierSelectionRule.forced_tier_skips_roll}, luck_modifier_formula: "${tierSelectionRule.luck_modifier_formula}", effective_roll_max_formula: "${tierSelectionRule.effective_roll_max_formula}" };`,
  '#[rustfmt::skip]',
  'pub const TOWN_TIER_WEIGHTS: &[ImportedTierWeight] = &[',
  ...tierWeightRows.map((row) => `    ImportedTierWeight { boss_table: ${row.is_boss}, tier_id: ${row.tier_id}, tier: "${row.tier}", roll_max: ${row.roll_max}, min_roll: ${row.min_roll}, max_roll: ${row.max_roll}, weight: ${row.weight} },`),
  '];',
  '#[rustfmt::skip]',
  'pub const IMPORTED_LEVEL_MOVES: &[ImportedLevelMove] = &[',
  ...species.flatMap((s) => s.levelMoves.map((m) => `    ImportedLevelMove { species_id: "${s.id}", species_numeric_id: ${s.numeric_id}, level: ${m.level}, move_id: "${m.move_id}", move_numeric_id: ${m.numeric_id} },`)),
  '];',
  '#[rustfmt::skip]',
  'pub const IMPORTED_MOVES: &[ImportedMove] = &[',
  ...moves.map((m) => `    ImportedMove { id: "${m.id}", numeric_id: ${m.numeric_id}, name: "${m.name}", element: Element::${toPascal(m.type)}, power: ${m.power}, accuracy: ${m.accuracy}, pp: ${m.pp}, priority: ${m.priority}, is_special: ${m.special}, is_status: ${m.status}, is_unimplemented: ${m.is_unimplemented} },`),
  '];',
  '',
];
const csv = [
  '# Source fields: species(kind,id,numeric_id,name,type1,type2,hp,attack,defense,sp_attack,sp_defense,speed,base_exp,growth_rate,catch_rate,base_friendship); move(kind,id,numeric_id,name,type,category,power,accuracy,pp,priority,unimplemented); level_move(kind,species_id,species_numeric_id,level,move_id,move_numeric_id); pool_bucket(biome,tier,tier_id,time,time_id,count,id:numeric_id|...); tier_weight(table,tier,tier_id,roll_max,min,max,weight); condition(key,value)',
  ...species.map((s) => `species,${s.id},${s.numeric_id},${s.name},${s.type1},${s.type2 ?? ''},${s.hp},${s.attack},${s.defense},${s.special_attack},${s.special_defense},${s.speed},${s.base_exp},${s.growth_rate},${s.catch_rate},${s.base_friendship}`),
  ...species.flatMap((s) => s.levelMoves.map((m) => `level_move,${s.id},${s.numeric_id},${m.level},${m.move_id},${m.numeric_id}`)),
  ...species.flatMap((s) => s.tmMoves.map((id, index) => `species_tm,${s.id},${s.numeric_id},${index},${id},${requiredId(moveNumericIds, id, 'MoveId')}`)),
  ...prevolutionRows.map((row) => `prevolution,${row.species_numeric_id},${row.prevolution_numeric_id}`),
  ...tmTierRows.map((row) => `tm_tier,${row.move_id},${row.move_numeric_id},${row.tier},${row.tier_id}`),
  ...encounterPools.map((pool) => `pool_bucket,TOWN,${pool.tier},${requiredId(poolTierIds, pool.tier, 'BiomePoolTier')},${pool.time},${requiredId(timeOfDayIds, pool.time, 'TimeOfDay')},${pool.speciesIds.length},${pool.speciesIds.map((id) => `${id}:${requiredId(speciesNumericIds, id, 'SpeciesId')}`).join('|')}`),
  ...tierWeightRows.map((row) => `tier_weight,${row.is_boss ? 'BOSS' : 'NON_BOSS'},${row.tier},${row.tier_id},${row.roll_max},${row.min_roll},${row.max_roll},${row.weight}`),
  `condition,boss_table,${tierSelectionRule.boss_table_condition}`,
  `condition,override_species,${tierSelectionRule.override_species_source},bypasses_tier=${tierSelectionRule.override_species_bypasses_tier}`,
  `condition,forced_tier,${tierSelectionRule.forced_tier_source},skips_roll=${tierSelectionRule.forced_tier_skips_roll}`,
  `condition,luck_modifier,${tierSelectionRule.luck_modifier_formula},roll=${tierSelectionRule.effective_roll_max_formula}`,
  `condition,time_pool,${timePoolRule.always_bucket}+${timePoolRule.current_bucket},all_first=${timePoolRule.all_first}`,
  ...moves.map((m) => `move,${m.id},${m.numeric_id},${m.name},${m.type},${m.status ? 'STATUS' : m.special ? 'SPECIAL' : 'PHYSICAL'},${m.power},${m.accuracy},${m.pp},${m.priority},${m.is_unimplemented ? 1 : 0}`),
  '',
].join('\n');
const expCsv = [
  `# getLevelTotalExp executed from PokéRogue ${pin}.`,
  ...expThresholds.flatMap((row) => row.values.map((value, index) => `exp_threshold,${row.name},${row.id},${index + 1},${value}`)),
  '',
].join('\n');
const outputs = [
  [resolve(root, 'pokerogue-data/src/generated_data.rs'), lines.join('\n')],
  [resolve(root, 'pokerogue-tests/fixtures/imported-demo-data.csv'), csv],
  [resolve(root, 'pokerogue-tests/fixtures/exp-thresholds.csv'), expCsv],
];
if (process.argv.includes('--check')) {
  for (const [path, expected] of outputs) {
    if (!existsSync(path) || readFileSync(path, 'utf8') !== expected) {
      console.error(`${path.slice(root.length + 1)} is stale; run node scripts/import_demo_data.mjs`);
      process.exitCode = 1;
    }
  }
} else {
  for (const [path, contents] of outputs) writeFileSync(path, contents);
}
