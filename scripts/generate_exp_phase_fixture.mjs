#!/usr/bin/env node
// Execute the actual ExpPhase.start and its captured UI callback with original Pokemon.addExp.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve } from 'node:path';
const pin='7c0881255af5cd51992e513ff93c2f71adbe40e0';
function bodyAfter(source,at){const open=source.indexOf('{',at);assert.ok(open>=0);let d=1,q=null,e=false,l=false,c=false;for(let i=open+1;i<source.length;i++){const x=source[i],n=source[i+1];if(l){if(x==='\n')l=false;continue;}if(c){if(x==='*'&&n==='/'){c=false;i++;}continue;}if(q){if(e)e=false;else if(x==='\\')e=true;else if(x===q)q=null;continue;}if(x==='/'&&n==='/'){l=true;i++;continue;}if(x==='/'&&n==='*'){c=true;i++;continue;}if('"\'`'.includes(x))q=x;else if(x==='{')d++;else if(x==='}'&&--d===0)return source.slice(open+1,i);}throw Error('unclosed method');}
function arrayLiteral(source,marker){const start=source.indexOf('[',source.indexOf(marker));assert.ok(start>=0);let d=1,q=null,e=false;for(let i=start+1;i<source.length;i++){const x=source[i];if(q){if(e)e=false;else if(x==='\\')e=true;else if(x===q)q=null;continue;}if('"\'`'.includes(x))q=x;else if(x==='[')d++;else if(x===']'&&--d===0)return source.slice(start,i+1);}throw Error('unclosed exp table');}
const expSrc=readFileSync('original/src/data/exp.ts','utf8');const expLevels=arrayLiteral(expSrc,'const expLevels =');const totalBody=bodyAfter(expSrc,expSrc.indexOf('export function getLevelTotalExp'));
const pokemonSrc=readFileSync('original/src/field/pokemon.ts','utf8');const addBody=bodyAfter(pokemonSrc,pokemonSrc.indexOf('public addExp('));
const phaseSrc=readFileSync('original/src/phases/exp-phase.ts','utf8');const startBody=bodyAfter(phaseSrc,phaseSrc.indexOf('public override start(): void'));
const code=stripTypeScriptTypes(`
const expLevels=${expLevels};const GrowthRate={ERRATIC:0,FAST:1,MEDIUM_FAST:2,MEDIUM_SLOW:3,SLOW:4,FLUCTUATING:5};
function getLevelTotalExp(level,growthRate){${totalBody}} function getPokemonNameWithAffix(){return 'Test';}
class ValueHolder{constructor(value){this.value=value;}} class ExpBoosterModifier{}
class BasePhase{start(){this.trace.parentStarted=true;}end(){this.trace.phaseEnded=true;}}
class PlayerPartyMemberPokemonPhase extends BasePhase{getPlayerPokemon(){return this.pokemon;}}
class ExpPhase extends PlayerPartyMemberPokemonPhase{constructor(){super();this.pokemon=null;this.expValue=319;this.partyMemberIndex=1;this.trace={parentStarted:false,phaseEnded:false,queued:[],text:null};}start(){${startBody}}}
function makePokemon(level,exp){return {level,exp,species:{growthRate:3},addExp(exp,ignoreLevelCap=false){${addBody}},showExpGain(){return Promise.resolve();}};}
function run(i18next,pokemon){
 const phase=new ExpPhase();phase.trace.pokemon=pokemon;phase.pokemon=pokemon;
 const callbacks=[];globalScene.ui={showText:(...args)=>{phase.trace.text=args[0];callbacks.push(args[2]);}};
 phase.start();callbacks[0]();return Promise.resolve().then(()=>Promise.resolve()).then(()=>phase.trace);
}
`);
const globalScene={getMaxExpLevel:()=>10,applyModifiers(){},phaseManager:{unshiftNew:(...args)=>trace.queued.push(args)},ui:null};
const calls=[];globalScene.phaseManager.unshiftNew=(...args)=>calls.push(args);
const factory=new Function('globalScene','i18next',`${code};return {run,makePokemon};`)(globalScene,{t:()=>''});
const expTable=new Function(`${code};return getLevelTotalExp;`)();
const initialLevel=5,initialExp=expTable(5,3),amount=expTable(8,3)-initialExp;
const pokemon=factory.makePokemon(initialLevel,initialExp);
const result=await factory.run({t:()=>''},pokemon);
const row=`source=${pin};scenario,initial_level,initial_exp,awarded_exp,final_level,final_exp,parent_started,queued_phase,phase_ended\nexp-phase-level-up,${initialLevel},${initialExp},${amount},${pokemon.level},${pokemon.exp},${+result.parentStarted},${calls.map(args=>args.join(':')).join('|')||'-'},${+result.phaseEnded}`;
const output=`# ${row.split('\n')[0]}\n# scenario,initial_level,initial_exp,awarded_exp,final_level,final_exp,parent_started,queued_phase,phase_ended\n${row.split('\n')[1]}\n`;
const path=resolve('pokerogue-tests/fixtures/exp-phase.csv');
if(process.argv.includes('--check'))assert.equal(readFileSync(path,'utf8'),output);else writeFileSync(path,output);
console.log('EXP phase oracle current.');
