import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { firstDivergence, canonicalTrace, TRACE_SCHEMA, writeOrCheckTrace } from "./replay-trace.mjs";

const base = {
	schema: TRACE_SCHEMA,
	provenance: { source_revision: "pinned", asset_revision: "assets" },
	scenario: { seed: "TownWave1" },
	observations: [{ kind: "rng", value: 4 }, { kind: "phase", name: "EncounterPhase" }],
};
assert.equal(canonicalTrace(base), canonicalTrace(JSON.parse(canonicalTrace(base))));
const timed = (now) => ({ ...base, command_boundary_state: { phase: "CommandPhase", clock: { now } } });
assert.equal(canonicalTrace(timed(1234)), canonicalTrace(timed(5678)));
assert.notEqual(canonicalTrace(timed(1234)), canonicalTrace({ ...timed(1234), command_boundary_state: { phase: "OtherPhase", clock: { now: 5678 } } }));
const timerState = (elapsed, delay = 7) => ({ ...timed(1234), command_boundary_state: { phase: "CommandPhase", clock: { now: 1234 }, active_clock_events: [{ delay, elapsed, paused: false, repeat_count: 0 }] } });
assert.equal(canonicalTrace(timerState(52.32)), canonicalTrace(timerState(118.3466666664624)));
assert.equal(canonicalTrace(timerState(52.32)), canonicalTrace({ ...timerState(118.3466666664624, 8), command_boundary_state: { phase: "CommandPhase", clock: { now: 5678 }, active_clock_events: [{ delay: 8, elapsed: 1, paused: true, repeat_count: 1 }, { delay: 9 }] } }));
assert.deepEqual(firstDivergence(base, structuredClone(base)), null);
assert.deepEqual(firstDivergence(base, { ...base, observations: [{ kind: "rng", value: 5 }] }), {
	path: "trace.observations.0.value", expected: 4, actual: 5,
});
assert.throws(() => canonicalTrace({ ...base, scenario: {} }), /seed/);
const temp = fs.mkdtempSync(path.join(os.tmpdir(), "pokerust-replay-test-"));
const fixture = path.join(temp, "trace.json");
writeOrCheckTrace(fixture, base);
writeOrCheckTrace(fixture, base, true);
fs.writeFileSync(fixture, "stale\n");
assert.throws(() => writeOrCheckTrace(fixture, base, true), /stale/);
fs.rmSync(temp, { recursive: true, force: true });
