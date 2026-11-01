#!/usr/bin/env node
// Executes getNewModifierTypeOption when a generated COMMON candidate is null.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const source = readFileSync(resolve(root, 'original/src/modifier/modifier-type.ts'), 'utf8');
const marker = 'function getNewModifierTypeOption(';
const at = source.indexOf(marker); assert.ok(at >= 0);
const open = source.indexOf('{', at); let depth = 1, quote = null, escaped = false, end = -1;
for (let i=open+1;i<source.length;i++) { const c=source[i]; if (quote) { if (escaped) escaped=false; else if(c==='\\') escaped=true; else if(c===quote) quote=null; } else if(c==='"'||c==="'"||c==='`') quote=c; else if(c==='{') depth++; else if(c==='}'&&--depth===0) {end=i;break;} }
assert.ok(end>open);
const impl=source.slice(open+1,end)
  .replace('let thresholds: object;', 'let thresholds;').replace('let tier: ModifierTier;', 'let tier;')
  .replace('let modifierType: ModifierType | null', 'let modifierType').replace('let index: number | undefined;', 'let index;')
  .replace('(modifierType as ModifierTypeGenerator)', 'modifierType').replace('modifierType as ModifierType', 'modifierType').replace('upgradeCount!', 'upgradeCount').replace('.at(-1)!', '.at(-1)');
const tmClassAt=source.indexOf('class TmModifierTypeGenerator');
const tmMarker='super((party: readonly Pokemon[], pregenArgs?: any[]) => {';
const tmAt=source.indexOf(tmMarker,tmClassAt),tmOpen=source.indexOf('{',tmAt);let tmDepth=1,tmEnd=tmOpen+1;
for(;tmDepth;tmEnd++){if(source[tmEnd]==='{')tmDepth++;if(source[tmEnd]==='}')tmDepth--;}
const tmImpl=source.slice(tmOpen+1,tmEnd-1).replace('(p as PlayerPokemon)','p').replace('pregenArgs[0] as MoveId','pregenArgs[0]');
class Alea { constructor(seed){this.c=1;this.n=0xefc8249d;this.s0=this.hash(' ');this.s1=this.hash(' ');this.s2=this.hash(' ');this.s0=(this.s0-this.hash(seed)+1)%1;this.s1=(this.s1-this.hash(seed)+1)%1;this.s2=(this.s2-this.hash(seed)+1)%1;} hash(s){for(let i=0;i<s.length;i++){this.n+=s.charCodeAt(i);let h=.02519603282416938*this.n;this.n=Math.trunc(h)>>>0;h-=this.n;h*=this.n;this.n=Math.trunc(h)>>>0;h-=this.n;this.n+=h*4294967296;}return(this.n>>>0)/4294967296;} rnd(){const t=2091639*this.s0+this.c/4294967296;this.c=Math.trunc(t);this.s0=this.s1;this.s1=this.s2;this.s2=t-this.c;return this.s2;} frac(){const a=this.rnd();return a+Math.floor(this.rnd()*2097152)/9007199254740992;} int(n){return Math.floor(this.frac()*n);} }
const invokeSource = new Function('ModifierPoolType','ModifierTier','getModifierPoolForType','modifierPoolThresholds','enemyModifierPoolThresholds','enemyBuffModifierPoolThresholds','dailyStarterModifierPoolThresholds','getPartyLuckValue','randSeedInt','ModifierTypeGenerator','ModifierTypeOption','ignoredPoolIndexes',`return function getNewModifierTypeOption(party,poolType,tier,upgradeCount,retryCount=0,allowLuckUpgrades=true){${impl}}`);
const rows=[`# source=${pin}; executes getNewModifierTypeOption; COMMON pool indexes 0 and 9 weighted equally; index 9 generator returns null for empty compatible TMs`];
let matched=false;
for(let i=0;i<1000&&!matched;i++){
  const seed=`NullRetrySeed${String(i).padStart(4,'0')}`,alea=new Alea(seed),draws=[];
  const rand=(range,min=0)=>{const value=min+alea.int(range);draws.push(`${range}:${min}:${value}`);return value;};
  const pool=Array.from({length:10},(_,index)=>({modifierType:index===0?{id:'POKEBALL',name:'Poké Ball',group:'pb',tier:0}:{id:`UNUSED_${index}`,name:`unused-${index}`,group:`unused-${index}`,tier:0}}));
  const thresholds={0:{1:0,2:9},1:{1:0},2:{1:0},3:{1:0},4:{1:0}};
  class ModifierTypeGenerator{constructor(generate){this.generate=generate;}generateType(party){return this.generate(party);}}
  class TmModifierType{constructor(id){this.id=id;}}
  const makeTmGenerator=new Function('ModifierTypeGenerator','tmPoolTiers','tier','allMoves','randSeedInt','TmModifierType',`return class extends ModifierTypeGenerator { constructor(){super((party,pregenArgs)=>{${tmImpl}});} };`);
  pool[9].modifierType=new (makeTmGenerator(ModifierTypeGenerator,{},0,{},()=>0,TmModifierType))();
  class ModifierTypeOption{constructor(type,upgradeCount){this.type=type;this.upgradeCount=upgradeCount;}}
  const getNew=invokeSource({PLAYER:0,WILD:1,TRAINER:2,ENEMY_BUFF:3,DAILY_STARTER:4},{COMMON:0,GREAT:1,ULTRA:2,ROGUE:3,MASTER:4},()=>({0:pool}),thresholds,{}, {}, {},()=>0,rand,ModifierTypeGenerator,ModifierTypeOption,{0:[],1:[],2:[],3:[],4:[]});
  const old=console.log;console.log=()=>{};let result;try{result=getNew([],0,undefined,undefined,0,false);}finally{console.log=old;}
  if(draws.length===3&&draws[0].startsWith('1024:0:')&&Number(draws[0].split(':')[2])>255&&draws[1]==='2:0:1'&&draws[2]==='2:0:0'){
    assert.equal(result.type.id,'POKEBALL');
    rows.push(`null-retry,${seed},${draws.join('|')},${result.type.id},${result.upgradeCount}`);matched=true;
  }
}
assert.ok(matched,'could not find controlled null-generator retry seed');
let steppedDown=false;
for(let i=0;i<1000&&!steppedDown;i++){
  const seed=`NullStepdown${String(i).padStart(4,'0')}`,alea=new Alea(seed),draws=[];
  const rand=(range,min=0)=>{if(range<=1)return min;const value=min+alea.int(range);draws.push(`${range}:${min}:${value}`);return value;};
  let nullCalls=0;
  class ModifierTypeGenerator{constructor(generate){this.generate=generate;}generateType(party){nullCalls++;return this.generate(party);}}
  class TmModifierType{constructor(id){this.id=id;}}
  class ModifierTypeOption{constructor(type,upgradeCount){this.type=type;this.upgradeCount=upgradeCount;}}
  const makeTmGenerator=new Function('ModifierTypeGenerator','tmPoolTiers','tier','allMoves','randSeedInt','TmModifierType',`return class extends ModifierTypeGenerator { constructor(){super((party,pregenArgs)=>{${tmImpl}});} };`);
  const common=Array.from({length:10},(_,index)=>({modifierType:index===0?{id:'POKEBALL',name:'Poké Ball',group:'pb',tier:0}:{id:`C${index}`,name:`c${index}`,tier:0}}));
  const great=Array.from({length:19},(_,index)=>({modifierType:index===18?null:{id:`G${index}`,name:`g${index}`,tier:1}}));
  great[18].modifierType=new (makeTmGenerator(ModifierTypeGenerator,{},1,{},rand,TmModifierType))();
  const pool={0:common,1:great};
  const thresholds={0:{1:0},1:{1:18}};
  const getNew=invokeSource({PLAYER:0,WILD:1,TRAINER:2,ENEMY_BUFF:3,DAILY_STARTER:4},{COMMON:0,GREAT:1,ULTRA:2,ROGUE:3,MASTER:4},()=>pool,thresholds,{}, {}, {},()=>0,rand,ModifierTypeGenerator,ModifierTypeOption,{0:[],1:[],2:[],3:[],4:[]});
  const old=console.log;console.log=()=>{};let result;try{result=getNew([],0,undefined,undefined,0,false);}finally{console.log=old;}
  const tierRoll=Number(draws[0]?.split(':')[2]);
  if(draws.length===1&&tierRoll>=61&&tierRoll<=255&&result.type.id==='POKEBALL'&&nullCalls===100){
    rows.push(`stepdown,${seed},${draws.join('|')},${result.type.id},${nullCalls}`);steppedDown=true;
  }
}
assert.ok(steppedDown,'could not find seed for original 100-null tier fallback');
const output=`${rows.join('\n')}\n`,path=resolve(root,'pokerogue-tests/fixtures/null-reward-retry.csv');
if(process.argv.includes('--check'))assert.equal(readFileSync(path,'utf8'),output);else writeFileSync(path,output);
console.log('Null generated-reward retry oracle current.');
