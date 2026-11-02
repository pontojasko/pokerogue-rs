#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { compareTownAction, compareTownEntry } from "../tools/visual/replay-compare.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const sourcePath = path.resolve(process.argv[2] ?? path.join(root, "pokerogue-tests/fixtures/town-wave1-action-trace.json"));
const source = JSON.parse(fs.readFileSync(sourcePath, "utf8"));
if (source.scenario?.action !== "first-move") {
	console.error("Expected the pinned first-move source fixture");
	process.exit(2);
}
const run = spawnSync("cargo", ["run", "--offline", "-q", "-p", "pokerogue-tests", "--bin", "replay_town_entry", "--",
	source.scenario.seed, source.scenario.time_of_day, String(source.scenario.wave), source.scenario.action], { cwd: root, encoding: "utf8" });
if (run.status !== 0) {
	console.error(run.stderr || run.stdout || "Rust replay adapter failed");
	process.exit(run.status ?? 1);
}
const rust = JSON.parse(run.stdout.trim().split(/\r?\n/).at(-1));
const report = {
	scenario: source.scenario,
	entry: compareTownEntry(source, rust),
	action: compareTownAction(source, rust),
};
process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
if (!report.action.parity_pass) process.exitCode = 1;
