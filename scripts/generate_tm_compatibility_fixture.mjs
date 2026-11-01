#!/usr/bin/env node
// Executes original getCompatibleTms and TmModifierTypeGenerator on imported species data.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const pin='7c0881255af5cd51992e513ff93c2f71adbe40e0';
const fixture=readFileSync(resolve(root,'pokerogue-tests/fixtures/imported-demo-data.csv'),'utf8');
const pokemon=readFileSync(resolve(root,'original/src/field/pokemon.ts'),'utf8');
const modifier=readFileSync(resolve(root,'original/src/modifier/modifier-type.ts'),'utf8');
const registrySource=readFileSync(resolve(root,'original/src/data/species-data-registry.ts'),'utf8');
const tmTiersSource=readFileSync(resolve(root,'original/src/data/balance/tm-pool-tiers.ts'),'utf8');
function body(source,marker,from=0){const at=source.indexOf(marker,from);assert.ok(at>=0,`missing ${marker}`);const open=source.indexOf('{',at);let depth=1,quote=null,escaped=false;for(let i=open+1;i<source.length;i++){const c=source[i];if(quote){if(escaped)escaped=false;else if(c==='\\')escaped=true;else if(c===quote)quote=null;}else if(c==='"'||c==="'"||c==='`')quote=c;else if(c==='{')depth++;else if(c==='}'&&--depth===0)return source.slice(open+1,i);}throw new Error(`unclosed ${marker}`);}
const csv=fixture.split(/\r?\n/).filter(line=>line&&!line.startsWith('#')).map(line=>line.split(','));
const species=new Map(csv.filter(f=>f[0]==='species').map(f=>[f[1],Number(f[2])]));
const speciesTms=new Map();for(const f of csv.filter(f=>f[0]==='species_tm')){const a=speciesTms.get(f[1])??[];a.push(Number(f[5]));speciesTms.set(f[1],a);}
const levelMoves=new Map();for(const f of csv.filter(f=>f[0]==='level_move')){const a=levelMoves.get(f[1])??[];a.push(Number(f[5]));levelMoves.set(f[1],a);}
const moves=new Map(csv.filter(f=>f[0]==='move').map(f=>[Number(f[2]),{id:f[1],unimplemented:f[10]==='1'}]));
const moveIds=Object.fromEntries([...moves].map(([numeric,record])=>[record.id,numeric]));
const tmTiers=Object.fromEntries(csv.filter(f=>f[0]==='tm_tier').map(f=>[Number(f[2]),Number(f[4])]));
const prevolutions=new Map(csv.filter(f=>f[0]==='prevolution').map(f=>[f[1],f[2]]));
const getTmsBody=body(registrySource,'public getTms(');
const getTmsJs=stripTypeScriptTypes(`function getTms(speciesId,form){${getTmsBody}}`);
const getSourceTms=new Function(`${getTmsJs};return getTms;`)();
const sourceRegistry={
 getSpeciesData(id){return {tms:speciesTms.get(id)??[],formTms:{}};},
 getFormKey(_id,form){return form??'base';},
 getPrevolution(id){const numeric=species.get(id);const parent=prevolutions.get(String(numeric));return parent===undefined?null:[...species].find(([,value])=>value===Number(parent))?.[0]??null;},
};
sourceRegistry.getTms=(id,form)=>getSourceTms.call(sourceRegistry,id,form);
const getFamilyTms=id=>getSourceTms.call(sourceRegistry,id);
const methodAt=pokemon.indexOf('public getCompatibleTms(');
const getCompatibleBody=body(pokemon,'public getCompatibleTms(',methodAt);
const getCompatibleJs=stripTypeScriptTypes(`function getCompatibleTms(excludeKnown=false,excludeLevelUp=false,excludeUsedTMs=false){${getCompatibleBody}}`)
  .replace(/\bMoveId\[\]/g,'');
const getCompatible=new Function('ChallengeType','applyChallenges',`${getCompatibleJs};return getCompatibleTms;`)({PLAYER_TM_COMPATIBILITY:0},()=>{});
const generatorBody=body(modifier,'super((party: readonly Pokemon[], pregenArgs?: any[]) => {',modifier.indexOf('class TmModifierTypeGenerator'))
  .replace('(p as PlayerPokemon)','p').replace('pregenArgs[0] as MoveId','pregenArgs[0]');
class Generator{constructor(generate){this.generate=generate;}generateType(party){return this.generate(party);}}
class TmType{constructor(moveId){this.moveId=moveId;}}
const makeSourceGenerator=new Function('Generator','tmPoolTiers','tier','allMoves','randSeedInt','TmModifierType',`return class SourceTmGenerator extends Generator { constructor(){super((party,pregenArgs)=>{${generatorBody}});} };`);
const sourceTierMap={};for(const [moveId,tier] of Object.entries(tmTiersSource.matchAll(/\[MoveId\.([A-Z0-9_]+)\]:\s*ModifierTier\.([A-Z0-9_]+)/g).reduce((out,m)=>{out[m[1]]=m[2];return out;},{}))){const numeric=moveIds[moveId];if(numeric!==undefined)sourceTierMap[numeric]=({COMMON:0,GREAT:1,ULTRA:2})[tier];}
const allMoves=Object.fromEntries([...moves].map(([id,record])=>[id,{isUnimplemented:record.unimplemented}]));
const sourceSpecies=['BULBASAUR','CHARMANDER','SILCOON'];
const knownBySpecies={BULBASAUR:['VINE_WHIP','TACKLE'],CHARMANDER:['EMBER','TACKLE'],SILCOON:['HARDEN']};
const rows=[`# source=${pin}; executes original Pokemon.getCompatibleTms and TmModifierTypeGenerator with imported base species TMs/learnsets and fixed known moves; no fusion/challenges/used TMs`];
const partyMembers=[];
for(const name of sourceSpecies){
 const known=knownBySpecies[name].map(id=>moveIds[id]);
 const partyMember={species:{getTms:()=>getFamilyTms(name)},fusionSpecies:null,moveset:known.map(moveId=>({moveId})),usedTMs:[],getFormKey:()=>undefined,getFusionFormKey:()=>undefined,getLevelMoves:()=> (levelMoves.get(name)??[]).map(moveId=>[1,moveId]),getCompatibleTms(...args){return getCompatible.call(this,...args);}};
 if(name!=='SILCOON')partyMembers.push(partyMember);
 const getTierCandidates=(tier)=>{
  const compatible=getCompatible.call(partyMember,true,true,true);
  const SourceGenerator=makeSourceGenerator(Generator,sourceTierMap,tier,allMoves,(range)=>0,TmType);
  const first=new SourceGenerator().generateType([partyMember]);
  if(first===null)return [];
  const length=compatible.filter(tm=>sourceTierMap[tm]===tier&&!allMoves[tm]?.isUnimplemented).filter((tm,i,array)=>array.indexOf(tm)===i).length;
  const values=[];
  for(let i=0;i<length;i++){
   let rangeSeen=0;
   const gen=new (makeSourceGenerator(Generator,sourceTierMap,tier,allMoves,(range)=>{rangeSeen=range;return i;},TmType))();
   const chosen=gen.generateType([partyMember]);
   assert.equal(rangeSeen,length);
   values.push(chosen.moveId);
  }
  return values;
 };
 for(const tier of [0,1])rows.push(`compatible,${name},${species.get(name)},${tier},${known.join('|')},${getTierCandidates(tier).join('|')}`);
}
for(const tier of [0,1]){
 let range=0;
 const firstGen=new (makeSourceGenerator(Generator,sourceTierMap,tier,allMoves,(n)=>{range=n;return 0;},TmType))();
 const first=firstGen.generateType(partyMembers);
 const selected=[];
 if(first!==null)for(let i=0;i<range;i++){
  const gen=new (makeSourceGenerator(Generator,sourceTierMap,tier,allMoves,(n)=>n===range?i:0,TmType))();
  selected.push(gen.generateType(partyMembers).moveId);
 }
 rows.push(`party,${tier},${selected.join('|')}`);
}
const out=`${rows.join('\n')}\n`,path=resolve(root,'pokerogue-tests/fixtures/tm-compatibility.csv');
if(process.argv.includes('--check'))assert.equal(readFileSync(path,'utf8'),out);else writeFileSync(path,out);
console.log('TM compatibility oracle current.');
