import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import assert from 'node:assert/strict';
const root = new URL('../', import.meta.url);
const read = p => readFileSync(new URL(p, root), 'utf8');
const types = read('original/src/modifier/modifier-type.ts');
const mods = read('original/src/modifier/modifier.ts');
function block(source, marker) {
  const start = source.indexOf(marker); assert.ok(start >= 0, marker);
  const open = source.indexOf('{', start); let n=1, end=open+1;
  for (; n; end++) { if(source[end]==='{') n++; if(source[end]==='}') n--; }
  return source.slice(open+1,end-1);
}
const ids = ['POKEBALL','POTION','SUPER_POTION'];
const common = block(read('original/src/modifier/init-modifier-pools.ts'), 'function initCommonModifierPool()');
for (const id of ids) assert.ok(common.includes(`modifierTypes.${id},`));
const factories = Object.fromEntries(ids.map(id => [id, () => ({id, withIdFromFunc(){return this;}, withTierFromPool(){this.tier=0;return this;}})]));
const optionsBody = block(types, 'export function getPlayerModifierTypeOptions(');
const js = stripTypeScriptTypes(`function execute(count: number, party: any[], modifierTiers: any, customModifierSettings: any) {${optionsBody}}`);
const generate = new Function('modifierTypeInitObj','ModifierPoolType','ModifierTypeGenerator','ModifierTypeOption','overridePlayerModifierTypeOptions','getModifierTypeOptionWithRetry', `${js};return execute;`)(factories,{PLAYER:0},class {},class {constructor(type){this.type=type;}},()=>{},()=>{throw Error('unexpected random generation');});
const options = generate(3,[],undefined,{guaranteedModifierTypeFuncs:ids.map(id=>factories[id]),fillRemaining:false});
const rows = ['# source=7c0881255af5cd51992e513ff93c2f71adbe40e0; executed guaranteed options function and consumable apply bodies; mocked identity/tier lookup, no Phaser phases'];
options.forEach(o=>rows.push(`option,${o.type.id},${o.type.tier}`));
const applyBody = block(mods.slice(mods.indexOf('export class PokemonHpRestoreModifier')), 'override apply(');
const heal = new Function('pokemon','multiplier',applyBody);
for(const [id,points,percent] of [['POTION',20,10],['SUPER_POTION',50,25],['HYPER_POTION',200,50],['MAX_POTION',0,100],['FULL_RESTORE',0,100],['REVIVE',0,50],['MAX_REVIVE',0,100]]) {
 const fainted = id === 'REVIVE' || id === 'MAX_REVIVE';
 if (fainted) {
  assert.ok(new RegExp(`${id}:[\\s\\S]*?new PokemonReviveModifierType[^;]*, ${percent}\\)`,'s').test(types));
 } else if(id==='FULL_RESTORE') {
  assert.ok(/FULL_RESTORE:[\s\S]*?new PokemonHpRestoreModifierType\([^;]*0, 100, true\)/s.test(types));
 } else {
  assert.ok(new RegExp(`${id}:[\\s\\S]*?${points}, ${percent}\\)`).test(types));
 }
 for(const [hp,max] of fainted ? [[0,100],[0,1001],[1,100]] : [[1,100],[90,100],[1,1000],[0,100]]) {
  const pokemon={hp,getMaxHp:()=>max,resetStatus(){}};
  const applied=heal.call({restorePoints:points,restorePercent:percent,fainted,healStatus:id==='FULL_RESTORE'},pokemon,1);
  rows.push(`${fainted?'revive':'heal'},${id},${hp},${max},${pokemon.hp},${Number(applied)}`);
 }
}
const ballBody=block(mods.slice(mods.indexOf('export class AddPokeballModifier')), 'override apply(');
const limit=Number(read('original/src/data/pokeball.ts').match(/MAX_PER_TYPE_POKEBALLS(?:\s*:\s*number)?\s*=\s*(\d+)/)[1]);
for(const initial of [0,5,97,99]) {
 const scene={pokeballCounts:[initial]};
 new Function('globalScene','MAX_PER_TYPE_POKEBALLS',`return function(){${ballBody}}`)(scene,limit).call({pokeballType:0,count:5});
 rows.push(`balls,${initial},${scene.pokeballCounts[0]}`);
}
const output=rows.join('\n')+'\n', path=new URL('pokerogue-tests/fixtures/rewards.csv',root);
if(process.argv.includes('--check')) assert.equal(readFileSync(path,'utf8'),output); else writeFileSync(path,output);
console.log('Guaranteed rewards and consumable TypeScript oracle verified.');
