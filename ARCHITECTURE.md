# Architecture map

Current working-tree map reviewed 2026-11-01. [PORTING_PROGRESS.md](PORTING_PROGRESS.md) owns current validation/status; [ROADMAP.md](ROADMAP.md) owns order; [PARITY_PLAN.md](PARITY_PLAN.md) owns source evidence and gaps. The previous map is preserved in [ARCHITECTURE_HISTORY.md](ARCHITECTURE_HISTORY.md).

## Ownership

- `pokerogue-data`: generated species, moves, learnsets, pools and type chart; deterministic importers under `scripts/`. Imported records do not imply implemented move effects.
- `pokerogue-core/src/lib.rs`: Battler/Team, Battle and BattleRng, encounters, bounded combat/capture, MiniRun progression and checkpoint serialization. `rewards.rs` owns modeled reward rules. Core depends on data, not Macroquad.
- `pokerogue-client/src/main.rs`: local app loop, input, asset loading and presentation. `battle_ui.rs`/`hud.rs` own battle navigation and source-atlas HUD rendering; `visual.rs` owns deterministic presentation fixture loading/export; `audio.rs` owns optional local audio and cue timing. Gameplay decisions remain in core.
- `pokerogue-tests/src/lib.rs`: integration and differential fixture assertions. `cargo parity` aliases `cargo test -p pokerogue-tests`; the package also contains Rust-only route tests.

## Reference and simulation boundary

`original/` is pinned to `7c0881255af5cd51992e513ff93c2f71adbe40e0`, with Phaser 4.2.1 RNG. Reference ownership includes `src/field/pokemon.ts` (stats/damage), `src/data/moves/move.ts`, `src/phases/`, `src/battle.ts` and shared seeded helpers. Rust ports the Alea stream and battle/turn seed behavior for the covered slices; fixtures execute selected original methods with controlled collaborators. Normal full construction and PhaseManager replay are still missing.

The current simulation supports Town mini-runs with a party of up to six, four move slots, PP, stages, bounded AI/damage/accuracy/capture, EXP, move learning and selected rewards. Critical damage still has a fixed multiplier in the modeled damage path; complete abilities, weather, status, modifiers and original phase ordering are not implemented. Consult the parity ledger for individual exceptions and fixture scope.

## Persistence and runtime

Persistence belongs to core: `MiniRun::checkpoint` writes v27 and `from_checkpoint` reads v4–v27. Checkpoints are allowed at MoveLearning, Reward and Complete, not mid-battle. The client performs local filesystem storage under the XDG data directory or HOME fallback. Runtime needs no Node, server or network.

Macroquad loads local pinned assets, including Town, Pokémon/HUD atlases, fonts and selected audio. The default path is project-relative in the build configuration, with `POKEROGUE_ASSETS_DIR` override; source assets are not bundled. Unresolved rights are in `ASSET_CREDITS.md`. R36S remains future-only; no userspace bitness or renderer compatibility is assumed.

## Development verification

`tools/visual/` and `scripts/visual_parity.py` capture paired 960×540 source/native scenes, with a frozen presentation clock and isolated Xvfb support. `scripts/visual_suite.py` reads nine cases from `tools/visual/suite.json`. Command/moves use the full-game backend; other cases use bounded source execution. Full-game screenshots do not establish battle-rule parity. Explicit reviewed Rust baselines gate regressions separately from original pixel differences.

The next verification layer is a deterministic source/Rust replay with first-divergence reporting. Begin with normal entry construction and its RNG/state/initial queue before attempting action and victory phases; this is planned infrastructure, not an existing guarantee.
