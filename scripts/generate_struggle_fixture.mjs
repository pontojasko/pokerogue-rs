#!/usr/bin/env node
// Execute the pinned getNextMove no-usable-moves return object with wrappers.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const source = readFileSync(resolve(root, 'original/src/field/pokemon.ts'), 'utf8');
const marker = '// No moves left means struggle';
const start = source.indexOf(marker);
assert.ok(start >= 0, 'getNextMove Struggle fallback exists');
const tail = source.slice(start, start + 500);
const match = tail.match(/return\s*(\{\s*move:\s*MoveId\.STRUGGLE,[\s\S]*?useMode:\s*MoveUseMode\.IGNORE_PP,?\s*\});/);
assert.ok(match, 'source fallback return object');
const object = match[1].replace('this.getNextTargets(MoveId.STRUGGLE)', 'getNextTargets(MoveId.STRUGGLE)');
const getNextMoveFallback = new Function('MoveId', 'MoveUseMode', 'getNextTargets', `return (${object});`);
const result = getNextMoveFallback({ STRUGGLE: 165 }, { IGNORE_PP: 'IGNORE_PP' }, move => `targets:${move}`);
assert.equal(result.move, 165);
assert.equal(result.useMode, 'IGNORE_PP');
const moveSource = readFileSync(resolve(root, 'original/src/data/moves/move.ts'), 'utf8');
const declaration = moveSource.match(/new AttackMove\(MoveId\.STRUGGLE, PokemonType\.NORMAL, MoveCategory\.PHYSICAL, (\d+), (-?\d+), (\d+), (-?\d+), (-?\d+), (\d+)\)\s*\.attr\(RecoilAttr, true, ([\d.]+), true\)\s*\.attr\(TypelessAttr\)/);
assert.ok(declaration, 'source Struggle move declaration');
const output = `# source=${pin}; executed getNextMove fallback object; metadata parsed from move.ts declaration\n`+
  `fallback,${result.move},${result.useMode},${result.targets}\n`+
  `move,STRUGGLE,${declaration[1]},${declaration[2]},${declaration[3]},${declaration[4]},${declaration[5]},${declaration[6]},${declaration[7]},TYPELESS\n`;
const path = resolve(root, 'pokerogue-tests/fixtures/struggle.csv');
if (process.argv.includes('--check')) assert.equal(readFileSync(path, 'utf8'), output); else writeFileSync(path, output);
console.log('Struggle source fixture current.');
