import {readFileSync,writeFileSync} from 'node:fs';
import assert from 'node:assert/strict';
const root=new URL('../',import.meta.url);
const source=readFileSync(new URL('original/src/utils/common.ts',root),'utf8');
const marker='export function shiftCharCodes(str: string, shiftCount: number)';
const start=source.indexOf(marker); assert.ok(start>=0);
const open=source.indexOf('{',start); let depth=1,end=open+1;
for(;depth;end++){if(source[end]==='{')depth++;else if(source[end]==='}')depth--;}
const shift=new Function(`return function shiftCharCodes(str,shiftCount){${source.slice(open+1,end-1)}}`)();
const rows=['# source=7c0881255af5cd51992e513ff93c2f71adbe40e0; executes shiftCharCodes using JS UTF-16 code units'];
for(const [seed,wave] of [['RewardSeed',1],['RewardSeed',5],['MiniRunSeed0000',10],['A😀Z',10]]){
 const units=[];for(let i=0;i<shift(seed,wave).length;i++)units.push(shift(seed,wave).charCodeAt(i).toString(16));
 rows.push(`seed,${seed},${wave},${units.join('|')}`);
}
const output=rows.join('\n')+'\n', path=new URL('pokerogue-tests/fixtures/reward-seed.csv',root);
if(process.argv.includes('--check'))assert.equal(readFileSync(path,'utf8'),output);else writeFileSync(path,output);
console.log('Wave reward seed fixture current.');
