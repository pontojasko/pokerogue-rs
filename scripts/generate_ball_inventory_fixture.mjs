#!/usr/bin/env node
// Execute the pinned AddPokeballModifier.apply with a small inventory wrapper.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const source = readFileSync('original/src/modifier/modifier.ts', 'utf8');
const start = source.indexOf('override apply(): boolean', source.indexOf('export class AddPokeballModifier'));
const open = source.indexOf('{', start);
let depth = 1, end = open + 1;
for (; end < source.length && depth; end++) {
  if (source[end] === '{') depth++;
  if (source[end] === '}') depth--;
}
assert.ok(start >= 0 && depth === 0, 'could not isolate AddPokeballModifier.apply');
const body = source.slice(open + 1, end - 1);
const apply = new Function('globalScene', 'MAX_PER_TYPE_POKEBALLS', body);
const rows = ['# source=7c0881255af5cd51992e513ff93c2f71adbe40e0; executed AddPokeballModifier.apply', '# ball,count_before,added,expected'];
for (const [ball, before, added] of [['GREAT_BALL', 0, 5], ['GREAT_BALL', 97, 5], ['ULTRA_BALL', 0, 5], ['ULTRA_BALL', 97, 5]]) {
  const index = ball === 'GREAT_BALL' ? 1 : 2;
  const counts = [0, 0, 0];
  counts[index] = before;
  const context = { pokeballType: index, count: added };
  apply.call(context, { pokeballCounts: counts }, 99);
  rows.push(`${ball},${before},${added},${counts[index]}`);
}
const output = `${rows.join('\n')}\n`;
const fixture = resolve('pokerogue-tests/fixtures/ball-inventory.csv');
if (process.argv.includes('--check')) assert.equal(readFileSync(fixture, 'utf8'), output);
else writeFileSync(fixture, output);
console.log('Poké Ball inventory oracle current.');
