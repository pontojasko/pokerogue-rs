// Canonicalization helpers for source-executed replay traces. Volatile browser
// fields are deliberately excluded; gameplay observations remain untouched.
import assert from "node:assert/strict";
import fs from "node:fs";

export const TRACE_SCHEMA = "pokerust-source-replay/v1";

export function canonicalTrace(trace) {
	assert.equal(trace.schema, TRACE_SCHEMA, "unsupported trace schema");
	assert.ok(trace.provenance?.source_revision, "missing source revision");
	assert.ok(trace.provenance?.asset_revision, "missing asset revision");
	assert.ok(trace.scenario?.seed, "missing deterministic scenario seed");
	const stable = structuredClone(trace);
	// Phaser's simulation clock at CommandPhase depends on browser frame timing;
	// keep it in raw diagnostics but exclude it from deterministic trace identity.
	const boundary = stable.command_boundary_state;
	if (boundary?.clock) delete boundary.clock.now;
	// The active timer list is capture-driver diagnostics, not battle state; its
	// size/callbacks/timing vary with browser frame scheduling at the UI boundary.
	if (boundary) delete boundary.active_clock_events;
	return `${JSON.stringify(stable, null, 2)}\n`;
}

export function firstDivergence(expected, actual, path = "trace") {
	if (Object.is(expected, actual)) return null;
	if (expected === null || actual === null || typeof expected !== "object" || typeof actual !== "object") {
		return { path, expected, actual };
	}
	const keys = [...new Set([...Object.keys(expected), ...Object.keys(actual)])].sort();
	for (const key of keys) {
		if (!(key in expected) || !(key in actual)) {
			return { path: `${path}.${key}`, expected: expected[key], actual: actual[key] };
		}
		const difference = firstDivergence(expected[key], actual[key], `${path}.${key}`);
		if (difference) return difference;
	}
	return null;
}

export function writeOrCheckTrace(filename, trace, check = false) {
	const output = canonicalTrace(trace);
	if (check) {
		assert.ok(fs.existsSync(filename), `trace fixture missing: ${filename}`);
	const recorded = fs.readFileSync(filename, "utf8");
		assert.equal(recorded, output, `trace fixture stale: ${filename}`);
		return;
	}
	fs.writeFileSync(filename, output);
}
