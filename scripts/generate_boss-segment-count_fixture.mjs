import { readFileSync, writeFileSync } from 'node:fs';
import assert from 'node:assert/strict';

const root = new URL('../', import.meta.url);
const source = readFileSync(new URL('original/src/battle-scene.ts', root), 'utf8');
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

const body = methodBody(source, 'getEncounterBossSegments(waveIndex: number')
  .replace('let isBoss: boolean | undefined;', 'let isBoss;');
const rows = ['# source=7c0881255af5cd51992e513ff93c2f71adbe40e0; BattleScene.getEncounterBossSegments executed; wave10 short-circuit avoids RNG'];
for (const [wave, level, baseTotal, forceBoss] of [
  [10, 5, 600, false], [10, 100, 600, false], [10, 5, 670, false],
  [20, 100, 670, false], [250, 100, 670, true],
]) {
  globalThis.activeOverrides = { ENEMY_HEALTH_SEGMENTS_OVERRIDE: 0 };
  globalThis.randSeedInt = () => { throw new Error('unexpected RNG call'); };
  const scene = {
    gameMode: { isDaily: false, hasRandomBosses: false, isWaveFinal: () => false },
    executeWithSeedOffset(callback) { callback(); },
  };
  const species = { subLegendary: false, legendary: false, mythical: false, baseTotal };
  const result = new Function(`return function getEncounterBossSegments(waveIndex, level, species, forceBoss) { ${body} }`)()
    .call(scene, wave, level, species, forceBoss);
  rows.push(`boss_segments,${wave},${level},${baseTotal},${forceBoss ? 1 : 0},${result}`);
}
const output = `${rows.join('\n')}\n`;
const path = new URL('pokerogue-tests/fixtures/boss-segments.csv', root);
if (process.argv.includes('--check')) assert.equal(readFileSync(path, 'utf8'), output);
else writeFileSync(path, output);
console.log('Boss segment-count oracle current.');
