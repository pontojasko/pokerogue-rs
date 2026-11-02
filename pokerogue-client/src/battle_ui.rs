//! Battle UI composed from the pinned Phaser source's 320×180 UI coordinates.
//! The game framebuffer is 960×540, so the single transform is a 3× scale.
use crate::hud::Hud;
use macroquad::prelude::*;

const SCALE: f32 = 3.0;
const WINDOW_BORDER: f32 = 8.0 * SCALE;
const BALL_NAMES: [&str; 5] = [
    "Poké Ball",
    "Great Ball",
    "Ultra Ball",
    "Rogue Ball",
    "Master Ball",
];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct BallWigglePose {
    x: f32,
    angle_degrees: f32,
}

fn ball_wiggle_pose(elapsed: f64) -> BallWigglePose {
    // BallUiHandler.startPokeballWiggle: two 150 ms Cubic.easeOut yoyos,
    // a 250 ms delay between them, then a 1000 ms chain loop delay.
    const HALF: f64 = 0.150;
    const SECOND_START: f64 = 0.550;
    const SECOND_END: f64 = 0.850;
    const LOOP: f64 = 1.850;
    let t = elapsed.max(0.0) % LOOP;
    let ease_out = |value: f64| 1.0 - (1.0 - value.clamp(0.0, 1.0)).powi(3);
    let (progress, direction) = if t < HALF {
        (ease_out(t / HALF), 1.0)
    } else if t < HALF * 2.0 {
        (ease_out(1.0 - (t - HALF) / HALF), 1.0)
    } else if t < SECOND_START {
        return BallWigglePose::default();
    } else if t < SECOND_START + HALF {
        (ease_out((t - SECOND_START) / HALF), -1.0)
    } else if t < SECOND_END {
        (ease_out(1.0 - (t - SECOND_START - HALF) / HALF), -1.0)
    } else {
        return BallWigglePose::default();
    };
    BallWigglePose {
        x: 4.0 * progress as f32 * direction,
        angle_degrees: 27.5 * progress as f32 * direction,
    }
}

pub fn move_ball_cursor(cursor: usize, up: bool, down: bool) -> usize {
    if up {
        if cursor == 0 { 5 } else { cursor - 1 }
    } else if down {
        if cursor == 5 { 0 } else { cursor + 1 }
    } else {
        cursor
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BattleUiState {
    #[default]
    Command,
    Moves,
    Balls,
    Party,
    Message,
}

pub struct MoveInfo {
    pub type_name: String,
    pub pp: u8,
    pub max_pp: u8,
    pub power: i16,
    pub accuracy: i16,
    pub is_status: bool,
    pub effectiveness: f32,
}

pub struct PartyMemberView<'a> {
    pub name: &'a str,
    pub species_id: u16,
    pub level: u8,
    pub hp: i32,
    pub max_hp: i32,
    pub gender: pokerogue_core::Gender,
}

pub struct PartyUiAssets<'a> {
    pub background: Option<&'a Texture2D>,
    pub main_slot: Option<&'a crate::hud::Atlas>,
    pub bench_slot: Option<&'a crate::hud::Atlas>,
    pub pokeball: Option<&'a crate::hud::Atlas>,
    pub hp_bar: Option<&'a Texture2D>,
    pub hp_overlay: Option<&'a crate::hud::Atlas>,
    pub hp_label: Option<&'a Texture2D>,
    pub level_label: Option<&'a Texture2D>,
    pub cancel: Option<&'a crate::hud::Atlas>,
    pub icon_atlases: &'a [Option<crate::hud::Atlas>],
}

fn draw_window(texture: Option<&Texture2D>, x: f32, y: f32, width: f32, height: f32) {
    let Some(texture) = texture else {
        draw_rectangle(x, y, width, height, Color::from_rgba(52, 56, 70, 255));
        return;
    };
    let border = WINDOW_BORDER;
    let inner_w = (width - border * 2.0).max(0.0);
    let inner_h = (height - border * 2.0).max(0.0);
    for row in 0..3 {
        for column in 0..3 {
            let sx = column as f32 * 8.0;
            let sy = row as f32 * 8.0;
            let dx = x + match column {
                0 => 0.0,
                1 => border,
                _ => width - border,
            };
            let dy = y + match row {
                0 => 0.0,
                1 => border,
                _ => height - border,
            };
            draw_texture_ex(
                texture,
                dx,
                dy,
                WHITE,
                DrawTextureParams {
                    source: Some(Rect::new(sx, sy, 8.0, 8.0)),
                    dest_size: Some(vec2(
                        if column == 1 { inner_w } else { border },
                        if row == 1 { inner_h } else { border },
                    )),
                    ..Default::default()
                },
            );
        }
    }
}

fn draw_message_bg(texture: Option<&Texture2D>) {
    if let Some(texture) = texture {
        draw_texture_ex(
            texture,
            0.0,
            396.0,
            WHITE,
            DrawTextureParams {
                source: Some(Rect::new(0.0, 0.0, 320.0, 48.0)),
                dest_size: Some(vec2(960.0, 144.0)),
                ..Default::default()
            },
        );
    }
}

// Phaser uses the official 6×10 cursor image at native scale in source space.
fn draw_cursor(texture: Option<&Texture2D>, center_x: f32, center_y: f32) {
    if let Some(texture) = texture {
        draw_texture_ex(
            texture,
            center_x - texture.width() * SCALE / 2.0,
            center_y - texture.height() * SCALE / 2.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(texture.width() * SCALE, texture.height() * SCALE)),
                ..Default::default()
            },
        );
    }
}

fn draw_command_menu(
    message_bg: Option<&Texture2D>,
    window: Option<&Texture2D>,
    hud: &Hud,
    cursor_texture: Option<&Texture2D>,
    pokemon_name: &str,
    command_cursor: usize,
) {
    draw_message_bg(message_bg);
    draw_window(window, 202.0 * SCALE, 396.0, 118.0 * SCALE, 48.0 * SCALE);
    for (index, label) in ["Fight", "Ball", "Pokémon", "Run"].iter().enumerate() {
        let x = 217.0 * SCALE + if index % 2 == 1 { 55.8 * SCALE } else { 0.0 };
        let y = 423.9 + if index >= 2 { 16.0 * SCALE } else { 0.0 };
        if index == command_cursor {
            let center_x = (212.0 + (index % 2) as f32 * 56.0) * SCALE;
            let center_y = (149.3 + (index / 2) as f32 * 16.0) * SCALE;
            draw_cursor(cursor_texture, center_x, center_y);
        }
        hud.window_text(label, x, y);
    }
    hud.window_text("What will", 12.0 * SCALE, 423.0);
    hud.window_text(&format!("{pokemon_name} do?"), 12.0 * SCALE, 468.0);
}

fn ball_menu_widths(hud: &Hud, counts: &[u8; 5]) -> (f32, f32, f32) {
    let Some(font) = &hud.font else {
        return (80.0, 45.0, 20.0);
    };
    let label_width = BALL_NAMES
        .iter()
        .map(|name| measure_text(name, Some(font), 96, 0.5).width / SCALE)
        .fold(0.0, f32::max);
    let counts_width = counts
        .iter()
        .map(|count| measure_text(&format!("×{count}"), Some(font), 96, 0.5).width / SCALE)
        .fold(0.0, f32::max);
    let cancel_width = measure_text("Cancel", Some(font), 96, 0.5).width / SCALE;
    (
        64.0_f32
            .max(label_width + 4.0 + counts_width)
            .max(cancel_width),
        label_width,
        counts_width,
    )
}

fn draw_ball_menu(
    window: Option<&Texture2D>,
    cursor_texture: Option<&Texture2D>,
    ball_atlas: Option<&crate::hud::Atlas>,
    hud: &Hud,
    cursor: usize,
    counts: &[u8; 5],
    animation_elapsed: f64,
) {
    let (content_width, label_width, counts_width) = ball_menu_widths(hud, counts);
    let x = (320.0 - 51.0 - content_width) * SCALE;
    let height = 32.0 + 480.0 / 6.0;
    let y = (180.0 - 49.0 - height) * SCALE;
    let width = (50.0 + content_width) * SCALE;
    draw_window(window, x, y, width, height * SCALE);

    let pose = ball_wiggle_pose(animation_elapsed);
    for index in 0..5 {
        let row_y = 9.0 + 16.0 * index as f32;
        let icon_x = (x / SCALE + 24.0 + if index == cursor { pose.x } else { 0.0 }) * SCALE;
        let icon_y = (y / SCALE + 15.0 + 16.0 * index as f32) * SCALE;
        if let Some(atlas) = ball_atlas {
            atlas.draw_centered_rotated(
                ["pb", "gb", "ub", "rb", "mb"][index],
                icon_x,
                icon_y,
                SCALE,
                if index == cursor {
                    pose.angle_degrees.to_radians()
                } else {
                    0.0
                },
            );
        }
        if index == cursor {
            draw_cursor(
                cursor_texture,
                (x / SCALE + 12.0) * SCALE,
                (y / SCALE + 16.0 + 16.0 * index as f32) * SCALE,
            );
        }
        hud.window_text(
            BALL_NAMES[index],
            (x / SCALE + 36.0) * SCALE,
            (y / SCALE + row_y) * SCALE,
        );
        let count = format!("×{}", counts[index]);
        let count_width = hud.font.as_ref().map_or_else(
            || count.chars().count() as f32 * 8.0,
            |font| measure_text(&count, Some(font), 96, 0.5).width / SCALE,
        );
        let count_x = x / SCALE + 36.0 + label_width + 4.0 + counts_width - count_width;
        hud.window_text(&count, count_x * SCALE, (y / SCALE + row_y) * SCALE);
    }
    let cancel_y = 9.0 + 5.0 * 16.0;
    if cursor == 5 {
        draw_cursor(
            cursor_texture,
            (x / SCALE + 12.0) * SCALE,
            (y / SCALE + 16.0 + 5.0 * 16.0) * SCALE,
        );
    }
    hud.window_text(
        "Cancel",
        (x / SCALE + 18.0) * SCALE,
        (y / SCALE + cancel_y) * SCALE,
    );
}

fn icon_generation(species_id: u16) -> Option<usize> {
    match species_id {
        1..=151 => Some(1),
        152..=251 => Some(2),
        252..=386 => Some(3),
        387..=493 => Some(4),
        494..=649 => Some(5),
        650..=721 => Some(6),
        722..=809 => Some(7),
        810..=905 => Some(8),
        906..=1025 => Some(9),
        _ => None,
    }
}

fn party_icon_bob_offset(selected: bool, time: f64) -> f32 {
    // PokemonIconAnimHelper: a 200 ms yoyo counter toggles on each alternate;
    // passive icons move up 1 source pixel and the selected icon 2.
    let toggled = (time.max(0.0) * 5.0).floor() as u64 % 2 == 1;
    if !toggled {
        0.0
    } else if selected {
        -2.0
    } else {
        -1.0
    }
}

fn party_text(
    hud: &Hud,
    text: &str,
    x: f32,
    y: f32,
    color: Color,
    shadow: Color,
    right_aligned: bool,
) {
    let Some(font) = &hud.font else {
        return;
    };
    let width = measure_text(text, Some(font), 96, 0.5).width;
    let x = if right_aligned { x - width } else { x };
    let baseline = y + 36.0;
    let params = TextParams {
        font: Some(font),
        font_size: 96,
        font_scale: 0.5,
        color,
        ..Default::default()
    };
    draw_text_ex(
        text,
        x + 1.5,
        baseline + 1.5,
        TextParams {
            color: shadow,
            ..params.clone()
        },
    );
    draw_text_ex(text, x, baseline, params);
}

fn draw_party_slot(
    slot_index: usize,
    selected: bool,
    member: &PartyMemberView<'_>,
    assets: &PartyUiAssets<'_>,
    hud: &Hud,
) {
    let fainted = member.hp <= 0;
    let (slot_x, slot_y, background, frame, name_pos, level_pos, hp_pos, gender_offset) =
        if slot_index == 0 {
            let frame = match (fainted, selected) {
                (false, false) => "party_slot_main",
                (false, true) => "party_slot_main_sel",
                (true, false) => "party_slot_main_fnt",
                (true, true) => "party_slot_main_fnt_sel",
            };
            (
                9.0,
                180.0 - 148.5,
                assets.main_slot,
                frame,
                vec2(24.0, 10.0),
                vec2(32.0, 22.0),
                vec2(8.0, 31.0),
                vec2(68.0, -9.0),
            )
        } else {
            let frame = match (fainted, selected) {
                (false, false) => "party_slot",
                (false, true) => "party_slot_sel",
                (true, false) => "party_slot_fnt",
                (true, true) => "party_slot_fnt_sel",
            };
            (
                143.0,
                180.0 - 196.0 + 28.0 * slot_index as f32,
                assets.bench_slot,
                frame,
                vec2(21.0, 2.0),
                vec2(29.0, 14.0),
                vec2(72.0, 6.0),
                vec2(36.0, 0.0),
            )
        };

    if let Some(background) = background {
        background.draw_at(frame, slot_x * SCALE, slot_y * SCALE, SCALE);
    }

    let (icon_offset_x, icon_offset_y) = if slot_index == 0 {
        (4.0, 4.0)
    } else {
        (2.0, 12.0)
    };
    let icon_x = slot_x + icon_offset_x;
    let icon_y = slot_y + icon_offset_y;
    if let Some(pokeball) = assets.pokeball {
        pokeball.draw_centered(
            if selected { "party_pb_sel" } else { "party_pb" },
            icon_x * SCALE,
            icon_y * SCALE,
            SCALE,
        );
    }
    if let Some(generation) = icon_generation(member.species_id)
        && let Some(Some(icons)) = assets.icon_atlases.get(generation)
    {
        icons.draw_centered(
            &member.species_id.to_string(),
            icon_x * SCALE,
            icon_y * SCALE + party_icon_bob_offset(selected, crate::visual::time()) * SCALE,
            SCALE,
        );
    }

    party_text(
        hud,
        member.name,
        (slot_x + name_pos.x) * SCALE,
        (slot_y + name_pos.y) * SCALE,
        Color::from_rgba(248, 248, 248, 255),
        Color::from_rgba(107, 90, 115, 255),
        false,
    );
    if let Some(level_label) = assets.level_label {
        draw_texture_ex(
            level_label,
            (slot_x + level_pos.x) * SCALE,
            (slot_y + level_pos.y) * SCALE,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(
                    level_label.width() * SCALE,
                    level_label.height() * SCALE,
                )),
                ..Default::default()
            },
        );
    }
    party_text(
        hud,
        &member.level.to_string(),
        (slot_x + level_pos.x + 9.0) * SCALE,
        (slot_y + level_pos.y) * SCALE,
        Color::from_rgba(248, 248, 248, 255),
        Color::from_rgba(107, 90, 115, 255),
        false,
    );
    if let Some(symbol) = member.gender.symbol() {
        let (color, shadow) = match member.gender {
            pokerogue_core::Gender::Male => (
                Color::from_rgba(64, 200, 248, 255),
                Color::from_rgba(0, 96, 144, 255),
            ),
            pokerogue_core::Gender::Female => (
                Color::from_rgba(248, 152, 144, 255),
                Color::from_rgba(152, 64, 56, 255),
            ),
            pokerogue_core::Gender::Genderless => unreachable!(),
        };
        party_text(
            hud,
            symbol,
            (slot_x + level_pos.x + gender_offset.x) * SCALE,
            (slot_y + level_pos.y + gender_offset.y) * SCALE,
            color,
            shadow,
            false,
        );
    }

    if let Some(hp_label) = assets.hp_label {
        let x = slot_x + hp_pos.x + 15.0 - hp_label.width();
        draw_texture_ex(
            hp_label,
            x * SCALE,
            (slot_y + hp_pos.y) * SCALE,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(hp_label.width() * SCALE, hp_label.height() * SCALE)),
                ..Default::default()
            },
        );
    }
    if let Some(hp_bar) = assets.hp_bar {
        draw_texture_ex(
            hp_bar,
            (slot_x + hp_pos.x) * SCALE,
            (slot_y + hp_pos.y) * SCALE,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(hp_bar.width() * SCALE, hp_bar.height() * SCALE)),
                ..Default::default()
            },
        );
    }
    let hp_ratio = if member.max_hp > 0 {
        (member.hp as f32 / member.max_hp as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    if hp_ratio > 0.0
        && let Some(hp_overlay) = assets.hp_overlay
    {
        let frame = if hp_ratio > 0.5 {
            "high"
        } else if hp_ratio > 0.25 {
            "medium"
        } else {
            "low"
        };
        if let Some(source) = hp_overlay.frame(frame) {
            draw_texture_ex(
                &hp_overlay.texture,
                (slot_x + hp_pos.x + 16.0) * SCALE,
                (slot_y + hp_pos.y + 2.0) * SCALE,
                WHITE,
                DrawTextureParams {
                    source: Some(source),
                    dest_size: Some(vec2(source.w * SCALE * hp_ratio, source.h * SCALE)),
                    ..Default::default()
                },
            );
        }
    }
    party_text(
        hud,
        &format!("{}/{}", member.hp.max(0), member.max_hp),
        (slot_x + hp_pos.x + 100.0 - 3.0) * SCALE,
        (slot_y + hp_pos.y + 7.0 - 2.0) * SCALE,
        Color::from_rgba(248, 248, 248, 255),
        Color::from_rgba(107, 90, 115, 255),
        true,
    );
}

fn draw_party_selection(
    background: Option<&Texture2D>,
    window: Option<&Texture2D>,
    assets: &PartyUiAssets<'_>,
    hud: &Hud,
    party: &[PartyMemberView<'_>],
    cursor: usize,
) {
    if let Some(background) = assets.background.or(background) {
        draw_texture_ex(
            background,
            0.0,
            0.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(320.0 * SCALE, 180.0 * SCALE)),
                ..Default::default()
            },
        );
    } else {
        draw_rectangle(
            0.0,
            0.0,
            320.0 * SCALE,
            180.0 * SCALE,
            Color::from_rgba(15, 24, 30, 255),
        );
    }
    for (index, member) in party.iter().take(6).enumerate() {
        draw_party_slot(index, index == cursor, member, assets, hud);
    }

    draw_window(window, SCALE, 149.0 * SCALE, 262.0 * SCALE, 30.0 * SCALE);
    if cursor != 6 {
        hud.window_text("Choose a Pokémon.", 10.0 * SCALE, 156.0 * SCALE);
    }

    let cancel_x = 291.0;
    let cancel_y = 180.0 - 16.0;
    if let Some(cancel) = assets.cancel {
        cancel.draw_centered(
            if cursor == 6 {
                "party_cancel_sel"
            } else {
                "party_cancel"
            },
            cancel_x * SCALE,
            cancel_y * SCALE,
            SCALE,
        );
    }
    if let Some(pokeball) = assets.pokeball {
        pokeball.draw_centered(
            if cursor == 6 {
                "party_pb_sel"
            } else {
                "party_pb"
            },
            (cancel_x - 17.0) * SCALE,
            cancel_y * SCALE,
            SCALE,
        );
    }
    hud.window_text(
        "Cancel",
        (cancel_x - 10.0) * SCALE,
        (cancel_y - 7.0) * SCALE,
    );
}

#[cfg(test)]
mod party_animation_tests {
    use super::{ball_wiggle_pose, move_ball_cursor, party_icon_bob_offset};

    #[test]
    fn party_icons_use_source_active_and_passive_bob_amplitudes() {
        assert_eq!(party_icon_bob_offset(false, 0.0), 0.0);
        assert_eq!(party_icon_bob_offset(true, 0.0), 0.0);
        assert_eq!(party_icon_bob_offset(false, 0.2), -1.0);
        assert_eq!(party_icon_bob_offset(true, 0.2), -2.0);
        assert_eq!(party_icon_bob_offset(false, 0.4), 0.0);
        assert_eq!(party_icon_bob_offset(true, 0.4), 0.0);
    }

    #[test]
    fn ball_list_cursor_wraps_through_five_balls_and_cancel() {
        assert_eq!(move_ball_cursor(0, true, false), 5);
        assert_eq!(move_ball_cursor(5, false, true), 0);
        assert_eq!(move_ball_cursor(2, false, true), 3);
        assert_eq!(move_ball_cursor(2, false, false), 2);
        assert_eq!(move_ball_cursor(2, true, true), 1);
    }

    #[test]
    fn pokeball_wiggle_matches_source_chain_timing_and_extents() {
        assert_eq!(ball_wiggle_pose(0.0), Default::default());
        let peak = ball_wiggle_pose(0.15);
        assert_eq!(peak.x, 4.0);
        assert_eq!(peak.angle_degrees, 27.5);
        let reverse_midpoint = ball_wiggle_pose(0.225);
        assert_eq!(reverse_midpoint.x, 3.5);
        assert_eq!(reverse_midpoint.angle_degrees, 24.0625);
        assert_eq!(ball_wiggle_pose(0.30), Default::default());
        let second_peak = ball_wiggle_pose(0.70);
        assert_eq!(second_peak.x, -4.0);
        assert_eq!(second_peak.angle_degrees, -27.5);
        let second_reverse_midpoint = ball_wiggle_pose(0.775);
        assert_eq!(second_reverse_midpoint.x, -3.5);
        assert_eq!(second_reverse_midpoint.angle_degrees, -24.0625);
        assert_eq!(ball_wiggle_pose(0.85), Default::default());
        assert_eq!(ball_wiggle_pose(1.85), Default::default());
    }
}

pub fn draw(
    state: BattleUiState,
    message_bg: Option<&Texture2D>,
    window: Option<&Texture2D>,
    hud: &Hud,
    move_types: Option<&crate::hud::Atlas>,
    move_categories: Option<&crate::hud::Atlas>,
    cursor_texture: Option<&Texture2D>,
    pokemon_name: &str,
    dialogue: &str,
    move_names: &[&str],
    move_effectiveness: &[f32],
    move_is_status: &[bool],
    selected_move: usize,
    command_cursor: usize,
    ball_cursor: usize,
    ball_atlas: Option<&crate::hud::Atlas>,
    ball_wiggle_elapsed: f64,
    party_cursor: usize,
    party: &[PartyMemberView<'_>],
    party_assets: &PartyUiAssets<'_>,
    ball_counts: [u8; 5],
    selected_move_info: Option<&MoveInfo>,
    enemy_offset_x: f32,
) {
    match state {
        BattleUiState::Command => {
            draw_command_menu(
                message_bg,
                window,
                hud,
                cursor_texture,
                pokemon_name,
                command_cursor,
            );
        }
        BattleUiState::Moves => {
            // BattleMessageUiHandler.setup: 243×48 + right 80×48 move details.
            draw_window(window, 0.0, 396.0, 243.0 * SCALE, 48.0 * SCALE);
            draw_window(window, 240.0 * SCALE, 396.0, 80.0 * SCALE, 48.0 * SCALE);
            // FightUiHandler: moves container (18, -38.7), labels at (0|114, 0|16).
            for index in 0..4 {
                let label = move_names.get(index).copied().unwrap_or("-");
                let x = 18.0 * SCALE + if index % 2 == 1 { 114.0 * SCALE } else { 0.0 };
                let y = 423.9 + if index >= 2 { 16.0 * SCALE } else { 0.0 };
                if index == selected_move {
                    // FightUiHandler.setCursor(13 + column*114, -31 + row*15)
                    // positions the image from the bottom anchored UI origin.
                    let center_x = (13.0 + (index % 2) as f32 * 114.0) * SCALE;
                    let center_y = (149.0 + (index / 2) as f32 * 15.0) * SCALE;
                    draw_cursor(cursor_texture, center_x, center_y);
                }
                let color = move_effectiveness
                    .get(index)
                    .copied()
                    .zip(move_is_status.get(index).copied())
                    .and_then(|(effectiveness, is_status)| {
                        type_hint_color(effectiveness, is_status)
                    })
                    .unwrap_or(Color::from_rgba(248, 248, 248, 255));
                hud.window_text_color(label, x, y, color);
            }
            if let Some(info) = selected_move_info {
                if let Some(atlas) = move_types {
                    atlas.draw_centered(&info.type_name.to_lowercase(), 792.0, 432.0, 2.4);
                }
                let category = if info.is_status { "status" } else { "physical" };
                if let Some(atlas) = move_categories {
                    atlas.draw_centered(category, 888.0, 432.0, 3.0);
                }
                for (line, (label, value)) in [
                    ("PP", format!("{}/{}", info.pp, info.max_pp)),
                    ("Power", info.power.to_string()),
                    ("Accuracy", info.accuracy.to_string()),
                ]
                .iter()
                .enumerate()
                {
                    let center_y = 462.0 + line as f32 * 24.0;
                    hud.detail_text(label, 753.0, center_y, false);
                    hud.detail_text(value, 927.0, center_y, true);
                }
                draw_effectiveness(window, hud, Some(info), enemy_offset_x);
            }
        }
        BattleUiState::Message => {
            draw_message_bg(message_bg);
            for (line, text) in dialogue.lines().take(2).enumerate() {
                hud.window_text(text, 12.0 * SCALE, 423.0 + line as f32 * 45.0);
            }
        }
        BattleUiState::Balls => {
            // The original keeps the command box open underneath the ball list.
            draw_command_menu(
                message_bg,
                window,
                hud,
                cursor_texture,
                pokemon_name,
                command_cursor,
            );
            draw_ball_menu(
                window,
                cursor_texture,
                ball_atlas,
                hud,
                ball_cursor,
                &ball_counts,
                ball_wiggle_elapsed,
            );
        }
        BattleUiState::Party => {
            draw_party_selection(message_bg, window, party_assets, hud, party, party_cursor);
        }
    }
}

// Mirrors getTypeDamageMultiplierColor(..., "offense") in the pinned source.
fn type_hint_color(effectiveness: f32, is_status: bool) -> Option<Color> {
    if is_status && effectiveness != 0.0 {
        return None;
    }
    let (r, g, b) = if effectiveness == 0.0 {
        (0x92, 0x92, 0x92)
    } else if effectiveness == 0.125 {
        (0xff, 0x55, 0x00)
    } else if effectiveness == 0.25 {
        (0xff, 0x74, 0x00)
    } else if effectiveness == 0.5 {
        (0xfe, 0x8e, 0x00)
    } else if effectiveness == 2.0 {
        (0x4a, 0xa5, 0x00)
    } else if effectiveness == 4.0 {
        (0x4b, 0xb4, 0x00)
    } else if effectiveness == 8.0 {
        (0x52, 0xc2, 0x00)
    } else {
        return None;
    };
    Some(Color::from_rgba(r, g, b, 255))
}

fn draw_effectiveness(
    window: Option<&Texture2D>,
    hud: &Hud,
    move_info: Option<&MoveInfo>,
    enemy_offset_x: f32,
) {
    let Some(info) = move_info else {
        return;
    };
    let value = format!("{}×", info.effectiveness);
    let text_width = hud.font.as_ref().map_or(value.len() as f32 * 12.0, |font| {
        measure_text(&value, Some(font), 48, 0.5).width
    });
    let x = (140.0 - 15.0 + 22.0) * SCALE + enemy_offset_x;
    let y = (180.0 - 141.0 - 15.5 + 4.0) * SCALE;
    let width = 30.0 + text_width;
    let height = 60.0;
    // EnemyBattleInfo creates a 20 logical-pixel XTHIN window with a 10px
    // horizontal inset around its multiplier label.
    if window.is_some() {
        draw_rectangle(x, y, width, height, Color::from_rgba(158, 52, 42, 255));
        draw_rectangle(
            x + 3.0,
            y + 3.0,
            width - 6.0,
            height - 6.0,
            Color::from_rgba(43, 37, 50, 255),
        );
        draw_rectangle(
            x + 6.0,
            y + 6.0,
            width - 12.0,
            height - 12.0,
            Color::from_rgba(52, 46, 60, 255),
        );
    } else {
        draw_rectangle(x, y, width, height, Color::from_rgba(52, 46, 60, 255));
    }
    hud.detail_text(&value, x + 15.0, y + 13.0, false);
}
