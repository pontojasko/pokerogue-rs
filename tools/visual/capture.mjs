import { chromium } from "playwright";
import fs from "node:fs";
import { createHash } from "node:crypto";
import path from "node:path";
import { localFile, root } from "./local-files.mjs";
const [scene, output] = process.argv.slice(2);
if (!scene || !/^[a-z0-9-]+$/.test(scene) || !output)
	throw Error("Usage: node capture.mjs SCENE OUTPUT_DIR");
const fixture = JSON.parse(
	fs.readFileSync(path.join(root, `tools/visual/scenes/${scene}.json`)),
);
const options = {
	headless: true,
	args: ["--use-angle=swiftshader", "--disable-dev-shm-usage"],
};
// Prefer the user's Helium installation; an explicit path also supports other local Chromium builds.
const browserPath =
	process.env.POKERUST_BROWSER ||
	process.env.POKERUST_CHROMIUM ||
	[
		"/opt/helium-browser-bin/helium",
		"/usr/bin/helium-browser",
		"/usr/bin/helium",
	].find((p) => fs.existsSync(p));
if (browserPath) options.executablePath = browserPath;
const phaserVersion = JSON.parse(
	fs.readFileSync(path.join(root, "original/node_modules/phaser/package.json")),
).version;
if (phaserVersion !== "4.2.1")
	throw Error("Expected pinned Phaser 4.2.1, got " + phaserVersion);
const sourceFiles = [
	"original/src/ui/text.ts",
	"original/src/ui/battle-info/battle-info.ts",
	"original/src/ui/battle-info/player-battle-info.ts",
	"original/src/ui/battle-info/enemy-battle-info.ts",
	"original/src/ui/handlers/fight-ui-handler.ts",
	"original/src/data/battle-anims.ts",
	"original/src/field/arena-base.ts",
	"original/src/field/pokemon.ts",
	"original/src/phases/encounter-phase.ts",
	`original/assets/images/pokemon/back/${fixture.player.species}.json`,
	`original/assets/images/pokemon/${fixture.enemy.species}.json`,
];
if (fixture.scope === "attack") sourceFiles.push("original/assets/battle-anims/tackle.json", "original/assets/images/battle_anims/PRAS- Strike.png");
if (fixture.scope === "capture") sourceFiles.push("original/src/phases/attempt-capture-phase.ts", "original/src/data/pokeball.ts", "original/assets/images/pb.json", "original/assets/images/pb.png");
const sourceHashes = Object.fromEntries(
	sourceFiles.map((file) => [
		file,
		createHash("sha256")
			.update(fs.readFileSync(path.join(root, file)))
			.digest("hex"),
	]),
);
const browser = await chromium.launch(options);
try {
	const page = await browser.newPage({
		viewport: { width: fixture.width, height: fixture.height },
		deviceScaleFactor: 1,
		serviceWorkers: "block",
	});
	const errors = [];
	page.on("pageerror", (e) => errors.push(String(e)));
	// Every request is fulfilled from disk. No listener, official services, or outbound network.
	await page.route("**/*", async (route) => {
		const url = new URL(route.request().url());
		if (url.origin !== "http://127.0.0.1:19876") {
			errors.push(`Blocked external request: ${url.origin}`);
			await route.abort();
			return;
		}
		try {
			await route.fulfill({ status: 200, ...localFile(url) });
		} catch (e) {
			errors.push(`${url.pathname}: ${e.message}`);
			await route.fulfill({ status: 404, body: "Missing local asset" });
		}
	});
	await page.goto(`http://127.0.0.1:19876/?scene=${scene}`);
	await page.waitForFunction(
		() => window.visualReady || window.visualError,
		{},
		{ timeout: 30000 },
	);
	const status = await page.evaluate(() => ({
		ready: window.visualReady,
		error: window.visualError,
	}));
	if (status.error || errors.length)
		throw Error(JSON.stringify({ status, errors }));
	fs.mkdirSync(output, { recursive: true });
	await page
		.locator("canvas")
		.screenshot({
			path: path.join(output, "original.png"),
			animations: "disabled",
		});
	fs.writeFileSync(
		path.join(output, "reference.json"),
		JSON.stringify(
			{
				...status.ready,
				phaser: phaserVersion,
				sourceHashes,
				browser: browser.version(),
				executable: browserPath || chromium.executablePath(),
			},
			null,
			2,
		) + "\n",
	);
} finally {
	await browser.close();
}
