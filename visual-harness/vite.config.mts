import originalConfig from "../original/vite.config.ts";
import { execFileSync, spawn } from "node:child_process";
import { createReadStream, existsSync, mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const fixtureOverrides = `const overrides = {
  FIELD_SIZE_OVERRIDE: "single",
  BYPASS_TUTORIAL_SKIP_OVERRIDE: false,
  SEED_OVERRIDE: "MiniRunSeed0001",
  STARTING_BIOME_OVERRIDE: BiomeId.TOWN,
  TIME_OF_DAY_OVERRIDE: TimeOfDay.DAY,
  STARTING_LEVEL_OVERRIDE: 5,
  STARTER_SPECIES_OVERRIDE: SpeciesId.BULBASAUR,
  MOVESET_OVERRIDE: [MoveId.VINE_WHIP, MoveId.TACKLE],
  ENEMY_SPECIES_OVERRIDE: SpeciesId.MINCCINO,
  ENEMY_LEVEL_OVERRIDE: 2,
} satisfies Partial<InstanceType<OverridesType>>;`;

const visualFixturePlugin = {
  name: "pokerogue-visual-fixture",
  enforce: "pre" as const,
  transform(source: string, id: string) {
    const normalizedId = id.replaceAll("\\", "/");
    if (normalizedId.endsWith("/original/src/overrides.ts")) {
      const sourceDeclaration =
        "const overrides = {} satisfies Partial<InstanceType<OverridesType>>;";
      if (!source.includes(sourceDeclaration)) {
        throw new Error("Pinned overrides.ts declaration changed; update visual fixture plugin");
      }
      return source.replace(sourceDeclaration, fixtureOverrides);
    }
    if (normalizedId.endsWith("/original/src/globals/global-scene.ts")) {
      return source.replace(
        "  globalScene = scene;",
        "  globalScene = scene;\n  (globalThis as any).__pokerogueVisualScene = scene;",
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
    },
    server: { ...(original.server as object), host: "127.0.0.1", port: 8123, strictPort: true },
  };
};
