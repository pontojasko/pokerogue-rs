<div align="center">
  <picture>
    <img src="docs/logo.png" width="420" alt="PokéRogue Rust Edition">
  </picture>

  <p>
    <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-edition_2024-orange?style=flat&logo=rust&logoColor=white" alt="Rust 2024"></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/License-AGPL--3.0--only-blue.svg" alt="AGPL-3.0-only license"></a>
    <a href="tools/visual/README.md"><img src="https://img.shields.io/badge/Visual_scenes-9-22c55e?style=flat" alt="Nine declared visual scenes"></a>
  </p>
</div>

PokéRogue Rust is an experimental **reimplementation in Rust** of selected gameplay and presentation slices from PokéRogue. It is an offline-first Rust workspace with a Macroquad client and a local TypeScript reference checkout.

<p align="center">
  <img src="docs/roadmap-progress.svg" alt="Roadmap checklist progress" width="100%">
</p>

> **Progress note:** The chart counts roadmap checkboxes equally. It is not a measure of the percentage of the original game that has been ported. The work items vary in size, and substantial gameplay and visual gaps remain.

## What is implemented

- A Rust workspace split into battle rules, imported data, a Macroquad client, and differential tests.
- An offline Town mini-run with bounded battle, EXP, capture, reward, and save behavior.
- Source-backed fixtures for selected RNG, battle, and progression behavior.
- A local visual comparison harness for Helium/Playwright, Phaser, and the Macroquad framebuffer.
- Nine declared 960×540 visual scenes covering field layout, HUD, entry, move names, a Tackle animation frame, and the initial Poké Ball throw.

This is not a complete PokéRogue port. The visual harness does not run the full original `BattleScene` and `PhaseManager`; the attack and capture scenes cover specific animation stages only. See the [roadmap](ROADMAP.md), [porting progress](PORTING_PROGRESS.md), and [parity plan](PARITY_PLAN.md) for current scope and gaps.

## Build and play

Requirements: stable Rust with Rust 2024 edition support. Clone the repository and its pinned upstream source submodule:

```sh
git clone --recurse-submodules https://github.com/pontojasko/pokerogue-rs.git
cd pokerogue-rs
cargo build -p pokerogue-client
cargo run --offline -p pokerogue-client
```

The first build downloads Cargo dependencies if they are not cached. After that, the game client runs without contacting PokéRogue servers. The upstream source and asset submodules are checked out separately and remain subject to their own licenses and terms.

Controls: press Enter to start, `1` or `2` to choose a move, Enter to restart after a result, and Escape to quit.

## Tests

```sh
cargo check --workspace --offline
cargo test -p pokerogue-client --offline
python3 -m unittest discover -s scripts -p test_visual_parity.py
node tools/visual/harness.test.mjs
```

In the published snapshot, workspace compilation and all 13 client tests pass. The full workspace test command currently has one known failure: `deterministic_twenty_wave_run_clears_bosses_and_saves` loses at wave 12; the other workspace tests pass. This is documented in [PORTING_PROGRESS.md](PORTING_PROGRESS.md).

## Visual comparison

The visual harness runs locally and writes captures, diffs, reports, and regression results to the ignored `visual-artifacts/` directory. It does not use official game servers. Required development tools include Node.js, ImageMagick, Helium or a Playwright-compatible Chromium, and Xvfb for headless native captures.

```sh
npm ci --prefix tools/visual --ignore-scripts
python3 tools/visual/bootstrap.py --xvfb
python3 scripts/visual_parity.py --scene battle-town-capture --native-backend xvfb
```

To establish local regression baselines, inspect the captured images first, then explicitly record them:

```sh
python3 scripts/visual_suite.py --record-baselines --native-backend xvfb
python3 scripts/visual_suite.py --native-backend xvfb
```

Screenshots, baselines, downloaded runtimes, and game assets are not committed. See [visual tooling documentation](tools/visual/README.md) and [asset credits and restrictions](ASSET_CREDITS.md).

## Contributing

Start with the concise [contribution guide](CONTRIBUTING.md). It covers local setup, focused validation, pull requests, and AI-assisted contributions.

## License and attribution

This Rust workspace is licensed under **AGPL-3.0-only**; see [LICENSE](LICENSE). The TypeScript reference is a pinned upstream submodule and retains its own license and credits. Game assets are not bundled in this repository. Their redistribution rights are unresolved; review [ASSET_CREDITS.md](ASSET_CREDITS.md) before using or sharing them.

PokéRogue Rust is an unofficial fan project and is not affiliated with the original PokéRogue authors or Pokémon rights holders.
