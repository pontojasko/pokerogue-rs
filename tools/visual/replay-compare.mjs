import { firstDivergence } from "./replay-trace.mjs";

export function compareTownEntry(source, rust) {
	if (source?.schema !== "pokerust-source-replay/v1" ||
		source?.provenance?.source_revision !== "7c0881255af5cd51992e513ff93c2f71adbe40e0" ||
		source?.provenance?.asset_revision !== "fb8ccb6d0703ffe48cdf80a8102d5ed943c82dba" ||
		source?.provenance?.phaser_version !== "4.2.1") {
		return { status: "provenance-mismatch", parity_pass: false,
			first_divergence: { path: "source.provenance", expected: "pinned source/assets and Phaser", actual: source?.provenance ?? source?.schema },
			missing_observations: [] };
	}
	if (rust?.schema !== "pokerust-rust-entry/v1") {
		return { status: "schema-mismatch", parity_pass: false,
			first_divergence: { path: "rust.schema", expected: "pokerust-rust-entry/v1", actual: rust?.schema }, missing_observations: [] };
	}
	const missing = [];
	const sourceBattle = source?.observations?.find(event => event.kind === "newBattle.end");
	const sourceStart = source?.observations?.findIndex(event => event.kind === "phase.start" && event.phase === "EncounterPhase") ?? -1;
	const commandBoundary = source?.observations?.find(event => event.kind === "phase.command-boundary");
	const sourceEnemy = commandBoundary?.enemy_party?.[0];
	if (!sourceBattle) missing.push("source newBattle.end boundary");
	if (!commandBoundary || !sourceEnemy) missing.push("source first opponent construction");
	if (sourceStart < 0 || !commandBoundary) missing.push("source EncounterPhase RNG interval");
	const actual = rust?.observed;
	for (const field of ["battle_seed", "battle_seed_rng_draws", "wave_index", "species_id", "tier_roll", "level", "ability_id", "encounter_rng_draws", "is_mystery_encounter", "phase_manager_trace", "initial_phase_queue"])
		if (actual?.[field] === undefined) missing.push(`Rust ${field}`);
	if (rust?.missing?.length) missing.push(...rust.missing.map(field => `Rust ${field}`));

	const comparisons = [
		["battle_seed", sourceBattle?.battle?.seed, actual?.battle_seed],
		["wave_index", sourceBattle?.battle?.wave_index, actual?.wave_index],
		["level", sourceEnemy?.level, actual?.level],
	];
	for (const [path, expected, value] of comparisons) {
		if (expected === undefined || value === undefined) continue;
		const divergence = firstDivergence(expected, value, `entry.${path}`);
		if (divergence) return { status: "diagnostic-mismatch", parity_pass: false, first_divergence: divergence, missing_observations: missing };
	}
	const seedOffsetStart = source.observations.findIndex(event => event.kind === "seed-offset.begin" && event.offset === 8);
	const seedOffsetEnd = source.observations.findIndex((event, index) => index > seedOffsetStart && event.kind === "seed-offset.end" && event.offset === 8);
	const sourceBattleSeedDraws = seedOffsetStart >= 0 && seedOffsetEnd > seedOffsetStart
		? source.observations.slice(seedOffsetStart + 1, seedOffsetEnd)
			.filter(event => event.kind === "rng" && event.method === "integerInRange")
			.map(event => ({ range: event.args[1] - event.args[0] + 1, min: event.args[0], value: event.value }))
		: [];
	const seedTraceDivergence = firstDivergence(sourceBattleSeedDraws, actual?.battle_seed_rng_draws, "entry.battle_seed_rng_draws");
	if (seedTraceDivergence) return { status: "diagnostic-mismatch", parity_pass: false, first_divergence: seedTraceDivergence, missing_observations: missing };
	const mysteryDivergence = firstDivergence(sourceBattle?.battle?.battle_type === 3, actual?.is_mystery_encounter, "entry.is_mystery_encounter");
	if (mysteryDivergence) return { status: "diagnostic-mismatch", parity_pass: false, first_divergence: mysteryDivergence, missing_observations: missing };
	const sourceBattleIndex = source.observations.findIndex(event => event.kind === "newBattle.end");
	const offsetStart = source.observations.findIndex((event, index) => index < sourceBattleIndex && event.kind === "seed-offset.begin" && event.offset === 8);
	const sourceDoubleDraw = source.observations
		.slice(1, offsetStart < 0 ? sourceBattleIndex : offsetStart)
		.find(event => event.kind === "rng" && event.method === "integerInRange");
	if (!sourceDoubleDraw) missing.push("source checkIsDouble RNG draw");
	if (actual?.double_check_rng_draws?.length !== 1) missing.push("Rust double-check RNG draw");
	else if (sourceDoubleDraw) {
		const expected = { range: sourceDoubleDraw.args[1] - sourceDoubleDraw.args[0] + 1, min: sourceDoubleDraw.args[0], value: sourceDoubleDraw.value };
		const draw = actual.double_check_rng_draws[0];
		const divergence = firstDivergence(expected, { range: draw.range, min: draw.min, value: draw.value }, "entry.double_check_rng[0]");
		if (divergence) return { status: "diagnostic-mismatch", parity_pass: false, first_divergence: divergence, missing_observations: missing };
	}
	const doubleDivergence = firstDivergence(sourceBattle?.battle?.double, actual?.double, "entry.double");
	if (doubleDivergence) return { status: "diagnostic-mismatch", parity_pass: false, first_divergence: doubleDivergence, missing_observations: missing };
	const sourceEnd = source.observations.findIndex((event, index) => index > sourceStart && event.kind === "phase.command-boundary");
	const sourceDraws = sourceStart >= 0 && sourceEnd >= 0
		? source.observations.slice(sourceStart + 1, sourceEnd).filter(event => event.kind === "rng" && event.method === "integerInRange")
			.map(event => ({ range: event.args[1] - event.args[0] + 1, min: event.args[0], value: event.value }))
		: [];
	if (!sourceDraws.length) missing.push("source integer RNG draws within EncounterPhase");
	const rustDraws = actual?.encounter_rng_draws ?? [];
	for (let i = 0; i < Math.max(sourceDraws.length, rustDraws.length); i++) {
		const expected = sourceDraws[i];
		const value = rustDraws[i];
		if (!expected || !value) {
			return { status: "diagnostic-mismatch", parity_pass: false,
				first_divergence: { path: `entry.rng[${i}]`, expected: expected ?? null, actual: value ?? null },
				missing_observations: missing };
		}
		const divergence = firstDivergence(expected, { range: value.range, min: value.min, value: value.value }, `entry.rng[${i}]`);
		if (divergence) return { status: "diagnostic-mismatch", parity_pass: false, first_divergence: divergence, missing_observations: missing };
	}
	const sourcePhaseTrace = source.observations
		.slice(0, source.observations.findIndex(event => event.kind === "phase.command-boundary"))
		.filter(event => event.kind === "phase.start")
		.map(event => event.phase);
	if (actual?.phase_manager_trace !== undefined) {
		const phaseTraceDivergence = firstDivergence(sourcePhaseTrace, actual.phase_manager_trace, "entry.phase_trace");
		if (phaseTraceDivergence) return { status: "diagnostic-mismatch", parity_pass: false, first_divergence: phaseTraceDivergence, missing_observations: missing };
	}
	if (actual?.initial_phase_queue !== undefined) {
		const queueDivergence = firstDivergence(commandBoundary?.queue, actual.initial_phase_queue, "entry.initial_phase_queue");
		if (queueDivergence) return { status: "diagnostic-mismatch", parity_pass: false, first_divergence: queueDivergence, missing_observations: missing };
	}
	const sourceParty = commandBoundary?.enemy_party ?? [];
	const actualParty = actual?.opponents ?? [];
	if (actual?.opponent_party_cardinality === undefined) missing.push("Rust opponent_party_cardinality");
	else if (sourceParty.length !== actual.opponent_party_cardinality) {
		return { status: "diagnostic-mismatch", parity_pass: false,
			first_divergence: { path: "entry.opponent_party_cardinality", expected: sourceParty.length, actual: actual.opponent_party_cardinality },
			missing_observations: missing };
	}
	for (let index = 0; index < Math.min(sourceParty.length, actualParty.length); index++) {
		for (const field of ["species_id", "level", "ivs", "nature", "ability_id", "hp", "max_hp", "stats"]) {
			const divergence = firstDivergence(sourceParty[index]?.[field], actualParty[index]?.[field], `entry.opponents[${index}].${field}`);
			if (divergence) return { status: "diagnostic-mismatch", parity_pass: false, first_divergence: divergence, missing_observations: missing };
		}
		const sourceMoves = sourceParty[index]?.moves?.map(move => move.id);
		const rustMoves = actualParty[index]?.moves?.map(move => move.id);
		const moveDivergence = firstDivergence(sourceMoves, rustMoves, `entry.opponents[${index}].moves`);
		if (moveDivergence) return { status: "diagnostic-mismatch", parity_pass: false, first_divergence: moveDivergence, missing_observations: missing };
	}
	const sourcePlayerParty = commandBoundary?.player_party ?? [];
	const actualPlayerParty = actual?.player_party ?? [];
	if (sourcePlayerParty.length !== actualPlayerParty.length) {
		return { status: "diagnostic-mismatch", parity_pass: false,
			first_divergence: { path: "entry.player_party_cardinality", expected: sourcePlayerParty.length, actual: actualPlayerParty.length },
			missing_observations: missing };
	}
	for (let index = 0; index < sourcePlayerParty.length; index++) {
		for (const field of ["species_id", "level", "hp", "max_hp", "stats"]) {
			const divergence = firstDivergence(sourcePlayerParty[index]?.[field], actualPlayerParty[index]?.[field], `entry.player_party[${index}].${field}`);
			if (divergence) return { status: "diagnostic-mismatch", parity_pass: false, first_divergence: divergence, missing_observations: missing };
		}
		const sourceMoves = sourcePlayerParty[index]?.moves?.map(move => move.id);
		const rustMoves = actualPlayerParty[index]?.moves?.map(move => move.id);
		const moveDivergence = firstDivergence(sourceMoves, rustMoves, `entry.player_party[${index}].moves`);
		if (moveDivergence) return { status: "diagnostic-mismatch", parity_pass: false, first_divergence: moveDivergence, missing_observations: missing };
	}
	const speciesDivergence = firstDivergence(sourceEnemy?.species_id, actual?.species_id, "entry.species_id");
	if (speciesDivergence) return { status: "diagnostic-mismatch", parity_pass: false, first_divergence: speciesDivergence, missing_observations: missing };
	for (const field of ["hp", "max_hp", "stats"]) {
		if (sourceEnemy?.[field] === undefined || actual?.[field] === undefined) {
			missing.push(`HP/stat field ${field}`);
			continue;
		}
		const divergence = firstDivergence(sourceEnemy[field], actual[field], `entry.${field}`);
		if (divergence) return { status: "diagnostic-mismatch", parity_pass: false, first_divergence: divergence, missing_observations: missing };
	}
	for (const field of ["ivs", "nature"]) {
		if (sourceEnemy?.[field] === undefined || actual?.[field] === undefined) {
			missing.push(`entry field ${field}`);
			continue;
		}
		const divergence = firstDivergence(sourceEnemy[field], actual[field], `entry.${field}`);
		if (divergence) return { status: "diagnostic-mismatch", parity_pass: false, first_divergence: divergence, missing_observations: missing };
	}
	const abilityDivergence = firstDivergence(sourceEnemy?.ability_id, actual?.ability_id, "entry.ability_id");
	if (abilityDivergence) return { status: "diagnostic-mismatch", parity_pass: false, first_divergence: abilityDivergence, missing_observations: missing };
	const sourceMoves = sourceEnemy?.moves?.map(move => move.id);
	const rustMoves = actual?.moves?.map(move => move.id);
	if (!sourceMoves || !rustMoves) missing.push("move IDs");
	else {
		const divergence = firstDivergence(sourceMoves, rustMoves, "entry.moves");
		if (divergence) return { status: "diagnostic-mismatch", parity_pass: false, first_divergence: divergence, missing_observations: missing };
	}
	return { status: missing.length ? "incomplete-observations" : "matched", parity_pass: missing.length === 0, first_divergence: null, missing_observations: missing };
}

export function compareTownAction(source, rust) {
	const provenance = source?.provenance;
	if (source?.schema !== "pokerust-source-replay/v1" ||
		provenance?.source_revision !== "7c0881255af5cd51992e513ff93c2f71adbe40e0" ||
		provenance?.asset_revision !== "fb8ccb6d0703ffe48cdf80a8102d5ed943c82dba" ||
		provenance?.phaser_version !== "4.2.1") {
		return { status: "provenance-mismatch", parity_pass: false,
			first_divergence: { path: "source.provenance", expected: "pinned source/assets and Phaser", actual: provenance ?? source?.schema }, missing_observations: [] };
	}
	if (rust?.schema !== "pokerust-rust-entry/v1") {
		return { status: "schema-mismatch", parity_pass: false,
			first_divergence: { path: "rust.schema", expected: "pokerust-rust-entry/v1", actual: rust?.schema }, missing_observations: [] };
	}
	const missing = [];
	if (source?.scenario?.action !== "first-move") missing.push("source first-move scenario");
	const observations = source?.observations ?? [];
	const boundaries = observations.filter(event => event.kind === "phase.command-boundary");
	if (boundaries.length !== 2) missing.push("initial and returned CommandPhase boundaries");
	const fightInput = observations.find(event => event.kind === "harness.input" && event.command === "Fight");
	const moveInput = observations.find(event => event.kind === "harness.input" && event.command === "first-move");
	if (!fightInput || fightInput.handler !== "CommandUiHandler") missing.push("original CommandUiHandler Fight input");
	if (!moveInput || moveInput.handler !== "FightUiHandler") missing.push("original FightUiHandler move input");
	const actionResult = rust?.observed?.action_result;
	if (!actionResult || actionResult.request !== "first-move") missing.push("Rust first-move Battle::take_turn result");
	if (missing.length) return { status: "incomplete-observations", parity_pass: false, first_divergence: null, missing_observations: missing };

	const moveInputIndex = observations.indexOf(moveInput);
	const returnedBoundaryIndex = observations.findIndex((event, index) => index > moveInputIndex && event.kind === "phase.command-boundary");
	if (returnedBoundaryIndex < 0) return { status: "incomplete-observations", parity_pass: false, first_divergence: null, missing_observations: ["returned CommandPhase boundary"] };
	const sourceMovePhases = observations.slice(moveInputIndex + 1, returnedBoundaryIndex)
		.filter(event => event.kind === "phase.start" && event.phase === "MovePhase");
	if (sourceMovePhases.length !== 2 || sourceMovePhases.some(phase => phase.actor_is_player === undefined || phase.move_id === undefined || !Array.isArray(phase.targets))) {
		return { status: "incomplete-observations", parity_pass: false, first_divergence: null, missing_observations: ["source MovePhase actor/move/target observations"] };
	}
	const sourceMoveActions = sourceMovePhases.map(phase => ({
		actor: phase.actor_is_player ? "player" : "enemy",
		move_id: phase.move_id,
		target: phase.targets.length === 1 ? ({ 0: "player", 2: "enemy" })[phase.targets[0]] ?? null : null,
	}));
	const sourceDraws = [];
	let seedOffsetDepth = 0;
	for (const event of observations.slice(moveInputIndex + 1, returnedBoundaryIndex)) {
		if (event.kind === "seed-offset.begin") { seedOffsetDepth++; continue; }
		if (event.kind === "seed-offset.end") { seedOffsetDepth--; continue; }
		if (seedOffsetDepth === 0 && event.kind === "rng" && event.method === "integerInRange") {
			sourceDraws.push({ range: event.args[1] - event.args[0] + 1, min: event.args[0], value: event.value });
		}
	}
	const rustDraws = actionResult.rng_draws?.map(({ range, min, value }) => ({ range, min, value }));
	const drawDivergence = firstDivergence(sourceDraws, rustDraws, "action.rng");
	if (drawDivergence) return { status: "diagnostic-mismatch", parity_pass: false, first_divergence: drawDivergence, missing_observations: missing };
	const rustMoveActions = actionResult.move_actions?.map(({ actor, move_id, target }) => ({ actor, move_id, target }));
	const moveDivergence = firstDivergence(sourceMoveActions, rustMoveActions, "action.moves");
	if (moveDivergence) return { status: "diagnostic-mismatch", parity_pass: false, first_divergence: moveDivergence, missing_observations: missing };
	const inputDivergence = firstDivergence(moveInput.move_id, actionResult.selected_move_id, "action.selected_move_id");
	if (inputDivergence) return { status: "diagnostic-mismatch", parity_pass: false, first_divergence: inputDivergence, missing_observations: missing };
	const sourceInitial = boundaries[0];
	const sourceFinal = boundaries[1];
	for (const [side, sourceParty, rustParty] of [
		["player", sourceInitial.player_party, rust?.observed?.player_party],
		["enemy", sourceInitial.enemy_party, rust?.observed?.opponents],
	]) {
		if (!Array.isArray(sourceParty) || !Array.isArray(rustParty) || sourceParty.length !== rustParty.length) {
			return { status: "diagnostic-mismatch", parity_pass: false,
				first_divergence: { path: `action.entry.${side}.party_length`, expected: sourceParty?.length ?? null, actual: rustParty?.length ?? null }, missing_observations: missing };
		}
		for (let index = 0; index < sourceParty.length; index++) {
			for (const field of ["species_id", "level", "hp", "max_hp", "stats"]) {
				const divergence = firstDivergence(sourceParty[index]?.[field], rustParty[index]?.[field], `action.entry.${side}[${index}].${field}`);
				if (divergence) return { status: "diagnostic-mismatch", parity_pass: false, first_divergence: divergence, missing_observations: missing };
			}
			const sourceMoves = sourceParty[index]?.moves;
			const rustMoves = rustParty[index]?.moves;
			if (!Array.isArray(sourceMoves) || !Array.isArray(rustMoves)) {
				return { status: "incomplete-observations", parity_pass: false, first_divergence: null,
					missing_observations: [`initial ${side} move PP state`] };
			}
			for (let moveIndex = 0; moveIndex < sourceMoves.length; moveIndex++) {
				for (const field of ["id", "pp", "pp_used", "max_pp"]) {
					const divergence = firstDivergence(sourceMoves[moveIndex]?.[field], rustMoves[moveIndex]?.[field], `action.entry.${side}[${index}].moves.${moveIndex}.${field}`);
					if (divergence) return { status: "diagnostic-mismatch", parity_pass: false, first_divergence: divergence, missing_observations: missing };
				}
			}
		}
	}
	const sourcePlayer = sourceFinal.player_party?.[0];
	const sourceEnemy = sourceFinal.enemy_party?.[0];
	for (const [path, expected, actual] of [
		["turn", 2, actionResult.turn],
		["player_hp", sourcePlayer?.hp, actionResult.player_hp],
		["player_max_hp", sourcePlayer?.max_hp, actionResult.player_max_hp],
		["enemy_hp", sourceEnemy?.hp, actionResult.enemy_hp],
		["enemy_max_hp", sourceEnemy?.max_hp, actionResult.enemy_max_hp],
		["player_move_pp_used", sourcePlayer?.moves?.find(move => move.id === moveInput.move_id)?.pp_used, actionResult.player_move_pp_used],
		["player_move_pp_remaining", sourcePlayer?.moves?.find(move => move.id === moveInput.move_id)?.pp, actionResult.player_move_pp_remaining],
		["enemy_move_pp_used", sourceEnemy?.moves?.find(move => move.id === actionResult.enemy_move_id)?.pp_used, actionResult.enemy_move_pp_used],
		["enemy_move_pp_remaining", sourceEnemy?.moves?.find(move => move.id === actionResult.enemy_move_id)?.pp, actionResult.enemy_move_pp_remaining],
	]) {
		const divergence = firstDivergence(expected, actual, `action.${path}`);
		if (divergence) return { status: "diagnostic-mismatch", parity_pass: false, first_divergence: divergence, missing_observations: missing };
	}
	const phaseNames = observations.slice(moveInputIndex + 1, returnedBoundaryIndex)
		.filter(event => event.kind === "phase.start")
		.map(event => event.phase);
	for (const required of ["EnemyCommandPhase", "TurnStartPhase", "CheckInterludePhase", "TurnEndPhase", "TurnInitPhase", "CommandPhase"])
		if (!phaseNames.includes(required)) missing.push(`source action phase ${required}`);
	if (sourceInitial.player_party?.length !== 1 || sourceInitial.enemy_party?.length !== 1) missing.push("single-target source boundary mapping");
	if (sourceMoveActions.some(action => action.target === null)) missing.push("source action target mapping to supported single-battler Rust target");
	if (missing.length) return { status: "incomplete-observations", parity_pass: false, first_divergence: null, missing_observations: missing };
	return {
		status: "matched",
		parity_pass: true,
		entry_precondition: "matched semantic party/enemy IDs, stats, HP, moves and PP",
		first_divergence: null,
		missing_observations: [],
		source_phase_mapping: [
			"EnemyCommandPhase/getNextTargets → Battle::select_enemy_move and single-target selection RNG",
			"TurnStartPhase/MovePhase/MoveEffectPhase → Battle::take_turn move order, accuracy, critical check, damage, PP and HP events",
			"TurnEndPhase/TurnInitPhase/CommandPhase → Rust turn completion and next-command semantic boundary",
		],
	};
}
