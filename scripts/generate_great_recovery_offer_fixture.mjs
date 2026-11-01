#!/usr/bin/env node
// Execute pinned getNewModifierTypeOption for controlled offers. The Amulet
// scenario also executes the original Temp Stat Booster generator callback.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve } from 'node:path';

const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const source = readFileSync('original/src/modifier/modifier-type.ts', 'utf8');
const commonFixture = readFileSync('pokerogue-tests/fixtures/common-rewards.csv', 'utf8');
const greatFixture = readFileSync('pokerogue-tests/fixtures/great-rewards.csv', 'utf8');
const catalog = readFileSync('pokerogue-tests/fixtures/modifier-pools.csv', 'utf8');
const ultraFixture = readFileSync('pokerogue-tests/fixtures/ultra-rewards.csv', 'utf8');
const tmFixture = readFileSync('pokerogue-tests/fixtures/tm-compatibility.csv', 'utf8');
const importedData = readFileSync('pokerogue-data/src/generated_data.rs', 'utf8');
function bodyAfter(text, marker) {
  const at = text.indexOf(marker); assert.ok(at >= 0, `missing ${marker}`);
  const open = text.indexOf('{', at); let depth = 1, quote = null, escaped = false, line = false, comment = false;
  for (let i = open + 1; i < text.length; i++) {
    const c = text[i], n = text[i + 1];
    if (line) { if (c === '\n') line = false; continue; }
    if (comment) { if (c === '*' && n === '/') { comment = false; i++; } continue; }
    if (quote) { if (escaped) escaped = false; else if (c === '\\') escaped = true; else if (c === quote) quote = null; continue; }
    if (c === '/' && n === '/') { line = true; i++; continue; }
    if (c === '/' && n === '*') { comment = true; i++; continue; }
    if ('"\'`'.includes(c)) quote = c;
    else if (c === '{') depth++;
    else if (c === '}' && --depth === 0) return text.slice(open + 1, i);
  }
  throw new Error(`unclosed ${marker}`);
}
class Alea {
  constructor(seed) { this.c=1; this.n=0xefc8249d; this.s0=this.hash(' '); this.s1=this.hash(' '); this.s2=this.hash(' '); this.s0=(this.s0-this.hash(seed)+1)%1; this.s1=(this.s1-this.hash(seed)+1)%1; this.s2=(this.s2-this.hash(seed)+1)%1; }
  hash(text) { for(let i=0;i<text.length;i++){this.n+=text.charCodeAt(i);let h=0.02519603282416938*this.n;this.n=Math.trunc(h)>>>0;h-=this.n;h*=this.n;this.n=Math.trunc(h)>>>0;h-=this.n;this.n+=h*4294967296;}return(this.n>>>0)/4294967296; }
  rnd(){const t=2091639*this.s0+this.c*(1/4294967296);this.c=Math.trunc(t);this.s0=this.s1;this.s1=this.s2;this.s2=t-this.c;return this.s2;}
  int(range){return Math.floor((this.rnd()+Math.floor(this.rnd()*2097152)*(1/9007199254740992))*range);}
}
const readWeights = (fixture, prefix, key) => {
  const row = fixture.split(/\r?\n/).find(line => line.startsWith(`${prefix},${key},`));
  assert.ok(row, `missing source weights ${key}`);
  return row.split(',')[3].split('|').map(Number);
};
const commonWeights = commonFixture.split(/\r?\n/)
  .find(line => line.startsWith('common,') && line.endsWith('6|2|3|1|0|0|2|4|2|2')).split(',')[6].split('|').map(Number);
const healthyCommonWeights = commonFixture.split(/\r?\n/)
  .find(line => line.startsWith('common,') && line.endsWith('6|2|0|0|0|0|2|4|2|2')).split(',')[6].split('|').map(Number);
const elixirCommonWeights = commonFixture.split(/\r?\n/)
  .find(line => line.startsWith('common,1,19,5,') && line.includes('18:160:0:25:20')).split(',')[6].split('|').map(Number);
const greatWeights = readWeights(greatFixture, 'great', 'mini-run-great-recovery-wave-19');
const healthyGreatWeights = readWeights(greatFixture, 'great', 'mini-run-healthy-wave-19');
const liveAmuletGreatWeights = readWeights(greatFixture, 'great', 'mini-run-live-amulet-wave-19');
const fullRestoreGreatWeights = readWeights(greatFixture, 'great', 'mini-run-great-full-restore-wave-19');
const fullRestoreCommonWeights = commonFixture.split(/\r?\n/).find(line=>line.startsWith('common,1,19,5,-,100:300:0:35:0|100:300:0:35:0,')).split(',')[6].split('|').map(Number);
const fullRestoreUltraWeights = readWeights(ultraFixture, 'ultra', 'mini-run-wave-19');
const elixirWeights = readWeights(greatFixture, 'great', 'mini-run-great-elixir-wave-19');
const ultraWeights = readWeights(ultraFixture, 'ultra', 'mini-run-wave-19');
const idsFor = tier => catalog.split(/\r?\n/).filter(line => line.startsWith(`modifier,${tier},`)).map(line => line.split(',')[4]);
const ids = [idsFor('COMMON'), idsFor('GREAT'), idsFor('ULTRA'), ['UNSUPPORTED_ROGUE'], ['UNSUPPORTED_MASTER']];
const teraAt=source.indexOf('TERA_SHARD: () =>');
const teraBody=bodyAfter(source.slice(teraAt),'(party: readonly Pokemon[], pregenArgs?: any[]) => {');
const teraMethod=stripTypeScriptTypes(`function generateTera(party,pregenArgs){${teraBody}}`);
const ModifierTypeGenerator=class { constructor(fn){this.fn=fn;this.tier=null;} generateType(party){const type=this.fn(party);if(type){type.id=this.id;type.tier=this.tier;}return type;} };
const sourceGenerateTera=new Function('globalScene','PokemonType','TerastallizeAccessModifier',`${teraMethod};return generateTera;`)({getModifiers:()=>[]},{},class{});
let activeRandSeedInt=()=>0;
const tempClass=stripTypeScriptTypes(`class TempStatStageBoosterModifierTypeGenerator extends ModifierTypeGenerator {${bodyAfter(source,'class TempStatStageBoosterModifierTypeGenerator extends ModifierTypeGenerator')}}`);
class TempStatStageBoosterModifierType { constructor(stat) { this.id='TEMP_STAT_STAGE_BOOSTER'; this.stat=stat; } }
const SourceTempStatGenerator=new Function('ModifierTypeGenerator','Stat','TEMP_BATTLE_STATS','TempStatStageBoosterModifierType','randSeedInt',`${tempClass};return TempStatStageBoosterModifierTypeGenerator;`)(ModifierTypeGenerator,{ATK:1,DEF:2,SPATK:3,SPDEF:4,SPD:5,ACC:6},[1,2,3,4,5,6],TempStatStageBoosterModifierType,(range,min=0)=>activeRandSeedInt(range,min));
const tmClass=stripTypeScriptTypes(`class TmModifierTypeGenerator extends ModifierTypeGenerator {${bodyAfter(source,'class TmModifierTypeGenerator extends ModifierTypeGenerator')}}`);
const numericToMoveId=Object.fromEntries([...importedData.matchAll(/ImportedMove \{ id: "([A-Z0-9_]+)", numeric_id: (\d+),/g)].map(match=>[match[2],match[1]]));
const partyCommonTmIds=tmFixture.split(/\r?\n/).find(line=>line.startsWith('party,0,')).split(',')[2].split('|').map(id=>numericToMoveId[id]);
assert.ok(partyCommonTmIds.every(Boolean),'source TM fixture IDs must map to imported move IDs');
const partyGreatTmIds=tmFixture.split(/\r?\n/).find(line=>line.startsWith('party,1,')).split(',')[2].split('|').map(id=>numericToMoveId[id]);
assert.ok(partyGreatTmIds.every(Boolean),'source GREAT TM fixture IDs must map to imported move IDs');
const tmTiers=Object.fromEntries(partyCommonTmIds.map(id=>[id,0]));
partyGreatTmIds.forEach(id=>tmTiers[id]=1);
const allTmMoves=Object.fromEntries([...partyCommonTmIds,...partyGreatTmIds].map(id=>[id,{isUnimplemented:false}]));
class SourceTmModifierType { constructor(moveId){this.id='TM_COMMON';this.tier=0;this.group='tm';this.moveId=moveId;this.name=`TM_COMMON:${moveId}`;} }
const SourceTmGenerator=new Function('ModifierTypeGenerator','tmPoolTiers','tier','allMoves','randSeedInt','TmModifierType','MoveId',`${tmClass};return TmModifierTypeGenerator;`)(ModifierTypeGenerator,tmTiers,0,allTmMoves,(range,min=0)=>activeRandSeedInt(range,min),SourceTmModifierType,{});
assert.equal(ids[0].length, commonWeights.length);
assert.equal(ids[1].length, greatWeights.length);
const cumulative = weights => { const map={}; let sum=0; weights.forEach((weight,index)=>{if(weight){sum+=weight;map[sum]=index;}}); return map; };
const pool = { 0: ids[0].map(id=>({modifierType:{id,name:id,tier:0,group:({POKEBALL:'pb',POTION:'potion',SUPER_POTION:'potion',LURE:'lure',ETHER:'ether',MAX_ETHER:'ether',HYPER_POTION:'potion',MAX_POTION:'potion',REVIVE:'revive',MAX_REVIVE:'revive',ELIXIR:'elixir',MAX_ELIXIR:'elixir',NUGGET:'money'})[id]}})),
 1: ids[1].map(id=>({modifierType:id==='TERA_SHARD'?new ModifierTypeGenerator((party)=>sourceGenerateTera(party,undefined)):{id,name:id,tier:1,group:({GREAT_BALL:'pb',PP_UP:'ppUp',HYPER_POTION:'potion',MAX_POTION:'potion',FULL_RESTORE:'potion',REVIVE:'revive',MAX_REVIVE:'revive',ELIXIR:'elixir',MAX_ELIXIR:'elixir',NUGGET:'money'})[id]}})),
  2: ids[2].map(id=>({modifierType:id==='TERA_SHARD'?new ModifierTypeGenerator((party)=>sourceGenerateTera(party,undefined)):{id,name:id,tier:2,group:({PP_MAX:'ppUp',ULTRA_BALL:'pb',BIG_NUGGET:'money'})[id]}})),
  3: ids[3].map(id=>({modifierType:{id,name:id,tier:3}})),
  4: ids[4].map(id=>({modifierType:{id,name:id,tier:4}})) };
const poolWithTempStat={...pool,0:[...pool[0]]};
const poolWithTm={...pool,0:[...pool[0]],1:[...pool[1]]};
const tempStatGenerator=new SourceTempStatGenerator();
tempStatGenerator.id='TEMP_STAT_STAGE_BOOSTER';
tempStatGenerator.tier=0;
poolWithTempStat[0][7]={modifierType:tempStatGenerator};
poolWithTm[0][7]={modifierType:tempStatGenerator};
const tmGenerator=new SourceTmGenerator(0);tmGenerator.id='TM_COMMON';tmGenerator.tier=0;
poolWithTm[0][ids[0].indexOf('TM_COMMON')]={modifierType:tmGenerator};
const tmGreatGenerator=new SourceTmGenerator(1);tmGreatGenerator.id='TM_GREAT';tmGreatGenerator.tier=1;
poolWithTm[1][ids[1].indexOf('TM_GREAT')]={modifierType:tmGreatGenerator};
let activePool=pool;

const method = stripTypeScriptTypes(`function getNewModifierTypeOption(party,poolType,tier,upgradeCount,retryCount=0,allowLuckUpgrades=true){${bodyAfter(source,'function getNewModifierTypeOption(')}}`);
const retryMethod = stripTypeScriptTypes(`function getModifierTypeOptionWithRetry(existingOptions,retryCount,party,tier,allowLuckUpgrades){${bodyAfter(source,'function getModifierTypeOptionWithRetry(')}}`);
const make = new Function('ModifierPoolType','ModifierTier','getModifierPoolForType','modifierPoolThresholds','enemyModifierPoolThresholds','enemyBuffModifierPoolThresholds','dailyStarterModifierPoolThresholds','getPartyLuckValue','randSeedInt','ModifierTypeGenerator','ModifierTypeOption','ignoredPoolIndexes','BooleanHolder','applyChallenges','ChallengeType',`${method};${retryMethod};return [getNewModifierTypeOption,getModifierTypeOptionWithRetry];`);
class ModifierTypeOption { constructor(type, upgradeCount) { this.type=type; this.upgradeCount=upgradeCount; } }
const run = (runSeed,greatWeightsForSeed=greatWeights,commonWeightsForSeed=commonWeights,ultraWeightsForSeed=null,executeTempGenerator=false,waveValue=19,executeTmGenerator=false) => {
  const thresholds = {0:cumulative(commonWeightsForSeed),1:cumulative(greatWeightsForSeed),2:(ultraWeightsForSeed===null?{1:0}:cumulative(ultraWeightsForSeed)),3:{1:0},4:{1:0}};
  const wave=waveValue, shifted=Array.from(runSeed,c=>String.fromCharCode(c.charCodeAt(0)+wave)).join('');
  const rng=new Alea(shifted), calls=[];
  const rand=(range,min=0)=>{const value=min+rng.int(range);calls.push([range,min,value]);return value;};
  activePool=executeTmGenerator?poolWithTm:executeTempGenerator?poolWithTempStat:pool;
  activeRandSeedInt=rand;
  const [getOption,getOptionWithRetry]=make({PLAYER:0},{COMMON:0,GREAT:1,ULTRA:2,ROGUE:3,MASTER:4},()=>activePool,thresholds,{}, {}, {},()=>0,rand,ModifierTypeGenerator,ModifierTypeOption,{0:[],1:[],2:[],3:[],4:[]},class BooleanHolder { constructor(value){this.value=value;} },()=>{},{});
  const options=[]; const oldLog=console.log; console.log=()=>{};
  const offerParty=executeTmGenerator?[{getCompatibleTms:()=>[...partyCommonTmIds,...partyGreatTmIds]}]:[];
  try { for(let i=0;i<3;i++) options.push(getOptionWithRetry(options,15,offerParty,undefined,true)); } finally { console.log=oldLog; }
  const actual=options.map(option=>{const tier=option.type.tier??0;const tierIds=ids[tier];const variant=option.type.moveId??({1:'ATK',2:'DEF',3:'SPATK',4:'SPDEF',5:'SPD',6:'ACC'})[option.type.stat];const id=variant?`${option.type.id}:${variant}`:option.type.id;return `${id}:${option.upgradeCount}:${tier}:${tierIds.indexOf(option.type.id)}`;});
  return `${runSeed},${wave},${actual.join('|')},${calls.map(([range,min,value])=>`${range}:${min}:${value}`).join('|')}`;
};
const rows=[`# source=${pin}; executed getModifierTypeOptionWithRetry for three choices with source-executed COMMON/GREAT/ULTRA weight arrays and TERA_SHARD null generator; static groups mirror source constructors; TmSearch1 and TmGreatSearch execute original TM generators using source party TM candidates`,
  `# seed,wave,options,rng_calls`,
  run('GreatRecoverySeed0014'),
  (()=>{const allowed=new Set(['POKEBALL','RARE_CANDY','POTION','SUPER_POTION','ETHER','MAX_ETHER','LURE','GREAT_BALL','ELIXIR','MAX_ELIXIR','HYPER_POTION','MAX_POTION','FULL_RESTORE','SOOTHE_BELL','REVIVE','MAX_REVIVE','NUGGET','PP_UP','TM_COMMON','TM_GREAT','TEMP_STAT_STAGE_BOOSTER','BERRY']);for(let i=0;i<100000;i++){const row=run(`SootheBellSeed${String(i).padStart(5,'0')}`,liveAmuletGreatWeights,healthyCommonWeights,ultraWeights);const choices=row.split(',')[2].split('|').map(value=>value.split(':'));if(choices.some(([id,tier])=>id==='SOOTHE_BELL'&&tier==='1')&&choices.every(([id])=>allowed.has(id)))return row;}throw new Error('no supported source Soothe Bell offer found');})(),
  run('GreatBallSearch00021'),
  (()=>{const allowed=new Set(['POKEBALL','RARE_CANDY','POTION','SUPER_POTION','ETHER','MAX_ETHER','LURE','GREAT_BALL','ELIXIR','MAX_ELIXIR','HYPER_POTION','MAX_POTION','REVIVE','MAX_REVIVE']);for(let i=0;i<5000;i++){const row=run(`GreatElixirSeed${String(i).padStart(4,'0')}`,elixirWeights,elixirCommonWeights);const choices=row.split(',')[2].split('|').map(value=>value.split(':'));if(choices.some(([id])=>id==='ELIXIR'||id==='MAX_ELIXIR')&&choices.every(([id])=>allowed.has(id)))return row;}throw new Error('no bounded supported Elixir source scenario found');})(),
  (()=>{const allowed=new Set(['POKEBALL','RARE_CANDY','POTION','SUPER_POTION','ETHER','MAX_ETHER','LURE','GREAT_BALL','ELIXIR','MAX_ELIXIR','HYPER_POTION','MAX_POTION','REVIVE','MAX_REVIVE','NUGGET']);for(let i=0;i<10000;i++){const row=run(`GreatNuggetSeed${String(i).padStart(4,'0')}`);const choices=row.split(',')[2].split('|').map(value=>value.split(':'));if(choices.some(([id])=>id==='NUGGET')&&choices.every(([id])=>allowed.has(id)))return row;}throw new Error('no bounded source Nugget offer found');})(),
  (()=>{const allowed=new Set(['POKEBALL','RARE_CANDY','POTION','SUPER_POTION','ETHER','MAX_ETHER','LURE','GREAT_BALL','ELIXIR','MAX_ELIXIR','HYPER_POTION','MAX_POTION','FULL_RESTORE','REVIVE','MAX_REVIVE','NUGGET','PP_UP','TM_COMMON','TM_GREAT','TEMP_STAT_STAGE_BOOSTER','BERRY']);for(let i=0;i<100000;i++){const row=run(`GreatFullRestoreSeed${String(i).padStart(5,'0')}`,fullRestoreGreatWeights,fullRestoreCommonWeights,fullRestoreUltraWeights,false,19,true);const choices=row.split(',')[2].split('|').map(value=>value.split(':'));if(choices.some(([id,tier])=>id==='FULL_RESTORE'&&tier==='1')&&choices.every(([id])=>allowed.has(id)))return row;}throw new Error('no supported source Full Restore offer found');})(),
  (()=>{const allowed=new Set(['POKEBALL','RARE_CANDY','POTION','SUPER_POTION','ETHER','MAX_ETHER','LURE','GREAT_BALL','PP_UP','ELIXIR','MAX_ELIXIR','HYPER_POTION','MAX_POTION','REVIVE','MAX_REVIVE','NUGGET']);for(let i=0;i<20000;i++){const row=run(`GreatPpUpSeed${String(i).padStart(4,'0')}`);const choices=row.split(',')[2].split('|').map(value=>value.split(':'));if(choices.some(([id])=>id==='PP_UP')&&choices.every(([id])=>allowed.has(id)))return row;}throw new Error('no bounded source PP_UP offer found');})(),
  (()=>{const allowed=new Set(['POKEBALL','RARE_CANDY','POTION','SUPER_POTION','ETHER','MAX_ETHER','LURE','GREAT_BALL','ULTRA_BALL','BIG_NUGGET','PP_MAX','EXP_CHARM','EXP_SHARE','AMULET_COIN','ELIXIR','MAX_ELIXIR','HYPER_POTION','MAX_POTION','REVIVE','MAX_REVIVE','NUGGET']);for(let i=0;i<100000;i++){const row=run(`UltraExpShareSeed${String(i).padStart(5,'0')}`,greatWeights,commonWeights,ultraWeights);const choices=row.split(',')[2].split('|').map(value=>value.split(':'));if(choices.some(([id,tier])=>id==='EXP_SHARE'&&tier==='2')&&choices.every(([id])=>allowed.has(id)))return row;}throw new Error('no bounded source EXP_SHARE offer found');})(),
  (()=>{const allowed=new Set(['POKEBALL','RARE_CANDY','POTION','SUPER_POTION','ETHER','MAX_ETHER','LURE','GREAT_BALL','ULTRA_BALL','BIG_NUGGET','PP_MAX','EXP_CHARM','EXP_SHARE','AMULET_COIN','ELIXIR','MAX_ELIXIR','HYPER_POTION','MAX_POTION','REVIVE','MAX_REVIVE','NUGGET']);for(let i=0;i<100000;i++){const row=run(`UltraAmuletCoinSeed${String(i).padStart(5,'0')}`,healthyGreatWeights,healthyCommonWeights,ultraWeights,true);const choices=row.split(',')[2].split('|').map(value=>value.split(':'));if(choices.some(([id,tier])=>id==='AMULET_COIN'&&tier==='2')&&choices.every(([id])=>allowed.has(id)))return row;}throw new Error('no bounded source AMULET_COIN offer found');})(),
  run('LiveAmulet000766',liveAmuletGreatWeights,healthyCommonWeights,ultraWeights,true),
  run('TmSearch1',greatWeights,healthyCommonWeights,ultraWeights,false,1,true),
  (()=>{const allowed=new Set(['POKEBALL','RARE_CANDY','POTION','SUPER_POTION','ETHER','MAX_ETHER','LURE','GREAT_BALL','PP_UP','ELIXIR','MAX_ELIXIR','HYPER_POTION','MAX_POTION','REVIVE','MAX_REVIVE','NUGGET','TM_COMMON','TM_GREAT','TEMP_STAT_STAGE_BOOSTER']);for(let i=0;i<50000;i++){const row=run(`TmGreatSearch${String(i).padStart(4,'0')}`,liveAmuletGreatWeights,healthyCommonWeights,ultraWeights,false,19,true);const choices=row.split(',')[2].split('|');const ids=choices.map(choice=>choice.split(':')[0]);if(ids.includes('TM_GREAT')&&ids.every(id=>allowed.has(id)))return row;}throw new Error('no supported Great TM source offer found');})(),
  (()=>{const allowed=new Set(['POKEBALL','RARE_CANDY','POTION','SUPER_POTION','ETHER','MAX_ETHER','LURE','GREAT_BALL','ULTRA_BALL','BIG_NUGGET','PP_MAX','EXP_CHARM','ELIXIR','MAX_ELIXIR','HYPER_POTION','MAX_POTION','REVIVE','MAX_REVIVE','NUGGET']);for(let i=0;i<100000;i++){const row=run(`UltraBigNuggetSeed${String(i).padStart(5,'0')}`,greatWeights,commonWeights,ultraWeights);const choices=row.split(',')[2].split('|').map(value=>value.split(':'));if(choices.some(([id,tier])=>id==='BIG_NUGGET'&&tier==='2')&&choices.every(([id])=>allowed.has(id)))return row;}throw new Error('no bounded source BIG_NUGGET offer found');})(),
  (()=>{const allowed=new Set(['POKEBALL','RARE_CANDY','POTION','SUPER_POTION','ETHER','MAX_ETHER','LURE','GREAT_BALL','ULTRA_BALL','PP_MAX','EXP_CHARM','ELIXIR','MAX_ELIXIR','HYPER_POTION','MAX_POTION','REVIVE','MAX_REVIVE','NUGGET']);for(let i=0;i<100000;i++){const row=run(`UltraExpCharmSeed${String(i).padStart(5,'0')}`,greatWeights,commonWeights,ultraWeights);const choices=row.split(',')[2].split('|').map(value=>value.split(':'));if(choices.some(([id,tier])=>id==='EXP_CHARM'&&tier==='2')&&choices.every(([id])=>allowed.has(id)))return row;}throw new Error('no bounded source EXP_CHARM offer found');})(),
  (()=>{const allowed=new Set(['POKEBALL','RARE_CANDY','POTION','SUPER_POTION','ETHER','MAX_ETHER','LURE','GREAT_BALL','ULTRA_BALL','PP_MAX','ELIXIR','MAX_ELIXIR','HYPER_POTION','MAX_POTION','REVIVE','MAX_REVIVE','NUGGET']);for(let i=0;i<100000;i++){const row=run(`UltraBallSeed${String(i).padStart(5,'0')}`,greatWeights,commonWeights,ultraWeights);const choices=row.split(',')[2].split('|').map(value=>value.split(':'));if(choices.some(([id,tier])=>id==='ULTRA_BALL'&&tier==='2')&&choices.every(([id])=>allowed.has(id)))return row;}throw new Error('no bounded source ULTRA_BALL offer found');})(),
  (()=>{const allowed=new Set(['POKEBALL','RARE_CANDY','POTION','SUPER_POTION','ETHER','MAX_ETHER','LURE','GREAT_BALL','PP_UP','PP_MAX','ULTRA_BALL','ELIXIR','MAX_ELIXIR','HYPER_POTION','MAX_POTION','REVIVE','MAX_REVIVE','NUGGET']);for(let i=0;i<100000;i++){const row=run(`UltraPpMaxSeed${String(i).padStart(5,'0')}`,greatWeights,commonWeights,ultraWeights);const choices=row.split(',')[2].split('|').map(value=>value.split(':'));if(choices.some(([id,tier])=>id==='PP_MAX'&&tier==='2')&&choices.every(([id])=>allowed.has(id)))return row;}throw new Error('no bounded source PP_MAX offer found');})()];
const output=`${rows.join('\n')}\n`, fixture=resolve('pokerogue-tests/fixtures/great-recovery-offer.csv');
if(process.argv.includes('--check')) assert.equal(readFileSync(fixture,'utf8'),output); else writeFileSync(fixture,output);
console.log('Great recovery offer oracle current.');
