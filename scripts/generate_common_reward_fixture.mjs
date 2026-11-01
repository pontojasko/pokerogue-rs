import {readFileSync, writeFileSync} from 'node:fs';
import {stripTypeScriptTypes} from 'node:module';
import assert from 'node:assert/strict';
const root=new URL('../',import.meta.url);
const src=readFileSync(new URL('original/src/modifier/init-modifier-pools.ts',root),'utf8');
function body(marker){const at=src.indexOf(marker);assert.ok(at>=0);const open=src.indexOf('{',at);let end=open+1,n=1;for(;n;end++){if(src[end]==='{')n++;if(src[end]==='}')n--;}return src.slice(open+1,end-1);}
const pokemonSource=readFileSync(new URL('original/src/field/pokemon.ts',root),'utf8');
const ratioStart=pokemonSource.indexOf('  getHpRatio(precise = false): number {');
const ratioEnd=pokemonSource.indexOf('\n  }',ratioStart);
assert.ok(ratioStart>0 && ratioEnd>ratioStart);
const ratioJs=stripTypeScriptTypes('function getHpRatio'+pokemonSource.slice(ratioStart+'  getHpRatio'.length,ratioEnd+4));
const sourceHpRatio=new Function(`${ratioJs};return getHpRatio;`)();
const setup=stripTypeScriptTypes(`function initCommonModifierPool(){${body('function initCommonModifierPool()')}} function lureWeightFunc(maxBattles:number,weight:number){${body('function lureWeightFunc(')}} function hasMaximumBalls(ballType:any){${body('function hasMaximumBalls(')}}`);
const ids=['POKEBALL','RARE_CANDY','POTION','SUPER_POTION','ETHER','MAX_ETHER','LURE','TEMP_STAT_STAGE_BOOSTER','BERRY','TM_COMMON'];
const rows=['# source=7c0881255af5cd51992e513ff93c2f71adbe40e0; executed complete initCommonModifierPool weight callbacks and lure/ball helpers; no cap filtering or generated-type execution'];
const contexts=[
 [1,1,5,[],[[100,100,0,35,0]]],
 [1,1,5,[],[[875,1000,0,35,0]]],
 [1,1,5,[],[[874,1000,0,35,0]]],
 [1,1,5,[],[[754,1000,0,35,0]]],
 [1,1,5,[],[[755,1000,0,35,0]]],
 [1,1,99,[],[[87,100,0,35,31]]],
 [1,1,5,[],[[88,100,0,35,30]]],
 [1,1,5,[],[[100,100,0,35,30],[100,100,0,35,0]]],
 [1,1,5,[],[[75,100,0,10,6]]],
 [1,1,5,[],[[76,100,0,10,5]]],
 [1,1,5,[],[[0,100,0,10,10]]],
 [0,199,99,[[10,6]],[[1,100,1,10,10]]],
 [1,199,5,[],[[1,100,0,10,9]]],
 [1,2,5,[[10,5]],Array(6).fill([1,100,0,10,9])],
 [1,2,5,[[10,6],[15,15]],[[90,100,0,10,9]]],
 [1,1,5,[],[[100,100,0,25,0],[100,100,0,35,0]]],
 [1,19,5,[],[[18,160,0,25,20],[0,22,0,25,0]]],
 [1,19,5,[],[[100,300,0,35,0],[100,300,0,35,0]]],
];
let recordedIds=false;
for(const [classic,wave,balls,lures,members] of contexts){
 const pool={};
 const scene={gameMode:{isClassic:!!classic},currentBattle:{waveIndex:wave},pokeballCounts:[balls],getModifiers:()=>lures.map(([max,count])=>({getMaxBattles:()=>max,getBattleCount:()=>count}))};
 class Weighted{constructor(id,weight){this.id=id;this.weight=weight;}setTier(){}}
 class Berry{};
 const init=new Function('modifierPool','modifierTypes','ModifierTier','WeightedModifierType','globalScene','DoubleBattleChanceBoosterModifier','PokeballType','MAX_PER_TYPE_POKEBALLS','BerryModifier','BerryType',`${setup};return initCommonModifierPool;`)(pool,Object.fromEntries(ids.map(id=>[id,id])),{COMMON:0},Weighted,scene,class{},{POKEBALL:0},99,Berry,{LEPPA:1});
 init();assert.deepEqual(pool[0].map(p=>p.id),ids);
 if(!recordedIds){ rows.push(`ids,${pool[0].map(p=>p.id).join('|')}`); recordedIds=true; }
 const party=members.map(([hp,max,leppa,pp,used])=>({hp,getInverseHp:()=>max-hp,getMaxHp:()=>max,getHpRatio:()=>sourceHpRatio.call({hp,getMaxHp:()=>max}),isFainted:()=>hp<=0,getHeldItems:()=>leppa?[Object.assign(new Berry(),{berryType:1})]:[],getMoveset:()=>[{ppUsed:used,getMovePp:()=>pp}]}));
 const weights=pool[0].map(p=>typeof p.weight==='function'?p.weight(party):p.weight);
 rows.push(`common,${classic},${wave},${balls},${lures.map(v=>v.join(':')).join('|')||'-'},${members.map(v=>v.join(':')).join('|')},${weights.join('|')}`);
}
const out=rows.join('\n')+'\n',path=new URL('pokerogue-tests/fixtures/common-rewards.csv',root);
if(process.argv.includes('--check'))assert.equal(readFileSync(path,'utf8'),out);else writeFileSync(path,out);
console.log('Complete common pool weight callbacks verified.');
