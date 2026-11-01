import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';

const source = readFileSync('original/src/phase-tree.ts', 'utf8');
const marker = 'export class PhaseTree';
const start = source.indexOf(marker);
assert.ok(start >= 0, 'pinned PhaseTree missing');
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
assert.equal(depth, 0, 'PhaseTree class did not close');
const classText = stripTypeScriptTypes(source.slice(start, end + 1).replace('export class', 'class'));
export const SourcePhaseTree = new Function(`${classText}; return PhaseTree;`)();

export function extractSourceBody(text, marker) {
  const start = text.indexOf(marker);
  assert.ok(start >= 0, `source method missing: ${marker}`);
  const open = text.indexOf('{', start);
  let depth = 1, quote = null, escaped = false, line = false, block = false, end = open + 1;
  for (; end < text.length && depth; end++) {
    const c = text[end], n = text[end + 1];
    if (line) { if (c === '\n') line = false; continue; }
    if (block) { if (c === '*' && n === '/') { block = false; end++; } continue; }
    if (quote) { if (escaped) escaped = false; else if (c === '\\') escaped = true; else if (c === quote) quote = null; continue; }
    if (c === '/' && n === '/') { line = true; end++; continue; }
    if (c === '/' && n === '*') { block = true; end++; continue; }
    if (`"'\``.includes(c)) { quote = c; continue; }
    if (c === '{') depth++;
    else if (c === '}' && --depth === 0) break;
  }
  assert.equal(depth, 0, `method did not close: ${marker}`);
  return text.slice(open + 1, end - 1);
}

const managerSource = readFileSync('original/src/phase-manager.ts', 'utf8');
const unshiftBody = extractSourceBody(managerSource, 'public unshiftPhase(');
const pushBody = extractSourceBody(managerSource, 'public pushPhase(');
const messageBody = extractSourceBody(managerSource, 'queueMessage(');
const unshiftNewBody = extractSourceBody(managerSource, 'public unshiftNew<');
const pushNewBody = extractSourceBody(managerSource, 'public pushNew<');
export function sourcePhaseManagerMethods(MessagePhase) {
  return {
    pushPhase: new Function(`return function(...phases){${pushBody}}`)(),
    unshiftPhase: new Function(`return function(...phases){${unshiftBody}}`)(),
    pushNew: new Function(`return function(phase,...args){${pushNewBody}}`)(),
    queueMessage: new Function('MessagePhase', `return function(message,callbackDelay,prompt,promptDelay,defer){${messageBody}}`)(MessagePhase),
    unshiftNew: new Function(`return function(phase,...args){${unshiftNewBody}}`)(),
  };
}
