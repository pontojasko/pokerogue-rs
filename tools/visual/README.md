# Offline visual parity and regression tooling

The capture/compare loop runs locally with **Helium + Phaser 4.2.1** and the real **Macroquad framebuffer**. Native capture prefers a private Xvfb server, so no desktop focus, window positioning, keyboard automation or manual screenshots are needed. All assets and captured images remain local.

## Run

```sh
# Restore the selected HUD resources from the pinned local asset Git objects,
# install cached development dependencies, optionally prepare isolated Xvfb:
python3 tools/visual/bootstrap.py --xvfb

# Capture both renderers and generate images, metrics and an HTML report:
python3 scripts/visual_parity.py --scene battle-town

# Capture every declared case and compare with existing local Rust baselines:
python3 scripts/visual_suite.py
```

Prerequisites: Node, the original TypeScript dependencies, ImageMagick (`magick`), the pinned original/asset checkouts, a local Helium/Chromium executable, and the Rust toolchain. Bootstrap uses the local npm cache; on a fresh machine populate dependencies with `npm ci --prefix tools/visual --ignore-scripts` first. Its optional checksum-pinned Linux x86_64 Xvfb download is setup-only; Xorg libraries and `bsdtar` are needed. It extracts the binary into ignored `runtime/` without system installation or desktop changes. A system `Xvfb` also works. Neither game requires Node/network in normal runtime.

The runner detects `/usr/bin/helium-browser` and `/opt/helium-browser-bin/helium`; override with `POKERUST_BROWSER=/absolute/path`. Playwright uses a temporary browser profile. It intercepts every page HTTP request and fulfills it from local disk; no official game server is contacted and no listener is opened. `node tools/visual/server.mjs 8080` is an optional loopback preview, independent of the automatic path. The full-game capture defaults to ANGLE SwiftShader; `POKERUST_BROWSER_GL=egl` selects EGL for renderer diagnostics. EGL is experimental here and can prevent the scripted onboarding from reaching battle. `POKERUST_CAPTURE_TIME_SCALE` can accelerate only the original opening encounter animations before the settled screenshot; the harness restores Phaser clocks to 1 before capture. It does not change turns or RNG.

Native backend `auto` prefers system/local Xvfb, falling back to the inherited display. `--native-backend xvfb` requires isolated execution; `--native-backend display --display :0` explicitly uses a display. Xvfb starts on a free display with TCP disabled and software GL, and is terminated on completion/failure. Macroquad internally calls `get_screen_data().export_png()`, then exits. No personal browser/MCP/compositor configuration is modified.

## Controlled states and source evidence

Both renderers read the same `scenes/*.json`: output resolution 960x540, species, levels, HP, phase and exact cosmetic time. Rust constructs disposable battlers and freezes only presentation time; it never dispatches gameplay actions, advances battle RNG or touches saves. Normal gameplay retains real time.

- `battle-town`: settled Town DAY field, frame zero.
- `battle-town-frame-3`: same field at 200 ms (source 10fps atlas sample).
- `battle-town-mid-entry`: first-entry transforms at 1000 ms.
- `battle-town-wave1-entry.json`: local client fixture built from a real `MiniRun` wave-1 core state at 1000 ms; `show_entry_message` selects the opening core log for comparison with the existing original message-window reference. It currently documents one modeled enemy and is not a source/Rust screenshot parity case.
- `battle-town-hud`: source battle-info constructors, names, levels and full HP.
- `battle-town-hud-damaged`: medium/low HP frames, partial fills and numeric values.
- `battle-town-ui`: shared command/message screen fixture. For direct Phaser comparison use `python3 scripts/visual_parity.py --scene battle-town-ui --reference-backend full-game`.
- `battle-town-moves`: shared Vine Whip/Tackle/Growl fixture; the full-game backend opens the original Fight screen through its real command and fight handlers. Use `python3 scripts/visual_parity.py --scene battle-town-moves --reference-backend full-game` for a real Phaser screenshot.
- `battle-town-reward`: deterministic Rust reward menu with Soothe Bell, PP Up and TM091 Sunny Day. Its layout was reviewed against the user-supplied original screenshot at 960×540; the original `SelectModifierPhase` was not executed for this screenshot, so it is a presentation fixture rather than phase parity evidence.
- `battle-town-attack`: shared Town/HUD state, Tackle move, player actor and 100 ms timestamp. The source harness executes the pinned `BattleAnim.getGraphicFrameData` projector against the original `tackle.json`, uses the source 50 ms animation tick and PRAS Strike spritesheet. Rust samples the same source frame/timing through its actual playback renderer using a synthetic presentation event; no turn, damage, RNG, or saves are changed. Full `MovePhase`/`MoveEffectPhase` execution and the surrounding battle UI are not part of this fixture. Its 2.5% image gate includes the already known HUD/rasterization differences.
- `battle-town-capture`: same Town/HUD state, Poké Ball and 250 ms throw time. The source harness executes the original `AttemptCapturePhase.start` with deterministic Pokemon/capture collaborators, captures the exact Phaser tween configuration, and evaluates that tween with Phaser's Cubic ease-out. It stops before delayed callbacks, shake checks or capture resolution. Rust supplies a disposable capture result to its normal capture renderer. The current whole-image difference is 0.91%, including known HUD differences; it does not certify capture logic or the later capture stages.
- `title-screen`: `node tools/visual/capture-original-title.mjs` boots the pinned local app and captures the real `TitleUiHandler`/`BaseOptionSelectUiHandler` render path at 960×540. The retained reference currently includes a clipped first-run gender prompt and Chromium software-renderer background striping; it is not a clean title-only baseline. Pair `original.png` with the Macroquad client screenshot; Rust offers only implemented local mini-run/resume actions. Pixel-difference artifacts are local diagnostics; see `PARITY_PLAN.md` for measured gaps and excluded source features.

`source-harness` remains the default for bounded source-method tests. It runs real Phaser, source BattleInfo type-icon constructors and `setTypes`, and source phase/move projectors with controlled collaborators. To compare complete UI screens, `--reference-backend full-game` boots the pinned local Phaser application in Helium, runs its real `BattleScene`, `EncounterPhase`, `CommandPhase`, `BattleInfo`, `CommandUiHandler` and `FightUiHandler`, then captures the canvas at 960x540. The isolated browser skips first-run onboarding/tutorial/starter selection and injects the deterministic starter through the original phase API; persistence is memory-only. Immediately before capture it pauses the player/enemy idle animations at the original `0001.png` atlas frame and records the source Pokémon coordinates, so the Rust fixture can render the same pose. These harness-only overrides do not touch the checked-out source or Rust runtime.

`visual-harness/vite.config.mts` and `tools/visual/capture-original-game.mjs` provide the isolated full-game launch and deterministic UI driver. This is local-only and uses the pinned checkout/assets; no official server is contacted. The full-game backend presently captures command and move selection only. A fresh Helium/SwiftShader run on 2026-11-02 reached the real `CommandPhase` and `FightUiHandler`, but produced vertical renderer bands in the Phaser canvas; its pixel difference is therefore diagnostic only. The clean archived Phaser capture remains the basis of the prior cross-run 3.2351% command comparison, marked `external-unverified` in its compare-only report. Attack, capture, rewards and transitions still use bounded source-method fixtures until their real Phaser phases are driven.

Real captures revealed and corrected native sprite subpixel alignment, the three-color HP-atlas error, missing level/HP digits/labels, font placement, command prompt line spacing/cursor, source type/category atlas frames, gender symbols, the selected-move effectiveness window, source idle-frame selection and Phaser's one-code white blend on scene-art textures. Archived clean captures at zero tolerance differ by 9,487/518,400 command pixels (1.8301%; MAE 1.2200) and 14,766/518,400 moves pixels (2.8484%; MAE 1.7715). A fresh cross-run command comparison on 2026-11-02 measured 16,771 pixels (3.2351%; MAE 2.0482) against an archived clean Phaser frame; its report is `external-unverified`. A fresh SwiftShader capture reached the original command and move handlers but introduced vertical bands, so it is not a reliable pixel baseline. The command uses the official 6×10 cursor asset at source positions; move UI applies original type-hint colors, center/right anchors, localized label capitalization and detail font size. Enemy HP bar half-pixel projection follows Phaser capture sampling. The money header uses the original `₽` glyph and `TextStyle.MONEY` foreground/shadow colors. The Town label's glyph scale is adjusted against the measured Phaser full-game capture. Remaining errors include text rasterization and HUD/art edges; neither screen has pixel parity. See per-run `report.json` files for raw metrics and `suite.json` for each case's actual reference backend and scope. Thresholds do not mask pixels: complete original/Rust screenshots and raw RGBA errors are retained.

The extra type/gender atlases are loaded from the pinned local asset cache when the source checkout omits them. Their bytes are not copied into the repository or committed. The Rust battle core does not model gender; full-game fixtures read gender from the actual Phaser Pokémon and pass it only as presentation metadata.

## Artifacts and gates

Single-case output: ignored `visual-artifacts/<scene>/` with `original.png`, `rust.png`, `side-by-side.png`, `diff.png`, `reference.json`, `report.json`, `report.html`, and `capture.log`. Reports record source pins/hashes, fixture/capture hashes, browser executable/version, native backend, reference limitations, differing-pixel count/ratio, RGBA MAE/RMSE, maximum error and bounding box. `--tolerance` changes pixel classification only; raw error remains reported. Failed capture removes prior images and writes an error report.

Single-case comparison exits 0 when measured; `--max-parity-ratio` explicitly gates the original/Rust difference. `--baseline /path/to/reviewed/rust.png` separately gates Rust regression (zero differing pixels by default); `--max-changed-ratio` changes that regression budget. Failed gates exit 1, dependency/capture/geometry errors exit 2. `--compare-only` compares existing images and marks externally supplied provenance unverified.

The suite writes `visual-artifacts/suite/index.html` and `summary.json`. The command and move-selection cases use the full local Phaser app; their large original/Rust image differences are diagnostic and are not hidden behind a loose parity gate. Rust regression is measured against separate reviewed captures. The default regression command requires existing baselines and verifies their fixture/image hashes. It never silently accepts current output as the baseline. After inspecting actual captures, an explicit invocation can snapshot them into a **new** local baseline directory:

```sh
python3 scripts/visual_suite.py --record-baselines --baseline-dir visual-artifacts/reviewed-baselines-v2
python3 scripts/visual_suite.py --baseline-dir visual-artifacts/reviewed-baselines-v2
```

Recording refuses to overwrite existing baseline directories. Output and baseline roots must not overlap. The default local baseline directory is ignored `visual-artifacts/reviewed-baselines-v2`; earlier baseline sets are retained as historical data because their fixture provenance predates the expanded Town UI. Preserve metadata/review reports with baselines; browser/GPU changes may intentionally require another explicit review. A one-pixel altered copy of a real capture was rejected with exit 1 without changing baselines. Baseline success does not certify original-game parity.

## Tests and next increments

```sh
python3 -m unittest discover -s scripts -p test_visual_parity.py
node tools/visual/harness.test.mjs
python3 scripts/visual_suite.py --native-backend xvfb
```

Comparator contract tests include alpha differences, dimensions, missing captures and failing regression gates. Node tests execute original ArenaBase with controlled collaborators, compile the original HUD bundle, and check local-file routing boundaries. The visual suite supplies actual rendered evidence in addition to those contract tests.

Next: drive source attack, capture, reward and transition phases through the full Phaser runtime. Continue correcting command/move UI from full-game captures. Do not create an approximate reference solely to reduce metrics. Complete PhaseManager/gameplay parity remains separate.

Raw screenshots, baselines, asset bytes and the downloaded runtime contain restricted imagery: **keep them local and do not upload or distribute them**. The repository README has one separately documented, owner-requested gameplay reference screenshot; see `ASSET_CREDITS.md`. All harness captures and baseline directories remain ignored/local outputs.
