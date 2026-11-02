import fs from "node:fs";
import path from "node:path";
import { uiSource } from "./ui-source.mjs";
import { animSource } from "./anim-source.mjs";
import { captureSource } from "./capture-source.mjs";
import { createRequire } from "node:module";
export const root = path.resolve(import.meta.dirname, "../..");
export function arenaSource() {
	const require = createRequire(path.join(root, "original/package.json"));
	const ts = require("typescript");
	const source = fs
		.readFileSync(path.join(root, "original/src/field/arena-base.ts"), "utf8")
		.replace(/^import .*;\n/gm, "")
		.replace("export class ArenaBase", "class ArenaBase");
	return (
		`const globalScene=window.visualScene; const Phaser=window.Phaser;
 const getBiomeKey=()=> 'town'; const getBiomeHasProps=()=>false;
 const randSeedInt=()=> {throw Error('Unexpected RNG in visual harness');};\n` +
		ts.transpile(source, {
			target: ts.ScriptTarget.ES2022,
			module: ts.ModuleKind.None,
		}) +
		"\nwindow.ArenaBase=ArenaBase;"
	);
}
export function localFile(url) {
	const pathname = new URL(url).pathname;
	if (pathname === "/ui-source.js")
		return { contentType: "text/javascript", body: uiSource() };
	if (pathname === "/anim-source.js")
		return { contentType: "text/javascript", body: animSource() };
	if (pathname === "/capture-source.js")
		return { contentType: "text/javascript", body: captureSource() };
	if (pathname === "/arena-source.js")
		return { contentType: "text/javascript", body: arenaSource() };
	let file = path.resolve(
		root,
		"." +
			decodeURIComponent(
				pathname === "/" ? "/tools/visual/harness.html" : pathname,
			),
	);
	// The pinned asset checkout omits some atlas files from the repository tree.
	// Resolve only those known local source paths from the matching asset cache.
	const sourceImages = path.resolve(root, "original/assets/images");
	if (file.startsWith(sourceImages + path.sep) && !fs.existsSync(file)) {
		const cacheImages = path.resolve(
			root,
			"visual-harness/.cache/assets/fb8ccb6d0703ffe48cdf80a8102d5ed943c82dba/images",
		);
		file = path.resolve(cacheImages, path.relative(sourceImages, file));
	}
	const real = fs.realpathSync(file);
	// Whitelist public test content; never serve saves, secrets or arbitrary workspace files.
	const allowed = [
		"tools/visual/",
		"original/assets/images/",
		"original/assets/battle-anims/",
		"original/assets/fonts/",
		"visual-harness/.cache/assets/fb8ccb6d0703ffe48cdf80a8102d5ed943c82dba/images/",
		"original/node_modules/phaser/dist/",
	];
	const permitted = allowed.find((prefix) =>
		file.startsWith(path.resolve(root, prefix) + path.sep),
	);
	if (
		!permitted ||
		!real.startsWith(fs.realpathSync(path.join(root, permitted)) + path.sep)
	)
		throw Error("Path forbidden");
	const types = {
		".js": "text/javascript",
		".json": "application/json",
		".html": "text/html",
		".png": "image/png",
		".ttf": "font/ttf",
	};
	return {
		contentType: types[path.extname(file)] || "application/octet-stream",
		body: fs.readFileSync(real),
	};
}
