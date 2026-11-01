#!/usr/bin/env node
// Execute pinned getLevelMoves and filterAndSortLevelMoves over imported Silcoon/Wurmple rows.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve } from 'node:path';

const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const source = readFileSync('original/src/field/learnsets.ts', 'utf8');
function extractFunction(name) {
  const marker = `function ${name}(`;
  const start = source.indexOf(marker);
  assert.ok(start >= 0, `${name} missing`);
  const open = source.indexOf('{', start);
  let depth = 1, quote = null, escaped = false, line = false, block = false, end = open + 1;
  for (; end < source.length && depth; end++) {
    const c = source[end], n = source[end + 1];
    if (line) { if (c === '\n') line = false; continue; }
    if (block) { if (c === '*' && n === '/') { block = false; end++; } continue; }
    if (quote) { if (escaped) escaped = false; else if (c === '\\') escaped = true; else if (c === quote) quote = null; continue; }
    if (c === '/' && n === '/') { line = true; end++; continue; }
    if (c === '/' && n === '*') { block = true; end++; continue; }
    if (`"'\``.includes(c)) { quote = c; continue; }
    if (c === '{') depth++;
    else if (c === '}' && --depth === 0) break;
  }
  assert.equal(depth, 0, `${name} did not close`);
  return stripTypeScriptTypes(`${source.slice(start, open)}{${source.slice(open + 1, end - 1)}}`);
}
const sourceRows = readFileSync('pokerogue-tests/fixtures/imported-demo-data.csv', 'utf8')
  .split('\n').filter(line => line.startsWith('level_move,SILCOON,') || line.startsWith('level_move,WURMPLE,'));
const forms = {};
for (const species of ['SILCOON', 'WURMPLE']) {
  const moves = sourceRows.filter(line => line.startsWith(`level_move,${species},`))
    .map(line => { const [, , , level, move] = line.split(','); return [Number(level), move]; });
  forms[species] = { speciesId: species, getLevelMoves: () => moves };
}
const funcs = ['getBaseLearnableMoveSource', 'getUniqueMoves', 'getRegularLevelMoves', 'getPrevolutionMoves', 'filterAndSortLevelMoves', 'getLevelMoves']
  .map(extractFunction).join('\n');
const execute = new Function('context', 'forms', 'LearnableMoveSource', 'LearnMoveSituation', 'EVOLVE_MOVE', 'RELEARN_MOVE', `${funcs}; const speciesDataRegistry={getPrevolutionChain:()=>['WURMPLE'],getPokemonSpeciesForm:id=>forms[id]}; return getLevelMoves(context,false,true,false,LearnMoveSituation.MISC);`);
const scenarios = [[1, 1], [5, 5], [15, 15]];
const rows = scenarios.map(([startingLevel, currentLevel]) => {
  const run = execute(
    { pokemonSpeciesForm: forms.SILCOON, pokemonFormKey: undefined, level: currentLevel, startingLevel, includeDuplicates: false },
    forms,
    { LEVEL: 0, RELEARN: 2, EVOLUTION: 4, PREVO: 6 },
    { MISC: 0, EVOLUTION_FUSED: 4, EVOLUTION_FUSED_BASE: 5 },
    0,
    -1,
  );
  const result = run.map(([level, move, source]) => `${level}:${move}:${source}`).join('|');
  const expected = currentLevel === 1
    ? '1:TACKLE:6|1:STRING_SHOT:6'
    : currentLevel === 5 ? '5:POISON_STING:6' : '15:BUG_BITE:6';
  assert.equal(result, expected);
  return `SILCOON,${startingLevel},${currentLevel},${result}`;
});
const output = `# source=${pin}; executes original getLevelMoves/getPrevolutionMoves/filterAndSortLevelMoves; registry chain is pinned Silcoon→Wurmple\n# species,starting_level,current_level,level_move_candidates\n${rows.join('\n')}\n`;
const file = resolve('pokerogue-tests/fixtures/level-move-filter.csv');
if (process.argv.includes('--check')) assert.equal(readFileSync(file, 'utf8'), output);
else writeFileSync(file, output);

const edgeInput = [
  [15, 2, 6], [16, 2, 0], // own future move locks the prevo candidate
  [15, 3, 6], [10, 3, 0], // already-own move suppresses prevo source
  [15, 4, 6], [15, 4, 6], // post-filter duplicate
  [14, 5, 6], [15, 1, 0], [15, 1, 6], [16, 6, 6],
  [14, 7, 6], [15, 7, 6], // below-range duplicate is removed before uniquing
];
const filterSource = `${extractFunction('getBaseLearnableMoveSource')}\n${extractFunction('getUniqueMoves')}\n${extractFunction('filterAndSortLevelMoves')}`;
const filtered = new Function('context', 'candidates', 'LearnableMoveSource', `${filterSource}; return filterAndSortLevelMoves(context,candidates);`)(
  { level: 15, startingLevel: 15, includeDuplicates: false }, edgeInput,
  { LEVEL: 0, RELEARN: 2, EVOLUTION: 4, PREVO: 6 },
);
const edgeResult = filtered.map(([level, id, source]) => `${level}:${id}:${source}`).join('|');
assert.equal(edgeResult, '15:1:0|15:4:6|15:7:6');
const edgeOutput = `# source=${pin}; executes original filterAndSortLevelMoves over controlled eligibility and duplicate boundaries\n# start,current,candidates,result\n15,15,${edgeInput.map(([l, id, s]) => `${l}:${id}:${s}`).join('|')},${edgeResult}\n`;
const edgeFile = resolve('pokerogue-tests/fixtures/level-move-filter-edges.csv');
if (process.argv.includes('--check')) assert.equal(readFileSync(edgeFile, 'utf8'), edgeOutput);
else writeFileSync(edgeFile, edgeOutput);
console.log('Level-move source filter fixtures current.');
