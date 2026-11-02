//! Small best-effort audio layer. All assets are read from the local pinned
//! PokéRogue assets checkout and are never copied into the executable.

use macroquad::audio::{self, PlaySoundParams, Sound};
use pokerogue_core::{Battle, BattleEvent, Outcome};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cue {
    Navigate,
    Confirm,
    Cancel,
    Impact,
    ImpactStrong,
    ImpactWeak,
    Faint,
    AttackTackle,
    AttackVineWhip,
    CaptureThrow,
    CaptureCriticalThrow,
    CaptureRelease,
    CaptureAbsorb,
    CaptureShake,
    CaptureLock,
    HealthUp95,
    HealthUp110,
    HealthUp125,
    TownBgm,
}

impl Cue {
    fn relative_path(self) -> &'static str {
        match self {
            Self::Navigate => "ui/select.wav",
            Self::Confirm => "ui/menu_open.wav",
            Self::Cancel => "ui/error.wav",
            Self::Impact => "se/hit.wav",
            Self::ImpactStrong => "se/hit_strong.wav",
            Self::ImpactWeak => "se/hit_weak.wav",
            Self::Faint => "se/faint.wav",
            Self::AttackTackle => "battle_anims/PRSFX- Tackle.wav",
            Self::AttackVineWhip => "battle_anims/PRSFX- Vine Whip.wav",
            Self::CaptureThrow => "se/pb_throw.wav",
            Self::CaptureCriticalThrow => "se/crit_throw.wav",
            Self::CaptureRelease => "se/pb_rel.wav",
            Self::CaptureAbsorb => "se/pb_catch.wav",
            Self::CaptureShake => "se/pb_move.wav",
            Self::CaptureLock => "se/pb_lock.wav",
            Self::HealthUp95 | Self::HealthUp110 | Self::HealthUp125 => {
                "battle_anims/PRSFX- Health Up.wav"
            }
            // The pinned MP3 is converted once to a local Ogg cache because
            // Macroquad/quad-snd decodes WAV and Ogg on native targets.
            Self::TownBgm => "bgm/town.ogg",
        }
    }

    pub fn from_move_name(name: &str) -> Option<Self> {
        match name {
            "Tackle" => Some(Self::AttackTackle),
            "Vine Whip" => Some(Self::AttackVineWhip),
            _ => None,
        }
    }

    fn health_up_pitch(self) -> Option<f32> {
        match self {
            Self::HealthUp95 => Some(0.95),
            Self::HealthUp110 => Some(1.10),
            Self::HealthUp125 => Some(1.25),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ScheduledCue {
    at: f64,
    cue: Cue,
}

/// Resolve the local assets audio directory from the same checkout used by the
/// sprite loader. The visual harness cache is only a fallback for dev machines
/// with a sparse `original/assets` checkout.
pub fn audio_asset_directory(
    manifest_dir: &Path,
    configured_pokemon_dir: Option<&Path>,
) -> PathBuf {
    let assets_root = configured_pokemon_dir
        .and_then(|path| path.parent()?.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| manifest_dir.join("../original/assets"));
    let primary = assets_root.join("audio");
    if primary.is_dir() {
        return primary;
    }
    manifest_dir
        .join("../visual-harness/.cache/assets/fb8ccb6d0703ffe48cdf80a8102d5ed943c82dba/audio")
}

pub fn available_asset(directory: &Path, cue: Cue) -> Option<PathBuf> {
    let path = directory.join(cue.relative_path());
    path.is_file().then_some(path)
}

fn retune_pcm_wav_header(bytes: &[u8], pitch: f32) -> Option<Vec<u8>> {
    if !pitch.is_finite()
        || pitch <= 0.0
        || bytes.get(0..4)? != b"RIFF"
        || bytes.get(8..12)? != b"WAVE"
    {
        return None;
    }
    let mut tuned = bytes.to_vec();
    let mut offset = 12_usize;
    while offset.checked_add(8)? <= tuned.len() {
        let chunk_size = usize::try_from(u32::from_le_bytes(
            tuned.get(offset + 4..offset + 8)?.try_into().ok()?,
        ))
        .ok()?;
        let content = offset + 8;
        let end = content.checked_add(chunk_size)?;
        if end > tuned.len() {
            return None;
        }
        if tuned.get(offset..offset + 4)? == b"fmt " {
            if chunk_size < 16
                || u16::from_le_bytes(tuned.get(content..content + 2)?.try_into().ok()?) != 1
            {
                return None;
            }
            let sample_rate =
                u32::from_le_bytes(tuned.get(content + 4..content + 8)?.try_into().ok()?);
            let block_align =
                u16::from_le_bytes(tuned.get(content + 12..content + 14)?.try_into().ok()?);
            let tuned_rate = (sample_rate as f32 * pitch).round() as u32;
            let byte_rate = tuned_rate.checked_mul(u32::from(block_align))?;
            tuned[content + 4..content + 8].copy_from_slice(&tuned_rate.to_le_bytes());
            tuned[content + 8..content + 12].copy_from_slice(&byte_rate.to_le_bytes());
            return Some(tuned);
        }
        offset = end.checked_add(chunk_size & 1)?;
    }
    None
}

pub fn input_cue(navigation_changed: bool) -> Option<Cue> {
    navigation_changed.then_some(Cue::Navigate)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiAction {
    Confirm,
    Cancel,
}

pub fn cue_for_ui_action(action: UiAction) -> Cue {
    match action {
        UiAction::Confirm => Cue::Confirm,
        UiAction::Cancel => Cue::Cancel,
    }
}

pub fn sound_for_move(name: &str) -> Option<Cue> {
    Cue::from_move_name(name)
}

pub struct AudioManager {
    assets: HashMap<Cue, Sound>,
    pending: VecDeque<(Cue, PathBuf)>,
    scheduled: VecDeque<ScheduledCue>,
    bgm_playing: bool,
    enabled: bool,
}

impl AudioManager {
    pub fn new() -> Self {
        let configured = std::env::var_os("POKEROGUE_ASSETS_DIR").map(PathBuf::from);
        let directory =
            audio_asset_directory(Path::new(env!("CARGO_MANIFEST_DIR")), configured.as_deref());
        let cues = [
            Cue::Navigate,
            Cue::Confirm,
            Cue::Cancel,
            Cue::Impact,
            Cue::ImpactStrong,
            Cue::ImpactWeak,
            Cue::Faint,
            Cue::AttackTackle,
            Cue::AttackVineWhip,
            Cue::CaptureThrow,
            Cue::CaptureCriticalThrow,
            Cue::CaptureRelease,
            Cue::CaptureAbsorb,
            Cue::CaptureShake,
            Cue::CaptureLock,
            Cue::HealthUp95,
            Cue::HealthUp110,
            Cue::HealthUp125,
            Cue::TownBgm,
        ];
        let mut pending = VecDeque::new();
        for cue in cues {
            let Some(path) = available_asset(&directory, cue) else {
                continue;
            };
            // The source list puts the BGM last, after lightweight UI effects.
            pending.push_back((cue, path));
        }
        Self {
            assets: HashMap::new(),
            pending,
            scheduled: VecDeque::new(),
            bgm_playing: false,
            enabled: true,
        }
    }

    /// Load at most one selected asset for each rendered frame.
    pub async fn load_next(&mut self) {
        let Some((cue, path)) = self.pending.pop_front() else {
            return;
        };
        let sound = if let Some(pitch) = cue.health_up_pitch() {
            let Ok(bytes) = std::fs::read(&path) else {
                return;
            };
            let Some(bytes) = retune_pcm_wav_header(&bytes, pitch) else {
                return;
            };
            audio::load_sound_from_bytes(&bytes).await
        } else {
            let Some(path) = path.to_str() else {
                return;
            };
            audio::load_sound(path).await
        };
        if let Ok(sound) = sound {
            self.assets.insert(cue, sound);
            if cue == Cue::TownBgm && self.enabled && self.play(cue) {
                self.bgm_playing = true;
            }
        }
    }

    pub fn play(&self, cue: Cue) -> bool {
        if !self.enabled {
            return false;
        }
        let Some(sound) = self.assets.get(&cue) else {
            return false;
        };
        let bgm = cue == Cue::TownBgm;
        let volume = match cue {
            Cue::TownBgm => 0.15, // source defaults: master 0.3 × BGM 0.5
            Cue::Navigate | Cue::Confirm | Cue::Cancel => 0.15, // master 0.3 × UI 0.5
            Cue::AttackTackle | Cue::AttackVineWhip => 0.075, // field volume × PRSFX 0.5
            Cue::HealthUp95 | Cue::HealthUp110 | Cue::HealthUp125 => 0.12, // master 0.3 × effect 0.8 × SFX 0.5
            _ => 0.15, // master 0.3 × sound effects 0.5
        };
        audio::play_sound(
            sound,
            PlaySoundParams {
                looped: bgm,
                volume,
            },
        );
        true
    }

    pub fn schedule(&mut self, cue: Cue, at: f64) {
        let scheduled = ScheduledCue { at, cue };
        let index = self
            .scheduled
            .iter()
            .position(|pending| pending.at > at)
            .unwrap_or(self.scheduled.len());
        self.scheduled.insert(index, scheduled);
    }

    /// Schedule cues from already-resolved core events. This reads battle
    /// output only; it never changes battle state or consumes randomness.
    pub fn schedule_battle_events(&mut self, battle: &Battle, now: f64) {
        let captured = battle.last_events.iter().any(|event| {
            matches!(
                event,
                BattleEvent::CaptureAttemptResolved { caught: true, .. }
            )
        });
        let has_capture = battle
            .last_events
            .iter()
            .any(|event| matches!(event, BattleEvent::CaptureAttemptResolved { .. }));
        let mut visual_delay = 0.0;

        if has_capture {
            if let Some(attempt) = battle.last_capture_attempt.as_ref() {
                let throw = if attempt.critical {
                    Cue::CaptureCriticalThrow
                } else {
                    Cue::CaptureThrow
                };
                self.schedule(throw, now);
                self.schedule(Cue::CaptureRelease, now + 0.5);
                self.schedule(Cue::CaptureAbsorb, now + 1.0);

                let shake_start = capture_shake_start(attempt.critical);
                let successful_shakes = if attempt.caught {
                    if attempt.critical { 1 } else { 3 }
                } else {
                    attempt.shake_rolls.len().saturating_sub(1)
                };
                for index in 0..successful_shakes {
                    self.schedule(Cue::CaptureShake, now + shake_start + index as f64);
                }
                if attempt.caught {
                    self.schedule(
                        Cue::CaptureLock,
                        now + shake_start + if attempt.critical { 2.0 } else { 4.0 },
                    );
                }
                visual_delay = capture_animation_duration(
                    attempt.critical,
                    attempt.caught,
                    attempt.shake_rolls.len(),
                );
            }
        }

        for event in &battle.last_events {
            match event {
                BattleEvent::MoveExecuted { move_, .. } => {
                    let animation = crate::generated_battle_anims::for_move(move_.numeric_id());
                    let duration = animation.map_or(0.0, |animation| {
                        animation.frames.len() as f64 * crate::BATTLE_ANIMATION_FRAME_SECONDS
                    });
                    if let Some(cue) = sound_for_move(move_.name()) {
                        self.schedule(cue, now + visual_delay);
                    }
                    visual_delay += duration;
                }
                BattleEvent::DamageApplied {
                    amount,
                    effectiveness,
                    ..
                } => {
                    self.schedule(damage_cue(*effectiveness), now + visual_delay);
                    visual_delay += crate::BattleAnimationPlayback::HIT_FLASH_SECONDS
                        + crate::hp_bar_animation_duration(*amount, 0);
                }
                BattleEvent::SitrusBerryConsumed { hp_restored, .. } => {
                    self.schedule(Cue::HealthUp95, now + visual_delay + 0.05);
                    self.schedule(Cue::HealthUp110, now + visual_delay + 0.15);
                    self.schedule(Cue::HealthUp125, now + visual_delay + 0.15);
                    visual_delay += crate::BattleAnimationPlayback::HEALTH_UP_NUMBER_DELAY_SECONDS
                        + crate::hp_bar_animation_duration(*hp_restored, 0);
                }
                BattleEvent::Switch(pokerogue_core::SwitchEvent::SwitchCompleted {
                    outgoing,
                    active,
                }) => {
                    let returns_outgoing = !battle.last_events.iter().any(|event| {
                        matches!(event, BattleEvent::PokemonFainted { slot } if slot == outgoing)
                    });
                    if returns_outgoing {
                        self.schedule(Cue::CaptureRelease, now + visual_delay);
                        visual_delay += 1.0;
                    }
                    self.schedule(Cue::CaptureRelease, now + visual_delay + 0.65);
                    let switch = crate::ActiveSwitchAnimation {
                        outgoing_slot: *outgoing,
                        active_slot: *active,
                        returns_outgoing,
                        started_at: now + visual_delay - if returns_outgoing { 1.0 } else { 0.0 },
                    };
                    visual_delay += crate::switch_animation_duration(switch);
                }
                BattleEvent::PokemonFainted { slot } if *slot == battle.player.active_slot() => {
                    self.schedule(Cue::Faint, now + visual_delay);
                }
                BattleEvent::BattleEnded(Outcome::PlayerWin) if !captured => {
                    self.schedule(Cue::Faint, now + visual_delay);
                }
                _ => {}
            }
        }
    }

    pub fn update(&mut self, now: f64) {
        while self
            .scheduled
            .front()
            .is_some_and(|pending| pending.at <= now)
        {
            if let Some(pending) = self.scheduled.pop_front() {
                self.play(pending.cue);
            }
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        if self.enabled == enabled && self.bgm_playing == enabled {
            return;
        }
        self.enabled = enabled;
        if !enabled {
            if let Some(sound) = self.assets.get(&Cue::TownBgm) {
                audio::stop_sound(sound);
            }
            self.bgm_playing = false;
        } else if self.play(Cue::TownBgm) {
            self.bgm_playing = true;
        }
    }

    pub fn toggle(&mut self) {
        self.set_enabled(!self.enabled);
    }
}

fn damage_cue(effectiveness: pokerogue_core::DamageEffectiveness) -> Cue {
    match effectiveness {
        pokerogue_core::DamageEffectiveness::Normal => Cue::Impact,
        pokerogue_core::DamageEffectiveness::SuperEffective => Cue::ImpactStrong,
        pokerogue_core::DamageEffectiveness::NotVeryEffective => Cue::ImpactWeak,
    }
}

fn capture_bounce_duration() -> f64 {
    let mut duration = 0.35;
    let mut power = 0.5_f64;
    while power > 0.01 {
        duration += power * 0.7;
        power *= 0.5;
    }
    duration
}

fn capture_shake_start(critical: bool) -> f64 {
    1.25 + if critical { 2.06 } else { 0.0 } + capture_bounce_duration()
}

fn capture_animation_duration(critical: bool, caught: bool, shake_roll_count: usize) -> f64 {
    let shake_end = if caught {
        if critical { 3.0 } else { 5.0 }
    } else if critical {
        2.25
    } else {
        shake_roll_count.clamp(1, 4) as f64 + 0.25
    };
    capture_shake_start(critical) + shake_end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_events_map_to_distinct_navigation_confirmation_and_cancel_cues() {
        assert_eq!(input_cue(true), Some(Cue::Navigate));
        assert_eq!(input_cue(false), None);
        assert_eq!(cue_for_ui_action(UiAction::Confirm), Cue::Confirm);
        assert_eq!(cue_for_ui_action(UiAction::Cancel), Cue::Cancel);
        assert_ne!(
            cue_for_ui_action(UiAction::Confirm),
            cue_for_ui_action(UiAction::Cancel)
        );
        assert_eq!(sound_for_move("Tackle"), Some(Cue::AttackTackle));
        assert_eq!(sound_for_move("Vine Whip"), Some(Cue::AttackVineWhip));
        assert_eq!(sound_for_move("Growl"), None);
    }

    #[test]
    fn health_up_pitch_variants_retime_only_pcm_header_rates() {
        let mut wav = vec![0; 48];
        wav[0..4].copy_from_slice(b"RIFF");
        wav[4..8].copy_from_slice(&40_u32.to_le_bytes());
        wav[8..12].copy_from_slice(b"WAVE");
        wav[12..16].copy_from_slice(b"fmt ");
        wav[16..20].copy_from_slice(&16_u32.to_le_bytes());
        wav[20..22].copy_from_slice(&1_u16.to_le_bytes());
        wav[22..24].copy_from_slice(&2_u16.to_le_bytes());
        wav[24..28].copy_from_slice(&12_000_u32.to_le_bytes());
        wav[28..32].copy_from_slice(&24_000_u32.to_le_bytes());
        wav[32..34].copy_from_slice(&2_u16.to_le_bytes());
        wav[34..36].copy_from_slice(&8_u16.to_le_bytes());
        wav[36..40].copy_from_slice(b"data");
        wav[40..44].copy_from_slice(&4_u32.to_le_bytes());
        wav[44..48].copy_from_slice(&[10, 20, 30, 40]);

        let tuned = retune_pcm_wav_header(&wav, 0.95).unwrap();
        assert_eq!(
            u32::from_le_bytes(tuned[24..28].try_into().unwrap()),
            11_400
        );
        assert_eq!(
            u32::from_le_bytes(tuned[28..32].try_into().unwrap()),
            22_800
        );
        assert_eq!(&tuned[44..48], &[10, 20, 30, 40]);
        assert!(retune_pcm_wav_header(&wav, 0.0).is_none());
    }

    #[test]
    fn sitrus_event_schedules_the_original_health_up_sound_sequence() {
        let mut run = pokerogue_core::MiniRun::new("HealthUpEventSeed", "DAY").unwrap();
        let slot = run.battle.player.active_slot();
        run.battle.last_events = vec![BattleEvent::SitrusBerryConsumed {
            slot,
            hp_restored: 9,
        }];
        let mut manager = AudioManager {
            assets: HashMap::new(),
            scheduled: VecDeque::new(),
            pending: VecDeque::new(),
            bgm_playing: false,
            enabled: true,
        };

        manager.schedule_battle_events(&run.battle, 7.0);
        let scheduled = manager.scheduled.iter().copied().collect::<Vec<_>>();
        assert_eq!(scheduled.len(), 3);
        assert_eq!(scheduled[0].cue, Cue::HealthUp95);
        assert!((scheduled[0].at - 7.05).abs() < 1e-9);
        assert_eq!(scheduled[1].cue, Cue::HealthUp110);
        assert!((scheduled[1].at - 7.15).abs() < 1e-9);
        assert_eq!(scheduled[2].cue, Cue::HealthUp125);
        assert!((scheduled[2].at - 7.15).abs() < 1e-9);
    }

    #[test]
    fn switch_events_schedule_recall_and_summon_release_sounds() {
        let mut battle = pokerogue_core::Battle::demo();
        let events = battle.switch_player(1).unwrap();
        battle.last_events = events.into_iter().map(BattleEvent::Switch).collect();
        let mut manager = AudioManager {
            assets: HashMap::new(),
            scheduled: VecDeque::new(),
            pending: VecDeque::new(),
            bgm_playing: false,
            enabled: true,
        };

        manager.schedule_battle_events(&battle, 12.0);
        let releases = manager
            .scheduled
            .iter()
            .filter(|cue| cue.cue == Cue::CaptureRelease)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(releases.len(), 2);
        assert!((releases[0].at - 12.0).abs() < 1e-9);
        assert!((releases[1].at - 13.65).abs() < 1e-9);
    }

    #[test]
    fn absent_assets_resolve_to_none_and_keep_audio_optional() {
        let missing = Path::new(env!("CARGO_MANIFEST_DIR")).join("missing-audio-fixture");
        assert!(available_asset(&missing, Cue::Navigate).is_none());
        assert!(available_asset(&missing, Cue::TownBgm).is_none());
        let manager = AudioManager {
            assets: HashMap::new(),
            scheduled: VecDeque::new(),
            pending: VecDeque::new(),
            bgm_playing: false,
            enabled: true,
        };
        assert!(!manager.play(Cue::Navigate));
        assert!(!manager.play(Cue::TownBgm));
    }

    #[test]
    fn mute_hotkey_state_can_toggle_even_when_assets_are_missing() {
        let mut manager = AudioManager {
            assets: HashMap::new(),
            scheduled: VecDeque::new(),
            pending: VecDeque::new(),
            bgm_playing: false,
            enabled: true,
        };
        manager.toggle();
        assert!(!manager.enabled);
        manager.toggle();
        assert!(manager.enabled);
    }

    #[test]
    fn cache_audio_directory_is_used_when_checkout_is_sparse() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let path = audio_asset_directory(manifest, None);
        assert!(path.ends_with("fb8ccb6d0703ffe48cdf80a8102d5ed943c82dba/audio"));
    }

    #[test]
    fn resolved_move_and_capture_events_schedule_cues_once_at_presentation_times() {
        let mut run = pokerogue_core::MiniRun::new("AudioEventSeed", "DAY").unwrap();
        run.battle
            .take_turn(pokerogue_core::PlayerAction::UseMove(1))
            .unwrap();
        let mut manager = AudioManager {
            assets: HashMap::new(),
            scheduled: VecDeque::new(),
            pending: VecDeque::new(),
            bgm_playing: false,
            enabled: true,
        };
        manager.schedule_battle_events(&run.battle, 10.0);
        assert!(
            manager
                .scheduled
                .iter()
                .any(|cue| cue.cue == Cue::AttackTackle)
        );
        assert!(manager.scheduled.iter().any(|cue| cue.cue == Cue::Impact));
        assert_eq!(
            manager
                .scheduled
                .iter()
                .filter(|cue| {
                    matches!(cue.cue, Cue::Impact | Cue::ImpactStrong | Cue::ImpactWeak)
                })
                .count(),
            2
        );
        assert!(
            manager
                .scheduled
                .iter()
                .all(|cue| cue.at >= 10.0 && cue.at <= 12.5)
        );
    }

    #[test]
    fn damage_cue_follows_the_core_effectiveness_event_and_original_sound_map() {
        assert_eq!(
            damage_cue(pokerogue_core::DamageEffectiveness::Normal),
            Cue::Impact
        );
        assert_eq!(
            damage_cue(pokerogue_core::DamageEffectiveness::SuperEffective),
            Cue::ImpactStrong
        );
        assert_eq!(
            damage_cue(pokerogue_core::DamageEffectiveness::NotVeryEffective),
            Cue::ImpactWeak
        );
        assert_eq!(Cue::ImpactStrong.relative_path(), "se/hit_strong.wav");
        assert_eq!(Cue::ImpactWeak.relative_path(), "se/hit_weak.wav");
    }

    #[test]
    fn faint_and_capture_events_schedule_their_source_named_effects() {
        let mut run = pokerogue_core::MiniRun::new("AudioCueSeed", "DAY").unwrap();
        run.battle.last_events = vec![BattleEvent::PokemonFainted {
            slot: run.battle.player.active_slot(),
        }];
        let mut faint_audio = AudioManager {
            assets: HashMap::new(),
            scheduled: VecDeque::new(),
            pending: VecDeque::new(),
            bgm_playing: false,
            enabled: true,
        };
        faint_audio.schedule_battle_events(&run.battle, 1.0);
        assert_eq!(faint_audio.scheduled.front().unwrap().cue, Cue::Faint);

        run.battle.last_events = vec![BattleEvent::CaptureAttemptResolved {
            species_id: run.battle.enemy.species.numeric_id(),
            caught: true,
            party_slot: Some(1),
        }];
        run.battle.last_capture_attempt = Some(pokerogue_core::CaptureAttempt {
            modified_catch_rate: 255,
            shake_probability: 1.0,
            critical: false,
            shake_rolls: vec![0, 0, 0],
            caught: true,
        });
        let mut capture_audio = AudioManager {
            assets: HashMap::new(),
            scheduled: VecDeque::new(),
            pending: VecDeque::new(),
            bgm_playing: false,
            enabled: true,
        };
        capture_audio.schedule_battle_events(&run.battle, 3.0);
        let scheduled = capture_audio
            .scheduled
            .iter()
            .map(|cue| (cue.at, cue.cue))
            .collect::<Vec<_>>();
        assert_eq!(scheduled[0].1, Cue::CaptureThrow);
        assert!(scheduled.iter().any(|(_, cue)| *cue == Cue::CaptureRelease));
        assert!(scheduled.iter().any(|(_, cue)| *cue == Cue::CaptureAbsorb));
        assert!(scheduled.iter().any(|(_, cue)| *cue == Cue::CaptureShake));
        assert!(scheduled.iter().any(|(_, cue)| *cue == Cue::CaptureLock));
        assert!(scheduled.windows(2).all(|pair| pair[0].0 <= pair[1].0));
    }
}
