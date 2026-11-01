#!/usr/bin/env node
// Execute pinned PokemonMove.getMovePp, PP_UP eligibility, and modifier apply methods.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve } from 'node:path';
const pin='7c0881255af5cd51992e513ff93c2f71adbe40e0';
function bodyAfter(source,at){const open=source.indexOf('{',at);assert.ok(open>=0);let d=1,q=null,e=false,l=false,c=false;for(let i=open+1;i<source.length;i++){const x=source[i],n=source[i+1];if(l){if(x==='\n')l=false;continue;}if(c){if(x==='*'&&n==='/'){c=false;i++;}continue;}if(q){if(e)e=false;else if(x==='\\')e=true;else if(x===q)q=null;continue;}if(x==='/'&&n==='/'){l=true;i++;continue;}if(x==='/'&&n==='*'){c=true;i++;continue;}if('"\'`'.includes(x))q=x;else if(x==='{')d++;else if(x==='}'&&--d===0)return source.slice(open+1,i);}throw Error('unclosed source body');}
const moveSrc=readFileSync('original/src/data/moves/pokemon-move.ts','utf8');
const moveBody=bodyAfter(moveSrc,moveSrc.indexOf('getMovePp(): number'));
const modSrc=readFileSync('original/src/modifier/modifier.ts','utf8');
const clazz=modSrc.indexOf('export class PokemonPpUpModifier extends ConsumablePokemonMoveModifier');
const applyBody=bodyAfter(modSrc,modSrc.indexOf('override apply(playerPokemon: PlayerPokemon): boolean',clazz));
const typeSrc=readFileSync('original/src/modifier/modifier-type.ts','utf8');
const typeAt=typeSrc.indexOf('export class PokemonPpUpModifierType');
const filterAt=typeSrc.indexOf('(pokemonMove: PokemonMove) => {',typeAt);
const filterBody=bodyAfter(typeSrc,filterAt);
const harness=stripTypeScriptTypes(`
function toDmgValue(value){return Math.max(Math.floor(value),1)}
class PokemonMove{constructor(pp,ppUp=0,maxPpOverride){this.basePp=pp;this.ppUp=ppUp;this.maxPpOverride=maxPpOverride;}getMove(){return {pp:this.basePp}}getMovePp(){${moveBody}}}
function eligible(pokemonMove){${filterBody}}
const PartyUiHandler={NoEffectMessage:'no effect'};
function apply(move,upPoints){const playerPokemon={getMoveset:()=>[move]};const modifier={moveIndex:0,upPoints};return (function(){${applyBody}}).call(modifier)}
`);
const execute=new Function(`${harness};return (pp,ups,override,points)=>{const move=new PokemonMove(pp,ups,override);const canApply=eligible(move)===null;if(canApply)apply(move,points);return {eligible:canApply,ppUp:move.ppUp,maxPp:move.getMovePp()}};`)();
const scenarios=[['tackle-first',35,0,0,1],['tackle-near-cap',35,2,0,1],['already-capped',35,3,0,1],['ember',25,1,0,1],['override',35,0,20,3],['low-base-pp',4,0,0,1],['pp-max',35,0,0,3]];
const rows=[`# source=${pin}; executes PokemonMove.getMovePp, PP_UP/PP_MAX move filter, and PokemonPpUpModifier.apply`,'# scenario,base_pp,initial_pp_up,max_pp_override,up_points,eligible,final_pp_up,final_max_pp'];
for(const [name,pp,ups,override,points] of scenarios){const result=execute(pp,ups,override||undefined,points);rows.push(`${name},${pp},${ups},${override},${points},${Number(result.eligible)},${result.ppUp},${result.maxPp}`)}
const output=`${rows.join('\n')}\n`,path=resolve('pokerogue-tests/fixtures/pp-up.csv');
if(process.argv.includes('--check'))assert.equal(readFileSync(path,'utf8'),output);else writeFileSync(path,output);
console.log('PP Up source oracle current.');
