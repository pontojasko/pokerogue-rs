#!/usr/bin/env node
// Execute the pinned LearnMovePhase.start method body with phase/UI boundaries stubbed.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const source = readFileSync(resolve(root, 'original/src/phases/learn-move-phase.ts'), 'utf8');
const pokemonSource = readFileSync(resolve(root, 'original/src/field/pokemon.ts'), 'utf8');
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
const startBody = body(source, 'public override start(): void {').replace('super.start();', 'this.superStart();');
const setMoveBody = body(pokemonSource, 'public setMove(moveIndex: number, moveId: MoveId): void {');
function execute(ids, newId, unimplemented = false) {
  const calls = [];
  const pokemon = { getMoveset: () => ids.map(moveId => ({ moveId })) };
  const ctx = {
    moveId: newId,
    superStart: () => calls.push('super'),
    getPokemon: () => pokemon,
    end: () => calls.push('end'),
    learnMove: (index) => calls.push(`learn:${index}`),
    replaceMoveCheck: () => calls.push('replace'),
  };
  const run = new Function('globalScene', 'EvolutionSceneUiHandler', 'UiMode', 'allMoves', 'MoveId', startBody);
  run.call(ctx, { ui: { getHandler: () => ({}), setMode: () => calls.push('mode') } }, class EvolutionSceneUiHandler {},
    { EVOLUTION_SCENE: 1, MESSAGE: 2 }, { [newId]: { id: newId, isUnimplemented: unimplemented } }, { SKETCH: 'SKETCH' });
  return calls.join('|');
}
const cases = [
  ['empty-slot', 'TACKLE|GROWL', 'VINE_WHIP', false, 'super|mode|learn:2'],
  ['duplicate-move', 'TACKLE|GROWL', 'GROWL', false, 'super|end'],
  ['full-moveset', 'TACKLE|GROWL|VINE_WHIP|LEECH_SEED', 'RAZOR_LEAF', false, 'super|mode|replace'],
  ['unimplemented', 'TACKLE', 'UNIMPLEMENTED_MOVE', true, 'super|end'],
];
const rows = [`# source=${pin}; executes LearnMovePhase.start and Pokemon.setMove bodies with narrow boundaries stubbed`,
  'case,known_moves,new_move,unimplemented,phase_calls'];
for (const [name, known, move, unimplemented, expected] of cases) {
  const actual = execute(known.split('|'), move, unimplemented);
  assert.equal(actual, expected, name);
  rows.push([name, known, move, Number(unimplemented), actual].join(','));
}
const setter = new Function('MoveId', 'PokemonMove', 'moveIndex', 'moveId', setMoveBody);
for (const [name, selected, incoming, expected] of [
  ['set-move', 2, 'BEAT_UP', 'TACKLE|GROWL|BEAT_UP|LEECH_SEED'],
  ['set-move-none', 2, 'NONE', 'TACKLE|GROWL|VINE_WHIP|LEECH_SEED'],
]) {
  const moveset = ['TACKLE', 'GROWL', 'VINE_WHIP', 'LEECH_SEED'].map(moveId => ({ moveId }));
  const ctx = { moveset, summonData: { moveset: moveset.slice() } };
  setter.call(ctx, { NONE: 'NONE' }, class PokemonMove { constructor(moveId) { this.moveId = moveId; } }, selected, incoming);
  const actual = ctx.moveset.map(move => move.moveId).join('|');
  assert.equal(actual, expected, name);
  assert.equal(ctx.summonData.moveset.map(move => move.moveId).join('|'), expected, `${name} summon data`);
  rows.push(`${name},${selected},${incoming},${actual}`);
}
const output = `${rows.join('\n')}\n`;
const path = resolve(root, 'pokerogue-tests/fixtures/learn-move-phase.csv');
if (process.argv.includes('--check')) assert.equal(readFileSync(path, 'utf8'), output);
else writeFileSync(path, output);
console.log('LearnMovePhase method fixture current.');
