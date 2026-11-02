# Local sprite asset credits and usage

The client reads these files from the local `original/assets` checkout at runtime:

- `images/pokemon/back/1.png` and `.json` — Bulbasaur back atlas, frame metadata.
- `images/pokemon/4.png` and `.json` — Charmander front atlas, frame metadata.

Source: [pagefaultgames/pokerogue-assets](https://github.com/pagefaultgames/pokerogue-assets), beta branch, commit `fb8ccb6`. Bulbasaur and Charmander are Pokémon franchise characters; the associated sprite pixels are credited to Nintendo, Game Freak, and Creatures Inc. through the source project context.

The asset repository README states that assets are CC BY-NC-SA 4.0 only to the extent they are licensable and applicable, and documents third-party exceptions. `images/REUSE.toml` does not give these species atlas PNGs a specific per-file attribution/license entry. Their exact redistribution status is therefore unresolved. The sprite files remain only in the local upstream checkout and are not copied into this Rust project or included in a Rust package. The client loads them by path. Do not redistribute these sprite files or builds containing them without clarifying the rights. `POKEROGUE_ASSETS_DIR` may point to the upstream `images/pokemon` directory if it is elsewhere.

Relevant source metadata: upstream `README.md`, `images/REUSE.toml`, `images/pokemon/1.json`, `images/pokemon/back/1.json`, and `images/pokemon/4.json`. The JSON atlas coordinates select the idle animation frames 0001 and 0003. Texture sampling uses nearest-neighbor filtering to preserve pixel edges and alpha transparency.

## Additional local Town battle presentation assets

The client also loads `images/arenas/town_bg.png`, `town_a.png`, and `town_b.png`; the latter two are transparent player/enemy platforms. The battle HUD uses `images/ui/bg.png`, `images/ui/windows/window_1.png`, `images/ui/pbinfo_player.png`, `images/ui/pbinfo_enemy_mini.png`, `images/ui/pbinfo_enemy_boss.png`, `images/ui/overlay_hp.png`, and `images/ui/overlay_hp_boss.png`. Runtime Pokémon sprites are selected from the Town species pool; `scripts/generate_town_sprite_manifest.mjs` extracts their front/back atlas frame rectangles and trim/source dimensions from the corresponding local JSON files. The checkout is pinned to full asset commit `fb8ccb6d0703ffe48cdf80a8102d5ed943c82dba`, exactly the gitlink revision in the pinned game repository.

The upstream `images/REUSE.toml` contains no specific attribution/license entry for these Town PNGs or species PNG atlases. The asset repository README says CC BY-NC-SA 4.0 applies only to the extent assets are licensable/applicable and points to per-file exceptions. Pokémon imagery is third-party copyrighted material; the exact rights for these selected files remain unresolved. These files and atlas metadata remain in the local `original/assets` checkout only, are not copied into this repository's packages, and must not be redistributed or embedded in release builds without rights clarification. Local runtime fallback remains available when the checkout is absent.

## Local move animation assets

The client optionally reads `battle-anims/tackle.json`, `battle-anims/ember.json`, `images/battle_anims/PRAS- Strike.png`, and `images/battle_anims/PRAS- Fire.png` from that same local assets checkout. The two JSON files are listed under `battle-anims/REUSE.toml` as `The Pokemon Reborn Team` / `LicenseRef-POKEMON-REBORN`; the image paths have no matching specific entry in `images/REUSE.toml`. Rights for redistribution of both the animation data and spritesheets are unresolved, so they remain local-only and must not be bundled or redistributed without clarification. No sound files were loaded.

The battle capture animation and ball-selection menu read `images/pb.png` and atlas frames from `images/pb.json` for Poké, Great, Ultra, Rogue and Master Balls. The PNG's `images/REUSE.toml` entry credits Nintendo, Game Freak, and Creatures Inc. under `LicenseRef-FAIR-USE`; the JSON is listed with generated atlas metadata under AGPL-3.0-only. These files remain in the pinned local assets checkout and are not copied into Rust packages. The PNG is local-only and must not be redistributed without resolving its fair-use status.

The capture opening particles optionally read `images/effects/pb_particles.png` with coordinates extracted from `images/effects/pb_particles.json`. The metadata JSON is listed under the generated atlas metadata annotation in `images/REUSE.toml` (AGPL-3.0-only), but the PNG has no per-file entry. Its redistribution rights are unresolved. Both files are restored only inside the pinned local `original/assets` checkout, not copied into the Rust package, and must not be redistributed absent rights clarification.


## Visual HUD reference resources (local only)

`images/ui/overlay_exp.png` is additionally loaded at runtime for the player's EXP meter. It has no separate redistribution grant in the pinned source beyond the conditional asset-license statement, so it remains local-only and is not copied or bundled.

The visual tooling/client additionally read `fonts/pokemon-emerald-pro.ttf`, `images/ui/numbers.{json,png}`, `images/ui/overlay_hp{,_boss}.json`, and English `images/ui/text_images/en/battle_ui/overlay_{lv,hp_label,exp_label}.png` from the pinned asset commit. `overlay_exp.png` is loaded by the original PlayerBattleInfo constructor. These resources were restored from existing local Git objects, not copied into client packages. The numbers/HP JSON files are covered by the generated-metadata entry (AGPL-3.0-only) in pinned `images/REUSE.toml`; specific rights for the selected PNGs and font are unresolved under the upstream conditional asset-license statement. Keep them, captured screenshots and local regression baselines out of commits/releases. `tools/visual/bootstrap.py` restores only these selected local paths and checks the asset commit first.

Optional Xvfb is development tooling: a checksum-pinned package binary is extracted into ignored `tools/visual/runtime/` without installing system packages or changing display configuration. It is not distributed with the client.

## First local audio slice

The client optionally reads selected files from `audio/` at the pinned asset revision `fb8ccb6d0703ffe48cdf80a8102d5ed943c82dba`: `ui/select.wav`, `ui/menu_open.wav`, `ui/error.wav`; `se/hit.wav`, `se/faint.wav`, `se/pb_throw.wav`, `se/crit_throw.wav`, `se/pb_rel.wav`, `se/pb_catch.wav`, `se/pb_move.wav`, `se/pb_lock.wav`; and `battle_anims/PRSFX- Tackle.wav` / `PRSFX- Vine Whip.wav`. They are not copied into this repository or embedded in the binary. Pinned `audio/REUSE.toml` assigns the UI, SE, and Pokémon-derived effects `LicenseRef-FAIR-USE` to Nintendo, Creatures Inc., and GAME FREAK inc.; the two `PRSFX-` clips are credited to The Pokemon Reborn Team under `LicenseRef-POKEMON-REBORN`. Those rights remain local-only and must not be redistributed without clarification.

Town music is `audio/bgm/town.mp3`, attributed to Nintendo, Creatures Inc., and GAME FREAK inc. under `LicenseRef-FAIR-USE` in the same pinned `audio/REUSE.toml`. Macroquad's native decoder accepts WAV/Ogg but not MP3, so the local ignored audio cache contains a Vorbis Ogg conversion of that file; the conversion is not committed or bundled. Recreate it from the asset root with `ffmpeg -i audio/bgm/town.mp3 -vn -c:a libvorbis -q:a 4 audio/bgm/town.ogg`. The source `Arena` uses Town's BGM and its loop point is 7.288 seconds. This first client pass loops the converted whole track from its beginning because Macroquad exposes whole-sound looping but no seek/loop-point control. Keep source and converted audio files out of commits/releases.

The battle command/move UI also reads `images/ui/cursor.png` from the same local pinned asset checkout (6×10 source pixels). `images/REUSE.toml` has no individual entry for this file, so its redistribution status is unresolved; it remains local-only and is not copied or embedded.

## Party selection screen assets

The Rust party selector reads the pinned `images/ui/party_bg.png`, `party_slot_main.{png,json}`, `party_slot.{png,json}`, `party_pb.{png,json}`, `party_slot_hp_bar.png`, `party_slot_hp_overlay.{png,json}`, `party_cancel.{png,json}`, and English `images/ui/text_images/en/party_ui/party_slot_overlay_{hp,lv}.png`. Pokémon menu icons load the matching generation atlas `images/pokemon_icons_<generation>.{png,json}`. The atlas JSON files have generated-metadata entries under `images/REUSE.toml`; the selected PNGs have no matching per-file rights entry there. The upstream README applies CC BY-NC-SA only to assets where it is licensable and applicable. These UI/sprite assets therefore remain local-only with unresolved redistribution rights: they are restored from the pinned asset Git objects into `original/assets` by `tools/visual/bootstrap.py`, read at runtime, and are not copied or embedded in the Rust package.

## Title screen assets

The Rust title screen optionally reads `images/logo.png`, `images/arenas/plains_bg.png`, `images/ui/windows/window_1.png`, and (during the pinned Halloween 2026 event window) `images/events/logo_halloween.png` / `images/events/halloween2026-en.png` from the pinned local `pokerogue-assets` checkout/cache. `images/REUSE.toml` has no per-file entry for these paths. The asset repository README applies CC BY-NC-SA 4.0 only to assets that are licensable and applicable; the logos, arena and event artwork include Pokémon-derived artwork, so their redistribution rights remain unresolved. These assets are loaded from the local checkout/cache, are not copied into Rust packages, and must not be redistributed or embedded in a release without rights clarification.

## README gameplay reference screenshot

`docs/latest-gameplay-reference.png` is the latest user-provided local screenshot (captured 2026-11-02) included in the public README at the project owner's request. It shows the original PokéRogue UI and Pokémon artwork; it is explicitly labeled as an original-game reference, not a Rust-client capture. The screenshot does not include a separate license grant for the underlying artwork. Rights remain unresolved; its presence in this repository is a deliberate documentation exception requested by the project owner and must not be read as permission to extract, reuse, or redistribute the depicted assets separately.
