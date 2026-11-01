use macroquad::prelude::*;
use pokerogue_core::{BattleActor, BattleEvent, MiniRun, MiniRunStage, PlayerAction};
use pokerogue_data::Species;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

mod generated_battle_anims;
mod generated_capture_particles;
mod generated_pokeballs;
mod generated_sprites;
mod hud;
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

fn window_conf() -> Conf {
    let visual_battle = std::env::var_os("POKEROGUE_VISUAL_BATTLE").is_some();
    let visual_fixture = std::env::args().any(|s| s == "--visual-fixture");
    #[cfg(target_os = "linux")]
    let linux_backend = if visual_fixture
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
        fullscreen: visual_battle && !std::env::args().any(|s| s == "--visual-fixture"),
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
    pokeball_atlas: Option<Texture2D>,
    pokeball_particles: Option<Texture2D>,
    pokemon_root: PathBuf,
    active_player: Option<AnimatedSprite>,
    active_enemy: Option<AnimatedSprite>,
}

struct AnimatedSprite {
    species_id: u16,
    texture: Texture2D,
    frames: &'static [generated_sprites::SpriteFrame],
    started_at: f64,
}

#[derive(Clone, Copy)]
struct BattleAnimationRequest {
    move_id: u16,
    actor: BattleActor,
}

struct ActiveBattleAnimation {
    request: BattleAnimationRequest,
    started_at: f64,
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

impl BattleAnimationPlayback {
    const CAPTURE_THROW_SECONDS: f64 = 0.5;
    const CAPTURE_ABSORB_SECONDS: f64 = 0.5;
    const CAPTURE_FRAME_SECONDS: f64 = 0.017;

    fn enqueue_turn(
        &mut self,
        battle: &pokerogue_core::Battle,
        now: f64,
        ball: Option<CaptureBall>,
    ) {
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
                    });
                    visual_delay += animation.frames.len() as f64 * BATTLE_ANIMATION_FRAME_SECONDS;
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
            || self.faint.is_some_and(|faint| now < faint.starts_at + 0.5)
            || self.capture.is_some_and(|capture| {
                now < capture.started_at + capture_animation_duration(capture)
            })
    }

    fn clear_faint(&mut self) {
        self.faint = None;
        self.capture = None;
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
                self.active = self.queue.pop_front().map(|request| ActiveBattleAnimation {
                    request,
                    started_at: visual::time(),
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
}

impl AnimatedSprite {
    fn draw(&self, center_x: f32, baseline_y: f32, offset_x: f32, offset_y: f32, scale: f32) {
        if self.frames.is_empty() {
            return;
        }
        const PIXEL_SCALE: f32 = 3.0;
        let frame_index = if std::env::var_os("POKEROGUE_SCREENSHOT_PATH").is_some() {
            0
        } else {
            (visual::sprite_elapsed(self.started_at) * 10.0) as usize % self.frames.len()
        };
        let frame = self.frames[frame_index];
        let [source_x, source_y, source_w, source_h] = frame.rect;
        let [canvas_w, canvas_h] = frame.source;
        let [trim_x, trim_y] = frame.offset;
        let [trim_w, trim_h] = frame.size;
        draw_texture_ex(
            &self.texture,
            (center_x + offset_x - f32::from(canvas_w) * PIXEL_SCALE * scale / 2.0
                + f32::from(trim_x) * PIXEL_SCALE * scale)
                .round(),
            (baseline_y + offset_y - f32::from(canvas_h) * PIXEL_SCALE * scale
                + f32::from(trim_y) * PIXEL_SCALE * scale)
                .round(),
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(
                    f32::from(trim_w) * PIXEL_SCALE * scale,
                    f32::from(trim_h) * PIXEL_SCALE * scale,
                )),
                source: Some(Rect::new(
                    f32::from(source_x),
                    f32::from(source_y),
                    f32::from(source_w),
                    f32::from(source_h),
                )),
                ..Default::default()
            },
        );
    }
}

impl SpriteAtlas {
    async fn load() -> Self {
        // These are read from the original assets checkout and are not copied
        // into this project because their individual redistribution status is unclear.
        let pokemon_root = std::env::var_os("POKEROGUE_ASSETS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../original/assets/images/pokemon")
            });
        let images_root = pokemon_root
            .parent()
            .unwrap_or(Path::new("original/assets/images"));
        let load = |path: PathBuf| async move {
            load_texture(&path.to_string_lossy())
                .await
                .ok()
                .inspect(|texture| texture.set_filter(FilterMode::Nearest))
        };
        let hud = hud::Hud::load(images_root).await;
        let town_background = load(images_root.join("arenas/town_bg.png")).await;
        let town_player_platform = load(images_root.join("arenas/town_a.png")).await;
        let town_enemy_platform = load(images_root.join("arenas/town_b.png")).await;
        let message_window = load(images_root.join("ui/bg.png")).await;
        let window_border = load(images_root.join("ui/windows/window_1.png")).await;
        let player_info = load(images_root.join("ui/pbinfo_player.png")).await;
        let enemy_info = load(images_root.join("ui/pbinfo_enemy_mini.png")).await;
        let enemy_boss_info = load(images_root.join("ui/pbinfo_enemy_boss.png")).await;
        let hp_bar = load(images_root.join("ui/overlay_hp.png")).await;
        let boss_hp_bar = load(images_root.join("ui/overlay_hp_boss.png")).await;
        let battle_anim_strike = load(images_root.join("battle_anims/PRAS- Strike.png")).await;
        let battle_anim_fire = load(images_root.join("battle_anims/PRAS- Fire.png")).await;
        let pokeball_atlas = load(images_root.join("pb.png")).await;
        let pokeball_particles = load(images_root.join("effects/pb_particles.png")).await;
        Self {
            hud,
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
            pokeball_atlas,
            pokeball_particles,
            pokemon_root,
            active_player: None,
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
        let texture = load_texture(&path.to_string_lossy())
            .await
            .map_err(|error| {
                eprintln!("Unable to load local sprite {}: {error}", path.display());
                error
            })
            .ok()?;
        texture.set_filter(FilterMode::Nearest);
        Some(AnimatedSprite {
            species_id,
            texture,
            frames,
            started_at: visual::time(),
        })
    }

    async fn set_battlers(&mut self, player: Species, enemy: Species) {
        if self.active_player.as_ref().map(|sprite| sprite.species_id) != Some(player.numeric_id())
        {
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
    ) {
        // Constructors in Pokemon use player (106,148) and enemy (236,84)
        // field coordinates. The 960x540 client canvas is half Phaser's 1920x1080.
        let faint = player_faint.unwrap_or(FaintPose {
            offset_y: 0.0,
            hide_info: false,
            hide_sprite: false,
        });
        if !faint.hide_sprite {
            if let Some(sprite) = &self.active_player {
                sprite.draw(
                    106.0 * 3.0,
                    148.0 * 3.0,
                    field.player_x * 3.0,
                    faint.offset_y,
                    1.0,
                );
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
                sprite.draw(
                    236.0 * 3.0,
                    84.0 * 3.0
                        + enemy_faint.offset_y
                        + capture_pose.map_or(0.0, |pose| pose.enemy_offset_y),
                    field.enemy_x * 3.0,
                    0.0,
                    capture_scale,
                );
            } else {
                draw_fallback_sprite(
                    236.0 * 3.0 + field.enemy_x * 3.0,
                    84.0 * 3.0 - 70.0 + enemy_faint.offset_y,
                    enemy_species.name(),
                    ORANGE,
                );
            }
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
    title: &str,
    level: u8,
    hp: i32,
    max_hp: i32,
    boss_segments: u8,
    offset_x: f32,
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
    let anchor_x = (if player { 310.0 } else { 140.0 }) * 3.0 + offset_x;
    let anchor_y = (if player { 108.0 } else { 39.0 }) * 3.0;
    sprites.hud.name(title, name_x, name_y);
    sprites.hud.level(player, level, anchor_x, anchor_y);
    hud::Hud::image(
        sprites.hud.hp_label.as_ref(),
        bar_x - 3.0,
        bar_y - 9.0,
        vec2(1.0, 0.0),
    );

    let ratio = (hp.max(0) as f32 / max_hp.max(1) as f32).clamp(0.0, 1.0);
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
                bar_y.round(),
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

fn draw_party_strip(party: &pokerogue_core::Team) {
    let count = party.len().min(6);
    let x0 = 716.0;
    let y0 = 13.0;
    let width = 36.0 * count as f32 + 10.0;
    draw_rectangle(x0, y0, width, 31.0, Color::from_rgba(248, 248, 231, 232));
    draw_rectangle_lines(
        x0,
        y0,
        width,
        31.0,
        1.0,
        Color::from_rgba(173, 189, 162, 255),
    );
    for slot in 0..count {
        let Some(member) = party.member(slot) else {
            continue;
        };
        let x = x0 + 19.0 + slot as f32 * 36.0;
        let fainted = member.hp <= 0;
        let color = if fainted {
            Color::from_rgba(169, 112, 102, 255)
        } else {
            GREEN
        };
        draw_circle(x, y0 + 15.0, 6.0, color);
        draw_circle_lines(
            x,
            y0 + 15.0,
            7.0,
            if slot == party.active_slot() {
                2.0
            } else {
                1.0
            },
            INK,
        );
        let ratio = (member.hp.max(0) as f32 / member.max_hp.max(1) as f32).clamp(0.0, 1.0);
        draw_rectangle(
            x - 8.0,
            y0 + 25.0,
            16.0,
            3.0,
            Color::from_rgba(63, 72, 61, 255),
        );
        draw_rectangle(
            x - 8.0,
            y0 + 25.0,
            16.0 * ratio,
            3.0,
            if ratio > 0.5 { GREEN } else { ORANGE },
        );
    }
}

fn move_type_color(element: pokerogue_data::Element) -> Color {
    use pokerogue_data::Element;
    match element {
        Element::Fire => Color::from_rgba(226, 111, 73, 255),
        Element::Water => Color::from_rgba(83, 149, 207, 255),
        Element::Grass | Element::Bug => Color::from_rgba(105, 166, 105, 255),
        Element::Electric => Color::from_rgba(221, 183, 72, 255),
        Element::Psychic | Element::Fairy => Color::from_rgba(184, 122, 177, 255),
        Element::Rock | Element::Ground => Color::from_rgba(162, 133, 92, 255),
        Element::Poison | Element::Ghost | Element::Dark => Color::from_rgba(128, 111, 158, 255),
        _ => Color::from_rgba(146, 150, 140, 255),
    }
}

fn draw_move_card(
    x: f32,
    y: f32,
    width: f32,
    movement: pokerogue_data::Move,
    remaining_pp: u8,
    slot: usize,
    selected: bool,
) {
    let tint = move_type_color(movement.element());
    let background = if selected {
        Color::from_rgba(232, 241, 219, 255)
    } else {
        Color::from_rgba(250, 250, 240, 255)
    };
    draw_rectangle(x, y, width, 28.0, background);
    draw_rectangle(x, y, 4.0, 28.0, tint);
    draw_rectangle_lines(
        x,
        y,
        width,
        28.0,
        if selected { 2.0 } else { 1.0 },
        if selected {
            GREEN
        } else {
            Color::from_rgba(201, 210, 193, 255)
        },
    );
    draw_text(&format!("{}", slot + 1), x + 12.0, y + 20.0, 18.0, GREEN);
    draw_text(movement.name(), x + 38.0, y + 19.0, 17.0, INK);

    let type_name = format!("{:?}", movement.element()).to_uppercase();
    let badge_width = 54.0;
    let badge_x = x + width - 155.0;
    draw_rectangle(badge_x, y + 5.0, badge_width, 18.0, tint);
    draw_text(&type_name, badge_x + 5.0, y + 18.0, 10.0, WHITE);
    let power = if movement.is_status() {
        "STATUS".to_owned()
    } else {
        format!("PWR {}", movement.power())
    };
    draw_text(&power, x + width - 94.0, y + 18.0, 11.0, DARKGRAY);
    let pp_color = if remaining_pp == 0 { DARKGRAY } else { INK };
    draw_text(
        &format!("PP {remaining_pp}/{}", movement.pp()),
        x + width - 55.0,
        y + 18.0,
        11.0,
        pp_color,
    );
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

fn draw_official_message_window(texture: Option<&Texture2D>) {
    if let Some(texture) = texture {
        draw_texture_ex(
            texture,
            0.0,
            396.0,
            WHITE,
            DrawTextureParams {
                source: Some(Rect::new(0.0, 0.0, 320.0, 48.0)),
                dest_size: Some(vec2(LOGICAL_WIDTH, 144.0)),
                ..Default::default()
            },
        );
    }
}

/// Draws the original Phaser 24x24, 8-pixel-border window as a nine-slice.
fn draw_official_window(texture: Option<&Texture2D>, x: f32, y: f32, width: f32, height: f32) {
    let Some(texture) = texture else {
        draw_rectangle(x, y, width, height, PANEL);
        draw_rectangle_lines(
            x,
            y,
            width,
            height,
            2.0,
            Color::from_rgba(184, 197, 177, 255),
        );
        return;
    };
    let source_border = 8.0;
    let destination_border = 24.0;
    let center_width = (width - destination_border * 2.0).max(0.0);
    let center_height = (height - destination_border * 2.0).max(0.0);
    for row in 0..3 {
        for column in 0..3 {
            let source_x = column as f32 * source_border;
            let source_y = row as f32 * source_border;
            let source_width = if column == 1 {
                source_border
            } else {
                source_border
            };
            let source_height = if row == 1 {
                source_border
            } else {
                source_border
            };
            let destination_x = x + if column == 0 {
                0.0
            } else if column == 1 {
                destination_border
            } else {
                width - destination_border
            };
            let destination_y = y + if row == 0 {
                0.0
            } else if row == 1 {
                destination_border
            } else {
                height - destination_border
            };
            let destination_width = if column == 1 {
                center_width
            } else {
                destination_border
            };
            let destination_height = if row == 1 {
                center_height
            } else {
                destination_border
            };
            draw_texture_ex(
                texture,
                destination_x,
                destination_y,
                WHITE,
                DrawTextureParams {
                    source: Some(Rect::new(source_x, source_y, source_width, source_height)),
                    dest_size: Some(vec2(destination_width, destination_height)),
                    ..Default::default()
                },
            );
        }
    }
}

fn draw_battle(
    battle: &mut pokerogue_core::Battle,
    sprites: &SpriteAtlas,
    animation_playback: &mut BattleAnimationPlayback,
    wave: u8,
    _wave_limit: u8,
    pokeballs: u8,
    greatballs: u8,
    ultraballs: u8,
    selected_move: usize,
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
    // Phaser's biome/wave label is right-aligned text, not a pair of header cards.
    let biome_wave = format!("TOWN - {wave}");
    let text_width = measure_text(&biome_wave, None, 17, 1.0).width;
    draw_text(
        &biome_wave,
        954.0 - text_width + 2.0,
        38.0,
        17.0,
        Color::from_rgba(99, 99, 99, 255),
    );
    draw_text(
        &biome_wave,
        954.0 - text_width,
        36.0,
        17.0,
        Color::from_rgba(248, 248, 248, 255),
    );

    sprites.draw(
        battle.player.species,
        battle.enemy.species,
        field,
        player_faint,
        enemy_faint,
        capture_pose,
    );
    if capture_pose.is_some_and(|pose| !pose.ball_behind_enemy) {
        sprites.draw_capture_ball(capture_pose);
    }
    sprites.draw_capture_open_particles(animation_playback, visual::time());
    sprites.draw_capture_success_fx(animation_playback, visual::time());
    sprites.draw_battle_animation(animation_playback);
    if !player_faint.is_some_and(|pose| pose.hide_info) {
        draw_hp_card(
            sprites,
            true,
            battle.player.species.name(),
            battle.player.level,
            battle.player.hp,
            battle.player.max_hp,
            0,
            field.player_x * 3.0,
        );
    }
    if !enemy_faint.is_some_and(|pose| pose.hide_info)
        && !capture_pose.is_some_and(|pose| pose.hide_info)
    {
        draw_hp_card(
            sprites,
            false,
            battle.enemy.species.name(),
            battle.enemy.level,
            battle.enemy.hp,
            battle.enemy.max_hp,
            battle.enemy.boss_segments,
            field.enemy_x * 3.0,
        );
    }
    draw_party_strip(&battle.player);

    // Phaser's battle message and command windows share the original 320x48 UI row.
    draw_official_message_window(sprites.message_window.as_ref());
    draw_official_window(sprites.window_border.as_ref(), 0.0, 396.0, 729.0, 144.0);
    // The move details panel begins at x=240 in the source's 320x180 canvas.
    draw_official_window(sprites.window_border.as_ref(), 720.0, 396.0, 240.0, 144.0);
    let message = if battle.needs_replacement() {
        format!("{}\nA replacement is required!", battle.log)
    } else {
        battle.log.clone()
    };
    for (line, text) in message
        .lines()
        .take(2)
        .enumerate()
        .filter(|_| !visual::moves_fixture())
    {
        let x = 35.0;
        let y = 426.0 + line as f32 * 23.0;
        draw_text(
            text,
            x + 2.0,
            y + 2.0,
            20.0,
            Color::from_rgba(107, 90, 115, 255),
        );
        draw_text(text, x, y, 20.0, Color::from_rgba(248, 248, 248, 255));
    }

    if battle.outcome.is_some() {
        draw_text("ENTER  Play again", 35.0, 500.0, 19.0, INK);
    } else if visual::moves_fixture() {
        for index in 0..4 {
            let x = 54.0 + (index % 2) as f32 * 342.0;
            let y = 423.9 + (index / 2) as f32 * 48.0;
            let name = battle
                .player
                .moves
                .get(index)
                .filter(|_| index < battle.player.move_count())
                .map_or("-", |movement| movement.name());
            sprites.hud.window_text(name, x, y);
        }
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
    } else {
        let has_usable_move = (0..battle.player.move_count())
            .any(|i| battle.player.move_pp_remaining(i).unwrap_or(0) > 0);
        if !has_usable_move {
            draw_move_card(
                24.0,
                485.0,
                320.0,
                pokerogue_data::Move::struggle(),
                1,
                0,
                true,
            );
        } else {
            for index in 0..battle.player.move_count() {
                draw_move_card(
                    24.0 + (index % 2) as f32 * 333.0,
                    465.0 + (index / 2) as f32 * 34.0,
                    320.0,
                    battle.player.moves[index],
                    battle.player.move_pp_remaining(index).unwrap_or(0),
                    index,
                    selected_move == index,
                );
            }
        }
        draw_text(&format!("[Tab] Switch  [S] Sand"), 632.0, 455.0, 15.0, INK);
        draw_text(
            &format!("[B] Poké ×{pokeballs}  [G] Great ×{greatballs}"),
            632.0,
            481.0,
            14.0,
            INK,
        );
        draw_text(
            &format!("[U] Ultra ×{ultraballs}  [Esc] Quit"),
            632.0,
            507.0,
            14.0,
            INK,
        );
    }
}

fn save_path() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    base.join("pokerogue-rust").join("mini-run.sav")
}

fn team_slot_key(slot: usize) -> Option<KeyCode> {
    match slot {
        0 => Some(KeyCode::Key1),
        1 => Some(KeyCode::Key2),
        2 => Some(KeyCode::Key3),
        3 => Some(KeyCode::Key4),
        4 => Some(KeyCode::Key5),
        5 => Some(KeyCode::Key6),
        _ => None,
    }
}

fn save_run(run: &MiniRun) -> Result<(), String> {
    let path = save_path();
    let parent = path
        .parent()
        .ok_or_else(|| "invalid local save path".to_owned())?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let checkpoint = run
        .checkpoint()
        .map_err(|error| format!("checkpoint unavailable: {error:?}"))?;
    let temporary = path.with_extension("sav.tmp");
    std::fs::write(&temporary, checkpoint).map_err(|error| error.to_string())?;
    std::fs::rename(temporary, path).map_err(|error| error.to_string())
}

fn load_run() -> Result<MiniRun, String> {
    let save = std::fs::read_to_string(save_path()).map_err(|error| error.to_string())?;
    MiniRun::from_checkpoint(&save).map_err(|error| format!("invalid save: {error:?}"))
}

#[macroquad::main(window_conf)]
async fn main() {
    let visual_started_at = visual::time();
    let screenshot_path = std::env::var_os("POKEROGUE_SCREENSHOT_PATH").map(PathBuf::from);
    if visual::run_if_requested().await {
        return;
    }
    let mut sprites = SpriteAtlas::load().await;
    sprites
        .set_battlers(Species::Bulbasaur, Species::Charmander)
        .await;
    let visual_battle = std::env::var_os("POKEROGUE_VISUAL_BATTLE").is_some();
    let mut run: Option<MiniRun> = visual_battle
        .then(|| MiniRun::new_with_wave_limit("MiniRunSeed0001", "DAY", 10).ok())
        .flatten();
    let mut animation_playback = BattleAnimationPlayback::default();
    let mut last_displayed_wave = None;
    let mut encounter_entrance_playback: Option<EncounterEntrance> = None;
    let mut selected_wave_limit = MiniRun::DEFAULT_WAVE_LIMIT;
    let mut notice = String::new();
    let mut reward_target = 0usize;
    let mut reward_move = 0usize;
    let mut selected_move = 0usize;
    let mut visual_capture_ready = false;
    let mut natural_offer_cache: Option<(
        String,
        u8,
        Option<pokerogue_core::rewards::NaturalRewardOffer>,
    )> = None;
    loop {
        set_logical_viewport();
        if let Some(current) = &mut run {
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
            draw_battle(
                &mut current.battle,
                &sprites,
                &mut animation_playback,
                current.wave,
                current.wave_limit,
                current.pokeballs,
                current.greatballs,
                current.ultraballs,
                selected_move,
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
                    let mut capture_ball = None;
                    if current.battle.needs_replacement() {
                        if let Some(slot) = (0..current.battle.player.len())
                            .find(|slot| team_slot_key(*slot).is_some_and(is_key_pressed))
                        {
                            let _ = current.replace_fainted_player(slot);
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
                    if current.battle.last_events != events_before_action {
                        animation_playback.enqueue_turn(
                            &current.battle,
                            visual::time(),
                            capture_ball,
                        );
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
                        "Choose 1-3 · TAB target · LEFT/RIGHT move · ENTER skip · F5 save"
                            .to_owned()
                    } else {
                        "1-3 choose item · TAB target · ENTER skip · F5 save".to_owned()
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
                            reward_option_label(&offer.options[index])
                        }))
                    } else {
                        Some([
                            "Poké Balls · +5".into(),
                            "Potion · restore 20 HP".into(),
                            "Super Potion · restore 50 HP".into(),
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
                    if let Some(index) = [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3]
                        .iter()
                        .position(|key| is_key_pressed(*key))
                    {
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
                    } else if is_key_pressed(KeyCode::Enter) {
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
            draw_battlefield_backdrop(sprites.town_background.as_ref());
            draw_rectangle(0.0, 0.0, 960.0, 57.0, Color::from_rgba(48, 78, 65, 255));
            draw_text("POKÉROGUE", 28.0, 38.0, 26.0, WHITE);
            draw_text(
                "OFFLINE RUST BUILD",
                221.0,
                37.0,
                15.0,
                Color::from_rgba(192, 217, 189, 255),
            );
            sprites.draw(
                Species::Bulbasaur,
                Species::Charmander,
                final_field_presentation(),
                None,
                None,
                None,
            );
            draw_rectangle(
                0.0,
                330.0,
                960.0,
                210.0,
                Color::from_rgba(241, 240, 220, 255),
            );
            draw_rectangle(0.0, 330.0, 960.0, 5.0, Color::from_rgba(93, 139, 102, 255));
            draw_rectangle(
                46.0,
                352.0,
                868.0,
                153.0,
                Color::from_rgba(255, 255, 246, 255),
            );
            draw_rectangle(46.0, 352.0, 8.0, 153.0, GREEN);
            draw_rectangle_lines(
                46.0,
                352.0,
                868.0,
                153.0,
                2.0,
                Color::from_rgba(183, 196, 174, 255),
            );
            draw_text("TOWN EXPEDITION", 78.0, 390.0, 29.0, INK);
            let short_run = selected_wave_limit == 10;
            for (index, (label, active)) in [("10 WAVES", short_run), ("20 WAVES", !short_run)]
                .into_iter()
                .enumerate()
            {
                let x = 610.0 + index as f32 * 142.0;
                draw_rectangle(
                    x,
                    366.0,
                    128.0,
                    34.0,
                    if active {
                        Color::from_rgba(215, 235, 205, 255)
                    } else {
                        BG
                    },
                );
                draw_rectangle_lines(
                    x,
                    366.0,
                    128.0,
                    34.0,
                    if active { 2.0 } else { 1.0 },
                    if active {
                        GREEN
                    } else {
                        Color::from_rgba(183, 196, 174, 255)
                    },
                );
                draw_text(label, x + 17.0, 389.0, 17.0, INK);
            }
            draw_text("ENTER  Begin run", 80.0, 431.0, 21.0, GREEN);
            draw_text("F9  Resume local save", 312.0, 431.0, 19.0, INK);
            draw_text("Z  Switch length", 610.0, 431.0, 19.0, INK);
            let seed = if selected_wave_limit == 20 {
                "MiniRunSeed0667"
            } else {
                "MiniRunSeed0001"
            };
            draw_text(
                &format!("DAY · fixed offline seed {seed}"),
                80.0,
                475.0,
                18.0,
                DARKGRAY,
            );
            if is_key_pressed(KeyCode::Z) {
                selected_wave_limit = if selected_wave_limit == 10 { 20 } else { 10 };
            }
            if is_key_pressed(KeyCode::Enter) {
                run = MiniRun::new_with_wave_limit(seed, "DAY", selected_wave_limit).ok();
                natural_offer_cache = None;
                last_displayed_wave = None;
                encounter_entrance_playback = None;
                animation_playback = BattleAnimationPlayback::default();
            } else if is_key_pressed(KeyCode::F9) {
                match load_run() {
                    Ok(loaded) => {
                        last_displayed_wave = Some(loaded.wave);
                        encounter_entrance_playback = None;
                        animation_playback = BattleAnimationPlayback::default();
                        run = Some(loaded);
                        natural_offer_cache = None;
                        notice.clear();
                    }
                    Err(error) => notice = error,
                }
            }
            if !notice.is_empty() {
                draw_text(&notice, 300.0, 412.0, 16.0, DARKGRAY);
            }
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

fn reward_option_label(option: &pokerogue_core::rewards::NaturalRewardOption) -> String {
    if option.id == "GREAT_BALL" {
        "5 Great Balls".into()
    } else if option.id == "ULTRA_BALL" {
        "5 Ultra Balls".into()
    } else if option.id == "NUGGET" {
        "Nugget · money".into()
    } else if option.id == "BIG_NUGGET" {
        "Big Nugget · money".into()
    } else if option.id == "PP_UP" {
        "PP Up · increase selected move's max PP".into()
    } else if option.id == "PP_MAX" {
        "PP Max · maximize selected move's max PP".into()
    } else if option.id == "TM_COMMON" || option.id == "TM_GREAT" {
        option
            .generated_variant
            .as_deref()
            .and_then(pokerogue_data::move_by_id)
            .map(|movement| format!("TM · teach {}", movement.name))
            .unwrap_or_else(|| option.name.clone())
    } else if option.id == "EXP_CHARM" {
        "EXP Charm · +25% EXP for future wins".into()
    } else if option.id == "EXP_SHARE" {
        "EXP Share · share 20% with reserve members".into()
    } else if option.id == "ELIXIR" {
        "Elixir · restore 10 PP per move".into()
    } else if option.id == "MAX_ELIXIR" {
        "Max Elixir · restore all PP".into()
    } else if option.id == "FULL_RESTORE" {
        "Full Restore · heal selected Pokémon to max HP".into()
    } else if option.id == "SOOTHE_BELL" {
        "Soothe Bell · +50% friendship gains for selected Pokémon".into()
    } else if option.id == "BERRY:SITRUS" {
        "Sitrus Berry".into()
    } else if option.id == "TEMP_STAT_STAGE_BOOSTER" {
        match option.generated_variant.as_deref() {
            Some("ACC") => "X Accuracy · 5 battles".into(),
            Some("SPD") => "X Speed · 5 battles".into(),
            Some("ATK") => "X Attack · 5 battles".into(),
            Some("DEF") => "X Defense · 5 battles".into(),
            Some("SPATK") => "X Sp. Attack · 5 battles".into(),
            Some("SPDEF") => "X Sp. Defense · 5 battles".into(),
            _ => option.name.clone(),
        }
    } else {
        option.name.clone()
    }
}

fn draw_reward_screen(
    wave: u8,
    wave_limit: u8,
    experience: u32,
    money: u64,
    title: &str,
    choices: Option<[String; 3]>,
    tier: &str,
    target: (&str, i32, i32, usize, &str),
    notice: &str,
    controls: &str,
) {
    draw_rectangle(0.0, 330.0, 960.0, 210.0, BG);
    draw_rectangle(0.0, 330.0, 960.0, 4.0, GREEN);
    draw_text(title, 38.0, 365.0, 25.0, INK);
    draw_text(
        &format!("Wave {wave}/{wave_limit} · experience gained: {experience}"),
        38.0,
        393.0,
        17.0,
        Color::from_rgba(88, 105, 91, 255),
    );
    let (name, hp, max_hp, slot, move_info) = target;
    draw_text(
        &format!("TARGET {slot}: {name}  HP {hp}/{max_hp}  {move_info}"),
        430.0,
        393.0,
        17.0,
        INK,
    );
    draw_text(&format!("MONEY {money}"), 790.0, 418.0, 14.0, INK);
    if let Some(choices) = choices {
        draw_text(tier, 38.0, 423.0, 15.0, GREEN);
        for (index, label) in choices.iter().enumerate() {
            let x = 35.0 + index as f32 * 300.0;
            draw_rectangle(x, 432.0, 284.0, 60.0, PANEL);
            draw_rectangle(x, 432.0, 284.0, 4.0, GREEN);
            draw_rectangle_lines(
                x,
                432.0,
                284.0,
                60.0,
                1.5,
                Color::from_rgba(174, 190, 169, 255),
            );
            draw_text(
                &format!("{}  {label}", index + 1),
                x + 12.0,
                468.0,
                17.0,
                INK,
            );
        }
    }
    if !notice.is_empty() {
        draw_text(notice, 570.0, 366.0, 15.0, DARKGRAY);
    }
    draw_text(controls, 38.0, 519.0, 16.0, GREEN);
}

#[cfg(test)]
mod viewport_tests {
    use super::*;

    #[test]
    fn keeps_phaser_sixteen_nine_canvas_centered_without_stretching() {
        assert_eq!(fitted_viewport(960.0, 540.0), (0, 0, 960, 540));
        assert_eq!(fitted_viewport(640.0, 480.0), (0, 60, 640, 360));
        assert_eq!(fitted_viewport(1280.0, 720.0), (0, 0, 1280, 720));
        assert_eq!(fitted_viewport(800.0, 600.0), (0, 75, 800, 450));
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
