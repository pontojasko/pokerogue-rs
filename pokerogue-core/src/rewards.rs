//! Source-backed reward definitions, weights and deterministic offer helpers.
//! MiniRun still uses a custom guaranteed option list until all effects are wired.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reward {
    Pokeballs,
    GreatBalls,
    UltraBalls,
    ExpCharm,
    ExpShare,
    AmuletCoin,
    BigNugget,
    PpUp,
    PpMax,
    Nugget,
    Elixir,
    MaxElixir,
    RareCandy,
    Potion,
    SuperPotion,
    HyperPotion,
    MaxPotion,
    FullRestore,
    SootheBell,
    Revive,
    MaxRevive,
    Ether,
    MaxEther,
    Lure,
    TempAccuracy,
    TempSpeed,
    TempAttack,
    TempDefense,
    TempSpecialAttack,
    TempSpecialDefense,
    SitrusBerry,
}
impl Reward {
    pub fn id(self) -> &'static str {
        match self {
            Self::Pokeballs => "POKEBALL",
            Self::GreatBalls => "GREAT_BALL",
            Self::UltraBalls => "ULTRA_BALL",
            Self::ExpCharm => "EXP_CHARM",
            Self::ExpShare => "EXP_SHARE",
            Self::AmuletCoin => "AMULET_COIN",
            Self::BigNugget => "BIG_NUGGET",
            Self::PpUp => "PP_UP",
            Self::PpMax => "PP_MAX",
            Self::Nugget => "NUGGET",
            Self::Elixir => "ELIXIR",
            Self::MaxElixir => "MAX_ELIXIR",
            Self::RareCandy => "RARE_CANDY",
            Self::Potion => "POTION",
            Self::SuperPotion => "SUPER_POTION",
            Self::HyperPotion => "HYPER_POTION",
            Self::MaxPotion => "MAX_POTION",
            Self::FullRestore => "FULL_RESTORE",
            Self::SootheBell => "SOOTHE_BELL",
            Self::Revive => "REVIVE",
            Self::MaxRevive => "MAX_REVIVE",
            Self::Ether => "ETHER",
            Self::MaxEther => "MAX_ETHER",
            Self::Lure => "LURE",
            Self::TempAccuracy => "TEMP_STAT_STAGE_BOOSTER:ACC",
            Self::TempSpeed => "TEMP_STAT_STAGE_BOOSTER:SPD",
            Self::TempAttack => "TEMP_STAT_STAGE_BOOSTER:ATK",
            Self::TempDefense => "TEMP_STAT_STAGE_BOOSTER:DEF",
            Self::TempSpecialAttack => "TEMP_STAT_STAGE_BOOSTER:SPATK",
            Self::TempSpecialDefense => "TEMP_STAT_STAGE_BOOSTER:SPDEF",
            Self::SitrusBerry => "BERRY:SITRUS",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Pokeballs => "5 Poke Balls",
            Self::GreatBalls => "5 Great Balls",
            Self::UltraBalls => "5 Ultra Balls",
            Self::ExpCharm => "EXP Charm · +25% EXP",
            Self::ExpShare => "EXP Share · 20% to nonparticipants",
            Self::AmuletCoin => "Amulet Coin · +20% money per stack",
            Self::BigNugget => "Big Nugget",
            Self::PpUp => "PP Up",
            Self::PpMax => "PP Max",
            Self::Nugget => "Nugget",
            Self::Elixir => "Elixir",
            Self::MaxElixir => "Max Elixir",
            Self::RareCandy => "Rare Candy",
            Self::Potion => "Potion",
            Self::SuperPotion => "Super Potion",
            Self::HyperPotion => "Hyper Potion",
            Self::MaxPotion => "Max Potion",
            Self::FullRestore => "Full Restore",
            Self::SootheBell => "Soothe Bell · +50% friendship gains",
            Self::Revive => "Revive",
            Self::MaxRevive => "Max Revive",
            Self::Ether => "Ether",
            Self::MaxEther => "Max Ether",
            Self::Lure => "Lure (10 battles)",
            Self::TempAccuracy => "X Accuracy (5 battles)",
            Self::TempSpeed => "X Speed (5 battles)",
            Self::TempAttack => "X Attack (5 battles)",
            Self::TempDefense => "X Defense (5 battles)",
            Self::TempSpecialAttack => "X Sp. Atk (5 battles)",
            Self::TempSpecialDefense => "X Sp. Def (5 battles)",
            Self::SitrusBerry => "Sitrus Berry",
        }
    }
    // Great-tier values are needed by the natural offer display and claim path.
    pub fn tier(self) -> u8 {
        match self {
            Self::GreatBalls
            | Self::Nugget
            | Self::PpUp
            | Self::Elixir
            | Self::MaxElixir
            | Self::HyperPotion
            | Self::MaxPotion
            | Self::FullRestore
            | Self::SootheBell
            | Self::Revive
            | Self::MaxRevive => 1,
            Self::PpMax
            | Self::UltraBalls
            | Self::ExpCharm
            | Self::ExpShare
            | Self::AmuletCoin
            | Self::BigNugget => 2,
            _ => 0,
        }
    }
    pub fn heal(self, hp: i32, max_hp: i32) -> Option<i32> {
        if hp <= 0 || hp >= max_hp {
            return None;
        }
        let (points, percent) = match self {
            Self::Potion => (20, 10),
            Self::SuperPotion => (50, 25),
            Self::HyperPotion => (200, 50),
            Self::MaxPotion | Self::FullRestore => (0, 100),
            Self::Pokeballs
            | Self::GreatBalls
            | Self::UltraBalls
            | Self::ExpCharm
            | Self::ExpShare
            | Self::AmuletCoin
            | Self::BigNugget
            | Self::PpUp
            | Self::PpMax
            | Self::Nugget
            | Self::Elixir
            | Self::MaxElixir
            | Self::RareCandy
            | Self::Ether
            | Self::MaxEther
            | Self::Revive
            | Self::MaxRevive
            | Self::SootheBell
            | Self::Lure => return None,
            Self::TempAccuracy
            | Self::TempSpeed
            | Self::TempAttack
            | Self::TempDefense
            | Self::TempSpecialAttack
            | Self::TempSpecialDefense => return None,
            Self::SitrusBerry => return None,
        };
        Some(
            (hp + points
                .max((f64::from(percent) * 0.01 * f64::from(max_hp)).floor() as i32)
                .max(1))
            .min(max_hp),
        )
    }

    /// Source revive branch of PokemonHpRestoreModifier.apply.
    pub fn revive(self, hp: i32, max_hp: i32) -> Option<i32> {
        if hp != 0 || !matches!(self, Self::Revive | Self::MaxRevive) || max_hp <= 0 {
            return None;
        }
        let percent = if self == Self::Revive { 50 } else { 100 };
        Some((max_hp * percent / 100).max(1).min(max_hp))
    }
}

pub const LURE_MAX_BATTLES: u8 = 10;
pub const TEMP_STAT_BOOSTER_MAX_BATTLES: u8 = 5;

/// `LapsingPersistentModifier.lapse`: decrement after a battle and remove at
/// zero. `None` means the modifier expires at this battle end.
pub fn lapse_lure_battles(remaining: u8) -> Option<u8> {
    let next = remaining.saturating_sub(1);
    (next > 0).then_some(next)
}

pub fn lapse_temp_stat_booster(remaining: u8) -> Option<u8> {
    let next = remaining.saturating_sub(1);
    (next > 0).then_some(next)
}

pub const GUARANTEED_REWARDS: [Reward; 3] =
    [Reward::Pokeballs, Reward::Potion, Reward::SuperPotion];

/// Reward RNG reset used by SelectModifierPhase -> BattleScene.resetSeed.
/// It is a separate Phaser stream seeded with the run seed shifted by wave.
pub fn reward_rng_for_wave(run_seed: &str, wave: u32) -> crate::BattleRng {
    crate::BattleRng::from_utf16_seed(shifted_reward_seed_units(run_seed, wave))
}

pub fn shifted_reward_seed_units(run_seed: &str, wave: u32) -> Vec<u16> {
    run_seed
        .encode_utf16()
        .map(|unit| unit.wrapping_add(wave as u16))
        .collect()
}

/// Natural PLAYER tier branch of getNewModifierTypeOption, with no forced tier.
/// The caller supplies the reward RNG stream and the actual nonempty pool tiers.
/// Pool eligibility and weighted item generation must follow this operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TierRoll {
    pub tier: u8,
    pub upgrades: u32,
}

pub fn roll_player_reward_tier(
    party_luck: u8,
    allow_luck_upgrades: bool,
    nonempty_tiers: [bool; 5],
    mut rand_int: impl FnMut(u32) -> u32,
) -> TierRoll {
    let value = rand_int(1024);
    let mut upgrades: u32 = 0;
    if value != 0 && allow_luck_upgrades {
        let odds = (128.0 / ((f64::from(party_luck) + 4.0) / 4.0)).floor() as u32;
        while rand_int(odds) < 4 {
            upgrades += 1;
        }
    }
    let base = if value > 255 {
        0
    } else if value > 60 {
        1
    } else if value > 12 {
        2
    } else if value != 0 {
        3
    } else {
        4
    };
    let mut tier = base + upgrades;
    while tier > 0 && (tier > 4 || !nonempty_tiers[tier as usize]) {
        tier -= 1;
        upgrades = upgrades.saturating_sub(1);
    }
    TierRoll {
        tier: tier as u8,
        upgrades,
    }
}

/// Select an original pool index from source-computed eligibility weights.
/// Zero-weight entries retain their indices but cannot be selected.
/// An empty eligible pool returns None without drawing RNG.
pub fn roll_weighted_reward_index(
    weights: &[u32],
    mut rand_int: impl FnMut(u32) -> u32,
) -> Option<usize> {
    let total = weights
        .iter()
        .try_fold(0u32, |sum, weight| sum.checked_add(*weight))?;
    if total == 0 {
        return None;
    }
    let value = rand_int(total);
    let mut threshold = 0;
    for (index, weight) in weights.iter().enumerate() {
        threshold += weight;
        if value < threshold {
            return Some(index);
        }
    }
    None
}

/// Inputs to the original common-pool weight callbacks (before stack-cap filtering).
#[derive(Clone, Debug)]
pub struct RewardPartyMember {
    pub hp: i32,
    pub max_hp: i32,
    pub holds_leppa: bool,
    /// (maximum PP, PP used), matching the original move wrapper.
    pub move_pp: Vec<(u32, u32)>,
}

pub const COMMON_POOL_IDS: [&str; 10] = [
    "POKEBALL",
    "RARE_CANDY",
    "POTION",
    "SUPER_POTION",
    "ETHER",
    "MAX_ETHER",
    "LURE",
    "TEMP_STAT_STAGE_BOOSTER",
    "BERRY",
    "TM_COMMON",
];

/// Original initCommonModifierPool weights. Other eligibility layers (existing
/// modifier caps and generated-type validity) must still be applied by the caller.
pub fn common_pool_weights(
    party: &[RewardPartyMember],
    classic: bool,
    wave: u32,
    pokeballs: u32,
    lure_battles: &[(u32, u32)],
) -> [u32; 10] {
    let count_hp = |missing: i32, ratio: f64| {
        party
            .iter()
            .filter(|p| {
                p.hp > 0
                    && p.max_hp - p.hp >= missing
                    && (f64::from(p.hp) / f64::from(p.max_hp) * 100.0).round() / 100.0 <= ratio
            })
            .count()
            .min(3) as u32
    };
    let pp_count = party
        .iter()
        .filter(|p| {
            p.hp != 0
                && !p.holds_leppa
                && p.move_pp.iter().any(|&(max, used)| {
                    used != 0 && i64::from(max) - i64::from(used) <= 5 && used > max / 2
                })
        })
        .count()
        .min(3) as u32;
    let lure = !(classic && wave == 199)
        && !lure_battles
            .iter()
            .any(|&(max, remaining)| max == 10 && f64::from(remaining) >= f64::from(max) * 0.6);
    [
        if classic && pokeballs >= 99 { 0 } else { 6 },
        2,
        count_hp(10, 0.875) * 3,
        count_hp(25, 0.75),
        pp_count * 3,
        pp_count,
        if lure { 2 } else { 0 },
        4,
        2,
        2,
    ]
}

#[derive(Clone, Debug)]
pub struct GreatRewardPartyMember {
    pub hp: i32,
    pub max_hp: i32,
    pub has_status: bool,
    pub status_protected: bool,
    pub holds_leppa: bool,
    pub move_pp: Vec<(u32, u32)>,
    pub has_learnable_level_moves: bool,
    pub tera_eligible: bool,
    pub is_fused: bool,
}

#[derive(Clone, Debug)]
pub struct GreatRewardContext {
    pub classic: bool,
    pub daily: bool,
    pub spliced_only: bool,
    pub endless: bool,
    pub fresh_start_challenge: bool,
    pub mini_black_hole_unlocked: bool,
    pub wave: u32,
    /// Poké Ball, Great Ball, Ultra Ball, Rogue Ball, Master Ball.
    pub ball_counts: [u8; 5],
    pub lures: Vec<(u32, u32)>,
    pub soothe_bell_boosted: bool,
    pub fusions_boosted: bool,
    pub party: Vec<GreatRewardPartyMember>,
}

/// Source callback weights for the original GREAT PLAYER pool, before stack
/// caps/reroll challenge adjustments. Array positions retain imported order.
pub fn great_pool_weights(context: &GreatRewardContext, reroll_count: u32) -> [u32; 24] {
    let eligible = |predicate: &dyn Fn(&GreatRewardPartyMember) -> bool| {
        context
            .party
            .iter()
            .filter(|member| predicate(member))
            .count()
            .min(3) as u32
    };
    let status_count =
        eligible(&|member| member.hp > 0 && member.has_status && !member.status_protected);
    let fainted_count = context.party.iter().filter(|member| member.hp <= 0).count() as u32;
    let hp_count = |minimum_missing: i32, ratio: f64| {
        eligible(&|member| {
            member.hp > 0
                && member.max_hp - member.hp >= minimum_missing
                && (f64::from(member.hp) / f64::from(member.max_hp) * 100.0).round() / 100.0
                    <= ratio
        })
    };
    let pp_count = eligible(&|member| {
        member.hp > 0
            && !member.holds_leppa
            && member.move_pp.iter().any(|&(max, used)| {
                used != 0 && i64::from(max) - i64::from(used) <= 5 && used > max / 2
            })
    });
    let lure_allowed = !(context.classic && context.wave == 199)
        && !context
            .lures
            .iter()
            .any(|&(max, remaining)| max == 15 && f64::from(remaining) >= f64::from(max) * 0.6);
    let wave_default = |weight: u32| {
        if context.classic && context.wave >= 199 {
            0
        } else {
            weight
        }
    };
    let mode_adjusted_wave = context.wave;
    let has_non_fused_pair = context
        .party
        .iter()
        .filter(|member| !member.is_fused)
        .count()
        > 1;
    [
        if context.classic && context.ball_counts[1] >= 99 {
            0
        } else {
            6
        },
        2,
        status_count * 6,
        fainted_count.min(3) * 9,
        fainted_count.min(3) * 3,
        u32::from(fainted_count >= context.party.len().div_ceil(2) as u32),
        hp_count(100, 0.625) * 3,
        hp_count(100, 0.5),
        ((hp_count(100, 0.5) + status_count) / 2),
        pp_count * 3,
        pp_count,
        4,
        if lure_allowed { 4 } else { 0 },
        wave_default(5),
        2,
        (context.wave.div_ceil(15)).min(8),
        if context.classic && context.wave < 180 {
            2
        } else {
            0
        },
        if context.soothe_bell_boosted { 3 } else { 2 },
        3,
        if context
            .party
            .iter()
            .any(|member| member.has_learnable_level_moves)
        {
            (1 + mode_adjusted_wave / 30).min(4)
        } else {
            0
        },
        3,
        u32::from(context.party.iter().any(|member| member.tera_eligible)),
        if has_non_fused_pair && context.spliced_only {
            4
        } else if has_non_fused_pair && context.classic && context.fusions_boosted {
            2
        } else {
            0
        },
        if context.daily {
            0
        } else {
            1u32.saturating_sub(reroll_count)
        },
    ]
}

pub fn rogue_pool_weights(context: &GreatRewardContext, reroll_count: u32) -> [u32; 19] {
    let wave_default = |weight: u32| {
        if context.classic && context.wave >= 199 {
            0
        } else {
            weight
        }
    };
    [
        if context.classic && context.ball_counts[3] >= 99 {
            0
        } else {
            16
        },
        wave_default(2),
        3,
        3,
        4,
        5,
        4,
        2,
        7,
        if context.classic { 0 } else { 4 },
        if context.classic && context.wave >= 189 {
            0
        } else {
            6
        },
        5,
        3,
        if context.classic { 0 } else { 3 },
        wave_default(8),
        context.wave.div_ceil(50).min(4) * 6,
        context.wave.div_ceil(50).min(4) * 9,
        context.wave.div_ceil(50).min(4) * 9,
        if context.daily {
            0
        } else {
            3u32.saturating_sub(reroll_count)
        },
    ]
}

pub fn master_pool_weights(context: &GreatRewardContext, reroll_count: u32) -> [u32; 7] {
    let unmerged_party = context
        .party
        .iter()
        .filter(|member| !member.is_fused)
        .count();
    [
        if context.classic && context.ball_counts[4] >= 99 {
            0
        } else {
            24
        },
        14,
        18,
        18,
        if context.daily || context.endless || context.spliced_only {
            0
        } else {
            5u32.saturating_sub(reroll_count * 2)
        },
        if context.spliced_only || (context.classic && context.fusions_boosted) {
            0
        } else if unmerged_party > 1 {
            24
        } else {
            0
        },
        if context.daily || (!context.fresh_start_challenge && context.mini_black_hole_unlocked) {
            1
        } else {
            0
        },
    ]
}

#[derive(Clone, Copy, Debug)]
pub struct UltraRewardContext {
    pub classic: bool,
    pub daily: bool,
    pub fresh_start_challenge: bool,
    pub eviolite_unlocked: bool,
    pub wave: u32,
    pub ultra_ball_count: u8,
    pub max_lure_battles_remaining: Option<u32>,
    pub eviolite_eligible: bool,
    pub leek_eligible: bool,
    pub toxic_orb_eligible: bool,
    pub flame_orb_eligible: bool,
    pub mystical_rock_eligible: bool,
}

/// Source callback weights for the original ULTRA PLAYER pool. Eligibility
/// booleans are the results of party predicates; form/ability modeling remains
/// the caller's responsibility.
pub fn ultra_pool_weights(context: &UltraRewardContext) -> [u32; 26] {
    let classic_last = context.classic && context.wave == 199;
    let last_wave_default = |weight| {
        if classic_last { 0 } else { weight }
    };
    let max_lure_allowed = !classic_last
        && !context
            .max_lure_battles_remaining
            .is_some_and(|remaining| remaining >= 18);
    let eviolite_allowed =
        context.daily || (!context.fresh_start_challenge && context.eviolite_unlocked);
    let form_item_weight = context.wave.div_ceil(50).min(4) * 6;
    let tera_orb_weight = if context.classic {
        0
    } else {
        ((context.wave / 50) * 2).clamp(1, 4)
    };
    [
        if context.classic && context.ultra_ball_count >= 99 {
            0
        } else {
            15
        },
        if max_lure_allowed { 4 } else { 0 },
        last_wave_default(12),
        3,
        4,
        (context.wave.div_ceil(15) * 4).min(32),
        form_item_weight,
        last_wave_default(3),
        if eviolite_allowed && context.eviolite_eligible {
            10
        } else {
            0
        },
        12,
        if context.leek_eligible { 12 } else { 0 },
        if context.toxic_orb_eligible { 10 } else { 0 },
        if context.flame_orb_eligible { 10 } else { 0 },
        if context.mystical_rock_eligible {
            10
        } else {
            0
        },
        4,
        last_wave_default(5),
        9,
        11,
        4,
        last_wave_default(2),
        last_wave_default(4),
        last_wave_default(8),
        last_wave_default(10),
        tera_orb_weight,
        3,
        7,
    ]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RewardCandidate {
    pub name: String,
    /// None matches the source's undefined group, including undefined equality.
    pub group: Option<String>,
    pub tier: u8,
    pub upgrades: u32,
}

/// No-challenge path of getModifierTypeOptionWithRetry. Retries preserve the
/// candidate's tier and upgrade count; duplicate results are accepted at the limit.
/// A generator error propagates rather than silently removing unsupported items.
pub fn reward_candidate_with_retry<E>(
    existing: &[RewardCandidate],
    retry_count: usize,
    mut generate: impl FnMut(Option<u8>, Option<u32>) -> Result<RewardCandidate, E>,
) -> Result<RewardCandidate, E> {
    let mut candidate = generate(None, None)?;
    let mut retries = 0;
    while !existing.is_empty() {
        retries += 1;
        if retries >= retry_count
            || !existing
                .iter()
                .any(|old| old.name == candidate.name || old.group == candidate.group)
        {
            break;
        }
        candidate = generate(Some(candidate.tier), Some(candidate.upgrades))?;
    }
    Ok(candidate)
}

/// Stack-cap gate from regenerateModifierPoolThresholds, after generated-type
/// classification and before cumulative thresholds are built.
pub fn stack_filtered_weight(
    raw_weight: u32,
    existing_stacks_and_caps: &[(u32, u32)],
    is_held_item: bool,
    is_form_change_item: bool,
) -> u32 {
    if existing_stacks_and_caps.is_empty()
        || is_held_item
        || is_form_change_item
        || existing_stacks_and_caps
            .iter()
            .any(|&(stack, cap)| stack < cap)
    {
        raw_weight
    } else {
        0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExistingModifierStack {
    pub id: &'static str,
    pub stack: u32,
    pub cap: u32,
    pub is_held_item: bool,
    pub is_form_change_item: bool,
}

/// Applies regenerateModifierPoolThresholds' existing-stack gate to raw
/// source-ordered weights. `None` means the supplied slice does not match the
/// imported pool; zero weights retain their original pool indexes.
pub fn filter_modifier_pool_stack_caps(
    tier: u8,
    raw_weights: &[u32],
    existing: &[ExistingModifierStack],
) -> Option<Vec<u32>> {
    let entries: Vec<_> = pokerogue_data::IMPORTED_PLAYER_MODIFIER_POOL
        .iter()
        .filter(|entry| entry.tier == tier)
        .collect();
    if entries.len() != raw_weights.len() {
        return None;
    }
    Some(
        entries
            .iter()
            .zip(raw_weights)
            .map(|(entry, &weight)| {
                let matching: Vec<_> = existing
                    .iter()
                    .filter(|modifier| modifier.id == entry.id)
                    .map(|modifier| (modifier.stack, modifier.cap))
                    .collect();
                let held_item = existing
                    .iter()
                    .any(|modifier| modifier.id == entry.id && modifier.is_held_item);
                let form_change = existing
                    .iter()
                    .any(|modifier| modifier.id == entry.id && modifier.is_form_change_item);
                stack_filtered_weight(weight, &matching, held_item, form_change)
            })
            .collect(),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RolledRewardIndex {
    pub tier: u8,
    pub upgrades: u32,
    pub pool_index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NaturalRewardCandidate {
    pub id: &'static str,
    pub tier: u8,
    pub upgrades: u32,
    pub pool_index: usize,
    pub rng_draws: Vec<crate::RngDraw>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NaturalRewardOption {
    pub id: &'static str,
    pub tier: u8,
    pub upgrades: u32,
    pub pool_index: usize,
    pub name: String,
    pub group: Option<String>,
    pub generated_variant: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NaturalRewardIdentity {
    pub name: String,
    pub group: Option<String>,
    pub generated_variant: Option<String>,
}

/// Resolve currently supported generated PLAYER reward types.
/// `compatible_tms` must already be source-filtered and de-duplicated by tier.
pub fn resolve_generated_modifier(
    id: &str,
    rng: &mut crate::BattleRng,
    turn: u32,
    compatible_tms: &[&str],
) -> Option<NaturalRewardIdentity> {
    let variant = match id {
        "TEMP_STAT_STAGE_BOOSTER" => {
            const STATS: [&str; 6] = ["ATK", "DEF", "SPATK", "SPDEF", "SPD", "ACC"];
            STATS[rng.rand_int(turn, STATS.len() as u32, 1) as usize - 1]
        }
        "BERRY" => {
            const BERRIES: [&str; 11] = [
                "SITRUS", "LUM", "ENIGMA", "LIECHI", "GANLON", "PETAYA", "APICOT", "SALAC",
                "LANSAT", "STARF", "LEPPA",
            ];
            let roll = rng.rand_int(turn, 12, 0);
            match roll {
                0..=1 => "SITRUS",
                2..=3 => "LUM",
                4..=5 => "LEPPA",
                _ => BERRIES[rng.rand_int(turn, 8, 0) as usize + 2],
            }
        }
        "TM_COMMON" | "TM_GREAT" => {
            if compatible_tms.is_empty() {
                return None;
            }
            compatible_tms[rng.rand_int(turn, compatible_tms.len() as u32, 0) as usize]
        }
        _ => return None,
    };
    let group = match id {
        "BERRY" => Some("berry".to_owned()),
        "TM_COMMON" | "TM_GREAT" => Some("tm".to_owned()),
        _ => None,
    };
    Some(NaturalRewardIdentity {
        name: format!("{id}:{variant}"),
        group,
        generated_variant: Some(variant.to_owned()),
    })
}

/// Resolve a modifier from the imported COMMON PLAYER pool using the source
/// generator for generated entries and the source group for duplicate checks.
/// Names stay as stable IDs until localized modifier names are imported.
pub fn resolve_common_player_modifier(
    id: &'static str,
    rng: &mut crate::BattleRng,
    turn: u32,
    compatible_tms: &[&str],
) -> Option<NaturalRewardIdentity> {
    if !pokerogue_data::IMPORTED_PLAYER_MODIFIER_POOL
        .iter()
        .any(|entry| entry.tier == 0 && entry.id == id)
    {
        return None;
    }
    if matches!(id, "TEMP_STAT_STAGE_BOOSTER" | "BERRY" | "TM_COMMON") {
        return resolve_generated_modifier(id, rng, turn, compatible_tms);
    }
    let group = match id {
        "POKEBALL" => Some("pb"),
        "POTION" | "SUPER_POTION" => Some("potion"),
        "ETHER" | "MAX_ETHER" => Some("ether"),
        "LURE" => Some("lure"),
        _ => None,
    };
    Some(NaturalRewardIdentity {
        name: id.to_owned(),
        group: group.map(str::to_owned),
        generated_variant: None,
    })
}

/// Static Great-tier reward identities/groups needed by the bounded selector.
/// Only the subset represented by MiniRun may be claimed.
pub fn resolve_great_direct_modifier(id: &'static str) -> Option<NaturalRewardIdentity> {
    let group = match id {
        "GREAT_BALL" => Some("pb"),
        "PP_UP" => Some("ppUp"),
        "ELIXIR" | "MAX_ELIXIR" => Some("elixir"),
        "HYPER_POTION" | "MAX_POTION" => Some("potion"),
        "FULL_RESTORE" => Some("potion"),
        "SOOTHE_BELL" => None,
        "REVIVE" | "MAX_REVIVE" => Some("revive"),
        // The original DIRE_HIT type has no duplicate group, matching Rare Candy's None.
        "DIRE_HIT" => None,
        "NUGGET" => Some("money"),
        _ => return None,
    };
    Some(NaturalRewardIdentity {
        name: id.to_owned(),
        group: group.map(str::to_owned),
        generated_variant: None,
    })
}

pub fn resolve_ultra_direct_modifier(id: &'static str) -> Option<NaturalRewardIdentity> {
    let group = match id {
        "ULTRA_BALL" => Some("pb"),
        "BIG_NUGGET" => Some("money"),
        "PP_MAX" => Some("ppUp"),
        // `ExpBoosterModifierType` omits the group argument to ModifierType.
        // Its `undefined` group is significant for source duplicate rerolls.
        "EXP_CHARM" | "EXP_SHARE" | "AMULET_COIN" => None,
        _ => return None,
    };
    Some(NaturalRewardIdentity {
        name: id.to_owned(),
        group: group.map(str::to_owned),
        generated_variant: None,
    })
}

/// Source-ordered compatible TM candidates from the current modeled party.
/// This port only models the imported base-species TM data.
pub fn compatible_tms_for_party(team: &crate::Team, tier: u8) -> Vec<&'static str> {
    compatible_tms_for_party_excluding_used(team, tier)
}

/// As the original party generator, but including each Pokémon's run-local
/// `usedTMs` list when deciding which generated TM candidates remain legal.
pub fn compatible_tms_for_party_excluding_used(team: &crate::Team, tier: u8) -> Vec<&'static str> {
    let mut result = Vec::new();
    for member in 0..team.len() {
        let Some(pokemon) = team.member(member) else {
            continue;
        };
        let known = pokemon.moves[..pokemon.move_count()]
            .iter()
            .map(|movement| movement.numeric_id())
            .collect::<Vec<_>>();
        for movement in pokerogue_data::compatible_tms_for_species(
            pokemon.species.numeric_id(),
            &known,
            &pokemon.used_tm_ids,
            tier,
        ) {
            if !result.contains(&movement.id) {
                result.push(movement.id);
            }
        }
    }
    result
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NaturalRewardOffer {
    pub options: Vec<NaturalRewardOption>,
    pub rng_draws: Vec<crate::RngDraw>,
}

fn roll_offer_option(
    rng: &mut crate::BattleRng,
    turn: u32,
    forced: Option<(u8, u32)>,
    party_luck: u8,
    allow_luck_upgrades: bool,
    weights_for_tier: &mut impl FnMut(u8) -> Vec<u32>,
    identify: &mut impl FnMut(&'static str, &mut crate::BattleRng) -> Option<NaturalRewardIdentity>,
) -> Option<NaturalRewardOption> {
    let nonempty_tiers = std::array::from_fn(|tier| {
        pokerogue_data::IMPORTED_PLAYER_MODIFIER_POOL
            .iter()
            .any(|entry| entry.tier as usize == tier)
    });
    let (mut tier, upgrades) = if let Some(forced) = forced {
        forced
    } else {
        let roll =
            roll_player_reward_tier(party_luck, allow_luck_upgrades, nonempty_tiers, |range| {
                rng.rand_int(turn, range, 0) as u32
            });
        (roll.tier, roll.upgrades)
    };
    let mut generator_retries = 0usize;
    loop {
        let weights = weights_for_tier(tier);
        let pool_index =
            roll_weighted_reward_index(&weights, |range| rng.rand_int(turn, range, 0) as u32)?;
        let entry = pokerogue_data::IMPORTED_PLAYER_MODIFIER_POOL
            .iter()
            .find(|entry| entry.tier == tier && entry.pool_index as usize == pool_index)?;
        if let Some(identity) = identify(entry.id, rng) {
            return Some(NaturalRewardOption {
                id: entry.id,
                tier,
                upgrades,
                pool_index,
                name: identity.name,
                group: identity.group,
                generated_variant: identity.generated_variant,
            });
        }
        // getNewModifierTypeOption retries a null generated type with its
        // current tier/upgrades; every 100 failed candidates it steps down.
        generator_retries += 1;
        if generator_retries >= 100 && tier > 0 {
            tier -= 1;
            generator_retries = 0;
        }
    }
}

/// Generate source-ordered natural PLAYER options on one wave-seeded RNG stream.
/// Candidate identity resolves generated types; returning `None` models a
/// source generator returning null and retries weighted selection.
pub fn roll_natural_player_offer(
    run_seed: &str,
    wave: u32,
    party_luck: u8,
    allow_luck_upgrades: bool,
    option_count: usize,
    retry_count: usize,
    mut weights_for_tier: impl FnMut(u8) -> Vec<u32>,
    mut identify: impl FnMut(&'static str, &mut crate::BattleRng) -> Option<NaturalRewardIdentity>,
) -> Option<NaturalRewardOffer> {
    let mut rng = reward_rng_for_wave(run_seed, wave);
    let mut options: Vec<NaturalRewardOption> = Vec::with_capacity(option_count);
    for _ in 0..option_count {
        let mut option = roll_offer_option(
            &mut rng,
            0,
            None,
            party_luck,
            allow_luck_upgrades,
            &mut weights_for_tier,
            &mut identify,
        )?;
        if !options.is_empty() {
            let mut retries = 0;
            while retries + 1 < retry_count
                && options
                    .iter()
                    .any(|old| old.name == option.name || old.group == option.group)
            {
                retries += 1;
                option = roll_offer_option(
                    &mut rng,
                    0,
                    Some((option.tier, option.upgrades)),
                    party_luck,
                    allow_luck_upgrades,
                    &mut weights_for_tier,
                    &mut identify,
                )?;
            }
        }
        options.push(option);
    }
    Some(NaturalRewardOffer {
        options,
        rng_draws: rng.draws().to_vec(),
    })
}

/// Generate one natural PLAYER candidate on the source wave-seeded stream.
/// `weights_for_tier` supplies source-ordered eligible weights after the same
/// dynamic callbacks and stack-cap filtering used by pool regeneration.
pub fn roll_natural_player_candidate(
    run_seed: &str,
    wave: u32,
    party_luck: u8,
    allow_luck_upgrades: bool,
    mut weights_for_tier: impl FnMut(u8) -> Vec<u32>,
) -> Option<NaturalRewardCandidate> {
    let nonempty_tiers = std::array::from_fn(|tier| {
        pokerogue_data::IMPORTED_PLAYER_MODIFIER_POOL
            .iter()
            .any(|entry| entry.tier as usize == tier)
    });
    let mut rng = reward_rng_for_wave(run_seed, wave);
    let selected = roll_player_reward_index(
        party_luck,
        allow_luck_upgrades,
        nonempty_tiers,
        |tier| weights_for_tier(tier),
        |range| rng.rand_int(0, range, 0) as u32,
    )?;
    let entry = pokerogue_data::IMPORTED_PLAYER_MODIFIER_POOL
        .iter()
        .find(|entry| {
            entry.tier == selected.tier && entry.pool_index as usize == selected.pool_index
        })?;
    Some(NaturalRewardCandidate {
        id: entry.id,
        tier: selected.tier,
        upgrades: selected.upgrades,
        pool_index: selected.pool_index,
        rng_draws: rng.draws().to_vec(),
    })
}

/// Compose source tier selection followed by the tier's cumulative weight draw.
/// `weights_for_tier` must expose source-ordered weights after eligibility filters.
pub fn roll_player_reward_index(
    party_luck: u8,
    allow_luck_upgrades: bool,
    nonempty_tiers: [bool; 5],
    weights_for_tier: impl FnOnce(u8) -> Vec<u32>,
    mut rand_int: impl FnMut(u32) -> u32,
) -> Option<RolledRewardIndex> {
    let tier = roll_player_reward_tier(
        party_luck,
        allow_luck_upgrades,
        nonempty_tiers,
        &mut rand_int,
    );
    let weights = weights_for_tier(tier.tier);
    let pool_index = roll_weighted_reward_index(&weights, &mut rand_int)?;
    Some(RolledRewardIndex {
        tier: tier.tier,
        upgrades: tier.upgrades,
        pool_index,
    })
}
