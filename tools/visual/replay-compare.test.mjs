import assert from "node:assert/strict";
import { compareTownAction, compareTownEntry } from "./replay-compare.mjs";

const source = {
	schema: "pokerust-source-replay/v1",
	provenance: { source_revision: "7c0881255af5cd51992e513ff93c2f71adbe40e0", asset_revision: "fb8ccb6d0703ffe48cdf80a8102d5ed943c82dba", phaser_version: "4.2.1" },
	observations: [
		{ kind: "newBattle.begin" },
		{ kind: "rng", method: "integerInRange", args: [0, 7], value: 0 },
		{ kind: "seed-offset.begin", offset: 8 },
		{ kind: "rng", method: "integerInRange", args: [0, 1], value: 0 },
		{ kind: "rng", method: "integerInRange", args: [0, 1], value: 1 },
		{ kind: "seed-offset.end", offset: 8 },
		{ kind: "newBattle.end", battle: { seed: "same", wave_index: 1, double: true, battle_type: 0 } },
		{ kind: "phase.start", phase: "EncounterPhase" },
		{ kind: "rng", method: "integerInRange", args: [0, 511], value: 39 },
		{ kind: "rng", method: "integerInRange", args: [0, 13], value: 7 },
		{ kind: "phase.command-boundary", enemy_party: [{ species_id: 10, level: 2, ivs: [1, 2], nature: 3, ability_id: 7, hp: 10, max_hp: 10, stats: [10, 2], moves: [{ id: 33 }] }], player_party: [], queue: [] },
	],
};
const rust = { schema: "pokerust-rust-entry/v1", observed: {
	battle_seed: "same", battle_seed_rng_draws: [{ range: 2, min: 0, value: 0 }, { range: 2, min: 0, value: 1 }], wave_index: 1, double: true, double_check_rng_draws: [{ range: 8, min: 0, value: 0 }], species_id: 10, tier_roll: 39, level: 2, ivs: [1, 2], nature: 3, ability_id: 7, hp: 10, max_hp: 10, stats: [10, 2], moves: [{ id: 33 }], opponent_party_cardinality: 1, opponents: [{ species_id: 10, level: 2, ivs: [1, 2], nature: 3, ability_id: 7, hp: 10, max_hp: 10, stats: [10, 2], moves: [{ id: 33 }] }], player_party: [], is_mystery_encounter: false, phase_manager_trace: ["EncounterPhase"], initial_phase_queue: [],
	encounter_rng_draws: [{ range: 512, min: 0, value: 39 }, { range: 14, min: 0, value: 7 }],
}, missing: [] };
assert.equal(compareTownEntry(source, rust).parity_pass, true);
const unobservedPhases = structuredClone(rust);
delete unobservedPhases.observed.phase_manager_trace;
delete unobservedPhases.observed.initial_phase_queue;
unobservedPhases.missing.push("phase-manager execution");
const incompletePhases = compareTownEntry(source, unobservedPhases);
assert.equal(incompletePhases.status, "incomplete-observations");
assert.equal(incompletePhases.parity_pass, false);
assert.equal(incompletePhases.first_divergence, null);
assert.ok(incompletePhases.missing_observations.includes("Rust phase_manager_trace"));
const changed = structuredClone(rust);
changed.observed.encounter_rng_draws[0].value = 40;
assert.deepEqual(compareTownEntry(source, changed).first_divergence, {
	path: "entry.rng[0].value", expected: 39, actual: 40,
});
const reordered = structuredClone(rust);
[reordered.observed.encounter_rng_draws[0], reordered.observed.encounter_rng_draws[1]] =
	[reordered.observed.encounter_rng_draws[1], reordered.observed.encounter_rng_draws[0]];
assert.equal(compareTownEntry(source, reordered).first_divergence.path, "entry.rng[0].range");
const missingDraw = structuredClone(rust);
missingDraw.observed.encounter_rng_draws.pop();
assert.equal(compareTownEntry(source, missingDraw).first_divergence.path, "entry.rng[1]");
const changedHp = structuredClone(rust);
changedHp.observed.hp = 9;
assert.deepEqual(compareTownEntry(source, changedHp).first_divergence, {
	path: "entry.hp", expected: 10, actual: 9,
});
const omitted = structuredClone(rust);
omitted.missing = ["ivs"];
assert.equal(compareTownEntry(source, omitted).parity_pass, false);
assert.equal(compareTownEntry({ ...source, schema: "wrong" }, rust).status, "provenance-mismatch");
assert.equal(compareTownEntry(source, { ...rust, schema: "wrong" }).status, "schema-mismatch");

const actionDraws = [
	{ range: 8, min: 0, value: 5 },
	{ range: 100, min: 0, value: 9 },
	{ range: 24, min: 0, value: 12 },
	{ range: 16, min: 85, value: 90 },
	{ range: 100, min: 0, value: 24 },
	{ range: 24, min: 0, value: 9 },
	{ range: 16, min: 85, value: 88 },
];
const initialPlayer = { species_id: 1, level: 5, hp: 20, max_hp: 20, stats: [20, 10, 10, 12, 12, 10], moves: [{ id: 33, pp: 35, pp_used: 0, max_pp: 35 }] };
const initialEnemy = { species_id: 734, level: 2, hp: 14, max_hp: 14, stats: [14, 8, 6, 7, 5, 6], moves: [{ id: 33, pp: 35, pp_used: 0, max_pp: 35 }] };
const makeActionSource = () => ({
	schema: "pokerust-source-replay/v1",
	provenance: source.provenance,
	scenario: { action: "first-move" },
	observations: [
		{ kind: "harness.input", command: "Fight", handler: "CommandUiHandler" },
		{ kind: "harness.input", command: "first-move", handler: "FightUiHandler", move_id: 33 },
		{ kind: "rng", method: "integerInRange", args: [0, 7], value: 5 },
		{ kind: "seed-offset.begin", offset: 2 },
		{ kind: "rng", method: "integerInRange", args: [0, 1], value: 0 },
		{ kind: "seed-offset.end", offset: 2 },
		...actionDraws.slice(1).flatMap(draw => [{ kind: "rng", method: "integerInRange", args: [draw.min, draw.min + draw.range - 1], value: draw.value }]),
		{ kind: "phase.start", phase: "EnemyCommandPhase" },
		{ kind: "phase.start", phase: "TurnStartPhase" },
		{ kind: "phase.start", phase: "MovePhase", actor_is_player: true, move_id: 33, targets: [2] },
		{ kind: "phase.start", phase: "MovePhase", actor_is_player: false, move_id: 33, targets: [0] },
		{ kind: "phase.start", phase: "CheckInterludePhase" },
		{ kind: "phase.start", phase: "TurnEndPhase" },
		{ kind: "phase.start", phase: "TurnInitPhase" },
		{ kind: "phase.start", phase: "CommandPhase" },
		{ kind: "phase.command-boundary", player_party: [initialPlayer], enemy_party: [initialEnemy] },
		{ kind: "phase.command-boundary", player_party: [{ hp: 15, max_hp: 20, moves: [{ id: 33, pp: 34, pp_used: 1 }] }], enemy_party: [{ hp: 8, max_hp: 14, moves: [{ id: 33, pp: 34, pp_used: 1 }] }] },
	],
});
const actionRust = {
	schema: "pokerust-rust-entry/v1",
	observed: { player_party: [initialPlayer], opponents: [initialEnemy], action_result: {
		request: "first-move", selected_move_id: 33, enemy_move_id: 33, turn: 2,
		player_hp: 15, player_max_hp: 20, player_move_pp_used: 1, player_move_pp_remaining: 34,
		enemy_hp: 8, enemy_max_hp: 14, enemy_move_pp_used: 1, enemy_move_pp_remaining: 34,
		move_actions: [{ actor: "player", move_id: 33, target: "enemy" }, { actor: "enemy", move_id: 33, target: "player" }],
		rng_draws: actionDraws,
	} },
};
assert.equal(compareTownAction(makeActionSource(), actionRust).parity_pass, true);
const changedActionDraw = structuredClone(actionRust);
changedActionDraw.observed.action_result.rng_draws[2].value = 11;
assert.deepEqual(compareTownAction(makeActionSource(), changedActionDraw).first_divergence, {
	path: "action.rng.2.value", expected: 12, actual: 11,
});
const changedAction = structuredClone(actionRust);
changedAction.observed.action_result.selected_move_id = 45;
assert.deepEqual(compareTownAction(makeActionSource(), changedAction).first_divergence, {
	path: "action.selected_move_id", expected: 33, actual: 45,
});
const missingActionInput = makeActionSource();
missingActionInput.observations = missingActionInput.observations.filter(event => event.command !== "first-move");
assert.equal(compareTownAction(missingActionInput, actionRust).status, "incomplete-observations");
