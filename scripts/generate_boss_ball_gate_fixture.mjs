#!/usr/bin/env node
// Execute the original handleBallCommand boss-capture guard in isolation.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve } from 'node:path';
const pin='7c0881255af5cd51992e513ff93c2f71adbe40e0';
const source=readFileSync('original/src/phases/command-phase.ts','utf8');
const methodAt=source.indexOf('private handleBallCommand(cursor: number): boolean');
assert.ok(methodAt>=0);
const start=source.indexOf('if (\n        targetPokemon?.isBoss()',methodAt);
assert.ok(start>=0,'boss gate not found');
let depth=0,open=-1,quote=null,escape=false;
for(let i=source.indexOf('(',start);i<source.length;i++){const c=source[i];if(quote){if(escape)escape=false;else if(c==='\\')escape=true;else if(c===quote)quote=null;continue;}if('"\'`'.includes(c)){quote=c;continue;}if(c==='('){depth++;if(open<0)open=i;}else if(c===')'&&--depth===0){const brace=source.indexOf('{',i);let b=1;for(let j=brace+1;j<source.length;j++){if(source[j]==='{')b++;else if(source[j]==='}'&&--b===0){var gate=source.slice(start,j+1);break;}}break;}}
assert.ok(gate?.includes('bossSegmentIndex >= 1'),'failed extracting exact source branch');
const wrapped=stripTypeScriptTypes(`function sourceGate(segment,cursor,flags){const PokeballType={MASTER_BALL:4};const AbilityId={WONDER_GUARD:'WONDER_GUARD'};const targetPokemon={bossSegmentIndex:segment,isBoss:()=>true,hasAbility:()=>flags.wonder};const isFinalBoss=flags.final;const isChallengeActive=flags.challenge;const isCatchableDailyBoss=flags.daily;let reason='';const phase={queueShowText(text){reason=text},test(){${gate};return true}};const allowed=phase.test();return {allowed,reason,draws:[]};}`);
const sourceGate=new Function(`${wrapped};return sourceGate;`)();
const rows=[`# source=${pin}; executes exact boss condition/rejection branch extracted from CommandPhase.handleBallCommand; no Phaser phase queue`, '# scenario,segment,cursor,final_boss,challenge,daily_boss,wonder_guard,allowed,message,rng_draws'];
for(const [name,segment,cursor,final,challenge,daily,wonder] of [
 ['ordinary-boss-shield',1,0,0,0,0,0],['ordinary-boss-last-segment',0,0,0,0,0,0],['ordinary-master-exception',1,4,0,0,0,0],['final-boss-pokeball',1,0,1,0,0,0],['final-boss-master-no-challenge',1,4,1,0,0,0],['final-boss-master-challenge',1,4,1,1,0,0],['daily-boss-master',1,4,0,0,1,0],['wonder-guard-exception',1,0,0,0,0,1]]){
 const result=sourceGate(segment,cursor,{final:!!final,challenge:!!challenge,daily:!!daily,wonder:!!wonder});
 rows.push(`${name},${segment},${cursor},${final},${challenge},${daily},${wonder},${Number(result.allowed)},${result.reason||'-'},${result.draws.join('|')}`);
}
const output=`${rows.join('\n')}\n`,path=resolve('pokerogue-tests/fixtures/boss-ball-gate.csv');
if(process.argv.includes('--check'))assert.equal(readFileSync(path,'utf8'),output);else writeFileSync(path,output);
console.log('Boss capture gate source oracle current.');
