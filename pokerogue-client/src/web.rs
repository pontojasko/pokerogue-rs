//! Small browser build of the existing Rust wave-1 battle slice.
//!
//! Artwork is fetched from the upstream, pinned pokerogue-assets repository at
//! runtime. This web target does not copy those files into this repository or
//! its GitHub Pages artifact.

use macroquad::prelude::*;
use pokerogue_core::{BattleActor, BattleEvent, MiniRun, MiniRunStage, PlayerAction};
use serde_json::Value;

const ASSET_REVISION: &str = "fb8ccb6d0703ffe48cdf80a8102d5ed943c82dba";
const ASSET_ROOT: &str = "https://raw.githubusercontent.com/pagefaultgames/pokerogue-assets";

struct Sprite {
    texture: Texture2D,
    source: Rect,
    width: f32,
    height: f32,
}

fn asset_url(path: &str) -> String {
    format!("{ASSET_ROOT}/{ASSET_REVISION}/images/{path}")
}

async fn load_source_texture(path: &str) -> Option<Texture2D> {
    let texture = macroquad::texture::load_texture(&asset_url(path))
        .await
        .ok()?;
    texture.set_filter(FilterMode::Nearest);
    Some(texture)
}

async fn load_sprite(species_id: u16, back: bool) -> Option<Sprite> {
    let folder = if back { "pokemon/back" } else { "pokemon" };
    let stem = format!("{folder}/{species_id}");
    let texture = load_source_texture(&format!("{stem}.png")).await?;
    let metadata = load_string(&asset_url(&format!("{stem}.json")))
        .await
        .ok()?;
    let atlas: Value = serde_json::from_str(&metadata).ok()?;
    let frames = atlas["textures"][0]["frames"].as_array()?;
    let frame = frames
        .iter()
        .find(|frame| frame["filename"].as_str() == Some("0001.png"))
        .or_else(|| frames.first())?;
    let source = &frame["frame"];
    let dimensions = &frame["sourceSize"];
    Some(Sprite {
        texture,
        source: Rect::new(
            source["x"].as_f64()? as f32,
            source["y"].as_f64()? as f32,
            source["w"].as_f64()? as f32,
            source["h"].as_f64()? as f32,
        ),
        width: dimensions["w"].as_f64()? as f32,
        height: dimensions["h"].as_f64()? as f32,
    })
}

fn draw_sprite(sprite: Option<&Sprite>, x: f32, y: f32, scale: f32, flip: bool) {
    let Some(sprite) = sprite else { return };
    let width = sprite.width * scale;
    let height = sprite.height * scale;
    draw_texture_ex(
        &sprite.texture,
        x,
        y,
        WHITE,
        DrawTextureParams {
            source: Some(sprite.source),
            dest_size: Some(vec2(width, height)),
            flip_x: flip,
            ..Default::default()
        },
    );
}

fn draw_health_card(label: &str, level: u8, hp: i32, max_hp: i32, x: f32, y: f32) {
    draw_rectangle(x, y, 292.0, 88.0, Color::from_rgba(248, 248, 232, 245));
    draw_rectangle_lines(x, y, 292.0, 88.0, 3.0, Color::from_rgba(48, 72, 88, 255));
    draw_text(label, x + 14.0, y + 26.0, 25.0, DARKBROWN);
    draw_text(&format!("Lv{level}"), x + 217.0, y + 26.0, 22.0, DARKBROWN);
    draw_text("HP", x + 20.0, y + 54.0, 17.0, DARKBROWN);
    draw_rectangle(
        x + 57.0,
        y + 40.0,
        218.0,
        16.0,
        Color::from_rgba(48, 72, 88, 255),
    );
    draw_rectangle(x + 60.0, y + 43.0, 212.0, 10.0, GRAY);
    let fraction = if max_hp > 0 {
        hp.max(0) as f32 / max_hp as f32
    } else {
        0.0
    };
    let color = if fraction > 0.5 {
        GREEN
    } else if fraction > 0.2 {
        ORANGE
    } else {
        RED
    };
    draw_rectangle(x + 60.0, y + 43.0, 212.0 * fraction, 10.0, color);
    draw_text(
        &format!("{}/{}", hp.max(0), max_hp),
        x + 178.0,
        y + 76.0,
        18.0,
        DARKBROWN,
    );
}

fn action_message(events: &[BattleEvent]) -> String {
    let mut messages = Vec::new();
    for event in events {
        match event {
            BattleEvent::MoveExecuted { actor, move_ } => {
                let who = if *actor == BattleActor::Player {
                    "Your Pokémon"
                } else {
                    "Wild Pokémon"
                };
                messages.push(format!("{who} used {}!", move_.name()));
            }
            BattleEvent::DamageApplied { target, amount, .. } => {
                let who = if *target == BattleActor::Player {
                    "Your Pokémon"
                } else {
                    "Wild Pokémon"
                };
                messages.push(format!("{who} took {amount} damage."));
            }
            BattleEvent::BattleEnded(outcome) => {
                messages.push(format!("Battle result: {outcome:?}."))
            }
            _ => {}
        }
    }
    if messages.is_empty() {
        "Choose a move with keys 1–4.".to_owned()
    } else {
        messages.join("  ")
    }
}

fn wrap_message(message: &str, max_chars: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    for word in message.split_whitespace() {
        let line = lines.last_mut().expect("at least one line");
        if !line.is_empty() && line.len() + word.len() + 1 > max_chars {
            lines.push(word.to_owned());
        } else {
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
    }
    lines
}

#[macroquad::main("PokéRogue Rust Web")]
async fn main() {
    let mut run = MiniRun::new_with_wave_limit("M12Speed0016", "DAY", 10).ok();
    let background = load_source_texture("arenas/town_bg.png").await;
    let player_platform = load_source_texture("arenas/town_a.png").await;
    let enemy_platform = load_source_texture("arenas/town_b.png").await;
    let mut loaded_species = (0, 0);
    let mut player_sprite = None;
    let mut enemy_sprite = None;
    let mut message = "Wave 1 · Town".to_owned();

    loop {
        let (screen_width, screen_height) = (screen_width(), screen_height());
        let scale = (screen_width / 960.0).min(screen_height / 540.0);
        let viewport = vec2(960.0 * scale, 540.0 * scale);
        let offset = vec2(
            (screen_width - viewport.x) * 0.5,
            (screen_height - viewport.y) * 0.5,
        );
        set_camera(&Camera2D {
            target: vec2(480.0, 270.0),
            zoom: vec2(2.0 / viewport.x, 2.0 / viewport.y),
            viewport: Some((
                offset.x as i32,
                offset.y as i32,
                viewport.x as i32,
                viewport.y as i32,
            )),
            ..Default::default()
        });
        clear_background(Color::from_rgba(224, 236, 238, 255));
        if let Some(bg) = &background {
            draw_texture_ex(
                bg,
                0.0,
                0.0,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(960.0, 540.0)),
                    ..Default::default()
                },
            );
        } else {
            draw_rectangle(0.0, 0.0, 960.0, 540.0, Color::from_rgba(190, 220, 218, 255));
            draw_rectangle(
                0.0,
                340.0,
                960.0,
                200.0,
                Color::from_rgba(228, 220, 172, 255),
            );
        }

        if let Some(current) = run.as_mut() {
            let player = current
                .battle
                .player
                .member(current.battle.player.active_slot())
                .unwrap();
            let enemy = &current.battle.enemy;
            let species = (player.species.numeric_id(), enemy.species.numeric_id());
            if loaded_species != species {
                loaded_species = species;
                player_sprite = load_sprite(species.0, true).await;
                enemy_sprite = load_sprite(species.1, false).await;
            }

            if let Some(platform) = &enemy_platform {
                draw_texture_ex(
                    platform,
                    510.0,
                    160.0,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(390.0, 125.0)),
                        ..Default::default()
                    },
                );
            }
            if let Some(platform) = &player_platform {
                draw_texture_ex(
                    platform,
                    70.0,
                    320.0,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(440.0, 150.0)),
                        ..Default::default()
                    },
                );
            }
            draw_sprite(enemy_sprite.as_ref(), 640.0, 96.0, 3.0, false);
            draw_sprite(player_sprite.as_ref(), 190.0, 235.0, 4.0, true);
            draw_health_card(
                enemy.species.name(),
                enemy.level,
                enemy.hp,
                enemy.max_hp,
                590.0,
                34.0,
            );
            draw_health_card(
                player.species.name(),
                player.level,
                player.hp,
                player.max_hp,
                74.0,
                380.0,
            );

            if is_key_pressed(KeyCode::Key1)
                || is_key_pressed(KeyCode::Key2)
                || is_key_pressed(KeyCode::Key3)
                || is_key_pressed(KeyCode::Key4)
            {
                let index = match get_last_key_pressed() {
                    Some(KeyCode::Key2) => 1,
                    Some(KeyCode::Key3) => 2,
                    Some(KeyCode::Key4) => 3,
                    _ => 0,
                };
                message = match current.take_action(PlayerAction::UseMove(index)) {
                    Ok(()) => action_message(&current.battle.last_events),
                    Err(error) => format!("That action is not available: {error:?}"),
                };
            }
            if matches!(
                current.stage,
                MiniRunStage::Reward | MiniRunStage::Complete | MiniRunStage::Lost
            ) {
                message = format!(
                    "Run status: {:?}. Press R to start a fresh run.",
                    current.stage
                );
            }
        }

        draw_rectangle(0.0, 0.0, 960.0, 33.0, Color::from_rgba(28, 51, 63, 235));
        draw_text("POKEROGUE RUST · WEB DEMO", 22.0, 24.0, 20.0, WHITE);
        draw_rectangle(
            28.0,
            468.0,
            904.0,
            54.0,
            Color::from_rgba(251, 251, 241, 245),
        );
        draw_rectangle_lines(
            28.0,
            468.0,
            904.0,
            54.0,
            3.0,
            Color::from_rgba(48, 72, 88, 255),
        );
        let lines = wrap_message(&message, 88);
        draw_text(
            lines.first().map_or("", String::as_str),
            46.0,
            489.0,
            15.0,
            DARKBROWN,
        );
        draw_text(
            lines.get(1).map_or("", String::as_str),
            46.0,
            507.0,
            15.0,
            DARKBROWN,
        );
        draw_text("1–4: move   ·   R: restart", 726.0, 518.0, 12.0, DARKBROWN);

        if is_key_pressed(KeyCode::R) {
            run = MiniRun::new_with_wave_limit("M12Speed0016", "DAY", 10).ok();
            loaded_species = (0, 0);
            message = "Wave 1 · Town".to_owned();
        }
        next_frame().await;
    }
}
