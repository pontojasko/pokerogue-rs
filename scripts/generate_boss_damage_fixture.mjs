import {readFileSync,writeFileSync} from 'node:fs';
import assert from 'node:assert/strict';
const root=new URL('../',import.meta.url),src=readFileSync(new URL('original/src/utils/damage.ts',root),'utf8'),common=readFileSync(new URL('original/src/utils/common.ts',root),'utf8');
function body(text,marker){const at=text.indexOf(marker);assert.ok(at>=0,marker);const open=text.indexOf('{',at);let d=1,end=open+1;for(;d;end++){if(text[end]==='{')d++;else if(text[end]==='}')d--;}return text.slice(open+1,end-1);}
const helper=new Function('toDmgValue',`return function calculateBossSegmentDamage(damage,currentHp,segmentHp,minSegmentIndex=0,currentSegmentIndex){${body(src,'export function calculateBossSegmentDamage(')}}`)(((value)=>new Function('value',body(common,'export function toDmgValue('))(value)));
const rows=['# source=7c0881255af5cd51992e513ff93c2f71adbe40e0; source calculateBossSegmentDamage + toDmgValue executed'];
for(const [d,h,s,min,idx] of [[20,250,100,0,undefined],[50,250,100,0,undefined],[51,250,100,0,undefined],[150,250,100,0,undefined],[250,250,100,0,undefined],[250,250,100,1,undefined],[500,450,100,1,4],[1,100,33.333,0,undefined],[45,45,33.333,0,1],[0,10,5,0,2]]){const out=helper(d,h,s,min,idx);rows.push(`boss_damage,${d},${h},${s},${min},${idx??'-'},${out[0]},${out[1]}`)}
const data=rows.join('\n')+'\n',path=new URL('pokerogue-tests/fixtures/boss-damage.csv',root);if(process.argv.includes('--check'))assert.equal(readFileSync(path,'utf8'),data);else writeFileSync(path,data);console.log('Boss segment damage oracle current.');
