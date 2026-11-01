#!/usr/bin/env node
// Execute the pinned applyPartyExp body with controlled party state and optional EXP Share stacks.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve } from 'node:path';
import { SourcePhaseTree, sourcePhaseManagerMethods } from './lib/source_phase_tree.mjs';

const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const source = readFileSync('original/src/battle-scene.ts', 'utf8');
const marker = '  applyPartyExp(\n';
const start = source.indexOf(marker); assert.ok(start >= 0);
const bodyStart = source.indexOf('{', source.indexOf('): void', start)); assert.ok(bodyStart >= 0);
function bodyAfter(text, open) {
  let depth = 1, quote = null, escaped = false, line = false, comment = false;
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
  throw new Error('unclosed applyPartyExp');
}
const body = bodyAfter(source, bodyStart);
const executable = stripTypeScriptTypes(`
class ExpShareModifier { constructor(stack) { this.stack=stack; } getStackCount() { return this.stack; } } class ExpBalanceModifier {} class MultipleParticipantExpBonusModifier {}
class PokemonIncrementingStatModifier {} class PokemonExpBoosterModifier {}
class NumberHolder { constructor(value) { this.value = value; } }
const BattleType={WILD:'wild',TRAINER:'trainer'};
const MysteryEncounterMode={TRAINER_BATTLE:'trainer'};
const Phaser={Math:{Linear:(a,b,t)=>a+(b-a)*t}};
const FRIENDSHIP_GAIN_FROM_BATTLE=3;
const activeOverrides={XP_MULTIPLIER_OVERRIDE:null};
class Scene {
 constructor(party,shareStacks) {
   this.party=party; this.shareStacks=shareStacks; this.phaseTree=new PhaseTree(); this.currentBattle={battleType:BattleType.WILD,isBattleMysteryEncounter:()=>false};
   this.phaseManager=Object.assign({phaseQueue:this.phaseTree,checkDynamic:phase=>phase,create:(name,index,exp)=>({phaseName:name,index,exp,is(type){return this.phaseName===type;}})},phaseManagerMethods());
 }
 getPlayerParty(){return this.party;} findModifier(predicate){const share=this.shareStacks?new ExpShareModifier(this.shareStacks):null;return share && predicate(share)?share:null;} getMaxExpLevel(){return 10;}
 applyModifiers(){} updateModifiers(){}
 applyPartyExp(expValue,pokemonDefeated,useWaveIndexMultiplier=false,participantIds=new Set()) { ${body} }
}
`);
const run = new Function('config', 'PhaseTree', 'phaseManagerMethods', `${executable};
 const friendshipGains=[];
 const party=config.party.map((p,index)=>({...p,id:index+1,pokerus:false,addFriendship(amount){friendshipGains.push([index,amount]);},getHeldItems(){return[];},isOnField(){return p.onField;}}));
 const scene=new Scene(party,config.expShareStacks??0); scene.applyPartyExp(config.exp,config.pokemonDefeated??true,false,new Set(config.participants));
 const queued=[]; let phase; while ((phase=scene.phaseTree.getNextPhase())) queued.push([phase.index,phase.exp,phase.phaseName]);
 return {queued, party:party.map(p=>p.id), friendshipGains};
`);
class MessagePhase { constructor(message){ this.phaseName='MessagePhase'; this.message=message; } is(type){ return this.phaseName===type; } }
const managerMethods = () => sourcePhaseManagerMethods(MessagePhase);
const scenarios = [
  {name:'two-participants-one-at-cap',exp:1001,participants:[1,2],party:[
    {hp:20,level:10,onField:true},{hp:20,level:5,onField:false},
  ]},
  {name:'one-participant',exp:1001,participants:[1],party:[
    {hp:20,level:5,onField:true},{hp:20,level:5,onField:false},
  ]},
  {name:'exp-share-one-stack',exp:1001,participants:[1],expShareStacks:1,party:[
    {hp:20,level:5,onField:true},{hp:20,level:5,onField:false},
  ]},
  {name:'exp-share-five-stacks-two-participants',exp:1001,participants:[1,2],expShareStacks:5,party:[
    {hp:20,level:5,onField:true},{hp:20,level:5,onField:true},{hp:20,level:5,onField:false},
  ]},
  {name:'no-defeat-no-friendship',exp:1001,pokemonDefeated:false,participants:[1],party:[
    {hp:20,level:5,onField:true},
  ]},
];
const rows=[`# source=${pin}; executes original BattleScene.applyPartyExp with controlled EXP Share and records friendship calls; phase queue order is captured`, '# scenario,raw_exp,participant_ids,party_hp_levels,queued_phase_index_exp,friendship_member_amounts'];
for (const scenario of scenarios) {
  const result=run(scenario, SourcePhaseTree, managerMethods);
  const members=scenario.party.map(p=>`${p.hp}:${p.level}`).join('|');
  const queue=result.queued.map(([index,exp])=>`${index}:${exp}`).join('|')||'-';
  const friendship=result.friendshipGains.map(([index,amount])=>`${index}:${amount}`).join('|')||'-';
  rows.push(`${scenario.name},${scenario.exp},${scenario.participants.join('|')},${members},${queue},${friendship}`);
}
const output=`${rows.join('\n')}\n`, file=resolve('pokerogue-tests/fixtures/party-exp.csv');
if(process.argv.includes('--check')) assert.equal(readFileSync(file,'utf8'),output); else writeFileSync(file,output);
console.log('Party EXP oracle current.');
