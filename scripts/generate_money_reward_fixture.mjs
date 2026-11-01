#!/usr/bin/env node
// Execute pinned getWaveMoneyAmount, MoneyRewardModifier.apply and addMoney bodies.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve } from 'node:path';
const pin='7c0881255af5cd51992e513ff93c2f71adbe40e0';
function bodyAfter(source,at){const open=source.indexOf('{',at);assert.ok(open>=0);let d=1,q=null,e=false,l=false,c=false;for(let i=open+1;i<source.length;i++){const x=source[i],n=source[i+1];if(l){if(x==='\n')l=false;continue;}if(c){if(x==='*'&&n==='/'){c=false;i++;}continue;}if(q){if(e)e=false;else if(x==='\\')e=true;else if(x===q)q=null;continue;}if(x==='/'&&n==='/'){l=true;i++;continue;}if(x==='/'&&n==='*'){c=true;i++;continue;}if('"\'`'.includes(x))q=x;else if(x==='{')d++;else if(x==='}'&&--d===0)return source.slice(open+1,i);}throw Error('unclosed source body');}
const sceneSrc=readFileSync('original/src/battle-scene.ts','utf8');
const moneyBody=bodyAfter(sceneSrc,sceneSrc.indexOf('getWaveMoneyAmount(moneyMultiplier: number): number'));
const addBody=bodyAfter(sceneSrc,sceneSrc.indexOf('addMoney(amount: number): void'));
const modSrc=readFileSync('original/src/modifier/modifier.ts','utf8');
const classAt=modSrc.indexOf('export class MoneyRewardModifier extends ConsumableModifier');assert.ok(classAt>=0);
const applyBody=bodyAfter(modSrc,modSrc.indexOf('override apply(): boolean',classAt));
const multiplierClass=modSrc.indexOf('export class MoneyMultiplierModifier extends PersistentModifier');assert.ok(multiplierClass>=0);
const multiplierBody=bodyAfter(modSrc,modSrc.indexOf('override apply(multiplier: NumberHolder): boolean',multiplierClass));
const code=stripTypeScriptTypes(`
class NumberHolder{constructor(value){this.value=value;}} const MoneyAchv={};
class Scene{constructor(wave,money){this.currentBattle={waveIndex:wave};this.money=money;this.gameMode={};}getWaveMoneyAmount(moneyMultiplier){${moneyBody}}addMoney(amount){this.rewardAmount=amount;${addBody}}}
class MoneyRewardModifier{constructor(multiplier){this.moneyMultiplier=multiplier;}apply(){${applyBody}}}
class MoneyMultiplierModifier{constructor(stackCount){this.stackCount=stackCount;}getStackCount(){return this.stackCount;}apply(multiplier){${multiplierBody}}}
`);
const execute=new Function('wave','initialMoney','multiplier',`${code};const scene=new Scene(wave,initialMoney);globalThis.globalScene=scene;scene.applyModifiers=()=>{};scene.getPlayerParty=()=>[];scene.updateMoneyText=()=>{};scene.animateMoneyChanged=()=>{};scene.validateAchvs=()=>{};new MoneyRewardModifier(multiplier).apply();return {money:scene.money,amount:scene.rewardAmount};`);
const MAX=Number.MAX_SAFE_INTEGER;
const rows=[`# source=${pin}; executes original BattleScene.getWaveMoneyAmount/addMoney and MoneyRewardModifier.apply`, '# scenario,wave,multiplier,initial_money,reward_amount,final_money'];
for(const [name,wave,initial,multiplier] of [['wave-1',1,0,1],['wave-10',10,0,1],['wave-11',11,0,1],['wave-20',20,0,1],['wave-19',19,0,1],['wallet-cap',1,MAX-5,1],['big-nugget-wave-1',1,0,2.5],['big-nugget-wave-19',19,0,2.5],['big-nugget-wave-20',20,0,2.5],['big-nugget-wallet-cap',1,MAX-5,2.5]]){
 const result=execute(wave,initial,multiplier); rows.push(`${name},${wave},${multiplier},${initial},${result.amount},${result.money}`);
}
const applyMoneyMultiplier=new Function('stacks','amount',`${code};const holder=new NumberHolder(amount);new MoneyMultiplierModifier(stacks).apply(holder);return holder.value;`);
for(const [stacks,amount] of [[0,100],[1,100],[2,155],[5,100]]) rows.push(`money_modifier,${stacks},${amount},${applyMoneyMultiplier(stacks,amount)}`);
const output=`${rows.join('\n')}\n`,path=resolve('pokerogue-tests/fixtures/money-reward.csv');
if(process.argv.includes('--check'))assert.equal(readFileSync(path,'utf8'),output);else writeFileSync(path,output);
console.log('Money reward oracle current.');
