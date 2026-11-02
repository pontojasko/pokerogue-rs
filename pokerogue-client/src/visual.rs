//! Offline presentation fixtures. Never dispatches gameplay commands or reads saves.
use super::*;
use serde_json::Value;
use std::sync::OnceLock;
static FIXED_TIME: OnceLock<f64> = OnceLock::new();
static UI_STATE: OnceLock<String> = OnceLock::new();
static PLAYER_GENDER: OnceLock<String> = OnceLock::new();
static ENEMY_GENDER: OnceLock<String> = OnceLock::new();
static ENEMY_OWNED_ICON: OnceLock<bool> = OnceLock::new();
pub fn ui_state() -> &'static str {
    UI_STATE.get().map(String::as_str).unwrap_or("command")
}
pub fn gender(player: bool) -> Option<&'static str> {
    let value = if player {
        PLAYER_GENDER.get()
    } else {
        ENEMY_GENDER.get()
    }?;
    matches!(value.as_str(), "male" | "female").then_some(value.as_str())
}
pub fn enemy_owned_icon() -> bool {
    ENEMY_OWNED_ICON.get().copied().unwrap_or(false)
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
    let fixture_ui = fixture["ui_state"].as_str().unwrap_or_else(|| {
        if fixture["scope"] == "moves" {
            "moves"
        } else {
            "command"
        }
    });
    UI_STATE
        .set(fixture_ui.to_owned())
        .expect("visual UI state initialized once");
    PLAYER_GENDER
        .set(
            fixture["player"]["gender"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        )
        .expect("visual player gender initialized once");
    ENEMY_GENDER
        .set(
            fixture["enemy"]["gender"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        )
        .expect("visual enemy gender initialized once");
    ENEMY_OWNED_ICON
        .set(fixture["enemy"]["owned_icon"].as_bool().unwrap_or(false))
        .expect("visual enemy ownership initialized once");
    FIXED_TIME
        .set(fixture["time_ms"].as_f64().expect("time_ms") / 1000.0)
        .unwrap();
    let wave1_entry = fixture["scope"] == "wave1-entry";
    let mut entry_run = wave1_entry.then(|| {
        pokerogue_core::MiniRun::new_with_wave_limit(
            fixture["seed"].as_str().expect("seed"),
            fixture["time_of_day"].as_str().expect("time_of_day"),
            fixture["wave_limit"].as_u64().expect("wave_limit") as u8,
        )
        .expect("core wave-1 entry state")
    });
    let mut battle = entry_run
        .as_ref()
        .map(|run| run.battle.clone())
        .unwrap_or_else(pokerogue_core::Battle::demo);
    if !wave1_entry {
        for (key, battler) in [
            ("player", &mut *battle.player),
            ("enemy", &mut battle.enemy),
        ] {
            let state = &fixture[key];
            let species =
                Species::from_numeric_id(state["species"].as_u64().expect("species") as u16)
                    .expect("supported species");
            let moves = if key == "player" && fixture["player_moves"].is_array() {
                fixture["player_moves"]
                    .as_array()
                    .expect("player_moves array")
                    .iter()
                    .map(|movement| match movement.as_str().expect("move id") {
                        "VineWhip" => pokerogue_data::Move::VineWhip,
                        "Tackle" => pokerogue_data::Move::Tackle,
                        "Ember" => pokerogue_data::Move::Ember,
                        "Growl" => pokerogue_data::Move::Imported(45),
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
    }
    if let Some(party) = fixture["party"].as_array() {
        for (slot, state) in party.iter().enumerate() {
            let Some(member) = battle.player.member_mut(slot) else {
                continue;
            };
            member.hp = state["hp"].as_i64().expect("party hp") as i32;
            member.max_hp = state["max_hp"].as_i64().expect("party max_hp") as i32;
            assert!(member.hp >= 0 && member.hp <= member.max_hp);
        }
    }
    if fixture["scope"] == "attack" || fixture["scope"] == "hit" {
        let attacks = fixture["attacks"]
            .as_array()
            .cloned()
            .unwrap_or_else(|| vec![fixture["attack"].clone()]);
        let mut events = Vec::new();
        for attack in attacks {
            let movement = match attack["move"].as_str().expect("attack move") {
                "Tackle" => pokerogue_data::Move::Tackle,
                other => panic!("unsupported visual attack {other}"),
            };
            let actor = match attack["actor"].as_str().expect("attack actor") {
                "player" => pokerogue_core::BattleActor::Player,
                "enemy" => pokerogue_core::BattleActor::Enemy,
                other => panic!("unsupported visual attack actor {other}"),
            };
            events.push(pokerogue_core::BattleEvent::MoveExecuted {
                actor,
                move_: movement,
            });
            if let Some(damage) = attack.get("damage") {
                events.push(pokerogue_core::BattleEvent::DamageApplied {
                    target: if actor == pokerogue_core::BattleActor::Player {
                        pokerogue_core::BattleActor::Enemy
                    } else {
                        pokerogue_core::BattleActor::Player
                    },
                    amount: damage["amount"].as_i64().expect("damage amount") as i32,
                    critical: damage["critical"].as_bool().unwrap_or(false),
                    effectiveness: match damage["effectiveness"].as_str().unwrap_or("normal") {
                        "super" => pokerogue_core::DamageEffectiveness::SuperEffective,
                        "not_very" => pokerogue_core::DamageEffectiveness::NotVeryEffective,
                        _ => pokerogue_core::DamageEffectiveness::Normal,
                    },
                });
            }
        }
        battle.last_events = events;
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
    let visual_ball_counts: [u8; 5] = fixture["ball_counts"]
        .as_array()
        .map(|counts| {
            std::array::from_fn(|index| {
                counts.get(index).and_then(Value::as_u64).unwrap_or(0) as u8
            })
        })
        .unwrap_or([5, 0, 0, 0, 0]);
    let visual_ball_cursor = fixture["ball_cursor"].as_u64().unwrap_or(0) as usize;
    let visual_command_cursor = fixture["command_cursor"].as_u64().unwrap_or(0) as usize;
    let ball_wiggle_started_at =
        time() - fixture["ball_wiggle_ms"].as_f64().unwrap_or(0.0) / 1000.0;
    let mut playback = BattleAnimationPlayback::default();
    if fixture["scope"] == "attack" || fixture["scope"] == "hit" {
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
                    &playback,
                );
                if fixture["scope"] == "hud" {
                    draw_hp_card(
                        &sprites,
                        true,
                        battle.player.species,
                        battle.player.species.name(),
                        battle.player.level,
                        battle.player.hp,
                        battle.player.max_hp,
                        0,
                        field.player_x * 3.0,
                        battle.player.gender,
                        battle.player.exp,
                    );
                    draw_hp_card(
                        &sprites,
                        false,
                        battle.enemy.species,
                        battle.enemy.species.name(),
                        battle.enemy.level,
                        battle.enemy.hp,
                        battle.enemy.max_hp,
                        0,
                        field.enemy_x * 3.0,
                        battle.enemy.gender,
                        battle.enemy.exp,
                    );
                }
            }
            "attack" | "hit" => {
                draw_battlefield_backdrop(sprites.town_background.as_ref());
                sprites.draw_town_platforms(field);
                sprites.draw(
                    battle.player.species,
                    battle.enemy.species,
                    field,
                    None,
                    None,
                    None,
                    &playback,
                );
                sprites.draw_battle_animation(&mut playback);
                sprites.draw_damage_numbers(&playback, field);
                draw_hp_card(
                    &sprites,
                    true,
                    battle.player.species,
                    battle.player.species.name(),
                    battle.player.level,
                    battle.player.hp,
                    battle.player.max_hp,
                    0,
                    field.player_x * 3.0,
                    battle.player.gender,
                    battle.player.exp,
                );
                draw_hp_card(
                    &sprites,
                    false,
                    battle.enemy.species,
                    battle.enemy.species.name(),
                    battle.enemy.level,
                    battle.enemy.hp,
                    battle.enemy.max_hp,
                    battle.enemy.boss_segments,
                    field.enemy_x * 3.0,
                    battle.enemy.gender,
                    battle.enemy.exp,
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
                    &playback,
                );
                if capture_pose.is_some_and(|pose| !pose.ball_behind_enemy) {
                    sprites.draw_capture_ball(capture_pose);
                }
                sprites.draw_capture_open_particles(&playback, visual::time());
                sprites.draw_capture_success_fx(&playback, visual::time());
                draw_hp_card(
                    &sprites,
                    true,
                    battle.player.species,
                    battle.player.species.name(),
                    battle.player.level,
                    battle.player.hp,
                    battle.player.max_hp,
                    0,
                    field.player_x * 3.0,
                    battle.player.gender,
                    battle.player.exp,
                );
                draw_hp_card(
                    &sprites,
                    false,
                    battle.enemy.species,
                    battle.enemy.species.name(),
                    battle.enemy.level,
                    battle.enemy.hp,
                    battle.enemy.max_hp,
                    battle.enemy.boss_segments,
                    field.enemy_x * 3.0,
                    battle.enemy.gender,
                    battle.enemy.exp,
                );
            }
            "battle" | "moves" => draw_battle(
                &mut battle,
                &sprites,
                &mut playback,
                fixture["wave"].as_u64().unwrap_or(1) as u8,
                10,
                1000,
                0,
                match ui_state() {
                    "moves" => crate::battle_ui::BattleUiState::Moves,
                    "party" => crate::battle_ui::BattleUiState::Party,
                    "balls" => crate::battle_ui::BattleUiState::Balls,
                    _ => crate::battle_ui::BattleUiState::Command,
                },
                "",
                visual_command_cursor,
                visual_ball_cursor,
                ball_wiggle_started_at,
                0,
                visual_ball_counts,
                field,
            ),
            "reward" => {
                playback.faint = Some(crate::ActiveFaintAnimation {
                    actor: BattleActor::Enemy,
                    slot: 0,
                    starts_at: time() - 1.0,
                });
                draw_battle(
                    &mut battle,
                    &sprites,
                    &mut playback,
                    fixture["wave"].as_u64().unwrap_or(1) as u8,
                    fixture["wave_limit"].as_u64().unwrap_or(10) as u8,
                    fixture["money"].as_u64().unwrap_or(1000),
                    0,
                    crate::battle_ui::BattleUiState::Command,
                    "",
                    0,
                    0,
                    ball_wiggle_started_at,
                    0,
                    visual_ball_counts,
                    field,
                );
                let reward = &fixture["reward"];
                let cards = reward["options"].as_array().map(|options| {
                    std::array::from_fn(|index| {
                        let option = &options[index];
                        crate::RewardCard {
                            name: option["name"].as_str().unwrap_or_default().to_owned(),
                            icon: option["icon"].as_str().unwrap_or_default().to_owned(),
                            description: option["description"]
                                .as_str()
                                .unwrap_or_default()
                                .to_owned(),
                            tier: option["tier"].as_u64().unwrap_or(0) as u8,
                        }
                    })
                });
                crate::draw_reward_screen(
                    fixture["wave"].as_u64().unwrap_or(1) as u8,
                    fixture["wave_limit"].as_u64().unwrap_or(10) as u8,
                    fixture["experience"].as_u64().unwrap_or(0) as u32,
                    fixture["money"].as_u64().unwrap_or(1000),
                    "Wave clear",
                    cards,
                    "NATURAL · GREAT",
                    reward["selected"].as_u64().unwrap_or(1) as usize,
                    time() as f32,
                    &sprites,
                    (
                        battle.player.species.name(),
                        battle.player.hp,
                        battle.player.max_hp,
                        reward["party_slot"].as_u64().unwrap_or(1) as usize,
                        "",
                    ),
                    "",
                    "LEFT/RIGHT choose · TAB target · ENTER select",
                );
            }
            "wave1-entry" => {
                let run = entry_run.as_mut().expect("core wave-1 entry state");
                let requested_ui = match ui_state() {
                    "message" => crate::battle_ui::BattleUiState::Message,
                    "moves" => crate::battle_ui::BattleUiState::Moves,
                    "party" => crate::battle_ui::BattleUiState::Party,
                    _ => crate::battle_ui::BattleUiState::Command,
                };
                let ui = if fixture["show_entry_message"].as_bool().unwrap_or(false)
                    && show_first_entry_message(EncounterEntranceKind::First, time())
                {
                    crate::battle_ui::BattleUiState::Message
                } else {
                    requested_ui
                };
                draw_battle(
                    &mut battle,
                    &sprites,
                    &mut playback,
                    run.wave,
                    run.wave_limit,
                    run.money,
                    0,
                    ui,
                    "",
                    0,
                    0,
                    ball_wiggle_started_at,
                    0,
                    [run.pokeballs, run.greatballs, run.ultraballs, 0, 0],
                    field,
                );
            }
            other => panic!("unsupported scope {other}"),
        }
        if frame == 1 {
            let screenshot = get_screen_data();
            if (screenshot.width, screenshot.height) == (960, 540) {
                screenshot.export_png(output);
            } else {
                let screen_w = screen_width();
                let screen_h = screen_height();
                let (x, y, width, height) = fitted_viewport(screen_w, screen_h);
                let viewport = (
                    (x as f32 * f32::from(screenshot.width) / screen_w).round() as i32,
                    (y as f32 * f32::from(screenshot.height) / screen_h).round() as i32,
                    (width as f32 * f32::from(screenshot.width) / screen_w).round() as i32,
                    (height as f32 * f32::from(screenshot.height) / screen_h).round() as i32,
                );
                image_at_logical_viewport(&screenshot, viewport).export_png(output);
            }
        }
        next_frame().await;
    }
    if let Ok(hold_ms) = std::env::var("POKEROGUE_VISUAL_HOLD_MS") {
        if let Ok(hold_ms) = hold_ms.parse::<u64>() {
            let until = get_time() + hold_ms as f64 / 1000.0;
            while get_time() < until {
                next_frame().await;
            }
        }
    }
    true
}
