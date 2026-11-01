// Executes the original unforced PLAYER tier branch with controlled RNG calls.
import {readFileSync, writeFileSync} from 'node:fs';
import {stripTypeScriptTypes} from 'node:module';
import assert from 'node:assert/strict';
const root = new URL('../', import.meta.url);
const source = readFileSync(new URL('original/src/modifier/modifier-type.ts', root), 'utf8');
const start = source.indexOf('  if (tier === undefined) {', source.indexOf('function getNewModifierTypeOption('));
const end = source.indexOf('  } else if (upgradeCount === undefined && player)', start);
assert.ok(start > 0 && end > start);
const branch = stripTypeScriptTypes(`function run(party:any, pool:any, allowLuckUpgrades:boolean) { let tier:any; let upgradeCount:any; const player = true; ${source.slice(start,end)} } return {tier, upgrades:upgradeCount}; }`);
const rows = ['# source=7c0881255af5cd51992e513ff93c2f71adbe40e0; executed getNewModifierTypeOption unforced PLAYER tier branch; controlled rolls, actual source branch, excludes weighted item selection'];
const cases = [];
for(const value of [0,1,12,13,60,61,255,256,1023]) cases.push([0,true,'11111',[value,...(value ? [4] : [])]]);
cases.push([0,false,'11111',[256]],[0,true,'11111',[256,0,3,4]],[14,true,'11111',[61,3,4]],[0,true,'11111',[1,0,0,4]],[0,true,'11000',[13,4]],[0,true,'10000',[0]]);
for (const [luck,enabled,mask,draws] of cases) {
 const trace = []; let index=0;
 const rand = range => { assert.ok(index < draws.length); const value=draws[index++]; assert.ok(value < range); trace.push(`${range}:${value}`); return value; };
 const pool = Object.fromEntries([...mask].map((v,i)=>[i,v==='1'?[{}]:[]]));
 const run = new Function('randSeedInt','getPartyLuckValue','ModifierTier',`${branch};return run;`)(rand,()=>luck,{COMMON:0,GREAT:1,ULTRA:2,ROGUE:3,MASTER:4});
 const result=run([],pool,enabled);
 assert.equal(index,draws.length);
 rows.push(`tier,${luck},${Number(enabled)},${mask},${trace.join('|')},${result.tier},${result.upgrades}`);
}
// Execute the exact cumulative-threshold lookup independently of modifier factories.
const weightStart=source.indexOf('  const tierThresholds = Object.keys(thresholds[tier]);',end);
const weightEnd=source.indexOf('  if (index === undefined)',weightStart);
assert.ok(weightStart>0 && weightEnd>weightStart);
const weightedJs=stripTypeScriptTypes(`function pick(thresholds:any,tier:number) {${source.slice(weightStart,weightEnd)} return index;}`);
for(const weights of [[6,2,0,9,3],[0,6,0,3],[1]]) {
 let sum=0; const thresholds={}; weights.forEach((w,i)=>{if(w) thresholds[sum+=w]=i;});
 for(let value=0;value<sum;value++) {
  const trace=[];
  const pick=new Function('randSeedInt',`${weightedJs}; return pick;`)(range=>{trace.push(range);return value;});
  const index=pick({0:thresholds},0);
  assert.deepEqual(trace,[sum]);
  rows.push(`weight,${weights.join('|')},${sum},${value},${index}`);
 }
}
const out=rows.join('\n')+'\n', path=new URL('pokerogue-tests/fixtures/reward-tiers.csv',root);
if(process.argv.includes('--check')) assert.equal(readFileSync(path,'utf8'),out); else writeFileSync(path,out);
console.log('Natural reward tier source fixtures verified.');
