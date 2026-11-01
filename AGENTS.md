# Workspace notes
- Keep rules, RNG, progression and persistence in `pokerogue-core` (or data-only content in `pokerogue-data`); rendering/input belong in `pokerogue-client`.
- Before work, read `PORTING_PROGRESS.md` and the relevant `ARCHITECTURE.md` / `PARITY_PLAN.md` sections. `ROADMAP.md` defines milestone order.
- TypeScript reference: `original/` commit `7c0881255af5cd51992e513ff93c2f71adbe40e0`. Claim parity only for behavior compared against executed original code/fixtures; record inference and gaps in `PARITY_PLAN.md`.
- Workflow: make one bounded increment, run focused checks while editing, then at milestone completion run `cargo fmt --all`, `cargo test --workspace`, `cargo parity`, `cargo check --workspace`, and confirm `cargo run -p pokerogue-client`; fix failures and update progress/parity docs. When touching imported data or parity fixtures, run their generator `--check` (`import_demo_data.mjs`, `import_modifier_pool_catalog.mjs`, and any relevant fixture generator). Avoid repeating full-suite runs for unrelated small edits.
- Runtime must remain fully offline; Node is development tooling only. Saves/configuration stay local. Do not add a server or multiplayer.
- Review asset-specific licenses and attribution before copying or redistributing assets; unresolved sprite rights are documented in `ASSET_CREDITS.md`.
- R36S is future-only: RK3326 Cortex-A35, 1 GB RAM, 640x480, Linux/ArkOS. Do not assume 64-bit userspace or direct Macroquad compatibility; do not start its port without an explicit milestone.
