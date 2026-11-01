#!/usr/bin/env node
// Executes the pinned friendship held-item modifier methods.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve } from 'node:path';
const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const source = readFileSync('original/src/modifier/modifier.ts', 'utf8');
function body(marker, input = source) {
  const at = input.indexOf(marker); assert.ok(at >= 0, `missing ${marker}`);
  const open = input.indexOf('{', at); let depth = 1, quote = null, escaped = false;
  for (let i = open + 1; i < source.length; i++) {
    const c = input[i];
    if (quote) { if (escaped) escaped = false; else if (c === '\\') escaped = true; else if (c === quote) quote = null; }
    else if ('"\'`'.includes(c)) quote = c;
    else if (c === '{') depth++;
    else if (c === '}' && --depth === 0) return input.slice(open + 1, i);
  }
  throw new Error(`unclosed ${marker}`);
}
const apply = stripTypeScriptTypes(`function apply(pokemon, friendship) { ${body('override apply(_pokemon: Pokemon, friendship: NumberHolder)')} }`);
const sootheClass = source.slice(source.indexOf('export class PokemonFriendshipBoosterModifier extends'));
const cap = stripTypeScriptTypes(`function getMaxHeldItemCount(pokemon) { ${body('getMaxHeldItemCount(_pokemon: Pokemon)', sootheClass)} }`);
const applySource = new Function(`${apply}; return apply;`)();
const capSource = new Function(`${cap}; return getMaxHeldItemCount;`)();
const rows = [`# source=${pin}; executes PokemonFriendshipBoosterModifier.apply/getMaxHeldItemCount`, `# context,stacks,base_gain,boosted_gain,max_held_count`];
for (const [context, baseGain] of [['rare_candy', 6], ['battle', 3]]) {
  for (const stacks of [0, 1, 2, 3]) {
    const holder = { value: baseGain };
    applySource.call({ getStackCount: () => stacks }, {}, holder);
    const max = capSource.call({});
    rows.push(`${context},${stacks},${baseGain},${holder.value},${max}`);
  }
}
const output = `${rows.join('\n')}\n`, path = resolve('pokerogue-tests/fixtures/soothe-bell.csv');
if (process.argv.includes('--check')) assert.equal(readFileSync(path, 'utf8'), output);
else writeFileSync(path, output);
console.log('Soothe Bell TypeScript oracle verified.');
