#!/usr/bin/env node
// Run the pinned BattleScene.newBattle path with controlled wild/Town collaborators.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve } from 'node:path';
import { execFileSync } from 'node:child_process';
import { extractSourceBody } from './lib/source_phase_tree.mjs';

const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const actualPin = execFileSync('git', ['-C', 'original', 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
assert.equal(actualPin, pin, 'original source checkout changed');
const sceneSource = readFileSync('original/src/battle-scene.ts', 'utf8');
const utilSource = readFileSync('original/src/utils/common.ts', 'utf8');
const methods = [
  `getNewBattleProps(fromSession){${extractSourceBody(sceneSource, 'private getNewBattleProps(')}}`,
  `resetSeed(waveIndex){${extractSourceBody(sceneSource, 'resetSeed(waveIndex?: number): void')}}`,
  `executeWithSeedOffset(func,offset,seedOverride){${extractSourceBody(sceneSource, 'executeWithSeedOffset(func: () => void, offset: number, seedOverride?: string): void')}}`,
  `handleNonFixedBattle(resolved){${extractSourceBody(sceneSource, 'private handleNonFixedBattle(')}}`,
  `newBattle(fromSession){${extractSourceBody(sceneSource, 'public newBattle(')}}`,
].join('\n');
const shiftCharCodes = `function shiftCharCodes(str,shiftCount){${extractSourceBody(utilSource, 'export function shiftCharCodes(')}}`;
const executable = stripTypeScriptTypes(`
const calls=record; let rngState='saved-phaser-state';
const console={log:()=>{}};
const Phaser={Math:{RND:{sow:seeds=>{calls.push('sow:'+seeds[0]);rngState=seeds[0];},state:next=>{if(next!==undefined){rngState=next;return;}return rngState;}}}};
const BattleType={WILD:'WILD'}; const STARTING_WAVE=1;
const activeOverrides={STARTING_WAVE_OVERRIDE:null,DISABLE_STANDARD_TRAINERS_OVERRIDE:false,BATTLE_TYPE_OVERRIDE:null};
${shiftCharCodes}
class Battle { constructor(gameMode,props){Object.assign(this,props);this.gameMode=gameMode;this.turn=0;calls.push('construct:'+props.waveIndex+':'+props.battleType+':'+(+props.double));} incrementTurn(){this.turn++;calls.push('turn:'+this.turn);} }
class Scene { ${methods} }
`);
const makeScene = new Function('record', `${executable}; return (previousWave,seed)=>{
  const scene=new Scene(); scene.seed=seed; scene.waveSeed=''; scene.rngOffset=0; scene.rngSeedOverride='';
  scene.currentBattle=previousWave ? {waveIndex:previousWave,double:false,trainer:null,mysteryEncounter:null} : null;
  scene.gameMode={hasTrainers:false,isFixedBattle:()=>false};
  scene.isWaveMysteryEncounter=()=>false; scene.checkIsDouble=()=>false;
  scene.getMaxExpLevel=()=>100; scene.doPostBattleCleanup=(_battle,cap)=>record.push('cleanup:'+cap);
  scene.handleNonFixedBattle=function(resolved){ return Scene.prototype.handleNonFixedBattle.call(this,resolved); };
  scene.executeWithSeedOffset=function(...args){ return Scene.prototype.executeWithSeedOffset.apply(this,args); };
  scene.resetSeed=function(...args){ return Scene.prototype.resetSeed.apply(this,args); };
  scene.getNewBattleProps=function(...args){ return Scene.prototype.getNewBattleProps.apply(this,args); };
  scene.currentTrace=record;
  const result=Scene.prototype.newBattle.call(scene);
  return {wave:result.waveIndex,type:result.battleType,double:result.double,turn:result.turn,waveSeed:scene.waveSeed,offset:scene.rngOffset,rngState:Phaser.Math.RND.state()};
};`);

// The RNG double records the exact seeds while preserving/restoring the source state.
const rows=[`# source=${pin}; executes BattleScene.newBattle, getNewBattleProps, resetSeed, handleNonFixedBattle, executeWithSeedOffset and shiftCharCodes; Battle/checkIsDouble/mystery/cleanup collaborators controlled`, '# previous_wave,new_wave,battle_type,double,turn,wave_seed,offset_rng_seed,final_rng_state,call_trace'];
for (const previousWave of [0,1,8,9,10,19,20]) {
  const seed='MiniRunTownSeed0001';
  const trace=[];
  const result=makeScene(trace)(previousWave,seed);
  const wave = previousWave ? previousWave + 1 : 1;
  const waveSeed=[...seed].map(ch=>String.fromCharCode(ch.charCodeAt(0)+wave)).join('');
  const offsetSeed=[...waveSeed].map(ch=>String.fromCharCode(ch.charCodeAt(0)+(wave<<3))).join('');
  assert.equal(result.wave,wave);
  assert.equal(result.waveSeed,waveSeed);
  assert.equal(result.type,'WILD');
  const calls=trace.concat([]);
  // Calls emitted by the original methods are captured in the shared recorder.
  const output=`${previousWave},${result.wave},${result.type},${+result.double},${result.turn},${result.waveSeed},${offsetSeed},${result.rngState},${trace.join('|')||'-'}`;
  rows.push(output);
}
const fixture=resolve('pokerogue-tests/fixtures/new-battle-execution.csv');
const output=`${rows.join('\n')}\n`;
if(process.argv.includes('--check')) assert.equal(readFileSync(fixture,'utf8'),output);
else writeFileSync(fixture,output);
console.log('BattleScene.newBattle source fixture current.');
