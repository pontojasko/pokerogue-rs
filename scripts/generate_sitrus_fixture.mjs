#!/usr/bin/env node
// Execute the pinned Sitrus predicate/effect bodies with minimal phase wrappers.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const source = readFileSync(resolve(root, 'original/src/data/berry.ts'), 'utf8');
function body(marker) {
  const at = source.indexOf(marker); assert.ok(at >= 0, `missing ${marker}`);
  const open = source.indexOf('{', at); let depth = 1, quote = null, escaped = false, lineComment = false, blockComment = false;
  for (let i = open + 1; i < source.length; i++) {
    const ch = source[i], next = source[i + 1];
    if (lineComment) { if (ch === '\n') lineComment = false; continue; }
    if (blockComment) { if (ch === '*' && next === '/') { blockComment = false; i++; } continue; }
    if (quote) { if (escaped) escaped = false; else if (ch === '\\') escaped = true; else if (ch === quote) quote = null; }
    else if (ch === '/' && next === '/') { lineComment = true; i++; }
    else if (ch === '/' && next === '*') { blockComment = true; i++; }
    else if (ch === '"' || ch === "'" || ch === '`') quote = ch;
    else if (ch === '{') depth++;
    else if (ch === '}' && --depth === 0) return source.slice(open + 1, i);
  }
  throw new Error(`unterminated ${marker}`);
}
const stripTypes = value => value.replace(/\(([A-Za-z_$][\w$]*): [A-Za-z_$][\w$]*\)/g, '($1)').replace(/: [A-Za-z_$][\w$]*(?=\s*=)/g, '');
const BerryType = { SITRUS:0, LUM:1, ENIGMA:2, LIECHI:3, GANLON:4, PETAYA:5, APICOT:6, SALAC:7, LANSAT:8, STARF:9, LEPPA:10 };
const predicate = new Function('BerryType','BattlerTagType','HitResult', `return function getBerryPredicate(berryType){${stripTypes(body('export function getBerryPredicate(berryType: BerryType): BerryPredicate {'))}}`)(BerryType,{},{});
const calls = [];
const globalScene = { phaseManager: { unshiftNew: (...args) => calls.push(args) } };
class NumberHolder { constructor(value) { this.value=value; } }
const effect = new Function('BerryType','NumberHolder','toDmgValue','applyAbAttrs','globalScene','i18next','getPokemonNameWithAffix','getBerryName', `return function getBerryEffectFunc(berryType, berryPhase=false){${stripTypes(body('export function getBerryEffectFunc(berryType: BerryType, berryPhase = false): BerryEffectFunc {'))}}`)(BerryType,NumberHolder,value=>Math.max(Math.floor(value),1),()=>{},globalScene,{t:()=>''},()=>'',()=> 'Sitrus Berry');
const rows = [`# source=${pin}; executed pinned Sitrus predicate and effect; PokemonHealPhase creation captured`];
for (const [max,hp] of [[100,50],[100,49],[42,21],[42,20],[1,1],[1,0]]) {
  const result = predicate(BerryType.SITRUS)({getHpRatio:()=>hp/max});
  rows.push(`predicate,${max},${hp},${result}`);
}
for (const max of [1,4,42,43]) {
  calls.length=0;
  effect(BerryType.SITRUS,true)({getMaxHp:()=>max,getBattlerIndex:()=>0});
  assert.equal(calls.length,1);
  rows.push(`effect,${max},${calls[0][0]},${calls[0][2]}`);
}
const output=`${rows.join('\n')}\n`, path=resolve(root,'pokerogue-tests/fixtures/sitrus.csv');
if (process.argv.includes('--check')) assert.equal(readFileSync(path,'utf8'),output); else writeFileSync(path,output);
console.log('Sitrus Berry source fixture current.');
