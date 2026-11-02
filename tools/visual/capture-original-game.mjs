// Capture real in-game Phaser UI states from the pinned local PokéRogue app.
// The app and its BattleScene run normally; this driver only starts the
// deterministic fixture and advances the actual UI handlers between captures.
import { chromium } from "playwright";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const originalRoot = path.join(root, "original");
const output = path.resolve(process.argv[2] ?? path.join(root, "visual-artifacts/full-game/battle-town"));
const replayPort = Number(process.env.POKERUST_REPLAY_PORT ?? 8123);
const origin = `http://127.0.0.1:${replayPort}`;
const browserExecutable = process.env.POKERUST_BROWSER
	?? (["/usr/bin/helium-browser", "/usr/bin/helium"].find((candidate) => fs.existsSync(candidate)) ?? undefined);
const fixture = {
	resolution: [960, 540],
	biome: "TOWN",
	seed: "MiniRunSeed0001",
	player: { speciesId: 1, name: "Bulbasaur", level: 5, moves: ["Vine Whip", "Tackle"] },
	enemy: { speciesId: 4, name: "Charmander", level: 5, moves: ["Tackle"] },
};
const replayMode = process.env.POKERUST_REPLAY_TRACE === "1";
const replayThroughCommand = process.env.POKERUST_REPLAY_THROUGH_COMMAND === "1";
const replaySeed = process.env.POKERUST_REPLAY_SEED ?? "M12TownWave1Replay6";
const replayAction = process.env.POKERUST_REPLAY_ACTION ?? "";
const replayStopAfterReplacement = process.env.POKERUST_REPLAY_STOP_AFTER_REPLACEMENT === "1";
if (replayAction && !["first-move", "move-sequence"].includes(replayAction)) throw new Error(`Unsupported replay action ${replayAction}`);
const replayMoveSequence = replayAction === "first-move" ? [0] : replayAction === "move-sequence"
	? (process.env.POKERUST_REPLAY_MOVE_SEQUENCE ?? "").split(",").map(value => Number(value.trim()))
	: [];
if (replayAction === "move-sequence" && (!replayMoveSequence.length || replayMoveSequence.some(index => !Number.isInteger(index) || index < 0 || index > 3)))
	throw new Error("POKERUST_REPLAY_MOVE_SEQUENCE must be a comma-separated list of move indexes 0–3");
const replayPartySize = Number(process.env.POKERUST_REPLAY_PARTY_SIZE ?? 1);
if (!Number.isInteger(replayPartySize) || replayPartySize < 1 || replayPartySize > 2) throw new Error(`Unsupported replay party size ${replayPartySize}`);
const replayStarterIds = (process.env.POKERUST_REPLAY_STARTER_IDS ?? (replayPartySize === 1 ? "1" : "1,4"))
	.split(",").map(value => Number(value.trim()));
if (replayStarterIds.length !== replayPartySize || replayStarterIds.some(id => !Number.isInteger(id) || id < 1))
	throw new Error("POKERUST_REPLAY_STARTER_IDS must contain one valid species ID per selected starter");
const bootTimeoutMs = Number(process.env.POKERUST_BOOT_TIMEOUT_MS ?? 60000);
// Software-rendered browsers can take minutes to finish only the opening
// source tweens. Accelerate that presentation window, then restore normal
// timing before the settled command/move screenshot. Gameplay and RNG remain
// source-owned; the captured UI state is still reached through PhaseManager.
const captureTimeScale = Number(process.env.POKERUST_CAPTURE_TIME_SCALE ?? 0);
const browserGl = process.env.POKERUST_BROWSER_GL ?? "swiftshader";
const browserGlArgs = browserGl === "swiftshader"
	? ["--use-angle=swiftshader"]
	: browserGl === "swiftshader-webgl"
		? ["--use-angle=swiftshader-webgl"]
	: [`--use-gl=${browserGl}`];

function run(command, args, options = {}) {
	return new Promise((resolve, reject) => {
		const child = spawn(command, args, { stdio: ["ignore", "pipe", "pipe"], ...options });
		let stdout = "";
		let stderr = "";
		child.stdout.on("data", (chunk) => (stdout += chunk));
		child.stderr.on("data", (chunk) => (stderr += chunk));
		child.on("error", reject);
		child.on("close", (code) => {
			if (code === 0) resolve({ stdout, stderr });
			else reject(new Error(`${command} exited ${code}: ${stderr || stdout}`));
		});
	});
}

function sha256(filename) {
	return createHash("sha256").update(fs.readFileSync(filename)).digest("hex");
}

async function capturePage(page, name) {
	const nativePath = path.join(output, name.replace(/\.png$/, "-native.png"));
	await page.screenshot({ path: nativePath, animations: "disabled", scale: "device" });
	await run("magick", [nativePath, "-filter", "point", "-resize", "960x540!", path.join(output, name)]);
}

async function waitForServer(server, timeoutMs = 60000) {
	const deadline = Date.now() + timeoutMs;
	let lastError;
	while (Date.now() < deadline) {
		if (server.exitCode !== null) throw new Error(`Vite server exited ${server.exitCode}`);
		try {
			const response = await fetch(origin);
			if (response.ok) return;
		} catch (error) {
			lastError = error;
		}
		await new Promise((resolve) => setTimeout(resolve, 250));
	}
	throw new Error(`Timed out waiting for local Phaser app: ${lastError}`);
}

function stop(child) {
	if (child.exitCode === null) child.kill("SIGTERM");
}

fs.mkdirSync(output, { recursive: true });
for (const name of ["A-command.png", "B-moves.png", "reference.json", "encounter-timeout.png", "boot-failure.png"]) {
	fs.rmSync(path.join(output, name), { force: true });
}
const vite = path.join(originalRoot, "node_modules/.bin/vite");
const server = spawn(
	vite,
	["--config", "../visual-harness/vite.config.mts", "--host", "127.0.0.1", "--port", String(replayPort), "--strictPort"],
	{ cwd: originalRoot, stdio: ["ignore", fs.openSync(path.join(output, "vite.log"), "w"), fs.openSync(path.join(output, "vite.log"), "a")] },
);
let browser;
const browserLifecycle = [];
const metadata = {
	status: "incomplete",
	reference: "full-local-pokerogue-phaser-app",
	fixture,
	limitations: [
		"The driver uses the real Phaser BattleScene, field, BattleInfo, message handler, command handler, and fight handler.",
		"Replay mode completes gender and starter selection through original UI handlers; non-replay screenshot mode skips them through the original phase API. Tutorials are suppressed only in this isolated harness.",
		...(captureTimeScale > 0 ? [`The isolated browser accelerates Phaser presentation timers by ${captureTimeScale}x from the first-run gender prompt (or starter selection if no prompt appears), then restores 1x at CommandPhase; original UI confirmation inputs, phase callbacks and RNG remain active.`] : []),
		"Persistence calls are replaced with successful in-memory responses in this disposable browser context; no save is read or written.",
		"Capture A is after the opening encounter sequence; capture B opens Fight through the original CommandUiHandler.",
		"If first-run onboarding reaches an empty TurnInitPhase without a party, the isolated capture driver requeues the original SelectStarterPhase through PhaseManager.",
	],
};

try {
	const [sourceRevision, assetRevision, phaserVersion] = await Promise.all([
		run("git", ["-C", originalRoot, "rev-parse", "HEAD"]).then((r) => r.stdout.trim()),
		run("git", ["-C", path.join(originalRoot, "assets"), "rev-parse", "HEAD"]).then((r) => r.stdout.trim()),
		Promise.resolve(JSON.parse(fs.readFileSync(path.join(originalRoot, "node_modules/phaser/package.json"))).version),
	]);
	if (sourceRevision !== "7c0881255af5cd51992e513ff93c2f71adbe40e0") throw new Error(`Unexpected original revision ${sourceRevision}`);
	if (assetRevision !== "fb8ccb6d0703ffe48cdf80a8102d5ed943c82dba") throw new Error(`Unexpected asset revision ${assetRevision}`);
	if (phaserVersion !== "4.2.1") throw new Error(`Unexpected Phaser version ${phaserVersion}`);
	metadata.source_revision = sourceRevision;
	metadata.asset_revision = assetRevision;
	metadata.phaser_version = phaserVersion;
	metadata.source_hashes = Object.fromEntries(
		[
			"src/battle-scene.ts",
			"src/phase-manager.ts",
			"src/phases/select-starter-phase.ts",
			"src/phases/encounter-phase.ts",
			"src/ui/handlers/battle-message-ui-handler.ts",
			"src/ui/handlers/command-ui-handler.ts",
			"src/ui/handlers/fight-ui-handler.ts",
		].map((file) => [file, sha256(path.join(originalRoot, file))]),
	);
	await waitForServer(server);
	browser = await chromium.launch({
		headless: true,
		args: [...browserGlArgs, "--disable-dev-shm-usage"],
		executablePath: browserExecutable,
	});
	metadata.browser_version = browser.version();
	metadata.browser_executable = browserExecutable ?? "Playwright bundled Chromium";
	const page = await browser.newPage({ viewport: { width: 960, height: 540 }, deviceScaleFactor: 2 });
	const errors = [];
	const consoleErrors = [];
	const httpErrors = [];
	const failedRequests = [];
	page.on("crash", () => browserLifecycle.push({ kind: "page-crash", at_ms: Date.now() }));
	page.on("close", () => browserLifecycle.push({ kind: "page-close", at_ms: Date.now() }));
	browser.on("disconnected", () => browserLifecycle.push({ kind: "browser-disconnected", at_ms: Date.now() }));
	page.on("pageerror", (error) => errors.push(String(error)));
	page.on("requestfailed", (request) => failedRequests.push({ url: request.url(), error: request.failure()?.errorText }));
	page.on("console", (message) => {
		if (message.type() === "error") consoleErrors.push(message.text());
	});
	page.on("response", (response) => {
		if (response.status() >= 400) httpErrors.push(`${response.status()} ${response.url()}`);
	});
	await page.route("**/*", (route) => {
		if (new URL(route.request().url()).origin === origin) return route.continue();
		errors.push(`Blocked external request: ${route.request().url()}`);
		return route.abort();
	});
	await page.goto(`${origin}/`, { waitUntil: "domcontentloaded", timeout: 60000 });
	try {
		await page.waitForFunction(() => window.__pokerogueVisualScene, null, { timeout: bootTimeoutMs });
		await page.waitForFunction(() => window.__pokerogueVisualScene?.gameData, null, { timeout: bootTimeoutMs });
	await page.evaluate(() => {
		window.__pokerogueVisualSuppressTutorials = true;
		const gameData = window.__pokerogueVisualScene.gameData;
		gameData.saveAll = async () => true;
		gameData.saveSystem = async () => true;
	});
	} catch (error) {
		metadata.boot_diagnostic = { page_closed: page.isClosed(), browser_connected: browser.isConnected(), lifecycle: browserLifecycle };
		try {
			Object.assign(metadata.boot_diagnostic, await page.evaluate(() => ({
				title: document.title,
				body: document.body.innerText.slice(0, 1000),
				canvas: [...document.querySelectorAll("canvas")].map((canvas) => ({ width: canvas.width, height: canvas.height })),
				exposed_scene: Boolean(window.__pokerogueVisualScene),
				game_data: window.__pokerogueVisualScene?.gameData && { constructor: window.__pokerogueVisualScene.gameData.constructor?.name, saveAll: typeof window.__pokerogueVisualScene.gameData.saveAll, saveSystem: typeof window.__pokerogueVisualScene.gameData.saveSystem },
				phase: window.__pokerogueVisualScene?.phaseManager?.getCurrentPhase?.()?.phaseName ?? null,
				ui_mode: window.__pokerogueVisualScene?.ui?.mode ?? null,
				ui_handler: window.__pokerogueVisualScene?.ui?.getHandler?.()?.constructor?.name ?? null,
			})));
		} catch (diagnosticError) {
			metadata.boot_diagnostic.diagnostic_error = String(diagnosticError);
		}
		metadata.boot_diagnostic.browser_errors = errors;
		metadata.boot_diagnostic.console_errors = consoleErrors;
		metadata.boot_diagnostic.http_errors = httpErrors;
		metadata.boot_diagnostic.browser_errors = errors;
		metadata.boot_diagnostic.failed_requests = failedRequests;
		metadata.boot_diagnostic.scripts = await page.locator("script").evaluateAll(scripts => scripts.map(script => ({ src: script.src, type: script.type })));
		metadata.boot_diagnostic.links = await page.locator("link[rel=modulepreload]").evaluateAll(links => links.map(link => link.href)).catch(() => []);
		metadata.status = "boot-failure";
		fs.writeFileSync(path.join(output, "reference.json"), `${JSON.stringify(metadata, null, 2)}\n`);
		await page.screenshot({ path: path.join(output, "boot-failure.png") }).catch(() => {});
		throw new Error(`${error instanceof Error ? error.message : String(error)}; boot diagnostic: ${JSON.stringify(metadata.boot_diagnostic)}`);
	}
	const readState = () =>
		page.evaluate(() => {
			const scene = window.__pokerogueVisualScene;
			const phase = scene?.phaseManager?.getCurrentPhase?.();
			const [player] = scene?.getPlayerParty?.() ?? [];
			const [enemy] = scene?.getEnemyParty?.() ?? [];
			const uiHandler = scene?.ui?.getHandler?.();
			const messageHandler = scene?.ui?.getCurrentMessageHandler?.();
			const messageTimer = messageHandler?.textTimer;
			return {
				phase: phase?.phaseName,
				phase_runtime: phase && { constructor: phase.constructor?.name, init_battle: typeof phase.initBattle, own_keys: Object.keys(phase), prototype_keys: Object.getOwnPropertyNames(Object.getPrototypeOf(phase) ?? {}) },
				phase_manager: (() => { const manager = scene?.phaseManager; return manager && { keys: Object.keys(manager), phase_queue: manager.phaseQueue?.levels?.flat?.().map((queued) => queued.phaseName) ?? [], dynamic_manager: Object.keys(manager.dynamicPhaseQueueManager ?? {}), standby: manager.getStandbyPhase?.()?.phaseName ?? null, active: manager.isActive }; })(),
				phase_queue: scene?.phaseManager?.phaseQueue?.levels?.flat?.().map((queued) => queued.phaseName) ?? [],
				ui_mode: scene?.ui?.mode,
				ui_ready: typeof scene?.ui?.processInput === "function",
				awaiting_action: messageHandler?.awaitingActionInput ?? false,
				message_handler: messageHandler?.constructor?.name ?? null,
				ui_handler: uiHandler?.constructor?.name ?? null,
				ui_handler_blocked: uiHandler?.blockInput ?? false,
				ui_handler_tutorial_active: uiHandler?.tutorialActive ?? false,
				message_text: messageHandler?.message?.text ?? uiHandler?.message?.text ?? null,
				message_timer: messageTimer && { delay: messageTimer.delay, elapsed: messageTimer.elapsed, repeat: messageTimer.repeat, repeat_count: messageTimer.getRepeatCount?.(), paused: messageTimer.paused, progress: messageTimer.getProgress?.() },
				message_pending_prompt: messageHandler?.pendingPrompt ?? false,
				message_action_callback: typeof messageHandler?.onActionInput === "function",
				message_callback_timer: messageHandler?.textCallbackTimer && { delay: messageHandler.textCallbackTimer.delay, elapsed: messageHandler.textCallbackTimer.elapsed, paused: messageHandler.textCallbackTimer.paused, repeat_count: messageHandler.textCallbackTimer.getRepeatCount?.() },
				active_clock_events: scene?.time?._active?.slice?.(0, 12).map((event) => ({ delay: event.delay, elapsed: event.elapsed, paused: event.paused, repeat_count: event.getRepeatCount?.(), callback: event.callback?.name ?? "" })) ?? [],
				clock: scene?.time && { now: scene.time.now, paused: scene.time.paused, time_scale: scene.time.timeScale },
				ui_handler_cursor: scene?.ui?.getHandler?.()?.cursor ?? null,
				starter_count: scene?.getPlayerParty?.()?.length ?? 0,
				starter_ui: (() => { const h = scene?.ui?.getHandler?.(); return h && { grid: h.cursorObj?.visible, random: h.randomCursorObj?.visible, start: h.startCursorObj?.visible, filtered: h.filteredStarterIds?.length, selected: h.partyStarterIds?.length }; })(),
				wave: scene?.currentBattle?.waveIndex,
				biome: scene?.arena?.biomeId,
				player: player && { species_id: player.species.speciesId, level: player.level, hp: player.hp, max_hp: player.getMaxHp(), gender: player.gender === 0 ? "male" : player.gender === 1 ? "female" : "", moves: player.getMoveset().map((m) => m.getMove().name) },
				enemy: enemy && { species_id: enemy.species.speciesId, level: enemy.level, hp: enemy.hp, max_hp: enemy.getMaxHp(), gender: enemy.gender === 0 ? "male" : enemy.gender === 1 ? "female" : "", owned_icon: enemy.getBattleInfo()?.getByName("icon_owned")?.visible ?? false, moves: enemy.getMoveset().map((m) => m.getMove().name) },
				canvas: (() => { const c = document.querySelector("canvas"); return c && { width: c.width, height: c.height, css: c.getBoundingClientRect().toJSON() }; })(),
				loader: { is_loading: scene?.load?.isLoading?.(), progress: scene?.load?.progress, total: scene?.load?.totalToLoad, complete: scene?.load?.totalComplete },
			};
		});
	let lastAction = 0;
	let finalState;
	let enteredBattleSetup = false;
	let replayInputStage = 0;
	let replayStarterGridRecorded = false;
	let replayTimeScaled = false;
	let commandTimeRestored = false;
	let replayActionStage = 0;
	let replayActionIndex = 0;
	let replayReplacementConfirmed = false;
	let replayActionSawMove = false;
	let replayActionSawTurnEnd = false;
	let lastBootstrapInputSignature;
	let lastPhase;
	let lastUiSignature;
	const phaseHistory = [];
	const startedAt = Date.now();
	const deadline = startedAt + Number(process.env.POKERUST_REPLAY_TIMEOUT_MS ?? 180000);
	while (Date.now() < deadline) {
		const state = await readState();
		if (replayMode) {
			metadata.last_observation = state;
			fs.writeFileSync(path.join(output, "reference.json"), `${JSON.stringify(metadata, null, 2)}\n`);
			const capturedEvents = await page.evaluate(() => window.__pokerustReplayEvents ?? []);
			if (capturedEvents.some(event => event.kind === "phase.start" && event.phase === "MovePhase")) replayActionSawMove = true;
			if (replayActionSawMove && capturedEvents.some(event => event.kind === "phase.end" && event.phase === "TurnEndPhase")) replayActionSawTurnEnd = true;
			const newBattleEnd = capturedEvents.findIndex((event) => event.kind === "newBattle.end");
			const replayEventsRaw = replayThroughCommand || newBattleEnd < 0
				? capturedEvents
				: capturedEvents.slice(0, newBattleEnd + 1);
			const replayEvents = replayEventsRaw.filter(event => !(event.kind === "harness.input" && event.accepted === false));
			const replacementConfirmationIndex = replayStopAfterReplacement
				? capturedEvents.findIndex(event => event.kind === "harness.input" && event.command === "confirm-replacement" && event.accepted === true)
				: -1;
			const replacementCommandBoundary = replacementConfirmationIndex >= 0
				&& capturedEvents.some((event, index) => index > replacementConfirmationIndex && event.kind === "phase.command-boundary");
			const commandUiReady = state.ui_mode === 2 || (replayStopAfterReplacement && state.ui_mode === 3);
			const reachedBoundary = state.phase === "CommandPhase" && ((commandUiReady && (!replayAction || (replayActionStage === 2 && replayActionSawMove && replayActionSawTurnEnd && replayActionIndex >= replayMoveSequence.length - 1))) || (replayStopAfterReplacement && replayReplacementConfirmed && replacementCommandBoundary));
			if (replayEvents.some((event) => event.kind === "newBattle.end") &&
				(!replayThroughCommand || reachedBoundary)) {
				metadata.replay_events = replayEvents;
				metadata.status = "captured";
				metadata.observation_boundary = replayThroughCommand
					? replayStopAfterReplacement ? "first player CommandPhase after original faint and mandatory replacement phases" : replayAction ? "next player CommandPhase after one original UI-selected move and the source turn phases complete" : "first player CommandPhase after the real EncounterPhase completes"
					: "immediately after the real BattleScene.newBattle return, before EncounterPhase constructs opponents";
				const sourceEntry = replayEvents.find(event => event.kind === "newBattle.begin");
				const sourceTimeOfDay = ["DAWN", "DAY", "DUSK", "NIGHT"][sourceEntry?.before?.time_of_day] ?? "UNKNOWN";
				const trace = {
					schema: "pokerust-source-replay/v1",
					provenance: { source_revision: metadata.source_revision, asset_revision: metadata.asset_revision, phaser_version: metadata.phaser_version, source_hashes: metadata.source_hashes },
					scenario: { seed: replaySeed, game_mode: "CLASSIC", biome: "TOWN", time_of_day: sourceTimeOfDay, wave: 1, starter_input: metadata.starter_confirmation_bypass ? "Starter(s) selected through original starter grid; source initBattle called after blocked final confirmation" : "Starter(s) via original starter grid and save-slot UI", starter_count: replayPartySize, ...(process.env.POKERUST_REPLAY_ACTION ? { action: process.env.POKERUST_REPLAY_ACTION } : {}), ...(replayStopAfterReplacement ? { stop_after_replacement: true } : {}) },
					controlled_collaborators: ["Tutorials suppressed in the isolated browser", "Persistence calls replaced with in-memory success", "One save slot confirmed through original UI", ...(metadata.starter_confirmation_bypass ? [metadata.starter_confirmation_bypass] : [])],
					observations: replayEvents,
					...(replayThroughCommand ? { command_boundary_state: replayStopAfterReplacement
						? { phase: state.phase, phase_queue: state.phase_queue, wave: state.wave, biome: state.biome, player: state.player, enemy: state.enemy }
						: state } : {}),
				};
				fs.writeFileSync(path.join(output, "replay-trace.json"), `${JSON.stringify(trace, null, 2)}\n`);
				fs.writeFileSync(path.join(output, "reference.json"), `${JSON.stringify(metadata, null, 2)}\n`);
				finalState = state;
				break;
			}
		}
		if (state.phase !== lastPhase) {
			phaseHistory.push({ phase: state.phase, ui_mode: state.ui_mode, elapsed_ms: Date.now() - startedAt });
			lastPhase = state.phase;
			metadata.phase_history = phaseHistory;
			fs.writeFileSync(path.join(output, "reference.json"), `${JSON.stringify(metadata, null, 2)}\n`);
		}
		if (!enteredBattleSetup && state.phase === "TurnInitPhase" && state.starter_count === 0) {
			metadata.driver_recovery = {
				trigger: "empty TurnInitPhase before starter selection",
				method: "clear pending phases, queue original SelectStarterPhase, then end the empty TurnInitPhase through PhaseManager",
			};
			await page.evaluate(() => {
				const manager = window.__pokerogueVisualScene.phaseManager;
				manager.clearAllPhases();
				manager.unshiftNew("SelectStarterPhase");
				manager.getCurrentPhase().end();
			});
			await page.waitForTimeout(200);
			continue;
		}
		if (replayMode && state.phase === "SelectGenderPhase" && captureTimeScale > 0 && !replayTimeScaled) {
			await page.evaluate((scale) => {
				const scene = window.__pokerogueVisualScene;
				scene.time.timeScale = scale;
				scene.tweens.timeScale = scale;
				scene.anims.globalTimeScale = scale;
			}, captureTimeScale);
			replayTimeScaled = true;
		}
		const uiSignature = `${state.phase}/${state.ui_mode}/${state.ui_handler}/${state.ui_handler_blocked}/${state.awaiting_action}/${state.message_text}`;
		if (uiSignature !== lastUiSignature) {
			(metadata.ui_history ??= []).push({ at_ms: Date.now() - startedAt, phase: state.phase, mode: state.ui_mode, handler: state.ui_handler, blocked: state.ui_handler_blocked, awaiting_action: state.awaiting_action, text: state.message_text, timer: state.message_timer, clock: state.clock });
			lastUiSignature = uiSignature;
		}
		if (replayThroughCommand && state.phase === "CheckSwitchPhase" && state.ui_mode === 14 && state.ui_handler === "ConfirmUiHandler" && !state.ui_handler_blocked && Date.now() - lastAction > 700) {
			const button = state.ui_handler_cursor === 0 ? 1 : 5;
			const command = button === 1 ? "select-no-initial-switch" : "confirm-no-initial-switch";
			const result = await page.evaluate(({ input, label }) => {
				const scene = window.__pokerogueVisualScene;
				const handler = scene.ui.getHandler();
				const event = { kind: "harness.input", command: label, button: input, phase: scene.phaseManager.getCurrentPhase()?.phaseName, handler: handler?.constructor?.name, cursor: handler?.cursor };
				const accepted = scene.ui.processInput(input);
				event.accepted = accepted;
				window.__pokerustReplayEvents.push(event);
				return { accepted, event };
			}, { input: button, label: command });
			(metadata.source_actions ??= []).push({ at_ms: Date.now() - startedAt, ...result.event, result: result.accepted });
			lastAction = Date.now();
			await page.waitForTimeout(200);
			continue;
		}
		if (replayThroughCommand && state.phase === "SwitchPhase" && state.ui_mode === 8 && state.ui_handler === "PartyUiHandler" && !state.ui_handler_blocked && Date.now() - lastAction > 700) {
			const result = await page.evaluate(() => {
				const scene = window.__pokerogueVisualScene;
				const handler = scene.ui.getHandler();
				const forcedReplacement = handler.partyUiMode === 1;
				if (!forcedReplacement) return { accepted: false, ignored: true };
				const button = handler.optionsMode ? 5 : handler.cursor === 1 ? 5 : 1;
				const command = handler.optionsMode ? "confirm-replacement" : handler.cursor === 1 ? "open-replacement-options" : "select-reserve";
				const event = { kind: "harness.input", command, button, phase: scene.phaseManager.getCurrentPhase()?.phaseName, handler: handler.constructor.name, party_mode: handler.partyUiMode, cursor: handler.cursor, option_cursor: handler.optionsCursor };
				const accepted = scene.ui.processInput(button);
				event.accepted = accepted;
				window.__pokerustReplayEvents.push(event);
				return { accepted, event };
			});
			if (!result.ignored) {
				(metadata.source_actions ??= []).push({ at_ms: Date.now() - startedAt, ...result.event, result: result.accepted });
				if (result.event.command === "confirm-replacement" && result.accepted) replayReplacementConfirmed = true;
				lastAction = Date.now();
			}
			await page.waitForTimeout(200);
			continue;
		}
		if (replayThroughCommand && state.phase === "SelectTargetPhase" && state.ui_mode === 5 && state.ui_handler === "TargetSelectUiHandler" && !state.ui_handler_blocked && Date.now() - lastAction > 700) {
			const result = await page.evaluate(() => {
				const scene = window.__pokerogueVisualScene;
				const handler = scene.ui.getHandler();
				const event = { kind: "harness.input", command: "confirm-target", button: 5, phase: scene.phaseManager.getCurrentPhase()?.phaseName, handler: handler.constructor.name, cursor: handler.cursor };
				event.accepted = scene.ui.processInput(5);
				window.__pokerustReplayEvents.push(event);
				return event;
			});
			(metadata.source_actions ??= []).push({ at_ms: Date.now() - startedAt, ...result, result: result.accepted });
			lastAction = Date.now();
			await page.waitForTimeout(200);
			continue;
		}
		if (state.phase === "SelectStarterPhase" && (replayMode || !enteredBattleSetup)) {
			enteredBattleSetup = true;
			if (replayMode) {
				if (!replayStarterGridRecorded) {
					const starterGridIds = await page.evaluate(() => window.__pokerogueVisualScene.ui.getHandler()?.filteredStarterIds?.slice() ?? []);
					if (starterGridIds.length > 0) {
						metadata.starter_grid_ids = starterGridIds;
						replayStarterGridRecorded = true;
						for (const id of replayStarterIds) {
							if (!starterGridIds.includes(id)) throw new Error(`Requested starter species ${id} is absent from the original starter UI grid`);
						}
					}
				}
				if (!replayStarterGridRecorded) {
					await page.waitForTimeout(200);
					continue;
				}
				metadata.replay_boundary = "normal starter UI; source SelectStarterPhase.initBattle and BattleScene.newBattle";
				if (captureTimeScale > 0 && !replayTimeScaled) {
					await page.evaluate((scale) => {
						const scene = window.__pokerogueVisualScene;
						scene.time.timeScale = scale;
						scene.tweens.timeScale = scale;
						scene.anims.globalTimeScale = scale;
					}, captureTimeScale);
					replayTimeScaled = true;
				}
				if (Date.now() - lastAction > 900 && !state.ui_handler_blocked && [7, 10, 13, 14, 15].includes(state.ui_mode)) {
					let button;
					if (state.ui_mode === 7 || state.ui_mode === 13 || state.ui_mode === 14 || state.ui_mode === 15) button = 5;
					else if ((state.starter_ui?.selected ?? 0) >= replayPartySize) button = 4;
					else {
						const targetId = replayStarterIds[state.starter_ui?.selected ?? 0];
						const cursorState = await page.evaluate((id) => {
							const handler = window.__pokerogueVisualScene.ui.getHandler();
						const targetCursor = handler.filteredStarterIds.indexOf(id);
						const cursor = handler.cursor;
						if (targetCursor < 0) return { targetCursor };
						const currentRow = Math.floor(cursor / 9), targetRow = Math.floor(targetCursor / 9);
						const currentColumn = cursor % 9, targetColumn = targetCursor % 9;
						const button = currentRow < targetRow ? 1 : currentRow > targetRow ? 0 : currentColumn < targetColumn ? 3 : currentColumn > targetColumn ? 2 : 5;
						return { targetCursor, button, currentSpeciesId: handler.filteredStarterIds[cursor] };
						}, targetId);
						if (cursorState.targetCursor < 0) throw new Error(`Requested starter species ${targetId} is not available in the current filtered original UI grid`);
						button = cursorState.button;
						replayInputStage = 1;
					}
					metadata.last_input = { phase: state.phase, mode: state.ui_mode, handler: state.ui_handler, button,
						result: await page.evaluate((input) => window.__pokerogueVisualScene.ui.processInput(input), button) };
					(metadata.replay_actions ??= []).push({ ...metadata.last_input, at_ms: Date.now() - startedAt, blocked: state.ui_handler_blocked,
						tutorial_active: state.ui_handler_tutorial_active, awaiting_action: state.awaiting_action,
						message_handler: state.message_handler, message_text: state.message_text });
					lastAction = Date.now();
					if (replayMode) fs.writeFileSync(path.join(output, "reference.json"), `${JSON.stringify(metadata, null, 2)}\n`);
				}
				await page.waitForTimeout(200);
				continue;
			}
			await page.waitForFunction(() => window.__pokerogueVisualScene?.ui?.mode === 10, null, { timeout: 30000 });
			await page.evaluate(async (scale) => {
				const scene = window.__pokerogueVisualScene;
				await scene.ui.setMode(0);
				scene.money = scene.gameMode.getStartingMoney();
				scene.phaseManager.getCurrentPhase().initBattle([{
					speciesId: 1, shiny: false, variant: 0, formIndex: 0, abilityIndex: 0,
					passive: false, nature: 0, pokerus: false, ivs: [0, 0, 0, 0, 0, 0],
				}]);
				if (scale > 0) {
					scene.time.timeScale = scale;
					scene.tweens.timeScale = scale;
					scene.anims.globalTimeScale = scale;
				}
			}, captureTimeScale);
		}
		if (state.phase === "SelectGenderPhase") {
			if (!replayMode) {
				// Non-replay screenshot mode only needs the profile preference skipped.
				await page.evaluate(() => window.__pokerogueVisualScene.phaseManager.getCurrentPhase().end());
				await page.waitForTimeout(100);
				continue;
			}
		}
		if (state.phase === "CommandPhase" && state.ui_mode === 2) {
			if (captureTimeScale > 0 && !commandTimeRestored && !replayAction) {
				await page.evaluate(() => {
					const scene = window.__pokerogueVisualScene;
					scene.time.timeScale = 1;
					scene.tweens.timeScale = 1;
					scene.anims.globalTimeScale = 1;
				});
				commandTimeRestored = true;
				await page.waitForTimeout(300);
			}
			if (!replayAction) {
				finalState = await readState();
				break;
			}
			if (replayActionStage === 2) {
				if (replayActionSawMove && replayActionSawTurnEnd) {
					replayActionIndex++;
					replayActionStage = 0;
					replayActionSawMove = false;
					replayActionSawTurnEnd = false;
				}
				if (replayActionStage === 0 && replayActionIndex >= replayMoveSequence.length) {
				if (captureTimeScale > 0 && !commandTimeRestored) {
						await page.evaluate(() => {
							const scene = window.__pokerogueVisualScene;
							scene.time.timeScale = 1;
							scene.tweens.timeScale = 1;
							scene.anims.globalTimeScale = 1;
						});
						commandTimeRestored = true;
						await page.waitForTimeout(300);
					}
					finalState = await readState();
					break;
				}
			}
			if (replayActionStage === 0) {
				const result = await page.evaluate(() => {
					const scene = window.__pokerogueVisualScene;
					const event = { kind: "harness.input", command: "Fight", button: 5, phase: scene.phaseManager.getCurrentPhase()?.phaseName, handler: scene.ui.getHandler()?.constructor?.name };
					const accepted = scene.ui.processInput(5);
					window.__pokerustReplayEvents.push(event);
					return accepted;
				});
				(metadata.source_actions ??= []).push({ at_ms: Date.now() - startedAt, phase: state.phase, mode: state.ui_mode, handler: state.ui_handler, command: "Fight", button: 5, result });
				replayActionStage = 1;
				lastAction = Date.now();
			}
			await page.waitForTimeout(200);
			continue;
		}
		if (replayAction && replayActionStage === 1 && state.phase === "CommandPhase" && state.ui_mode === 3 && state.ui_handler === "FightUiHandler") {
			const moveIndex = replayMoveSequence[replayActionIndex];
			if (state.ui_handler_cursor !== moveIndex) {
				const cursorRow = Math.floor(state.ui_handler_cursor / 2);
				const targetRow = Math.floor(moveIndex / 2);
				const cursorColumn = state.ui_handler_cursor % 2;
				const targetColumn = moveIndex % 2;
				const button = cursorRow < targetRow ? 1 : cursorRow > targetRow ? 0 : cursorColumn < targetColumn ? 3 : 2;
				const result = await page.evaluate(({ input, target }) => {
					const scene = window.__pokerogueVisualScene;
					const handler = scene.ui.getHandler();
					const event = { kind: "harness.input", command: "select-move-index", move_index: target, button: input, phase: scene.phaseManager.getCurrentPhase()?.phaseName, handler: handler.constructor.name, cursor: handler.cursor };
					const accepted = scene.ui.processInput(input);
					event.accepted = accepted;
					window.__pokerustReplayEvents.push(event);
					return { accepted, event };
				}, { input: button, target: moveIndex });
				(metadata.source_actions ??= []).push({ at_ms: Date.now() - startedAt, ...result.event, result: result.accepted });
				lastAction = Date.now();
				await page.waitForTimeout(200);
				continue;
			}
			const result = await page.evaluate(({ target, request }) => {
				const scene = window.__pokerogueVisualScene;
				const handler = scene.ui.getHandler();
				const pokemon = scene.getPlayerField()[handler.fieldIndex];
				const move = pokemon?.getMoveset?.()[target];
				const event = { kind: "harness.input", command: request, button: 5, phase: scene.phaseManager.getCurrentPhase()?.phaseName, handler: handler.constructor.name, move_index: target, move_id: move?.moveId };
				const accepted = scene.ui.processInput(5);
				event.accepted = accepted;
				window.__pokerustReplayEvents.push(event);
				return { accepted, event };
			}, { target: moveIndex, request: replayAction === "first-move" ? "first-move" : `move-sequence-${replayActionIndex}` });
			(metadata.source_actions ??= []).push({ at_ms: Date.now() - startedAt, ...result.event, result: result.accepted });
			replayActionStage = 2;
			lastAction = Date.now();
			await page.waitForTimeout(200);
			continue;
		}
		if (!enteredBattleSetup && state.ui_ready && !state.ui_handler_blocked && !(state.message_timer?.repeat_count > 0) && [1, 14, 15].includes(state.ui_mode)) {
			const signature = `${state.phase}/${state.ui_mode}/${state.ui_handler}/${state.ui_handler_cursor}`;
			if (signature !== lastBootstrapInputSignature && Date.now() - lastAction > 500) {
				const result = await page.evaluate(() => window.__pokerogueVisualScene.ui.processInput(5));
				(metadata.bootstrap_actions ??= []).push({ at_ms: Date.now() - startedAt, phase: state.phase, mode: state.ui_mode, handler: state.ui_handler, cursor: state.ui_handler_cursor, result });
				lastBootstrapInputSignature = signature;
				lastAction = Date.now();
			}
		} else if (enteredBattleSetup && state.ui_mode === 0 && state.awaiting_action && Date.now() - lastAction > 850) {
			const input = await page.evaluate(() => {
				const scene = window.__pokerogueVisualScene;
				const event = { kind: "harness.input", command: "continue-message", button: 5, phase: scene.phaseManager.getCurrentPhase()?.phaseName, handler: scene.ui.getHandler()?.constructor?.name };
				const accepted = scene.ui.processInput(5);
				window.__pokerustReplayEvents?.push(event);
				return { accepted, event };
			});
			(metadata.source_actions ??= []).push({ at_ms: Date.now() - startedAt, ...input.event, result: input.accepted });
			lastAction = Date.now();
		}
		await page.waitForTimeout(200);
	}
	metadata.phase_history = phaseHistory;
	metadata.http_errors = httpErrors;
	metadata.console_errors = consoleErrors;
	if (replayMode) {
		if (!finalState) {
			const diagnostic = await readState();
			metadata.last_observation = diagnostic;
			metadata.replay_event_tail = await page.evaluate(() => (window.__pokerustReplayEvents ?? []).slice(-80)).catch((eventError) => ({ error: String(eventError) }));
			metadata.status = "timeout";
			await page.screenshot({ path: path.join(output, "replay-timeout.png") }).catch(() => {});
			throw new Error(`Replay timed out; last observation: ${JSON.stringify(diagnostic)}; recent inputs: ${JSON.stringify(metadata.replay_actions?.slice(-10) ?? [])}`);
		}
		metadata.status = "captured";
	} else {
	if (!finalState) {
		const diagnostic = await readState();
		await page.screenshot({ path: path.join(output, "encounter-timeout.png") }).catch(() => {});
		throw new Error(`Timed out waiting for real Phaser command menu: ${JSON.stringify(diagnostic)}`);
	}
	metadata.command_state = finalState;
	if (replayMode) {
		metadata.replay_events = await page.evaluate(() => window.__pokerustReplayEvents ?? []);
		if (!metadata.replay_events.some((event) => event.kind === "newBattle.end")) {
			throw new Error("Replay trace missing real BattleScene.newBattle observation");
		}
	}
	metadata.fixture.player.gender = finalState.player?.gender ?? "";
	metadata.fixture.enemy.gender = finalState.enemy?.gender ?? "";
	await page.evaluate(() => {
		const scene = window.__pokerogueVisualScene;
		for (const pokemon of [...scene.getPlayerParty(), ...scene.getEnemyParty()]) {
			for (const sprite of [pokemon.getSprite(), pokemon.getTintSprite()]) {
				if (!sprite) continue;
				sprite.anims?.pause();
				sprite.setFrame("0001.png");
			}
		}
	});
	metadata.sprite_capture = "idle animation paused at original 0001.png frame for deterministic pixel comparison";
	metadata.sprite_state = await page.evaluate(() =>
		[...window.__pokerogueVisualScene.getPlayerParty(), ...window.__pokerogueVisualScene.getEnemyParty()].map((pokemon) => {
			const sprite = pokemon.getSprite();
			return {
				species_id: pokemon.species.speciesId,
				pokemon_xy: [pokemon.x, pokemon.y],
				sprite_frame: sprite?.frame?.name,
				sprite_xy: [sprite?.x, sprite?.y],
				origin: [sprite?.originX, sprite?.originY],
				bounds: sprite?.getBounds?.().toJSON?.(),
			};
		}),
	);
	if (finalState.canvas?.css?.width !== 960 || finalState.canvas?.css?.height !== 540) {
		throw new Error(`Unexpected original canvas CSS resolution: ${JSON.stringify(finalState.canvas)}`);
	}
	if (errors.length) throw new Error(`Reference runtime errors: ${errors.slice(0, 20).join("; ")}`);
	metadata.capture_resolution = { logical: [960, 540], canvas: [finalState.canvas.width, finalState.canvas.height] };
	metadata.capture_method = "browser viewport at device scale 2, matching Phaser's 1920x1080 drawing buffer";
	await capturePage(page, "A-command.png");
	metadata.captures = ["A-command.png"];
	await page.evaluate(() => window.__pokerogueVisualScene.ui.processInput(5));
	await page.waitForFunction(() => window.__pokerogueVisualScene?.ui?.mode === 3, null, { timeout: 10000 });
	await page.waitForTimeout(300);
	metadata.fight_state = await readState();
	await capturePage(page, "B-moves.png");
	metadata.captures.push("B-moves.png");
	metadata.capture_hashes = Object.fromEntries(
		metadata.captures.map((name) => [name, sha256(path.join(output, name))]),
	);
	metadata.status = "captured";
	}
} catch (error) {
	metadata.status = "error";
	metadata.error = String(error?.stack || error);
	metadata.browser_lifecycle = browserLifecycle;
	throw error;
} finally {
	fs.writeFileSync(path.join(output, "reference.json"), `${JSON.stringify(metadata, null, 2)}\n`);
	await browser?.close();
	stop(server);
}
