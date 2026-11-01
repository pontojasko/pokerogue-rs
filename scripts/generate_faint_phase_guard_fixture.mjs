#!/usr/bin/env node
// Execute pinned MovePhase's inactive-user guard and TurnEndPhase.start with isolated wrappers.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve } from 'node:path';

const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
function blockAfter(source, marker) {
  const at = source.indexOf(marker); assert.ok(at >= 0, `missing ${marker}`);
  const open = source.indexOf('{', at); let depth = 1, quote = null, escaped = false, line = false, comment = false;
  for (let i = open + 1; i < source.length; i++) {
    const c = source[i], n = source[i + 1];
    if (line) { if (c === '\n') line = false; continue; }
    if (comment) { if (c === '*' && n === '/') { comment = false; i++; } continue; }
    if (quote) { if (escaped) escaped = false; else if (c === '\\') escaped = true; else if (c === quote) quote = null; continue; }
    if (c === '/' && n === '/') { line = true; i++; continue; }
    if (c === '/' && n === '*') { comment = true; i++; continue; }
    if (c === '"' || c === "'" || c === '`') quote = c;
    else if (c === '{') depth++;
    else if (c === '}' && --depth === 0) return source.slice(open + 1, i);
  }
  throw new Error(`unclosed block ${marker}`);
}
const moveSource = readFileSync('original/src/phases/move-phase.ts', 'utf8');
const moveStart = moveSource.indexOf('export class MovePhase');
const moveClass = moveSource.slice(moveStart);
const moveBody = blockAfter(moveClass, 'public start(): void');
const turnSource = readFileSync('original/src/phases/turn-end-phase.ts', 'utf8');
const turnStart = turnSource.indexOf('export class TurnEndPhase');
const turnClass = turnSource.slice(turnStart);
const turnBody = blockAfter(turnClass, 'start()');
const wrapper = stripTypeScriptTypes(`
class PokemonPhase { start(){ this.parentStarted = true; } end(){ this.ended = true; } }
class MovePhase extends PokemonPhase { constructor(pokemon){ super(); this.pokemon=pokemon; } start(){ ${moveBody} } }
class FieldPhase { start(){ this.parentStarted = true; } end(){ this.ended = true; } executeForAll(_fn){} }
class TurnEndPhase extends FieldPhase { constructor(){ super(); this.upcomingInterlude=false; } start(){ ${turnBody} } }
function runMoveGuard(){ const phase=new MovePhase({isActive:()=>false}); phase.start(); return {parent:!!phase.parentStarted, ended:!!phase.ended}; }
function runTurnEnd(){
 const trace={turn:1,events:0,cleared:0,hidden:0,ended:false};
 globalScene.currentBattle={turn:1,incrementTurn(){this.turn++;trace.turn=this.turn;}};
 globalScene.eventTarget={dispatchEvent(event){trace.events=event.turnCount;}};
 globalScene.phaseManager={dynamicQueueManager:{clearLastTurnOrder(){trace.cleared++;}},hideAbilityBar(){trace.hidden++;}};
 globalScene.arena={weather:null,terrain:null,lapseTags(){}};
 const phase=new TurnEndPhase(); phase.end=function(){trace.ended=true;}; phase.start();
 return trace;
}
`);
const makeHarness = new Function('globalScene', 'TurnEndEvent', 'BattlerTagLapseType', 'TerrainType', 'WeatherType', 'TurnHealModifier', 'TurnStatusEffectModifier', 'TurnHeldItemTransferModifier', 'EnemyTurnHealModifier', 'EnemyStatusEffectHealChanceModifier', 'applyAbAttrs', 'toDmgValue', 'i18next', `${wrapper};return [runMoveGuard,runTurnEnd];`);
const scene = {};
const [runMoveGuard, runTurnEnd] = makeHarness(scene, class { constructor(turn){ this.turnCount=turn; } }, { TURN_END: 0 }, { GRASSY: 1 }, { NONE: 0 }, class {}, class {}, class {}, class {}, class {}, () => {}, () => 0, { t: () => '' });
const guard = runMoveGuard();
const turn = runTurnEnd();
const rows = [
  `# source=${pin}; actual MovePhase.start inactive guard and TurnEndPhase.start executed with no active battlers or status hooks`,
  '# scenario,parent_started,move_phase_ended,turn,turn_end_event_turn,queue_clears,ability_bar_hides,phase_ended',
  `inactive-move,${+guard.parent},${+guard.ended},-,-,-,-,-`,
  `turn-end,-,-,${turn.turn},${turn.events},${turn.cleared},${turn.hidden},${+turn.ended}`,
];
const output = `${rows.join('\n')}\n`;
const fixture = resolve('pokerogue-tests/fixtures/faint-phase-guards.csv');
if (process.argv.includes('--check')) assert.equal(readFileSync(fixture, 'utf8'), output);
else writeFileSync(fixture, output);
console.log('Faint phase guard oracle current.');
