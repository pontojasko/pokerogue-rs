#!/usr/bin/env node
// Execute PokemonLevelIncrementModifier.apply from the pinned source in an isolated harness.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve } from 'node:path';

const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const source = readFileSync('original/src/modifier/modifier.ts', 'utf8');
const pokemonSource = readFileSync('original/src/field/pokemon.ts', 'utf8');
function blockAfter(text, at) {
  const open = text.indexOf('{', at); assert.ok(open >= 0); let depth = 1, quote = null, escaped = false, line = false, comment = false;
  for (let i = open + 1; i < text.length; i++) {
    const c = text[i], next = text[i + 1];
    if (line) { if (c === '\n') line = false; continue; }
    if (comment) { if (c === '*' && next === '/') { comment = false; i++; } continue; }
    if (quote) { if (escaped) escaped = false; else if (c === '\\') escaped = true; else if (c === quote) quote = null; continue; }
    if (c === '/' && next === '/') { line = true; i++; continue; }
    if (c === '/' && next === '*') { comment = true; i++; continue; }
    if ('"\'`'.includes(c)) quote = c;
    else if (c === '{') depth++;
    else if (c === '}' && --depth === 0) return text.slice(open + 1, i);
  }
  throw new Error('unclosed source block');
}
const classAt = source.indexOf('export class PokemonLevelIncrementModifier'); assert.ok(classAt >= 0);
const classBody = blockAfter(source, source.indexOf('{', classAt));
const methodAt = classBody.indexOf('override apply('); assert.ok(methodAt >= 0);
const methodBody = blockAfter(classBody, classBody.indexOf('{', methodAt));
const friendshipAt = pokemonSource.indexOf('public addFriendship('); assert.ok(friendshipAt >= 0);
const friendshipBody = blockAfter(pokemonSource, pokemonSource.indexOf('{', friendshipAt));
const friendshipStarter = { friendship: 0 };
const friendshipScene = {
  gameMode: { isClassic: false },
  gameData: { starterData: { 1: friendshipStarter }, addStarterCandy: () => false },
  applyModifier() {}, validateAchv() {},
};
const sourceAddFriendship = new Function('globalScene','PokemonFriendshipBoosterModifier','NumberHolder','getStarterValueFriendshipCap','SpeciesId','activeOverrides','timedEventManager','awardRibbonsToSpeciesLine','achvs','RibbonData','RARE_CANDY_FRIENDSHIP_CAP','speciesDataRegistry',
  `${stripTypeScriptTypes(`function addFriendship(friendship, capped = false) {${friendshipBody}}`)}; return addFriendship;`)(
  friendshipScene, class {}, class NumberHolder { constructor(value) { this.value = value; } }, () => 20,
  { 1: 1 }, { IMMEDIATE_ADD_CANDY_OVERRIDE: 0 }, { getClassicFriendshipMultiplier: () => 1, areFusionsBoosted: () => false }, () => {}, { MAX_FRIENDSHIP: 1 }, { FRIENDSHIP: 1 }, 200, { getStarterCost: () => 10 },
);
const apply = new Function('globalScene', 'FRIENDSHIP_GAIN_FROM_RARE_CANDY', 'getLevelTotalExp', 'NumberHolder', 'LevelIncrementBoosterModifier',
  `${stripTypeScriptTypes(`function apply(playerPokemon, levelCount = new NumberHolder(1)) {${methodBody}}`)}; return apply;`)(
  { applyModifiers() {}, getMaxExpLevel: () => 100, phaseManager: { unshiftNew: (...args) => calls.push(args) }, getPlayerParty: () => [pokemon] },
  6, (level, growthRate) => thresholds.get(`${growthRate},${level}`), class NumberHolder { constructor(value) { this.value = value; } }, {},
);
const thresholdRows = readFileSync('pokerogue-tests/fixtures/exp-thresholds.csv', 'utf8').split('\n').filter(line => !line.startsWith('#'));
const thresholds = new Map(thresholdRows.map(line => { const [, growth, , level, exp] = line.split(','); return [`${growth},${level}`, Number(exp)]; }));
const rows = [`# source=${pin}; executed PokemonLevelIncrementModifier.apply and Pokemon.addFriendship; starter-candy/phase callbacks stubbed`];
for (const [growth, level] of [['MEDIUM_SLOW', 5], ['MEDIUM_SLOW', 25], ['MEDIUM_SLOW', 98], ['MEDIUM_SLOW', 99]]) {
  const exp = thresholds.get(`${growth},${level}`); assert.notEqual(exp, undefined);
  globalThis.calls = [];
  globalThis.pokemon = { level, exp, friendship: 50, species: { growthRate: growth, speciesId: 1, getRootSpeciesId: () => 1 }, isFusion: () => false, addFriendship(amount, capped) { sourceAddFriendship.call(this, amount, capped); } };
  const context = { applyModifiers() {}, getMaxExpLevel: () => 100, phaseManager: { unshiftNew: (...args) => calls.push(args) }, getPlayerParty: () => [pokemon] };
  apply.call(context, pokemon);
  rows.push(`${growth},${level},${exp},${pokemon.level},${pokemon.exp},${JSON.stringify(calls[0])}`);
}
for (const initial of [50, 194, 198, 200, 254]) {
  const pokemon = { friendship: initial, species: { speciesId: 1, getRootSpeciesId: () => 1 }, isFusion: () => false };
  sourceAddFriendship.call(pokemon, 6, true);
  rows.push(`friendship,${initial},6,1,${pokemon.friendship}`);
}
const fixture = resolve('pokerogue-tests/fixtures/rare-candy.csv'), output = `${rows.join('\n')}\n`;
if (process.argv.includes('--check')) assert.equal(readFileSync(fixture, 'utf8'), output); else writeFileSync(fixture, output);
console.log('Rare Candy oracle current.');
