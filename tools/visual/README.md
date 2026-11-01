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

The runner prefers `/opt/helium-browser-bin/helium`; override with `POKERUST_BROWSER=/absolute/path`. Playwright uses a temporary browser profile. It intercepts every page HTTP request and fulfills it from local disk; no official game server is contacted and no listener is opened. `node tools/visual/server.mjs 8080` is an optional loopback preview, independent of the automatic path.

Native backend `auto` prefers system/local Xvfb, falling back to the inherited display. `--native-backend xvfb` requires isolated execution; `--native-backend display --display :0` explicitly uses a display. Xvfb starts on a free display with TCP disabled and software GL, and is terminated on completion/failure. Macroquad internally calls `get_screen_data().export_png()`, then exits. No personal browser/MCP/compositor configuration is modified.

## Controlled states and source evidence

Both renderers read the same `scenes/*.json`: output resolution 960x540, species, levels, HP, phase and exact cosmetic time. Rust constructs disposable battlers and freezes only presentation time; it never dispatches gameplay actions, advances battle RNG or touches saves. Normal gameplay retains real time.

- `battle-town`: settled Town DAY field, frame zero.
- `battle-town-frame-3`: same field at 200 ms (source 10fps atlas sample).
- `battle-town-mid-entry`: first-entry transforms at 1000 ms.
- `battle-town-hud`: source battle-info constructors, names, levels and full HP.
- `battle-town-hud-damaged`: medium/low HP frames, partial fills and numeric values.
- `battle-town-ui`: diagnostic full Rust interface; original command/message and move-detail/cursor UI remain unimplemented in the harness.
- `battle-town-moves`: shared Vine Whip/Tackle fixture. The source harness executes the pinned `FightUiHandler.displayMoves`, with the original 320x180 UI transform and nine-slice windows; Rust reads the same move list and renders the source menu positions/font. The current original-vs-Rust gate is 2% and measures the whole captured battle view, including known HUD/font/panel differences. It does not certify the rest of the battle UI.
- `battle-town-attack`: shared Town/HUD state, Tackle move, player actor and 100 ms timestamp. The source harness executes the pinned `BattleAnim.getGraphicFrameData` projector against the original `tackle.json`, uses the source 50 ms animation tick and PRAS Strike spritesheet. Rust samples the same source frame/timing through its actual playback renderer using a synthetic presentation event; no turn, damage, RNG, or saves are changed. Full `MovePhase`/`MoveEffectPhase` execution and the surrounding battle UI are not part of this fixture. Its 2.5% image gate includes the already known HUD/rasterization differences.
- `battle-town-capture`: same Town/HUD state, Poké Ball and 250 ms throw time. The source harness executes the original `AttemptCapturePhase.start` with deterministic Pokemon/capture collaborators, captures the exact Phaser tween configuration, and evaluates that tween with Phaser's Cubic ease-out. It stops before delayed callbacks, shake checks or capture resolution. Rust supplies a disposable capture result to its normal capture renderer. The current whole-image difference is 0.91%, including known HUD differences; it does not certify capture logic or the later capture stages.

Reference mode is explicitly **source-harness**, not the full original application. It runs real Phaser, transpiles/executes the original `ArenaBase` constructor/setBiome and the original `BattleInfo`, `PlayerBattleInfo`, `EnemyBattleInfo` constructors/name/level/HP-number methods. For the menu and attack fixtures it also compiles the pinned `FightUiHandler.displayMoves` and `BattleAnim.getGraphicFrameData` methods. Original text style/color functions and original atlas metadata are used. Hidden interactive identity/type/stat/tooltip collaborators are stubbed and disclosed. Pokemon sprites use source-derived constructor anchors (106,148)/(236,84), origin (.5,1), 3x scale and 10fps sampling. Full BattleScene/Pokemon construction, PhaseManager, complete command/message/fight phases, MovePhase/MoveEffectPhase, capture/reward execution, special shaders, audio and official services are not bootstrapped.

`visual-harness/vite.config.mts` is an exploratory launch of the pinned full Phaser app. Its Vite-only overrides and Git-object asset fallback stay outside the original source and normal Rust runtime. The offline boot (login/tutorial bypass, external Playwright origins blocked) was verified only through initial gender selection; that path did not produce a battle screenshot. The bounded source harness above remains the validated capture workflow.

Real captures revealed and corrected native sprite subpixel alignment, the three-color HP-atlas error, missing level/HP digits/labels and font placement. Current bounded results: field samples differ by 12–72 pixels (0.0023–0.0139%); HUD samples differ by 3711–3959 pixels (0.716–0.764%). Remaining font/rasterization/panel-edge differences are visible; no complete visual parity is claimed. Full Rust UI differs substantially because its source menu reference is still missing, and has no original-parity gate. See `suite.json` for declared gates and diagnostic scope. Thresholds do not mask pixels: complete original/Rust screenshots and raw RGBA errors are retained.

## Artifacts and gates

Single-case output: ignored `visual-artifacts/<scene>/` with `original.png`, `rust.png`, `side-by-side.png`, `diff.png`, `reference.json`, `report.json`, `report.html`, and `capture.log`. Reports record source pins/hashes, fixture/capture hashes, browser executable/version, native backend, reference limitations, differing-pixel count/ratio, RGBA MAE/RMSE, maximum error and bounding box. `--tolerance` changes pixel classification only; raw error remains reported. Failed capture removes prior images and writes an error report.

Single-case comparison exits 0 when measured; `--max-parity-ratio` explicitly gates the original/Rust difference. `--baseline /path/to/reviewed/rust.png` separately gates Rust regression (zero differing pixels by default); `--max-changed-ratio` changes that regression budget. Failed gates exit 1, dependency/capture/geometry errors exit 2. `--compare-only` compares existing images and marks externally supplied provenance unverified.

The suite writes `visual-artifacts/suite/index.html` and `summary.json`. Its default regression command requires existing baselines and verifies their fixture/image hashes. It never silently accepts current output as the baseline. After inspecting actual captures, an explicit invocation can snapshot them into a **new** local baseline directory:

```sh
python3 scripts/visual_suite.py --record-baselines --baseline-dir visual-artifacts/reviewed-baselines
python3 scripts/visual_suite.py --baseline-dir visual-artifacts/reviewed-baselines
```

Recording refuses to overwrite existing baseline directories. Output and baseline roots must not overlap. Preserve metadata/review reports with baselines; browser/GPU changes may intentionally require another explicit review. Nine cases have reviewed local Rust baselines; the original six field/HUD cases passed with zero Rust pixels changed on repeat, and move/attack/capture scenes have scene-filtered regression checks. A one-pixel altered copy of a real capture was rejected with exit 1 without changing baselines. Baseline success does not certify original-game parity.

## Tests and next increments

```sh
python3 -m unittest discover -s scripts -p test_visual_parity.py
node tools/visual/harness.test.mjs
python3 scripts/visual_suite.py --native-backend xvfb
```

Comparator contract tests include alpha differences, dimensions, missing captures and failing regression gates. Node tests execute original ArenaBase with controlled collaborators, compile the original HUD bundle, and check local-file routing boundaries. The visual suite supplies actual rendered evidence in addition to those contract tests.

Next: compare one deterministic capture-animation frame using source-executed Phaser capture methods and the frozen Macroquad capture pose; then extend source command/attack, rewards and transitions. Do not create an approximate reference solely to reduce metrics. Complete PhaseManager/gameplay parity remains separate.

Screenshots and baselines contain restricted imagery: **never commit, upload or distribute them**, asset bytes or the downloaded runtime. See `ASSET_CREDITS.md`. All remain in ignored/local directories.
