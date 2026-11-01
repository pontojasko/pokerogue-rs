#!/usr/bin/env node
// Execute PokemonMove.usePp from the pinned source for bounded PP cases.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve } from 'node:path';

const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const path = resolve('original/src/data/moves/pokemon-move.ts');
const source = readFileSync(path, 'utf8');
const marker = 'public usePp(count = 1): void {';
const start = source.indexOf(marker); assert.ok(start >= 0);
const open = source.indexOf('{', start); let depth=1, quote=null, escaped=false, line=false, block=false;
for (let i=open+1; i<source.length && depth; i++) {
  const c=source[i], next=source[i+1];
  if (line) { if (c==='\n') line=false; continue; }
  if (block) { if (c==='*'&&next==='/') {block=false;i++;} continue; }
  if (quote) { if (escaped) escaped=false; else if (c==='\\') escaped=true; else if (c===quote) quote=null; continue; }
  if (c==='/'&&next==='/') {line=true;i++;continue;}
  if (c==='/'&&next==='*') {block=true;i++;continue;}
  if (c==='"'||c==="'"||c==='`') quote=c;
  else if (c==='{') depth++;
  else if (c==='}') depth--;
}
assert.equal(depth,0);
const body=source.slice(open+1,source.indexOf('}',open));
const method=stripTypeScriptTypes(`function usePp(count=1){${body}}`);
const usePp=new Function(`${method};return usePp;`)();
const cases=[[0,1,35],[34,2,35],[24,5,25],[25,1,25]];
const rows=[`# source=${pin}; executes original PokemonMove.usePp; PP Ups/max overrides excluded`];
for (const [used,count,max] of cases) {
  const instance={ppUsed:used,getMovePp:()=>max}; usePp.call(instance,count);
  rows.push(`use,${used},${count},${max},${instance.ppUsed}`);
}
const output=`${rows.join('\n')}\n`, fixture=resolve('pokerogue-tests/fixtures/move-pp.csv');
if (process.argv.includes('--check')) assert.equal(readFileSync(fixture,'utf8'),output); else writeFileSync(fixture,output);
console.log('Move PP oracle current.');
