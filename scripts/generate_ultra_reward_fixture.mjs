#!/usr/bin/env node
// Execute the pinned ULTRA PLAYER pool callbacks against small controlled parties.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const source = readFileSync(resolve(root, 'original/src/modifier/init-modifier-pools.ts'), 'utf8');
function body(marker) {
  const at = source.indexOf(marker); assert.ok(at >= 0);
  const open = source.indexOf('{', at); let depth = 1; let quote = null; let escape = false; let lineComment = false; let blockComment = false;
  for (let i = open + 1; i < source.length; i++) {
    const c = source[i], next = source[i + 1];
    if (lineComment) { if (c === '\n') lineComment = false; continue; }
    if (blockComment) { if (c === '*' && next === '/') { blockComment = false; i++; } continue; }
    if (quote) { if (escape) escape = false; else if (c === '\\') escape = true; else if (c === quote) quote = null; }
    else if (c === '/' && next === '/') { lineComment = true; i++; }
    else if (c === '/' && next === '*') { blockComment = true; i++; }
    else if (c === '"' || c === "'" || c === '`') quote = c;
    else if (c === '{') depth++;
    else if (c === '}' && --depth === 0) return source.slice(open + 1, i);
  }
  throw new Error(marker);
}
const helpers = stripTypeScriptTypes(`function skipInClassicAfterWave(wave:number,defaultWeight:number){${body('function skipInClassicAfterWave(')}} function skipInLastClassicWaveOrDefault(defaultWeight:number){${body('function skipInLastClassicWaveOrDefault(')}} function lureWeightFunc(maxBattles:number,weight:number){${body('function lureWeightFunc(')}} function hasMaximumBalls(ballType:any){${body('function hasMaximumBalls(')}}`);
const ids = ['ULTRA_BALL','MAX_LURE','BIG_NUGGET','PP_MAX','MINT','RARE_EVOLUTION_ITEM','FORM_CHANGE_ITEM','AMULET_COIN','EVIOLITE','RARE_SPECIES_STAT_BOOSTER','LEEK','TOXIC_ORB','FLAME_ORB','MYSTICAL_ROCK','REVIVER_SEED','CANDY_JAR','ATTACK_TYPE_BOOSTER','TM_ULTRA','RARER_CANDY','GOLDEN_PUNCH','IV_SCANNER','EXP_CHARM','EXP_SHARE','TERA_ORB','QUICK_CLAW','WIDE_LENS'];
const contexts = [
  { key:'classic-wave-1-eligible', wave:1, classic:true, daily:false, fresh:false, unlocked:true, balls:0, lure:null, party:true },
  { key:'classic-wave-1-ultra-balls-capped', wave:1, classic:true, daily:false, fresh:false, unlocked:true, balls:99, lure:null, party:true },
  { key:'classic-wave-199-capped', wave:199, classic:true, daily:false, fresh:false, unlocked:true, balls:99, lure:18, party:true },
  { key:'endless-wave-100-locked-challenge', wave:100, classic:false, daily:false, fresh:true, unlocked:false, balls:0, lure:0, party:true },
  { key:'mini-run-wave-19', wave:19, classic:true, daily:false, fresh:false, unlocked:false, balls:0, lure:null, party:false },
];
const output = [`# source=${pin}; executes original initUltraModifierPool and all callbacks with controlled party predicates`];
const poolIds = { ULTRA: ids };
for (const c of contexts) {
  const pool = {};
  class Weighted { constructor(id, weight, max) { this.id=id; this.weight=weight; this.max=max; } setTier(tier) { this.tier=tier; } }
  class SpeciesCritBoosterModifier {}
  const party = c.party ? [{
    isMax:()=>false, isFusion:()=>false,
    getSpeciesForm:()=>({speciesId: 1}), getFusionSpeciesForm:()=>({speciesId:1}),
    getHeldItems:()=>[], getMoveset:()=>[], canSetStatus:()=>true, hasAbility:()=>false,
  }, { isMax:()=>false, isFusion:()=>false, getSpeciesForm:()=>({speciesId:2}), getHeldItems:()=>[], getMoveset:()=>[], canSetStatus:()=>true, hasAbility:()=>false }] : [];
  const mode = { isClassic:c.classic, isDaily:c.daily, isEndless:false, isSplicedOnly:false, isFreshStartChallenge:()=>c.fresh };
  const globalScene = {
    gameMode:mode, currentBattle:{waveIndex:c.wave}, pokeballCounts:[0,0,c.balls,0,0],
    gameData:{isUnlocked:()=>c.unlocked}, getModifiers:()=>c.lure===null?[]:[{getMaxBattles:()=>30,getBattleCount:()=>c.lure}],
  };
  const modifierPool = {};
  const init = stripTypeScriptTypes(`function initUltraModifierPool(){${body('function initUltraModifierPool()')}}`);
  const invoke = new Function('modifierPool','modifierTypes','ModifierTier','WeightedModifierType','globalScene','DoubleBattleChanceBoosterModifier','PokeballType','MAX_PER_TYPE_POKEBALLS','Unlockables','timedEventManager','speciesDataRegistry','SpeciesCritBoosterModifier','SpeciesId','AbilityId','MoveId','StatusEffect', `${helpers};${init};return initUltraModifierPool;`);
  const makeEnum = names => new Proxy(Object.fromEntries(names.map(x=>[x,x])), {get:(obj,key)=>obj[key] ?? String(key)});
  invoke(pool,makeEnum(ids),{ULTRA:2},Weighted,globalScene,class{}, {ULTRA_BALL:2},99,{EVIOLITE:1}, {areFusionsBoosted:()=>false}, {hasEvolutions:id=>id===1}, SpeciesCritBoosterModifier, {FARFETCHD:1,GALAR_FARFETCHD:3,SIRFETCHD:4}, makeEnum([]), makeEnum([]), makeEnum([]))();
  const entries = pool[2]; assert.deepEqual(entries.map(entry=>entry.id),ids);
  const weights = entries.map(entry=>typeof entry.weight==='function'?entry.weight(party,0):entry.weight);
  output.push(`ultra,${c.key},${c.wave},${weights.join('|')}`);
}
output.splice(1,0,`ultra_ids,${poolIds.ULTRA.join('|')}`);
const text = `${output.join('\n')}\n`;
const path = resolve(root,'pokerogue-tests/fixtures/ultra-rewards.csv');
if (process.argv.includes('--check')) assert.equal(readFileSync(path,'utf8'),text); else writeFileSync(path,text);
console.log('Ultra reward oracle current.');
