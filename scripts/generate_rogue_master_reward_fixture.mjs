#!/usr/bin/env node
// Executes the pinned ROGUE/MASTER PLAYER pool callbacks for controlled modes.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..'), pin='7c0881255af5cd51992e513ff93c2f71adbe40e0';
const source=readFileSync(resolve(root,'original/src/modifier/init-modifier-pools.ts'),'utf8');
function body(marker){const at=source.indexOf(marker);assert.ok(at>=0);const open=source.indexOf('{',at);let d=1,q=null,e=false;for(let i=open+1;i<source.length;i++){const c=source[i];if(q){if(e)e=false;else if(c==='\\')e=true;else if(c===q)q=null;}else if(c==='"'||c==="'"||c==='`')q=c;else if(c==='{')d++;else if(c==='}'&&--d===0)return source.slice(open+1,i);}throw new Error(marker);}
const helpers=stripTypeScriptTypes(`function skipInClassicAfterWave(wave:number,defaultWeight:number){${body('function skipInClassicAfterWave(')}} function skipInLastClassicWaveOrDefault(defaultWeight:number){${body('function skipInLastClassicWaveOrDefault(')}} function hasMaximumBalls(ballType:any){${body('function hasMaximumBalls(')}}`);
const ids={
 ROGUE:['ROGUE_BALL','RELIC_GOLD','LEFTOVERS','SHELL_BELL','BERRY_POUCH','GRIP_CLAW','SCOPE_LENS','BATON','SOUL_DEW','CATCHING_CHARM','ABILITY_CHARM','FOCUS_BAND','KINGS_ROCK','LOCK_CAPSULE','SUPER_EXP_CHARM','RARE_FORM_CHANGE_ITEM','MEGA_BRACELET','DYNAMAX_BAND','VOUCHER_PLUS'],
 MASTER:['MASTER_BALL','SHINY_CHARM','HEALING_CHARM','MULTI_LENS','VOUCHER_PREMIUM','DNA_SPLICERS','MINI_BLACK_HOLE'],
};
const contexts=[
 {key:'classic-wave-1',wave:1,classic:true,daily:false,endless:false,spliced:false,fresh:false,unlocked:false,balls:[0,0,0,0,0],fusions:false,party:[0,0]},
 {key:'classic-wave-199',wave:199,classic:true,daily:false,endless:false,spliced:false,fresh:false,unlocked:false,balls:[0,0,0,99,99],fusions:true,party:[0,0]},
 {key:'endless-wave-100',wave:100,classic:false,daily:false,endless:true,spliced:false,fresh:false,unlocked:true,balls:[0,0,0,0,0],fusions:false,party:[0,0]},
];
const rows=[`# source=${pin}; executed original initRogueModifierPool/initMasterModifierPool and helper callbacks; one reroll-free pass per context`];
let recordedIds = false;
for(const context of contexts)for(const tier of ['ROGUE','MASTER']){
 const modifierPool={},party=context.party.map((fused)=>({fusionSpecies:fused?{}:null}));
 class Weighted{constructor(id,weight,max){this.id=id;this.weight=weight;this.max=max;}setTier(t){this.tier=t;}}
 const globalScene={gameMode:{isClassic:context.classic,isDaily:context.daily,isEndless:context.endless,isSplicedOnly:context.spliced,isFreshStartChallenge:()=>context.fresh},currentBattle:{waveIndex:context.wave},pokeballCounts:context.balls,gameData:{isUnlocked:()=>context.unlocked}};
 const timedEventManager={areFusionsBoosted:()=>context.fusions};
 const bodyName=tier==='ROGUE'?'function initRogueModifierPool()':'function initMasterModifierPool()';
 const init=stripTypeScriptTypes(`function ${bodyName.split('function ')[1].split('(')[0]}(){${body(bodyName)}}`);
 const fn=new Function('modifierPool','modifierTypes','ModifierTier','WeightedModifierType','globalScene','timedEventManager','PokeballType','MAX_PER_TYPE_POKEBALLS','Unlockables','skipInClassicAfterWave','skipInLastClassicWaveOrDefault','hasMaximumBalls',`${helpers};${init};return ${bodyName.split('function ')[1].split('(')[0]};`)(modifierPool,new Proxy({}, {get:(_,k)=>String(k)}),{ROGUE:3,MASTER:4},Weighted,globalScene,timedEventManager,{POKEBALL:0,GREAT_BALL:1,ULTRA_BALL:2,ROGUE_BALL:3,MASTER_BALL:4},99,{MINI_BLACK_HOLE:1},()=>{},()=>{},()=>{});
 fn();const entries=modifierPool[tier==='ROGUE'?3:4];assert.deepEqual(entries.map(x=>x.id),ids[tier]);
 if (!recordedIds) { rows.push(`rogue_ids,${ids.ROGUE.join('|')}`, `master_ids,${ids.MASTER.join('|')}`); recordedIds = true; }
 const weights=entries.map(x=>typeof x.weight==='function'?x.weight(party,0):x.weight);
 rows.push(`${tier.toLowerCase()},${context.key},${context.wave},${weights.join('|')}`);
}
const output=`${rows.join('\n')}\n`,path=resolve(root,'pokerogue-tests/fixtures/rogue-master-rewards.csv');
if(process.argv.includes('--check'))assert.equal(readFileSync(path,'utf8'),output);else writeFileSync(path,output);
console.log('Rogue/Master reward oracle current.');
