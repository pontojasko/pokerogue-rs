import originalConfig from "../original/vite.config.ts";
import { execFileSync, spawn } from "node:child_process";
import { createReadStream, existsSync, mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const visualFixtureOverrides = `const overrides = {
  FIELD_SIZE_OVERRIDE: "single",
  BYPASS_TUTORIAL_SKIP_OVERRIDE: false,
  SEED_OVERRIDE: "MiniRunSeed0001",
  STARTING_BIOME_OVERRIDE: BiomeId.TOWN,
  TIME_OF_DAY_OVERRIDE: TimeOfDay.DAY,
  STARTING_LEVEL_OVERRIDE: 5,
  STARTER_SPECIES_OVERRIDE: SpeciesId.BULBASAUR,
  MOVESET_OVERRIDE: [MoveId.VINE_WHIP, MoveId.TACKLE],
  ENEMY_SPECIES_OVERRIDE: SpeciesId.CHARMANDER,
  ENEMY_MOVESET_OVERRIDE: [MoveId.TACKLE],
  ENEMY_LEVEL_OVERRIDE: 5,
} satisfies Partial<InstanceType<OverridesType>>;`;

// Replay mode supplies only the deterministic run seed. Starter selection,
// encounter generation, battle construction and phase queue stay source-owned.
const replayFixtureOverrides = `const overrides = {
  SEED_OVERRIDE: ${JSON.stringify(process.env.POKERUST_REPLAY_SEED ?? "M12TownWave1Replay6")},
} satisfies Partial<InstanceType<OverridesType>>;`;

const visualFixturePlugin = {
  name: "pokerogue-visual-fixture",
  enforce: "pre" as const,
  transform(source: string, id: string) {
    const normalizedId = id.replaceAll("\\", "/");
    if (normalizedId.endsWith("/original/src/ai/ai-moveset-gen.ts") && process.env.POKERUST_REPLAY_TRACE === "1") {
      const stabAnchor = "    const selectedId = chosenPool[index][0];";
      const fillAnchor = "    const selectedMoveId = filteredPool[index][0];";
      if (!source.includes(stabAnchor) || !source.includes(fillAnchor)) {
        throw new Error("Pinned moveset selection changed; update replay trace instrumentation");
      }
      return source
        .replace(stabAnchor, `${stabAnchor}\n    (globalThis as any).__pokerustReplayEvents?.push({ kind: "moveset.selection", stage: "stab", species_id: pokemon.species.speciesId, level: pokemon.level, candidates: chosenPool, total_weight: totalWeight.value, rand, selected_id: selectedId });`)
        .replace(fillAnchor, `${fillAnchor}\n    (globalThis as any).__pokerustReplayEvents?.push({ kind: "moveset.selection", stage: "fill", species_id: pokemon.species.speciesId, level: pokemon.level, candidates: filteredPool, total_weight: filteredPool.reduce((total, [, weight]) => total + weight, 0), rand, selected_id: selectedMoveId });`);
    }
    if (normalizedId.endsWith("/original/src/overrides.ts")) {
      const sourceDeclaration =
        "const overrides = {} satisfies Partial<InstanceType<OverridesType>>;";
      if (!source.includes(sourceDeclaration)) {
        throw new Error("Pinned overrides.ts declaration changed; update visual fixture plugin");
      }
      return source.replace(sourceDeclaration, process.env.POKERUST_REPLAY_TRACE === "1" ? replayFixtureOverrides : visualFixtureOverrides);
    }
    if (normalizedId.endsWith("/original/src/phases/select-starter-phase.ts") &&
      process.env.POKERUST_REPLAY_TRACE === "1" && process.env.POKERUST_CAPTURE_STARTER_INPUT === "1") {
      const starterAnchor = "  initBattle(starters: Starter[]) {";
      if (!source.includes(starterAnchor)) {
        throw new Error("Pinned starter initialization changed; update replay input capture");
      }
      return source.replace(
        starterAnchor,
        `${starterAnchor}
    (globalThis as any).__pokerustStarterInput = starters.map(starter => ({
      speciesId: starter.speciesId,
      abilityIndex: starter.abilityIndex,
      formIndex: starter.formIndex,
      female: starter.female,
      shiny: starter.shiny,
      variant: starter.variant,
      ivs: starter.ivs ? [...starter.ivs] : undefined,
      nature: starter.nature,
      moveset: starter.moveset ? [...starter.moveset] : undefined,
      passive: starter.passive,
      pokerus: starter.pokerus,
      nickname: starter.nickname,
      teraType: starter.teraType,
    }));`,
      );
    }
    if (normalizedId.endsWith("/original/src/globals/global-scene.ts")) {
      if (!source.includes("  globalScene = scene;")) {
        throw new Error("Pinned global scene initialization changed; update visual fixture exposure");
      }
      const instrumented = source.replace(
        "  globalScene = scene;",
        `  globalScene = scene;
  (globalThis as any).__pokerogueVisualScene = scene;
  if (import.meta.env.VITE_POKERUST_REPLAY_TRACE === "1") {
    const traceEvents: any[] = [];
    let captureTrace = true;
    const recordTrace = (event: any) => {
      if (captureTrace) traceEvents.push(event);
    };
    (globalThis as any).__pokerustReplayEvents = traceEvents;
    const rng = Phaser.Math.RND as any;
    for (const method of ["frac", "integerInRange", "realInRange", "pick"]) {
      const original = rng[method].bind(rng);
      rng[method] = (...args: any[]) => {
        const value = original(...args);
        const stack = import.meta.env.VITE_POKERUST_TRACE_STACKS === "1"
          ? new Error().stack?.split("\\n").slice(2, 9)
          : undefined;
        recordTrace({ kind: "rng", method, args, value, ...(stack ? { stack } : {}) });
        return value;
      };
    }
    const originalOffset = scene.executeWithSeedOffset.bind(scene);
    (scene as any).executeWithSeedOffset = (callback: () => void, offset: number, seedOverride?: string) => {
      const before = rng.state();
      recordTrace({ kind: "seed-offset.begin", offset, seed_override: seedOverride ?? null, state: before });
      originalOffset(callback, offset, seedOverride);
      recordTrace({ kind: "seed-offset.end", offset, state: rng.state() });
    };
    const originalNewBattle = scene.newBattle.bind(scene);
    (scene as any).newBattle = (...args: any[]) => {
      // Startup/title RNG belongs to the harness bootstrap. The replay boundary
      // begins exactly at newBattle so independent browser startup noise is excluded.
      traceEvents.length = 0;
      const before = { seed: scene.seed, wave_seed: scene.waveSeed, wave_index: scene.currentBattle?.waveIndex ?? 0,
        starter_input: (globalThis as any).__pokerustStarterInput ?? null,
        rng_state: rng.state(), current_phase: scene.phaseManager.getCurrentPhase()?.phaseName ?? null,
        biome: scene.arena?.biomeId ?? null, time_of_day: scene.arena?.getTimeOfDay?.() ?? null };
      recordTrace({ kind: "newBattle.begin", before });
      const battle = originalNewBattle(...args);
      const phaseQueue = (scene.phaseManager as any).phaseQueue;
      recordTrace({ kind: "newBattle.end", battle: {
        seed: battle.battleSeed, wave_index: battle.waveIndex, battle_type: battle.battleType,
        double: battle.double, turn: battle.turn,
      }, rng_state: rng.state(), current_phase: scene.phaseManager.getCurrentPhase()?.phaseName ?? null,
        queued_phases: phaseQueue?.levels?.flat?.().map((phase: any) => phase.phaseName) ?? null });
      if (import.meta.env.VITE_POKERUST_REPLAY_THROUGH_COMMAND !== "1") captureTrace = false;
      return battle;
    };
    const phaseManager = scene.phaseManager as any;
    const snapshotParty = (members: any[]) => members.map(p => ({
      species_id: p.species.speciesId, level: p.level, ivs: p.ivs, nature: p.nature, ability_id: p.getAbility?.()?.id ?? null,
      stats: p.stats, hp: p.hp, max_hp: p.getMaxHp(), moves: p.getMoveset().map((m: any) => ({ id: m.moveId, pp: m.getMovePp() - m.ppUsed, pp_used: m.ppUsed, max_pp: m.getMovePp() })),
    }));
    const phaseEvent = (kind: string, data: any = {}) => recordTrace({ kind, ...data });
    const capturedCommandBoundaries = new WeakSet<object>();
    for (const method of ["pushNew", "unshiftNew", "pushPhase", "unshiftPhase"]) {
      const original = phaseManager[method]?.bind(phaseManager);
      if (!original) continue;
      phaseManager[method] = (...args: any[]) => {
        phaseEvent("phase.enqueue", { operation: method, args: args.map(arg => typeof arg === "string" ? arg : arg?.phaseName ?? null) });
        return original(...args);
      };
    }
    for (const method of ["startCurrentPhase", "shiftPhase"]) {
      const original = phaseManager[method]?.bind(phaseManager);
      if (!original) continue;
      phaseManager[method] = (...args: any[]) => {
        const before = phaseManager.getCurrentPhase?.()?.phaseName ?? null;
        const startingPhase = phaseManager.getCurrentPhase?.();
        const captureCommandBoundary = method === "startCurrentPhase"
          && startingPhase?.phaseName === "CommandPhase"
          && !capturedCommandBoundaries.has(startingPhase);
        if (captureCommandBoundary) capturedCommandBoundaries.add(startingPhase);
        if (method === "startCurrentPhase") {
          const currentPhase = phaseManager.getCurrentPhase?.();
          const moveDetails = before === "MovePhase" && currentPhase ? {
            actor_species_id: currentPhase.pokemon?.species?.speciesId ?? null,
            actor_is_player: currentPhase.pokemon?.isPlayer?.() ?? null,
            actor_is_active: currentPhase.pokemon?.isActive?.() ?? null,
            move_id: currentPhase.move?.moveId ?? null,
            targets: currentPhase.targets ?? null,
          } : {};
          const faintDetails = before === "FaintPhase" && currentPhase ? {
            fainted_species_id: currentPhase.getPokemon?.()?.species?.speciesId ?? null,
            fainted_is_player: currentPhase.getPokemon?.()?.isPlayer?.() ?? null,
            fainted_hp: currentPhase.getPokemon?.()?.hp ?? null,
          } : {};
          const switchDetails = before === "SwitchSummonPhase" && currentPhase ? {
            field_index: currentPhase.fieldIndex ?? null,
            slot_index: currentPhase.slotIndex ?? null,
          } : {};
          phaseEvent("phase.start", { phase: before, queue: phaseManager.phaseQueue?.levels?.flat?.().map((phase: any) => phase.phaseName) ?? null, ...moveDetails, ...faintDetails, ...switchDetails });
          if (currentPhase && !(currentPhase as any).__pokerustEndWrapped) {
            const end = currentPhase.end.bind(currentPhase);
            (currentPhase as any).__pokerustEndWrapped = true;
            currentPhase.end = (...endArgs: any[]) => {
              phaseEvent("phase.end", { phase: currentPhase.phaseName, queue: phaseManager.phaseQueue?.levels?.flat?.().map((phase: any) => phase.phaseName) ?? null });
              return end(...endArgs);
            };
          }
        }
        const result = original(...args);
        const current = phaseManager.getCurrentPhase?.()?.phaseName ?? null;
        if (method === "shiftPhase") phaseEvent("phase.transition", { before, current, queue: phaseManager.phaseQueue?.levels?.flat?.().map((phase: any) => phase.phaseName) ?? null });
        if (captureCommandBoundary && current === "CommandPhase") phaseEvent("phase.command-boundary", { player_party: snapshotParty(scene.getPlayerParty()), enemy_party: snapshotParty(scene.getEnemyParty()), queue: phaseManager.phaseQueue?.levels?.flat?.().map((phase: any) => phase.phaseName) ?? null });
        return result;
      };
    }
  }`,
      );
      return process.env.POKERUST_REPLAY_TRACE === "1" ? `import Phaser from "phaser";\n${instrumented}` : instrumented;
    }
    if (normalizedId.endsWith("/original/src/tutorial.ts")) {
      const handler = "export async function handleTutorial(tutorial: Tutorial): Promise<boolean> {";
      if (!source.includes(handler)) {
        throw new Error("Pinned tutorial handler changed; update the isolated visual bootstrap");
      }
      return source.replace(
        handler,
        `${handler}\n  if ((globalThis as any).__pokerogueVisualSuppressTutorials) return false;`,
      );
    }
    if (normalizedId.endsWith("/original/src/loading-scene.ts")) {
      const fullIconPreload = `  private loadPokemonIcons(): this {
    for (let i = 0; i < 10; i++) {
      this.loadAtlas(\`pokemon_icons_\${i}\`, "");
      if (i) {
        this.loadAtlas(\`pokemon_icons_\${i}v\`, "");
      }
    }

    return this;
  }`;
      const firstGenerationOnly = `  private loadPokemonIcons(): this {
    this.loadAtlas("pokemon_icons_0", "");
    return this;
  }`;
      if (!source.includes(fullIconPreload)) {
        throw new Error("Pinned LoadingScene icon preload changed; update the visual harness allowlist");
      }
      return source.replace(fullIconPreload, firstGenerationOnly);
    }
    return null;
  },
  configureServer(server: {
    middlewares: { use: (middleware: (request: any, response: any, next: () => void) => void) => void };
  }) {
    const assetRoot = fileURLToPath(new URL("../original/assets/", import.meta.url));
    const originalRoot = fileURLToPath(new URL("../original/", import.meta.url));
    const localAssetCache = fileURLToPath(new URL("./.cache/assets/fb8ccb6d0703ffe48cdf80a8102d5ed943c82dba/", import.meta.url));
    const assetRevision = execFileSync("git", ["-C", assetRoot, "rev-parse", "HEAD"], {
      encoding: "utf8",
    }).trim();
    if (assetRevision !== "fb8ccb6d0703ffe48cdf80a8102d5ed943c82dba") {
      throw new Error(`Visual fixture expects the pinned asset revision, got ${assetRevision}`);
    }
    const cache = new Map<string, Buffer>();
    let cacheBytes = 0;
    const inFlight = new Map<string, Promise<Buffer | undefined>>();
    const objectReader = (repository: string) => {
      const git = spawn("git", ["-C", repository, "cat-file", "--batch"], {
        stdio: ["pipe", "pipe", "ignore"],
      });
      const waiting: Array<(content: Buffer | undefined) => void> = [];
      let output = Buffer.alloc(0);
      git.stdout.on("data", (chunk: Buffer) => {
        output = Buffer.concat([output, chunk]);
        while (waiting.length > 0) {
          const headerEnd = output.indexOf(10);
          if (headerEnd < 0) return;
          const header = output.subarray(0, headerEnd).toString("utf8");
          const fields = header.split(" ");
          if (header.endsWith(" missing") || fields.length !== 3) {
            output = output.subarray(headerEnd + 1);
            waiting.shift()?.(undefined);
            continue;
          }
          const size = Number(fields[2]);
          const bodyStart = headerEnd + 1;
          const bodyEnd = bodyStart + size;
          if (!Number.isSafeInteger(size) || size < 0 || output.length < bodyEnd + 1) return;
          const content = Buffer.from(output.subarray(bodyStart, bodyEnd));
          output = output.subarray(bodyEnd + 1);
          waiting.shift()?.(content);
        }
      });
      git.on("error", () => {
        while (waiting.length) waiting.shift()?.(undefined);
      });
      return (assetPath: string) => new Promise<Buffer | undefined>(resolve => {
        waiting.push(resolve);
        git.stdin.write(`HEAD:${assetPath}\n`);
      });
    };
    const readAsset = objectReader(assetRoot);
    const readOriginal = objectReader(originalRoot);
    const contentTypes: Record<string, string> = {
      ".json": "application/json",
      ".png": "image/png",
      ".jpg": "image/jpeg",
      ".jpeg": "image/jpeg",
      ".webp": "image/webp",
      ".ttf": "font/ttf",
      ".woff": "font/woff",
      ".woff2": "font/woff2",
      ".m4a": "audio/mp4",
      ".wav": "audio/wav",
      ".mp3": "audio/mpeg",
      ".mp4": "video/mp4",
    };
    server.middlewares.use((request, response, next) => {
      const requestPath = decodeURIComponent((request.url ?? "").split("?", 1)[0]);
      // The game also loads animation manifests and experience sprites from
      // the asset repository root. Keep the allowlist local and explicit;
      // otherwise Phaser receives an empty JSON body and aborts BattleScene.
      if (!/^\/(?:fonts\/|images\/|audio\/|battle-anims\/|(?:starter-colors|biome-bgm-loop-points|exp-sprites)\.json$|manifest\.webmanifest$)/.test(requestPath)) {
        next();
        return;
      }
      const assetPath = requestPath.slice(1).replaceAll(/\/{2,}/g, "/");
      if (assetPath.split("/").some(segment => segment === "..")) {
        response.statusCode = 400;
        response.end("Bad asset path");
        return;
      }
      if (existsSync(path.join(assetRoot, assetPath))) {
        next();
        return;
      }
      const cachedPath = path.join(localAssetCache, assetPath);
      if (existsSync(cachedPath)) {
        response.setHeader("Content-Type", contentTypes[path.extname(assetPath)] ?? "application/octet-stream");
        createReadStream(cachedPath).pipe(response);
        return;
      }
      const cached = cache.get(assetPath);
      const load = cached
        ? Promise.resolve(cached)
        : inFlight.get(assetPath) ?? readAsset(assetPath).then(asset => asset ?? readOriginal(assetPath));
      inFlight.set(assetPath, load);
      void load.then(content => {
        inFlight.delete(assetPath);
        if (!content) {
          next();
          return;
        }
        if (content.byteLength <= 8 * 1024 * 1024 && cacheBytes + content.byteLength <= 64 * 1024 * 1024) {
          cache.set(assetPath, content);
          cacheBytes += content.byteLength;
        }
        mkdirSync(path.dirname(cachedPath), { recursive: true });
        writeFileSync(cachedPath, content);
        response.setHeader("Content-Type", contentTypes[path.extname(assetPath)] ?? "application/octet-stream");
        response.setHeader("Content-Length", content.byteLength);
        response.end(content);
      }).catch(next);
    });
  },
};

export default async (config: Parameters<NonNullable<typeof originalConfig>>[0]) => {
  const original = await (originalConfig as (input: typeof config) => Promise<Record<string, unknown>>)(config);
  return {
    ...original,
    plugins: [...((original.plugins as unknown[]) ?? []), visualFixturePlugin],
    // The pinned game supports this explicit local-test mode. It avoids all
    // account/server flows; Playwright separately blocks any external origin.
    define: {
      ...((original.define as Record<string, unknown> | undefined) ?? {}),
      "import.meta.env.VITE_BYPASS_LOGIN": JSON.stringify("1"),
      "import.meta.env.VITE_BYPASS_TUTORIAL": JSON.stringify("1"),
      "import.meta.env.VITE_POKERUST_REPLAY_TRACE": JSON.stringify(process.env.POKERUST_REPLAY_TRACE === "1" ? "1" : "0"),
      "import.meta.env.VITE_POKERUST_REPLAY_THROUGH_COMMAND": JSON.stringify(process.env.POKERUST_REPLAY_THROUGH_COMMAND === "1" ? "1" : "0"),
      "import.meta.env.VITE_POKERUST_TRACE_STACKS": JSON.stringify(process.env.POKERUST_TRACE_STACKS === "1" ? "1" : "0"),
    },
    server: { ...(original.server as object), host: "127.0.0.1", port: Number(process.env.POKERUST_REPLAY_PORT ?? 8123), strictPort: true },
  };
};
