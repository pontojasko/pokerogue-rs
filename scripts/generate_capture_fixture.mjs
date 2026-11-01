#!/usr/bin/env node
// Executes the pinned catch-rate helpers and AttemptCapturePhase formulas for simple ordinary catches.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const root = resolve(import.meta.dirname, '..');
const original = resolve(root, 'original/src');
const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const phase = readFileSync(resolve(original, 'phases/attempt-capture-phase.ts'), 'utf8').replace(/\/\/.*$/gm, '');
const battleScene = readFileSync(resolve(original, 'battle-scene.ts'), 'utf8');
const balls = readFileSync(resolve(original, 'data/pokeball.ts'), 'utf8');
const imported = readFileSync(resolve(root, 'pokerogue-tests/fixtures/imported-demo-data.csv'), 'utf8')
  .split(/\r?\n/).filter((row) => row.startsWith('species,')).map((row) => row.split(','));
const bulbasaur = imported.find((row) => row[1] === 'BULBASAUR');
assert.ok(bulbasaur, 'Bulbasaur imported source record unavailable');
function block(source, marker) {
  const at = source.indexOf(marker);
  assert.notEqual(at, -1, `missing source block ${marker}`);
  const open = source.indexOf('{', at);
  let depth = 0;
  for (let i = open; i < source.length; i++) {
    if (source[i] === '{') depth++;
    else if (source[i] === '}' && --depth === 0) return source.slice(open + 1, i);
  }
  throw new Error(`unclosed source block ${marker}`);
}
function uint32(n) { return Math.trunc(n) >>> 0; }
class Alea {
  constructor(seed) {
    this.c = 1; this.n = 0xefc8249d;
    this.s0 = this.hash(' '); this.s1 = this.hash(' '); this.s2 = this.hash(' ');
    this.s0 = (this.s0 - this.hash(seed) + 1) % 1;
    this.s1 = (this.s1 - this.hash(seed) + 1) % 1;
    this.s2 = (this.s2 - this.hash(seed) + 1) % 1;
  }
  hash(text) {
    for (let i = 0; i < text.length; i++) {
      this.n += text.charCodeAt(i); let h = 0.02519603282416938 * this.n;
      this.n = uint32(h); h -= this.n; h *= this.n; this.n = uint32(h); h -= this.n;
      this.n += h * 4294967296;
    }
    return uint32(this.n) / 4294967296;
  }
  rnd() {
    const t = 2091639 * this.s0 + this.c / 4294967296;
    this.c = Math.trunc(t); this.s0 = this.s1; this.s1 = this.s2; this.s2 = t - this.c;
    return this.s2;
  }
  frac() { const a = this.rnd(); return a + Math.floor(this.rnd() * 2097152) / 9007199254740992; }
  integer(range) { return Math.floor(this.frac() * range); }
}
const shiftSeed = (seed, shift) => Array.from(seed, (ch) => String.fromCharCode(ch.charCodeAt(0) + shift)).join('');
const pokeballEnum = readFileSync(resolve(original, 'enums/pokeball.ts'), 'utf8');
const enumBody = block(pokeballEnum, 'export enum PokeballType');
const ballIds = Object.fromEntries([...enumBody.matchAll(/([A-Z_]+)\s*,/g)].map((match, i) => [match[1], i]));
const multiplierBody = block(balls, 'export function getPokeballCatchMultiplier(');
const getMultiplier = new Function('PokeballType', `return function(type) { ${multiplierBody} };`)(ballIds);
const captureSource = phase.match(/const modifiedCatchRate = Math\.round\(([\s\S]*?)\n\s*\);/)?.[1];
const shakeSource = phase.match(/const shakeProbability = Math\.round\(([^;]+)\);/)?.[1];
assert.ok(captureSource && shakeSource, 'capture formulas not found in source');
const modifiedRate = new Function('_3m', '_2h', 'catchRate', 'pokeballMultiplier', 'statusMultiplier', 'shinyMultiplier', `return Math.round(${captureSource});`);
const shakeChance = new Function('modifiedCatchRate', `return Math.round(${shakeSource});`);
const criticalBody = block(readFileSync(resolve(original, 'data/pokeball.ts'), 'utf8'), 'export function getCriticalCaptureChance(');
const NumberHolder = class { constructor(value) { this.value = value; } };
const getCritical = new Function('globalScene', 'NumberHolder', `return function(modifiedCatchRate) { ${criticalBody} };`)(
  { gameMode: { isFreshStartChallenge: () => false, isDaily: false }, gameData: { getSpeciesCount: () => 0 }, findModifier: () => undefined },
  NumberHolder,
);
function scenario(seed) {
  const rng = new Alea(shiftSeed(seed, 64));
  const calls = [];
  const roll = (range) => { const value = rng.integer(range); calls.push(`I:${range}:0:${value}`); return value; };
  const maxHp = 44, hp = 22, catchRate = Number(bulbasaur[14]);
  const ball = ballIds.POKEBALL;
  const ballMultiplier = getMultiplier(ball);
  const statusMultiplier = 1, shinyMultiplier = 1;
  const _3m = 3 * maxHp, _2h = 2 * hp;
  const modified = modifiedRate(_3m, _2h, catchRate, ballMultiplier, statusMultiplier, shinyMultiplier);
  const probability = shakeChance(modified);
  const criticalChance = getCritical(modified);
  const critical = roll(256) < criticalChance;
  const shakes = [];
  let caught = true;
  const checkCount = critical ? 1 : 4;
  if (ballMultiplier !== -1 && modified < 255) {
    for (let i = 0; i < checkCount; i++) {
      const value = roll(65536); shakes.push(value);
      if (value >= probability) { caught = false; break; }
    }
  }
  return { seed, catchRate, hp, maxHp, ball, ballMultiplier, statusMultiplier, shinyMultiplier,
    criticalChance, modified, probability, critical, shakes, caught, calls };
}
let fail, success;
for (let i = 0; i < 20000 && (!fail || !success); i++) {
  const result = scenario(`CaptureOracleSeed${String(i).padStart(5, '0')}`);
  if (!result.caught && !fail) fail = result;
  if (result.caught && !success) success = result;
}
assert.ok(fail && success, 'could not find deterministic catch and failure seeds');
const rows = [`# source=${pin}; AttemptCapturePhase formulas, getPokeballCatchMultiplier and getCriticalCaptureChance executed with fresh dex/no status/no shiny/no modifiers`];
const initialBalls = Number(battleScene.match(/this\.pokeballCounts\[PokeballType\.POKEBALL\]\s*=\s*(\d+)/)?.[1]);
assert.equal(initialBalls, 5, 'initial Poké Ball count changed in source');
rows.push(`initial_balls,${pin},${initialBalls}`);
for (let ball = 0; ball < 6; ball++) {
  rows.push(`ball_multiplier,${pin},${ball},${getMultiplier(ball)}`);
}
for (const result of [fail, success]) {
  rows.push(`capture,${pin},${result.seed},${bulbasaur[2]},${result.catchRate},${result.hp},${result.maxHp},${result.ball},${result.ballMultiplier},${result.statusMultiplier},${result.shinyMultiplier},${result.criticalChance},${result.modified},${result.probability},${Number(result.critical)},${result.shakes.join('|')},${Number(result.caught)},${result.calls.join('|')}`);
}
const path = resolve(root, 'pokerogue-tests/fixtures/capture-attempt.csv');
const output = `${rows.join('\n')}\n`;
if (process.argv.includes('--check')) {
  assert.equal(readFileSync(path, 'utf8'), output, 'capture oracle fixture stale');
  console.log('Capture oracle fixture is current.');
} else {
  writeFileSync(path, output);
  console.log('Wrote source-executed capture attempt fixtures.');
}
