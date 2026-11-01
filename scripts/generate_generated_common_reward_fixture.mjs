#!/usr/bin/env node
// Executes the pinned generator callbacks used by COMMON PLAYER rewards.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const source = readFileSync(resolve(root, 'original/src/modifier/modifier-type.ts'), 'utf8');
function body(marker, from = 0) {
  const at = source.indexOf(marker, from); assert.ok(at >= 0, `missing ${marker}`);
  const open = source.indexOf('{', at); let depth = 1, quote = null, escaped = false;
  for (let i = open + 1; i < source.length; i++) {
    const ch = source[i];
    if (quote) { if (escaped) escaped = false; else if (ch === '\\') escaped = true; else if (ch === quote) quote = null; }
    else if (ch === '"' || ch === "'" || ch === '`') quote = ch;
    else if (ch === '{') depth++;
    else if (ch === '}' && --depth === 0) return source.slice(open + 1, i);
  }
  throw new Error(`unterminated ${marker}`);
}
class Alea {
  constructor(seed) { this.c=1; this.n=0xefc8249d; this.s0=this.hash(' '); this.s1=this.hash(' '); this.s2=this.hash(' '); this.s0=(this.s0-this.hash(seed)+1)%1; this.s1=(this.s1-this.hash(seed)+1)%1; this.s2=(this.s2-this.hash(seed)+1)%1; }
  hash(text) { for (let i=0;i<text.length;i++) { this.n+=text.charCodeAt(i); let h=0.02519603282416938*this.n; this.n=Math.trunc(h)>>>0; h-=this.n; h*=this.n; this.n=Math.trunc(h)>>>0; h-=this.n; this.n+=h*4294967296; } return (this.n>>>0)/4294967296; }
  rnd() { const t=2091639*this.s0+this.c*(1/4294967296); this.c=Math.trunc(t); this.s0=this.s1; this.s1=this.s2; this.s2=t-this.c; return this.s2; }
  frac() { const a=this.rnd(); return a+Math.floor(this.rnd()*2097152)*(1/9007199254740992); }
  int(n) { return Math.floor(this.frac()*n); }
}
const tempBody = body('super((_party: readonly Pokemon[], pregenArgs?: any[]) => {', source.indexOf('class TempStatStageBoosterModifierTypeGenerator'))
  .replace('const randStat: TempBattleStat =', 'const randStat =');
const berryBody = body('new ModifierTypeGenerator((_party: readonly Pokemon[], pregenArgs?: any[]) => {', source.indexOf('  BERRY: () =>'))
  .replace('let randBerryType: BerryType;', 'let randBerryType;')
  .replace('pregenArgs[0] as BerryType', 'pregenArgs[0]');
const tmBody = body('super((party: readonly Pokemon[], pregenArgs?: any[]) => {', source.indexOf('class TmModifierTypeGenerator'))
  .replace('(p as PlayerPokemon)', 'p')
  .replace('pregenArgs[0] as MoveId', 'pregenArgs[0]');
const stat = { ATK:1, DEF:2, SPATK:3, SPDEF:4, SPD:5, ACC:6 };
const rows = [`# source=${pin}; exact extracted TempStatStageBoosterModifierTypeGenerator, BERRY and TmModifierTypeGenerator callbacks; supplied TM candidates`];
const cases = ['TEMP_STAT_STAGE_BOOSTER','BERRY','TM_COMMON','TM_COMMON_EMPTY'];
for (const id of cases) {
  let found = false;
  for (let i=0; i<500 && !found; i++) {
    const seed=`GeneratedReward${String(i).padStart(4,'0')}`, rng=new Alea(seed), draws=[];
    const randSeedInt=(range,min=0)=>{ const value=min+rng.int(range); draws.push(`${range}:${min}:${value}`); return value; };
    let output;
    try {
      const generator = id==='TEMP_STAT_STAGE_BOOSTER'
        ? new Function('TEMP_BATTLE_STATS','randSeedInt','Stat','TempStatStageBoosterModifierType',`return function(party,pregenArgs){${tempBody}}`)(Object.values(stat),randSeedInt,stat,class { constructor(stat) { this.stat=stat; } })
        : id==='BERRY'
          ? new Function('BerryType','getEnumValues','randSeedInt','BerryModifierType',`return function(party,pregenArgs){${berryBody}}`)({SITRUS:0,LUM:1,ENIGMA:2,LIECHI:3,GANLON:4,PETAYA:5,APICOT:6,SALAC:7,LANSAT:8,STARF:9,LEPPA:10},e=>Object.keys(e).filter(k=>Number.isNaN(Number(k))).map(k=>e[k]),randSeedInt,class { constructor(berryType) { this.berryType=berryType; } })
          : new Function('tmPoolTiers','tier','allMoves','randSeedInt','TmModifierType',`return function(party,pregenArgs){${tmBody}}`)({TACKLE:0,GROWL:0},0,{TACKLE:{isUnimplemented:false},GROWL:{isUnimplemented:false}},randSeedInt,class { constructor(id) { this.id=id; } });
      const party = id==='TM_COMMON' ? [{getCompatibleTms:()=>['TACKLE','GROWL']}] : [];
      output=generator(party,undefined);
    } catch (e) { throw new Error(`${id}: ${e.stack}`); }

    const value=typeof output==='number'?output:(output?.stat ?? output?.berryType ?? output?.id);
    if (id==='TM_COMMON_EMPTY' && output===null && draws.length===0) {
      rows.push(`${id},${seed},NONE,-`); found=true;
    } else if (id==='TM_COMMON' || (id==='BERRY' && draws.length===2) || (id==='TEMP_STAT_STAGE_BOOSTER' && draws.length===1)) {
      rows.push(`${id},${seed},${value},${draws.join('|')}`); found=true;
    }
  }
  assert.ok(found,`failed to create ${id} fixture`);
}
const output=`${rows.join('\n')}\n`, path=resolve(root,'pokerogue-tests/fixtures/generated-common-rewards.csv');
if (process.argv.includes('--check')) assert.equal(readFileSync(path,'utf8'),output); else writeFileSync(path,output);
console.log('Generated COMMON reward oracle current.');
