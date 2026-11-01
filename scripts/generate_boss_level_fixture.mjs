import { readFileSync, writeFileSync } from 'node:fs';
import assert from 'node:assert/strict';

const root = new URL('../', import.meta.url);
const source = readFileSync(new URL('original/src/battle.ts', root), 'utf8');
function methodBody(text, marker) {
  const at = text.indexOf(marker);
  assert.ok(at >= 0, `missing ${marker}`);
  const open = text.indexOf('{', at);
  let depth = 1;
  let end = open + 1;
  for (; depth; end++) {
    if (text[end] === '{') depth++;
    else if (text[end] === '}') depth--;
  }
  return text.slice(open + 1, end - 1);
}

const body = methodBody(source, 'public getLevelForWave(): number {');
const rows = ['# source=7c0881255af5cd51992e513ff93c2f71adbe40e0; Battle.getLevelForWave executed; realInRange calls recorded'];
for (const [wave, real, waveFinal] of [
  [10, -1, false], [10, -0.5, false], [10, 0, false], [10, 0.5, false], [10, 1, false],
  [20, -1, false], [20, 0, false], [20, 1, false],
  [200, 0.75, true],
]) {
  const calls = [];
  globalThis.isDailyFinalBoss = () => false;
  globalThis.DAILY_BOSS_LEVEL = 100;
  globalThis.Phaser = { Math: { RND: { realInRange(min, max) { calls.push([min, max]); return min + (real + 1) * (max - min) / 2; } } } };
  const evaluate = new Function(`return function getLevelForWave() { ${body} }`)();
  const gameMode = {
    getWaveForDifficulty: index => index,
    isBoss: index => index % 10 === 0,
    isWaveFinal: index => index === 200,
  };
  const battle = { waveIndex: wave, gameMode, isClassicFinalBoss: waveFinal };
  const level = evaluate.call(battle);
  rows.push(`boss_level,${wave},${real},${waveFinal ? 1 : 0},${level},${calls.length},${calls.map(c => `${c[0]}:${c[1]}`).join('|') || '-'}`);
}
const output = `${rows.join('\n')}\n`;
const path = new URL('pokerogue-tests/fixtures/boss-level.csv', root);
if (process.argv.includes('--check')) assert.equal(readFileSync(path, 'utf8'), output);
else writeFileSync(path, output);
console.log('Boss level oracle current.');
