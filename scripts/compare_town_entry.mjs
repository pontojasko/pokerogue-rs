#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { compareTownEntry } from "../tools/visual/replay-compare.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const sourcePath = path.resolve(process.argv[2] ?? path.join(root, "pokerogue-tests/fixtures/town-wave1-command-trace.json"));
const source = JSON.parse(fs.readFileSync(sourcePath, "utf8"));
if (source.schema !== "pokerust-source-replay/v1" || source.provenance?.source_revision !== "7c0881255af5cd51992e513ff93c2f71adbe40e0") {
	console.error("Source trace schema or pinned provenance mismatch");
	process.exit(2);
}
const run = spawnSync("cargo", ["run", "--offline", "-q", "-p", "pokerogue-tests", "--bin", "replay_town_entry", "--",
	source.scenario.seed, source.scenario.time_of_day, String(source.scenario.wave)], { cwd: root, encoding: "utf8" });
if (run.status !== 0) {
	console.error(run.stderr || run.stdout || "Rust entry adapter failed");
	process.exit(run.status ?? 1);
}
const rust = JSON.parse(run.stdout.trim().split(/\r?\n/).at(-1));
const report = compareTownEntry(source, rust);
process.stdout.write(`${JSON.stringify({ scenario: source.scenario, report }, null, 2)}\n`);
if (!report.parity_pass) process.exitCode = 1;
