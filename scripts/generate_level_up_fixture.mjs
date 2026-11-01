#!/usr/bin/env node
// Execute pinned Pokemon.addExp then LevelUpPhase's Pokemon.calculateStats method body.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve } from 'node:path';
const pin='7c0881255af5cd51992e513ff93c2f71adbe40e0';
function bodyAfter(source, at) {
  const open=source.indexOf('{',at); assert.ok(open>=0); let depth=1,quote=null,escaped=false,line=false,comment=false;
  for(let i=open+1;i<source.length;i++){const c=source[i],n=source[i+1];if(line){if(c==='\n')line=false;continue;}if(comment){if(c==='*'&&n==='/'){comment=false;i++;}continue;}if(quote){if(escaped)escaped=false;else if(c==='\\')escaped=true;else if(c===quote)quote=null;continue;}if(c==='/'&&n==='/'){line=true;i++;continue;}if(c==='/'&&n==='*'){comment=true;i++;continue;}if('"\'`'.includes(c))quote=c;else if(c==='{')depth++;else if(c==='}'&&--depth===0)return source.slice(open+1,i);}
  throw Error('unclosed source method');
}
function arrayLiteral(source, marker){const start=source.indexOf('[',source.indexOf(marker));assert.ok(start>=0);let depth=1,quote=null,escaped=false;for(let i=start+1;i<source.length;i++){const c=source[i];if(quote){if(escaped)escaped=false;else if(c==='\\')escaped=true;else if(c===quote)quote=null;continue;}if('"\'`'.includes(c))quote=c;else if(c==='[')depth++;else if(c===']'&&--depth===0)return source.slice(start,i+1);}throw Error('unclosed source array');}
const expSource=readFileSync('original/src/data/exp.ts','utf8');
const expLevels=arrayLiteral(expSource,'const expLevels =');
const expBody=bodyAfter(expSource,expSource.indexOf('export function getLevelTotalExp'));
const pokemonSource=readFileSync('original/src/field/pokemon.ts','utf8');
const addBody=bodyAfter(pokemonSource,pokemonSource.indexOf('public addExp('));
const calcBody=bodyAfter(pokemonSource,pokemonSource.indexOf('calculateStats(): void'));
const levelPhaseSource=readFileSync('original/src/phases/level-up-phase.ts','utf8');
const phaseBody=bodyAfter(levelPhaseSource,levelPhaseSource.indexOf('public override start(): void'));
const data=readFileSync('pokerogue-tests/fixtures/imported-demo-data.csv','utf8');
const speciesRows=new Map(data.split('\n').filter(line=>line.startsWith('species,')).map(line=>{const f=line.split(',');return[f[1],f];}));
const wrapper=stripTypeScriptTypes(`
const expLevels=${expLevels}; const GrowthRate={ERRATIC:0,FAST:1,MEDIUM_FAST:2,MEDIUM_SLOW:3,SLOW:4,FLUCTUATING:5};
function getLevelTotalExp(level,growthRate){${expBody}}
class NumberHolder{constructor(value){this.value=value;}} class PokemonIncrementingStatModifier{} class PokemonNatureWeightModifier{}
function getNatureStatMultiplier(){return 1;}
const Stat={HP:0}; const PERMANENT_STATS=[0,1,2,3,4,5]; const AbilityId={WONDER_GUARD:0};
const Phaser={Math:{Clamp:(v,min,max)=>Math.min(Math.max(v,min),max)}};
const ExpNotification={DEFAULT:0,ONLY_LEVEL_UP:1,SKIP:2};const settings={general:{partyExpNotificationMode:ExpNotification.SKIP}};const LevelAchv={};
const globalScene={getMaxExpLevel:()=>10,applyModifier(){},gameData:{gameStats:{highestLevel:5}},validateAchvs(_type,holder){this.validated=holder.value;}};
class BasePhase{start(){this.trace.parentStarted=true;}end(){this.trace.phaseEnded=true;}}
class LevelUpPhase extends BasePhase{constructor(pokemon,lastLevel){super();this.pokemon=pokemon;this.lastLevel=lastLevel;this.level=pokemon.level;this.partyMemberIndex=0;}start(){${phaseBody}}}
class Pokemon {
 constructor(level,exp,hp,baseStats){this.level=level;this.exp=exp;this.hp=undefined;this.stats=undefined;this.ivs=[0,0,0,0,0,0];this.baseStats=baseStats;this.species={growthRate:GrowthRate.MEDIUM_SLOW};}
 addExp(exp,ignoreLevelCap=false){${addBody}}
 calculateBaseStats(){return this.baseStats;}
 calculateStats(){${calcBody}}
 getMaxHp(){return this.stats[0];} updateInfo(){} isPlayer(){return true;} hasAbility(){return false;} getNature(){return 0;}
 setStat(stat,value){this.stats[stat]=value;}
}
`);
const execute=new Function(`${wrapper}; return (level,exp,amount,hp,bases)=>{const p=new Pokemon(level,exp,hp,bases);p.calculateStats();p.hp=p.stats[0]-hp;const oldStats=p.stats.slice();const oldHp=p.hp;p.addExp(amount);const trace={parentStarted:false,phaseEnded:false};const phase=new LevelUpPhase(p,level);phase.trace=trace;phase.start();return {level:p.level,exp:p.exp,hp:p.hp,oldHp,oldStats,stats:p.stats,phaseEnded:+trace.phaseEnded,parent:+trace.parentStarted,highest:globalScene.gameData.gameStats.highestLevel,validated:globalScene.validated};};`)();
const rows=[`# source=${pin}; executes original Pokemon.addExp and LevelUpPhase.start (calls calculateStats; SKIP notifications, no modifiers, zero IV, neutral nature)`, '# scenario,species,start_level,start_exp,amount,start_hp,final_level,final_exp,start_stats,final_stats,final_hp,parent_started,phase_ended,highest_level,validated_level'];
for(const [name,id,amountLevels] of [['single-level','BULBASAUR',6],['multi-level','CHARMANDER',8]]){
 const f=speciesRows.get(id);assert.ok(f,`missing species ${id}`);const bases=f.slice(6,12).map(Number);
 const growth=3;const level=5;const exp=getExp(level,growth);const amount=getExp(amountLevels,growth)-exp;
 const state=execute(level,exp,amount,3,bases);
 rows.push(`${name},${id},${level},${exp},${amount},${state.oldHp},${state.level},${state.exp},${state.oldStats.join('|')},${state.stats.join('|')},${state.hp},${state.parent},${state.phaseEnded},${state.highest},${state.validated}`);
}
function getExp(level,growth){return new Function(`${wrapper};return getLevelTotalExp;`)()(level,growth);}
const output=`${rows.join('\n')}\n`,file=resolve('pokerogue-tests/fixtures/level-up.csv');
if(process.argv.includes('--check'))assert.equal(readFileSync(file,'utf8'),output);else writeFileSync(file,output);
console.log('Level-up oracle current.');
