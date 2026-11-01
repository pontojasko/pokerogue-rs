import {readFileSync,writeFileSync} from 'node:fs';
import {stripTypeScriptTypes} from 'node:module';
import assert from 'node:assert/strict';
const root=new URL('../',import.meta.url), src=readFileSync(new URL('original/src/modifier/modifier-type.ts',root),'utf8');
function body(marker){const at=src.indexOf(marker);assert.ok(at>=0);const open=src.indexOf('{',at);let end=open+1,n=1;for(;n;end++){if(src[end]==='{')n++;if(src[end]==='}')n--;}return src.slice(open+1,end-1);}
const js=stripTypeScriptTypes(`function run(existingOptions:any[],retryCount:number,party:any[],tier?:number,allowLuckUpgrades?:boolean){${body('function getModifierTypeOptionWithRetry(')}}`);
const decode=token=>{const [name,group,tier,upgradeCount]=token.split(':');return{type:{name,group:group==='-'?undefined:group,tier:Number(tier)},upgradeCount:Number(upgradeCount)};};
const rows=['# source=7c0881255af5cd51992e513ff93c2f71adbe40e0; executed retry method, no challenges; generator mocked with controlled candidate sequence'];
const cases=[
 ['empty',3,[],['A:-:0:0']],
 ['same_name',15,['A:balls:0:0'],['A:other:1:2','B:heal:1:2']],
 ['same_group',15,['A:heal:0:0'],['B:heal:2:1','C:balls:2:1']],
 ['undefined_group',15,['A:-:0:0'],['B:-:1:0','C:heal:1:0']],
 ['retry_limit',3,['A:heal:0:0'],['B:heal:0:0']],
 ['limit_one',1,['A:heal:0:0'],['B:heal:0:0']],
 ['distinct',15,['A:heal:0:0'],['B:balls:0:0']],
];
for(const [id,limit,existing,candidates] of cases){
 const trace=[];let cursor=0;
 const gen=(_party,_pool,tier,upgrades,_retry,allow)=>{trace.push(`${tier??'-'}:${upgrades??'-'}:${Number(allow)}`);return decode(candidates[Math.min(cursor++,candidates.length-1)]);};
 const run=new Function('getNewModifierTypeOption','ModifierPoolType','BooleanHolder','applyChallenges','ChallengeType',`${js};return run;`)(gen,{PLAYER:0},class{constructor(value){this.value=value;}},()=>{},{WAVE_REWARD:0});
 const result=run(existing.map(decode),limit,[]);
 const encode=c=>`${c.type.name}:${c.type.group??'-'}:${c.type.tier}:${c.upgradeCount}`;
 rows.push(`retry,${id},${limit},${existing.join('|')||'-'},${candidates.join('|')},${encode(result)},${trace.join('|')}`);
}
const expr=src.match(/const weight =\s*([\s\S]*?)\n\s*if \(weightedModifierType.maxWeight\)/)[1].trim().replace(/;$/,'');
const weightJs=stripTypeScriptTypes(`function gate(existingModifiers:any[],itemModifierType:any,weightedModifierType:any,party:any[],rerollCount:number){return ${expr};}`);
class Held{}class Form{};
for(const [stacks,held,form] of [[[],false,false],[[[2,2]],false,false],[[[2,2],[1,2]],false,false],[[[2,2]],true,false],[[[2,2]],false,true]]){
 const run=new Function('PokemonHeldItemModifierType','FormChangeItemModifierType',`${weightJs};return gate;`)(Held,Form);
 const item=held?new Held():form?new Form():{};
 const actual=run(stacks.map(([stack,cap])=>({stackCount:stack,getMaxStackCount:()=>cap})),item,{weight:6},[],0);
 rows.push(`cap,${stacks.map(v=>v.join(':')).join('|')||'-'},${Number(held)},${Number(form)},${actual}`);
}
const output=rows.join('\n')+'\n',path=new URL('pokerogue-tests/fixtures/reward-retries.csv',root);
if(process.argv.includes('--check'))assert.equal(readFileSync(path,'utf8'),output);else writeFileSync(path,output);
console.log('Original reward retry and stack-cap fixtures verified.');
