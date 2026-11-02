// Capture the real pinned Phaser title UI at the shared 960x540 logical size.
import { chromium } from "playwright";
import fs from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const out = path.resolve(process.argv[2] ?? path.join(root, "visual-artifacts/title-screen"));
const port = 8124;
const origin = `http://127.0.0.1:${port}`;
const browserExecutable = "Playwright Chromium";
const vite = spawn(path.join(root, "original/node_modules/.bin/vite"), [
	"--config", "../visual-harness/vite.config.mts", "--host", "127.0.0.1", "--port", String(port), "--strictPort",
], { cwd: path.join(root, "original"), stdio: "ignore" });
let browser;
try {
	const deadline = Date.now() + 60000;
	while (Date.now() < deadline) {
		try { if ((await fetch(origin)).ok) break; } catch {}
		await new Promise((resolve) => setTimeout(resolve, 200));
	}
	browser = await chromium.launch({ headless: true, args: ["--use-angle=swiftshader", "--disable-dev-shm-usage"] });
	const page = await browser.newPage({ viewport: { width: 960, height: 540 }, deviceScaleFactor: 1 });
	const errors = [];
	page.on("pageerror", (error) => errors.push(String(error)));
	page.on("console", (message) => { if (message.type() === "error") errors.push(message.text()); });
	await page.route("**/*", (route) => new URL(route.request().url()).origin === origin ? route.continue() : route.abort());
	await page.goto(`${origin}/`, { waitUntil: "domcontentloaded", timeout: 60000 });
	await page.waitForFunction(() => window.__pokerogueVisualScene?.phaseManager?.getCurrentPhase, null, { timeout: 60000 });
	await page.waitForFunction(() => window.__pokerogueVisualScene?.gameData?.saveAll, null, { timeout: 60000 });
	await page.evaluate(() => {
		const scene = window.__pokerogueVisualScene;
		window.__pokerogueVisualSuppressTutorials = true;
		scene.gameData.saveAll = async () => true;
		scene.gameData.saveSystem = async () => true;
		scene.phaseManager.toTitleScreen();
		if (scene.phaseManager.getCurrentPhase()?.phaseName === "SelectGenderPhase") {
			scene.phaseManager.getCurrentPhase().end();
		}
	});
	await page.waitForFunction(() => {
		const scene = window.__pokerogueVisualScene;
		return scene?.ui?.mode === 1
			&& scene?.ui?.getHandler?.()?.constructor?.name === "TitleUiHandler";
	}, null, { timeout: 60000 });
	await page.evaluate(async () => {
		const ui = window.__pokerogueVisualScene.ui;
		// Remove any first-run overlay through Phaser UI's original mode-chain API.
		await ui.revertModes();
	});
	fs.mkdirSync(out, { recursive: true });
	const rawCapture = path.join(out, "original-raw.png");
	await page.screenshot({ path: rawCapture, animations: "disabled" });
	const state = await page.evaluate(() => {
		const canvas = document.querySelector("canvas");
		const handler = window.__pokerogueVisualScene.ui.getHandler();
		return { size: { width: canvas?.width, height: canvas?.height }, phase: window.__pokerogueVisualScene.phaseManager.getCurrentPhase()?.phaseName, mode: window.__pokerogueVisualScene.ui.mode, ui: handler.constructor.name, options: handler.options?.map((o) => o.label) ?? null };
	});
	const resize = spawn("magick", [rawCapture, "-filter", "point", "-resize", "960x540!", path.join(out, "original.png")]);
	await new Promise((resolve, reject) => { resize.on("close", (code) => code === 0 ? resolve() : reject(new Error(`magick exited ${code}`))); });
	fs.writeFileSync(path.join(out, "capture.json"), `${JSON.stringify({ reference: "executed-pinned-phaser-title-ui", source_revision: "7c0881255af5cd51992e513ff93c2f71adbe40e0", asset_revision: "fb8ccb6d0703ffe48cdf80a8102d5ed943c82dba", phaser_version: "4.2.1", browser: browserExecutable, capture_size: state.size, phase: state.phase, ui_mode: state.mode, handler: state.ui, options: state.options, scope: "TitlePhase queued through original phase manager; SelectGenderPhase ended through its source end lifecycle; source TitleUiHandler and option handler rendered; persistence callbacks stubbed", console_errors: errors }, null, 2)}\n`);
	console.log(JSON.stringify({ size: state.size, handler: state.ui, errors: errors.length }));
} finally {
	await browser?.close();
	vite.kill("SIGTERM");
}
