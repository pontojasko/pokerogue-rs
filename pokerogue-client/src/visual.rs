//! Offline presentation fixtures. Never dispatches gameplay commands or reads saves.
use super::*;
use serde_json::Value;
use std::sync::OnceLock;
static FIXED_TIME: OnceLock<f64> = OnceLock::new();
static MOVES_FIXTURE: OnceLock<bool> = OnceLock::new();
pub fn moves_fixture() -> bool {
    MOVES_FIXTURE.get().copied().unwrap_or(false)
}
pub fn time() -> f64 {
    FIXED_TIME.get().copied().unwrap_or_else(get_time)
}
pub fn sprite_elapsed(started_at: f64) -> f64 {
    FIXED_TIME
        .get()
        .copied()
        .unwrap_or_else(|| get_time() - started_at)
}
pub async fn run_if_requested() -> bool {
    let args: Vec<String> = std::env::args().collect();
    let Some(index) = args.iter().position(|s| s == "--visual-fixture") else {
        return false;
    };
    let fixture: Value = serde_json::from_slice(
        &std::fs::read(args.get(index + 1).expect("fixture path")).expect("read fixture"),
    )
    .expect("fixture JSON");
    let output = args
        .iter()
        .position(|s| s == "--visual-output")
        .and_then(|i| args.get(i + 1))
        .expect("--visual-output PNG");
    assert_eq!(fixture["width"], 960);
    assert_eq!(fixture["height"], 540);
    MOVES_FIXTURE
        .set(fixture["scope"] == "moves")
        .expect("visual fixture initialized once");
    FIXED_TIME
        .set(fixture["time_ms"].as_f64().expect("time_ms") / 1000.0)
        .unwrap();
    let mut battle = pokerogue_core::Battle::demo();
    for (key, battler) in [
        ("player", &mut *battle.player),
        ("enemy", &mut battle.enemy),
    ] {
        let state = &fixture[key];
        let species = Species::from_numeric_id(state["species"].as_u64().expect("species") as u16)
            .expect("supported species");
        let moves = if key == "player" && fixture["scope"] == "moves" {
            fixture["player_moves"]
                .as_array()
                .expect("player_moves array")
                .iter()
                .map(|movement| match movement.as_str().expect("move id") {
                    "VineWhip" => pokerogue_data::Move::VineWhip,
                    "Tackle" => pokerogue_data::Move::Tackle,
                    "Ember" => pokerogue_data::Move::Ember,
                    other => panic!("unsupported visual move {other}"),
                })
                .collect::<Vec<_>>()
        } else {
            vec![pokerogue_data::Move::Tackle]
        };
        let level = state["level"].as_u64().expect("level") as u8;
        *battler = match moves.as_slice() {
            [a] => pokerogue_core::Battler::demo(species, level, [*a]),
            [a, b] => pokerogue_core::Battler::demo(species, level, [*a, *b]),
            [a, b, c] => pokerogue_core::Battler::demo(species, level, [*a, *b, *c]),
            [a, b, c, d] => pokerogue_core::Battler::demo(species, level, [*a, *b, *c, *d]),
            _ => panic!("visual fixture requires one to four moves"),
        };
        battler.hp = state["hp"].as_i64().expect("hp") as i32;
        battler.max_hp = state["max_hp"].as_i64().expect("max_hp") as i32;
        assert!(battler.hp >= 0 && battler.hp <= battler.max_hp);
    }
    if fixture["scope"] == "attack" {
        let movement = match fixture["attack"]["move"].as_str().expect("attack move") {
            "Tackle" => pokerogue_data::Move::Tackle,
            other => panic!("unsupported visual attack {other}"),
        };
        battle.last_events = vec![pokerogue_core::BattleEvent::MoveExecuted {
            actor: match fixture["attack"]["actor"].as_str().expect("attack actor") {
                "player" => pokerogue_core::BattleActor::Player,
                other => panic!("unsupported visual attack actor {other}"),
            },
            move_: movement,
        }];
    }
    if fixture["scope"] == "capture" {
        battle.last_capture_attempt = Some(pokerogue_core::CaptureAttempt {
            modified_catch_rate: 255,
            shake_probability: 65_536.0,
            critical: false,
            shake_rolls: Vec::new(),
            caught: fixture["capture"]["caught"]
                .as_bool()
                .expect("capture caught"),
        });
        battle.last_events = vec![pokerogue_core::BattleEvent::CaptureAttemptResolved {
            species_id: battle.enemy.species.numeric_id(),
            caught: fixture["capture"]["caught"]
                .as_bool()
                .expect("capture caught"),
            party_slot: Some(1),
        }];
    }
    let mut sprites = SpriteAtlas::load().await;
    sprites
        .set_battlers(battle.player.species, battle.enemy.species)
        .await;
    assert!(
        sprites.town_background.is_some()
            && sprites.town_player_platform.is_some()
            && sprites.town_enemy_platform.is_some(),
        "local Town assets required"
    );
    assert!(
        sprites.active_player.is_some() && sprites.active_enemy.is_some(),
        "local battler atlases required"
    );
    let field = match fixture["phase"].as_str().expect("phase") {
        "settled" => final_field_presentation(),
        "entry" => encounter_entrance(EncounterEntranceKind::First, (time() / 2.0) as f32),
        other => panic!("unsupported phase {other}"),
    };
    let mut playback = BattleAnimationPlayback::default();
    if fixture["scope"] == "attack" {
        playback.enqueue_turn(&battle, 0.0, None);
    }
    if fixture["scope"] == "capture" {
        let ball = match fixture["capture"]["ball"].as_str().expect("capture ball") {
            "POKEBALL" => CaptureBall::Poke,
            other => panic!("unsupported visual capture ball {other}"),
        };
        playback.enqueue_turn(&battle, 0.0, Some(ball));
    }
    // Warm up the framebuffer; read back the same render on the second frame.
    for frame in 0..2 {
        clear_background(BLACK);
        set_logical_viewport();
        match fixture["scope"].as_str().expect("scope") {
            "field" | "hud" => {
                draw_battlefield_backdrop(sprites.town_background.as_ref());
                sprites.draw_town_platforms(field);
                sprites.draw(
                    battle.player.species,
                    battle.enemy.species,
                    field,
                    None,
                    None,
                    None,
                );
                if fixture["scope"] == "hud" {
                    draw_hp_card(
                        &sprites,
                        true,
                        battle.player.species.name(),
                        battle.player.level,
                        battle.player.hp,
                        battle.player.max_hp,
                        0,
                        field.player_x * 3.0,
                    );
                    draw_hp_card(
                        &sprites,
                        false,
                        battle.enemy.species.name(),
                        battle.enemy.level,
                        battle.enemy.hp,
                        battle.enemy.max_hp,
                        0,
                        field.enemy_x * 3.0,
                    );
                }
            }
            "attack" => {
                draw_battlefield_backdrop(sprites.town_background.as_ref());
                sprites.draw_town_platforms(field);
                sprites.draw(
                    battle.player.species,
                    battle.enemy.species,
                    field,
                    None,
                    None,
                    None,
                );
                sprites.draw_battle_animation(&mut playback);
                draw_hp_card(
                    &sprites,
                    true,
                    battle.player.species.name(),
                    battle.player.level,
                    battle.player.hp,
                    battle.player.max_hp,
                    0,
                    field.player_x * 3.0,
                );
                draw_hp_card(
                    &sprites,
                    false,
                    battle.enemy.species.name(),
                    battle.enemy.level,
                    battle.enemy.hp,
                    battle.enemy.max_hp,
                    battle.enemy.boss_segments,
                    field.enemy_x * 3.0,
                );
            }
            "capture" => {
                let capture_pose = playback.capture_pose(visual::time());
                draw_battlefield_backdrop(sprites.town_background.as_ref());
                sprites.draw_town_platforms(field);
                sprites.draw(
                    battle.player.species,
                    battle.enemy.species,
                    field,
                    None,
                    None,
                    capture_pose,
                );
                if capture_pose.is_some_and(|pose| !pose.ball_behind_enemy) {
                    sprites.draw_capture_ball(capture_pose);
                }
                sprites.draw_capture_open_particles(&playback, visual::time());
                sprites.draw_capture_success_fx(&playback, visual::time());
                draw_hp_card(
                    &sprites,
                    true,
                    battle.player.species.name(),
                    battle.player.level,
                    battle.player.hp,
                    battle.player.max_hp,
                    0,
                    field.player_x * 3.0,
                );
                draw_hp_card(
                    &sprites,
                    false,
                    battle.enemy.species.name(),
                    battle.enemy.level,
                    battle.enemy.hp,
                    battle.enemy.max_hp,
                    battle.enemy.boss_segments,
                    field.enemy_x * 3.0,
                );
            }
            "battle" | "moves" => draw_battle(
                &mut battle,
                &sprites,
                &mut playback,
                1,
                10,
                5,
                0,
                0,
                0,
                field,
            ),
            other => panic!("unsupported scope {other}"),
        }
        if frame == 1 {
            assert_eq!(
                (screen_width() as u32, screen_height() as u32),
                (960, 540),
                "capture requires 960x540 framebuffer"
            );
            get_screen_data().export_png(output);
        }
        next_frame().await;
    }
    true
}
