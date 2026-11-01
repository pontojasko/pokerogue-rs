#!/usr/bin/env node
// Executes the pinned GREAT PLAYER pool callbacks for controlled local-run states.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const pin='7c0881255af5cd51992e513ff93c2f71adbe40e0';
const pools=readFileSync(resolve(root,'original/src/modifier/init-modifier-pools.ts'),'utf8');
const pokemon=readFileSync(resolve(root,'original/src/field/pokemon.ts'),'utf8');
function body(source,marker){const at=source.indexOf(marker);assert.ok(at>=0,`missing ${marker}`);const open=source.indexOf('{',at);let n=1,quote=null,escaped=false;for(let i=open+1;i<source.length;i++){const c=source[i];if(quote){if(escaped)escaped=false;else if(c==='\\')escaped=true;else if(c===quote)quote=null;}else if(c==='"'||c==="'"||c==='`')quote=c;else if(c==='{')n++;else if(c==='}'&&--n===0)return source.slice(open+1,i);}throw new Error(`unterminated ${marker}`);}
const init=stripTypeScriptTypes(`function initGreatModifierPool(){${body(pools,'function initGreatModifierPool()')}}`);
const helpers=stripTypeScriptTypes(`function skipInClassicAfterWave(wave:number,defaultWeight:number){${body(pools,'function skipInClassicAfterWave(')}} function skipInLastClassicWaveOrDefault(defaultWeight:number){${body(pools,'function skipInLastClassicWaveOrDefault(')}} function lureWeightFunc(maxBattles:number,weight:number){${body(pools,'function lureWeightFunc(')}} function hasMaximumBalls(ballType:any){${body(pools,'function hasMaximumBalls(')}}`);
const ratioAt=pokemon.indexOf('  getHpRatio(precise = false): number {');assert.ok(ratioAt>=0);
const ratioEnd=pokemon.indexOf('\n  }',ratioAt);const ratioJs=stripTypeScriptTypes(`function getHpRatio${pokemon.slice(ratioAt+'  getHpRatio'.length,ratioEnd+4)}`);
const sourceRatio=new Function(`${ratioJs};return getHpRatio;`)();
const ids=['GREAT_BALL','PP_UP','FULL_HEAL','REVIVE','MAX_REVIVE','SACRED_ASH','HYPER_POTION','MAX_POTION','FULL_RESTORE','ELIXIR','MAX_ELIXIR','DIRE_HIT','SUPER_LURE','NUGGET','SPECIES_STAT_BOOSTER','EVOLUTION_ITEM','MAP','SOOTHE_BELL','TM_GREAT','MEMORY_MUSHROOM','BASE_STAT_BOOSTER','TERA_SHARD','DNA_SPLICERS','VOUCHER'];
const makeParty=(members)=>members.map(([hp,max,status,protectedStatus,leppa,ppPairs,learnable,tera,fused])=>({
 hp,status:status?{effect:1}:null,fusionSpecies:fused?{}:null,
 getInverseHp:()=>max-hp,getMaxHp:()=>max,getHpRatio:()=>sourceRatio.call({hp,getMaxHp:()=>max}),
 isFainted:()=>hp<=0,getStatus:()=>status?{effect:1}:null,
 getHeldItems:()=>protectedStatus?[new TurnStatusEffectModifier()]:leppa?[new BerryModifier()]:[],
 getMoveset:()=>ppPairs.map(([maxPp,used])=>({ppUsed:used,getMovePp:()=>maxPp})),
 getLearnableLevelMoves:()=>learnable?[['MOVE']]:[],
 hasSpecies:()=>!tera,
}));
const contexts=[
 {key:'fresh-wave-1',wave:1,classic:true,daily:false,spliced:false,balls:[0,0,0,0,0],lures:[],soothe:false,fusions:false,members:[[100,100,0,0,0,[],1,1,0],[100,100,0,0,0,[],1,1,0]]},
 {key:'damaged-status-faint-wave-180',wave:180,classic:true,daily:false,spliced:false,balls:[0,99,0,0,0],lures:[[15,9]],soothe:true,fusions:false,members:[[100,300,1,0,0,[[35,30]],1,1,0],[0,200,0,0,0,[[40,0]],0,0,0]]},
 {key:'classic-last-wave',wave:199,classic:true,daily:false,spliced:false,balls:[0,0,0,0,0],lures:[],soothe:false,fusions:false,members:[[50,300,0,0,0,[],1,1,0],[200,300,0,0,0,[],1,1,0]]},
 {key:'mini-run-great-recovery-wave-19',wave:19,classic:true,daily:false,spliced:false,balls:[5,0,0,0,0],lures:[],soothe:false,fusions:false,members:[[18,160,0,0,0,[[25,0],[35,0]],1,1,0],[0,22,0,0,0,[[25,0],[35,0]],1,1,0]]},
 {key:'mini-run-great-elixir-wave-19',wave:19,classic:true,daily:false,spliced:false,balls:[5,0,0,0,0],lures:[],soothe:false,fusions:false,members:[[18,160,0,0,0,[[25,20],[35,30]],1,1,0],[0,22,0,0,0,[[25,0],[35,0]],1,1,0]]},
 {key:'mini-run-great-full-restore-wave-19',wave:19,classic:true,daily:false,spliced:false,balls:[5,0,0,0,0],lures:[],soothe:false,fusions:false,members:[[100,300,0,0,0,[[25,0],[35,0]],1,1,0],[100,300,0,0,0,[[25,0],[35,0]],1,1,0]]},
 {key:'mini-run-healthy-wave-19',wave:19,classic:true,daily:false,spliced:false,balls:[5,0,0,0,0],lures:[],soothe:false,fusions:false,members:[[100,100,0,0,0,[[25,0],[35,0]],0,0,0],[100,100,0,0,0,[[25,0],[35,0]],0,0,0]]},
 {key:'mini-run-live-amulet-wave-19',wave:19,classic:true,daily:false,spliced:false,balls:[5,0,0,0,0],lures:[],soothe:false,fusions:false,members:[[100,100,0,0,0,[[25,0],[35,0]],1,1,0],[100,100,0,0,0,[[25,0],[35,0]],1,1,0]]},
];
const results=[`great_ids,${ids.join('|')}`];
for(const context of contexts){
 const party=makeParty(context.members),modifierPool={};
 class Weighted{constructor(id,weight,maxWeight){this.id=id;this.weight=weight;this.maxWeight=maxWeight;}setTier(tier){this.tier=tier;}}
 class BerryModifier{constructor(){this.berryType=1;}}
 class TurnStatusEffectModifier{getStatusEffect(){return 1;}}
 class DoubleBattleChanceBoosterModifier{}
 const globalScene={gameMode:{isClassic:context.classic,isSplicedOnly:context.spliced,isDaily:context.daily,getWaveForDifficulty:(wave)=>wave},currentBattle:{waveIndex:context.wave},pokeballCounts:context.balls,getModifiers:(type)=>type===DoubleBattleChanceBoosterModifier?context.lures.map(([max,count])=>({getMaxBattles:()=>max,getBattleCount:()=>count})):[],getPlayerParty:()=>party};
 const sourceInit=new Function('modifierPool','modifierTypes','ModifierTier','WeightedModifierType','globalScene','timedEventManager','PokeballType','MAX_PER_TYPE_POKEBALLS','BerryModifier','TurnStatusEffectModifier','DoubleBattleChanceBoosterModifier','SpeciesId',`${helpers};${init};return initGreatModifierPool;`)(modifierPool,new Proxy({}, {get:(_,key)=>String(key)}),{GREAT:1},Weighted,globalScene,{isSootheBellBoosted:()=>context.soothe,areFusionsBoosted:()=>context.fusions},{POKEBALL:0,GREAT_BALL:1,ULTRA_BALL:2,ROGUE_BALL:3,MASTER_BALL:4},99,BerryModifier,TurnStatusEffectModifier,DoubleBattleChanceBoosterModifier,{TERAPAGOS:1,OGERPON:2,SHEDINJA:3});
 sourceInit();
 const entries=modifierPool[1];assert.deepEqual(entries.map(item=>item.id),ids);
 const weights=entries.map(item=>typeof item.weight==='function'?item.weight(party,0):item.weight);
 results.push(`great,${context.key},${context.wave},${weights.join('|')}`);
}
const output=`# source=${pin}; executed initGreatModifierPool and weight callbacks; controlled Classic contexts\n${results.join('\n')}\n`;
const path=resolve(root,'pokerogue-tests/fixtures/great-rewards.csv');
if(process.argv.includes('--check'))assert.equal(readFileSync(path,'utf8'),output);else writeFileSync(path,output);
console.log('Great reward oracle current.');
