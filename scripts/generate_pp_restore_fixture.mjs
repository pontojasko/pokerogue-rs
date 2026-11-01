#!/usr/bin/env node
// Execute the pinned Pokemon PP restore modifier apply methods.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve } from 'node:path';

const pin='7c0881255af5cd51992e513ff93c2f71adbe40e0';
const source=readFileSync('original/src/modifier/modifier.ts','utf8');
function blockAfter(text, at) {
  const open=text.indexOf('{',at); assert.ok(open>=0); let depth=1,quote=null,escaped=false,line=false,block=false;
  for(let i=open+1;i<text.length;i++) {
    const c=text[i],next=text[i+1];
    if(line){if(c==='\n')line=false;continue;}
    if(block){if(c==='*'&&next==='/'){block=false;i++;}continue;}
    if(quote){if(escaped)escaped=false;else if(c==='\\')escaped=true;else if(c===quote)quote=null;continue;}
    if(c==='/'&&next==='/'){line=true;i++;continue;}
    if(c==='/'&&next==='*'){block=true;i++;continue;}
    if(c==='"'||c==="'"||c==='`')quote=c;
    else if(c==='{')depth++;
    else if(c==='}'&&--depth===0)return text.slice(open+1,i);
  }
  throw new Error('unclosed source block');
}
function classMethod(name) {
  const at=source.indexOf(`export class ${name}`); assert.ok(at>=0);
  const classBody=blockAfter(source,source.indexOf('{',at));
  const method='override apply(playerPokemon: PlayerPokemon): boolean {';
  const methodAt=classBody.indexOf(method); assert.ok(methodAt>=0,name);
  return blockAfter(classBody,classBody.indexOf('{',methodAt));
}
const one=stripTypeScriptTypes(`function apply(playerPokemon){${classMethod('PokemonPpRestoreModifier')}}`);
const all=stripTypeScriptTypes(`function apply(playerPokemon){${classMethod('PokemonAllMovePpRestoreModifier')}}`);
const singleApply=new Function(`${one};return apply;`)();
const allApply=new Function(`${all};return apply;`)();
const rows=[`# source=${pin}; executed PokemonPpRestoreModifier.apply and PokemonAllMovePpRestoreModifier.apply; PP Ups excluded`];
for(const [used,restore] of [[30,10],[0,10],[35,-1]]) {
  const move={ppUsed:used},pokemon={getMoveset:()=>[move]};
  singleApply.call({moveIndex:0,restorePoints:restore},pokemon);
  rows.push(`single,${used},${restore},${move.ppUsed}`);
}
for(const [uses,restore] of [[[30,0],10],[[30,25],-1]]) {
  const moves=uses.map(ppUsed=>({ppUsed})),pokemon={getMoveset:()=>moves};
  allApply.call({restorePoints:restore},pokemon);
  rows.push(`all,${uses.join('|')},${restore},${moves.map(move=>move.ppUsed).join('|')}`);
}
const output=`${rows.join('\n')}\n`,fixture=resolve('pokerogue-tests/fixtures/pp-restore.csv');
if(process.argv.includes('--check'))assert.equal(readFileSync(fixture,'utf8'),output);else writeFileSync(fixture,output);
console.log('PP restore oracle current.');
