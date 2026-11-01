#!/usr/bin/env node
// Extract the local player-party size bound from the pinned source.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const root = resolve(import.meta.dirname, '..');
const pin = '7c0881255af5cd51992e513ff93c2f71adbe40e0';
const source = readFileSync(resolve(root, 'original/src/constants.ts'), 'utf8');
const match = source.match(/export const PLAYER_PARTY_MAX_SIZE:\s*number\s*=\s*(\d+)\s*;/);
assert.ok(match, 'PLAYER_PARTY_MAX_SIZE declaration missing');
const output = `# extracted from original/src/constants.ts at ${pin}\nparty_capacity,${pin},${match[1]}\n`;
const path = resolve(root, 'pokerogue-tests/fixtures/party-capacity.csv');
if (process.argv.includes('--check')) {
  assert.equal(readFileSync(path, 'utf8'), output, 'party capacity fixture stale');
  console.log('Party capacity fixture is current.');
} else {
  writeFileSync(path, output);
  console.log('Wrote party capacity source fixture.');
}
