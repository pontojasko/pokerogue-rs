use macroquad::prelude::*;
use pokerogue_core::{BattleActor, BattleEvent, MiniRun, MiniRunStage, PlayerAction};
use pokerogue_data::Species;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

mod audio;
mod battle_ui;
mod generated_battle_anims;
mod generated_capture_particles;
mod generated_pokeballs;
mod generated_sprites;
mod hud;
mod title_screen;
mod visual;

const BG: Color = Color::from_rgba(239, 239, 222, 255);
const PANEL: Color = Color::from_rgba(255, 255, 248, 255);
const INK: Color = Color::from_rgba(43, 54, 49, 255);
const GREEN: Color = Color::from_rgba(79, 162, 96, 255);
const LOGICAL_WIDTH: f32 = 960.0;
const LOGICAL_HEIGHT: f32 = 540.0;
const FIELD_PIXELS_PER_SOURCE_UNIT: f32 = 3.0;
const BATTLE_ANIMATION_FRAME_SECONDS: f64 = 0.05;

fn project_field_layer(source_x: f32, texture_width: f32, texture_height: f32) -> Rect {
    Rect::new(
        source_x * FIELD_PIXELS_PER_SOURCE_UNIT,
        0.0,
        texture_width * FIELD_PIXELS_PER_SOURCE_UNIT,
        texture_height * FIELD_PIXELS_PER_SOURCE_UNIT,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EncounterEntranceKind {
    First,
    NextWave,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FieldPresentation {
    player_x: f32,
    enemy_x: f32,
    player_platform_x: f32,
    enemy_platform_x: f32,
}

fn encounter_entrance(kind: EncounterEntranceKind, progress: f32) -> FieldPresentation {
    let progress = progress.clamp(0.0, 1.0);
    let remaining = 1.0 - progress;
    FieldPresentation {
        player_x: if kind == EncounterEntranceKind::First {
            300.0 * remaining
        } else {
            0.0
        },
        enemy_x: -300.0 * remaining,
        player_platform_x: if kind == EncounterEntranceKind::First {
            300.0 * remaining
        } else {
            0.0
        },
        enemy_platform_x: -280.0 + 300.0 * progress,
    }
}

fn final_field_presentation() -> FieldPresentation {
    encounter_entrance(EncounterEntranceKind::NextWave, 1.0)
}

struct EncounterEntrance {
    kind: EncounterEntranceKind,
    started_at: f64,
}

impl EncounterEntrance {
    const DURATION_SECONDS: f64 = 2.0;

    fn presentation(&self, now: f64) -> FieldPresentation {
        let progress = ((now - self.started_at) / Self::DURATION_SECONDS) as f32;
        encounter_entrance(self.kind, progress)
    }
}

fn show_first_entry_message(kind: EncounterEntranceKind, elapsed: f64) -> bool {
    kind == EncounterEntranceKind::First
        && (0.0..EncounterEntrance::DURATION_SECONDS).contains(&elapsed)
}

fn window_conf() -> Conf {
    #[cfg(not(target_arch = "wasm32"))]
    let visual_battle = std::env::var_os("POKEROGUE_VISUAL_BATTLE").is_some();
    #[cfg(target_arch = "wasm32")]
    let visual_battle = false;
    #[cfg(not(target_arch = "wasm32"))]
    let visual_fixture = std::env::args().any(|s| s == "--visual-fixture");
    #[cfg(target_arch = "wasm32")]
    let visual_fixture = false;
    #[cfg(target_os = "linux")]
    let linux_backend = if std::env::var_os("POKEROGUE_FORCE_WAYLAND").is_some() {
        miniquad::conf::LinuxBackend::WaylandOnly
    } else if visual_fixture
        || std::env::var_os("POKEROGUE_FORCE_X11").is_some()
        || std::env::var_os("WAYLAND_DISPLAY").is_none()
    {
        miniquad::conf::LinuxBackend::X11Only
    } else {
        miniquad::conf::LinuxBackend::WaylandOnly
    };
    Conf {
        window_title: "PokéRogue · Rust prototype".into(),
        window_width: 960,
        window_height: 540,
        fullscreen: visual_battle && !visual_fixture,
        #[cfg(target_os = "linux")]
        platform: miniquad::conf::Platform {
            linux_backend,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Phaser uses a 320x180 scaled canvas at 6x; the Rust layout uses an equivalent
/// 960x540 canvas at 2x. Fit it without stretching, including 640x480 displays.
fn fitted_viewport(screen_w: f32, screen_h: f32) -> (i32, i32, i32, i32) {
    if screen_w <= 0.0 || screen_h <= 0.0 {
        return (0, 0, 0, 0);
    }
    let scale = (screen_w / LOGICAL_WIDTH).min(screen_h / LOGICAL_HEIGHT);
    let width = (LOGICAL_WIDTH * scale).round() as i32;
    let height = (LOGICAL_HEIGHT * scale).round() as i32;
    (
        (screen_w.round() as i32 - width) / 2,
        (screen_h.round() as i32 - height) / 2,
        width,
        height,
    )
}

fn image_at_logical_viewport(image: &Image, viewport: (i32, i32, i32, i32)) -> Image {
    let (x, y, width, height) = viewport;
    assert!(x >= 0 && y >= 0 && width > 0 && height > 0);
    assert!(x + width <= i32::from(image.width) && y + height <= i32::from(image.height));
    let source_stride = usize::from(image.width) * 4;
    let mut bytes = vec![0; LOGICAL_WIDTH as usize * LOGICAL_HEIGHT as usize * 4];
    for output_y in 0..LOGICAL_HEIGHT as usize {
        let source_y = y as usize + output_y * height as usize / LOGICAL_HEIGHT as usize;
        for output_x in 0..LOGICAL_WIDTH as usize {
            let source_x = x as usize + output_x * width as usize / LOGICAL_WIDTH as usize;
            let source = source_y * source_stride + source_x * 4;
            let target = (output_y * LOGICAL_WIDTH as usize + output_x) * 4;
            bytes[target..target + 4].copy_from_slice(&image.bytes[source..source + 4]);
        }
    }
    Image {
        bytes,
        width: LOGICAL_WIDTH as u16,
        height: LOGICAL_HEIGHT as u16,
    }
}

fn logical_camera() -> Camera2D {
    let mut camera =
        Camera2D::from_display_rect(Rect::new(0.0, 0.0, LOGICAL_WIDTH, LOGICAL_HEIGHT));
    // Macroquad's display rect uses a bottom-left world origin; the client layout
    // is authored in screen coordinates (origin at top-left).
    camera.zoom.y = camera.zoom.y.abs();
    camera
}

fn set_logical_viewport() {
    let (screen_w, screen_h) = (screen_width(), screen_height());
    let mut camera = logical_camera();
    camera.viewport = Some(fitted_viewport(screen_w, screen_h));
    set_camera(&camera);
}

struct SpriteAtlas {
    hud: hud::Hud,
    move_types: Option<hud::Atlas>,
    move_categories: Option<hud::Atlas>,
    player_type: Option<hud::Atlas>,
    player_type1: Option<hud::Atlas>,
    player_type2: Option<hud::Atlas>,
    enemy_type: Option<hud::Atlas>,
    enemy_type1: Option<hud::Atlas>,
    enemy_type2: Option<hud::Atlas>,
    item_icons: Option<hud::Atlas>,
    reward_namebox: Option<hud::Atlas>,
    owned_icon: Option<Texture2D>,
    menu_cursor: Option<Texture2D>,
    title_logo: Option<Texture2D>,
    title_background: Option<Texture2D>,
    halloween_logo: Option<Texture2D>,
    halloween_banner: Option<Texture2D>,
    town_background: Option<Texture2D>,
    town_player_platform: Option<Texture2D>,
    town_enemy_platform: Option<Texture2D>,
    message_window: Option<Texture2D>,
    window_border: Option<Texture2D>,
    player_info: Option<Texture2D>,
    enemy_info: Option<Texture2D>,
    enemy_boss_info: Option<Texture2D>,
    hp_bar: Option<Texture2D>,
    boss_hp_bar: Option<Texture2D>,
    battle_anim_strike: Option<Texture2D>,
    battle_anim_fire: Option<Texture2D>,
    battle_anim_health_up: Option<Texture2D>,
    health_up_frames: Vec<Vec<HealthUpGraphicFrame>>,
    pokeball_atlas: Option<Texture2D>,
    ball_menu_atlas: Option<hud::Atlas>,
    pokeball_particles: Option<Texture2D>,
    party_background: Option<Texture2D>,
    party_main_slot: Option<hud::Atlas>,
    party_bench_slot: Option<hud::Atlas>,
    party_pokeball: Option<hud::Atlas>,
    party_hp_bar: Option<Texture2D>,
    party_hp_overlay: Option<hud::Atlas>,
    party_hp_label: Option<Texture2D>,
    party_level_label: Option<Texture2D>,
    party_cancel: Option<hud::Atlas>,
    pokemon_icon_atlases: Vec<Option<hud::Atlas>>,
    pokemon_root: PathBuf,
    active_player: Option<AnimatedSprite>,
    outgoing_player: Option<AnimatedSprite>,
    active_enemy: Option<AnimatedSprite>,
}

#[derive(Clone)]
struct RewardCard {
    name: String,
    icon: String,
    description: String,
    tier: u8,
}

async fn load_texture_nearest(path: &Path) -> Option<Texture2D> {
    let texture = load_texture(&path.to_string_lossy()).await.ok()?;
    texture.set_filter(FilterMode::Nearest);
    Some(texture)
}

async fn load_local_atlas(roots: &[PathBuf], relative: &str) -> Option<hud::Atlas> {
    for root in roots {
        let image = root.join(format!("{relative}.png"));
        let metadata = root.join(format!("{relative}.json"));
        if let Some(atlas) = hud::Atlas::load_at(&image, &metadata).await {
            return Some(atlas);
        }
    }
    None
}

async fn load_local_texture(roots: &[PathBuf], relative: &str) -> Option<Texture2D> {
    for root in roots {
        if let Some(texture) = load_texture_nearest(&root.join(relative)).await {
            return Some(texture);
        }
    }
    None
}

async fn load_phaser_scene_texture(path: &Path) -> Option<Texture2D> {
    let bytes = load_file(&path.to_string_lossy()).await.ok()?;
    let mut image = Image::from_file_with_format(&bytes, None).ok()?;
    // Matching opaque pixels from the Town field and Charmander against live
    // Phaser output shows a one-code white blend: channels below 128 round up.
    for rgba in image.bytes.chunks_exact_mut(4) {
        for channel in &mut rgba[..3] {
            *channel = phaser_canvas_channel(*channel);
        }
    }
    let texture = Texture2D::from_image(&image);
    texture.set_filter(FilterMode::Nearest);
    Some(texture)
}

async fn load_pokeball_tint_texture(path: &Path) -> Option<Texture2D> {
    let bytes = load_file(&path.to_string_lossy()).await.ok()?;
    let mut image = Image::from_file_with_format(&bytes, None).ok()?;
    for rgba in image.bytes.chunks_exact_mut(4) {
        if rgba[3] != 0 {
            rgba[..3].copy_from_slice(&[0xd5, 0x29, 0x29]);
        }
    }
    let texture = Texture2D::from_image(&image);
    texture.set_filter(FilterMode::Nearest);
    Some(texture)
}

fn phaser_canvas_channel(channel: u8) -> u8 {
    channel.saturating_add(u8::from(channel < 128))
}

struct AnimatedSprite {
    species_id: u16,
    texture: Texture2D,
    pokeball_tint: Option<Texture2D>,
    frames: &'static [generated_sprites::SpriteFrame],
    started_at: f64,
}

#[derive(Clone, Copy)]
struct BattleAnimationRequest {
    move_id: u16,
    actor: BattleActor,
    starts_at: f64,
}

struct ActiveBattleAnimation {
    request: BattleAnimationRequest,
    started_at: f64,
}

#[derive(Clone, Copy)]
struct ActiveDamageAnimation {
    target: BattleActor,
    amount: i32,
    critical: bool,
    effectiveness: pokerogue_core::DamageEffectiveness,
    started_at: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ActiveHpBarAnimation {
    actor: BattleActor,
    party_slot: usize,
    from_hp: i32,
    to_hp: i32,
    max_hp: i32,
    started_at: f64,
    duration: f64,
}

struct BattleHpSnapshot {
    party_hp: Vec<i32>,
    player_active_slot: usize,
    enemy_hp: i32,
}

impl BattleHpSnapshot {
    fn capture(battle: &pokerogue_core::Battle) -> Self {
        Self {
            party_hp: (0..battle.player.len())
                .filter_map(|slot| battle.player.member(slot).map(|member| member.hp))
                .collect(),
            player_active_slot: battle.player.active_slot(),
            enemy_hp: battle.enemy.hp,
        }
    }
}

#[derive(Clone, Copy)]
struct ActiveHealAnimation {
    party_slot: usize,
    amount: i32,
    started_at: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct HealthUpGraphicFrame {
    frame: u16,
    x: f32,
    y: f32,
    zoom_x: f32,
    zoom_y: f32,
    visible: bool,
    opacity: u8,
    focus: u8,
}

fn parse_health_up_graphic_frames(bytes: &[u8]) -> Option<Vec<Vec<HealthUpGraphicFrame>>> {
    let source: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let frames = source.get("frames")?.as_array()?;
    Some(
        frames
            .iter()
            .map(|frame| {
                frame
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|entry| entry.get("target").and_then(|value| value.as_u64()) == Some(2))
                    .filter_map(|entry| {
                        Some(HealthUpGraphicFrame {
                            frame: u16::try_from(entry.get("graphicFrame")?.as_u64()?).ok()?,
                            x: entry.get("x")?.as_f64()? as f32,
                            y: entry.get("y")?.as_f64()? as f32,
                            zoom_x: entry.get("zoomX")?.as_f64()? as f32 / 100.0,
                            zoom_y: entry.get("zoomY")?.as_f64()? as f32 / 100.0,
                            visible: entry.get("visible")?.as_bool()?,
                            opacity: u8::try_from(entry.get("opacity")?.as_u64()?).ok()?,
                            focus: u8::try_from(entry.get("focus")?.as_u64()?).ok()?,
                        })
                    })
                    .collect()
            })
            .collect(),
    )
}

async fn load_health_up_graphic_frames(path: &Path) -> Option<Vec<Vec<HealthUpGraphicFrame>>> {
    let bytes = load_file(&path.to_string_lossy()).await.ok()?;
    parse_health_up_graphic_frames(&bytes)
}

#[derive(Clone, Copy)]
struct ActiveExpAnimation {
    party_slot: usize,
    from_exp: u32,
    to_exp: u32,
    from_level: u8,
    to_level: u8,
    started_at: f64,
    duration: f64,
}

#[derive(Clone, Copy)]
struct DamageNumberPose {
    alpha: f32,
    y_offset: f32,
    scale_x: f32,
    scale_y: f32,
}

#[derive(Clone, Copy)]
struct ActiveFaintAnimation {
    actor: BattleActor,
    slot: usize,
    starts_at: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CaptureBall {
    Poke,
    Great,
    Ultra,
}

impl CaptureBall {
    fn prefix(self) -> &'static str {
        match self {
            Self::Poke => "pb",
            Self::Great => "gb",
            Self::Ultra => "ub",
        }
    }
}

#[derive(Clone, Copy)]
struct ActiveCaptureAnimation {
    ball: CaptureBall,
    caught: bool,
    critical: bool,
    shake_roll_count: usize,
    star_offsets: [f32; 3],
    started_at: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ActiveSwitchAnimation {
    outgoing_slot: usize,
    active_slot: usize,
    returns_outgoing: bool,
    started_at: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SwitchPose {
    outgoing_visible: bool,
    outgoing_scale: f32,
    outgoing_tint_alpha: f32,
    ball_visible: bool,
    ball_x: f32,
    ball_y: f32,
    ball_angle: f32,
    incoming_visible: bool,
    incoming_scale: f32,
    incoming_tint_alpha: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CapturePose {
    enemy_scale: f32,
    enemy_offset_y: f32,
    hide_enemy: bool,
    hide_info: bool,
    ball_behind_enemy: bool,
    ball_x: f32,
    ball_y: f32,
    ball_angle: f32,
    ball_frame: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CaptureParticlePose {
    x: f32,
    y: f32,
    frame: u8,
    alpha: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CaptureSuccessStarPose {
    x: f32,
    y: f32,
    alpha: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CaptureBallTintPose {
    alpha: f32,
}

#[derive(Clone, Copy)]
struct FaintPose {
    offset_y: f32,
    hide_info: bool,
    hide_sprite: bool,
}

#[derive(Default)]
struct BattleAnimationPlayback {
    queue: VecDeque<BattleAnimationRequest>,
    active: Option<ActiveBattleAnimation>,
    faint: Option<ActiveFaintAnimation>,
    capture: Option<ActiveCaptureAnimation>,
    switch: Option<ActiveSwitchAnimation>,
    damage: Vec<ActiveDamageAnimation>,
    hp_bars: Vec<ActiveHpBarAnimation>,
    healing: Vec<ActiveHealAnimation>,
    experience: Option<ActiveExpAnimation>,
    active_player_slot: usize,
}

fn battle_animation_center(
    frame: generated_battle_anims::AnimGraphicFrame,
    actor: BattleActor,
    player_half_height: f32,
    enemy_half_height: f32,
) -> Vec2 {
    let (user_x, user_y, user_half_height, target_x, target_y, target_half_height) =
        if actor == BattleActor::Player {
            (
                106.0,
                148.0,
                player_half_height,
                236.0,
                84.0,
                enemy_half_height,
            )
        } else {
            (
                236.0,
                84.0,
                enemy_half_height,
                106.0,
                148.0,
                player_half_height,
            )
        };
    let source_x = 106.0 + frame.x;
    let source_y = 116.0 + frame.y;
    let tx = (source_x - 106.0) / (234.0 - 106.0);
    let ty = (source_y - 116.0) / (52.0 - 116.0);
    vec2(
        (user_x + tx * (target_x - user_x)) * 3.0,
        (user_y - user_half_height
            + ty * ((target_y - target_half_height) - (user_y - user_half_height)))
            * 3.0,
    )
}

fn capture_bounce_duration() -> f64 {
    let mut duration = 0.35;
    let mut power = 0.5;
    while power > 0.01 {
        duration += power * 0.7;
        power *= 0.5;
    }
    duration
}

fn capture_shake_start(critical: bool) -> f64 {
    1.25 + if critical { 2.06 } else { 0.0 } + capture_bounce_duration()
}

fn attempt_animation_duration(critical: bool, caught: bool, shake_roll_count: usize) -> f64 {
    let shake_end = if caught {
        if critical { 3.0 } else { 5.0 }
    } else if critical {
        2.25
    } else {
        shake_roll_count.clamp(1, 4) as f64 + 0.25
    };
    capture_shake_start(critical) + shake_end
}

fn capture_animation_duration(capture: ActiveCaptureAnimation) -> f64 {
    attempt_animation_duration(capture.critical, capture.caught, capture.shake_roll_count)
}

fn capture_bounce_y(elapsed: f64) -> f32 {
    let mut elapsed = elapsed.max(0.0);
    if elapsed < 0.35 {
        let t = (elapsed / 0.35) as f32;
        return 16.0 + 56.0 * t.powi(3);
    }
    elapsed -= 0.35;
    let mut power = 0.5_f64;
    while power > 0.01 {
        let target_y = 72.0 - 56.0 * power;
        let leg = power * 0.35;
        if elapsed < leg {
            let t = (elapsed / leg) as f32;
            let ease_out = 1.0 - (1.0 - t).powi(3);
            return 72.0 - 56.0 * power as f32 * ease_out;
        }
        elapsed -= leg;
        if elapsed < leg {
            let t = (elapsed / leg) as f32;
            return target_y as f32 + (72.0 - target_y as f32) * t.powi(3);
        }
        elapsed -= leg;
        power *= 0.5;
    }
    72.0
}

fn capture_open_particle_pose(
    capture: ActiveCaptureAnimation,
    index: u8,
    now: f64,
) -> Option<CaptureParticlePose> {
    let radius = match capture.ball {
        CaptureBall::Poke => 48.0,
        CaptureBall::Great => 96.0,
        CaptureBall::Ultra => return None,
    };
    let age = now - (capture.started_at + 0.5 + f64::from(index) * 0.02);
    if !(0.0..0.575).contains(&age) {
        return None;
    }
    let angle = f64::from(index) * std::f64::consts::FRAC_PI_4;
    let progress = (age / 0.575) as f32;
    let alpha = if age <= 0.5 {
        1.0
    } else {
        (1.0 - ((age - 0.5) / 0.075) as f32).clamp(0.0, 1.0)
    };
    Some(CaptureParticlePose {
        x: 236.0 + (radius as f32 * angle.cos() as f32) * progress,
        y: 16.0 + (radius as f32 * angle.sin() as f32) * progress,
        frame: ((index + 3 + (age * 16.0) as u8) % 4),
        alpha,
    })
}

fn capture_star_offsets(
    species_id: u16,
    turn: u32,
    attempt: &pokerogue_core::CaptureAttempt,
) -> [f32; 3] {
    // Cosmetic-only SplitMix64 stream: deliberately separate from BattleRng.
    let mut state = (u64::from(species_id) << 48)
        ^ (u64::from(turn) << 16)
        ^ attempt.modified_catch_rate as u32 as u64
        ^ (u64::from(attempt.critical) << 63);
    for roll in &attempt.shake_rolls {
        state ^= (*roll as u32 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        state = state.rotate_left(19);
    }
    std::array::from_fn(|_| {
        let mut next_uniform = || {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^= z >> 31;
            ((z >> 11) as f64 + 0.5) / 9_007_199_254_740_992.0
        };
        let u = next_uniform();
        let v = next_uniform();
        (-2.0 * u.ln())
            .sqrt()
            .mul_add((std::f64::consts::TAU * v).cos(), 0.0) as f32
            * 25.0
    })
}

fn capture_success_star_pose(
    capture: ActiveCaptureAnimation,
    index: usize,
    now: f64,
) -> Option<CaptureSuccessStarPose> {
    if !capture.caught || index >= capture.star_offsets.len() {
        return None;
    }
    let lock_delay = if capture.critical { 2.0 } else { 4.0 };
    let age = now - (capture.started_at + capture_shake_start(capture.critical) + lock_delay);
    if !(0.0..0.5).contains(&age) {
        return None;
    }
    let y = if age < 0.25 {
        72.0 - 10.0 * (age / 0.25 * std::f64::consts::FRAC_PI_2).sin() as f32
    } else {
        let progress = ((age - 0.25) / 0.25 * std::f64::consts::FRAC_PI_2).cos() as f32;
        62.0 + 10.0 * (1.0 - progress)
    };
    let alpha = if age <= 0.425 {
        0.5
    } else {
        (0.5 * (1.0 - (age - 0.425) / 0.075) as f32).clamp(0.0, 0.5)
    };
    Some(CaptureSuccessStarPose {
        x: 236.0 + capture.star_offsets[index] * (age / 0.5) as f32,
        y,
        alpha,
    })
}

fn capture_ball_tint_pose(
    capture: ActiveCaptureAnimation,
    now: f64,
) -> Option<CaptureBallTintPose> {
    if !capture.caught {
        return None;
    }
    let lock_delay = if capture.critical { 2.0 } else { 4.0 };
    let age = now - (capture.started_at + capture_shake_start(capture.critical) + lock_delay);
    if !(0.0..0.4).contains(&age) {
        return None;
    }
    let alpha = if age < 0.2 {
        0.375 * (age / 0.2 * std::f64::consts::FRAC_PI_2).sin() as f32
    } else {
        0.375 * (1.0 - (1.0 - ((age - 0.2) / 0.2 * std::f64::consts::FRAC_PI_2).cos()) as f32)
    };
    Some(CaptureBallTintPose { alpha })
}

fn switch_summon_started_at(switch: ActiveSwitchAnimation) -> f64 {
    switch.started_at
        + if switch.returns_outgoing {
            0.25 + 0.75
        } else {
            0.0
        }
}

fn switch_animation_duration(switch: ActiveSwitchAnimation) -> f64 {
    (switch_summon_started_at(switch) - switch.started_at) + 0.65 + 0.25 + 1.0
}

fn switch_pose(switch: ActiveSwitchAnimation, now: f64) -> Option<SwitchPose> {
    let elapsed = now - switch.started_at;
    if elapsed < 0.0 || elapsed >= switch_animation_duration(switch) {
        return None;
    }
    if switch.returns_outgoing && elapsed < 0.25 - 1e-9 {
        let progress = (elapsed / 0.25) as f32;
        let ease = 1.0 - (progress * std::f32::consts::FRAC_PI_2).cos();
        return Some(SwitchPose {
            outgoing_visible: true,
            outgoing_scale: 1.0 - 0.5 * ease,
            outgoing_tint_alpha: ease,
            ball_visible: false,
            ball_x: 0.0,
            ball_y: 0.0,
            ball_angle: 0.0,
            incoming_visible: false,
            incoming_scale: 0.5,
            incoming_tint_alpha: 1.0,
        });
    }
    if switch.returns_outgoing && elapsed < 1.0 - 1e-9 {
        return Some(SwitchPose {
            outgoing_visible: false,
            outgoing_scale: 0.5,
            outgoing_tint_alpha: 1.0,
            ball_visible: false,
            ball_x: 0.0,
            ball_y: 0.0,
            ball_angle: 0.0,
            incoming_visible: false,
            incoming_scale: 0.5,
            incoming_tint_alpha: 1.0,
        });
    }

    let summon_elapsed = now - switch_summon_started_at(switch);
    if summon_elapsed < 0.65 - 1e-9 {
        let x_progress = (summon_elapsed / 0.65).clamp(0.0, 1.0) as f32;
        let (ball_y, ball_angle) = if summon_elapsed < 0.15 {
            let progress = (summon_elapsed / 0.15).clamp(0.0, 1.0) as f32;
            (80.0 - 10.0 * progress, 0.0)
        } else {
            let progress = ((summon_elapsed - 0.15) / 0.5).clamp(0.0, 1.0) as f32;
            (70.0 + 62.0 * progress.powi(3), 1440.0 * progress)
        };
        return Some(SwitchPose {
            outgoing_visible: false,
            outgoing_scale: 0.5,
            outgoing_tint_alpha: 1.0,
            ball_visible: true,
            ball_x: 36.0 + 64.0 * x_progress,
            ball_y,
            ball_angle,
            incoming_visible: false,
            incoming_scale: 0.5,
            incoming_tint_alpha: 1.0,
        });
    }

    let reveal_elapsed = summon_elapsed - 0.65;
    let progress = (reveal_elapsed / 0.25).clamp(0.0, 1.0) as f32;
    let ease = 1.0 - (progress * std::f32::consts::FRAC_PI_2).cos();
    Some(SwitchPose {
        outgoing_visible: false,
        outgoing_scale: 0.5,
        outgoing_tint_alpha: 1.0,
        ball_visible: false,
        ball_x: 0.0,
        ball_y: 0.0,
        ball_angle: 0.0,
        incoming_visible: true,
        incoming_scale: 0.5 + 0.5 * ease,
        incoming_tint_alpha: 1.0 - ease,
    })
}

fn switch_message(
    switch: ActiveSwitchAnimation,
    battle: &pokerogue_core::Battle,
    now: f64,
) -> Option<String> {
    if now < switch.started_at || now >= switch.started_at + switch_animation_duration(switch) {
        return None;
    }
    let name = if switch.returns_outgoing && now < switch_summon_started_at(switch) {
        battle.player.member(switch.outgoing_slot)?.species.name()
    } else {
        battle.player.member(switch.active_slot)?.species.name()
    };
    Some(
        if switch.returns_outgoing && now < switch_summon_started_at(switch) {
            format!("Come back, {name}!")
        } else {
            format!("Go! {name}!")
        },
    )
}

fn hp_bar_animation_duration(from_hp: i32, to_hp: i32) -> f64 {
    (f64::from((from_hp - to_hp).abs()) * 0.005).clamp(0.25, 5.0)
}

fn hp_bar_visual_sample(animation: ActiveHpBarAnimation, now: f64) -> (f32, i32) {
    let progress = ((now - animation.started_at) / animation.duration).clamp(0.0, 1.0);
    let eased = (progress * std::f64::consts::FRAC_PI_2).sin();
    let hp = f64::from(animation.from_hp) + f64::from(animation.to_hp - animation.from_hp) * eased;
    let ratio = (hp / f64::from(animation.max_hp.max(1))).clamp(0.0, 1.0) as f32;
    (ratio, hp.ceil() as i32)
}

impl BattleAnimationPlayback {
    // Phaser's TimerEvent starts with elapsed=200ms. The first callback
    // therefore runs on the next clock tick, followed by five 100ms repeats.
    const HIT_FLASH_SECONDS: f64 = 0.7;
    const DAMAGE_NUMBER_SECONDS: f64 = 1.7;
    // Nine 50ms animation frames, followed by the 20 60Hz frames returned by
    // the original 650ms Health Up sound event (ceil(650 / 33.33)).
    const HEALTH_UP_NUMBER_DELAY_SECONDS: f64 = 0.5 + 20.0 / 60.0;
    const HEALTH_UP_FRAME_SECONDS: f64 = 0.05;
    const CAPTURE_THROW_SECONDS: f64 = 0.5;
    const CAPTURE_ABSORB_SECONDS: f64 = 0.5;
    const CAPTURE_FRAME_SECONDS: f64 = 0.017;

    fn enqueue_turn(
        &mut self,
        battle: &pokerogue_core::Battle,
        now: f64,
        ball: Option<CaptureBall>,
    ) {
        self.enqueue_turn_with_hp_snapshot(battle, now, ball, None);
    }

    fn enqueue_turn_with_hp_snapshot(
        &mut self,
        battle: &pokerogue_core::Battle,
        now: f64,
        ball: Option<CaptureBall>,
        hp_before: Option<&BattleHpSnapshot>,
    ) {
        self.active_player_slot = battle.player.active_slot();
        self.hp_bars.clear();
        let (mut party_hp, mut player_slot, mut enemy_slot_hp) = if let Some(snapshot) = hp_before {
            (
                snapshot.party_hp.clone(),
                snapshot.player_active_slot,
                snapshot.enemy_hp,
            )
        } else {
            let mut party_hp = (0..battle.player.len())
                .filter_map(|slot| battle.player.member(slot).map(|member| member.hp))
                .collect::<Vec<_>>();
            let mut player_slot = battle.player.active_slot();
            let mut enemy_hp = battle.enemy.hp;
            // Fallback for isolated playback tests without a pre-action snapshot.
            for event in battle.last_events.iter().rev() {
                match event {
                    BattleEvent::DamageApplied { target, amount, .. } => match target {
                        BattleActor::Player => {
                            if let Some(hp) = party_hp.get_mut(player_slot) {
                                *hp += amount;
                            }
                        }
                        BattleActor::Enemy => enemy_hp += amount,
                    },
                    BattleEvent::SitrusBerryConsumed { slot, hp_restored } => {
                        if let Some(hp) = party_hp.get_mut(*slot) {
                            *hp -= hp_restored;
                        }
                    }
                    BattleEvent::Switch(pokerogue_core::SwitchEvent::SwitchCompleted {
                        outgoing,
                        ..
                    }) => player_slot = *outgoing,
                    _ => {}
                }
            }
            (party_hp, player_slot, enemy_hp)
        };
        self.damage
            .retain(|damage| now < damage.started_at + Self::DAMAGE_NUMBER_SECONDS);
        self.healing.retain(|heal| {
            now < heal.started_at
                + Self::HEALTH_UP_NUMBER_DELAY_SECONDS
                + Self::DAMAGE_NUMBER_SECONDS
        });
        let mut visual_delay = 0.0;
        let captured_enemy = battle.last_events.iter().any(|event| {
            matches!(
                event,
                BattleEvent::CaptureAttemptResolved { caught: true, .. }
            )
        });
        if let (Some(ball), Some(attempt)) = (ball, battle.last_capture_attempt.as_ref()) {
            if battle
                .last_events
                .iter()
                .any(|event| matches!(event, BattleEvent::CaptureAttemptResolved { .. }))
            {
                self.capture = Some(ActiveCaptureAnimation {
                    ball,
                    caught: attempt.caught,
                    critical: attempt.critical,
                    shake_roll_count: attempt.shake_rolls.len(),
                    star_offsets: capture_star_offsets(
                        battle.enemy.species.numeric_id(),
                        battle.turn,
                        attempt,
                    ),
                    started_at: now,
                });
                visual_delay = attempt_animation_duration(
                    attempt.critical,
                    attempt.caught,
                    attempt.shake_rolls.len(),
                );
            }
        }
        for event in &battle.last_events {
            match event {
                BattleEvent::MoveExecuted { actor, move_ } => {
                    let Some(animation) = generated_battle_anims::for_move(move_.numeric_id())
                    else {
                        continue;
                    };
                    let user_name = match actor {
                        BattleActor::Player => battle.player.species.name(),
                        BattleActor::Enemy => battle.enemy.species.name(),
                    };
                    let miss = format!("{user_name} used {}! It missed!", move_.name());
                    let immune = format!("{user_name} used {}! No effect!", move_.name());
                    if battle
                        .log
                        .lines()
                        .any(|line| line == miss || line == immune)
                    {
                        continue;
                    }
                    self.queue.push_back(BattleAnimationRequest {
                        move_id: move_.numeric_id(),
                        actor: *actor,
                        starts_at: now + visual_delay,
                    });
                    visual_delay += animation.frames.len() as f64 * BATTLE_ANIMATION_FRAME_SECONDS;
                }
                BattleEvent::DamageApplied {
                    target,
                    amount,
                    critical,
                    effectiveness,
                } => {
                    self.damage.push(ActiveDamageAnimation {
                        target: *target,
                        amount: *amount,
                        critical: *critical,
                        effectiveness: *effectiveness,
                        started_at: now + visual_delay,
                    });
                    let (party_slot, from_hp, max_hp, to_hp) = match target {
                        BattleActor::Player => {
                            let Some(member) = battle.player.member(player_slot) else {
                                continue;
                            };
                            let Some(from_hp) = party_hp.get(player_slot).copied() else {
                                continue;
                            };
                            let to_hp = (from_hp - amount).max(0);
                            party_hp[player_slot] = to_hp;
                            (player_slot, from_hp, member.max_hp, to_hp)
                        }
                        BattleActor::Enemy => {
                            let from_hp = enemy_slot_hp;
                            let to_hp = (from_hp - amount).max(0);
                            enemy_slot_hp = to_hp;
                            (0, from_hp, battle.enemy.max_hp, to_hp)
                        }
                    };
                    let hp_duration = hp_bar_animation_duration(from_hp, to_hp);
                    self.hp_bars.push(ActiveHpBarAnimation {
                        actor: *target,
                        party_slot,
                        from_hp,
                        to_hp,
                        max_hp,
                        started_at: now + visual_delay + Self::HIT_FLASH_SECONDS,
                        duration: hp_duration,
                    });
                    visual_delay += Self::HIT_FLASH_SECONDS + hp_duration;
                }
                BattleEvent::SitrusBerryConsumed { slot, hp_restored } => {
                    self.healing.push(ActiveHealAnimation {
                        party_slot: *slot,
                        amount: *hp_restored,
                        started_at: now + visual_delay,
                    });
                    if let (Some(member), Some(from_hp)) =
                        (battle.player.member(*slot), party_hp.get(*slot).copied())
                    {
                        let to_hp = (from_hp + hp_restored).min(member.max_hp);
                        party_hp[*slot] = to_hp;
                        let hp_duration = hp_bar_animation_duration(from_hp, to_hp);
                        self.hp_bars.push(ActiveHpBarAnimation {
                            actor: BattleActor::Player,
                            party_slot: *slot,
                            from_hp,
                            to_hp,
                            max_hp: member.max_hp,
                            started_at: now + visual_delay,
                            duration: hp_duration,
                        });
                        visual_delay += Self::HEALTH_UP_NUMBER_DELAY_SECONDS + hp_duration;
                    } else {
                        visual_delay += Self::HEALTH_UP_NUMBER_DELAY_SECONDS;
                    }
                }
                BattleEvent::Switch(pokerogue_core::SwitchEvent::SwitchCompleted {
                    outgoing,
                    active,
                }) => {
                    player_slot = *active;
                    let returns_outgoing = !battle.last_events.iter().any(|event| {
                        matches!(event, BattleEvent::PokemonFainted { slot } if slot == outgoing)
                    });
                    let switch = ActiveSwitchAnimation {
                        outgoing_slot: *outgoing,
                        active_slot: *active,
                        returns_outgoing,
                        started_at: now + visual_delay,
                    };
                    self.switch = Some(switch);
                    visual_delay += switch_animation_duration(switch);
                }
                BattleEvent::ExperienceGained {
                    party_slot,
                    previous_exp,
                    current_exp,
                    previous_level,
                    current_level,
                    ..
                } if *party_slot == battle.player.active_slot() => {
                    let level_steps = current_level.saturating_sub(*previous_level);
                    let duration = (f64::from(level_steps.max(1)) * 1.65).max(0.35);
                    self.experience = Some(ActiveExpAnimation {
                        party_slot: *party_slot,
                        from_exp: *previous_exp,
                        to_exp: *current_exp,
                        from_level: *previous_level,
                        to_level: *current_level,
                        started_at: now + visual_delay,
                        duration,
                    });
                }
                BattleEvent::PokemonFainted { slot } if *slot == battle.player.active_slot() => {
                    self.faint = Some(ActiveFaintAnimation {
                        actor: BattleActor::Player,
                        slot: *slot,
                        starts_at: now + visual_delay,
                    });
                }
                BattleEvent::BattleEnded(pokerogue_core::Outcome::PlayerWin) if !captured_enemy => {
                    self.faint = Some(ActiveFaintAnimation {
                        actor: BattleActor::Enemy,
                        slot: 0,
                        starts_at: now + visual_delay,
                    });
                    // Victory EXP starts after the source faint tween completes.
                    visual_delay += 0.5;
                }
                _ => {}
            }
        }
    }

    fn faint_pose(
        &mut self,
        actor: BattleActor,
        active_slot: usize,
        now: f64,
    ) -> Option<FaintPose> {
        let faint = self.faint?;
        if faint.actor != actor {
            return None;
        }
        if actor == BattleActor::Player && faint.slot != active_slot {
            self.faint = None;
            return None;
        }
        if now < faint.starts_at {
            return Some(FaintPose {
                offset_y: 0.0,
                hide_info: false,
                hide_sprite: false,
            });
        }
        if !self.queue.is_empty() || self.active.is_some() {
            return Some(FaintPose {
                offset_y: 0.0,
                hide_info: false,
                hide_sprite: false,
            });
        }
        let progress = ((now - faint.starts_at) / 0.5).clamp(0.0, 1.0);
        if progress >= 1.0 {
            return Some(FaintPose {
                offset_y: 450.0,
                hide_info: true,
                hide_sprite: true,
            });
        }
        let sine_in = 1.0 - (progress as f32 * std::f32::consts::FRAC_PI_2).cos();
        Some(FaintPose {
            offset_y: 450.0 * sine_in,
            hide_info: true,
            hide_sprite: false,
        })
    }

    fn is_busy(&self) -> bool {
        self.is_busy_at(visual::time())
    }

    fn is_busy_at(&self, now: f64) -> bool {
        !self.queue.is_empty()
            || self.active.is_some()
            || self.damage.iter().any(|damage| {
                now >= damage.started_at && now < damage.started_at + Self::HIT_FLASH_SECONDS
            })
            || self
                .hp_bars
                .iter()
                .any(|hp| now < hp.started_at + hp.duration)
            || self
                .experience
                .is_some_and(|exp| now < exp.started_at + exp.duration)
            || self.faint.is_some_and(|faint| now < faint.starts_at + 0.5)
            || self
                .switch
                .is_some_and(|switch| now < switch.started_at + switch_animation_duration(switch))
            || self.capture.is_some_and(|capture| {
                now < capture.started_at + capture_animation_duration(capture)
            })
    }

    fn clear_faint(&mut self) {
        self.faint = None;
        self.capture = None;
        self.switch = None;
    }

    fn capture_pose(&self, now: f64) -> Option<CapturePose> {
        let capture = self.capture?;
        let elapsed = now - capture.started_at;
        let prefix = capture.ball.prefix();
        if elapsed < 0.0 {
            return None;
        }
        let shake_start = capture_shake_start(capture.critical);
        let duration = capture_animation_duration(capture);
        if !capture.caught && elapsed >= duration {
            return None;
        }
        if elapsed < Self::CAPTURE_THROW_SECONDS {
            let progress = (elapsed / Self::CAPTURE_THROW_SECONDS) as f32;
            let eased_y = 1.0 - (1.0 - progress).powi(3);
            return Some(CapturePose {
                enemy_scale: 1.0,
                enemy_offset_y: 0.0,
                hide_enemy: false,
                hide_info: false,
                ball_behind_enemy: elapsed >= 0.3,
                ball_x: (16.0 + 220.0 * progress) * 3.0,
                ball_y: (80.0 - 64.0 * eased_y) * 3.0,
                ball_angle: 0.0,
                ball_frame: prefix,
            });
        }
        let absorb = ((elapsed - Self::CAPTURE_THROW_SECONDS) / Self::CAPTURE_ABSORB_SECONDS)
            .clamp(0.0, 1.0) as f32;
        let ease_in = 1.0 - (absorb * std::f32::consts::FRAC_PI_2).cos();
        let mut enemy_scale = 1.0 - 0.75 * ease_in;
        let mut enemy_offset_y = -64.0 * ease_in * 3.0;
        let mut hide_enemy = elapsed >= Self::CAPTURE_THROW_SECONDS + Self::CAPTURE_ABSORB_SECONDS;
        let mut hide_info = elapsed >= Self::CAPTURE_THROW_SECONDS;
        let mut ball_x = 236.0;
        let mut ball_y = 16.0;
        let mut ball_angle = 0.0;
        let mut ball_frame = if elapsed < Self::CAPTURE_THROW_SECONDS + Self::CAPTURE_FRAME_SECONDS
        {
            match capture.ball {
                CaptureBall::Poke => "pb_opening",
                CaptureBall::Great => "gb_opening",
                CaptureBall::Ultra => "ub_opening",
            }
        } else if elapsed < Self::CAPTURE_THROW_SECONDS + Self::CAPTURE_ABSORB_SECONDS {
            match capture.ball {
                CaptureBall::Poke => "pb_open",
                CaptureBall::Great => "gb_open",
                CaptureBall::Ultra => "ub_open",
            }
        } else if elapsed < 1.0 + Self::CAPTURE_FRAME_SECONDS {
            match capture.ball {
                CaptureBall::Poke => "pb_opening",
                CaptureBall::Great => "gb_opening",
                CaptureBall::Ultra => "ub_opening",
            }
        } else {
            prefix
        };

        if elapsed >= 1.0 {
            enemy_scale = 0.25;
            enemy_offset_y = -64.0 * 3.0;
        }

        if elapsed >= 1.25 && elapsed < shake_start {
            let bounce_start = if capture.critical {
                if elapsed < 1.75 {
                    // The critical ball pauses before its four short side-to-side shakes.
                    ball_y = 16.0;
                    0.0
                } else if elapsed < 2.81 {
                    let t = elapsed - 1.75;
                    let shake = (t / 0.25).floor() as usize;
                    let phase = (t % 0.25) / 0.125;
                    if shake < 4 {
                        let x = if phase < 1.0 {
                            0.0 - 3.0 * phase
                        } else {
                            -3.0 + 6.0 * (phase - 1.0)
                        };
                        ball_x += x as f32;
                    } else {
                        ball_x = 236.0;
                    }
                    ball_y = 16.0;
                    0.0
                } else {
                    (elapsed - 3.31).max(0.0)
                }
            } else {
                (elapsed - 1.25).max(0.0)
            };
            if !capture.critical || elapsed >= 3.31 {
                ball_y = capture_bounce_y(bounce_start);
            }
        }

        if elapsed >= shake_start {
            let shake_elapsed = elapsed - shake_start;
            let shake_cycle = (shake_elapsed.floor() as usize).min(5);
            let phase = shake_elapsed % 1.0;
            if shake_cycle > 0 && shake_cycle < if capture.critical { 2 } else { 4 } {
                let value = if phase < 0.25 {
                    phase / 0.25
                } else if phase < 0.5 {
                    (0.5 - phase) / 0.25
                } else {
                    0.0
                } as f32;
                let direction = if shake_cycle % 2 == 1 { 1.0 } else { -1.0 };
                ball_x += value * 4.0 * direction;
                ball_angle = value * 27.5 * direction;
            }
        }

        if !capture.caught {
            let failure_at = if capture.critical {
                shake_start + 2.0
            } else {
                shake_start + capture.shake_roll_count.clamp(1, 4) as f64
            };
            if elapsed >= failure_at {
                let restored = ((elapsed - failure_at) / 0.25).clamp(0.0, 1.0) as f32;
                let ease_out = 1.0 - (1.0 - restored).powi(2);
                enemy_scale = 0.25 + 0.75 * ease_out;
                enemy_offset_y = -64.0 * (1.0 - ease_out) * 3.0;
                hide_enemy = false;
                hide_info = restored < 1.0;
                ball_frame = match capture.ball {
                    CaptureBall::Poke => "pb_opening",
                    CaptureBall::Great => "gb_opening",
                    CaptureBall::Ultra => "ub_opening",
                };
            }
        }

        Some(CapturePose {
            enemy_scale,
            enemy_offset_y,
            hide_enemy,
            hide_info,
            ball_behind_enemy: true,
            ball_x: ball_x * 3.0,
            ball_y: ball_y * 3.0,
            ball_angle,
            ball_frame,
        })
    }

    fn current_frame(
        &mut self,
    ) -> Option<(
        BattleAnimationRequest,
        &'static generated_battle_anims::MoveAnimation,
        generated_battle_anims::AnimGraphicFrame,
    )> {
        if self.capture.is_some_and(|capture| {
            visual::time() < capture.started_at + capture_animation_duration(capture)
        }) {
            return None;
        }
        loop {
            if self.active.is_none() {
                if self
                    .queue
                    .front()
                    .is_some_and(|request| visual::time() < request.starts_at)
                {
                    return None;
                }
                self.active = self.queue.pop_front().map(|request| ActiveBattleAnimation {
                    request,
                    started_at: request.starts_at,
                });
            }
            let active = self.active.as_ref()?;
            let animation = generated_battle_anims::for_move(active.request.move_id)?;
            let frame_index =
                ((visual::time() - active.started_at) / BATTLE_ANIMATION_FRAME_SECONDS) as usize;
            if frame_index >= animation.frames.len() {
                self.active = None;
                continue;
            }
            if let Some(frame) = animation.frames[frame_index] {
                if frame.visible {
                    return Some((active.request, animation, frame));
                }
            }
            // A source animation frame may contain no graphic while its frame duration still elapses.
            return None;
        }
    }

    fn damage_for(&self, actor: BattleActor, now: f64) -> Option<ActiveDamageAnimation> {
        self.damage.iter().rev().copied().find(|damage| {
            damage.target == actor
                && now >= damage.started_at
                && now < damage.started_at + Self::DAMAGE_NUMBER_SECONDS
        })
    }

    fn heal_for(&self, party_slot: usize, now: f64) -> Option<ActiveHealAnimation> {
        self.healing.iter().rev().copied().find(|heal| {
            heal.party_slot == party_slot
                && now >= heal.started_at
                && now
                    < heal.started_at
                        + Self::HEALTH_UP_NUMBER_DELAY_SECONDS
                        + Self::DAMAGE_NUMBER_SECONDS
        })
    }

    fn hp_bar_sample(
        &self,
        actor: BattleActor,
        party_slot: usize,
        hp: i32,
        max_hp: i32,
        now: f64,
    ) -> (f32, i32) {
        let animation = self
            .hp_bars
            .iter()
            .rev()
            .find(|animation| {
                animation.actor == actor
                    && animation.party_slot == party_slot
                    && animation.started_at <= now
            })
            .or_else(|| {
                self.hp_bars.iter().find(|animation| {
                    animation.actor == actor
                        && animation.party_slot == party_slot
                        && animation.started_at > now
                })
            });
        if let Some(animation) = animation {
            hp_bar_visual_sample(*animation, now)
        } else {
            (
                (hp.max(0) as f32 / max_hp.max(1) as f32).clamp(0.0, 1.0),
                hp,
            )
        }
    }

    fn health_up_frame_index(
        &self,
        party_slot: usize,
        now: f64,
        frame_count: usize,
    ) -> Option<usize> {
        let heal = self.heal_for(party_slot, now)?;
        let age = now - heal.started_at;
        if age < Self::HEALTH_UP_FRAME_SECONDS - 1e-9
            || age >= Self::HEALTH_UP_FRAME_SECONDS * (frame_count as f64 + 1.0)
        {
            return None;
        }
        let index = ((age + 1e-9) / Self::HEALTH_UP_FRAME_SECONDS).floor() as usize - 1;
        (index < frame_count).then_some(index)
    }

    fn damage_flash_visible(&self, actor: BattleActor, now: f64) -> bool {
        let Some(damage) = self.damage_for(actor, now) else {
            return true;
        };
        let age = now - damage.started_at;
        if age < 0.2 - 1e-9 || age >= Self::HIT_FLASH_SECONDS - 1e-9 {
            return true;
        }
        // TimerEvent starts at 200ms and dispatches at 200, 300, ... 700ms.
        ((((age - 0.2 + 1e-9) / 0.1).floor() as u32) % 2) == 1
    }

    fn experience_sample(
        &self,
        party_slot: usize,
        fallback_level: u8,
        fallback_exp: u32,
        growth_rate: pokerogue_data::GrowthRate,
        now: f64,
    ) -> (u8, u32) {
        let Some(animation) = self
            .experience
            .filter(|animation| animation.party_slot == party_slot)
        else {
            return (fallback_level, fallback_exp);
        };
        if now < animation.started_at {
            return (animation.from_level, animation.from_exp);
        }
        if now >= animation.started_at + animation.duration {
            return (animation.to_level, animation.to_exp);
        }
        let t = ((now - animation.started_at) / animation.duration).clamp(0.0, 1.0);
        let eased = 1.0 - (t * std::f64::consts::FRAC_PI_2).cos();
        let exp = animation.from_exp
            + ((animation.to_exp - animation.from_exp) as f64 * eased).round() as u32;
        let mut level = animation.from_level;
        while level < animation.to_level
            && exp >= pokerogue_data::total_exp_for_level(level + 1, growth_rate)
        {
            level += 1;
        }
        (level, exp)
    }
}

fn damage_number_pose(age: f64) -> Option<DamageNumberPose> {
    const KEYS: [(f64, f32, f32, f32); 10] = [
        (0.25, -16.0, 0.75, 1.25),
        (0.425, 0.0, 0.875, 1.125),
        (0.525, 0.0, 1.25, 0.75),
        (0.7, -8.0, 0.875, 1.125),
        (0.75, 0.0, 0.925, 1.075),
        (0.85, 0.0, 1.125, 0.875),
        (1.025, -4.0, 0.925, 1.075),
        (1.075, 0.0, 0.975, 1.025),
        (1.175, 0.0, 1.075, 0.925),
        (1.2, 0.0, 1.0, 1.0),
    ];
    if !(0.0..BattleAnimationPlayback::DAMAGE_NUMBER_SECONDS).contains(&age) {
        return None;
    }
    let (y_offset, scale_x, scale_y, alpha) = if age <= KEYS[0].0 {
        let t = (age / KEYS[0].0) as f32;
        let ease = 1.0 - (1.0 - t).powi(3);
        (
            KEYS[0].1 * ease,
            1.0 + (KEYS[0].2 - 1.0) * ease,
            1.0 + (KEYS[0].3 - 1.0) * ease,
            ease,
        )
    } else if let Some(index) = KEYS.iter().position(|key| age <= key.0) {
        let previous = if index == 0 {
            (0.0, 0.0, 1.0, 1.0)
        } else {
            let key = KEYS[index - 1];
            (key.0, key.1, key.2, key.3)
        };
        let current = KEYS[index];
        let t = ((age - previous.0) / (current.0 - previous.0)) as f32;
        let ease = 1.0 - (1.0 - t).powi(3);
        (
            previous.1 + (current.1 - previous.1) * ease,
            previous.2 + (current.2 - previous.2) * ease,
            previous.3 + (current.3 - previous.3) * ease,
            1.0,
        )
    } else {
        (0.0, 1.0, 1.0, (1.0 - (age - 1.2) / 0.5) as f32)
    };
    Some(DamageNumberPose {
        alpha,
        y_offset,
        scale_x,
        scale_y,
    })
}

fn heal_number_pose(heal: ActiveHealAnimation, now: f64) -> Option<DamageNumberPose> {
    let age = now - heal.started_at - BattleAnimationPlayback::HEALTH_UP_NUMBER_DELAY_SECONDS;
    (age >= -1e-9)
        .then_some(age.max(0.0))
        .and_then(damage_number_pose)
}

impl AnimatedSprite {
    fn draw_ground_shadow(&self, center_x: f32, baseline_y: f32, offset_x: f32, scale: f32) {
        let Some(frame) = self.frames.first() else {
            return;
        };
        // The pinned Phaser sprite shader projects a black ellipse into the
        // bottom 15% of the sprite quad, centered on the entity baseline.
        // The Phaser shader uses the full sprite canvas dimensions (the
        // untrimmed quad), not the trimmed opaque frame bounds.
        let width = f32::from(frame.source[0]) * FIELD_PIXELS_PER_SOURCE_UNIT * scale;
        let height = f32::from(frame.source[1]) * FIELD_PIXELS_PER_SOURCE_UNIT * scale;
        draw_pixelated_shadow(
            center_x + offset_x,
            baseline_y - height * 0.025,
            width,
            height * 0.15,
            FIELD_PIXELS_PER_SOURCE_UNIT * scale,
        );
    }

    fn draw(
        &self,
        center_x: f32,
        baseline_y: f32,
        offset_x: f32,
        offset_y: f32,
        scale: f32,
        tint_alpha: f32,
        animation_started_at: f64,
        visible: bool,
    ) {
        if !visible {
            return;
        }
        if self.frames.is_empty() {
            return;
        }
        const PIXEL_SCALE: f32 = 3.0;
        #[cfg(not(target_arch = "wasm32"))]
        let screenshot_mode = std::env::var_os("POKEROGUE_SCREENSHOT_PATH").is_some();
        #[cfg(target_arch = "wasm32")]
        let screenshot_mode = false;
        let frame_index = if screenshot_mode {
            0
        } else {
            (visual::sprite_elapsed(animation_started_at) * 10.0) as usize % self.frames.len()
        };
        let frame = self.frames[frame_index];
        let [source_x, source_y, source_w, source_h] = frame.rect;
        let [canvas_w, canvas_h] = frame.source;
        let [trim_x, trim_y] = frame.offset;
        let [trim_w, trim_h] = frame.size;
        let dest_x = (center_x + offset_x - f32::from(canvas_w) * PIXEL_SCALE * scale / 2.0
            + f32::from(trim_x) * PIXEL_SCALE * scale)
            .round();
        let dest_y = (baseline_y + offset_y - f32::from(canvas_h) * PIXEL_SCALE * scale
            + f32::from(trim_y) * PIXEL_SCALE * scale)
            .round();
        let dest_size = vec2(
            f32::from(trim_w) * PIXEL_SCALE * scale,
            f32::from(trim_h) * PIXEL_SCALE * scale,
        );
        let source = Rect::new(
            f32::from(source_x),
            f32::from(source_y),
            f32::from(source_w),
            f32::from(source_h),
        );
        draw_texture_ex(
            &self.texture,
            dest_x,
            dest_y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(dest_size),
                source: Some(source),
                ..Default::default()
            },
        );
        if tint_alpha > 0.0 {
            if let Some(tint_texture) = &self.pokeball_tint {
                draw_texture_ex(
                    tint_texture,
                    dest_x,
                    dest_y,
                    Color::new(1.0, 1.0, 1.0, tint_alpha.clamp(0.0, 1.0)),
                    DrawTextureParams {
                        dest_size: Some(dest_size),
                        source: Some(source),
                        ..Default::default()
                    },
                );
            }
        }
    }
}

fn draw_pixelated_shadow(center_x: f32, center_y: f32, width: f32, height: f32, pixel: f32) {
    for (x, y) in pixelated_shadow_cells(width, height, pixel) {
        draw_rectangle(
            center_x + x,
            center_y + y,
            pixel,
            pixel,
            Color::from_rgba(0, 0, 0, 128),
        );
    }
}

fn pixelated_shadow_cells(width: f32, height: f32, pixel: f32) -> Vec<(f32, f32)> {
    if width <= 0.0 || height <= 0.0 || pixel <= 0.0 {
        return Vec::new();
    }
    let radius_x = width * 0.5;
    let radius_y = height * 0.5;
    let left = (-radius_x / pixel).floor() * pixel;
    let top = (-radius_y / pixel).floor() * pixel;
    let mut cells = Vec::new();
    for y in (0..)
        .map(|row| top + row as f32 * pixel)
        .take_while(|y| *y < radius_y)
    {
        for x in (0..)
            .map(|column| left + column as f32 * pixel)
            .take_while(|x| *x < radius_x)
        {
            let dx = (x + pixel * 0.5) / radius_x;
            let dy = (y + pixel * 0.5) / radius_y;
            if dx * dx + dy * dy < 1.0 {
                cells.push((x, y));
            }
        }
    }
    cells
}

impl SpriteAtlas {
    async fn load() -> Self {
        // These are read from the original assets checkout and are not copied
        // into this project because their individual redistribution status is unclear.
        #[cfg(target_arch = "wasm32")]
        let pokemon_root = PathBuf::from(
            "https://raw.githubusercontent.com/pagefaultgames/pokerogue-assets/fb8ccb6d0703ffe48cdf80a8102d5ed943c82dba/images/pokemon",
        );
        #[cfg(not(target_arch = "wasm32"))]
        let pokemon_root = std::env::var_os("POKEROGUE_ASSETS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../original/assets/images/pokemon")
            });
        let images_root = pokemon_root
            .parent()
            .unwrap_or(Path::new("original/assets/images"));
        // Some pinned UI atlases are intentionally absent from the source
        // checkout. Use the local, commit-addressed asset cache when present;
        // never copy these restricted assets into the project.
        #[cfg(target_arch = "wasm32")]
        let cached_images = images_root.to_path_buf();
        #[cfg(not(target_arch = "wasm32"))]
        let cached_images = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../visual-harness/.cache/assets/fb8ccb6d0703ffe48cdf80a8102d5ed943c82dba/images",
        );
        #[cfg(target_arch = "wasm32")]
        let atlas_images = images_root;
        #[cfg(not(target_arch = "wasm32"))]
        let atlas_images = if images_root.join("ui/pbinfo_player_type.json").exists() {
            images_root
        } else if cached_images.join("ui/pbinfo_player_type.json").exists() {
            cached_images.as_path()
        } else {
            images_root
        };
        let asset_roots = [
            images_root.to_path_buf(),
            atlas_images.to_path_buf(),
            cached_images.clone(),
        ];
        let load = |path: PathBuf| async move { load_texture_nearest(&path).await };
        let hud = hud::Hud::load(images_root).await;
        let town_background =
            load_phaser_scene_texture(&images_root.join("arenas/town_bg.png")).await;
        let town_player_platform =
            load_phaser_scene_texture(&images_root.join("arenas/town_a.png")).await;
        let town_enemy_platform =
            load_phaser_scene_texture(&images_root.join("arenas/town_b.png")).await;
        let message_window = load(images_root.join("ui/bg.png")).await;
        let window_border = load(images_root.join("ui/windows/window_1.png")).await;
        let player_info = load(images_root.join("ui/pbinfo_player.png")).await;
        let enemy_info = load(images_root.join("ui/pbinfo_enemy_mini.png")).await;
        let enemy_boss_info = load(images_root.join("ui/pbinfo_enemy_boss.png")).await;
        let hp_bar = load(images_root.join("ui/overlay_hp.png")).await;
        let boss_hp_bar = load(images_root.join("ui/overlay_hp_boss.png")).await;
        let move_types = hud::Atlas::load_at(
            &atlas_images.join("types.png"),
            &atlas_images.join("types.json"),
        )
        .await;
        let move_categories = hud::Atlas::load_at(
            &atlas_images.join("categories.png"),
            &atlas_images.join("categories.json"),
        )
        .await;
        let item_icons = hud::Atlas::load_at(
            &cached_images.join("items.png"),
            &cached_images.join("items.json"),
        )
        .await;
        let reward_namebox = hud::Atlas::load_at(
            &atlas_images.join("ui/namebox.png"),
            &atlas_images.join("ui/namebox.json"),
        )
        .await;
        let load_ui_atlas = |name: &'static str| async move {
            hud::Atlas::load_at(
                &atlas_images.join(format!("ui/{name}.png")),
                &atlas_images.join(format!("ui/{name}.json")),
            )
            .await
        };
        let player_type = load_ui_atlas("pbinfo_player_type").await;
        let player_type1 = load_ui_atlas("pbinfo_player_type1").await;
        let player_type2 = load_ui_atlas("pbinfo_player_type2").await;
        let enemy_type = load_ui_atlas("pbinfo_enemy_type").await;
        let enemy_type1 = load_ui_atlas("pbinfo_enemy_type1").await;
        let enemy_type2 = load_ui_atlas("pbinfo_enemy_type2").await;
        let owned_icon = load_texture_nearest(&images_root.join("ui/icon_owned.png"))
            .await
            .or(load_texture_nearest(&cached_images.join("ui/icon_owned.png")).await);
        let menu_cursor = load_texture_nearest(&atlas_images.join("ui/cursor.png")).await;
        let title_logo = load_texture_nearest(&images_root.join("logo.png"))
            .await
            .or(load_texture_nearest(&cached_images.join("logo.png")).await);
        let title_background = load_phaser_scene_texture(&images_root.join("arenas/plains_bg.png"))
            .await
            .or(load_phaser_scene_texture(&cached_images.join("arenas/plains_bg.png")).await);
        let halloween_logo = load_texture_nearest(&images_root.join("events/logo_halloween.png"))
            .await
            .or(load_texture_nearest(&cached_images.join("events/logo_halloween.png")).await);
        let halloween_banner =
            load_texture_nearest(&images_root.join("events/halloween2026-en.png"))
                .await
                .or(load_texture_nearest(&cached_images.join("events/halloween2026-en.png")).await);
        let battle_anim_strike = load(images_root.join("battle_anims/PRAS- Strike.png")).await;
        let battle_anim_fire = load(images_root.join("battle_anims/PRAS- Fire.png")).await;
        let battle_anim_health_up =
            load_local_texture(&asset_roots, "battle_anims/PRAS- Recovery.png").await;
        let health_up_json = images_root
            .parent()
            .unwrap_or(images_root)
            .join("battle-anims/common-health-up.json");
        let health_up_frames = load_health_up_graphic_frames(&health_up_json)
            .await
            .unwrap_or_default();
        let pokeball_atlas = load(images_root.join("pb.png")).await;
        let ball_menu_atlas =
            hud::Atlas::load_at(&images_root.join("pb.png"), &images_root.join("pb.json")).await;
        let pokeball_particles = load(images_root.join("effects/pb_particles.png")).await;
        let party_background = load_local_texture(&asset_roots, "ui/party_bg.png").await;
        let party_main_slot = load_local_atlas(&asset_roots, "ui/party_slot_main").await;
        let party_bench_slot = load_local_atlas(&asset_roots, "ui/party_slot").await;
        let party_pokeball = load_local_atlas(&asset_roots, "ui/party_pb").await;
        let party_hp_bar = load_local_texture(&asset_roots, "ui/party_slot_hp_bar.png").await;
        let party_hp_overlay = load_local_atlas(&asset_roots, "ui/party_slot_hp_overlay").await;
        let party_hp_label = load_local_texture(
            &asset_roots,
            "ui/text_images/en/party_ui/party_slot_overlay_hp.png",
        )
        .await;
        let party_level_label = load_local_texture(
            &asset_roots,
            "ui/text_images/en/party_ui/party_slot_overlay_lv.png",
        )
        .await;
        let party_cancel = load_local_atlas(&asset_roots, "ui/party_cancel").await;
        let mut pokemon_icon_atlases = Vec::with_capacity(10);
        for generation in 0..10 {
            pokemon_icon_atlases
                .push(load_local_atlas(&asset_roots, &format!("pokemon_icons_{generation}")).await);
        }
        Self {
            hud,
            move_types,
            move_categories,
            player_type,
            player_type1,
            player_type2,
            enemy_type,
            enemy_type1,
            enemy_type2,
            item_icons,
            reward_namebox,
            owned_icon,
            menu_cursor,
            title_logo,
            title_background,
            halloween_logo,
            halloween_banner,
            town_background,
            town_player_platform,
            town_enemy_platform,
            message_window,
            window_border,
            player_info,
            enemy_info,
            enemy_boss_info,
            hp_bar,
            boss_hp_bar,
            battle_anim_strike,
            battle_anim_fire,
            battle_anim_health_up,
            health_up_frames,
            pokeball_atlas,
            ball_menu_atlas,
            pokeball_particles,
            party_background,
            party_main_slot,
            party_bench_slot,
            party_pokeball,
            party_hp_bar,
            party_hp_overlay,
            party_hp_label,
            party_level_label,
            party_cancel,
            pokemon_icon_atlases,
            pokemon_root,
            active_player: None,
            outgoing_player: None,
            active_enemy: None,
        }
    }

    async fn load_pokemon_sprite(&self, species: Species, back: bool) -> Option<AnimatedSprite> {
        let species_id = species.numeric_id();
        let metadata = generated_sprites::sprite_frames(species_id).or_else(|| {
            eprintln!(
                "Sprite atlas metadata unavailable for {} ({species_id})",
                species.name()
            );
            None
        })?;
        let frames = if back { metadata.back } else { metadata.front };
        if frames.is_empty() {
            return None;
        }
        let path = if back {
            self.pokemon_root
                .join("back")
                .join(format!("{species_id}.png"))
        } else {
            self.pokemon_root.join(format!("{species_id}.png"))
        };
        let texture = load_phaser_scene_texture(&path).await?;
        let pokeball_tint = load_pokeball_tint_texture(&path).await;
        Some(AnimatedSprite {
            species_id,
            texture,
            pokeball_tint,
            frames,
            started_at: visual::time(),
        })
    }

    async fn set_battlers(&mut self, player: Species, enemy: Species) {
        if self.active_player.as_ref().map(|sprite| sprite.species_id) != Some(player.numeric_id())
        {
            self.outgoing_player = self.active_player.take();
            self.active_player = self.load_pokemon_sprite(player, true).await;
        }
        if self.active_enemy.as_ref().map(|sprite| sprite.species_id) != Some(enemy.numeric_id()) {
            self.active_enemy = self.load_pokemon_sprite(enemy, false).await;
        }
    }

    fn draw(
        &self,
        player_species: Species,
        enemy_species: Species,
        field: FieldPresentation,
        player_faint: Option<FaintPose>,
        enemy_faint: Option<FaintPose>,
        capture_pose: Option<CapturePose>,
        playback: &BattleAnimationPlayback,
    ) {
        // Constructors in Pokemon use player (106,148) and enemy (236,84)
        // field coordinates. The 960x540 client canvas is half Phaser's 1920x1080.
        let faint = player_faint.unwrap_or(FaintPose {
            offset_y: 0.0,
            hide_info: false,
            hide_sprite: false,
        });
        let now = visual::time();
        let switch_pose = playback.switch.and_then(|switch| switch_pose(switch, now));
        if let Some(pose) = switch_pose {
            if pose.outgoing_visible {
                if let Some(sprite) = self
                    .outgoing_player
                    .as_ref()
                    .or(self.active_player.as_ref())
                {
                    sprite.draw_ground_shadow(
                        106.0 * 3.0,
                        148.0 * 3.0,
                        field.player_x * 3.0,
                        pose.outgoing_scale,
                    );
                    sprite.draw(
                        106.0 * 3.0,
                        148.0 * 3.0,
                        field.player_x * 3.0,
                        0.0,
                        pose.outgoing_scale,
                        pose.outgoing_tint_alpha,
                        sprite.started_at,
                        true,
                    );
                }
            } else if pose.incoming_visible && !faint.hide_sprite {
                if let Some(sprite) = &self.active_player {
                    sprite.draw_ground_shadow(
                        106.0 * 3.0,
                        148.0 * 3.0,
                        field.player_x * 3.0,
                        pose.incoming_scale,
                    );
                    sprite.draw(
                        106.0 * 3.0,
                        148.0 * 3.0,
                        field.player_x * 3.0,
                        0.0,
                        pose.incoming_scale,
                        pose.incoming_tint_alpha,
                        switch_summon_started_at(playback.switch.unwrap()) + 0.65,
                        true,
                    );
                } else {
                    draw_fallback_sprite(
                        106.0 * 3.0 + field.player_x * 3.0,
                        148.0 * 3.0 - 70.0,
                        player_species.name(),
                        GREEN,
                    );
                }
            }
        } else if !faint.hide_sprite {
            if let Some(sprite) = &self.active_player {
                if playback.damage_flash_visible(BattleActor::Player, visual::time()) {
                    sprite.draw_ground_shadow(106.0 * 3.0, 148.0 * 3.0, field.player_x * 3.0, 1.0);
                    let animation_started_at = playback
                        .switch
                        .map(|switch| switch_summon_started_at(switch) + 0.65)
                        .filter(|started_at| visual::time() >= *started_at)
                        .unwrap_or(sprite.started_at);
                    sprite.draw(
                        106.0 * 3.0,
                        148.0 * 3.0,
                        field.player_x * 3.0,
                        faint.offset_y,
                        1.0,
                        0.0,
                        animation_started_at,
                        true,
                    );
                }
            } else {
                draw_fallback_sprite(
                    106.0 * 3.0 + field.player_x * 3.0,
                    148.0 * 3.0 - 70.0 + faint.offset_y,
                    player_species.name(),
                    GREEN,
                );
            }
        }
        let enemy_faint = enemy_faint.unwrap_or(FaintPose {
            offset_y: 0.0,
            hide_info: false,
            hide_sprite: false,
        });
        let capture_scale = capture_pose.map_or(1.0, |pose| pose.enemy_scale);
        let capture_hides_enemy = capture_pose.is_some_and(|pose| pose.hide_enemy);
        if capture_pose.is_some_and(|pose| pose.ball_behind_enemy) {
            self.draw_capture_ball(capture_pose);
        }
        if !enemy_faint.hide_sprite && !capture_hides_enemy {
            if let Some(sprite) = &self.active_enemy {
                if playback.damage_flash_visible(BattleActor::Enemy, visual::time()) {
                    sprite.draw_ground_shadow(
                        234.0 * 3.0,
                        84.0 * 3.0
                            + enemy_faint.offset_y
                            + capture_pose.map_or(0.0, |pose| pose.enemy_offset_y),
                        field.enemy_x * 3.0,
                        capture_scale,
                    );
                    sprite.draw(
                        234.0 * 3.0,
                        84.0 * 3.0
                            + enemy_faint.offset_y
                            + capture_pose.map_or(0.0, |pose| pose.enemy_offset_y),
                        field.enemy_x * 3.0,
                        0.0,
                        capture_scale,
                        0.0,
                        sprite.started_at,
                        true,
                    );
                }
            } else {
                draw_fallback_sprite(
                    234.0 * 3.0 + field.enemy_x * 3.0,
                    84.0 * 3.0 - 70.0 + enemy_faint.offset_y,
                    enemy_species.name(),
                    ORANGE,
                );
            }
        }
    }

    fn draw_damage_numbers(&self, playback: &BattleAnimationPlayback, field: FieldPresentation) {
        let now = visual::time();
        for target in [BattleActor::Player, BattleActor::Enemy] {
            let Some(hit) = playback.damage_for(target, now) else {
                continue;
            };
            let Some(pose) = damage_number_pose(now - hit.started_at) else {
                continue;
            };
            let (sprite, x, baseline_y, offset_x) = if target == BattleActor::Player {
                (
                    self.active_player.as_ref(),
                    106.0 * 3.0,
                    148.0 * 3.0,
                    field.player_x * 3.0,
                )
            } else {
                (
                    self.active_enemy.as_ref(),
                    234.0 * 3.0,
                    84.0 * 3.0,
                    field.enemy_x * 3.0,
                )
            };
            let height = sprite
                .and_then(|s| s.frames.first())
                .map_or(42.0, |f| f32::from(f.source[1]) * 3.0);
            let text = hit.amount.to_string();
            let (color, shadow) = match hit.effectiveness {
                pokerogue_core::DamageEffectiveness::Normal => {
                    (WHITE, Color::from_rgba(99, 99, 99, 255))
                }
                pokerogue_core::DamageEffectiveness::SuperEffective => (
                    Color::from_rgba(248, 208, 48, 255),
                    Color::from_rgba(184, 160, 56, 255),
                ),
                pokerogue_core::DamageEffectiveness::NotVeryEffective => (
                    Color::from_rgba(240, 128, 48, 255),
                    Color::from_rgba(192, 48, 40, 255),
                ),
            };
            let font = self.hud.font.as_ref();
            let params = TextParams {
                font,
                font_size: 72,
                font_scale: 0.5 * pose.scale_y,
                color: Color::new(color.r, color.g, color.b, pose.alpha),
                ..Default::default()
            };
            let width = measure_text(&text, font, params.font_size, params.font_scale).width
                * pose.scale_x
                / pose.scale_y;
            let text_x = (x + offset_x - width / 2.0).round();
            let text_y = (baseline_y - height * 0.72 + pose.y_offset * 3.0).round();
            if hit.critical {
                for (dx, dy) in [(-1.5, 0.0), (1.5, 0.0), (0.0, -1.5), (0.0, 1.5)] {
                    draw_text_ex(
                        &text,
                        text_x + dx,
                        text_y + dy,
                        TextParams {
                            color: Color::new(shadow.r, shadow.g, shadow.b, pose.alpha),
                            ..params.clone()
                        },
                    );
                }
            } else {
                draw_text_ex(
                    &text,
                    text_x + 2.0,
                    text_y + 2.0,
                    TextParams {
                        color: Color::new(shadow.r, shadow.g, shadow.b, pose.alpha),
                        ..params.clone()
                    },
                );
            }
            draw_text_ex(&text, text_x, text_y, params);
        }
        if let Some(heal) = playback.heal_for(playback.active_player_slot, now) {
            if let Some(pose) = heal_number_pose(heal, now) {
                let text = heal.amount.to_string();
                let font = self.hud.font.as_ref();
                // PokemonHealPhase feeds HitResult.HEAL to DamageNumberHandler.
                let color = Color::from_rgba(0x78, 0xc8, 0x50, 255);
                let shadow = Color::from_rgba(0x58, 0x80, 0x40, 255);
                let params = TextParams {
                    font,
                    font_size: 72,
                    font_scale: 0.5 * pose.scale_y,
                    color: Color::new(color.r, color.g, color.b, pose.alpha),
                    ..Default::default()
                };
                let width = measure_text(&text, font, params.font_size, params.font_scale).width
                    * pose.scale_x
                    / pose.scale_y;
                let height = self
                    .active_player
                    .as_ref()
                    .and_then(|sprite| sprite.frames.first())
                    .map_or(42.0, |frame| f32::from(frame.source[1]) * 3.0);
                let text_x = (106.0 * 3.0 + field.player_x * 3.0 - width / 2.0).round();
                let text_y = (148.0 * 3.0 - height * 0.72 + pose.y_offset * 3.0).round();
                draw_text_ex(
                    &text,
                    text_x + 2.0,
                    text_y + 2.0,
                    TextParams {
                        color: Color::new(shadow.r, shadow.g, shadow.b, pose.alpha),
                        ..params.clone()
                    },
                );
                draw_text_ex(&text, text_x, text_y, params);
            }
        }
    }

    fn draw_health_up_animation(
        &self,
        playback: &BattleAnimationPlayback,
        field: FieldPresentation,
    ) {
        let Some(texture) = self.battle_anim_health_up.as_ref() else {
            return;
        };
        let Some(frame_index) = playback.health_up_frame_index(
            playback.active_player_slot,
            visual::time(),
            self.health_up_frames.len(),
        ) else {
            return;
        };
        let Some(frame) = self.health_up_frames.get(frame_index) else {
            return;
        };
        let half_height = self
            .active_player
            .as_ref()
            .and_then(|sprite| sprite.frames.first())
            .map_or(21.0, |sprite_frame| f32::from(sprite_frame.source[1]) / 2.0);
        let user_x = 106.0 + field.player_x;
        let user_y = 148.0;
        for graphic in frame.iter().filter(|graphic| graphic.visible) {
            let mut x = graphic.x + 106.0;
            let mut y = graphic.y + 116.0;
            match graphic.focus {
                // AnimFocus.TARGET: 1
                1 => {
                    x += user_x - 234.0;
                    y += user_y - half_height - 52.0;
                }
                // AnimFocus.USER: 2
                2 => {
                    x += user_x - 106.0;
                    y += user_y - half_height - 116.0;
                }
                // AnimFocus.USER_TARGET: 3. HEALTH_UP targets the user itself,
                // so the source line collapses to that sprite's position.
                3 => {
                    x = user_x;
                    y = user_y - half_height;
                }
                // AnimFocus.SCREEN: 4 keeps the source's fixed canvas position.
                _ => {}
            }
            let source_x = f32::from(graphic.frame % 5) * 96.0;
            let source_y = f32::from(graphic.frame / 5) * 96.0;
            draw_texture_ex(
                texture,
                x * 3.0 - 144.0 * graphic.zoom_x,
                y * 3.0 - 144.0 * graphic.zoom_y,
                Color::new(1.0, 1.0, 1.0, f32::from(graphic.opacity) / 255.0),
                DrawTextureParams {
                    source: Some(Rect::new(source_x, source_y, 96.0, 96.0)),
                    dest_size: Some(vec2(288.0 * graphic.zoom_x, 288.0 * graphic.zoom_y)),
                    ..Default::default()
                },
            );
        }
    }

    fn draw_capture_ball(&self, pose: Option<CapturePose>) {
        let (Some(pose), Some(texture)) = (pose, self.pokeball_atlas.as_ref()) else {
            return;
        };
        let Some(frame) = generated_pokeballs::frame(pose.ball_frame) else {
            return;
        };
        draw_texture_ex(
            texture,
            pose.ball_x - f32::from(frame.width) * 1.5,
            pose.ball_y - f32::from(frame.height) * 3.0 * 0.625,
            WHITE,
            DrawTextureParams {
                source: Some(Rect::new(
                    f32::from(frame.x),
                    f32::from(frame.y),
                    f32::from(frame.width),
                    f32::from(frame.height),
                )),
                dest_size: Some(vec2(
                    f32::from(frame.width) * 3.0,
                    f32::from(frame.height) * 3.0,
                )),
                rotation: pose.ball_angle.to_radians(),
                ..Default::default()
            },
        );
    }

    fn draw_switch_ball(&self, playback: &BattleAnimationPlayback, field: FieldPresentation) {
        let (Some(switch), Some(texture), Some(frame)) = (
            playback.switch,
            self.pokeball_atlas.as_ref(),
            generated_pokeballs::frame("pb"),
        ) else {
            return;
        };
        let Some(pose) = switch_pose(switch, visual::time()).filter(|pose| pose.ball_visible)
        else {
            return;
        };
        draw_texture_ex(
            texture,
            (pose.ball_x + field.player_x) * 3.0 - f32::from(frame.width) * 1.5,
            pose.ball_y * 3.0 - f32::from(frame.height) * 3.0 * 0.625,
            WHITE,
            DrawTextureParams {
                source: Some(Rect::new(
                    f32::from(frame.x),
                    f32::from(frame.y),
                    f32::from(frame.width),
                    f32::from(frame.height),
                )),
                dest_size: Some(vec2(
                    f32::from(frame.width) * 3.0,
                    f32::from(frame.height) * 3.0,
                )),
                rotation: pose.ball_angle.to_radians(),
                pivot: Some(vec2(
                    (pose.ball_x + field.player_x) * 3.0,
                    pose.ball_y * 3.0,
                )),
                ..Default::default()
            },
        );
    }

    fn draw_switch_open_particles(&self, playback: &BattleAnimationPlayback, now: f64) {
        let (Some(texture), Some(switch)) = (self.pokeball_particles.as_ref(), playback.switch)
        else {
            return;
        };
        let particles_started_at = switch_summon_started_at(switch) + 0.65;
        for index in 0..8_u8 {
            let age = now - (particles_started_at + f64::from(index) * 0.02);
            if !(0.0..0.575).contains(&age) {
                continue;
            }
            let angle = f64::from(index) * std::f64::consts::FRAC_PI_4;
            let progress = (age / 0.575) as f32;
            let alpha = if age <= 0.5 {
                1.0
            } else {
                (((age - 0.5) / 0.075).clamp(0.0, 1.0) * std::f64::consts::FRAC_PI_2).cos() as f32
            };
            let frame_index = (index + 3 + (age * 16.0) as u8) % 4;
            let Some(frame) = generated_capture_particles::frame(frame_index) else {
                continue;
            };
            let x = 106.0 + 48.0 * angle.cos() as f32 * progress;
            let y = 132.0 + 48.0 * angle.sin() as f32 * progress;
            draw_texture_ex(
                texture,
                x * 3.0 - f32::from(frame.source_width) * 1.5 + f32::from(frame.offset_x) * 3.0,
                y * 3.0 - f32::from(frame.source_height) * 1.5 + f32::from(frame.offset_y) * 3.0,
                Color::new(1.0, 1.0, 1.0, alpha),
                DrawTextureParams {
                    source: Some(Rect::new(
                        f32::from(frame.x),
                        f32::from(frame.y),
                        f32::from(frame.width),
                        f32::from(frame.height),
                    )),
                    dest_size: Some(vec2(
                        f32::from(frame.width) * 3.0,
                        f32::from(frame.height) * 3.0,
                    )),
                    ..Default::default()
                },
            );
        }
    }

    fn draw_capture_open_particles(&self, playback: &BattleAnimationPlayback, now: f64) {
        let (Some(texture), Some(capture)) = (self.pokeball_particles.as_ref(), playback.capture)
        else {
            return;
        };
        for index in 0..17_u8 {
            let Some(pose) = capture_open_particle_pose(capture, index, now) else {
                continue;
            };
            let Some(frame) = generated_capture_particles::frame(pose.frame) else {
                continue;
            };
            let mut tint = WHITE;
            tint.a = pose.alpha;
            draw_texture_ex(
                texture,
                pose.x * 3.0 - f32::from(frame.source_width) * 1.5
                    + f32::from(frame.offset_x) * 3.0,
                pose.y * 3.0 - f32::from(frame.source_height) * 1.5
                    + f32::from(frame.offset_y) * 3.0,
                tint,
                DrawTextureParams {
                    source: Some(Rect::new(
                        f32::from(frame.x),
                        f32::from(frame.y),
                        f32::from(frame.width),
                        f32::from(frame.height),
                    )),
                    dest_size: Some(vec2(
                        f32::from(frame.width) * 3.0,
                        f32::from(frame.height) * 3.0,
                    )),
                    ..Default::default()
                },
            );
        }
    }

    fn draw_capture_success_fx(&self, playback: &BattleAnimationPlayback, now: f64) {
        let Some(capture) = playback.capture else {
            return;
        };
        if let (Some(texture), Some(frame)) = (
            self.pokeball_particles.as_ref(),
            generated_capture_particles::frame(4),
        ) {
            for index in 0..3 {
                let Some(pose) = capture_success_star_pose(capture, index, now) else {
                    continue;
                };
                let mut tint = WHITE;
                tint.a = pose.alpha;
                draw_texture_ex(
                    texture,
                    pose.x * 3.0 - f32::from(frame.source_width) * 1.5
                        + f32::from(frame.offset_x) * 3.0,
                    pose.y * 3.0 - f32::from(frame.source_height) * 3.0 * 0.625
                        + f32::from(frame.offset_y) * 3.0,
                    tint,
                    DrawTextureParams {
                        source: Some(Rect::new(
                            f32::from(frame.x),
                            f32::from(frame.y),
                            f32::from(frame.width),
                            f32::from(frame.height),
                        )),
                        dest_size: Some(vec2(
                            f32::from(frame.width) * 3.0,
                            f32::from(frame.height) * 3.0,
                        )),
                        ..Default::default()
                    },
                );
            }
        }

        let (Some(texture), Some(pose)) =
            (self.pokeball_atlas.as_ref(), playback.capture_pose(now))
        else {
            return;
        };
        let prefix = capture.ball.prefix();
        let Some(frame) = generated_pokeballs::frame(prefix) else {
            return;
        };
        let Some(tint_pose) = capture_ball_tint_pose(capture, now) else {
            return;
        };
        draw_texture_ex(
            texture,
            pose.ball_x - f32::from(frame.width) * 1.5,
            pose.ball_y - f32::from(frame.height) * 3.0 * 0.625,
            Color::from_rgba(0, 0, 0, (tint_pose.alpha * 255.0).round() as u8),
            DrawTextureParams {
                source: Some(Rect::new(
                    f32::from(frame.x),
                    f32::from(frame.y),
                    f32::from(frame.width),
                    f32::from(frame.height),
                )),
                dest_size: Some(vec2(
                    f32::from(frame.width) * 3.0,
                    f32::from(frame.height) * 3.0,
                )),
                rotation: pose.ball_angle.to_radians(),
                ..Default::default()
            },
        );
    }

    fn draw_town_platforms(&self, field: FieldPresentation) {
        // EncounterPhase settles the player arena at x=0 and the enemy arena
        // at x=20 after its 2s entrance tween. These are Phaser world units.
        for (texture, source_x) in [
            (self.town_player_platform.as_ref(), field.player_platform_x),
            (self.town_enemy_platform.as_ref(), field.enemy_platform_x),
        ] {
            if let Some(texture) = texture {
                let dest = project_field_layer(source_x, texture.width(), texture.height());
                draw_texture_ex(
                    texture,
                    dest.x,
                    dest.y,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(dest.w, dest.h)),
                        ..Default::default()
                    },
                );
            }
        }
    }

    fn draw_battle_animation(&self, playback: &mut BattleAnimationPlayback) {
        let Some((request, animation, frame)) = playback.current_frame() else {
            return;
        };
        // This slice ports only the source's USER_TARGET/priority-1 graphics
        // used by Tackle and Ember; other focus/z modes remain unsupported.
        if frame.focus != 3 || frame.priority != 1 {
            return;
        }
        let texture = match animation.graphic {
            "PRAS- Strike" => self.battle_anim_strike.as_ref(),
            "PRAS- Fire" => self.battle_anim_fire.as_ref(),
            _ => None,
        };
        let Some(texture) = texture else {
            return;
        };

        // Mirrors Phaser's USER_TARGET transform using the source focus points,
        // then maps source canvas units onto the client's 3x logical canvas.
        let player_half_height = self
            .active_player
            .as_ref()
            .and_then(|sprite| sprite.frames.first())
            .map_or(21.0, |frame| f32::from(frame.source[1]) / 2.0);
        let enemy_half_height = self
            .active_enemy
            .as_ref()
            .and_then(|sprite| sprite.frames.first())
            .map_or(21.0, |frame| f32::from(frame.source[1]) / 2.0);
        let center =
            battle_animation_center(frame, request.actor, player_half_height, enemy_half_height);
        let scale_x = frame.zoom_x / 100.0;
        let scale_y = frame.zoom_y / 100.0;
        let cell = 96.0;
        let source_x = f32::from(frame.frame % (texture.width() as u16 / 96)) * cell;
        let source_y = f32::from(frame.frame / (texture.width() as u16 / 96)) * cell;
        let reversed = request.actor == BattleActor::Enemy;
        let flip_x = frame.mirror ^ reversed;
        let mut tint = WHITE;
        tint.a = f32::from(frame.opacity) / 255.0;
        draw_texture_ex(
            texture,
            center.x - cell * scale_x * 1.5,
            center.y - cell * scale_y * 1.5,
            tint,
            DrawTextureParams {
                dest_size: Some(vec2(cell * scale_x * 3.0, cell * scale_y * 3.0)),
                source: Some(Rect::new(source_x, source_y, cell, cell)),
                rotation: -frame.angle.to_radians(),
                flip_x,
                ..Default::default()
            },
        );
    }
}

fn draw_fallback_sprite(x: f32, y: f32, name: &str, color: Color) {
    draw_ellipse(
        x,
        y + 35.0,
        58.0,
        14.0,
        0.0,
        Color::from_rgba(66, 95, 69, 55),
    );
    draw_ellipse(x, y, 34.0, 42.0, 0.0, color);
    let initial = name.chars().next().unwrap_or('?').to_string();
    draw_text(&initial, x - 8.0, y + 9.0, 26.0, WHITE);
    draw_text("sprite unavailable", x - 64.0, y + 66.0, 13.0, INK);
}

fn draw_hp_card(
    sprites: &SpriteAtlas,
    player: bool,
    species: Species,
    title: &str,
    level: u8,
    hp: i32,
    max_hp: i32,
    boss_segments: u8,
    offset_x: f32,
    gender: pokerogue_core::Gender,
    exp: u32,
) {
    draw_hp_card_with_ratio(
        sprites,
        player,
        species,
        title,
        level,
        hp,
        None,
        max_hp,
        boss_segments,
        offset_x,
        gender,
        exp,
    );
}

fn draw_hp_card_with_ratio(
    sprites: &SpriteAtlas,
    player: bool,
    species: Species,
    title: &str,
    level: u8,
    hp: i32,
    hp_ratio_override: Option<f32>,
    max_hp: i32,
    boss_segments: u8,
    offset_x: f32,
    gender: pokerogue_core::Gender,
    exp: u32,
) {
    const PIXEL_SCALE: f32 = 3.0;
    let (art, x, y, art_width, art_height, name_x, name_y, bar_x, bar_y, bar) = if player {
        let width = sprites
            .player_info
            .as_ref()
            .map_or(130.0, |texture| texture.width());
        let height = sprites
            .player_info
            .as_ref()
            .map_or(42.0, |texture| texture.height());
        (
            sprites.player_info.as_ref(),
            (310.0 - width) * PIXEL_SCALE,
            (180.0 - 72.0 - height / 2.0) * PIXEL_SCALE,
            width,
            height,
            (310.0 - 115.0) * PIXEL_SCALE,
            (180.0 - 72.0 - 15.2) * PIXEL_SCALE,
            (310.0 - 61.0) * PIXEL_SCALE,
            (180.0 - 72.0 - 1.0) * PIXEL_SCALE,
            sprites.hp_bar.as_ref(),
        )
    } else {
        let is_boss = boss_segments > 0;
        let art = if is_boss {
            sprites
                .enemy_boss_info
                .as_ref()
                .or(sprites.enemy_info.as_ref())
        } else {
            sprites.enemy_info.as_ref()
        };
        let (width, height) =
            art.map_or((130.0, 31.0), |texture| (texture.width(), texture.height()));
        (
            art,
            (140.0 - width) * PIXEL_SCALE,
            (180.0 - 141.0 - height / 2.0) * PIXEL_SCALE,
            width,
            height,
            (140.0 - 124.0) * PIXEL_SCALE,
            (180.0 - 141.0 - 11.2) * PIXEL_SCALE,
            (140.0 - 71.0 - if is_boss { 38.0 } else { 0.0 }) * PIXEL_SCALE,
            (180.0 - 141.0 + 4.5 + if is_boss { 2.0 } else { 0.0 }) * PIXEL_SCALE,
            if is_boss {
                sprites.boss_hp_bar.as_ref().or(sprites.hp_bar.as_ref())
            } else {
                sprites.hp_bar.as_ref()
            },
        )
    };
    let (x, name_x, bar_x) = (x + offset_x, name_x + offset_x, bar_x + offset_x);
    if let Some(art) = art {
        draw_texture_ex(
            art,
            x,
            y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(art_width * PIXEL_SCALE, art_height * PIXEL_SCALE)),
                ..Default::default()
            },
        );
    } else {
        let w = art_width * PIXEL_SCALE;
        let h = art_height * PIXEL_SCALE;
        draw_rectangle(x, y, w, h, Color::from_rgba(250, 251, 239, 248));
        draw_rectangle_lines(x, y, w, h, 2.0, Color::from_rgba(104, 126, 108, 255));
    }
    if !player && visual::enemy_owned_icon() {
        if let Some(icon) = &sprites.owned_icon {
            draw_texture_ex(
                icon,
                name_x,
                name_y + 11.75 * PIXEL_SCALE,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(7.0 * PIXEL_SCALE, 7.0 * PIXEL_SCALE)),
                    ..Default::default()
                },
            );
        }
    }
    let anchor_x = (if player { 310.0 } else { 140.0 }) * 3.0 + offset_x;
    let anchor_y = (if player { 108.0 } else { 39.0 }) * 3.0;
    let (type1, type2) = species.types();
    let type_key = |element: pokerogue_data::Element| format!("{:?}", element).to_lowercase();
    if player {
        if let Some(second) = type2 {
            if let Some(atlas) = &sprites.player_type1 {
                atlas.draw_at(
                    &type_key(type1),
                    anchor_x - 139.0 * PIXEL_SCALE,
                    anchor_y - 17.0 * PIXEL_SCALE,
                    PIXEL_SCALE,
                );
            }
            if let Some(atlas) = &sprites.player_type2 {
                atlas.draw_at(
                    &type_key(second),
                    anchor_x - 139.0 * PIXEL_SCALE,
                    anchor_y - PIXEL_SCALE,
                    PIXEL_SCALE,
                );
            }
        } else if let Some(atlas) = &sprites.player_type {
            atlas.draw_at(
                &type_key(type1),
                anchor_x - 154.0 * PIXEL_SCALE,
                anchor_y - 17.0 * PIXEL_SCALE,
                PIXEL_SCALE,
            );
        }
    } else if let Some(second) = type2 {
        if let Some(atlas) = &sprites.enemy_type1 {
            atlas.draw_at(
                &type_key(type1),
                anchor_x - 15.0 * PIXEL_SCALE,
                anchor_y - 15.5 * PIXEL_SCALE,
                PIXEL_SCALE,
            );
        }
        if let Some(atlas) = &sprites.enemy_type2 {
            atlas.draw_at(
                &type_key(second),
                anchor_x - 15.0 * PIXEL_SCALE,
                anchor_y - 2.5 * PIXEL_SCALE,
                PIXEL_SCALE,
            );
        }
    } else if let Some(atlas) = &sprites.enemy_type {
        atlas.draw_at(
            &type_key(type1),
            anchor_x - 15.0 * PIXEL_SCALE,
            anchor_y - 15.5 * PIXEL_SCALE,
            PIXEL_SCALE,
        );
    }
    sprites.hud.name(title, name_x, name_y);
    let gender = visual::gender(player)
        .map(|value| match value {
            "female" => pokerogue_core::Gender::Female,
            "male" => pokerogue_core::Gender::Male,
            _ => pokerogue_core::Gender::Genderless,
        })
        .unwrap_or(gender);
    if let Some(gender) = match gender {
        pokerogue_core::Gender::Male => Some("male"),
        pokerogue_core::Gender::Female => Some("female"),
        pokerogue_core::Gender::Genderless => None,
    } {
        sprites.hud.gender(title, gender, name_x, name_y);
    }
    sprites.hud.level(player, level, anchor_x, anchor_y);
    hud::Hud::image(
        sprites.hud.hp_label.as_ref(),
        bar_x - 3.0,
        bar_y - 9.0,
        vec2(1.0, 0.0),
    );

    if player {
        if let Some(texture) = &sprites.hud.exp_bar {
            let base = pokerogue_data::total_exp_for_level(level, species.growth_rate());
            let next =
                pokerogue_data::total_exp_for_level(level.saturating_add(1), species.growth_rate());
            let progress = if level >= 100 || next <= base {
                0.0
            } else {
                (exp.saturating_sub(base) as f32 / (next - base) as f32).clamp(0.0, 1.0)
            };
            let source_width = texture.width() * progress;
            // PlayerBattleInfo places the EXP line at (-98, 18) from its
            // right-aligned container anchor and crops its 85px source strip.
            let bar_x = anchor_x - 98.0 * PIXEL_SCALE;
            let bar_y = anchor_y + 18.0 * PIXEL_SCALE;
            if source_width > 0.0 {
                draw_texture_ex(
                    texture,
                    bar_x,
                    bar_y,
                    WHITE,
                    DrawTextureParams {
                        source: Some(Rect::new(0.0, 0.0, source_width, texture.height())),
                        dest_size: Some(vec2(
                            source_width * PIXEL_SCALE,
                            texture.height() * PIXEL_SCALE,
                        )),
                        ..Default::default()
                    },
                );
            }
        }
    }

    let ratio = hp_ratio_override
        .unwrap_or_else(|| (hp.max(0) as f32 / max_hp.max(1) as f32).clamp(0.0, 1.0));
    let hp_atlas = if boss_segments > 0 {
        sprites.hud.boss_hp.as_ref()
    } else {
        sprites.hud.hp.as_ref()
    };
    if let Some(atlas) = hp_atlas {
        let key = if ratio > 0.5 {
            "high"
        } else if ratio > 0.25 {
            "medium"
        } else {
            "low"
        };
        if let Some(rect) = atlas.frame(key) {
            draw_texture_ex(
                &atlas.texture,
                bar_x.round(),
                // Phaser renders this 4.5-unit enemy offset at 6× and the
                // reference capture samples every second backing pixel; its
                // 130.5 output coordinate therefore begins on row 130.
                bar_y.floor(),
                WHITE,
                DrawTextureParams {
                    source: Some(rect),
                    dest_size: Some(vec2(rect.w * 3.0 * ratio, rect.h * 3.0)),
                    ..Default::default()
                },
            );
            if boss_segments > 1 {
                for segment in 1..boss_segments {
                    draw_rectangle(
                        bar_x + rect.w * 3.0 * f32::from(segment) / f32::from(boss_segments),
                        bar_y,
                        3.0,
                        rect.h * 3.0,
                        DARKGRAY,
                    );
                }
            }
        }
    } else if let Some(bar) = bar {
        let bar_width = bar.width() * PIXEL_SCALE;
        let frame_height = bar.height() / 3.0;
        let frame_y = frame_height
            * if ratio > 0.5 {
                0.0
            } else if ratio > 0.25 {
                1.0
            } else {
                2.0
            };
        let bar_height = frame_height * PIXEL_SCALE;
        draw_texture_ex(
            bar,
            bar_x,
            bar_y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(bar_width * ratio, bar_height)),
                source: Some(Rect::new(0.0, frame_y, bar.width() * ratio, frame_height)),
                ..Default::default()
            },
        );
        if boss_segments > 1 {
            for segment in 1..boss_segments {
                let marker_x = bar_x + bar_width * f32::from(segment) / f32::from(boss_segments);
                draw_rectangle(
                    marker_x - 1.5,
                    bar_y,
                    3.0,
                    bar_height,
                    Color::from_rgba(62, 48, 36, 220),
                );
            }
        }
    } else {
        let bar_width = art_width * PIXEL_SCALE - 50.0;
        draw_rectangle(bar_x, bar_y, bar_width, 15.0, DARKGRAY);
        draw_rectangle(
            bar_x,
            bar_y,
            bar_width * ratio,
            15.0,
            if ratio > 0.5 { GREEN } else { RED },
        );
    }
    if player {
        sprites
            .hud
            .hp_numbers(hp, max_hp, anchor_x - 45.0, anchor_y + 30.0);
        hud::Hud::image(
            sprites.hud.exp_label.as_ref(),
            anchor_x - 91.0 * 3.0,
            anchor_y + 20.0 * 3.0,
            vec2(1.0, 1.0),
        );
    }
}

fn draw_battlefield_backdrop(background: Option<&Texture2D>) {
    if let Some(background) = background {
        draw_texture_ex(
            background,
            0.0,
            0.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(LOGICAL_WIDTH, LOGICAL_HEIGHT)),
                ..Default::default()
            },
        );
        return;
    }
    draw_rectangle(
        0.0,
        57.0,
        960.0,
        155.0,
        Color::from_rgba(181, 219, 226, 255),
    );
    draw_rectangle(
        0.0,
        212.0,
        960.0,
        38.0,
        Color::from_rgba(168, 206, 183, 255),
    );
    draw_rectangle(
        0.0,
        250.0,
        960.0,
        80.0,
        Color::from_rgba(131, 177, 130, 255),
    );
    draw_rectangle(
        0.0,
        318.0,
        960.0,
        12.0,
        Color::from_rgba(104, 151, 111, 255),
    );

    // Stepped clouds, sun, and layered hills give the sprite field a soft
    // pixel-art backdrop without bundling additional assets.
    let cloud = Color::from_rgba(240, 247, 231, 225);
    for (x, y, w) in [
        (132.0, 110.0, 96.0),
        (386.0, 84.0, 124.0),
        (730.0, 145.0, 88.0),
    ] {
        draw_rectangle(x + 18.0, y - 10.0, w - 36.0, 12.0, cloud);
        draw_rectangle(x, y + 2.0, w, 14.0, cloud);
        draw_rectangle(x + 12.0, y + 16.0, w - 24.0, 7.0, cloud);
    }
    draw_rectangle(
        806.0,
        88.0,
        34.0,
        34.0,
        Color::from_rgba(249, 220, 137, 255),
    );
    draw_rectangle(
        798.0,
        96.0,
        50.0,
        18.0,
        Color::from_rgba(249, 220, 137, 255),
    );

    let hill_back = Color::from_rgba(142, 191, 163, 255);
    let hill_front = Color::from_rgba(115, 171, 143, 255);
    for (x, y, w, h) in [
        (-60.0, 214.0, 250.0, 28.0),
        (66.0, 195.0, 270.0, 47.0),
        (265.0, 217.0, 260.0, 25.0),
        (470.0, 196.0, 300.0, 46.0),
        (720.0, 212.0, 300.0, 30.0),
    ] {
        draw_rectangle(x, y, w, h, hill_back);
    }
    for (x, y, w, h) in [
        (-25.0, 239.0, 315.0, 13.0),
        (250.0, 234.0, 330.0, 18.0),
        (570.0, 240.0, 420.0, 12.0),
    ] {
        draw_rectangle(x, y, w, h, hill_front);
    }

    for x in (18..960).step_by(46) {
        let y = 257.0 + ((x / 46) % 3) as f32 * 9.0;
        draw_rectangle(x as f32, y, 3.0, 8.0, Color::from_rgba(178, 207, 143, 210));
        draw_rectangle(
            x as f32 + 7.0,
            y + 8.0,
            2.0,
            5.0,
            Color::from_rgba(94, 145, 103, 220),
        );
    }
}

fn draw_title_screen(
    sprites: &SpriteAtlas,
    state: &title_screen::TitleScreenState,
    _wave_limit: u8,
    notice: &str,
) {
    let event_active = title_screen::current_halloween_2026_active();
    let title_logo = if event_active {
        sprites
            .halloween_logo
            .as_ref()
            .or(sprites.title_logo.as_ref())
    } else {
        sprites.title_logo.as_ref()
    };
    if let Some(logo) = title_logo {
        let size = logo.size();
        let scale = (450.0 / size.x).min(99.0 / size.y);
        let width = size.x * scale;
        let height = size.y * scale;
        draw_texture_ex(
            logo,
            (LOGICAL_WIDTH - width) * 0.5,
            24.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(width, height)),
                ..Default::default()
            },
        );
    }
    let version = "v1.12.2.3 (Beta)";
    draw_text(
        version,
        234.0,
        145.0,
        17.0,
        Color::from_rgba(232, 232, 168, 255),
    );
    let online = "? Players Online";
    let online_width = measure_text(online, None, 24, 1.0).width;
    draw_text(
        online,
        LOGICAL_WIDTH - 8.0 - online_width,
        257.0,
        24.0,
        WHITE,
    );
    let guest = "Logged in as Guest";
    let guest_width = measure_text(guest, None, 24, 1.0).width;
    draw_text(guest, LOGICAL_WIDTH - 8.0 - guest_width, 288.0, 24.0, WHITE);

    if event_active {
        if let Some(banner) = &sprites.halloween_banner {
            draw_texture_ex(
                banner,
                112.0,
                167.0,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(524.0, 334.0)),
                    ..Default::default()
                },
            );
            let seconds_remaining = 1_794_182_400u64.saturating_sub(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |duration| duration.as_secs()),
            );
            let days = seconds_remaining / 86_400;
            let hours = seconds_remaining % 86_400 / 3_600;
            let minutes = seconds_remaining % 3_600 / 60;
            let seconds = seconds_remaining % 60;
            draw_text(
                &format!("Event Ends in: {days:02}d {hours:02}h {minutes:02}m {seconds:02}s"),
                147.0,
                520.0,
                24.0,
                WHITE,
            );
        }
    }

    let panel = Rect::new(716.0, 310.0, 240.0, 228.0);
    if let Some(frame) = &sprites.window_border {
        draw_texture_ex(
            frame,
            panel.x,
            panel.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(panel.w, panel.h)),
                ..Default::default()
            },
        );
    } else {
        draw_rectangle(panel.x, panel.y, panel.w, panel.h, PANEL);
        draw_rectangle_lines(panel.x, panel.y, panel.w, panel.h, 6.0, GREEN);
    }

    let labels = ["TOWN · 10 WAVES", "TOWN · 20 WAVES", "RESUME RUN"];
    for (index, label) in labels.into_iter().enumerate() {
        let y = panel.y + 48.0 + index as f32 * 48.0;
        if state.selected() == index {
            if let Some(cursor) = &sprites.menu_cursor {
                draw_texture_ex(
                    cursor,
                    panel.x + 20.0,
                    y - 20.0,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(18.0, 30.0)),
                        ..Default::default()
                    },
                );
            } else {
                draw_text("▶", panel.x + 17.0, y + 2.0, 21.0, GREEN);
            }
        }
        draw_text(label, panel.x + 43.0, y, 28.0, WHITE);
    }
    draw_text("↑ ↓ SELECT  ENTER", 728.0, 520.0, 13.0, WHITE);
    draw_text("M  AUDIO", 18.0, 522.0, 13.0, WHITE);
    draw_text("OFFLINE · LOCAL SAVE", 18.0, 504.0, 13.0, WHITE);
    if !notice.is_empty() {
        draw_text(notice, 624.0, 318.0, 14.0, WHITE);
    }
}

fn draw_battle_header_text(
    hud: &hud::Hud,
    text: &str,
    baseline: f32,
    color: Color,
    shadow_color: Color,
    font_scale: f32,
    font_scale_aspect: f32,
) {
    if let Some(font) = &hud.font {
        let width =
            measure_text(text, Some(font), 72, 0.5).width * (font_scale * font_scale_aspect / 0.5);
        // BattleScene places the biome/wave and money labels at
        // scaledCanvas.width - 2 with origin (1, 0.5), projected by 3×.
        let x = LOGICAL_WIDTH - 6.0 - width;
        let params = TextParams {
            font: Some(font),
            font_size: 72,
            font_scale,
            font_scale_aspect,
            color: shadow_color,
            ..Default::default()
        };
        draw_text_ex(text, x + 1.75, baseline + 1.75, params.clone());
        draw_text_ex(text, x, baseline, TextParams { color, ..params });
    } else {
        let width = measure_text(text, None, 24, 1.0).width;
        draw_text(text, LOGICAL_WIDTH - 4.0 - width, baseline, 24.0, color);
    }
}

fn format_money(money: u64) -> String {
    let digits = money.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    format!("₽{grouped}")
}

fn move_party_cursor(
    cursor: usize,
    max_slot: usize,
    left: bool,
    right: bool,
    up: bool,
    down: bool,
) -> usize {
    if up {
        if cursor == 0 {
            6
        } else if cursor < 6 {
            cursor - 1
        } else {
            max_slot
        }
    } else if down {
        if cursor < 6 && cursor < max_slot {
            cursor + 1
        } else if cursor < 6 {
            6
        } else {
            0
        }
    } else if left && cursor == 6 {
        0
    } else if left && cursor > 0 {
        0
    } else if right && cursor == 0 {
        if max_slot > 0 { 1 } else { 6 }
    } else if right && cursor == 6 {
        0
    } else {
        cursor
    }
}

fn can_select_replacement(battle: &pokerogue_core::Battle, slot: usize) -> bool {
    battle.player.can_switch_to(slot).is_ok()
}

fn draw_battle(
    battle: &mut pokerogue_core::Battle,
    sprites: &SpriteAtlas,
    animation_playback: &mut BattleAnimationPlayback,
    wave: u8,
    _wave_limit: u8,
    money: u64,
    selected_move: usize,
    ui_state: battle_ui::BattleUiState,
    notice: &str,
    command_cursor: usize,
    ball_cursor: usize,
    ball_wiggle_started_at: f64,
    party_cursor: usize,
    ball_counts: [u8; 5],
    field: FieldPresentation,
) {
    draw_rectangle(0.0, 0.0, LOGICAL_WIDTH, LOGICAL_HEIGHT, BG);
    draw_battlefield_backdrop(sprites.town_background.as_ref());
    sprites.draw_town_platforms(field);
    let player_faint = animation_playback.faint_pose(
        BattleActor::Player,
        battle.player.active_slot(),
        visual::time(),
    );
    let enemy_faint = animation_playback.faint_pose(BattleActor::Enemy, 0, visual::time());
    let capture_pose = animation_playback.capture_pose(visual::time());
    let biome_wave = format!("Town - {wave}");
    draw_battle_header_text(
        &sprites.hud,
        &biome_wave,
        27.0,
        WHITE,
        Color::from_rgba(99, 99, 99, 255),
        0.54,
        0.92,
    );
    draw_battle_header_text(
        &sprites.hud,
        &format_money(money),
        56.5,
        Color::from_rgba(232, 232, 168, 255),
        Color::from_rgba(160, 160, 96, 255),
        0.5,
        1.012,
    );

    sprites.draw(
        battle.player.species,
        battle.enemy.species,
        field,
        player_faint,
        enemy_faint,
        capture_pose,
        animation_playback,
    );
    sprites.draw_switch_ball(animation_playback, field);
    sprites.draw_switch_open_particles(animation_playback, visual::time());
    if capture_pose.is_some_and(|pose| !pose.ball_behind_enemy) {
        sprites.draw_capture_ball(capture_pose);
    }
    sprites.draw_capture_open_particles(animation_playback, visual::time());
    sprites.draw_capture_success_fx(animation_playback, visual::time());
    sprites.draw_battle_animation(animation_playback);
    sprites.draw_health_up_animation(animation_playback, field);
    sprites.draw_damage_numbers(animation_playback, field);
    let (player_display_level, player_display_exp) = animation_playback.experience_sample(
        battle.player.active_slot(),
        battle.player.level,
        battle.player.exp,
        battle.player.species.growth_rate(),
        visual::time(),
    );
    let switch_in_progress = animation_playback.switch.is_some_and(|switch| {
        visual::time() < switch.started_at + switch_animation_duration(switch)
    });
    let (player_hp_ratio, player_display_hp) = animation_playback.hp_bar_sample(
        BattleActor::Player,
        battle.player.active_slot(),
        battle.player.hp,
        battle.player.max_hp,
        visual::time(),
    );
    if !switch_in_progress && !player_faint.is_some_and(|pose| pose.hide_info) {
        draw_hp_card_with_ratio(
            sprites,
            true,
            battle.player.species,
            battle.player.species.name(),
            player_display_level,
            player_display_hp,
            Some(player_hp_ratio),
            battle.player.max_hp,
            0,
            field.player_x * 3.0,
            battle.player.gender,
            player_display_exp,
        );
    }
    if !enemy_faint.is_some_and(|pose| pose.hide_info)
        && !capture_pose.is_some_and(|pose| pose.hide_info)
    {
        let (enemy_hp_ratio, _) = animation_playback.hp_bar_sample(
            BattleActor::Enemy,
            0,
            battle.enemy.hp,
            battle.enemy.max_hp,
            visual::time(),
        );
        draw_hp_card_with_ratio(
            sprites,
            false,
            battle.enemy.species,
            battle.enemy.species.name(),
            battle.enemy.level,
            battle.enemy.hp,
            Some(enemy_hp_ratio),
            battle.enemy.max_hp,
            battle.enemy.boss_segments,
            field.enemy_x * 3.0,
            battle.enemy.gender,
            battle.enemy.exp,
        );
    }
    let prompt = battle.player.species.name();
    let move_names = battle.player.moves[..battle.player.move_count()]
        .iter()
        .map(|movement| movement.name())
        .collect::<Vec<_>>();
    let (enemy_type1, enemy_type2) = battle.enemy.species.types();
    let mut defender_types = vec![enemy_type1];
    if let Some(enemy_type2) = enemy_type2 {
        defender_types.push(enemy_type2);
    }
    let move_effectiveness = battle.player.moves[..battle.player.move_count()]
        .iter()
        .map(|movement| pokerogue_data::type_effectiveness(movement.element(), &defender_types))
        .collect::<Vec<_>>();
    let move_is_status = battle.player.moves[..battle.player.move_count()]
        .iter()
        .map(|movement| movement.is_status())
        .collect::<Vec<_>>();
    let selected_move_info = (selected_move < battle.player.move_count()).then(|| {
        let movement = battle.player.moves[selected_move];
        battle_ui::MoveInfo {
            type_name: format!("{:?}", movement.element()).to_uppercase(),
            pp: battle.player.move_pp_remaining(selected_move).unwrap_or(0),
            max_pp: battle.player.move_pp_max(selected_move).unwrap_or(0),
            power: movement.power(),
            accuracy: movement.accuracy(),
            is_status: movement.is_status(),
            effectiveness: pokerogue_data::type_effectiveness(movement.element(), &defender_types),
        }
    });
    let party = (0..battle.player.len())
        .filter_map(|slot| battle.player.member(slot))
        .map(|member| battle_ui::PartyMemberView {
            name: member.species.name(),
            species_id: member.species.numeric_id(),
            level: member.level,
            hp: member.hp,
            max_hp: member.max_hp,
            gender: member.gender,
        })
        .collect::<Vec<_>>();
    let party_assets = battle_ui::PartyUiAssets {
        background: sprites.party_background.as_ref(),
        main_slot: sprites.party_main_slot.as_ref(),
        bench_slot: sprites.party_bench_slot.as_ref(),
        pokeball: sprites.party_pokeball.as_ref(),
        hp_bar: sprites.party_hp_bar.as_ref(),
        hp_overlay: sprites.party_hp_overlay.as_ref(),
        hp_label: sprites.party_hp_label.as_ref(),
        level_label: sprites.party_level_label.as_ref(),
        cancel: sprites.party_cancel.as_ref(),
        icon_atlases: &sprites.pokemon_icon_atlases,
    };
    let switch_message = animation_playback
        .switch
        .and_then(|switch| switch_message(switch, battle, visual::time()));
    let battle_message = if notice.is_empty() {
        switch_message.as_deref().unwrap_or(&battle.log)
    } else {
        notice
    };
    battle_ui::draw(
        ui_state,
        sprites.message_window.as_ref(),
        sprites.window_border.as_ref(),
        &sprites.hud,
        sprites.move_types.as_ref(),
        sprites.move_categories.as_ref(),
        sprites.menu_cursor.as_ref(),
        prompt,
        battle_message,
        &move_names,
        &move_effectiveness,
        &move_is_status,
        selected_move,
        command_cursor,
        ball_cursor,
        sprites.ball_menu_atlas.as_ref(),
        (visual::time() - ball_wiggle_started_at).max(0.0),
        party_cursor,
        &party,
        &party_assets,
        ball_counts,
        selected_move_info.as_ref(),
        field.enemy_x * 3.0,
    );
    if battle.outcome.is_some() {
        draw_text("ENTER  Play again", 35.0, 500.0, 19.0, INK);
    } else if battle.needs_replacement() {
        for slot in 0..battle.player.len() {
            if let Some(member) = battle.player.member(slot) {
                let x = 35.0 + (slot % 3) as f32 * 325.0;
                let y = 490.0 + (slot / 3) as f32 * 27.0;
                draw_text(
                    &format!(
                        "[{}] {} {}/{}",
                        slot + 1,
                        member.species.name(),
                        member.hp,
                        member.max_hp
                    ),
                    x,
                    y,
                    17.0,
                    if member.hp > 0 { GREEN } else { DARKGRAY },
                );
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn save_path() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    base.join("pokerogue-rust").join("mini-run.sav")
}

fn throw_battle_ball(current: &mut MiniRun, selected: usize) -> (Option<CaptureBall>, String) {
    let (action, ball) = match selected {
        0 => (PlayerAction::ThrowPokeball, CaptureBall::Poke),
        1 => (PlayerAction::ThrowGreatBall, CaptureBall::Great),
        _ => (PlayerAction::ThrowUltraBall, CaptureBall::Ultra),
    };
    match current.take_action(action) {
        Ok(()) => (Some(ball), String::new()),
        Err(pokerogue_core::MiniRunError::NoGreatBalls) => (None, "No Great Balls left.".into()),
        Err(pokerogue_core::MiniRunError::NoUltraBalls) => (None, "No Ultra Balls left.".into()),
        Err(pokerogue_core::MiniRunError::Turn(
            pokerogue_core::TurnActionError::BossNotCatchable,
        )) => (
            None,
            "Boss shield active: break a segment before using a ball.".into(),
        ),
        Err(error) => (None, format!("Cannot throw ball: {error:?}")),
    }
}

fn save_run(run: &MiniRun) -> Result<(), String> {
    let checkpoint = run
        .checkpoint()
        .map_err(|error| format!("checkpoint unavailable: {error:?}"))?;
    #[cfg(target_arch = "wasm32")]
    {
        quad_storage::STORAGE
            .lock()
            .map_err(|error| error.to_string())?
            .set("pokerogue-rust-mini-run", &checkpoint);
        Ok(())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = save_path();
        let parent = path
            .parent()
            .ok_or_else(|| "invalid local save path".to_owned())?;
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let temporary = path.with_extension("sav.tmp");
        std::fs::write(&temporary, checkpoint).map_err(|error| error.to_string())?;
        std::fs::rename(temporary, path).map_err(|error| error.to_string())
    }
}

fn load_run() -> Result<MiniRun, String> {
    #[cfg(target_arch = "wasm32")]
    let save = quad_storage::STORAGE
        .lock()
        .map_err(|error| error.to_string())?
        .get("pokerogue-rust-mini-run")
        .ok_or_else(|| "no browser save found".to_owned())?;
    #[cfg(not(target_arch = "wasm32"))]
    let save = std::fs::read_to_string(save_path()).map_err(|error| error.to_string())?;
    MiniRun::from_checkpoint(&save).map_err(|error| format!("invalid save: {error:?}"))
}

#[macroquad::main(window_conf)]
async fn main() {
    let visual_started_at = visual::time();
    #[cfg(not(target_arch = "wasm32"))]
    let screenshot_path = std::env::var_os("POKEROGUE_SCREENSHOT_PATH").map(PathBuf::from);
    #[cfg(target_arch = "wasm32")]
    let screenshot_path: Option<PathBuf> = None;
    if visual::run_if_requested().await {
        return;
    }
    let mut sprites = SpriteAtlas::load().await;
    sprites
        .set_battlers(Species::Bulbasaur, Species::Charmander)
        .await;
    let mut audio_manager = audio::AudioManager::new();
    #[cfg(not(target_arch = "wasm32"))]
    let visual_battle = std::env::var_os("POKEROGUE_VISUAL_BATTLE").is_some();
    #[cfg(target_arch = "wasm32")]
    let visual_battle = false;
    let mut run: Option<MiniRun> = visual_battle
        .then(|| MiniRun::new_with_wave_limit("MiniRunSeed0001", "DAY", 10).ok())
        .flatten();
    let mut animation_playback = BattleAnimationPlayback::default();
    let mut last_displayed_wave = None;
    let mut encounter_entrance_playback: Option<EncounterEntrance> = None;
    let mut selected_wave_limit = MiniRun::DEFAULT_WAVE_LIMIT;
    let mut title_state = title_screen::TitleScreenState::default();
    let mut notice = String::new();
    let mut reward_target = 0usize;
    let mut reward_move = 0usize;
    let mut reward_selected = 1usize;
    let mut reward_screen_wave = None;
    let mut reward_screen_started_at = 0.0;
    let mut selected_move = 0usize;
    let mut ui_state = battle_ui::BattleUiState::Command;
    let mut command_cursor = 0usize;
    let mut ball_cursor = 0usize;
    let mut ball_wiggle_started_at = visual::time();
    let mut party_cursor = 0usize;
    let mut ui_message_started_at = 0.0;
    let mut visual_capture_ready;
    let mut natural_offer_cache: Option<(
        String,
        u8,
        Option<pokerogue_core::rewards::NaturalRewardOffer>,
    )> = None;
    loop {
        set_logical_viewport();
        if is_key_pressed(KeyCode::M) {
            audio_manager.toggle();
        }
        audio_manager.update(visual::time());
        if let Some(current) = &mut run {
            if ui_state == battle_ui::BattleUiState::Message
                && !animation_playback.is_busy()
                && visual::time() - ui_message_started_at >= 2.0
            {
                ui_state = battle_ui::BattleUiState::Command;
                notice.clear();
            }
            sprites
                .set_battlers(current.battle.player.species, current.battle.enemy.species)
                .await;
            let field = if current.stage == MiniRunStage::Battle {
                if last_displayed_wave != Some(current.wave) {
                    animation_playback.clear_faint();
                    let kind = if last_displayed_wave.is_none() && current.wave == 1 {
                        EncounterEntranceKind::First
                    } else {
                        EncounterEntranceKind::NextWave
                    };
                    encounter_entrance_playback = Some(EncounterEntrance {
                        kind,
                        started_at: visual::time(),
                    });
                    last_displayed_wave = Some(current.wave);
                }
                encounter_entrance_playback
                    .as_ref()
                    .map_or_else(final_field_presentation, |entrance| {
                        entrance.presentation(visual::time())
                    })
            } else {
                final_field_presentation()
            };
            let presentation_ui_state = if current.battle.needs_replacement() {
                battle_ui::BattleUiState::Party
            } else {
                encounter_entrance_playback
                    .as_ref()
                    .filter(|entrance| {
                        show_first_entry_message(
                            entrance.kind,
                            visual::time() - entrance.started_at,
                        )
                    })
                    .map_or(ui_state, |_| battle_ui::BattleUiState::Message)
            };
            draw_battle(
                &mut current.battle,
                &sprites,
                &mut animation_playback,
                current.wave,
                current.wave_limit,
                current.money,
                selected_move,
                presentation_ui_state,
                &notice,
                command_cursor,
                ball_cursor,
                ball_wiggle_started_at,
                party_cursor,
                [
                    current.pokeballs,
                    current.greatballs,
                    current.ultraballs,
                    0,
                    0,
                ],
                field,
            );
            visual_capture_ready = screenshot_path.is_some()
                && current.stage == MiniRunStage::Battle
                && encounter_entrance_playback.as_ref().is_none_or(|entrance| {
                    visual::time() - entrance.started_at >= EncounterEntrance::DURATION_SECONDS
                })
                && !animation_playback.is_busy();
            match current.stage {
                MiniRunStage::Battle
                    if encounter_entrance_playback
                        .as_ref()
                        .is_some_and(|entrance| {
                            visual::time() - entrance.started_at
                                < EncounterEntrance::DURATION_SECONDS
                        })
                        || animation_playback.is_busy() => {}
                MiniRunStage::MoveLearning
                | MiniRunStage::Reward
                | MiniRunStage::Complete
                | MiniRunStage::Lost
                    if animation_playback.is_busy() => {}
                MiniRunStage::Battle => {
                    let events_before_action = current.battle.last_events.clone();
                    let turn_before_action = current.battle.turn;
                    let hp_before_action = BattleHpSnapshot::capture(&current.battle);
                    let mut capture_ball = None;
                    if current.battle.needs_replacement() {
                        ui_state = battle_ui::BattleUiState::Party;
                        if is_key_pressed(KeyCode::Left)
                            || is_key_pressed(KeyCode::Right)
                            || is_key_pressed(KeyCode::Up)
                            || is_key_pressed(KeyCode::Down)
                        {
                            let previous_cursor = party_cursor;
                            party_cursor = move_party_cursor(
                                party_cursor,
                                current.battle.player.len().saturating_sub(1),
                                is_key_pressed(KeyCode::Left),
                                is_key_pressed(KeyCode::Right),
                                is_key_pressed(KeyCode::Up),
                                is_key_pressed(KeyCode::Down),
                            );
                            if previous_cursor != party_cursor {
                                audio_manager.play(audio::Cue::Navigate);
                            }
                        } else if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Z) {
                            if can_select_replacement(&current.battle, party_cursor) {
                                if current.replace_fainted_player(party_cursor).is_ok() {
                                    ui_state = battle_ui::BattleUiState::Command;
                                }
                            } else {
                                audio_manager.play(audio::Cue::Cancel);
                            }
                        }
                    } else if is_key_pressed(KeyCode::F) {
                        ui_state = match ui_state {
                            battle_ui::BattleUiState::Command => battle_ui::BattleUiState::Moves,
                            _ => battle_ui::BattleUiState::Command,
                        };
                    } else if is_key_pressed(KeyCode::Left)
                        || is_key_pressed(KeyCode::Right)
                        || is_key_pressed(KeyCode::Up)
                        || is_key_pressed(KeyCode::Down)
                    {
                        let previous_cursor = match ui_state {
                            battle_ui::BattleUiState::Command => command_cursor,
                            battle_ui::BattleUiState::Moves => selected_move,
                            battle_ui::BattleUiState::Balls => ball_cursor,
                            battle_ui::BattleUiState::Party => party_cursor,
                            battle_ui::BattleUiState::Message => 0,
                        };
                        let left = is_key_pressed(KeyCode::Left);
                        let right = is_key_pressed(KeyCode::Right);
                        let up = is_key_pressed(KeyCode::Up);
                        let down = is_key_pressed(KeyCode::Down);
                        match ui_state {
                            battle_ui::BattleUiState::Command => {
                                if up && command_cursor >= 2 {
                                    command_cursor -= 2;
                                } else if down && command_cursor < 2 {
                                    command_cursor += 2;
                                } else if left && command_cursor % 2 == 1 {
                                    command_cursor -= 1;
                                } else if right && command_cursor % 2 == 0 {
                                    command_cursor += 1;
                                }
                            }
                            battle_ui::BattleUiState::Moves => {
                                if up && selected_move >= 2 {
                                    selected_move -= 2;
                                } else if down && selected_move < 2 {
                                    selected_move = (selected_move + 2).min(3);
                                } else if left && selected_move % 2 == 1 {
                                    selected_move -= 1;
                                } else if right && selected_move % 2 == 0 {
                                    selected_move = (selected_move + 1).min(3);
                                }
                            }
                            battle_ui::BattleUiState::Balls => {
                                ball_cursor = battle_ui::move_ball_cursor(ball_cursor, up, down);
                            }
                            battle_ui::BattleUiState::Party => {
                                let max_slot = current.battle.player.len().saturating_sub(1);
                                party_cursor = move_party_cursor(
                                    party_cursor,
                                    max_slot,
                                    left,
                                    right,
                                    up,
                                    down,
                                );
                            }
                            battle_ui::BattleUiState::Message => {}
                        }
                        let current_cursor = match ui_state {
                            battle_ui::BattleUiState::Command => command_cursor,
                            battle_ui::BattleUiState::Moves => selected_move,
                            battle_ui::BattleUiState::Balls => ball_cursor,
                            battle_ui::BattleUiState::Party => party_cursor,
                            battle_ui::BattleUiState::Message => 0,
                        };
                        if ui_state == battle_ui::BattleUiState::Balls
                            && current_cursor != previous_cursor
                        {
                            ball_wiggle_started_at = visual::time();
                        }
                        if audio::input_cue(previous_cursor != current_cursor)
                            == Some(audio::Cue::Navigate)
                        {
                            audio_manager.play(audio::Cue::Navigate);
                        }
                    } else if is_key_pressed(KeyCode::X) {
                        if ui_state != battle_ui::BattleUiState::Command {
                            audio_manager.play(audio::cue_for_ui_action(audio::UiAction::Cancel));
                            ui_state = battle_ui::BattleUiState::Command;
                        }
                    } else if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Z) {
                        audio_manager.play(audio::cue_for_ui_action(audio::UiAction::Confirm));
                        match ui_state {
                            battle_ui::BattleUiState::Command => match command_cursor {
                                0 => {
                                    ui_state = battle_ui::BattleUiState::Moves;
                                    selected_move = 0;
                                }
                                1 => {
                                    ui_state = battle_ui::BattleUiState::Balls;
                                    ball_cursor = 0;
                                    ball_wiggle_started_at = visual::time();
                                }
                                2 => {
                                    ui_state = battle_ui::BattleUiState::Party;
                                    // PartyUiHandler.setup selects slot 0 by default.
                                    party_cursor = 0;
                                }
                                _ => {
                                    notice = "Run is unavailable in this Mini Run.".into();
                                    ui_state = battle_ui::BattleUiState::Message;
                                    ui_message_started_at = visual::time();
                                }
                            },
                            battle_ui::BattleUiState::Moves => {
                                if selected_move < current.battle.player.move_count() {
                                    if let Err(pokerogue_core::MiniRunError::Turn(
                                        pokerogue_core::TurnActionError::NoPp,
                                    )) =
                                        current.take_action(PlayerAction::UseMove(selected_move))
                                    {
                                        let movement = current.battle.player.moves[selected_move];
                                        notice = format!("{} has no PP left.", movement.name());
                                        ui_state = battle_ui::BattleUiState::Message;
                                        ui_message_started_at = visual::time();
                                    }
                                }
                            }
                            battle_ui::BattleUiState::Balls => {
                                if ball_cursor == 5 {
                                    ui_state = battle_ui::BattleUiState::Command;
                                } else if ball_cursor >= 3
                                    || match ball_cursor {
                                        0 => current.pokeballs,
                                        1 => current.greatballs,
                                        _ => current.ultraballs,
                                    } == 0
                                {
                                    audio_manager.play(audio::Cue::Cancel);
                                } else {
                                    (capture_ball, notice) =
                                        throw_battle_ball(current, ball_cursor);
                                    if capture_ball.is_none() && !notice.is_empty() {
                                        ui_state = battle_ui::BattleUiState::Message;
                                        ui_message_started_at = visual::time();
                                    }
                                }
                            }
                            battle_ui::BattleUiState::Party => {
                                if party_cursor == 6 {
                                    ui_state = battle_ui::BattleUiState::Command;
                                } else if current
                                    .battle
                                    .player
                                    .member(party_cursor)
                                    .is_some_and(|member| member.hp > 0)
                                {
                                    if let Err(error) =
                                        current.take_action(PlayerAction::Switch(party_cursor))
                                    {
                                        notice = format!("Cannot switch Pokémon: {error:?}");
                                        ui_state = battle_ui::BattleUiState::Message;
                                        ui_message_started_at = visual::time();
                                    }
                                }
                            }
                            battle_ui::BattleUiState::Message => {}
                        }
                    } else if is_key_pressed(KeyCode::Tab) {
                        if let Some(slot) = current.battle.player.next_switch_target() {
                            let _ = current.take_action(PlayerAction::Switch(slot));
                        }
                    } else if is_key_pressed(KeyCode::S) {
                        current.debug_sand_attack();
                    } else if is_key_pressed(KeyCode::Key1)
                        && ((0..current.battle.player.move_count()).all(|index| {
                            current.battle.player.move_pp_remaining(index).unwrap_or(0) == 0
                        }) || current.battle.player.move_pp_remaining(0).unwrap_or(0) > 0)
                    {
                        selected_move = 0;
                        let _ = current.take_action(PlayerAction::UseMove(0));
                    } else if is_key_pressed(KeyCode::Key2)
                        && current.battle.player.move_count() > 1
                        && ((0..current.battle.player.move_count()).all(|index| {
                            current.battle.player.move_pp_remaining(index).unwrap_or(0) == 0
                        }) || current.battle.player.move_pp_remaining(1).unwrap_or(0) > 0)
                    {
                        selected_move = 1;
                        let _ = current.take_action(PlayerAction::UseMove(1));
                    } else if is_key_pressed(KeyCode::Key3)
                        && current.battle.player.move_count() > 2
                        && ((0..current.battle.player.move_count()).all(|index| {
                            current.battle.player.move_pp_remaining(index).unwrap_or(0) == 0
                        }) || current.battle.player.move_pp_remaining(2).unwrap_or(0) > 0)
                    {
                        selected_move = 2;
                        let _ = current.take_action(PlayerAction::UseMove(2));
                    } else if is_key_pressed(KeyCode::Key4)
                        && current.battle.player.move_count() > 3
                        && ((0..current.battle.player.move_count()).all(|index| {
                            current.battle.player.move_pp_remaining(index).unwrap_or(0) == 0
                        }) || current.battle.player.move_pp_remaining(3).unwrap_or(0) > 0)
                    {
                        selected_move = 3;
                        let _ = current.take_action(PlayerAction::UseMove(3));
                    } else if is_key_pressed(KeyCode::B) {
                        notice = match current.take_action(PlayerAction::ThrowPokeball) {
                            Ok(()) => {
                                capture_ball = Some(CaptureBall::Poke);
                                String::new()
                            }
                            Err(pokerogue_core::MiniRunError::Turn(
                                pokerogue_core::TurnActionError::BossNotCatchable,
                            )) => "Boss shield active: break a segment before using a ball.".into(),
                            Err(error) => format!("Cannot throw ball: {error:?}"),
                        };
                    } else if is_key_pressed(KeyCode::G) {
                        notice = match current.take_action(PlayerAction::ThrowGreatBall) {
                            Ok(()) => {
                                capture_ball = Some(CaptureBall::Great);
                                String::new()
                            }
                            Err(pokerogue_core::MiniRunError::NoGreatBalls) => {
                                "No Great Balls left.".into()
                            }
                            Err(pokerogue_core::MiniRunError::Turn(
                                pokerogue_core::TurnActionError::BossNotCatchable,
                            )) => "Boss shield active: break a segment before using a ball.".into(),
                            Err(error) => format!("Cannot throw Great Ball: {error:?}"),
                        };
                    } else if is_key_pressed(KeyCode::U) {
                        notice = match current.take_action(PlayerAction::ThrowUltraBall) {
                            Ok(()) => {
                                capture_ball = Some(CaptureBall::Ultra);
                                String::new()
                            }
                            Err(pokerogue_core::MiniRunError::NoUltraBalls) => {
                                "No Ultra Balls left.".into()
                            }
                            Err(pokerogue_core::MiniRunError::Turn(
                                pokerogue_core::TurnActionError::BossNotCatchable,
                            )) => "Boss shield active: break a segment before using a ball.".into(),
                            Err(error) => format!("Cannot throw Ultra Ball: {error:?}"),
                        };
                    }
                    if current.battle.turn != turn_before_action
                        || current.battle.last_events != events_before_action
                    {
                        command_cursor = 0;
                        ui_state = battle_ui::BattleUiState::Message;
                        ui_message_started_at = visual::time();
                        animation_playback.enqueue_turn_with_hp_snapshot(
                            &current.battle,
                            visual::time(),
                            capture_ball,
                            Some(&hp_before_action),
                        );
                        audio_manager.schedule_battle_events(&current.battle, visual::time());
                    }
                }
                MiniRunStage::MoveLearning => {
                    let (party_slot, move_id) = if let Some(pending) = current.pending_level_move()
                    {
                        pending
                    } else if let Some(pending) = current.pending_prevolution_move() {
                        let Some(member) = current.battle.player.member(pending.0) else {
                            continue;
                        };
                        let Some(movement) = pokerogue_data::move_by_numeric_id(pending.1) else {
                            continue;
                        };
                        draw_prevolution_move_screen(
                            current.wave,
                            member.species.name(),
                            movement.name,
                            &notice,
                            sprites.town_background.as_ref(),
                        );
                        if is_key_pressed(KeyCode::F5) {
                            notice = match save_run(current) {
                                Ok(()) => "Saved move notice".into(),
                                Err(error) => error,
                            };
                        } else if is_key_pressed(KeyCode::Enter) {
                            let _ = current.acknowledge_prevolution_move();
                            notice.clear();
                        }
                        continue;
                    } else {
                        continue;
                    };
                    let Some(member) = current.battle.player.member(party_slot) else {
                        notice = "Invalid level-up move target".into();
                        continue;
                    };
                    let Some(movement) = pokerogue_data::move_by_numeric_id(move_id) else {
                        notice = "Unknown level-up move".into();
                        continue;
                    };
                    let known = std::array::from_fn(|index| {
                        (index < member.move_count()).then(|| member.moves[index].name())
                    });
                    draw_move_learning_screen(
                        current.wave,
                        member.species.name(),
                        movement.name,
                        known,
                        &notice,
                        sprites.town_background.as_ref(),
                    );
                    if is_key_pressed(KeyCode::F5) {
                        notice = match save_run(current) {
                            Ok(()) => "Saved pending level-up choice".into(),
                            Err(error) => error,
                        };
                    } else if is_key_pressed(KeyCode::Enter) {
                        let _ = current.resolve_pending_level_move(None);
                        notice.clear();
                    } else if let Some(slot) =
                        [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4]
                            .iter()
                            .position(|key| is_key_pressed(*key))
                    {
                        notice = match current.resolve_pending_level_move(Some(slot)) {
                            Ok(()) => String::new(),
                            Err(error) => format!("Cannot replace move: {error:?}"),
                        };
                    }
                }
                MiniRunStage::Reward => {
                    if reward_screen_wave != Some(current.wave) {
                        reward_screen_wave = Some(current.wave);
                        reward_screen_started_at = visual::time();
                        reward_selected = 1;
                    }
                    if natural_offer_cache.as_ref().is_none_or(|(seed, wave, _)| {
                        seed != &current.seed || *wave != current.wave
                    }) {
                        natural_offer_cache = Some((
                            current.seed.clone(),
                            current.wave,
                            current.natural_supported_reward_offer(),
                        ));
                    }
                    let natural_offer = natural_offer_cache
                        .as_ref()
                        .and_then(|(_, _, offer)| offer.as_ref())
                        .cloned();
                    let title = if current
                        .battle
                        .last_capture_attempt
                        .as_ref()
                        .is_some_and(|attempt| attempt.caught)
                    {
                        format!(
                            "Caught {} · EXP ×{} · Super ×{}",
                            current.battle.enemy.species.name(),
                            current.exp_charm_stacks,
                            current.super_exp_charm_stacks
                        )
                    } else {
                        format!(
                            "Wave clear · EXP ×{} · Super ×{}",
                            current.exp_charm_stacks, current.super_exp_charm_stacks
                        )
                    };
                    let learned_moves = current
                        .battle
                        .last_events
                        .iter()
                        .filter_map(|event| match event {
                            pokerogue_core::BattleEvent::MoveLearned { move_id, .. } => {
                                pokerogue_data::move_by_numeric_id(*move_id)
                                    .map(|movement| movement.name)
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    let reward_notice = if notice.is_empty() && !learned_moves.is_empty() {
                        format!("Learned: {learned_moves}")
                    } else {
                        notice.clone()
                    };
                    let controls = if current.reward_claimed {
                        if current.wave % 10 == 0 {
                            "Fixed boss reward applied. ENTER Continue   F5 Save locally".to_owned()
                        } else {
                            "Reward claimed. ENTER Continue   F5 Save locally".to_owned()
                        }
                    } else if natural_offer.is_some() {
                        "LEFT/RIGHT choose · TAB target · UP/DOWN move · ENTER select · X skip · F5 save"
                            .to_owned()
                    } else {
                        "LEFT/RIGHT choose · TAB target · ENTER select · X skip · F5 save"
                            .to_owned()
                    };
                    reward_target %= current.battle.player.len();
                    let target = current.battle.player.member(reward_target).unwrap();
                    reward_move %= target.move_count();
                    let selected_move = target.moves[reward_move];
                    let replacing_tm = natural_offer.as_ref().is_some_and(|offer| {
                        offer
                            .options
                            .iter()
                            .any(|option| matches!(option.id, "TM_COMMON" | "TM_GREAT"))
                    }) && target.move_count() == 4;
                    let move_info = if replacing_tm {
                        format!(
                            "TM REPLACES SLOT {} · {}",
                            reward_move + 1,
                            selected_move.name()
                        )
                    } else {
                        format!(
                            "MOVE {} PP {}/{}",
                            selected_move.name(),
                            target.move_pp_remaining(reward_move).unwrap_or(0),
                            target.move_pp_max(reward_move).unwrap_or(0),
                        )
                    };
                    let choices = if current.reward_claimed {
                        None
                    } else if let Some(offer) = &natural_offer {
                        Some(std::array::from_fn(|index| {
                            reward_card_for_option(&offer.options[index])
                        }))
                    } else {
                        Some([
                            RewardCard {
                                name: "Poké Ball ×5".into(),
                                icon: "pb".into(),
                                description: "Adds five Poké Balls to your inventory.".into(),
                                tier: 0,
                            },
                            RewardCard {
                                name: "Potion".into(),
                                icon: "potion".into(),
                                description:
                                    "Restores 20 HP or 10% HP for one Pokémon, whichever is higher."
                                        .into(),
                                tier: 0,
                            },
                            RewardCard {
                                name: "Super Potion".into(),
                                icon: "super_potion".into(),
                                description:
                                    "Restores 50 HP or 25% HP for one Pokémon, whichever is higher."
                                        .into(),
                                tier: 0,
                            },
                        ])
                    };
                    let tier = natural_offer
                        .as_ref()
                        .and_then(|offer| offer.options.first())
                        .map(|option| match option.tier {
                            0 => "NATURAL · COMMON",
                            1 => "NATURAL · GREAT",
                            2 => "NATURAL · ULTRA",
                            3 => "NATURAL · ROGUE",
                            4 => "NATURAL · MASTER",
                            _ => "NATURAL REWARD",
                        })
                        .unwrap_or("RECOVERY ITEMS");
                    draw_reward_screen(
                        current.wave,
                        current.wave_limit,
                        current.reward_experience,
                        current.money,
                        &title,
                        choices,
                        tier,
                        reward_selected,
                        (visual::time() - reward_screen_started_at) as f32,
                        &sprites,
                        (
                            target.species.name(),
                            target.hp,
                            target.max_hp,
                            reward_target + 1,
                            &move_info,
                        ),
                        &reward_notice,
                        &controls,
                    );
                    if is_key_pressed(KeyCode::Tab) {
                        reward_target = (reward_target + 1) % current.battle.player.len();
                    }
                    if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::Right) {
                        reward_move = (reward_move + 1) % target.move_count();
                    }
                    if is_key_pressed(KeyCode::Left) {
                        reward_selected = (reward_selected + 2) % 3;
                    } else if is_key_pressed(KeyCode::Right) {
                        reward_selected = (reward_selected + 1) % 3;
                    }
                    if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::Down) {
                        reward_move = (reward_move + 1) % target.move_count();
                    }
                    let selected_option = [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3]
                        .iter()
                        .position(|key| is_key_pressed(*key));
                    let selected_option = selected_option.or_else(|| {
                        (is_key_pressed(KeyCode::Enter) && !current.reward_claimed)
                            .then_some(reward_selected)
                    });
                    if let Some(index) = selected_option {
                        let claim = if natural_offer.is_some() {
                            let move_index = natural_offer
                                .as_ref()
                                .and_then(|offer| offer.options.get(index))
                                .filter(|option| {
                                    option.id == "PP_UP"
                                        || (matches!(option.id, "TM_COMMON" | "TM_GREAT")
                                            && target.move_count() == 4)
                                })
                                .map(|_| reward_move);
                            current.claim_natural_supported_reward_for_move(
                                index,
                                reward_target,
                                move_index,
                            )
                        } else {
                            current.claim_reward(index, reward_target)
                        };
                        notice = match claim {
                            Ok(()) => "Reward applied".into(),
                            Err(_) => {
                                "Reward effect is not implemented or target is invalid".into()
                            }
                        };
                    }
                    if is_key_pressed(KeyCode::F5) {
                        notice = match save_run(current) {
                            Ok(()) => "Saved between waves".into(),
                            Err(error) => error,
                        };
                    } else if is_key_pressed(KeyCode::Enter) && current.reward_claimed {
                        let _ = current.continue_after_reward();
                    } else if is_key_pressed(KeyCode::X) {
                        let _ = current.continue_after_reward();
                    }
                }
                MiniRunStage::Complete => {
                    draw_intermission(
                        current.wave,
                        current.wave_limit,
                        current.reward_experience,
                        "Mini-run complete!",
                        "F5 Save locally   ENTER Return",
                    );
                    if is_key_pressed(KeyCode::F5) {
                        notice = match save_run(current) {
                            Ok(()) => "Completed run saved locally".into(),
                            Err(error) => error,
                        };
                    } else if is_key_pressed(KeyCode::Enter) {
                        run = None;
                    }
                }
                MiniRunStage::Lost => {
                    draw_intermission(
                        current.wave,
                        current.wave_limit,
                        0,
                        "Run ended",
                        "ENTER Return to title",
                    );
                    if is_key_pressed(KeyCode::Enter) {
                        run = None;
                    }
                }
            }
        } else {
            draw_rectangle(0.0, 0.0, 960.0, 540.0, BG);
            draw_battlefield_backdrop(sprites.title_background.as_ref());
            draw_title_screen(&sprites, &title_state, selected_wave_limit, &notice);
            if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::Left) {
                if title_state.navigate(-1) {
                    audio_manager.play(audio::Cue::Navigate);
                }
            } else if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::Right) {
                if title_state.navigate(1) {
                    audio_manager.play(audio::Cue::Navigate);
                }
            } else if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Z) {
                match title_state.confirm() {
                    title_screen::TitleChoice::Town10Waves => {
                        selected_wave_limit = 10;
                        run = MiniRun::new_with_wave_limit(
                            "MiniRunSeed0001",
                            "DAY",
                            selected_wave_limit,
                        )
                        .ok();
                    }
                    title_screen::TitleChoice::Town20Waves => {
                        selected_wave_limit = 20;
                        run = MiniRun::new_with_wave_limit(
                            "MiniRunSeed0667",
                            "DAY",
                            selected_wave_limit,
                        )
                        .ok();
                    }
                    title_screen::TitleChoice::ResumeLocalRun => match load_run() {
                        Ok(loaded) => run = Some(loaded),
                        Err(error) => notice = error,
                    },
                }
                if run.is_some() {
                    audio_manager.play(audio::Cue::Confirm);
                    natural_offer_cache = None;
                    last_displayed_wave = None;
                    encounter_entrance_playback = None;
                    animation_playback = BattleAnimationPlayback::default();
                    notice.clear();
                }
            } else if is_key_pressed(KeyCode::F9) {
                title_state = title_screen::TitleScreenState::default();
                title_state.navigate(2);
                match load_run() {
                    Ok(loaded) => run = Some(loaded),
                    Err(error) => notice = error,
                }
            }
            if !notice.is_empty() {
                draw_text(&notice, 300.0, 412.0, 16.0, DARKGRAY);
            }
            visual_capture_ready =
                screenshot_path.is_some() && visual::time() - visual_started_at >= 2.5;
        }
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        set_default_camera();
        if visual_capture_ready && visual::time() - visual_started_at >= 2.5 {
            if let Some(path) = &screenshot_path {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                get_screen_data().export_png(&path.to_string_lossy());
                eprintln!("visual screenshot: {}", path.display());
                break;
            }
        }
        audio_manager.load_next().await;
        next_frame().await;
    }
}

fn draw_move_learning_screen(
    wave: u8,
    species: &str,
    new_move: &str,
    known_moves: [Option<&str>; 4],
    notice: &str,
    background: Option<&Texture2D>,
) {
    draw_rectangle(0.0, 0.0, 960.0, 540.0, BG);
    draw_battlefield_backdrop(background);
    draw_rectangle(105.0, 110.0, 750.0, 330.0, PANEL);
    draw_rectangle(105.0, 110.0, 750.0, 5.0, GREEN);
    draw_text("MOVE LEARNED", 145.0, 160.0, 27.0, GREEN);
    draw_text(
        &format!("{species} can learn {new_move}"),
        145.0,
        202.0,
        24.0,
        INK,
    );
    draw_text(
        "Choose a move to forget, or ENTER to decline.",
        145.0,
        237.0,
        18.0,
        DARKGRAY,
    );
    for (slot, name) in known_moves.iter().enumerate() {
        if let Some(name) = name {
            let y = 282.0 + slot as f32 * 34.0;
            draw_rectangle(145.0, y - 22.0, 650.0, 29.0, BG);
            draw_text(&format!("{}  {name}", slot + 1), 160.0, y, 19.0, INK);
        }
    }
    draw_text(
        &format!("WAVE {wave} · F5 SAVE LOCALLY"),
        145.0,
        418.0,
        16.0,
        GREEN,
    );
    if !notice.is_empty() {
        draw_text(notice, 520.0, 418.0, 14.0, DARKGRAY);
    }
}

fn draw_prevolution_move_screen(
    wave: u8,
    species: &str,
    movement: &str,
    notice: &str,
    background: Option<&Texture2D>,
) {
    draw_rectangle(0.0, 0.0, 960.0, 540.0, BG);
    draw_battlefield_backdrop(background);
    draw_rectangle(105.0, 110.0, 750.0, 230.0, PANEL);
    draw_rectangle(105.0, 110.0, 750.0, 5.0, GREEN);
    draw_text("PRE-EVOLUTION MOVE", 145.0, 160.0, 27.0, GREEN);
    draw_text(
        &format!("{species} can learn {movement}"),
        145.0,
        205.0,
        24.0,
        INK,
    );
    draw_text(
        "through its pre-evolution. This is a reminder; it was not learned.",
        145.0,
        244.0,
        18.0,
        DARKGRAY,
    );
    draw_text(
        &format!("WAVE {wave} · ENTER ACKNOWLEDGE · F5 SAVE"),
        145.0,
        305.0,
        16.0,
        GREEN,
    );
    if !notice.is_empty() {
        draw_text(notice, 520.0, 418.0, 14.0, DARKGRAY);
    }
}

fn draw_intermission(wave: u8, wave_limit: u8, experience: u32, title: &str, controls: &str) {
    draw_rectangle(0.0, 330.0, 960.0, 210.0, BG);
    draw_rectangle(0.0, 330.0, 960.0, 4.0, GREEN);
    draw_text(title, 38.0, 382.0, 26.0, INK);
    draw_text(
        &format!("Wave {wave}/{wave_limit} · experience gained: {experience}"),
        38.0,
        421.0,
        20.0,
        INK,
    );
    draw_text(controls, 38.0, 477.0, 19.0, GREEN);
}

fn reward_card_for_option(option: &pokerogue_core::rewards::NaturalRewardOption) -> RewardCard {
    let mut card = RewardCard {
        name: option.name.clone(),
        icon: option.id.to_ascii_lowercase(),
        description: option.name.clone(),
        tier: option.tier,
    };
    match option.id {
        "SOOTHE_BELL" => {
            card.icon = "soothe_bell".into();
            card.name = "Soothe Bell".into();
            card.description = "Increases friendship gain by 50%.".into();
        }
        "PP_UP" => {
            card.icon = "pp_up".into();
            card.name = "PP Up".into();
            card.description = "Permanently increases PP for one Pokémon move by 1 for every 5 maximum PP (maximum 3).".into();
        }
        "PP_MAX" => {
            card.icon = "pp_max".into();
            card.name = "PP Max".into();
            card.description = "Permanently maximizes PP for one Pokémon move.".into();
        }
        "TM_COMMON" | "TM_GREAT" => {
            if let Some(movement) = option
                .generated_variant
                .as_deref()
                .and_then(pokerogue_data::move_by_id)
            {
                card.icon = format!("tm_{:?}", movement.element).to_lowercase();
                let number = if movement.id == "SUNNY_DAY" {
                    "091"
                } else {
                    ""
                };
                card.name = if number.is_empty() {
                    format!("TM - {}", movement.name)
                } else {
                    format!("TM{number} - {}", movement.name)
                };
                card.description = format!("Teach {} to a Pokémon.", movement.name);
            }
        }
        "GREAT_BALL" => {
            card.icon = "great_ball".into();
            card.name = "5 Great Balls".into();
            card.description = "Receive Great Ball ×5.".into();
        }
        "ULTRA_BALL" => {
            card.icon = "ultra_ball".into();
            card.name = "5 Ultra Balls".into();
            card.description = "Receive Ultra Ball ×5.".into();
        }
        "POKEBALL" => {
            card.icon = "pokeball".into();
            card.name = "5 Poké Balls".into();
            card.description = "Receive Poké Ball ×5.".into();
        }
        "EXP_CHARM" => {
            card.icon = "exp_charm".into();
            card.name = "Exp. Charm".into();
            card.description = "Increases gain of Exp. Points by 25%.".into();
        }
        "EXP_SHARE" => {
            card.icon = "exp_share".into();
            card.name = "Exp. Share".into();
            card.description = "Shares experience with reserve party members.".into();
        }
        "ELIXIR" | "MAX_ELIXIR" => {
            card.icon = if option.id == "ELIXIR" {
                "elixir"
            } else {
                "max_elixir"
            }
            .into();
            card.name = option.name.clone();
            card.description = if option.id == "ELIXIR" {
                "Restores 10 PP for all of one Pokémon’s moves.".into()
            } else {
                "Restores all PP for all of one Pokémon’s moves.".into()
            };
        }
        "FULL_RESTORE" => {
            card.icon = "full_restore".into();
            card.name = "Full Restore".into();
            card.description =
                "Fully restores HP for one Pokémon and heals any status ailment.".into();
        }
        "TEMP_STAT_STAGE_BOOSTER" => {
            let stat = match option.generated_variant.as_deref() {
                Some("ACC") => "Accuracy",
                Some("SPD") => "Speed",
                Some("ATK") => "Attack",
                Some("DEF") => "Defense",
                Some("SPATK") => "Sp. Attack",
                Some("SPDEF") => "Sp. Defense",
                _ => "stat",
            };
            card.icon = match option.generated_variant.as_deref() {
                Some("ACC") => "x_accuracy",
                Some("SPD") => "x_speed",
                Some("ATK") => "x_attack",
                Some("DEF") => "x_defense",
                Some("SPATK") => "x_sp_atk",
                Some("SPDEF") => "x_sp_def",
                _ => "x_attack",
            }
            .into();
            card.name = format!("X {stat}");
            card.description =
                format!("Increases the {stat} of all party members by 20% for up to 5 battles.");
        }
        _ => {}
    }
    card
}

fn draw_reward_screen(
    wave: u8,
    wave_limit: u8,
    experience: u32,
    money: u64,
    title: &str,
    choices: Option<[RewardCard; 3]>,
    tier: &str,
    selected: usize,
    elapsed: f32,
    sprites: &SpriteAtlas,
    target: (&str, i32, i32, usize, &str),
    notice: &str,
    controls: &str,
) {
    let overlay_alpha = (elapsed / 0.75).clamp(0.0, 1.0) * 204.0;
    draw_rectangle(
        0.0,
        0.0,
        960.0,
        396.0,
        Color::from_rgba(7, 7, 7, overlay_alpha as u8),
    );
    draw_battle_header_text(
        &sprites.hud,
        &format!("Town - {wave}"),
        27.0,
        WHITE,
        Color::from_rgba(99, 99, 99, 255),
        0.54,
        0.92,
    );
    draw_battle_header_text(
        &sprites.hud,
        &format_money(money),
        56.5,
        Color::from_rgba(232, 232, 168, 255),
        Color::from_rgba(160, 160, 96, 255),
        0.5,
        1.012,
    );
    let top = [
        ("potion", "Potion", "₽36"),
        ("ether", "Ether", "₽72"),
        ("revive", "Revive", "₽360"),
    ];
    for (index, (icon, name, cost)) in top.iter().enumerate() {
        let center_x = [283.0, 478.0, 672.0][index];
        if elapsed > 0.35 + index as f32 * 0.12 {
            let bob = (elapsed * 2.2 + index as f32 * 1.4).sin() * 1.5;
            if let Some(atlas) = &sprites.item_icons {
                atlas.draw_centered(icon, center_x, 38.0 + bob, 2.2);
            }
            draw_reward_caption(
                &sprites.hud,
                name,
                center_x,
                80.0,
                Color::from_rgba(248, 248, 248, 255),
            );
            draw_reward_caption(
                &sprites.hud,
                cost,
                center_x,
                91.0,
                Color::from_rgba(232, 232, 168, 255),
            );
        }
    }
    draw_reward_right_text(
        &sprites.hud,
        "Luck: D",
        954.0,
        84.0,
        Color::from_rgba(248, 248, 248, 255),
    );

    if let Some(choices) = choices.as_ref() {
        let centers = [283.0, 478.0, 672.0];
        for (index, card) in choices.iter().enumerate() {
            let reveal = ((elapsed - 0.45 - index as f32 * 0.2) / 0.65).clamp(0.0, 1.0);
            if reveal <= 0.0 {
                continue;
            }
            let elastic = (1.0 - reveal) * (-(9.0 * reveal)).exp() * (13.0 * reveal).cos();
            let scale = 1.55 * (0.72 + 0.28 * reveal + elastic * 0.3);
            let bob = (elapsed * 2.0 + index as f32 * 1.8).sin() * 1.2;
            let icon_y = 195.0 + (1.0 - reveal) * -36.0 + bob;
            if let Some(atlas) = &sprites.item_icons {
                atlas.draw_centered(&card.icon, centers[index], icon_y, 3.2 * (scale / 1.55));
            }
            let tier_color = match card.tier {
                1 => Color::from_rgba(73, 152, 248, 255),
                2 => Color::from_rgba(248, 208, 56, 255),
                3 => Color::from_rgba(219, 67, 67, 255),
                4 => Color::from_rgba(227, 49, 197, 255),
                _ => Color::from_rgba(248, 248, 248, 255),
            };
            draw_reward_caption(
                &sprites.hud,
                &card.name,
                centers[index],
                250.0 + (1.0 - reveal) * -16.0,
                tier_color,
            );
        }
        let cursor_y = 174.0 + (elapsed * 4.0).sin() * 1.5;
        if let Some(cursor) = &sprites.menu_cursor {
            let center_x = centers[selected.min(2)];
            draw_texture_ex(
                cursor,
                center_x - 78.0,
                cursor_y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(32.0, 52.0)),
                    ..Default::default()
                },
            );
        }
    }

    if let Some(window) = &sprites.message_window {
        draw_texture_ex(
            window,
            0.0,
            396.0,
            WHITE,
            DrawTextureParams {
                source: Some(Rect::new(0.0, 0.0, 320.0, 48.0)),
                dest_size: Some(vec2(960.0, 144.0)),
                ..Default::default()
            },
        );
    } else {
        draw_rectangle(0.0, 396.0, 960.0, 144.0, PANEL);
    }
    let _ = (title, wave, wave_limit, experience);
    let (_name, hp, max_hp, slot, move_info) = target;
    let selected_card = choices
        .as_ref()
        .and_then(|cards| cards.get(selected.min(2)));
    if let Some(card) = selected_card {
        draw_reward_namebox(&sprites.reward_namebox, 35.0, 378.0, 181.0, 44.0);
        if let Some(atlas) = &sprites.item_icons {
            atlas.draw_centered(&card.icon, 78.0, 398.0 + (elapsed * 3.5).sin() * 1.3, 2.0);
        }
        draw_reward_left_caption(
            &sprites.hud,
            &card.name,
            120.0,
            405.0,
            Color::from_rgba(248, 248, 248, 255),
        );
        let description = if card.icon == "pp_up" {
            vec![
                "Permanently increases PP for one Pokémon move by 1 for".to_owned(),
                "every 5 maximum PP (maximum 3).".to_owned(),
            ]
        } else {
            wrap_reward_description(&sprites.hud, &card.description, 900.0)
        };
        for (line, text) in description.iter().enumerate().take(2) {
            draw_reward_description(&sprites.hud, text, 30.0, 458.0 + line as f32 * 50.0);
        }
    } else {
        draw_reward_description(&sprites.hud, "Choose an item to continue.", 30.0, 458.0);
    }
    draw_reward_right_text(
        &sprites.hud,
        &format!("{slot} Check Team"),
        948.0,
        372.0,
        Color::from_rgba(248, 248, 248, 255),
    );
    draw_reward_left_caption(
        &sprites.hud,
        "Reroll",
        30.0,
        367.0,
        Color::from_rgba(248, 248, 248, 255),
    );
    draw_reward_left_caption(
        &sprites.hud,
        "₽250",
        130.0,
        367.0,
        Color::from_rgba(232, 232, 168, 255),
    );
    if !notice.is_empty() {
        draw_reward_caption(
            &sprites.hud,
            notice,
            480.0,
            372.0,
            Color::from_rgba(248, 248, 248, 255),
        );
    }
    let _ = (tier, controls, hp, max_hp, move_info, money);
}

fn draw_reward_namebox(atlas: &Option<hud::Atlas>, x: f32, y: f32, width: f32, height: f32) {
    let Some(atlas) = atlas else {
        return;
    };
    let Some(frame) = atlas.frame("1") else {
        return;
    };
    let sx = [0.0, 8.0, 12.0];
    let sw = [8.0, 4.0, 8.0];
    let dx = [0.0, 8.0, width - 8.0];
    let dw = [8.0, width - 16.0, 8.0];
    let sy = [0.0, 5.0, 15.0];
    let sh = [5.0, 10.0, 5.0];
    let dy = [0.0, 5.0, height - 5.0];
    let dh = [5.0, height - 10.0, 5.0];
    for row in 0..3 {
        for column in 0..3 {
            draw_texture_ex(
                &atlas.texture,
                x + dx[column],
                y + dy[row],
                WHITE,
                DrawTextureParams {
                    source: Some(Rect::new(
                        frame.x + sx[column],
                        frame.y + sy[row],
                        sw[column],
                        sh[row],
                    )),
                    dest_size: Some(vec2(dw[column], dh[row])),
                    ..Default::default()
                },
            );
        }
    }
}

fn draw_reward_caption(hud: &hud::Hud, text: &str, center_x: f32, y: f32, color: Color) {
    let Some(font) = hud.font.as_ref() else {
        let width = measure_text(text, None, 18, 1.0).width;
        draw_text(text, center_x - width / 2.0, y, 18.0, color);
        return;
    };
    let params = TextParams {
        font: Some(font),
        font_size: 37,
        font_scale: 0.5,
        color,
        ..Default::default()
    };
    let width = measure_text(text, Some(font), 37, 0.5).width;
    draw_text_ex(text, center_x - width / 2.0, y, params);
}

fn draw_reward_right_text(hud: &hud::Hud, text: &str, right_x: f32, y: f32, color: Color) {
    let Some(font) = hud.font.as_ref() else {
        let width = measure_text(text, None, 18, 1.0).width;
        draw_text(text, right_x - width, y, 18.0, color);
        return;
    };
    let params = TextParams {
        font: Some(font),
        font_size: 72,
        font_scale: 0.5,
        color,
        ..Default::default()
    };
    let width = measure_text(text, Some(font), 72, 0.5).width;
    draw_text_ex(text, right_x - width, y, params);
}

fn draw_reward_left_caption(hud: &hud::Hud, text: &str, x: f32, y: f32, color: Color) {
    if let Some(font) = &hud.font {
        draw_text_ex(
            text,
            x,
            y,
            TextParams {
                font: Some(font),
                font_size: 60,
                font_scale: 0.5,
                color,
                ..Default::default()
            },
        );
    } else {
        draw_text(text, x, y, 18.0, color);
    }
}

fn draw_reward_description(hud: &hud::Hud, text: &str, x: f32, baseline: f32) {
    if let Some(font) = &hud.font {
        let params = TextParams {
            font: Some(font),
            font_size: 96,
            font_scale: 0.5,
            color: Color::from_rgba(248, 248, 248, 255),
            ..Default::default()
        };
        draw_text_ex(
            text,
            x + 1.0,
            baseline + 1.0,
            TextParams {
                color: Color::from_rgba(107, 90, 115, 255),
                ..params
            },
        );
        draw_text_ex(text, x, baseline, params);
    } else {
        draw_text(text, x, baseline, 36.0, WHITE);
    }
}

fn wrap_reward_description<'a>(hud: &hud::Hud, text: &'a str, max_width: f32) -> Vec<String> {
    let Some(font) = hud.font.as_ref() else {
        return vec![text.to_owned()];
    };
    let mut lines = vec![String::new()];
    for word in text.split_whitespace() {
        let line = lines.last().expect("description has line");
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if !line.is_empty() && measure_text(&candidate, Some(font), 96, 0.5).width > max_width {
            lines.push(word.to_owned());
        } else {
            *lines.last_mut().expect("description has line") = candidate;
        }
    }
    lines
}

#[cfg(test)]
mod viewport_tests {
    use super::*;

    #[test]
    fn phaser_canvas_blend_matches_sampled_source_channels() {
        assert_eq!(
            [0, 16, 41, 82, 96, 127, 128, 140, 222, 255].map(phaser_canvas_channel),
            [1, 17, 42, 83, 97, 128, 128, 140, 222, 255]
        );
    }

    #[test]
    fn source_shadow_mask_is_rasterized_on_the_pixel_art_grid() {
        let cells = pixelated_shadow_cells(60.0, 12.0, 3.0);
        assert!(!cells.is_empty());
        assert!(cells.iter().all(|(x, y)| {
            (x / 3.0 - (x / 3.0).round()).abs() < 0.001
                && (y / 3.0 - (y / 3.0).round()).abs() < 0.001
                && *x >= -30.0
                && *x < 30.0
                && *y >= -6.0
                && *y < 6.0
        }));
        assert!(pixelated_shadow_cells(0.0, 12.0, 3.0).is_empty());
    }

    #[test]
    fn damage_flash_and_pop_follow_source_phase_timing() {
        let playback = BattleAnimationPlayback {
            damage: vec![ActiveDamageAnimation {
                target: BattleActor::Enemy,
                amount: 7,
                critical: false,
                effectiveness: pokerogue_core::DamageEffectiveness::Normal,
                started_at: 1.0,
            }],
            ..Default::default()
        };
        assert!(playback.damage_flash_visible(BattleActor::Enemy, 1.19));
        assert!(!playback.damage_flash_visible(BattleActor::Enemy, 1.2));
        assert!(playback.damage_flash_visible(BattleActor::Enemy, 1.3));
        assert!(!playback.damage_flash_visible(BattleActor::Enemy, 1.4));
        assert!(!playback.damage_flash_visible(BattleActor::Enemy, 1.69));
        assert!(playback.damage_flash_visible(BattleActor::Enemy, 1.7));
        assert_eq!(damage_number_pose(0.0).unwrap().alpha, 0.0);
        let peak = damage_number_pose(0.25).unwrap();
        assert_eq!(peak.alpha, 1.0);
        assert_eq!(peak.y_offset, -16.0);
        assert!(damage_number_pose(1.7).is_none());
    }

    #[test]
    fn sequential_damage_feedback_delays_the_enemy_attack_until_hit_reaction() {
        let mut battle = pokerogue_core::Battle::demo();
        let movement = pokerogue_data::Move::Tackle;
        battle.last_events = vec![
            BattleEvent::MoveExecuted {
                actor: BattleActor::Player,
                move_: movement,
            },
            BattleEvent::DamageApplied {
                target: BattleActor::Enemy,
                amount: 6,
                critical: false,
                effectiveness: pokerogue_core::DamageEffectiveness::Normal,
            },
            BattleEvent::MoveExecuted {
                actor: BattleActor::Enemy,
                move_: movement,
            },
            BattleEvent::DamageApplied {
                target: BattleActor::Player,
                amount: 3,
                critical: false,
                effectiveness: pokerogue_core::DamageEffectiveness::Normal,
            },
        ];
        let frames = generated_battle_anims::for_move(movement.numeric_id())
            .unwrap()
            .frames
            .len() as f64
            * BATTLE_ANIMATION_FRAME_SECONDS;
        let mut playback = BattleAnimationPlayback::default();
        playback.enqueue_turn(&battle, 4.0, None);
        assert_eq!(playback.queue.len(), 2);
        assert_eq!(playback.damage.len(), 2);
        assert!((playback.damage[0].started_at - (4.0 + frames)).abs() < 1e-9);
        let first_hp_tween = playback.hp_bars[0];
        let hp_duration = hp_bar_animation_duration(first_hp_tween.from_hp, first_hp_tween.to_hp);
        assert!(
            (playback.queue[1].starts_at
                - (playback.damage[0].started_at
                    + BattleAnimationPlayback::HIT_FLASH_SECONDS
                    + hp_duration))
                .abs()
                < 1e-9
        );
        assert!(playback.damage[1].started_at > playback.damage[0].started_at);
    }

    #[test]
    fn hp_bar_tween_starts_from_pre_hit_core_hp_and_uses_source_sine_out() {
        let mut battle = pokerogue_core::Battle::demo();
        let enemy_hp_before = battle.enemy.hp;
        battle
            .take_turn(PlayerAction::UseMove(1))
            .expect("the demo's Tackle has PP");
        let enemy_hp_after = battle.enemy.hp;
        assert!(enemy_hp_after < enemy_hp_before);

        let mut playback = BattleAnimationPlayback::default();
        playback.enqueue_turn(&battle, 4.0, None);
        let animation = playback
            .hp_bars
            .iter()
            .find(|animation| animation.actor == BattleActor::Enemy)
            .copied()
            .unwrap();
        let damage_start = playback
            .damage
            .iter()
            .find(|damage| damage.target == BattleActor::Enemy)
            .unwrap()
            .started_at;
        let expected_start = damage_start + BattleAnimationPlayback::HIT_FLASH_SECONDS;
        assert!((animation.started_at - expected_start).abs() < 1e-9);
        assert_eq!(animation.from_hp, enemy_hp_before);
        assert_eq!(animation.to_hp, enemy_hp_after);
        assert_eq!(
            hp_bar_animation_duration(enemy_hp_before, enemy_hp_after),
            0.25
        );

        let (start_ratio, start_hp) = playback.hp_bar_sample(
            BattleActor::Enemy,
            0,
            enemy_hp_after,
            battle.enemy.max_hp,
            expected_start,
        );
        assert!((start_ratio - enemy_hp_before as f32 / battle.enemy.max_hp as f32).abs() < 1e-6);
        assert_eq!(start_hp, enemy_hp_before);
        let (mid_ratio, mid_hp) = playback.hp_bar_sample(
            BattleActor::Enemy,
            0,
            enemy_hp_after,
            battle.enemy.max_hp,
            expected_start + animation.duration / 2.0,
        );
        let eased_midpoint = std::f32::consts::FRAC_1_SQRT_2;
        let expected_mid_hp =
            enemy_hp_before as f32 + (enemy_hp_after - enemy_hp_before) as f32 * eased_midpoint;
        assert!((mid_ratio - expected_mid_hp / battle.enemy.max_hp as f32).abs() < 1e-5);
        assert_eq!(mid_hp, expected_mid_hp.ceil() as i32);
        let (end_ratio, end_hp) = playback.hp_bar_sample(
            BattleActor::Enemy,
            0,
            enemy_hp_after,
            battle.enemy.max_hp,
            expected_start + animation.duration,
        );
        assert!((end_ratio - enemy_hp_after as f32 / battle.enemy.max_hp as f32).abs() < 1e-6);
        assert_eq!(end_hp, enemy_hp_after);
    }

    #[test]
    fn player_hp_bar_stays_at_pre_hit_value_until_the_enemy_hit_animation_finishes() {
        let mut battle = pokerogue_core::Battle::demo();
        let hp_before = BattleHpSnapshot::capture(&battle);
        let player_slot = battle.player.active_slot();
        let player_hp_before = battle.player.hp;
        battle
            .take_turn(PlayerAction::UseMove(1))
            .expect("the demo's Tackle has PP");
        let player_hp_after = battle.player.hp;
        assert!(player_hp_after < player_hp_before);

        let mut playback = BattleAnimationPlayback::default();
        playback.enqueue_turn_with_hp_snapshot(&battle, 9.0, None, Some(&hp_before));
        let tween = playback
            .hp_bars
            .iter()
            .find(|animation| {
                animation.actor == BattleActor::Player && animation.party_slot == player_slot
            })
            .copied()
            .unwrap();
        assert_eq!(tween.from_hp, player_hp_before);
        assert_eq!(tween.to_hp, player_hp_after);

        let (before_ratio, before_hp) = playback.hp_bar_sample(
            BattleActor::Player,
            player_slot,
            player_hp_after,
            battle.player.max_hp,
            tween.started_at - 0.01,
        );
        assert!(
            (before_ratio - player_hp_before as f32 / battle.player.max_hp as f32).abs() < 1e-6
        );
        assert_eq!(before_hp, player_hp_before);
        assert!(playback.is_busy_at(tween.started_at));
    }

    #[test]
    fn victory_exp_meter_smoothly_crosses_growth_thresholds() {
        let growth = Species::Bulbasaur.growth_rate();
        let from = pokerogue_data::total_exp_for_level(5, growth);
        let to = pokerogue_data::total_exp_for_level(6, growth) + 10;
        let playback = BattleAnimationPlayback {
            experience: Some(ActiveExpAnimation {
                party_slot: 0,
                from_exp: from,
                to_exp: to,
                from_level: 5,
                to_level: 6,
                started_at: 2.0,
                duration: 2.0,
            }),
            ..Default::default()
        };
        assert_eq!(playback.experience_sample(0, 6, to, growth, 2.0), (5, from));
        let (mid_level, mid_exp) = playback.experience_sample(0, 6, to, growth, 3.0);
        assert!(mid_exp > from && mid_exp < to);
        assert!(mid_level >= 5 && mid_level <= 6);
        assert_eq!(playback.experience_sample(0, 6, to, growth, 4.0), (6, to));
    }

    #[test]
    fn party_cursor_follows_original_vertical_slots_and_cancel_button() {
        assert_eq!(move_party_cursor(0, 1, false, false, false, true), 1);
        assert_eq!(move_party_cursor(1, 1, false, false, false, true), 6);
        assert_eq!(move_party_cursor(0, 1, false, false, true, false), 6);
        assert_eq!(move_party_cursor(6, 1, false, false, true, false), 1);
        assert_eq!(move_party_cursor(0, 1, false, true, false, false), 1);
        assert_eq!(move_party_cursor(1, 1, true, false, false, false), 0);
    }

    #[test]
    fn forced_replacement_accepts_only_a_living_reserve_from_core() {
        let mut battle = pokerogue_core::Battle::demo();
        assert!(can_select_replacement(&battle, 1));
        assert!(!can_select_replacement(
            &battle,
            battle.player.active_slot()
        ));

        battle.player.member_mut(1).unwrap().hp = 0;
        assert!(!can_select_replacement(&battle, 1));
        assert!(!can_select_replacement(&battle, 99));
    }

    #[test]
    fn keeps_phaser_sixteen_nine_canvas_centered_without_stretching() {
        assert_eq!(fitted_viewport(960.0, 540.0), (0, 0, 960, 540));
        assert_eq!(fitted_viewport(640.0, 480.0), (0, 60, 640, 360));
        assert_eq!(fitted_viewport(1280.0, 720.0), (0, 0, 1280, 720));
        assert_eq!(fitted_viewport(800.0, 600.0), (0, 75, 800, 450));
    }

    #[test]
    fn logical_viewport_capture_crops_letterboxing_and_restores_logical_size() {
        let mut bytes = Vec::new();
        for y in 0..4u8 {
            for x in 0..4u8 {
                bytes.extend_from_slice(&[x, y, 0, 255]);
            }
        }
        let source = Image {
            bytes,
            width: 4,
            height: 4,
        };
        let output = image_at_logical_viewport(&source, (1, 1, 2, 2));
        assert_eq!((output.width, output.height), (960, 540));
        assert_eq!(&output.bytes[..4], &[1, 1, 0, 255]);
        assert_eq!(&output.bytes[output.bytes.len() - 4..], &[2, 2, 0, 255]);
    }

    #[test]
    fn camera_maps_top_left_to_top_and_bottom_right_to_bottom() {
        let camera = logical_camera();
        let matrix = camera.matrix();
        let top_left = matrix.transform_point3(vec3(0.0, 0.0, 0.0));
        let bottom_right = matrix.transform_point3(vec3(LOGICAL_WIDTH, LOGICAL_HEIGHT, 0.0));
        assert!(
            top_left.x < 0.0 && top_left.y > 0.0,
            "top-left: {top_left:?}"
        );
        assert!(
            bottom_right.x > 0.0 && bottom_right.y < 0.0,
            "bottom-right: {bottom_right:?}"
        );
    }

    #[test]
    fn town_arena_layers_use_settled_phaser_positions_and_field_scale() {
        assert_eq!(
            project_field_layer(0.0, 320.0, 132.0),
            Rect::new(0.0, 0.0, 960.0, 396.0)
        );
        assert_eq!(
            project_field_layer(20.0, 320.0, 132.0),
            Rect::new(60.0, 0.0, 960.0, 396.0)
        );
    }

    #[test]
    fn encounter_entry_matches_source_positions_at_start_midpoint_and_end() {
        assert_eq!(
            encounter_entrance(EncounterEntranceKind::First, 0.0),
            FieldPresentation {
                player_x: 300.0,
                enemy_x: -300.0,
                player_platform_x: 300.0,
                enemy_platform_x: -280.0,
            }
        );
        assert_eq!(
            encounter_entrance(EncounterEntranceKind::First, 0.5),
            FieldPresentation {
                player_x: 150.0,
                enemy_x: -150.0,
                player_platform_x: 150.0,
                enemy_platform_x: -130.0,
            }
        );
        let settled = final_field_presentation();
        assert_eq!(settled.player_x, 0.0);
        assert_eq!(settled.enemy_x, 0.0);
        assert_eq!(settled.enemy_platform_x, 20.0);
        let next = encounter_entrance(EncounterEntranceKind::NextWave, 0.0);
        assert_eq!(next.player_x, 0.0);
        assert_eq!(next.player_platform_x, 0.0);
        assert_eq!(next.enemy_x, -300.0);
        assert_eq!(next.enemy_platform_x, -280.0);
    }

    #[test]
    fn first_wave_entry_shows_the_core_encounter_message_until_entry_finishes() {
        assert!(show_first_entry_message(EncounterEntranceKind::First, 0.0));
        assert!(show_first_entry_message(EncounterEntranceKind::First, 1.0));
        assert!(!show_first_entry_message(
            EncounterEntranceKind::First,
            EncounterEntrance::DURATION_SECONDS
        ));
        assert!(!show_first_entry_message(
            EncounterEntranceKind::NextWave,
            1.0
        ));
    }

    #[test]
    fn player_faint_uses_the_source_half_second_sine_in_tween_then_hides() {
        let mut playback = BattleAnimationPlayback {
            faint: Some(ActiveFaintAnimation {
                actor: BattleActor::Player,
                slot: 0,
                starts_at: 1.0,
            }),
            ..Default::default()
        };
        let before = playback.faint_pose(BattleActor::Player, 0, 0.99).unwrap();
        assert_eq!(before.offset_y, 0.0);
        assert!(!before.hide_info);
        assert!(!before.hide_sprite);

        let midpoint = playback.faint_pose(BattleActor::Player, 0, 1.25).unwrap();
        assert!(midpoint.hide_info);
        let expected = 450.0 * (1.0 - std::f32::consts::FRAC_PI_4.cos());
        assert!((midpoint.offset_y - expected).abs() < 0.001);
        assert!(!midpoint.hide_sprite);

        let finished = playback.faint_pose(BattleActor::Player, 0, 1.5).unwrap();
        assert_eq!(finished.offset_y, 450.0);
        assert!(finished.hide_sprite);
        assert!(playback.faint_pose(BattleActor::Player, 1, 1.5).is_none());

        let mut enemy = BattleAnimationPlayback {
            faint: Some(ActiveFaintAnimation {
                actor: BattleActor::Enemy,
                slot: 0,
                starts_at: 1.0,
            }),
            ..Default::default()
        };
        assert!(enemy.faint_pose(BattleActor::Player, 0, 1.0).is_none());
        assert!(
            enemy
                .faint_pose(BattleActor::Enemy, 0, 1.0)
                .unwrap()
                .hide_info
        );
    }

    #[test]
    fn voluntary_switch_follows_source_recall_wait_ball_flight_and_summon_tween() {
        let switch = ActiveSwitchAnimation {
            outgoing_slot: 0,
            active_slot: 1,
            returns_outgoing: true,
            started_at: 1.0,
        };
        let recall = switch_pose(switch, 1.125).unwrap();
        assert!(recall.outgoing_visible);
        assert!((recall.outgoing_tint_alpha - 0.2928932).abs() < 0.0001);
        assert!((recall.outgoing_scale - 0.8535534).abs() < 0.0001);

        let wait = switch_pose(switch, 1.5).unwrap();
        assert!(!wait.outgoing_visible);
        assert!(!wait.ball_visible);
        assert!(!wait.incoming_visible);

        let ball = switch_pose(switch, 2.4).unwrap();
        assert!(ball.ball_visible);
        assert!((ball.ball_x - 75.3846).abs() < 0.01);
        assert!((ball.ball_y - 77.75).abs() < 0.01);
        assert!((ball.ball_angle - 720.0).abs() < 0.01);

        let released = switch_pose(switch, 2.65).unwrap();
        assert!(released.incoming_visible);
        assert_eq!(released.incoming_scale, 0.5);
        assert_eq!(released.incoming_tint_alpha, 1.0);
        let settled = switch_pose(switch, 2.9).unwrap();
        assert_eq!(settled.incoming_scale, 1.0);
        assert_eq!(settled.incoming_tint_alpha, 0.0);
        assert!(switch_pose(switch, 3.9).is_none());
    }

    #[test]
    fn mandatory_replacement_skips_recall_wait_but_keeps_summon_phase() {
        let replacement = ActiveSwitchAnimation {
            outgoing_slot: 0,
            active_slot: 1,
            returns_outgoing: false,
            started_at: 4.0,
        };
        let start = switch_pose(replacement, 4.0).unwrap();
        assert!(!start.outgoing_visible);
        assert!(start.ball_visible);
        assert_eq!(start.ball_x, 36.0);
        assert_eq!(start.ball_y, 80.0);
        assert!(switch_pose(replacement, 4.65).unwrap().incoming_visible);
        assert!(switch_pose(replacement, 5.9).is_none());
    }

    #[test]
    fn switch_message_changes_from_recall_to_sendout_at_the_source_phase_boundary() {
        let mut battle = pokerogue_core::Battle::demo();
        let events = battle.switch_player(1).unwrap();
        let completed = events
            .into_iter()
            .find_map(|event| match event {
                pokerogue_core::SwitchEvent::SwitchCompleted { outgoing, active } => {
                    Some((outgoing, active))
                }
                _ => None,
            })
            .unwrap();
        let switch = ActiveSwitchAnimation {
            outgoing_slot: completed.0,
            active_slot: completed.1,
            returns_outgoing: true,
            started_at: 6.0,
        };

        assert_eq!(
            switch_message(switch, &battle, 6.5).as_deref(),
            Some("Come back, Bulbasaur!")
        );
        assert_eq!(
            switch_message(switch, &battle, 7.0).as_deref(),
            Some("Go! Charmander!")
        );
    }

    #[test]
    fn faint_playback_uses_battle_events_and_does_not_treat_capture_as_a_faint() {
        let mut defeated = pokerogue_core::Battle::demo();
        defeated.last_events = vec![BattleEvent::BattleEnded(pokerogue_core::Outcome::PlayerWin)];
        let mut playback = BattleAnimationPlayback::default();
        playback.enqueue_turn(&defeated, 2.0, None);
        assert_eq!(playback.faint.unwrap().actor, BattleActor::Enemy);

        let mut captured = pokerogue_core::Battle::demo();
        captured.last_events = vec![
            BattleEvent::CaptureAttemptResolved {
                species_id: captured.enemy.species.numeric_id(),
                caught: true,
                party_slot: Some(2),
            },
            BattleEvent::BattleEnded(pokerogue_core::Outcome::PlayerWin),
        ];
        let mut playback = BattleAnimationPlayback::default();
        playback.enqueue_turn(&captured, 2.0, None);
        assert!(playback.faint.is_none());
    }

    #[test]
    fn sitrus_heal_popup_uses_only_the_core_consumption_event() {
        let mut battle = pokerogue_core::Battle::demo();
        battle.last_events = vec![BattleEvent::SitrusBerryConsumed {
            slot: battle.player.active_slot(),
            hp_restored: 9,
        }];
        let slot = battle.player.active_slot();
        let mut playback = BattleAnimationPlayback::default();
        playback.enqueue_turn(&battle, 3.0, None);

        assert_eq!(playback.heal_for(slot, 3.0).unwrap().amount, 9);
        assert!(playback.heal_for(slot + 1, 3.0).is_none());
        assert!(
            playback
                .heal_for(
                    slot,
                    3.0 + BattleAnimationPlayback::HEALTH_UP_NUMBER_DELAY_SECONDS
                        + BattleAnimationPlayback::DAMAGE_NUMBER_SECONDS
                )
                .is_none()
        );
    }

    #[test]
    fn health_up_animation_uses_original_three_frame_clock_before_heal_number() {
        let slot = 0;
        let mut playback = BattleAnimationPlayback {
            active_player_slot: slot,
            ..Default::default()
        };
        playback.healing.push(ActiveHealAnimation {
            party_slot: slot,
            amount: 9,
            started_at: 2.0,
        });

        assert!(playback.health_up_frame_index(slot, 2.049, 9).is_none());
        assert_eq!(playback.health_up_frame_index(slot, 2.05, 9), Some(0));
        assert_eq!(playback.health_up_frame_index(slot, 2.45, 9), Some(8));
        assert!(playback.health_up_frame_index(slot, 2.5, 9).is_none());
        let heal = playback
            .heal_for(
                slot,
                2.0 + BattleAnimationPlayback::HEALTH_UP_NUMBER_DELAY_SECONDS,
            )
            .unwrap();
        assert!(heal_number_pose(heal, 2.0 + 0.832).is_none());
        assert_eq!(
            heal_number_pose(
                heal,
                2.0 + BattleAnimationPlayback::HEALTH_UP_NUMBER_DELAY_SECONDS
            )
            .unwrap()
            .alpha,
            0.0
        );
    }

    #[test]
    fn cached_health_up_data_loads_the_original_graphic_frames() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../visual-harness/.cache/assets/fb8ccb6d0703ffe48cdf80a8102d5ed943c82dba/battle-anims/common-health-up.json",
        );
        if !path.exists() {
            return;
        }
        let bytes = std::fs::read(&path).unwrap();
        let frames = parse_health_up_graphic_frames(&bytes).unwrap();
        assert_eq!(frames.len(), 9);
        assert_eq!(frames[0].len(), 2);
        assert_eq!(frames[6][0].focus, 3); // AnimFocus.USER_TARGET
        assert_eq!(frames[8][0].frame, 6);
    }

    #[test]
    fn capture_animation_uses_source_ball_frames_and_hides_only_successful_catches() {
        let mut playback = BattleAnimationPlayback {
            capture: Some(ActiveCaptureAnimation {
                ball: CaptureBall::Poke,
                caught: true,
                critical: false,
                shake_roll_count: 3,
                star_offsets: [0.0; 3],
                started_at: 10.0,
            }),
            ..Default::default()
        };
        let thrown = playback.capture_pose(10.25).unwrap();
        assert_eq!(thrown.ball_frame, "pb");
        assert_eq!(thrown.ball_x, 16.0 * 3.0 + 220.0 * 0.5 * 3.0);
        assert!(!thrown.hide_enemy);
        assert!(!thrown.hide_info);

        let opening = playback.capture_pose(10.505).unwrap();
        assert_eq!(opening.ball_frame, "pb_opening");
        assert!(opening.hide_info);
        assert!(opening.enemy_scale < 1.0);

        let open = playback.capture_pose(10.75).unwrap();
        assert_eq!(open.ball_frame, "pb_open");
        assert!(!open.hide_enemy);

        let closing = playback.capture_pose(11.005).unwrap();
        assert_eq!(closing.ball_frame, "pb_opening");
        assert!(closing.hide_enemy);
        assert!(playback.is_busy_at(11.005));

        let caught = playback.capture_pose(11.02).unwrap();
        assert_eq!(caught.ball_frame, "pb");
        assert!(caught.hide_enemy);
        assert!(playback.is_busy_at(11.02));

        playback.capture = Some(ActiveCaptureAnimation {
            ball: CaptureBall::Great,
            caught: false,
            critical: false,
            shake_roll_count: 1,
            star_offsets: [0.0; 3],
            started_at: 10.0,
        });
        let failed_mid_close = playback.capture_pose(13.3).unwrap();
        assert_eq!(failed_mid_close.ball_frame, "gb_opening");
        assert!(!failed_mid_close.hide_enemy);
        assert!(failed_mid_close.enemy_scale > 0.25);
        assert!(playback.capture_pose(13.6).is_none());
    }

    #[test]
    fn capture_animation_is_started_only_from_a_resolved_core_capture_event() {
        let mut battle = pokerogue_core::Battle::demo();
        battle.last_capture_attempt = Some(pokerogue_core::CaptureAttempt {
            modified_catch_rate: 45,
            shake_probability: 0.5,
            critical: false,
            shake_rolls: vec![100],
            caught: false,
        });
        battle.last_events = vec![BattleEvent::CaptureAttemptResolved {
            species_id: battle.enemy.species.numeric_id(),
            caught: false,
            party_slot: None,
        }];
        let mut playback = BattleAnimationPlayback::default();
        playback.enqueue_turn(&battle, 7.0, Some(CaptureBall::Ultra));
        let pose = playback.capture_pose(7.0).unwrap();
        assert_eq!(pose.ball_frame, "ub");
        assert!(playback.is_busy_at(7.1));

        let mut no_capture = BattleAnimationPlayback::default();
        no_capture.enqueue_turn(&battle, 7.0, None);
        assert!(no_capture.capture_pose(7.0).is_none());
    }

    #[test]
    fn capture_bounce_and_shake_timing_uses_source_schedule_and_core_result_shape() {
        assert!((capture_bounce_duration() - 1.0390625).abs() < 0.000_001);
        assert_eq!(capture_bounce_y(0.0), 16.0);
        assert_eq!(capture_bounce_y(0.35), 72.0);
        assert!(capture_bounce_y(0.35 + 0.175) < 72.0);

        let normal_shake_start = capture_shake_start(false);
        assert!((normal_shake_start - 2.2890625).abs() < 0.000_001);
        assert!(
            (attempt_animation_duration(false, false, 1) - (normal_shake_start + 1.25)).abs()
                < 0.000_001
        );
        assert!(
            (attempt_animation_duration(false, false, 3) - (normal_shake_start + 3.25)).abs()
                < 0.000_001
        );
        assert!(
            (attempt_animation_duration(false, false, 4) - (normal_shake_start + 4.25)).abs()
                < 0.000_001
        );
        assert!(
            (attempt_animation_duration(false, true, 3) - (normal_shake_start + 5.0)).abs()
                < 0.000_001
        );

        let critical_shake_start = capture_shake_start(true);
        assert!((critical_shake_start - 4.3490625).abs() < 0.000_001);
        assert!(
            (attempt_animation_duration(true, false, 1) - (critical_shake_start + 2.25)).abs()
                < 0.000_001
        );
        assert!(
            (attempt_animation_duration(true, true, 1) - (critical_shake_start + 3.0)).abs()
                < 0.000_001
        );
    }

    #[test]
    fn capture_open_particles_follow_the_source_atlas_emitter_and_lifetime() {
        assert_eq!(generated_capture_particles::PARTICLE_FRAMES.len(), 11);
        assert_eq!(generated_capture_particles::frame(0).unwrap().width, 8);
        assert_eq!(generated_capture_particles::frame(1).unwrap().offset_x, 2);
        assert_eq!(generated_capture_particles::frame(4).unwrap().x, 28);

        let poke = ActiveCaptureAnimation {
            ball: CaptureBall::Poke,
            caught: false,
            critical: false,
            shake_roll_count: 1,
            star_offsets: [0.0; 3],
            started_at: 0.0,
        };
        let first = capture_open_particle_pose(poke, 0, 0.5).unwrap();
        assert_eq!(
            (first.x, first.y, first.frame, first.alpha),
            (236.0, 16.0, 3, 1.0)
        );
        let second = capture_open_particle_pose(poke, 1, 0.52).unwrap();
        assert!((second.x - 236.0).abs() < 0.001);
        assert!((second.y - 16.0).abs() < 0.001);
        assert_eq!(second.frame, 0);
        assert!(capture_open_particle_pose(poke, 0, 1.075).is_none());

        let great = ActiveCaptureAnimation {
            ball: CaptureBall::Great,
            ..poke
        };
        let great_pose = capture_open_particle_pose(great, 0, 0.7875).unwrap();
        assert!((great_pose.x - 284.0).abs() < 0.001);
        assert!((great_pose.y - 16.0).abs() < 0.001);
        let poke_midflight = capture_open_particle_pose(poke, 0, 0.7875).unwrap();
        assert!((poke_midflight.x - 260.0).abs() < 0.001);
        assert!((poke_midflight.alpha - 1.0).abs() < f32::EPSILON);
        assert!(
            capture_open_particle_pose(
                ActiveCaptureAnimation {
                    ball: CaptureBall::Ultra,
                    ..poke
                },
                0,
                0.5
            )
            .is_none()
        );
    }

    #[test]
    fn capture_success_stars_and_black_tint_use_a_separate_deterministic_cosmetic_stream() {
        let attempt = pokerogue_core::CaptureAttempt {
            modified_catch_rate: 80,
            shake_probability: 0.75,
            critical: false,
            shake_rolls: vec![120, 240, 360],
            caught: true,
        };
        let offsets = capture_star_offsets(4, 12, &attempt);
        assert_eq!(offsets, capture_star_offsets(4, 12, &attempt));
        assert!(offsets.iter().all(|offset| offset.is_finite()));
        assert_ne!(offsets, capture_star_offsets(1, 12, &attempt));

        let capture = ActiveCaptureAnimation {
            ball: CaptureBall::Poke,
            caught: true,
            critical: false,
            shake_roll_count: 3,
            star_offsets: offsets,
            started_at: 0.0,
        };
        let lock = capture_shake_start(false) + 4.0;
        assert!(capture_success_star_pose(capture, 0, lock - 0.001).is_none());
        let first = capture_success_star_pose(capture, 0, lock).unwrap();
        assert_eq!((first.x, first.y, first.alpha), (236.0, 72.0, 0.5));
        let apex = capture_success_star_pose(capture, 1, lock + 0.25).unwrap();
        assert!((apex.y - 62.0).abs() < 0.001);
        let faded = capture_success_star_pose(capture, 2, lock + 0.45).unwrap();
        assert!(faded.alpha > 0.0 && faded.alpha < 0.5);
        assert!(capture_success_star_pose(capture, 0, lock + 0.5).is_none());

        assert_eq!(capture_ball_tint_pose(capture, lock).unwrap().alpha, 0.0);
        assert!((capture_ball_tint_pose(capture, lock + 0.2).unwrap().alpha - 0.375).abs() < 0.001);
        assert!(capture_ball_tint_pose(capture, lock + 0.4).is_none());
        assert!(generated_capture_particles::frame(4).is_some());
    }

    #[test]
    fn extracted_tackle_and_ember_frames_match_the_local_source_data() {
        let tackle = generated_battle_anims::for_move(33).unwrap();
        assert_eq!(tackle.graphic, "PRAS- Strike");
        assert_eq!(tackle.frames.len(), 6);
        let strike = tackle.frames[0].unwrap();
        assert_eq!(strike.frame, 0);
        assert_eq!((strike.x, strike.y, strike.opacity), (121.5, -62.5, 100));
        assert!(tackle.frames[3].is_none());

        let ember = generated_battle_anims::for_move(52).unwrap();
        assert_eq!(ember.graphic, "PRAS- Fire");
        assert_eq!(ember.frames.len(), 14);
        assert_eq!(ember.frames[0].unwrap().frame, 36);
        assert_eq!(ember.frames[0].unwrap().zoom_x, 66.0);
        assert_eq!(ember.frames[5].unwrap().frame, 36);
        assert_eq!(ember.frames[6].unwrap().frame, 37);
        assert!(generated_battle_anims::for_move(999).is_none());
    }

    #[test]
    fn move_effect_center_uses_the_source_user_target_transform_both_directions() {
        let frame = generated_battle_anims::for_move(33).unwrap().frames[0].unwrap();
        let player_effect = battle_animation_center(frame, BattleActor::Player, 21.0, 21.0);
        let enemy_effect = battle_animation_center(frame, BattleActor::Enemy, 21.0, 21.0);
        assert!((player_effect.x - 688.2).abs() < 0.2);
        assert!((player_effect.y - 193.5).abs() < 0.2);
        assert!((enemy_effect.x - 337.8).abs() < 0.2);
        assert!((enemy_effect.y - 376.5).abs() < 0.2);
    }
}
