#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { writeOrCheckTrace } from "../tools/visual/replay-trace.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const check = process.argv.includes("--check");
const throughCommand = process.argv.includes("--through-command");
const partySize = Number(process.env.POKERUST_REPLAY_PARTY_SIZE ?? 1);
const starterIds = (process.env.POKERUST_REPLAY_STARTER_IDS ?? (partySize === 1 ? "1" : "1,4")).split(",").map(value => Number(value.trim()));
const expectedTimeOfDay = process.env.POKERUST_REPLAY_EXPECT_TIME_OF_DAY ?? "DAY";
const timeOfDayIds = { DAWN: 0, DAY: 1, DUSK: 2, NIGHT: 3 };
const sourceTimeOfDayId = timeOfDayIds[expectedTimeOfDay];
assert.ok(sourceTimeOfDayId !== undefined || expectedTimeOfDay === "AUTO", `unsupported expected time of day ${expectedTimeOfDay}`);
const destination = path.resolve(process.argv.find((arg, index) => index > 1 && !arg.startsWith("--"))
	?? path.join(root, "pokerogue-tests/fixtures/town-wave1-entry-trace.json"));
const temp = fs.mkdtempSync(path.join(os.tmpdir(), "pokerust-town-entry-"));
const capture = path.join(root, "tools/visual/capture-original-game.mjs");
const port = 8141;
let succeeded = false;
try {
	const result = spawnSync(process.execPath, [capture, temp], {
		cwd: root,
		env: { ...process.env, POKERUST_REPLAY_TRACE: "1", POKERUST_CAPTURE_STARTER_INPUT: "1", POKERUST_REPLAY_THROUGH_COMMAND: throughCommand ? "1" : "0", POKERUST_REPLAY_PARTY_SIZE: String(partySize), POKERUST_REPLAY_PORT: String(port) },
		encoding: "utf8",
		timeout: Number(process.env.POKERUST_REPLAY_CAPTURE_TIMEOUT_MS ?? 240_000),
	});
	if (result.error) throw result.error;
	if (result.status !== 0) throw new Error(result.stderr || result.stdout || `capture exited ${result.status}`);
	const tracePath = path.join(temp, "replay-trace.json");
	assert.ok(fs.existsSync(tracePath), "original replay did not produce replay-trace.json");
	const trace = JSON.parse(fs.readFileSync(tracePath, "utf8"));
	assert.equal(trace.provenance.source_revision, "7c0881255af5cd51992e513ff93c2f71adbe40e0");
	assert.equal(trace.provenance.asset_revision, "fb8ccb6d0703ffe48cdf80a8102d5ed943c82dba");
	assert.equal(trace.provenance.phaser_version, "4.2.1");
	assert.equal(trace.scenario.seed, process.env.POKERUST_REPLAY_SEED ?? "M12TownWave1Replay6");
	const entry = trace.observations.find(event => event.kind === "newBattle.begin");
	const exit = trace.observations.find(event => event.kind === "newBattle.end");
	assert.ok(entry, "missing newBattle entry boundary");
	assert.ok(Array.isArray(entry.before.starter_input), "missing original starter selection input");
	assert.ok(exit, "missing newBattle return boundary");
	assert.equal(entry.before.biome, 0, "source boundary is not Town");
	const observedTimeOfDay = Object.entries(timeOfDayIds).find(([, id]) => id === entry.before.time_of_day)?.[0];
	assert.ok(observedTimeOfDay, `source boundary has unknown time of day ${entry.before.time_of_day}`);
	assert.equal(trace.scenario.time_of_day, observedTimeOfDay, "trace scenario must report the observed original time of day");
	if (expectedTimeOfDay !== "AUTO") assert.equal(observedTimeOfDay, expectedTimeOfDay, `source boundary is not ${expectedTimeOfDay}`);
	assert.equal(exit.battle.wave_index, 1, "source boundary is not wave 1");
	assert.equal(entry.before.starter_input.length, partySize, `expected ${partySize} starters selected through the original UI`);
	assert.deepEqual(entry.before.starter_input.map(starter => starter.speciesId), starterIds, "starter input differs from the requested original UI selections");
	assert.equal(new Set(entry.before.starter_input.map(starter => starter.speciesId)).size, partySize, "starter input must contain distinct species");
	assert.deepEqual(exit.queued_phases, ["EncounterPhase"]);
	if (throughCommand) {
		assert.equal(trace.command_boundary_state?.phase, "CommandPhase", "source did not reach the first player command");
		assert.ok(trace.command_boundary_state?.enemy, "first player command lacks generated opponent state");
		assert.ok(trace.observations.some(event => event.kind === "phase.start" && event.phase === "EncounterPhase"));
		assert.ok(trace.observations.some(event => event.kind === "phase.end" && event.phase === "EncounterPhase"));
		const boundary = trace.observations.find(event => event.kind === "phase.command-boundary");
		assert.ok(boundary?.player_party?.length && boundary?.enemy_party?.length, "missing full party state at command boundary");
		assert.equal(boundary.player_party.length, partySize, "command boundary party size differs from original UI selection");
		if (partySize > 1) {
			const sourceInputs = trace.observations.filter(event => event.kind === "harness.input" && event.phase === "CheckSwitchPhase");
			assert.deepEqual(sourceInputs.map(event => [event.command, event.button, event.handler, event.accepted]), [
				["select-no-initial-switch", 1, "ConfirmUiHandler", true],
				["confirm-no-initial-switch", 5, "ConfirmUiHandler", true],
			], "initial reserve check must be answered through the original ConfirmUiHandler");
		}
		for (const pokemon of [...boundary.player_party, ...boundary.enemy_party]) {
			for (const field of ["species_id", "level", "ivs", "nature", "ability_id", "stats", "hp", "max_hp", "moves"])
				assert.ok(pokemon[field] !== undefined, `missing ${field} at command boundary`);
			for (const move of pokemon.moves)
				for (const field of ["id", "pp", "pp_used", "max_pp"])
					assert.ok(move[field] !== undefined, `missing move ${field} at command boundary`);
	}
		if (process.env.POKERUST_REPLAY_ACTION) {
			assert.equal(trace.scenario.action, process.env.POKERUST_REPLAY_ACTION);
			const moveSequence = process.env.POKERUST_REPLAY_ACTION === "first-move"
				? [0]
				: (process.env.POKERUST_REPLAY_MOVE_SEQUENCE ?? "").split(",").map(value => Number(value.trim()));
			const moveInputs = trace.observations.filter(event => event.kind === "harness.input" && (event.command === "first-move" || event.command.startsWith("move-sequence-")));
			const stopAfterReplacement = process.env.POKERUST_REPLAY_STOP_AFTER_REPLACEMENT === "1";
			if (stopAfterReplacement) assert.ok(moveInputs.length > 0 && moveInputs.length <= moveSequence.length, "expected a non-empty prefix of original FightUiHandler inputs before replacement");
			else assert.equal(moveInputs.length, moveSequence.length, "missing original FightUiHandler move inputs");
			assert.deepEqual(moveInputs.map(event => event.move_index), moveSequence.slice(0, moveInputs.length), "source move selection differs from the requested UI sequence");
			assert.ok(moveInputs.every(event => event.handler === "FightUiHandler" && event.accepted), "move selection must be accepted by the original FightUiHandler");
			const boundaries = trace.observations.filter(event => event.kind === "phase.command-boundary");
			assert.equal(boundaries.length, moveInputs.length + 1, "action trace must capture the initial and each returned CommandPhase boundary");
			if (stopAfterReplacement) {
				const starts = trace.observations.filter(event => event.kind === "phase.start");
				assert.ok(starts.some(event => event.phase === "FaintPhase" && event.fainted_is_player === true && event.fainted_hp === 0), "missing player faint phase");
				assert.ok(starts.some(event => event.phase === "MovePhase" && event.actor_is_player === true && event.actor_is_active === false), "missing canceled inactive-player MovePhase evidence");
				assert.ok(starts.some(event => event.phase === "SwitchSummonPhase" && event.slot_index === 1), "missing reserve SwitchSummonPhase");
				assert.ok(trace.observations.some(event => event.kind === "harness.input" && event.command === "confirm-replacement" && event.accepted), "missing accepted original replacement confirmation");
				assert.ok(starts.some(event => event.phase === "TurnEndPhase") && starts.some(event => event.phase === "TurnInitPhase"), "replacement did not reach turn end and next turn initialization");
				assert.equal(trace.command_boundary_state?.player?.species_id, starterIds[1], "next command did not return with the selected reserve active");
			} else assert.equal(exit.battle.double, false, "R5 first-move fixture requires a single wild opponent");
		}
	} else {
		assert.equal(trace.observations.at(-1)?.kind, "newBattle.end", "R1 trace must stop exactly at the newBattle return boundary");
	}
	writeOrCheckTrace(destination, trace, check);
	process.stdout.write(check ? `Town entry trace current: ${destination}\n` : `Captured Town entry trace: ${destination}\n`);
	succeeded = true;
} finally {
	if (succeeded) fs.rmSync(temp, { recursive: true, force: true });
	else process.stderr.write(`Replay artifacts retained for diagnosis: ${temp}\n`);
}
