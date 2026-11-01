# Contributing to PokéRogue Rust

Thanks for taking a look. Small, focused changes are easiest to review. The port is incomplete, so check the roadmap and parity notes before choosing a feature.

## Quick start

1. Fork the repository and clone it with the pinned source checkout:

   ```sh
   git clone --recurse-submodules https://github.com/<your-account>/pokerogue-rs.git
   cd pokerogue-rs
   cargo build -p pokerogue-client
   ```

2. Choose one bounded item from [ROADMAP.md](ROADMAP.md). Read [AGENTS.md](AGENTS.md), [ARCHITECTURE.md](ARCHITECTURE.md), and the relevant sections of [PARITY_PLAN.md](PARITY_PLAN.md).
3. Keep battle rules, RNG, progression, and saves in `pokerogue-core`; keep rendering and input in `pokerogue-client`.
4. Add or update a focused test or source fixture when behavior changes. For data and visual fixtures, run their generator/check where available.
5. Run the checks relevant to your change, then open a pull request with a short summary, evidence, and known limitations.

## Useful checks

```sh
cargo fmt --all -- --check
cargo check --workspace --offline
cargo test -p pokerogue-client --offline
cargo parity --offline
```

`cargo parity` currently includes a known failing twenty-wave route test that loses at wave 12. Report whether your change affects that result; do not weaken the assertion or change game rules only to make the test green. For the optional visual tools, see [tools/visual/README.md](tools/visual/README.md).

## Pull request checklist

- [ ] The change has one clear scope and follows the project architecture.
- [ ] Relevant tests, source fixtures, or visual captures are included or updated.
- [ ] `ROADMAP.md`, `PORTING_PROGRESS.md`, or `PARITY_PLAN.md` is updated when the supported scope or known gaps change.
- [ ] The PR description lists commands run and their results, including failures.
- [ ] No saves, credentials, screenshots, visual baselines, build output, or restricted game assets are included.

## Contributions made with AI

AI tools are welcome as assistants; the contributor remains responsible for the patch.

- Give the tool the relevant instructions from `AGENTS.md` and the specific roadmap item. Ask it to keep the change bounded and identify source evidence.
- Review the complete diff yourself. Check that generated code and tests are meaningful and that no unrelated files changed.
- Run the relevant checks locally. Do not rely on an AI summary as proof that tests passed or behavior matches the original.
- Mention AI assistance in the pull request when it materially produced code, tests, or documentation. Briefly state what you reviewed and verified.
- Do not give an AI tool credentials, personal saves, or private data. Do not ask it to copy or commit restricted assets, screenshots, or baselines.
- For parity work, never change RNG, battle state, or progression merely to make screenshots look closer. Use deterministic presentation fixtures and document harness limits.

## Assets and licensing

The original TypeScript project is included as a pinned submodule. Assets are checked out separately and are not redistributed by this repository. Review [ASSET_CREDITS.md](ASSET_CREDITS.md) before using any asset, and do not add asset bytes or screenshots to commits without clear permission.
