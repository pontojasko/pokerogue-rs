pub mod rewards;
use pokerogue_data::{
    BattleStat, Element, Move, MoveEffect, MoveTarget, Species, TOWN_ENCOUNTER_POOLS,
    TOWN_TIER_WEIGHTS, level_moves_for, species_by_numeric_id, total_exp_for_level,
    type_effectiveness,
};
use std::ops::{Deref, DerefMut};

const FRIENDSHIP_GAIN_FROM_BATTLE: u16 = 3;

/// Original switch/faint exit path. Only Baton Pass transfers stat stages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldExitKind {
    InitialSwitch,
    Switch,
    ForceSwitch,
    BatonPass,
    ShedTail,
    Faint,
    Return,
}

/// Pokémon gender shown by the client. Gender generation stays on the same
/// seeded draw already consumed by Town encounter construction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Gender {
    #[default]
    Genderless,
    Male,
    Female,
}

impl Gender {
    pub const fn symbol(self) -> Option<&'static str> {
        match self {
            Self::Genderless => None,
            Self::Male => Some("♂"),
            Self::Female => Some("♀"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Battler {
    pub species: Species,
    pub gender: Gender,
    pub level: u8,
    pub exp: u32,
    pub friendship: u8,
    /// Per-Pokémon Soothe Bell held-item stack, capped at the source count of 3.
    pub soothe_bell_stacks: u8,
    pub hp: i32,
    pub max_hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub special_attack: i32,
    pub special_defense: i32,
    pub speed: i32,
    pub moves: [Move; 4],
    pub move_count: u8,
    /// Source `Pokemon.usedTMs`, retained even if a move is later replaced.
    pub used_tm_ids: Vec<u16>,
    /// PP spent and source PP-Up points for each modeled move slot.
    pub move_pp_used: [u8; 4],
    pub move_pp_ups: [u8; 4],
    /// Zero for ordinary Pokémon; boss segment state is independent of render/UI.
    pub boss_segments: u8,
    pub boss_segment_index: u8,
    /// One modeled Sitrus Berry held modifier; consumed at the source HP threshold.
    pub has_sitrus_berry: bool,
    stat_stages: [i8; 7],
}

impl Battler {
    pub fn demo<const N: usize>(species: Species, level: u8, moves: [Move; N]) -> Self {
        assert!(
            (1..=4).contains(&N),
            "a Pokémon must have one to four moves"
        );
        let mut slots = [moves[N - 1]; 4];
        slots[..N].copy_from_slice(&moves);
        let mut move_count = N;
        while move_count > 1
            && slots[move_count - 1].numeric_id() == slots[move_count - 2].numeric_id()
        {
            move_count -= 1;
        }
        // Level 5, IV=0 fixture using the original calculateStats formula.
        let [hp, attack, defense, special_attack, special_defense, speed] =
            calculate_base_stats(species, level);
        Self {
            species,
            gender: species_by_numeric_id(species.numeric_id()).map_or(Gender::Male, |data| {
                if data.male_percent.is_some() {
                    Gender::Male
                } else {
                    Gender::Genderless
                }
            }),
            level,
            exp: total_exp_for_level(level, species.growth_rate()),
            friendship: species.base_friendship(),
            soothe_bell_stacks: 0,
            hp,
            max_hp: hp,
            attack,
            defense,
            special_attack,
            special_defense,
            speed,
            moves: slots,
            move_count: move_count as u8,
            used_tm_ids: Vec::new(),
            move_pp_used: [0; 4],
            move_pp_ups: [0; 4],
            boss_segments: 0,
            boss_segment_index: 0,
            has_sitrus_berry: false,
            stat_stages: [0; 7],
        }
    }

    pub fn grant_exp(&mut self, amount: u32) -> Vec<u8> {
        self.grant_exp_capped(amount, 100)
    }

    /// Match `Pokemon.addExp` at a caller-supplied source level cap.
    pub fn grant_exp_capped(&mut self, amount: u32, max_exp_level: u8) -> Vec<u8> {
        let max_exp_level = max_exp_level.clamp(1, 100);
        let initial_exp = self.exp;
        self.exp = self.exp.saturating_add(amount);
        let mut gained = Vec::new();
        while self.level < max_exp_level
            && self.exp >= total_exp_for_level(self.level + 1, self.species.growth_rate())
        {
            let old_max_hp = self.max_hp;
            let old_hp = self.hp;
            self.level += 1;
            let [hp, attack, defense, special_attack, special_defense, speed] =
                calculate_base_stats(self.species, self.level);
            self.max_hp = hp;
            self.attack = attack;
            self.defense = defense;
            self.special_attack = special_attack;
            self.special_defense = special_defense;
            self.speed = speed;
            self.hp = if old_hp <= 0 {
                0
            } else {
                (old_hp + (hp - old_max_hp).max(0)).min(hp)
            };
            gained.push(self.level);
        }
        if self.level >= max_exp_level {
            self.exp =
                total_exp_for_level(max_exp_level, self.species.growth_rate()).max(initial_exp);
        }
        gained
    }

    /// Apply the modeled Rare Candy level increment and personal friendship.
    pub fn rare_candy_level_up(&mut self) -> bool {
        if self.level >= 100 {
            return false;
        }
        let next_level_exp = total_exp_for_level(self.level + 1, self.species.growth_rate());
        let leveled = !self
            .grant_exp(next_level_exp.saturating_sub(self.exp))
            .is_empty();
        if leveled {
            self.add_rare_candy_friendship();
        }
        leveled
    }

    /// Source addFriendship(6, true), excluding friendship-boosting modifiers
    /// and starter-candy side effects which are not modeled by MiniRun.
    fn add_rare_candy_friendship(&mut self) {
        self.add_friendship(6, true);
    }

    fn add_friendship(&mut self, base_gain: u16, rare_candy_cap: bool) {
        if base_gain == 0 {
            return;
        }
        let gain = (f64::from(base_gain) * (1.0 + 0.5 * f64::from(self.soothe_bell_stacks))).floor()
            as u16;
        let current = u16::from(self.friendship);
        let next = current + gain;
        let final_value = if rare_candy_cap && next > 200 {
            200.max(current)
        } else {
            next.min(255)
        };
        self.friendship = final_value as u8;
    }

    /// The current encounter projection pads a one-move moveset by repeating
    /// slot zero. It is a compatibility storage detail, not a second move.
    pub fn move_count(&self) -> usize {
        usize::from(self.move_count)
    }

    /// Apply the empty-slot branch of the pinned `LearnMovePhase` flow.
    /// Full movesets require the source replacement decision and are rejected.
    pub fn learn_move(&mut self, movement: Move) -> bool {
        let count = self.move_count();
        if count >= self.moves.len()
            || self.moves[..count]
                .iter()
                .any(|known| known.numeric_id() == movement.numeric_id())
        {
            return false;
        }
        self.moves[count] = movement;
        self.move_pp_used[count] = 0;
        self.move_pp_ups[count] = 0;
        self.move_count += 1;
        for index in count + 1..self.moves.len() {
            self.moves[index] = movement;
            self.move_pp_used[index] = 0;
            self.move_pp_ups[index] = 0;
        }
        true
    }

    /// Apply only level-up moves that the source `LearnMovePhase` would add
    /// automatically to an empty slot. Full movesets need a player choice.
    fn learn_level_moves(&mut self, gained_levels: &[u8]) -> (Vec<u16>, Vec<u16>, Vec<u16>) {
        let (Some(first_level), Some(last_level)) = (
            gained_levels.first().copied(),
            gained_levels.last().copied(),
        ) else {
            return (Vec::new(), Vec::new(), Vec::new());
        };
        if last_level >= 100 {
            return (Vec::new(), Vec::new(), Vec::new());
        }
        let species_id = self.species.numeric_id();
        // LevelUpPhase schedules only non-prevolution moves as LearnMovePhase;
        // prevolution entries become a separate informational message.
        let resolved =
            pokerogue_data::level_moves_with_prevolutions(species_id, first_level, last_level);
        let direct: Vec<_> = resolved
            .iter()
            .filter(|entry| entry.source == pokerogue_data::LevelMoveSource::Direct)
            .map(|entry| entry.move_numeric_id)
            .filter(|move_id| {
                !self.moves[..self.move_count()]
                    .iter()
                    .any(|known| known.numeric_id() == *move_id)
            })
            .collect();
        let candidates: Vec<_> = direct
            .into_iter()
            .filter(|move_id| {
                pokerogue_data::move_by_numeric_id(*move_id)
                    .is_some_and(|movement| !movement.is_unimplemented)
            })
            .collect();
        let mut learned = Vec::new();
        let mut pending = Vec::new();
        for (index, move_id) in candidates.iter().copied().enumerate() {
            if !self.learn_move(Move::Imported(move_id)) {
                // The source pauses for a replacement choice and has already
                // queued the remaining LearnMovePhase entries.
                pending.extend_from_slice(&candidates[index..]);
                break;
            }
            learned.push(move_id);
        }
        let prevo_moves = resolved
            .iter()
            .filter(|entry| entry.source == pokerogue_data::LevelMoveSource::Prevolution)
            .map(|entry| entry.move_numeric_id)
            .collect();
        (learned, pending, prevo_moves)
    }

    /// Replace one move after the source full-moveset choice. PP Ups reset
    /// because the replacement creates a fresh `PokemonMove` in the source.
    pub fn replace_move(&mut self, index: usize, movement: Move) -> bool {
        if self.move_count() != self.moves.len()
            || index >= self.moves.len()
            || self.moves[..self.move_count()]
                .iter()
                .any(|known| known.numeric_id() == movement.numeric_id())
        {
            return false;
        }
        self.moves[index] = movement;
        self.move_pp_used[index] = 0;
        self.move_pp_ups[index] = 0;
        true
    }

    pub fn configure_boss_segments(&mut self, segments: u8) {
        self.boss_segments = segments;
        self.boss_segment_index = segments.saturating_sub(1);
    }

    /// Applies source-segmented damage and returns the actual HP removed.
    pub fn apply_damage(&mut self, damage: i32, min_segment_index: u32) -> i32 {
        self.apply_damage_inner(damage, min_segment_index, None, 0)
            .0
    }

    /// Apply move damage and resolve source wild-boss stat boosts immediately
    /// after the matching segment-clear calculation, consuming the battle RNG.
    pub fn apply_damage_with_rng(
        &mut self,
        damage: i32,
        min_segment_index: u32,
        rng: &mut BattleRng,
        turn: u32,
    ) -> (i32, Vec<(BattleStat, u8)>) {
        self.apply_damage_inner(damage, min_segment_index, Some(rng), turn)
    }

    fn apply_damage_inner(
        &mut self,
        damage: i32,
        min_segment_index: u32,
        mut rng: Option<&mut BattleRng>,
        turn: u32,
    ) -> (i32, Vec<(BattleStat, u8)>) {
        if self.hp <= 0 || damage <= 0 {
            return (0, Vec::new());
        }
        let mut actual = damage.min(self.hp);
        let old_segment_index = self.boss_segment_index;
        let mut cleared_segment_index = u32::from(old_segment_index).saturating_add(1);
        if self.boss_segments > 0 {
            let (segmented, cleared) = calculate_boss_segment_damage(
                actual,
                self.hp,
                f64::from(self.max_hp) / f64::from(self.boss_segments),
                min_segment_index,
                Some(u32::from(self.boss_segment_index)),
            );
            actual = segmented.min(self.hp);
            cleared_segment_index = cleared;
        }
        self.hp -= actual;
        let boosts =
            if self.boss_segments > 0 && cleared_segment_index <= u32::from(old_segment_index) {
                if let Some(battle_rng) = rng.as_deref_mut() {
                    self.apply_boss_segment_cleared(cleared_segment_index, battle_rng, turn)
                } else {
                    self.advance_boss_segments(cleared_segment_index);
                    Vec::new()
                }
            } else {
                Vec::new()
            };
        (actual, boosts)
    }

    /// Resolve the source `EnemyPokemon.handleBossSegmentCleared` behavior for
    /// a previously calculated segment index. Phase scheduling/ability hooks
    /// are outside this slice; wild-boss stages and their weighted RNG are local.
    pub fn apply_boss_segment_cleared(
        &mut self,
        segment_index: u32,
        rng: &mut BattleRng,
        turn: u32,
    ) -> Vec<(BattleStat, u8)> {
        if self.boss_segments == 0 {
            return Vec::new();
        }
        let original_stages = [
            self.stat_stage(BattleStat::Attack),
            self.stat_stage(BattleStat::Defense),
            self.stat_stage(BattleStat::SpecialAttack),
            self.stat_stage(BattleStat::SpecialDefense),
            self.stat_stage(BattleStat::Speed),
        ];
        let mut boosts = Vec::new();
        while self.boss_segment_index > 0
            && segment_index.saturating_sub(1) < u32::from(self.boss_segment_index)
        {
            self.boss_segment_index -= 1;
            let stats = [
                (BattleStat::Attack, self.attack),
                (BattleStat::Defense, self.defense),
                (BattleStat::SpecialAttack, self.special_attack),
                (BattleStat::SpecialDefense, self.special_defense),
                (BattleStat::Speed, self.speed),
            ];
            let mut eligible = Vec::new();
            for (index, (stat, value)) in stats.into_iter().enumerate() {
                let pending = boosts
                    .iter()
                    .filter(|(changed, _)| *changed == stat)
                    .map(|(_, stages)| i16::from(*stages))
                    .sum::<i16>();
                if i16::from(original_stages[index]) + pending < 6 {
                    eligible.push((stat, value.max(0) as u32));
                }
            }
            let total_weight = eligible.iter().map(|(_, weight)| *weight).sum::<u32>();
            if total_weight == 0 {
                continue;
            }
            let mut roll = rng.rand_int(turn, total_weight, 0) as u32;
            let mut selected = None;
            for (stat, weight) in eligible {
                if roll < weight {
                    selected = Some(stat);
                    break;
                }
                roll -= weight;
            }
            let Some(stat) = selected else { continue };
            let mut stages = 1;
            if self.boss_segments >= 3 && self.boss_segment_index == 0 {
                stages += 1;
            }
            if self.boss_segments >= 5 && self.boss_segment_index == 1 {
                stages += 1;
            }
            self.set_stat_stage(stat, self.stat_stage(stat).saturating_add(stages as i8));
            boosts.push((stat, stages));
        }
        boosts
    }

    fn advance_boss_segments(&mut self, segment_index: u32) {
        while self.boss_segment_index > 0
            && segment_index.saturating_sub(1) < u32::from(self.boss_segment_index)
        {
            self.boss_segment_index -= 1;
        }
    }

    fn maybe_consume_sitrus_berry(&mut self) -> Option<i32> {
        if !self.has_sitrus_berry || self.hp <= 0 || self.hp.saturating_mul(2) >= self.max_hp {
            return None;
        }
        let healed = ((f64::from(self.max_hp) / 4.0).floor() as i32).max(1);
        let actual = (self.hp + healed).min(self.max_hp) - self.hp;
        self.hp += actual;
        self.has_sitrus_berry = false;
        Some(actual)
    }

    fn restore_checkpoint(
        species_id: u16,
        level: u8,
        hp: i32,
        exp: u32,
        moves: [u16; 2],
        move_pp_used: [u8; 2],
        move_pp_ups: [u8; 2],
        stages: [i8; 7],
        has_sitrus_berry: bool,
    ) -> Option<Self> {
        let species = Species::from_numeric_id(species_id)?;
        if moves
            .iter()
            .any(|id| pokerogue_data::move_by_numeric_id(*id).is_none())
        {
            return None;
        }
        let mut battler = Self::demo(species, level, moves.map(Move::Imported));
        if hp < 0
            || hp > battler.max_hp
            || move_pp_ups.iter().any(|up| *up > 3)
            || move_pp_ups.iter().enumerate().any(|(index, up)| {
                (*up > 0 && battler.moves[index].pp() < 5)
                    || move_pp_used[index] > battler.move_pp_max(index).unwrap_or(0)
            })
            || stages.iter().any(|stage| !(-6..=6).contains(stage))
        {
            return None;
        }
        battler.hp = hp;
        battler.exp = exp;
        battler.move_pp_used[..2].copy_from_slice(&move_pp_used);
        battler.move_pp_ups[..2].copy_from_slice(&move_pp_ups);
        battler.stat_stages = stages;
        battler.has_sitrus_berry = has_sitrus_berry;
        Some(battler)
    }

    fn restore_extended_moveset(
        &mut self,
        count: u8,
        ids: [u16; 4],
        pp_used: [u8; 4],
        pp_ups: [u8; 4],
    ) -> bool {
        let count = usize::from(count);
        if !(1..=4).contains(&count) || pp_ups.iter().any(|up| *up > 3) {
            return false;
        }
        let Some(moves) = ids
            .map(|id| pokerogue_data::move_by_numeric_id(id).map(|_| Move::Imported(id)))
            .into_iter()
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
        if moves[count..]
            .iter()
            .any(|movement| movement.numeric_id() != moves[count - 1].numeric_id())
        {
            return false;
        }
        for index in 0..count {
            if pp_ups[index] > 0 && moves[index].pp() < 5 {
                return false;
            }
            let max_pp = moves[index].pp() + pp_ups[index] * (moves[index].pp() / 5).max(1);
            if pp_used[index] > max_pp {
                return false;
            }
        }
        let Ok(moves) = moves.try_into() else {
            return false;
        };
        self.moves = moves;
        self.move_count = count as u8;
        self.move_pp_used = pp_used;
        self.move_pp_ups = pp_ups;
        true
    }

    pub fn stat_stage(&self, stat: BattleStat) -> i8 {
        self.stat_stages[stat_stage_index(stat)]
    }

    pub fn move_pp_remaining(&self, index: usize) -> Option<u8> {
        if index >= self.move_count() {
            return None;
        }
        self.moves.get(index)?;
        Some(
            self.move_pp_max(index)?
                .saturating_sub(self.move_pp_used[index]),
        )
    }

    /// `PokemonMove.getMovePp`: base PP plus PP-Up points times
    /// max(floor(base PP / 5), 1). The current slice has no PP override.
    pub fn move_pp_max(&self, index: usize) -> Option<u8> {
        if index >= self.move_count() {
            return None;
        }
        let movement = *self.moves.get(index)?;
        Some(movement.pp() + self.move_pp_ups[index] * (movement.pp() / 5).max(1))
    }

    pub fn can_apply_pp_up(&self, index: usize) -> bool {
        index < self.move_count() && self.moves[index].pp() >= 5 && self.move_pp_ups[index] < 3
    }

    pub fn consume_move_pp(&mut self, index: usize, count: u8) -> bool {
        if index >= self.move_count() {
            return false;
        }
        let Some(movement) = self.moves.get(index).copied() else {
            return false;
        };
        self.move_pp_used[index] = self.move_pp_used[index]
            .saturating_add(count)
            .min(self.move_pp_max(index).unwrap_or(movement.pp()));
        true
    }

    pub fn restore_move_pp(&mut self, index: usize, points: Option<u8>) -> bool {
        let Some(used) = self.move_pp_used.get_mut(index) else {
            return false;
        };
        *used = points.map_or(0, |points| used.saturating_sub(points));
        true
    }

    pub fn restore_all_move_pp(&mut self, points: Option<u8>) {
        for used in &mut self.move_pp_used {
            *used = points.map_or(0, |points| used.saturating_sub(points));
        }
    }

    pub fn set_stat_stage(&mut self, stat: BattleStat, stage: i8) -> i8 {
        let index = stat_stage_index(stat);
        let old = self.stat_stages[index];
        self.stat_stages[index] = stage.clamp(-6, 6);
        self.stat_stages[index] - old
    }

    pub fn change_stat_stage(&mut self, stat: BattleStat, change: i8) -> i8 {
        self.set_stat_stage(stat, self.stat_stage(stat).saturating_add(change))
    }

    pub fn reset_stat_stages(&mut self) {
        self.stat_stages = [0; 7];
    }

    /// Apply the reset performed directly by `leaveField`; deferred switch kinds
    /// retain summon data until `finish_field_exit`, matching SwitchSummonPhase.
    /// Baton Pass returns the modeled stages for the receiving battler to inherit.
    pub fn leave_field(&mut self, reason: FieldExitKind) -> Option<[i8; 7]> {
        match reason {
            FieldExitKind::Switch | FieldExitKind::Faint | FieldExitKind::Return => {
                self.reset_stat_stages();
                None
            }
            FieldExitKind::BatonPass => Some(self.stat_stages),
            FieldExitKind::InitialSwitch | FieldExitKind::ForceSwitch | FieldExitKind::ShedTail => {
                None
            }
        }
    }

    /// Complete a deferred switch exit after any Baton Pass transfer has occurred.
    pub fn finish_field_exit(&mut self) {
        self.reset_stat_stages();
    }

    /// Replace this battler's stages with a Baton Pass snapshot from its predecessor.
    pub fn receive_passed_stages(&mut self, stages: [i8; 7]) {
        self.stat_stages = stages.map(|stage| stage.clamp(-6, 6));
    }

    pub fn effective_speed(&self) -> i32 {
        self.effective_speed_with_bonus(0.0).floor() as i32
    }

    pub fn effective_speed_with_bonus(&self, multiplier_bonus: f64) -> f64 {
        let stage = self.stat_stage(BattleStat::Speed).clamp(-6, 6) as i32;
        let multiplier = (2 + stage).max(2) as f64 / (2 - stage).max(2) as f64;
        (self.speed as f64 * (multiplier + multiplier_bonus).min(4.0)).max(1.0)
    }
}

fn calculate_base_stats(species: Species, level: u8) -> [i32; 6] {
    let level = level as i32;
    let calc = |base: i32| ((2 * base * level) / 100 + 5).max(1);
    [
        ((2 * species.base_hp() * level) / 100 + level + 10).max(1),
        calc(species.base_attack()),
        calc(species.base_defense()),
        calc(species.base_special_attack()),
        calc(species.base_special_defense()),
        calc(species.base_speed()),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwitchEvent {
    OutgoingLeftField { slot: usize },
    OutgoingStagesReset { slot: usize },
    IncomingStagesReset { slot: usize },
    IncomingEnteredField { slot: usize },
    SwitchCompleted { outgoing: usize, active: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwitchError {
    SlotOutOfRange,
    AlreadyActive,
    Fainted,
    BattleOver,
    ReplacementNotPending,
    MandatoryReplacement,
}

pub const PLAYER_PARTY_MAX_SIZE: usize = 6;
pub const INITIAL_POKEBALL_COUNT: u8 = 5;
pub const MAX_POKEBALL_COUNT: u8 = 99;
pub const MAX_RUN_MONEY: u64 = 9_007_199_254_740_991;

/// Source `BattleScene.getWaveMoneyAmount` for Classic wave money.
pub fn classic_wave_money_amount(wave: u32, multiplier: f64) -> u64 {
    let wave = wave.max(1);
    let wave_set_index = wave.div_ceil(10) - 1;
    let wave_in_set = (wave - 1) % 10 + 1;
    let money = (f64::from(wave_set_index + 1) + (0.75 + f64::from(wave_in_set) / 10.0)) * 100.0;
    let money = money.powf(1.0 + 0.005 * f64::from(wave_set_index)) * multiplier;
    ((money / 10.0).floor() * 10.0).min(MAX_RUN_MONEY as f64) as u64
}

/// Mirrors `MoneyMultiplierModifier.apply` for the modeled Amulet Coin stack.
pub fn apply_amulet_coin(amount: u64, stacks: u8) -> u64 {
    let amount =
        amount.saturating_add((amount as f64 * 0.2 * f64::from(stacks.min(5))).floor() as u64);
    amount.min(MAX_RUN_MONEY)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeamError {
    PartyFull,
}

/// Minimal local party representation. Field access through `Deref` addresses the active member.
#[derive(Clone, Debug)]
pub struct Team {
    members: Vec<Battler>,
    active_slot: usize,
}

impl Team {
    pub fn new(active: Battler, reserve: Battler) -> Self {
        Self {
            members: vec![active, reserve],
            active_slot: 0,
        }
    }

    /// Construct the source replay's selected starter party before a reserve is added.
    pub fn solo(active: Battler) -> Self {
        Self {
            members: vec![active],
            active_slot: 0,
        }
    }

    fn from_checkpoint(members: Vec<Battler>, active_slot: usize) -> Option<Self> {
        (members.len() >= 2
            && members.len() <= PLAYER_PARTY_MAX_SIZE
            && active_slot < members.len())
        .then_some(Self {
            members,
            active_slot,
        })
    }

    pub fn len(&self) -> usize {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    pub fn add_member(&mut self, member: Battler) -> Result<usize, TeamError> {
        if self.members.len() >= PLAYER_PARTY_MAX_SIZE {
            return Err(TeamError::PartyFull);
        }
        let slot = self.members.len();
        self.members.push(member);
        Ok(slot)
    }

    pub fn next_switch_target(&self) -> Option<usize> {
        (1..self.members.len())
            .map(|offset| (self.active_slot + offset) % self.members.len())
            .find(|slot| self.members[*slot].hp > 0)
    }

    pub fn active_slot(&self) -> usize {
        self.active_slot
    }

    pub fn member(&self, slot: usize) -> Option<&Battler> {
        self.members.get(slot)
    }

    pub fn member_mut(&mut self, slot: usize) -> Option<&mut Battler> {
        self.members.get_mut(slot)
    }

    pub fn reserve(&self) -> &Battler {
        &self.members[(0..self.members.len())
            .find(|slot| *slot != self.active_slot)
            .expect("Team::new always creates an initial reserve")]
    }

    pub fn can_switch_to(&self, slot: usize) -> Result<(), SwitchError> {
        let Some(incoming) = self.members.get(slot) else {
            return Err(SwitchError::SlotOutOfRange);
        };
        if incoming.hp <= 0 {
            return Err(SwitchError::Fainted);
        }
        if slot == self.active_slot {
            return Err(SwitchError::AlreadyActive);
        }
        Ok(())
    }

    /// Switch to a living member without consuming battle RNG.
    pub fn switch_to(&mut self, slot: usize) -> Result<[SwitchEvent; 5], SwitchError> {
        self.can_switch_to(slot)?;

        let outgoing_slot = self.active_slot;
        self.members[outgoing_slot].leave_field(FieldExitKind::Switch);
        self.members[slot].reset_stat_stages();
        self.active_slot = slot;
        self.members[outgoing_slot].finish_field_exit();

        Ok([
            SwitchEvent::OutgoingLeftField {
                slot: outgoing_slot,
            },
            SwitchEvent::OutgoingStagesReset {
                slot: outgoing_slot,
            },
            SwitchEvent::IncomingStagesReset { slot },
            SwitchEvent::IncomingEnteredField { slot },
            SwitchEvent::SwitchCompleted {
                outgoing: outgoing_slot,
                active: slot,
            },
        ])
    }
}

impl Deref for Team {
    type Target = Battler;

    fn deref(&self) -> &Self::Target {
        &self.members[self.active_slot]
    }
}

impl DerefMut for Team {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.members[self.active_slot]
    }
}

const fn stat_stage_index(stat: BattleStat) -> usize {
    match stat {
        BattleStat::Accuracy => 0,
        BattleStat::Evasion => 1,
        BattleStat::Speed => 2,
        BattleStat::Defense => 3,
        BattleStat::Attack => 4,
        BattleStat::SpecialAttack => 5,
        BattleStat::SpecialDefense => 6,
    }
}

const fn temp_booster_index(stat: BattleStat) -> Option<usize> {
    match stat {
        BattleStat::Attack => Some(0),
        BattleStat::Defense => Some(1),
        BattleStat::SpecialAttack => Some(2),
        BattleStat::SpecialDefense => Some(3),
        BattleStat::Speed => Some(4),
        BattleStat::Accuracy => Some(5),
        BattleStat::Evasion => None,
    }
}

const fn reward_temp_booster_index(reward: rewards::Reward) -> Option<usize> {
    match reward {
        rewards::Reward::TempAttack => Some(0),
        rewards::Reward::TempDefense => Some(1),
        rewards::Reward::TempSpecialAttack => Some(2),
        rewards::Reward::TempSpecialDefense => Some(3),
        rewards::Reward::TempSpeed => Some(4),
        rewards::Reward::TempAccuracy => Some(5),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    PlayerWin,
    PlayerLoss,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BattleActor {
    Player,
    Enemy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageEffectiveness {
    Normal,
    SuperEffective,
    NotVeryEffective,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BattleEvent {
    MoveExecuted {
        actor: BattleActor,
        move_: Move,
    },
    CriticalHit {
        actor: BattleActor,
        move_id: u16,
    },
    /// Presentation cue derived from damage that has already been resolved.
    DamageApplied {
        target: BattleActor,
        amount: i32,
        critical: bool,
        effectiveness: DamageEffectiveness,
    },
    ExperienceGained {
        party_slot: usize,
        amount: u32,
        previous_exp: u32,
        current_exp: u32,
        previous_level: u8,
        current_level: u8,
    },
    BossSegmentStatBoosted {
        stat: BattleStat,
        stages: u8,
    },
    StatStagesReset {
        slot: usize,
    },
    PokemonFainted {
        slot: usize,
    },
    ReplacementRequired {
        fainted_slot: usize,
    },
    Switch(SwitchEvent),
    TurnEnded {
        turn: u32,
    },
    BattleEnded(Outcome),
    SitrusBerryConsumed {
        slot: usize,
        hp_restored: i32,
    },
    CaptureAttemptResolved {
        species_id: u16,
        caught: bool,
        party_slot: Option<usize>,
    },
    MoveLearned {
        party_slot: usize,
        move_id: u16,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerAction {
    UseMove(usize),
    Switch(usize),
    ThrowPokeball,
    ThrowGreatBall,
    ThrowUltraBall,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnActionError {
    InvalidMoveIndex,
    NoPp,
    Switch(SwitchError),
    BattleOver,
    UnsupportedEnemyMoves,
    MandatoryReplacement,
    PartyFull,
    BossNotCatchable,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnemyAiDecision {
    pub selected_move: Move,
    pub ko_damage: Vec<(Move, i32)>,
    pub candidates: Vec<Move>,
    pub scores: Vec<(Move, f64)>,
    pub random_rolls: Vec<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitResult {
    Hit,
    Miss,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pokeball {
    Poke,
    Great,
    Ultra,
    Rogue,
    Master,
    Luxury,
}

impl Pokeball {
    pub fn catch_multiplier(self) -> f64 {
        match self {
            Self::Poke | Self::Luxury => 1.0,
            Self::Great => 1.5,
            Self::Ultra => 2.0,
            Self::Rogue => 3.0,
            Self::Master => -1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CaptureAttempt {
    pub modified_catch_rate: i32,
    pub shake_probability: f64,
    pub critical: bool,
    pub shake_rolls: Vec<i32>,
    pub caught: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureError {
    InvalidHp,
}

/// Resolve the Gen 6 capture calculation used by AttemptCapturePhase for an ordinary target.
/// Status/shiny bonuses are explicit inputs; dex catch bonuses are supplied as critical chance.
pub fn attempt_capture(
    rng: &mut BattleRng,
    turn: u32,
    catch_rate: u8,
    hp: i32,
    max_hp: i32,
    ball: Pokeball,
    status_multiplier: f64,
    shiny_multiplier: f64,
    critical_capture_chance: i32,
) -> Result<CaptureAttempt, CaptureError> {
    if max_hp <= 0 || hp <= 0 || hp > max_hp {
        return Err(CaptureError::InvalidHp);
    }
    let three_max = 3 * max_hp;
    let two_hp = 2 * hp;
    let modified_rate =
        ((three_max - two_hp) as f64 * f64::from(catch_rate) * ball.catch_multiplier())
            / f64::from(three_max)
            * status_multiplier
            * shiny_multiplier;
    // JavaScript Math.round ties toward +infinity; match it also for Master Ball's negative rate.
    let modified_catch_rate = (modified_rate.floor()
        + if modified_rate - modified_rate.floor() >= 0.5 {
            1.0
        } else {
            0.0
        }) as i32;
    let shake_probability =
        (65536.0 / (255.0 / f64::from(modified_catch_rate)).powf(0.1875)).round();
    // Original phase always rolls critical capture first, including Master Ball throws.
    let critical = rng.rand_int(turn, 256, 0) < critical_capture_chance as i32;
    let mut shake_rolls = Vec::new();
    let caught = if ball.catch_multiplier() == -1.0 || modified_catch_rate >= 255 {
        true
    } else if critical {
        let roll = rng.rand_int(turn, 65536, 0);
        shake_rolls.push(roll);
        (roll as f64) < shake_probability
    } else {
        let mut succeeded = true;
        for _ in 0..4 {
            let roll = rng.rand_int(turn, 65536, 0);
            shake_rolls.push(roll);
            if (roll as f64) >= shake_probability {
                succeeded = false;
                break;
            }
        }
        succeeded
    };
    Ok(CaptureAttempt {
        modified_catch_rate,
        shake_probability,
        critical,
        shake_rolls,
        caught,
    })
}

/// Phaser 4's battle-seeded Alea stream, isolated from the renderer.
#[derive(Clone, Debug)]
pub struct BattleRng {
    battle_seed: Vec<u16>,
    seeded_turn: Option<u32>,
    state: Option<PhaserRandomDataGenerator>,
    draws: Vec<RngDraw>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RngDraw {
    pub turn: u32,
    pub range: u32,
    pub min: i32,
    pub value: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TownEncounterSelection {
    pub tier: &'static str,
    pub tier_roll: u16,
    pub candidates: Vec<(u16, &'static str)>,
    pub selected_index: usize,
    pub species: (u16, &'static str),
    pub rng_draws: Vec<RngDraw>,
}

#[derive(Clone, Debug)]
pub struct TownWildEncounter {
    pub selection: TownEncounterSelection,
    /// The pre-EncounterPhase wave-stream check performed by BattleScene.checkIsDouble.
    pub entry_rng_draws: Vec<RngDraw>,
    pub double: Option<bool>,
    pub level: u8,
    pub boss_segments: u8,
    /// Seed generated by the source `Battle` constructor's seeded randomString.
    pub battle_seed: String,
    pub battle_seed_rng_draws: Vec<TownEntryRngDraw>,
    pub level_random_values: Vec<f64>,
    /// Global RNG draws made while constructing the first wild Pokémon.
    pub construction_rng_draws: Vec<TownEntryRngDraw>,
    pub generated_id: u32,
    pub generated_ivs: [u8; 6],
    pub generated_nature: u8,
    pub generated_gender: Gender,
    pub generated_ability_index: u8,
    pub generated_ability_id: u16,
    pub generated_moves: Vec<u16>,
    pub encounter_rng_trace: Vec<TownEntryRngDraw>,
    pub additional_opponents: Vec<TownGeneratedOpponent>,
    pub is_mystery_encounter: bool,
    pub battler: Battler,
    /// Original level-up moves available at this level; the current battle model uses two slots.
    pub available_level_moves: Vec<&'static str>,
}

#[derive(Clone, Debug)]
pub struct TownGeneratedOpponent {
    pub species_id: u16,
    pub level: u8,
    pub generated_id: u32,
    pub generated_ivs: [u8; 6],
    pub generated_nature: u8,
    pub gender: Gender,
    pub generated_ability_id: u16,
    pub generated_moves: Vec<u16>,
    pub battler: Battler,
}

struct TownGeneratedPokemonData {
    rng_draws: Vec<TownEntryRngDraw>,
    id: u32,
    ivs: [u8; 6],
    nature: u8,
    gender: Gender,
    ability_index: u8,
    ability_id: u16,
    moves: Vec<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TownEntryRngDraw {
    pub range: u64,
    pub min: i64,
    pub value: i64,
}

fn town_entry_rand(
    rng: &mut BattleRng,
    draws: &mut Vec<TownEntryRngDraw>,
    range: u64,
    min: i64,
) -> i64 {
    if range <= 1 {
        return min;
    }
    let value = (rng.rand_float(0) * range as f64).floor() as i64 + min;
    draws.push(TownEntryRngDraw { range, min, value });
    value
}

fn town_entry_shuffle_draws(wave_seed: &[u16], group_count: usize) -> Vec<TownEntryRngDraw> {
    let mut draws = Vec::new();
    if group_count < 2 {
        return draws;
    }
    // `sortInSpeedOrder` shuffles one group per active battler using
    // turn * 1000 + group count as the seed offset. The wave seed, rather
    // than Battle.battleSeed, is the source seed override here.
    let offset = 1_000_u32 + group_count as u32;
    let seed = wave_seed
        .iter()
        .map(|unit| unit.wrapping_add(offset as u16))
        .collect();
    let mut rng = PhaserRandomDataGenerator::sown(seed);
    for upper in (1..group_count).rev() {
        let value = rng.integer_in_range(0, upper as i32);
        draws.push(TownEntryRngDraw {
            range: upper as u64 + 1,
            min: 0,
            value: i64::from(value),
        });
    }
    draws
}

fn select_town_entry_moves(
    rng: &mut BattleRng,
    draws: &mut Vec<TownEntryRngDraw>,
    species: &pokerogue_data::ImportedSpecies,
    level: u8,
) -> Vec<u16> {
    let mut pool: Vec<(u16, u8, f64)> = Vec::new();
    for record in level_moves_for(species.id, level) {
        if pool.iter().any(|(id, _, _)| *id == record.move_numeric_id) {
            continue;
        }
        let Some(movement) = pokerogue_data::move_by_numeric_id(record.move_numeric_id) else {
            continue;
        };
        if movement.is_unimplemented {
            continue;
        }
        let weight = f64::from(record.level + 20);
        pool.push((record.move_numeric_id, record.level, weight));
    }
    if pool.is_empty() {
        return Vec::new();
    }
    // The pinned moveset generator raises adjusted learn weights to 1.6 and
    // scales by 100. For this bounded early-wave case the only damaging move
    // has power 40 and is the maximum-power/STAB candidate, so its adjustment
    // ratio is 1; status weights are unchanged.
    for (_, _, weight) in &mut pool {
        *weight = (*weight).powf(1.6) * 100.0;
    }
    let weighted_choice =
        |pool: &[(u16, u8, f64)], rng: &mut BattleRng, draws: &mut Vec<TownEntryRngDraw>| {
            let total = pool.iter().map(|(_, _, weight)| weight.ceil()).sum::<f64>() as u64;
            if total == 0 {
                return None;
            }
            let roll = town_entry_rand(rng, draws, total, 0) as f64;
            let mut remaining = roll;
            pool.iter().find_map(|(id, _, weight)| {
                let weight = weight.ceil();
                if remaining > weight {
                    remaining -= weight;
                    None
                } else {
                    Some(*id)
                }
            })
        };
    let damaging_stab: Vec<_> = pool
        .iter()
        .filter(|(id, _, _)| {
            let movement = pokerogue_data::move_by_numeric_id(*id).expect("imported move");
            !movement.is_status && movement.element == species.type1
        })
        .cloned()
        .collect();
    let mut selected = Vec::new();
    if let Some(id) = weighted_choice(&damaging_stab, rng, draws) {
        selected.push(id);
        pool.retain(|(candidate, _, _)| *candidate != id);
    }
    while selected.len() < 4 && !pool.is_empty() {
        let has_damaging = pool.iter().any(|(id, _, _)| {
            !pokerogue_data::move_by_numeric_id(*id)
                .expect("imported move")
                .is_status
        });
        let damaging_selected = selected
            .iter()
            .filter(|id| {
                !pokerogue_data::move_by_numeric_id(**id)
                    .expect("imported move")
                    .is_status
            })
            .count();
        let candidates: Vec<_> = if has_damaging && damaging_selected < 2 {
            pool.iter()
                .filter(|(id, _, _)| {
                    !pokerogue_data::move_by_numeric_id(*id)
                        .expect("imported move")
                        .is_status
                })
                .cloned()
                .collect()
        } else {
            pool.clone()
        };
        let Some(id) = weighted_choice(&candidates, rng, draws) else {
            break;
        };
        selected.push(id);
        pool.retain(|(candidate, _, _)| *candidate != id);
    }
    selected
}

fn generate_town_entry_pokemon_data(
    rng: &mut BattleRng,
    species: &pokerogue_data::ImportedSpecies,
    level: u8,
) -> TownGeneratedPokemonData {
    let mut rng_draws = Vec::new();
    let ability1 = species.ability1;
    let ability2 = if species.ability2 == "NONE" {
        species.ability1
    } else {
        species.ability2
    };
    let regular_ability = if ability2 == ability1 {
        0
    } else {
        town_entry_rand(rng, &mut rng_draws, 2, 0) as u8
    };
    let hidden_ability = if species.ability_hidden == "NONE" {
        false
    } else {
        town_entry_rand(rng, &mut rng_draws, 256, 0) == 0
    };
    let ability_index = if hidden_ability { 2 } else { regular_ability };
    let ability_id = match ability_index {
        2 => species.ability_hidden_id,
        1 if species.ability2 == "NONE" => species.ability1_id,
        1 => species.ability2_id,
        _ => species.ability1_id,
    };
    let id = town_entry_rand(rng, &mut rng_draws, 4_294_967_296, 0) as u32;
    let gender = if let Some(male_percent) = species.male_percent {
        // Source `generateGender()` uses a direct seeded fraction.
        if rng.rand_float(0) * 100.0 <= male_percent {
            Gender::Male
        } else {
            Gender::Female
        }
    } else {
        Gender::Genderless
    };
    let nature = town_entry_rand(rng, &mut rng_draws, 25, 0) as u8;
    let ivs = [
        ((id >> 25) & 0x1f) as u8,
        ((id >> 20) & 0x1f) as u8,
        ((id >> 15) & 0x1f) as u8,
        ((id >> 10) & 0x1f) as u8,
        ((id >> 5) & 0x1f) as u8,
        (id & 0x1f) as u8,
    ];
    let type_count = 1 + usize::from(species.type2.is_some_and(|kind| kind != species.type1));
    town_entry_rand(rng, &mut rng_draws, type_count as u64, 0);
    let moves = select_town_entry_moves(rng, &mut rng_draws, species, level);
    TownGeneratedPokemonData {
        rng_draws,
        id,
        ivs,
        nature,
        gender,
        ability_index,
        ability_id,
        moves,
    }
}

fn town_entry_stats(
    species: &pokerogue_data::ImportedSpecies,
    level: u8,
    ivs: [u8; 6],
    nature: u8,
) -> [i32; 6] {
    let level = i32::from(level);
    let hp = ((2 * species.base_hp + i32::from(ivs[0])) * level / 100) + level + 10;
    let bases = [
        species.base_attack,
        species.base_defense,
        species.base_special_attack,
        species.base_special_defense,
        species.base_speed,
    ];
    // Nature grid order from original Nature enum: +Atk, +Def, +Speed, +SpA, +SpD.
    const STAT_ORDER: [usize; 5] = [1, 2, 5, 3, 4];
    let raised = STAT_ORDER[usize::from(nature / 5).min(4)];
    let lowered = STAT_ORDER[usize::from(nature % 5).min(4)];
    let mut stats = [hp, 0, 0, 0, 0, 0];
    for (index, base) in bases.into_iter().enumerate() {
        let stat_index = index + 1;
        let raw = ((2 * base + i32::from(ivs[stat_index])) * level / 100) + 5;
        let multiplier = if stat_index == raised && stat_index != lowered {
            1.1
        } else if stat_index == lowered && stat_index != raised {
            0.9
        } else {
            1.0
        };
        stats[stat_index] = if multiplier > 1.0 {
            (f64::from(raw) * multiplier).ceil() as i32
        } else {
            (f64::from(raw) * multiplier).floor() as i32
        };
    }
    stats
}

fn town_entry_battler<const N: usize>(
    species_id: u16,
    level: u8,
    moves: [Move; N],
    ivs: [u8; 6],
    nature: u8,
    boss_segments: u8,
) -> Battler {
    let species = Species::Imported(species_id);
    let mut battler = Battler::demo(species, level, moves);
    let data = species_by_numeric_id(species_id).expect("generated Town species");
    let [hp, attack, defense, special_attack, special_defense, speed] =
        town_entry_stats(data, level, ivs, nature);
    battler.max_hp = hp;
    battler.hp = hp;
    battler.attack = attack;
    battler.defense = defense;
    battler.special_attack = special_attack;
    battler.special_defense = special_defense;
    battler.speed = speed;
    battler.configure_boss_segments(boss_segments);
    battler
}

fn town_replay_starter() -> Battler {
    // Captured from the pinned original's starter grid for M12TownWave1Replay6.
    town_entry_battler(
        1,
        5,
        [Move::Imported(33), Move::Imported(45), Move::Imported(22)],
        [15; 6],
        6,
        0,
    )
}

pub fn classic_boss_level(
    wave: u32,
    wave_difficulty: u32,
    random_real: f64,
    wave_final: bool,
) -> u8 {
    let w = f64::from(wave_difficulty);
    let base = 1.0 + w / 2.0 + (w / 25.0).powi(2);
    let ret = (base * 1.2).floor() as u32;
    if wave_final || wave % 250 == 0 {
        return ret.div_ceil(25).saturating_mul(25) as u8;
    }
    // JavaScript Math.round resolves negative half ties toward positive infinity.
    let offset = (random_real * f64::from(wave_difficulty / 10) + 0.5).floor() as i32;
    (ret as i32 + offset).max(1) as u8
}

pub fn classic_boss_segments(wave: u32, level: u8, base_total: u16) -> u8 {
    2 + u8::from(level >= 100)
        + u8::from(base_total >= 670)
        + (wave / 250).min(u32::from(u8::MAX)) as u8
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncounterSelectionError {
    InvalidTimeOfDay,
    MissingTier,
    EmptyCommonPool,
}

/// Bounded first-wave Town path matching Arena.randomSpecies with no boss,
/// override, forced daily tier, luck modifier, legendary reroll, or form swap.
pub fn select_town_encounter(
    seed: &str,
    time_of_day: &str,
) -> Result<TownEncounterSelection, EncounterSelectionError> {
    if !["DAWN", "DAY", "DUSK", "NIGHT"].contains(&time_of_day) {
        return Err(EncounterSelectionError::InvalidTimeOfDay);
    }
    let mut rng = BattleRng::new(seed);
    select_town_encounter_with_rng(&mut rng, time_of_day)
}

fn select_town_encounter_with_rng(
    rng: &mut BattleRng,
    time_of_day: &str,
) -> Result<TownEncounterSelection, EncounterSelectionError> {
    if !["DAWN", "DAY", "DUSK", "NIGHT"].contains(&time_of_day) {
        return Err(EncounterSelectionError::InvalidTimeOfDay);
    }
    let tier_roll = rng.rand_int(0, 512, 0) as u16;
    let mut tier = normal_tier_for_roll(tier_roll).ok_or(EncounterSelectionError::MissingTier)?;
    let candidates = loop {
        let pool = town_pool(tier, time_of_day);
        if !pool.is_empty() {
            break pool;
        }
        if tier == 0 {
            return Err(EncounterSelectionError::EmptyCommonPool);
        }
        tier -= 1;
    };
    let selected_index = rng.rand_int(0, candidates.len() as u32, 0) as usize;
    let species = candidates[selected_index];
    Ok(TownEncounterSelection {
        tier: TOWN_TIER_WEIGHTS
            .iter()
            .find(|w| !w.boss_table && w.tier_id == tier)
            .expect("imported Town tier")
            .tier,
        tier_roll,
        candidates,
        selected_index,
        species,
        rng_draws: rng.draws().to_vec(),
    })
}

/// Builds a Town encounter for the controlled single-battle/no-Mystery-Encounter slice.
/// Level and Battle seed use the `waveIndex << 3` constructor-offset stream, while Arena
/// selection uses the restored wave stream. Zero IVs/neutral nature and no rerolls remain
/// explicit projections; normal double/Mystery Encounter selection is not represented.
pub fn generate_town_wild_encounter(
    seed: &str,
    time_of_day: &str,
) -> Result<TownWildEncounter, EncounterSelectionError> {
    generate_town_wild_encounter_for_wave(seed, time_of_day, 1)
}

pub fn generate_town_wild_encounter_for_wave(
    seed: &str,
    time_of_day: &str,
    wave: u32,
) -> Result<TownWildEncounter, EncounterSelectionError> {
    generate_town_wild_encounter_for_wave_with_entry_check(seed, time_of_day, wave, false)
}

/// Executes the isolated `Arena.randomSpecies` projection without the earlier
/// `BattleScene.checkIsDouble` draw. Kept for method-level source fixtures only.
pub fn generate_town_wild_encounter_for_wave_isolated(
    seed: &str,
    time_of_day: &str,
    wave: u32,
) -> Result<TownWildEncounter, EncounterSelectionError> {
    generate_town_wild_encounter_for_wave_with_entry_check(seed, time_of_day, wave, false)
}

/// Builds a bounded Classic Town wave entry with the wave-stream
/// `BattleScene.checkIsDouble` draw before EncounterPhase species selection.
/// Final-wave, modifier/ability, Mystery Encounter, and opponent construction
/// behavior remain outside this projection.
pub fn generate_town_wild_encounter_for_classic_entry(
    seed: &str,
    time_of_day: &str,
    wave: u32,
) -> Result<TownWildEncounter, EncounterSelectionError> {
    generate_town_wild_encounter_for_wave_with_entry_check(seed, time_of_day, wave, true)
}

fn generate_town_wild_encounter_for_wave_with_entry_check(
    seed: &str,
    time_of_day: &str,
    wave: u32,
    include_double_check: bool,
) -> Result<TownWildEncounter, EncounterSelectionError> {
    if wave == 0 {
        return Err(EncounterSelectionError::MissingTier);
    }
    let wave_seed: Vec<u16> = seed
        .encode_utf16()
        .map(|unit| unit.wrapping_add(wave as u16))
        .collect();
    let level_seed = wave_seed
        .iter()
        .map(|unit| unit.wrapping_add(wave.wrapping_shl(3) as u16))
        .collect();
    let mut level_rng = PhaserRandomDataGenerator::sown(level_seed);
    const BATTLE_SEED_CHARS: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut battle_seed_rng_draws = Vec::with_capacity(16);
    let battle_seed: String = (0..16)
        .map(|_| {
            let index = level_rng.integer_in_range(0, (BATTLE_SEED_CHARS.len() - 1) as i32);
            battle_seed_rng_draws.push(TownEntryRngDraw {
                range: BATTLE_SEED_CHARS.len() as u64,
                min: 0,
                value: i64::from(index),
            });
            BATTLE_SEED_CHARS[index as usize] as char
        })
        .collect();
    let mut rng = BattleRng::from_utf16_seed(wave_seed.clone());
    let double = if include_double_check {
        // Source checkIsDouble runs on the wave stream before Battle's
        // waveIndex << 3 constructor-offset stream. Classic Town waves 1–199
        // use chance 8 (32 on each tenth wave); final-wave exceptions and
        // modifiers/abilities are not represented by this bounded adapter.
        let chance = if wave % 10 == 0 { 32 } else { 8 };
        let roll = rng.rand_int(0, chance, 0);
        let draw = *rng.draws().last().expect("double-check draw is recorded");
        Some((roll == 0, draw))
    } else {
        None
    };
    let entry_rng_draws = double.map(|(_, draw)| vec![draw]).unwrap_or_default();
    rng.draws.clear();
    let is_boss = wave % 10 == 0;
    let deviation = 10.0 / wave as f64;
    let mut level_random_values = Vec::new();
    let wave_f = wave as f64;
    let level = if is_boss {
        let real = level_rng.frac() * 2.0 - 1.0;
        level_random_values.push(real);
        classic_boss_level(wave, wave, real, false)
    } else {
        let mut sum = 0.0;
        let mut i = deviation;
        while i > 0.0 {
            let value = level_rng.frac();
            level_random_values.push(value);
            sum += value;
            i -= 1.0;
        }
        let base_level = 1.0 + wave_f / 2.0 + (wave_f / 25.0).powi(2);
        (base_level + (sum / deviation).abs()).round().max(1.0) as u8
    };
    let selection = select_town_encounter_with_rng(&mut rng, time_of_day)?;
    let species_data =
        species_by_numeric_id(selection.species.0).ok_or(EncounterSelectionError::MissingTier)?;
    let mut encounter_rng_trace = Vec::new();
    let mut generated_id = 0;
    let mut generated_ivs = [0; 6];
    let mut generated_nature = 0;
    let mut generated_gender = Gender::Genderless;
    let mut generated_ability_index = 0;
    let mut generated_ability_id = 0;
    let mut generated_moves = Vec::new();
    let mut construction_rng_draws = Vec::new();
    let mut additional_opponents = Vec::new();
    if include_double_check {
        encounter_rng_trace.extend(selection.rng_draws.iter().map(|draw| TownEntryRngDraw {
            range: u64::from(draw.range),
            min: i64::from(draw.min),
            value: i64::from(draw.value),
        }));
        rng.draws.clear();
        let generated = generate_town_entry_pokemon_data(&mut rng, species_data, level);
        encounter_rng_trace.extend(generated.rng_draws.iter().copied());
        generated_id = generated.id;
        generated_ivs = generated.ivs;
        generated_nature = generated.nature;
        generated_gender = generated.gender;
        generated_ability_index = generated.ability_index;
        generated_ability_id = generated.ability_id;
        generated_moves = generated.moves;
        construction_rng_draws = generated.rng_draws;
        rng.draws.clear();
        if double.is_some_and(|(is_double, _)| is_double) {
            let second_selection = select_town_encounter_with_rng(&mut rng, time_of_day)?;
            encounter_rng_trace.extend(second_selection.rng_draws.iter().map(|draw| {
                TownEntryRngDraw {
                    range: u64::from(draw.range),
                    min: i64::from(draw.min),
                    value: i64::from(draw.value),
                }
            }));
            rng.draws.clear();
            let second_data = species_by_numeric_id(second_selection.species.0)
                .ok_or(EncounterSelectionError::MissingTier)?;
            let second_generated = generate_town_entry_pokemon_data(&mut rng, second_data, level);
            encounter_rng_trace.extend(second_generated.rng_draws.iter().copied());
            let second_first = *second_generated.moves.first().unwrap_or(&33);
            let second_second = *second_generated.moves.get(1).unwrap_or(&second_first);
            let second_battler = town_entry_battler(
                second_data.numeric_id,
                level,
                [Move::Imported(second_first), Move::Imported(second_second)],
                second_generated.ivs,
                second_generated.nature,
                0,
            );
            let mut second_battler = second_battler;
            second_battler.gender = second_generated.gender;
            additional_opponents.push(TownGeneratedOpponent {
                species_id: second_data.numeric_id,
                level,
                generated_id: second_generated.id,
                generated_ivs: second_generated.ivs,
                generated_nature: second_generated.nature,
                gender: second_generated.gender,
                generated_ability_id: second_generated.ability_id,
                generated_moves: second_generated.moves,
                battler: second_battler,
            });
        }
        // EncounterPhase regenerates the wild modifier thresholds before it
        // rolls held modifiers. For a wave-1 Town wild battle this consumes
        // the Berry and base-stat generator draws, followed by the weighted
        // attack-type generator draw. The per-enemy chance is then checked.
        // These are source RNG operations, even when no item is awarded.
        let berry_roll = town_entry_rand(&mut rng, &mut encounter_rng_trace, 12, 0);
        if berry_roll >= 6 {
            // BerryType has 11 entries; the source selects from the eight
            // ordinary entries [2, 9] by drawing an index in [0, 7].
            town_entry_rand(&mut rng, &mut encounter_rng_trace, 8, 0);
        }
        town_entry_rand(&mut rng, &mut encounter_rng_trace, 6, 0);
        let attack_weight: u64 = std::iter::once(&generated_moves)
            .chain(
                additional_opponents
                    .iter()
                    .map(|opponent| &opponent.generated_moves),
            )
            .flat_map(|moves| moves.iter())
            .filter_map(|move_id| pokerogue_data::move_by_numeric_id(*move_id))
            .filter(|move_data| !move_data.is_status)
            .fold(Vec::<(Element, u8)>::new(), |mut weights, move_data| {
                if let Some((_, weight)) = weights
                    .iter_mut()
                    .find(|(element, _)| *element == move_data.element)
                {
                    if *weight < 3 {
                        *weight += 1;
                    }
                } else {
                    weights.push((move_data.element, 1));
                }
                weights
            })
            .iter()
            .map(|(_, weight)| u64::from(*weight))
            .sum();
        if attack_weight > 0 {
            town_entry_rand(&mut rng, &mut encounter_rng_trace, attack_weight, 0);
        }
        let enemy_count = 1 + additional_opponents.len();
        for _ in 0..enemy_count {
            town_entry_rand(&mut rng, &mut encounter_rng_trace, 18, 0);
        }
        // InitEncounterPhase queues one PostSummonPhase for each active
        // field member. The priority queue's speed-order shuffle is seeded
        // independently from the current wave seed.
        let active_group_count = 1 + enemy_count;
        encounter_rng_trace.extend(town_entry_shuffle_draws(&wave_seed, active_group_count));
        // As PostSummonPhase and its queued ability phases are popped, the
        // same priority queue reorders the remaining groups; the final
        // entries are two-member queues. PokemonPriorityQueue uses the same
        // source shuffle offset and contributes the interleaved two-member
        // sorts shown in the executed command-boundary trace.
        for group_count in [active_group_count, 2, 2, 2, 2] {
            encounter_rng_trace.extend(town_entry_shuffle_draws(&wave_seed, group_count));
        }
    }
    let boss_segments = if is_boss {
        let total = (species_data.base_hp
            + species_data.base_attack
            + species_data.base_defense
            + species_data.base_special_attack
            + species_data.base_special_defense
            + species_data.base_speed) as u16;
        classic_boss_segments(wave, level, total)
    } else {
        0
    };
    let available: Vec<_> = level_moves_for(species_data.id, level)
        .map(|record| record.move_numeric_id)
        .fold(Vec::new(), |mut unique, id| {
            if !unique.contains(&id) {
                unique.push(id);
            }
            unique
        });
    let first = *generated_moves
        .first()
        .or_else(|| available.first())
        .unwrap_or(&33);
    let second = *generated_moves
        .get(1)
        .or_else(|| available.get(1))
        .unwrap_or(&first);
    let moves = [Move::Imported(first), Move::Imported(second)];
    let mut battler = if include_double_check {
        town_entry_battler(
            species_data.numeric_id,
            level,
            moves,
            generated_ivs,
            generated_nature,
            boss_segments,
        )
    } else {
        Battler::demo(Species::Imported(species_data.numeric_id), level, moves)
    };
    if include_double_check {
        battler.gender = generated_gender;
    }
    if !include_double_check {
        battler.configure_boss_segments(boss_segments);
    }
    Ok(TownWildEncounter {
        selection,
        entry_rng_draws,
        double: double.map(|(is_double, _)| is_double),
        level,
        boss_segments,
        battle_seed,
        battle_seed_rng_draws,
        level_random_values,
        construction_rng_draws,
        generated_id,
        generated_ivs,
        generated_nature,
        generated_gender,
        generated_ability_index,
        generated_ability_id,
        generated_moves,
        encounter_rng_trace,
        additional_opponents,
        is_mystery_encounter: false,
        battler,
        available_level_moves: available
            .into_iter()
            .filter_map(|id| pokerogue_data::move_by_numeric_id(id).map(|m| m.id))
            .collect(),
    })
}

/// Source-tier lookup; intervals are inclusive in Arena's tier methods.
pub fn normal_tier_for_roll(roll: u16) -> Option<u8> {
    TOWN_TIER_WEIGHTS
        .iter()
        .find(|weight| !weight.boss_table && roll >= weight.min_roll && roll <= weight.max_roll)
        .map(|weight| weight.tier_id)
}

fn town_pool(tier_id: u8, time_of_day: &str) -> Vec<(u16, &'static str)> {
    ["ALL", time_of_day]
        .into_iter()
        .flat_map(|time| {
            TOWN_ENCOUNTER_POOLS
                .iter()
                .find(|pool| pool.tier_id == tier_id && pool.time_of_day == time)
                .into_iter()
                .flat_map(|pool| pool.species.iter().copied())
        })
        .collect()
}

impl BattleRng {
    pub fn new(battle_seed: impl Into<String>) -> Self {
        Self::from_utf16_seed(battle_seed.into().encode_utf16().collect())
    }

    pub(crate) fn from_utf16_seed(battle_seed: Vec<u16>) -> Self {
        Self {
            battle_seed,
            seeded_turn: None,
            state: None,
            draws: Vec::new(),
        }
    }

    pub fn rand_int(&mut self, turn: u32, range: u32, min: i32) -> i32 {
        if range <= 1 {
            return min;
        }
        if self.seeded_turn != Some(turn) {
            self.draws.clear();
            let shift = turn.wrapping_shl(6) as u16;
            let seed = self
                .battle_seed
                .iter()
                .map(|unit| unit.wrapping_add(shift))
                .collect();
            self.state = Some(PhaserRandomDataGenerator::sown(seed));
            self.seeded_turn = Some(turn);
        }
        let value = self
            .state
            .as_mut()
            .expect("turn initialization creates RNG state")
            .integer_in_range(min, min + range as i32 - 1);
        self.draws.push(RngDraw {
            turn,
            range,
            min,
            value,
        });
        value
    }

    pub fn rand_float(&mut self, turn: u32) -> f64 {
        if self.seeded_turn != Some(turn) {
            let shift = turn.wrapping_shl(6) as u16;
            let seed = self
                .battle_seed
                .iter()
                .map(|unit| unit.wrapping_add(shift))
                .collect();
            self.state = Some(PhaserRandomDataGenerator::sown(seed));
            self.seeded_turn = Some(turn);
            self.draws.clear();
        }
        self.state
            .as_mut()
            .expect("turn initialization creates RNG state")
            .frac()
    }

    pub fn rand_int_inclusive(&mut self, turn: u32, min: i32, max: i32) -> i32 {
        self.rand_int(turn, (max - min + 1) as u32, min)
    }

    pub fn draws(&self) -> &[RngDraw] {
        &self.draws
    }

    fn offset_integer(&self, offset: u32, min: i32, max: i32) -> i32 {
        let seed = self
            .battle_seed
            .iter()
            .map(|unit| unit.wrapping_add(offset as u16))
            .collect();
        PhaserRandomDataGenerator::sown(seed).integer_in_range(min, max)
    }
}

#[derive(Clone, Debug)]
struct PhaserRandomDataGenerator {
    c: i32,
    s0: f64,
    s1: f64,
    s2: f64,
    n: f64,
}

impl PhaserRandomDataGenerator {
    fn sown(seed: Vec<u16>) -> Self {
        let mut rng = Self {
            c: 1,
            s0: 0.0,
            s1: 0.0,
            s2: 0.0,
            n: 0xefc8_249du32 as f64,
        };
        rng.s0 = rng.hash(&[32]);
        rng.s1 = rng.hash(&[32]);
        rng.s2 = rng.hash(&[32]);
        rng.s0 -= rng.hash(&seed);
        if rng.s0 < 0.0 {
            rng.s0 += 1.0;
        }
        rng.s1 -= rng.hash(&seed);
        if rng.s1 < 0.0 {
            rng.s1 += 1.0;
        }
        rng.s2 -= rng.hash(&seed);
        if rng.s2 < 0.0 {
            rng.s2 += 1.0;
        }
        rng
    }

    fn hash(&mut self, data: &[u16]) -> f64 {
        for &unit in data {
            self.n += unit as f64;
            let mut h = 0.025_196_032_824_169_38 * self.n;
            self.n = js_to_uint32(h);
            h -= self.n;
            h *= self.n;
            self.n = js_to_uint32(h);
            h -= self.n;
            self.n += h * 4_294_967_296.0;
        }
        js_to_uint32(self.n) * (1.0 / 4_294_967_296.0)
    }

    fn rnd(&mut self) -> f64 {
        let t = 2_091_639.0 * self.s0 + self.c as f64 * (1.0 / 4_294_967_296.0);
        self.c = t as i32;
        self.s0 = self.s1;
        self.s1 = self.s2;
        self.s2 = t - self.c as f64;
        self.s2
    }

    fn frac(&mut self) -> f64 {
        let first = self.rnd();
        let low_bits = (self.rnd() * 2_097_152.0) as i32;
        first + low_bits as f64 * (1.0 / 9_007_199_254_740_992.0)
    }

    fn integer_in_range(&mut self, min: i32, max: i32) -> i32 {
        (self.frac() * (max - min + 1) as f64 + min as f64).floor() as i32
    }
}

fn js_to_uint32(value: f64) -> f64 {
    value.trunc().rem_euclid(4_294_967_296.0)
}

pub fn accuracy_multiplier(user_accuracy_stage: i8, target_evasion_stage: i8) -> f64 {
    let difference =
        user_accuracy_stage.clamp(-6, 6) as i32 - target_evasion_stage.clamp(-6, 6) as i32;
    if difference > 0 {
        (3 + difference.min(6)) as f64 / 3.0
    } else if difference < 0 {
        3.0 / (3 - difference.max(-6)) as f64
    } else {
        1.0
    }
}

pub fn apply_accuracy_stage_booster(stage: i8, active: bool) -> i8 {
    if active {
        stage.saturating_add(1).min(6)
    } else {
        stage.clamp(-6, 6)
    }
}

pub fn boosted_stage_multiplier(stage: i8, bonus: f64) -> f64 {
    let stage = i32::from(stage.clamp(-6, 6));
    (((2 + stage).max(2) as f64 / (2 - stage).max(2) as f64) + bonus).min(4.0)
}

pub fn check_move_accuracy(
    accuracy: i16,
    user_accuracy_stage: i8,
    target_evasion_stage: i8,
    turn: u32,
    rng: &mut BattleRng,
) -> HitResult {
    if accuracy == -1 {
        return HitResult::Hit;
    }
    let threshold =
        accuracy as f64 * accuracy_multiplier(user_accuracy_stage, target_evasion_stage);
    if (rng.rand_int(turn, 100, 0) as f64) < threshold {
        HitResult::Hit
    } else {
        HitResult::Miss
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamageResult {
    pub damage: i32,
    pub type_multiplier: f64,
    pub stab_multiplier: f64,
}

/// Port of utils/damage.calculateBossSegmentDamage plus toDmgValue.
pub fn calculate_boss_segment_damage(
    damage: i32,
    current_hp: i32,
    segment_hp: f64,
    min_segment_index: u32,
    current_segment_index: Option<u32>,
) -> (i32, u32) {
    let index = current_segment_index
        .unwrap_or_else(|| (f64::from(current_hp) / segment_hp).ceil() as u32 - 1);
    if index == 0 {
        return (damage, 1);
    }
    let threshold = segment_hp * f64::from(index);
    let rounded_threshold = threshold.round();
    let leftover = f64::from(damage) - (f64::from(current_hp) - rounded_threshold);
    if leftover < 0.0 {
        return (damage, index + 1);
    }
    if leftover == 0.0 {
        return (damage, index);
    }
    let bypass = ((leftover / segment_hp).log2().floor().max(0.0) as u32)
        .min(index.saturating_sub(min_segment_index));
    let adjusted = (f64::from(current_hp) - threshold + segment_hp * f64::from(bypass))
        .floor()
        .max(1.0) as i32;
    (adjusted, index - bypass)
}

pub fn calculate_damage(
    attacker: &Battler,
    defender: &Battler,
    move_: Move,
    random_percent: u8,
) -> DamageResult {
    calculate_damage_with_boosters(
        attacker,
        defender,
        move_,
        random_percent,
        [false; 6],
        [false; 6],
    )
}

pub fn calculate_damage_with_boosters(
    attacker: &Battler,
    defender: &Battler,
    move_: Move,
    random_percent: u8,
    attacker_boosters: [bool; 6],
    defender_boosters: [bool; 6],
) -> DamageResult {
    calculate_damage_with_boosters_and_critical(
        attacker,
        defender,
        move_,
        random_percent,
        attacker_boosters,
        defender_boosters,
        false,
    )
}

fn calculate_damage_with_boosters_and_critical(
    attacker: &Battler,
    defender: &Battler,
    move_: Move,
    random_percent: u8,
    attacker_boosters: [bool; 6],
    defender_boosters: [bool; 6],
    critical: bool,
) -> DamageResult {
    let (type1, type2) = defender.species.types();
    let defending_types = [type1, type2.unwrap_or(type1)];
    let type_count = if type2.is_some() { 2 } else { 1 };
    calculate_damage_for_types_with_boosters_and_critical(
        attacker,
        defender,
        move_,
        random_percent,
        &defending_types[..type_count],
        attacker_boosters,
        defender_boosters,
        critical,
    )
}

pub fn calculate_damage_for_types(
    attacker: &Battler,
    defender: &Battler,
    move_: Move,
    random_percent: u8,
    defending_types: &[Element],
) -> DamageResult {
    calculate_damage_for_types_with_boosters(
        attacker,
        defender,
        move_,
        random_percent,
        defending_types,
        [false; 6],
        [false; 6],
    )
}

pub fn calculate_damage_for_types_with_boosters(
    attacker: &Battler,
    defender: &Battler,
    move_: Move,
    random_percent: u8,
    defending_types: &[Element],
    attacker_boosters: [bool; 6],
    defender_boosters: [bool; 6],
) -> DamageResult {
    calculate_damage_for_types_with_boosters_and_critical(
        attacker,
        defender,
        move_,
        random_percent,
        defending_types,
        attacker_boosters,
        defender_boosters,
        false,
    )
}

fn calculate_damage_for_types_with_boosters_and_critical(
    attacker: &Battler,
    defender: &Battler,
    move_: Move,
    random_percent: u8,
    defending_types: &[Element],
    attacker_boosters: [bool; 6],
    defender_boosters: [bool; 6],
    critical: bool,
) -> DamageResult {
    let (attack_stat, mut attack_stage, defense_stat, mut defense_stage) = if move_.is_special() {
        (
            attacker.special_attack,
            attacker.stat_stage(BattleStat::SpecialAttack),
            defender.special_defense,
            defender.stat_stage(BattleStat::SpecialDefense),
        )
    } else {
        (
            attacker.attack,
            attacker.stat_stage(BattleStat::Attack),
            defender.defense,
            defender.stat_stage(BattleStat::Defense),
        )
    };
    // The pinned source ignores negative attacker and positive defender stages
    // for a critical hit. Other stat-stage/ability interactions remain bounded.
    if critical {
        attack_stage = attack_stage.max(0);
        defense_stage = defense_stage.min(0);
    }
    let attack_boost = temp_booster_index(if move_.is_special() {
        BattleStat::SpecialAttack
    } else {
        BattleStat::Attack
    })
    .is_some_and(|index| attacker_boosters[index]);
    let defense_boost = temp_booster_index(if move_.is_special() {
        BattleStat::SpecialDefense
    } else {
        BattleStat::Defense
    })
    .is_some_and(|index| defender_boosters[index]);
    let attack = f64::from(attack_stat)
        * boosted_stage_multiplier(attack_stage, if attack_boost { 0.2 } else { 0.0 });
    let defense = f64::from(defense_stat)
        * boosted_stage_multiplier(defense_stage, if defense_boost { 0.2 } else { 0.0 });
    let type_multiplier = if move_.is_typeless() {
        1.0
    } else {
        type_effectiveness(move_.element(), defending_types) as f64
    };
    if type_multiplier == 0.0 {
        return DamageResult {
            damage: 0,
            type_multiplier,
            stab_multiplier: 1.0,
        };
    }

    let (attacker_type1, attacker_type2) = attacker.species.types();
    let stab_multiplier = if !move_.is_typeless()
        && (move_.element() == attacker_type1 || attacker_type2 == Some(move_.element()))
    {
        1.5
    } else {
        1.0
    };
    let level_multiplier = (2.0 * attacker.level as f64) / 5.0 + 2.0;
    let base_damage = (level_multiplier * move_.power() as f64 * attack / defense / 50.0) + 2.0;
    // Mirrors getAttackDamage's final multiplier order and toDmgValue's floor/minimum.
    let final_damage = base_damage
        * 1.0 // single-target multiplier
        * 1.0 // multi-hit enhancement
        * 1.0 // weather
        * 1.0 // received double damage
        * if critical { 1.5 } else { 1.0 }
        * (random_percent as f64 / 100.0)
        * stab_multiplier
        * type_multiplier
        * 1.0 // burn
        * 1.0 // screen
        * 1.0; // move tag multiplier
    DamageResult {
        damage: (final_damage.floor() as i32).max(1),
        type_multiplier,
        stab_multiplier,
    }
}

/// SMART_RANDOM score subset for source moves that are ordinary damaging attacks with no attrs.
fn score_enemy_move(attacker: &Battler, target: &Battler, move_: Move) -> f64 {
    let (target_type1, target_type2) = target.species.types();
    let target_types = [target_type1, target_type2.unwrap_or(target_type1)];
    let type_count = if target_type2.is_some() { 2 } else { 1 };
    let effectiveness = type_effectiveness(move_.element(), &target_types[..type_count]) as f64;

    // Port of AttackMove.getTargetBenefitScore with no move attributes/conditions:
    // type matchup and power form attackScore; the enemy-target sign in getNextMove
    // negates this target benefit, making larger damage potential score higher.
    let type_delta = effectiveness - 1.0;
    let mut attack_score = type_delta.powi(2) * if effectiveness < 1.0 { -2.0 } else { 2.0 };
    let (main_stat, other_stat) = if move_.is_special() {
        (attacker.special_attack, attacker.attack)
    } else {
        (attacker.attack, attacker.special_attack)
    };
    let stat_ratio = other_stat as f64 / main_stat.max(1) as f64;
    if stat_ratio <= 0.75 {
        attack_score *= 2.0;
    } else if stat_ratio <= 0.875 {
        attack_score *= 1.5;
    }
    let accuracy_factor = if move_.accuracy() == -1 {
        1.0
    } else {
        (move_.accuracy() as f64).min(100.0) / 100.0
    };
    attack_score += (move_.power() as f64 * accuracy_factor / 5.0).floor();

    let (attacker_type1, attacker_type2) = attacker.species.types();
    let stab = if move_.element() == attacker_type1 || attacker_type2 == Some(move_.element()) {
        1.5
    } else {
        1.0
    };
    attack_score * effectiveness * stab
}

/// Source `EnemyPokemon.getNextTargets` consumes a battle-seed draw even when
/// there is only one possible target. For the bounded ordinary damaging-move
/// slice, reproduce the positive target-benefit weight used by `Move`.
fn single_enemy_target_rng_range(attacker: &Battler, target: &Battler, move_: Move) -> Option<u32> {
    if move_.is_status() || move_.power() == 0 {
        return None;
    }
    let (target_type1, target_type2) = target.species.types();
    let target_types = [target_type1, target_type2.unwrap_or(target_type1)];
    let type_count = if target_type2.is_some() { 2 } else { 1 };
    let effectiveness = if move_.is_typeless() {
        1.0
    } else {
        type_effectiveness(move_.element(), &target_types[..type_count]) as f64
    };
    let mut attack_score =
        (effectiveness - 1.0).powi(2) * if effectiveness < 1.0 { -2.0 } else { 2.0 };
    let (main_stat, off_stat) = if move_.is_special() {
        (attacker.special_attack, attacker.attack)
    } else {
        (attacker.attack, attacker.special_attack)
    };
    let stat_ratio = f64::from(off_stat) / f64::from(main_stat.max(1));
    if stat_ratio <= 0.75 {
        attack_score *= 2.0;
    } else if stat_ratio <= 0.875 {
        attack_score *= 1.5;
    }
    let accuracy_factor = if move_.accuracy() == -1 {
        1.0
    } else {
        f64::from(move_.accuracy().min(100)) / 100.0
    };
    attack_score += (f64::from(move_.power()) * accuracy_factor / 5.0).floor();

    // For an enemy targeting the player, source getNextTargets negates the
    // move's target-benefit score. Ordinary attacks use this score directly.
    positive_target_weight_range(attack_score)
}

fn positive_target_weight_range(mut weight: f64) -> Option<u32> {
    if weight < 1.0 {
        weight += (weight - 1.0).abs();
    }
    if weight < 1.0 || weight.fract() != 0.0 || weight > f64::from(u32::MAX) {
        return None;
    }
    Some(weight as u32)
}

/// Bounded port of StatStageChangeAttr.getTargetBenefitScore for Growl,
/// Tail Whip and Leer when a wild SMART_RANDOM enemy has only status moves.
fn score_enemy_stage_move(attacker: &Battler, target: &Battler, move_: Move) -> f64 {
    let Some(MoveEffect::StatStageChange { stat, stages, .. }) = move_.effect() else {
        return -20.0;
    };
    let current = target.stat_stage(stat);
    let effective = (current + stages).clamp(-6, 6) - current;
    // Original StatStageChangeAttr suppresses target DEF/SPDEF drops when
    // the user has no physical attack available in its moveset.
    if matches!(stat, BattleStat::Defense | BattleStat::SpecialDefense)
        && !attacker
            .moves
            .iter()
            .take(attacker.move_count())
            .any(|candidate| !candidate.is_status() && !candidate.is_special())
    {
        return -20.0;
    }
    // Source score is levels * 4 + (positive ? -2 : +2); enemy AI negates
    // target benefit, so lowering an opposing stat is beneficial.
    let source_score = effective * 4 + if effective > 0 { -2 } else { 2 };
    f64::from(if source_score == 0 {
        -20
    } else {
        -source_score
    })
}

#[derive(Clone, Debug)]
pub struct Battle {
    pub player: Team,
    pub enemy: Battler,
    pub turn: u32,
    pub log: String,
    pub outcome: Option<Outcome>,
    pub last_enemy_decision: Option<EnemyAiDecision>,
    pub last_action_order: Vec<BattleActor>,
    pub last_speed_shuffle_roll: Option<i32>,
    pub last_events: Vec<BattleEvent>,
    /// Level-up moves queued after `LevelUpPhase`; full movesets await a choice.
    pub pending_level_moves: Vec<(usize, u16)>,
    /// Informational prompts for moves available through a prevolution.
    pub pending_prevolution_moves: Vec<(usize, u16)>,
    /// LearnMove and Message phases in source queue order across party members.
    pub pending_level_up_prompts: Vec<PendingLevelUpPrompt>,
    pub initial_encounter: Option<TownWildEncounter>,
    pub last_capture_attempt: Option<CaptureAttempt>,
    /// Source-ordered [ATK, DEF, SPATK, SPDEF, SPD, ACC] boosters.
    player_temp_stat_boosters: [bool; 6],
    participants: Vec<bool>,
    pending_replacement: Option<usize>,
    rng: BattleRng,
}

impl Battle {
    pub fn demo() -> Self {
        Self::demo_with_seed("RustDemoSeed0001")
    }

    pub fn demo_with_seed(battle_seed: impl Into<String>) -> Self {
        Self {
            player: Team::new(
                Battler::demo(Species::Bulbasaur, 5, [Move::VineWhip, Move::Tackle]),
                Battler::demo(Species::Charmander, 5, [Move::Ember, Move::Tackle]),
            ),
            enemy: Battler::demo(Species::Charmander, 5, [Move::Ember, Move::Tackle]),
            turn: 1,
            log: "Choose a move: 1 or 2".into(),
            outcome: None,
            last_enemy_decision: None,
            last_action_order: Vec::new(),
            last_speed_shuffle_roll: None,
            last_events: Vec::new(),
            pending_level_moves: Vec::new(),
            pending_prevolution_moves: Vec::new(),
            pending_level_up_prompts: Vec::new(),
            initial_encounter: None,
            last_capture_attempt: None,
            player_temp_stat_boosters: [false; 6],
            participants: vec![true, false],
            pending_replacement: None,
            rng: BattleRng::new(battle_seed),
        }
    }

    pub fn town_encounter_with_seed(
        seed: impl Into<String>,
        time_of_day: &str,
    ) -> Result<Self, EncounterSelectionError> {
        let seed = seed.into();
        Self::town_encounter_for_wave(seed, time_of_day, 1)
    }

    pub fn town_encounter_for_wave(
        seed: impl Into<String>,
        time_of_day: &str,
        wave: u32,
    ) -> Result<Self, EncounterSelectionError> {
        let seed = seed.into();
        let player = Self::demo_with_seed(&seed).player;
        Self::town_encounter_for_wave_with_team(seed, time_of_day, wave, player)
    }

    pub fn town_encounter_for_wave_with_team(
        seed: impl Into<String>,
        time_of_day: &str,
        wave: u32,
        player: Team,
    ) -> Result<Self, EncounterSelectionError> {
        let seed = seed.into();
        let encounter = generate_town_wild_encounter_for_wave(&seed, time_of_day, wave)?;
        Ok(Self::from_town_encounter(encounter, player))
    }

    /// Normal-entry development constructor paired with the projected
    /// encounter details so replay tools can compare phase-entry observations.
    pub fn town_classic_entry_for_wave(
        seed: impl Into<String>,
        time_of_day: &str,
        wave: u32,
    ) -> Result<(Self, TownWildEncounter), EncounterSelectionError> {
        let seed = seed.into();
        let player = Team::solo(town_replay_starter());
        let encounter = generate_town_wild_encounter_for_classic_entry(&seed, time_of_day, wave)?;
        let battle = Self::from_town_encounter(encounter.clone(), player);
        Ok((battle, encounter))
    }

    /// Normal Classic Town entry paired with a source-captured player party.
    /// This supports deterministic replay adapters without replacing encounter
    /// selection, battle RNG, or party combat behavior.
    pub fn town_classic_entry_for_wave_with_team(
        seed: impl Into<String>,
        time_of_day: &str,
        wave: u32,
        player: Team,
    ) -> Result<(Self, TownWildEncounter), EncounterSelectionError> {
        let seed = seed.into();
        let encounter = generate_town_wild_encounter_for_classic_entry(&seed, time_of_day, wave)?;
        let battle = Self::from_town_encounter(encounter.clone(), player);
        Ok((battle, encounter))
    }

    fn from_town_encounter(encounter: TownWildEncounter, player: Team) -> Self {
        let mut battle = Self::demo_with_seed(encounter.battle_seed.clone());
        let active_slot = player.active_slot();
        battle.player = player;
        battle.participants = vec![false; battle.player.len()];
        battle.participants[active_slot] = true;
        battle.enemy = encounter.battler.clone();
        battle.log = format!("A wild {} appeared!", battle.enemy.species.name());
        battle.initial_encounter = Some(encounter);
        battle
    }

    pub fn choose_move(&mut self, index: usize) {
        let _ = self.take_turn(PlayerAction::UseMove(index));
    }

    pub fn needs_replacement(&self) -> bool {
        self.pending_replacement.is_some()
    }

    pub fn pending_replacement_slot(&self) -> Option<usize> {
        self.pending_replacement
    }

    pub fn rng_draws(&self) -> &[RngDraw] {
        self.rng.draws()
    }

    pub fn set_player_temp_stat_boosters(&mut self, active: [bool; 6]) {
        self.player_temp_stat_boosters = active;
    }

    /// Resolve one selected player action. A voluntary switch has command priority
    /// over a move and is followed by the enemy's action against the new active member.
    pub fn take_turn(
        &mut self,
        action: PlayerAction,
    ) -> Result<Option<[SwitchEvent; 5]>, TurnActionError> {
        if self.outcome.is_some() {
            return Err(TurnActionError::BattleOver);
        }
        if self.needs_replacement() {
            return Err(TurnActionError::MandatoryReplacement);
        }
        self.last_events.clear();
        self.last_capture_attempt = None;
        match action {
            PlayerAction::UseMove(index) => {
                if index >= self.player.move_count() {
                    return Err(TurnActionError::InvalidMoveIndex);
                }
                let selected = self.player.moves[index];
                let has_usable_move = (0..self.player.move_count())
                    .any(|slot| self.player.move_pp_remaining(slot).unwrap_or(0) > 0);
                let move_ = if self.player.move_pp_remaining(index) == Some(0) {
                    if has_usable_move {
                        return Err(TurnActionError::NoPp);
                    }
                    Move::struggle()
                } else {
                    selected
                };
                let enemy_move = self
                    .select_enemy_move()
                    .ok_or(TurnActionError::UnsupportedEnemyMoves)?;
                self.resolve_turn(move_, enemy_move);
                Ok(None)
            }
            PlayerAction::Switch(slot) => {
                self.player
                    .can_switch_to(slot)
                    .map_err(TurnActionError::Switch)?;
                let enemy_move = self
                    .select_enemy_move()
                    .ok_or(TurnActionError::UnsupportedEnemyMoves)?;
                let events = self.switch_player(slot).map_err(TurnActionError::Switch)?;
                self.record_switch_events(events);
                // Switch commands resolve before FIGHT commands in TurnStartPhase.
                self.last_action_order = vec![BattleActor::Player, BattleActor::Enemy];
                self.last_speed_shuffle_roll = None;
                self.enemy_turn(enemy_move);
                if self.player.hp <= 0 {
                    self.handle_player_faint();
                } else {
                    self.finish_turn();
                }
                Ok(Some(events))
            }
            PlayerAction::ThrowPokeball
            | PlayerAction::ThrowGreatBall
            | PlayerAction::ThrowUltraBall => {
                if self.player.len() >= PLAYER_PARTY_MAX_SIZE {
                    return Err(TurnActionError::PartyFull);
                }
                // CommandPhase blocks ordinary balls while a non-final boss has
                // more than one segment remaining (the Master Ball exception is
                // outside this client's current inventory).
                if self.enemy.boss_segments > 0 && self.enemy.boss_segment_index >= 1 {
                    return Err(TurnActionError::BossNotCatchable);
                }
                if self.enemy.moves.iter().all(|move_| move_.is_status()) {
                    return Err(TurnActionError::UnsupportedEnemyMoves);
                }
                let attempt = attempt_capture(
                    &mut self.rng,
                    self.turn,
                    self.enemy.species.catch_rate(),
                    self.enemy.hp,
                    self.enemy.max_hp,
                    match action {
                        PlayerAction::ThrowGreatBall => Pokeball::Great,
                        PlayerAction::ThrowUltraBall => Pokeball::Ultra,
                        _ => Pokeball::Poke,
                    },
                    1.0,
                    1.0,
                    0,
                )
                .map_err(|_| TurnActionError::BattleOver)?;
                let species_id = self.enemy.species.numeric_id();
                self.last_capture_attempt = Some(attempt.clone());
                self.last_events.push(BattleEvent::CaptureAttemptResolved {
                    species_id,
                    caught: attempt.caught,
                    party_slot: None,
                });
                if attempt.caught {
                    let mut captured = self.enemy.clone();
                    captured.reset_stat_stages();
                    let party_slot = self
                        .add_player_member(captured)
                        .map_err(|_| TurnActionError::PartyFull)?;
                    self.enemy.hp = 0;
                    if let Some(BattleEvent::CaptureAttemptResolved {
                        party_slot: slot, ..
                    }) = self.last_events.last_mut()
                    {
                        *slot = Some(party_slot);
                    }
                    self.last_action_order = vec![BattleActor::Player];
                    self.last_speed_shuffle_roll = None;
                    self.finish_turn();
                    self.log = format!("{} was caught!", self.enemy.species.name());
                } else {
                    // AttemptCapturePhase precedes the queued foe MovePhase; the foe acts only
                    // after a failed capture and therefore consumes its RNG afterward.
                    let enemy_move = self
                        .select_enemy_move()
                        .ok_or(TurnActionError::UnsupportedEnemyMoves)?;
                    self.last_action_order = vec![BattleActor::Player, BattleActor::Enemy];
                    self.last_speed_shuffle_roll = None;
                    self.log = format!("{} broke free!", self.enemy.species.name());
                    self.enemy_turn(enemy_move);
                    if self.player.hp <= 0 {
                        self.handle_player_faint();
                    } else {
                        self.finish_turn();
                    }
                }
                Ok(None)
            }
        }
    }

    pub fn switch_player(&mut self, slot: usize) -> Result<[SwitchEvent; 5], SwitchError> {
        if self.outcome.is_some() {
            return Err(SwitchError::BattleOver);
        }
        if self.needs_replacement() {
            return Err(SwitchError::MandatoryReplacement);
        }
        let outgoing = self.player.species.name();
        let events = self.player.switch_to(slot)?;
        self.participants[slot] = true;
        let incoming = self.player.species.name();
        self.log = format!("{outgoing}, come back! Go, {incoming}!");
        Ok(events)
    }

    /// Add a captured or otherwise newly acquired member to the local party.
    pub fn add_player_member(&mut self, member: Battler) -> Result<usize, TeamError> {
        let slot = self.player.add_member(member)?;
        self.participants.push(false);
        Ok(slot)
    }

    /// Complete the forced replacement. It does not consume a second action or trigger
    /// an extra enemy attack; the already-resolved turn ends after the summon.
    pub fn replace_fainted_player(&mut self, slot: usize) -> Result<[SwitchEvent; 5], SwitchError> {
        if self.outcome.is_some() {
            return Err(SwitchError::BattleOver);
        }
        if self.pending_replacement.is_none() {
            return Err(SwitchError::ReplacementNotPending);
        }
        self.player.can_switch_to(slot)?;
        let events = self.player.switch_to(slot)?;
        self.participants[slot] = true;
        self.record_switch_events(events);
        self.pending_replacement = None;
        self.finish_turn();
        Ok(events)
    }

    fn record_switch_events(&mut self, events: [SwitchEvent; 5]) {
        self.last_events
            .extend(events.into_iter().map(BattleEvent::Switch));
    }

    fn handle_player_faint(&mut self) {
        let slot = self.player.active_slot();
        self.player.leave_field(FieldExitKind::Faint);
        self.last_events.push(BattleEvent::StatStagesReset { slot });
        self.last_events.push(BattleEvent::PokemonFainted { slot });
        let has_reserve = (0..self.player.len()).any(|candidate| {
            candidate != slot
                && self
                    .player
                    .member(candidate)
                    .is_some_and(|member| member.hp > 0)
        });
        if has_reserve {
            self.pending_replacement = Some(slot);
            self.last_events
                .push(BattleEvent::ReplacementRequired { fainted_slot: slot });
            self.log = format!(
                "{} fainted! Choose a replacement.",
                self.player.species.name()
            );
        } else {
            self.outcome = Some(Outcome::PlayerLoss);
            self.last_events
                .push(BattleEvent::BattleEnded(Outcome::PlayerLoss));
            self.log = "You lost! Press Enter to play again".into();
        }
    }

    /// Debug-only scenario; Sand Attack is not inserted into the demo Pokémon's moveset.
    pub fn debug_sand_attack(&mut self) {
        if self.outcome.is_none() && !self.needs_replacement() {
            if let Some(enemy_move) = self.select_enemy_move() {
                self.resolve_turn(Move::SandAttack, enemy_move);
            }
        }
    }

    fn resolve_turn(&mut self, move_: Move, enemy_move: Move) {
        self.log.clear();
        let shuffle_roll =
            self.rng
                .offset_integer(self.turn.wrapping_mul(1000).wrapping_add(2), 0, 1);
        let player_first = if move_.priority() != enemy_move.priority() {
            move_.priority() > enemy_move.priority()
        } else if self
            .player
            .effective_speed_with_bonus(if self.player_temp_stat_boosters[4] {
                0.2
            } else {
                0.0
            })
            != f64::from(self.enemy.effective_speed())
        {
            self.player
                .effective_speed_with_bonus(if self.player_temp_stat_boosters[4] {
                    0.2
                } else {
                    0.0
                })
                > f64::from(self.enemy.effective_speed())
        } else {
            shuffle_roll == 1
        };
        self.last_speed_shuffle_roll = Some(shuffle_roll);
        self.last_action_order = if player_first {
            vec![BattleActor::Player, BattleActor::Enemy]
        } else {
            vec![BattleActor::Enemy, BattleActor::Player]
        };
        for actor in self.last_action_order.clone() {
            if self.player.hp <= 0 || self.enemy.hp <= 0 {
                break;
            }
            match actor {
                BattleActor::Player => self.attack(true, move_),
                BattleActor::Enemy => self.enemy_turn(enemy_move),
            }
            if self.player.hp <= 0 {
                self.handle_player_faint();
                return;
            }
        }
        if let Some(hp_restored) = self.player.maybe_consume_sitrus_berry() {
            let slot = self.player.active_slot();
            self.last_events
                .push(BattleEvent::SitrusBerryConsumed { slot, hp_restored });
            self.append_log(format!(
                "{}'s Sitrus Berry restored {hp_restored} HP!",
                self.player.species.name()
            ));
        }
        self.finish_turn();
    }

    fn finish_turn(&mut self) {
        self.turn += 1;
        self.last_events
            .push(BattleEvent::TurnEnded { turn: self.turn });
        self.outcome = if self.enemy.hp <= 0 {
            Some(Outcome::PlayerWin)
        } else if self.player.hp <= 0 {
            Some(Outcome::PlayerLoss)
        } else {
            None
        };
        if let Some(outcome) = self.outcome {
            self.last_events.push(BattleEvent::BattleEnded(outcome));
            self.log = match outcome {
                Outcome::PlayerWin => "You won! Press Enter to play again".into(),
                Outcome::PlayerLoss => "You lost! Press Enter to play again".into(),
            };
        }
    }

    pub fn award_victory_experience(&mut self) -> Vec<(usize, u32, Vec<u8>)> {
        self.award_victory_experience_with_charm_stacks(0)
    }

    pub fn award_victory_experience_with_charm_stacks(
        &mut self,
        charm_stacks: u8,
    ) -> Vec<(usize, u32, Vec<u8>)> {
        self.award_victory_experience_with_boosters(charm_stacks, 0)
    }

    pub fn award_victory_experience_with_boosters(
        &mut self,
        charm_stacks: u8,
        super_charm_stacks: u8,
    ) -> Vec<(usize, u32, Vec<u8>)> {
        self.award_victory_experience_with_level_cap(charm_stacks, super_charm_stacks, 100)
    }

    pub fn award_victory_experience_with_level_cap(
        &mut self,
        charm_stacks: u8,
        super_charm_stacks: u8,
        max_exp_level: u8,
    ) -> Vec<(usize, u32, Vec<u8>)> {
        self.award_victory_experience_with_exp_share(
            charm_stacks,
            super_charm_stacks,
            0,
            max_exp_level,
        )
    }

    pub fn award_victory_experience_with_exp_share(
        &mut self,
        charm_stacks: u8,
        super_charm_stacks: u8,
        exp_share_stacks: u8,
        max_exp_level: u8,
    ) -> Vec<(usize, u32, Vec<u8>)> {
        if self.outcome != Some(Outcome::PlayerWin) {
            return Vec::new();
        }
        let participant_count = self
            .participants
            .iter()
            .filter(|participated| **participated)
            .count();
        if participant_count == 0 {
            return Vec::new();
        }
        let exp_value = self.enemy_exp_value();
        let mut rewards = Vec::new();
        let mut learned_moves = Vec::new();
        let mut experience_events = Vec::new();
        for (slot, participated) in self.participants.iter().copied().enumerate() {
            if let Some(member) = self.player.member_mut(slot) {
                if member.hp <= 0 {
                    continue;
                }
                if participated {
                    member.add_friendship(FRIENDSHIP_GAIN_FROM_BATTLE, false);
                }
                if member.level >= max_exp_level.clamp(1, 100) {
                    continue;
                }
                let raw_exp = if participated {
                    (exp_value / participant_count as f64).floor() as u32
                } else if exp_share_stacks > 0 {
                    (exp_value * (f64::from(exp_share_stacks.min(5)) * 0.2)
                        / participant_count as f64)
                        .floor() as u32
                } else {
                    continue;
                };
                let earned = apply_exp_boosters(raw_exp, charm_stacks, super_charm_stacks);
                let previous_exp = member.exp;
                let previous_level = member.level;
                let levels = member.grant_exp_capped(earned, max_exp_level);
                if earned > 0 {
                    experience_events.push(BattleEvent::ExperienceGained {
                        party_slot: slot,
                        amount: earned,
                        previous_exp,
                        current_exp: member.exp,
                        previous_level,
                        current_level: member.level,
                    });
                }
                let (learned, pending, prevolution) = member.learn_level_moves(&levels);
                learned_moves.extend(learned.into_iter().map(|move_id| (slot, move_id)));
                self.pending_level_moves
                    .extend(pending.into_iter().map(|move_id| (slot, move_id)));
                self.pending_prevolution_moves
                    .extend(prevolution.into_iter().map(|move_id| (slot, move_id)));
                rewards.push((slot, earned, levels));
            }
        }
        self.pending_level_up_prompts = pending_level_up_sequence(
            &self.pending_level_moves,
            &self.pending_prevolution_moves,
            self.player.len(),
        );
        self.last_events.extend(experience_events);
        self.last_events
            .extend(learned_moves.into_iter().map(|(party_slot, move_id)| {
                BattleEvent::MoveLearned {
                    party_slot,
                    move_id,
                }
            }));
        rewards
    }

    pub fn enemy_exp_value(&self) -> f64 {
        self.enemy.species.base_exp() as f64 * self.enemy.level as f64 / 5.0 + 1.0
    }

    fn enemy_turn(&mut self, move_: Move) {
        self.attack(false, move_);
    }

    fn select_enemy_move(&mut self) -> Option<Move> {
        let usable_moves: Vec<_> = self
            .enemy
            .moves
            .iter()
            .take(self.enemy.move_count())
            .enumerate()
            .filter(|(index, _)| self.enemy.move_pp_remaining(*index).unwrap_or(0) > 0)
            .map(|(_, movement)| *movement)
            .collect();
        let attack_moves: Vec<_> = self
            .enemy
            .moves
            .iter()
            .take(self.enemy.move_count())
            .enumerate()
            .filter(|(index, move_)| {
                !move_.is_status() && self.enemy.move_pp_remaining(*index).unwrap_or(0) > 0
            })
            .map(|(_, move_)| *move_)
            .collect();
        if attack_moves.is_empty() {
            // Original getNextMove returns directly when only one usable move
            // remains, before SMART_RANDOM scoring and RNG. The fixed two-slot
            // port can duplicate that lone source move (e.g. Silcoon's Harden).
            let usable: Vec<_> = self
                .enemy
                .moves
                .iter()
                .take(self.enemy.move_count())
                .enumerate()
                .filter(|(index, _)| self.enemy.move_pp_remaining(*index).unwrap_or(0) > 0)
                .map(|(_, movement)| *movement)
                .collect();
            if let Some(movement) = usable.first().copied()
                && usable
                    .iter()
                    .all(|candidate| candidate.id() == movement.id())
            {
                self.last_enemy_decision = Some(EnemyAiDecision {
                    selected_move: movement,
                    ko_damage: Vec::new(),
                    candidates: vec![movement],
                    scores: Vec::new(),
                    random_rolls: Vec::new(),
                });
                return Some(movement);
            }
            if usable.is_empty() {
                let struggle = Move::struggle();
                self.last_enemy_decision = Some(EnemyAiDecision {
                    selected_move: struggle,
                    ko_damage: Vec::new(),
                    candidates: vec![struggle],
                    scores: Vec::new(),
                    random_rolls: Vec::new(),
                });
                return Some(struggle);
            }
            // Status-only pools do not enter the damage KO prefilter. The
            // pinned SMART_RANDOM scorer ranks every remaining move instead.
            let mut scores: Vec<_> = usable
                .iter()
                .copied()
                .map(|movement| {
                    (
                        movement,
                        score_enemy_stage_move(&self.enemy, &self.player, movement),
                    )
                })
                .collect();
            scores.sort_by(|a, b| b.1.total_cmp(&a.1));
            let mut chosen_index = 0;
            let mut random_rolls = Vec::new();
            while chosen_index < scores.len() - 1 {
                let roll = self.rng.rand_int(self.turn, 8, 0);
                random_rolls.push(roll);
                if roll >= 5 {
                    chosen_index += 1;
                } else {
                    break;
                }
            }
            let decision = EnemyAiDecision {
                selected_move: scores[chosen_index].0,
                ko_damage: Vec::new(),
                candidates: usable,
                scores,
                random_rolls,
            };
            self.last_enemy_decision = Some(decision.clone());
            return Some(decision.selected_move);
        }
        // getNextMove's KO prefilter uses simulated damage (random multiplier = 1)
        // and replaces the pool only when at least one eligible move can KO.
        let ko_damage: Vec<_> = attack_moves
            .iter()
            .copied()
            .filter(|move_| move_.target() != MoveTarget::Attacker)
            .map(|move_| {
                (
                    move_,
                    calculate_damage_with_boosters(
                        &self.enemy,
                        &self.player,
                        move_,
                        100,
                        [false; 6],
                        self.player_temp_stat_boosters,
                    )
                    .damage,
                )
            })
            .collect();
        let ko_moves: Vec<_> = ko_damage
            .iter()
            .filter(|(_, damage)| *damage >= self.player.hp)
            .map(|(move_, _)| *move_)
            .collect();
        let candidates = if ko_moves.is_empty() {
            // The source keeps status moves in SMART_RANDOM's scored pool when
            // no damaging move can KO. They affect both target scoring and the
            // 3/8 move-advance draw, even when the damaging move is preferred.
            usable_moves
        } else {
            ko_moves
        };
        // Source computes and caches one target choice per candidate move
        // before scoring the pool. This consumes RNG even with one target.
        for movement in &candidates {
            let range = if movement.is_status() {
                let score = score_enemy_stage_move(&self.enemy, &self.player, *movement);
                positive_target_weight_range(score)
            } else {
                single_enemy_target_rng_range(&self.enemy, &self.player, *movement)
            };
            if let Some(range) = range
                && range > 1
            {
                self.rng.rand_int(self.turn, range, 0);
            }
        }
        let mut scores: Vec<_> = candidates
            .iter()
            .copied()
            .map(|move_| {
                let score = if move_.is_status() {
                    score_enemy_stage_move(&self.enemy, &self.player, move_)
                } else {
                    score_enemy_move(&self.enemy, &self.player, move_)
                };
                (move_, score)
            })
            .collect();
        // Wild Pokémon default to SMART_RANDOM: descending benefit score, then each
        // next move is selected with a 3/8 roll; otherwise selection stops at this rank.
        scores.sort_by(|a, b| b.1.total_cmp(&a.1));
        let mut chosen_index = 0;
        let mut random_rolls = Vec::new();
        while chosen_index < scores.len() - 1 {
            let roll = self.rng.rand_int(self.turn, 8, 0);
            random_rolls.push(roll);
            if roll >= 5 {
                chosen_index += 1;
            } else {
                break;
            }
        }
        let decision = EnemyAiDecision {
            selected_move: scores[chosen_index].0,
            ko_damage,
            candidates,
            scores,
            random_rolls,
        };
        self.last_enemy_decision = Some(decision.clone());
        Some(decision.selected_move)
    }

    fn attack(&mut self, player: bool, move_: Move) {
        let user = if player {
            &mut self.player
        } else {
            &mut self.enemy
        };
        if let Some(index) = user
            .moves
            .iter()
            .take(user.move_count())
            .position(|candidate| candidate.numeric_id() == move_.numeric_id())
        {
            user.consume_move_pp(index, 1);
        }
        self.last_events.push(BattleEvent::MoveExecuted {
            actor: if player {
                BattleActor::Player
            } else {
                BattleActor::Enemy
            },
            move_,
        });
        let effectiveness = {
            if move_.is_status() {
                1.0
            } else {
                let defender = if player { &self.enemy } else { &self.player };
                if move_.is_typeless() {
                    1.0
                } else {
                    let (type1, type2) = defender.species.types();
                    let defending_types = [type1, type2.unwrap_or(type1)];
                    let type_count = if type2.is_some() { 2 } else { 1 };
                    type_effectiveness(move_.element(), &defending_types[..type_count]) as f64
                }
            }
        };
        if effectiveness == 0.0 {
            self.append_log(format!(
                "{} used {}! No effect!",
                if player {
                    self.player.species.name()
                } else {
                    self.enemy.species.name()
                },
                move_.name()
            ));
            return;
        }
        let (mut accuracy_stage, evasion_stage) = if player {
            (
                self.player.stat_stage(BattleStat::Accuracy),
                self.enemy.stat_stage(BattleStat::Evasion),
            )
        } else {
            (
                self.enemy.stat_stage(BattleStat::Accuracy),
                self.player.stat_stage(BattleStat::Evasion),
            )
        };
        if player {
            accuracy_stage =
                apply_accuracy_stage_booster(accuracy_stage, self.player_temp_stat_boosters[5]);
        }
        if check_move_accuracy(
            move_.accuracy(),
            accuracy_stage,
            evasion_stage,
            self.turn,
            &mut self.rng,
        ) == HitResult::Miss
        {
            self.append_log(format!(
                "{} used {}! It missed!",
                if player {
                    self.player.species.name()
                } else {
                    self.enemy.species.name()
                },
                move_.name()
            ));
            return;
        }
        let stage_effect = match move_.effect() {
            Some(MoveEffect::StatStageChange {
                stat,
                stages,
                chance,
            }) => Some((stat, stages, chance, false)),
            Some(MoveEffect::SelfStatStageChange { stat, stages }) => {
                Some((stat, stages, -1, true))
            }
            None => None,
        };
        if let Some((stat, stages, chance, self_target)) = stage_effect {
            if chance >= 0 && chance < 100 && self.rng.rand_int(self.turn, 100, 0) >= chance as i32
            {
                self.append_log(format!(
                    "{} used {}! The effect failed.",
                    if player {
                        self.player.species.name()
                    } else {
                        self.enemy.species.name()
                    },
                    move_.name()
                ));
                return;
            }
            let user_name = if player {
                self.player.species.name().to_owned()
            } else {
                self.enemy.species.name().to_owned()
            };
            let (target_name, applied, new_stage) = {
                let target = match (player, self_target) {
                    (true, false) => &mut self.enemy,
                    (false, false) => &mut self.player,
                    (true, true) => &mut self.player,
                    (false, true) => &mut self.enemy,
                };
                let applied = target.change_stat_stage(stat, stages);
                (
                    target.species.name().to_owned(),
                    applied,
                    target.stat_stage(stat),
                )
            };
            let message = if applied == 0 {
                format!(
                    "{user_name} used {}! {}'s {:?} cannot change further.",
                    move_.name(),
                    target_name,
                    stat
                )
            } else if stages > 0 {
                format!(
                    "{user_name} used {}! {}'s {:?} rose to {}.",
                    move_.name(),
                    target_name,
                    stat,
                    new_stage
                )
            } else {
                format!(
                    "{user_name} used {}! {}'s {:?} fell to {}.",
                    move_.name(),
                    target_name,
                    stat,
                    new_stage
                )
            };
            self.append_log(message);
            return;
        }
        if move_.is_status() {
            self.append_log(format!(
                "{} used {}!",
                if player {
                    self.player.species.name()
                } else {
                    self.enemy.species.name()
                },
                move_.name()
            ));
            return;
        }
        // Base critical stage in the pinned source is a 1/24 battle-seed roll.
        // Crit-stage modifiers, guaranteed crits and crit-blocking effects are
        // outside this supported battle slice.
        let is_critical = self.rng.rand_int(self.turn, 24, 0) == 0;
        let random_percent = self.rng.rand_int_inclusive(self.turn, 85, 100) as u8;
        let mut damage_result = {
            let (attacker, defender) = if player {
                (&*self.player, &mut self.enemy)
            } else {
                (&self.enemy, &mut *self.player)
            };
            let (attacker_boosters, defender_boosters) = if player {
                (self.player_temp_stat_boosters, [false; 6])
            } else {
                ([false; 6], self.player_temp_stat_boosters)
            };
            let mut result = calculate_damage_with_boosters_and_critical(
                attacker,
                defender,
                move_,
                random_percent,
                attacker_boosters,
                defender_boosters,
                is_critical,
            );
            result.damage = result.damage.min(defender.hp);
            result
        };
        let mut boss_boost_logs = Vec::new();
        if damage_result.damage > 0 {
            let minimum = if !player && self.enemy.boss_segments > 0 {
                1
            } else {
                0
            };
            let defender = if player {
                &mut self.enemy
            } else {
                &mut self.player
            };
            let (actual, boosts) = defender.apply_damage_with_rng(
                damage_result.damage,
                minimum,
                &mut self.rng,
                self.turn,
            );
            damage_result.damage = actual;
            for (stat, stages) in boosts {
                self.last_events
                    .push(BattleEvent::BossSegmentStatBoosted { stat, stages });
                let boss_name = if player {
                    self.enemy.species.name()
                } else {
                    self.player.species.name()
                };
                boss_boost_logs.push((boss_name, stat, stages));
            }
        }
        if is_critical {
            self.last_events.push(BattleEvent::CriticalHit {
                actor: if player {
                    BattleActor::Player
                } else {
                    BattleActor::Enemy
                },
                move_id: move_.numeric_id(),
            });
        }
        if damage_result.damage > 0 {
            self.last_events.push(BattleEvent::DamageApplied {
                target: if player {
                    BattleActor::Enemy
                } else {
                    BattleActor::Player
                },
                amount: damage_result.damage,
                critical: is_critical,
                effectiveness: if damage_result.type_multiplier >= 2.0 {
                    DamageEffectiveness::SuperEffective
                } else if damage_result.type_multiplier > 0.0
                    && damage_result.type_multiplier <= 0.5
                {
                    DamageEffectiveness::NotVeryEffective
                } else {
                    DamageEffectiveness::Normal
                },
            });
        }
        let (attacker, defender) = if player {
            (&*self.player, &self.enemy)
        } else {
            (&self.enemy, &*self.player)
        };
        let effectiveness_text = match damage_result.type_multiplier {
            0.0 => "No effect!".to_owned(),
            value if value >= 2.0 => format!("Super effective ×{value}!"),
            value if value <= 0.5 => format!("Not very effective ×{value}!"),
            _ => String::new(),
        };
        let message = if effectiveness_text.is_empty() {
            format!(
                "{} used {}! {} took {} damage",
                attacker.species.name(),
                move_.name(),
                defender.species.name(),
                damage_result.damage
            )
        } else if damage_result.type_multiplier == 0.0 {
            format!(
                "{} used {}! {}",
                attacker.species.name(),
                move_.name(),
                effectiveness_text
            )
        } else {
            format!(
                "{} used {}! {} {} took {} damage",
                attacker.species.name(),
                move_.name(),
                effectiveness_text,
                defender.species.name(),
                damage_result.damage
            )
        };
        self.append_log(message);
        for (name, stat, stages) in boss_boost_logs {
            let stat_name = match stat {
                BattleStat::Attack => "Attack",
                BattleStat::Defense => "Defense",
                BattleStat::SpecialAttack => "Sp. Atk",
                BattleStat::SpecialDefense => "Sp. Def",
                BattleStat::Speed => "Speed",
                _ => "Stat",
            };
            self.append_log(format!("{name}'s {stat_name} rose by {stages}!"));
        }
        if move_.is_typeless() && damage_result.damage > 0 {
            let user = if player {
                &mut self.player
            } else {
                &mut self.enemy
            };
            let recoil = ((f64::from(user.max_hp) * 0.25).floor() as i32).max(1);
            user.apply_damage(recoil, 0);
            self.append_log(format!(
                "{} was hurt by recoil!",
                if player {
                    self.player.species.name()
                } else {
                    self.enemy.species.name()
                }
            ));
        }
    }

    fn append_log(&mut self, message: String) {
        if !self.log.is_empty() {
            self.log.push('\n');
        }
        self.log.push_str(&message);
    }
}

/// Classic `getMaxExpLevel` for the source's ten-wave difficulty checkpoints.
pub fn classic_exp_level_cap(wave: u32, cap_override: i32, ignore_level_cap: bool) -> u8 {
    if cap_override > 0 {
        return cap_override.min(100) as u8;
    }
    if ignore_level_cap || cap_override < 0 {
        return 100;
    }
    let wave_index = wave.max(1).div_ceil(10) * 10;
    let wave = f64::from(wave_index);
    let base_level = (1.0 + wave / 2.0 + (wave / 25.0).powi(2)) * 1.2;
    ((base_level / 2.0).ceil() * 2.0 + 2.0)
        .ceil()
        .clamp(1.0, 100.0) as u8
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MiniRunStage {
    Battle,
    MoveLearning,
    Reward,
    Complete,
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingLevelUpPrompt {
    LearnMove { party_slot: usize, move_id: u16 },
    Prevolution { party_slot: usize, move_id: u16 },
}

fn pending_level_up_sequence(
    moves: &[(usize, u16)],
    prevolution: &[(usize, u16)],
    party_len: usize,
) -> Vec<PendingLevelUpPrompt> {
    let mut sequence = Vec::new();
    for slot in 0..party_len {
        sequence.extend(
            moves
                .iter()
                .filter(|(member_slot, _)| *member_slot == slot)
                .map(|(party_slot, move_id)| PendingLevelUpPrompt::LearnMove {
                    party_slot: *party_slot,
                    move_id: *move_id,
                }),
        );
        sequence.extend(
            prevolution
                .iter()
                .filter(|(member_slot, _)| *member_slot == slot)
                .map(|(party_slot, move_id)| PendingLevelUpPrompt::Prevolution {
                    party_slot: *party_slot,
                    move_id: *move_id,
                }),
        );
    }
    sequence
}

/// Apply the source `ExpBoosterModifier` formula for the supported 25% EXP Charm.
pub fn apply_exp_charm(exp: u32, stacks: u8) -> u32 {
    let stacks = stacks.min(99);
    (f64::from(exp) * (1.0 + f64::from(stacks) * 0.25)).floor() as u32
}

pub fn apply_exp_boosters(exp: u32, exp_charm_stacks: u8, super_exp_charm_stacks: u8) -> u32 {
    let after_exp_charm = apply_exp_charm(exp, exp_charm_stacks);
    (f64::from(after_exp_charm) * (1.0 + f64::from(super_exp_charm_stacks.min(30)) * 0.6)).floor()
        as u32
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClassicVictoryRewardSchedule {
    pub natural_modifier_offer: bool,
    pub exp_charm_count: u8,
    pub super_exp_charm_count: u8,
}

/// Bounded Classic reward branch from the pinned VictoryPhase.start for waves 1–20.
pub fn classic_victory_reward_schedule(wave: u32) -> ClassicVictoryRewardSchedule {
    let natural_modifier_offer = wave % 10 != 0;
    if natural_modifier_offer {
        return ClassicVictoryRewardSchedule {
            natural_modifier_offer: true,
            exp_charm_count: 0,
            super_exp_charm_count: 0,
        };
    }
    let special_ten_reward = u8::from(wave == 10);
    let periodic_booster = wave <= 750 && (wave <= 500 || wave % 30 == 20);
    let (periodic_exp_charm, periodic_super_charm) = if periodic_booster {
        if wave % 30 != 20 || wave > 250 {
            (1, 0)
        } else {
            (0, 1)
        }
    } else {
        (0, 0)
    };
    ClassicVictoryRewardSchedule {
        natural_modifier_offer: false,
        exp_charm_count: special_ten_reward + periodic_exp_charm,
        super_exp_charm_count: periodic_super_charm,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MiniRunError {
    Encounter(EncounterSelectionError),
    Turn(TurnActionError),
    NotAtReward,
    NoPokeballs,
    NoGreatBalls,
    NoUltraBalls,
    InvalidReward,
    RewardAlreadyClaimed,
    NoPendingMove,
    InvalidRewardTarget,
    UnsafeCheckpoint,
    InvalidCheckpoint,
}

#[derive(Clone, Debug)]
pub struct MiniRun {
    pub seed: String,
    pub time_of_day: String,
    pub wave: u8,
    /// Mini-run endpoint. New 10-wave runs can later be extended independently;
    /// legacy checkpoints retain their historical 20-wave endpoint.
    pub wave_limit: u8,
    pub stage: MiniRunStage,
    pub battle: Battle,
    pub reward_experience: u32,
    /// Number of permanent 25% EXP Charm stacks earned from the source-scheduled rewards.
    pub exp_charm_stacks: u8,
    pub exp_share_stacks: u8,
    pub amulet_coin_stacks: u8,
    pub super_exp_charm_stacks: u8,
    pub pokeballs: u8,
    pub greatballs: u8,
    pub ultraballs: u8,
    /// Local run currency; source MoneyReward modifiers use this wallet.
    pub money: u64,
    /// Remaining battles on the source 10-battle Lure modifier.
    pub lure_battles_remaining: u8,
    /// Remaining battles for the modeled X Accuracy generated reward.
    /// Source-ordered [ATK, DEF, SPATK, SPDEF, SPD, ACC] booster durations.
    pub temp_stat_booster_battles_remaining: [u8; 6],
    pub reward_claimed: bool,
}

impl MiniRun {
    pub const DEFAULT_WAVE_LIMIT: u8 = 10;
    pub const MAX_WAVE_LIMIT: u8 = 20;

    pub fn new(
        seed: impl Into<String>,
        time_of_day: impl Into<String>,
    ) -> Result<Self, MiniRunError> {
        Self::new_with_wave_limit(seed, time_of_day, Self::DEFAULT_WAVE_LIMIT)
    }

    pub fn new_with_wave_limit(
        seed: impl Into<String>,
        time_of_day: impl Into<String>,
        wave_limit: u8,
    ) -> Result<Self, MiniRunError> {
        if !(1..=Self::MAX_WAVE_LIMIT).contains(&wave_limit) {
            return Err(MiniRunError::InvalidCheckpoint);
        }
        let seed = seed.into();
        let time_of_day = time_of_day.into();
        let battle = Battle::town_encounter_for_wave(&seed, &time_of_day, 1)
            .map_err(MiniRunError::Encounter)?;
        Ok(Self {
            seed,
            time_of_day,
            wave: 1,
            wave_limit,
            stage: MiniRunStage::Battle,
            battle,
            reward_experience: 0,
            exp_charm_stacks: 0,
            exp_share_stacks: 0,
            amulet_coin_stacks: 0,
            super_exp_charm_stacks: 0,
            pokeballs: INITIAL_POKEBALL_COUNT,
            greatballs: 0,
            ultraballs: 0,
            // Pinned GameMode.getStartingMoney() returns 1000 for Classic.
            money: 1000,
            lure_battles_remaining: 0,
            temp_stat_booster_battles_remaining: [0; 6],
            reward_claimed: false,
        })
    }

    pub fn take_action(&mut self, action: PlayerAction) -> Result<(), MiniRunError> {
        if self.stage != MiniRunStage::Battle {
            return Err(MiniRunError::NotAtReward);
        }
        match action {
            PlayerAction::ThrowPokeball if self.pokeballs == 0 => {
                return Err(MiniRunError::NoPokeballs);
            }
            PlayerAction::ThrowGreatBall if self.greatballs == 0 => {
                return Err(MiniRunError::NoGreatBalls);
            }
            PlayerAction::ThrowUltraBall if self.ultraballs == 0 => {
                return Err(MiniRunError::NoUltraBalls);
            }
            _ => {}
        }
        self.battle.set_player_temp_stat_boosters(
            self.temp_stat_booster_battles_remaining
                .map(|remaining| remaining > 0),
        );
        self.battle.take_turn(action).map_err(MiniRunError::Turn)?;
        match action {
            PlayerAction::ThrowPokeball => self.pokeballs -= 1,
            PlayerAction::ThrowGreatBall => self.greatballs -= 1,
            PlayerAction::ThrowUltraBall => self.ultraballs -= 1,
            _ => {}
        }
        self.observe_outcome();
        Ok(())
    }

    pub fn replace_fainted_player(&mut self, slot: usize) -> Result<(), MiniRunError> {
        if self.stage != MiniRunStage::Battle {
            return Err(MiniRunError::NotAtReward);
        }
        self.battle
            .replace_fainted_player(slot)
            .map_err(|error| MiniRunError::Turn(TurnActionError::Switch(error)))?;
        self.observe_outcome();
        Ok(())
    }

    pub fn debug_sand_attack(&mut self) {
        if self.stage == MiniRunStage::Battle {
            self.battle.debug_sand_attack();
            self.observe_outcome();
        }
    }

    fn observe_outcome(&mut self) {
        if self.battle.outcome.is_some() {
            self.lure_battles_remaining =
                rewards::lapse_lure_battles(self.lure_battles_remaining).unwrap_or(0);
            self.temp_stat_booster_battles_remaining = self
                .temp_stat_booster_battles_remaining
                .map(|remaining| rewards::lapse_temp_stat_booster(remaining).unwrap_or(0));
        }
        match self.battle.outcome {
            Some(Outcome::PlayerWin) => {
                self.reward_experience = self
                    .battle
                    .award_victory_experience_with_exp_share(
                        self.exp_charm_stacks,
                        self.super_exp_charm_stacks,
                        self.exp_share_stacks,
                        classic_exp_level_cap(u32::from(self.wave), 0, false),
                    )
                    .iter()
                    .map(|(_, exp, _)| *exp)
                    .sum();
                let schedule = classic_victory_reward_schedule(u32::from(self.wave));
                self.exp_charm_stacks = self
                    .exp_charm_stacks
                    .saturating_add(schedule.exp_charm_count)
                    .min(99);
                self.super_exp_charm_stacks = self
                    .super_exp_charm_stacks
                    .saturating_add(schedule.super_exp_charm_count)
                    .min(30);
                self.reward_claimed = !schedule.natural_modifier_offer;
                self.stage = if self.battle.pending_level_moves.is_empty()
                    && self.battle.pending_prevolution_moves.is_empty()
                {
                    MiniRunStage::Reward
                } else {
                    MiniRunStage::MoveLearning
                };
            }
            Some(Outcome::PlayerLoss) => self.stage = MiniRunStage::Lost,
            None => {}
        }
    }

    pub fn pending_level_move(&self) -> Option<(usize, u16)> {
        match self.battle.pending_level_up_prompts.first().copied()? {
            PendingLevelUpPrompt::LearnMove {
                party_slot,
                move_id,
            } => Some((party_slot, move_id)),
            PendingLevelUpPrompt::Prevolution { .. } => None,
        }
    }

    pub fn pending_prevolution_move(&self) -> Option<(usize, u16)> {
        match self.battle.pending_level_up_prompts.first().copied()? {
            PendingLevelUpPrompt::Prevolution {
                party_slot,
                move_id,
            } => Some((party_slot, move_id)),
            PendingLevelUpPrompt::LearnMove { .. } => None,
        }
    }

    pub fn pending_level_up_prompt(&self) -> Option<PendingLevelUpPrompt> {
        self.battle.pending_level_up_prompts.first().copied()
    }

    pub fn acknowledge_prevolution_move(&mut self) -> Result<(), MiniRunError> {
        if self.stage != MiniRunStage::MoveLearning
            || !matches!(
                self.battle.pending_level_up_prompts.first(),
                Some(PendingLevelUpPrompt::Prevolution { .. })
            )
        {
            return Err(MiniRunError::NoPendingMove);
        }
        self.battle.pending_prevolution_moves.remove(0);
        self.battle.pending_level_up_prompts.remove(0);
        if self.battle.pending_level_up_prompts.is_empty() {
            self.stage = MiniRunStage::Reward;
        }
        Ok(())
    }

    /// Resolve the current LearnMovePhase. `None` declines; a slot index
    /// replaces that move, matching the source's full-moveset choice boundary.
    pub fn resolve_pending_level_move(
        &mut self,
        replace_slot: Option<usize>,
    ) -> Result<(), MiniRunError> {
        if self.stage != MiniRunStage::MoveLearning {
            return Err(MiniRunError::NoPendingMove);
        }
        let (party_slot, move_id) = self
            .pending_level_move()
            .ok_or(MiniRunError::NoPendingMove)?;
        let movement = pokerogue_data::move_by_numeric_id(move_id)
            .filter(|movement| !movement.is_unimplemented)
            .ok_or(MiniRunError::NoPendingMove)?;
        if let Some(index) = replace_slot {
            let member = self
                .battle
                .player
                .member_mut(party_slot)
                .ok_or(MiniRunError::InvalidRewardTarget)?;
            if !member.replace_move(index, Move::Imported(movement.numeric_id)) {
                return Err(MiniRunError::InvalidRewardTarget);
            }
            self.battle.last_events.push(BattleEvent::MoveLearned {
                party_slot,
                move_id,
            });
        }
        self.battle.pending_level_moves.remove(0);
        self.battle.pending_level_up_prompts.remove(0);
        loop {
            let Some((next_slot, next_id)) = self.pending_level_move() else {
                break;
            };
            let Some(next_move) = pokerogue_data::move_by_numeric_id(next_id) else {
                self.battle.pending_level_moves.remove(0);
                self.battle.pending_level_up_prompts.remove(0);
                continue;
            };
            let Some(member) = self.battle.player.member(next_slot) else {
                return Err(MiniRunError::InvalidRewardTarget);
            };
            let duplicate = member.moves[..member.move_count()]
                .iter()
                .any(|known| known.numeric_id() == next_id);
            if duplicate || next_move.is_unimplemented {
                self.battle.pending_level_moves.remove(0);
                self.battle.pending_level_up_prompts.remove(0);
            } else {
                break;
            }
        }
        if self.battle.pending_level_up_prompts.is_empty() {
            self.stage = MiniRunStage::Reward;
        }
        Ok(())
    }

    /// Source custom settings: three guaranteed functions, fillRemaining=false.
    pub fn reward_options(&self) -> &[rewards::Reward] {
        if self.stage == MiniRunStage::Reward && !self.reward_claimed {
            &rewards::GUARANTEED_REWARDS
        } else {
            &[]
        }
    }

    /// Raw source COMMON-pool callback weights from the currently modeled
    /// party and ball inventory. Lure inventory/status modifiers are not yet
    /// represented, so this bounded run currently supplies no active lures.
    pub fn common_reward_pool_weights(&self) -> [u32; 10] {
        let party: Vec<_> = (0..self.battle.player.len())
            .filter_map(|slot| self.battle.player.member(slot))
            .map(|member| rewards::RewardPartyMember {
                hp: member.hp,
                max_hp: member.max_hp,
                holds_leppa: false,
                move_pp: member
                    .moves
                    .iter()
                    .enumerate()
                    .map(|(index, movement)| {
                        (
                            u32::from(movement.pp()),
                            u32::from(member.move_pp_used[index]),
                        )
                    })
                    .collect(),
            })
            .collect();
        let lures = (self.lure_battles_remaining > 0)
            .then_some((
                u32::from(rewards::LURE_MAX_BATTLES),
                u32::from(self.lure_battles_remaining),
            ))
            .into_iter()
            .collect::<Vec<_>>();
        rewards::common_pool_weights(
            &party,
            true,
            u32::from(self.wave),
            u32::from(self.pokeballs),
            &lures,
        )
    }

    /// Build the source PLAYER pool weights for the current modeled Classic
    /// party. Eligibility requiring abilities, status, held items, forms,
    /// unlocks, lures, fusions or challenge state is currently absent from
    /// this run and is recorded as a parity limitation.
    pub fn modeled_player_modifier_tier_weights(&self, tier: u8) -> Option<Vec<u32>> {
        use rewards::{
            ExistingModifierStack, GreatRewardContext, GreatRewardPartyMember, UltraRewardContext,
        };
        let great_context = || {
            let party = (0..self.battle.player.len())
                .filter_map(|slot| self.battle.player.member(slot))
                .map(|member| {
                    let known: Vec<_> = member.moves.map(|move_| move_.numeric_id()).to_vec();
                    let has_learnable_level_moves =
                        species_by_numeric_id(member.species.numeric_id()).is_some_and(|species| {
                            level_moves_for(species.id, member.level)
                                .any(|entry| !known.contains(&entry.move_numeric_id))
                        });
                    GreatRewardPartyMember {
                        hp: member.hp,
                        max_hp: member.max_hp,
                        has_status: false,
                        status_protected: false,
                        holds_leppa: false,
                        move_pp: member
                            .moves
                            .iter()
                            .enumerate()
                            .map(|(index, movement)| {
                                (
                                    u32::from(movement.pp()),
                                    u32::from(member.move_pp_used[index]),
                                )
                            })
                            .collect(),
                        has_learnable_level_moves,
                        // All imported Town species are Tera-eligible except the
                        // three source-excluded species IDs.
                        tera_eligible: !matches!(member.species.numeric_id(), 292 | 1000 | 1017),
                        is_fused: false,
                    }
                })
                .collect();
            GreatRewardContext {
                classic: true,
                daily: false,
                spliced_only: false,
                endless: false,
                fresh_start_challenge: false,
                mini_black_hole_unlocked: false,
                wave: u32::from(self.wave),
                ball_counts: [self.pokeballs, self.greatballs, self.ultraballs, 0, 0],
                lures: Vec::new(),
                soothe_bell_boosted: false,
                fusions_boosted: false,
                party,
            }
        };
        let raw = match tier {
            0 => self.common_reward_pool_weights().to_vec(),
            1 => rewards::great_pool_weights(&great_context(), 0).to_vec(),
            2 => {
                rewards::ultra_pool_weights(&UltraRewardContext {
                    classic: true,
                    daily: false,
                    fresh_start_challenge: false,
                    // Source game-data initializes the Eviolite unlock false.
                    eviolite_unlocked: false,
                    wave: u32::from(self.wave),
                    ultra_ball_count: self.ultraballs,
                    max_lure_battles_remaining: None,
                    eviolite_eligible: false,
                    leek_eligible: false,
                    // Eligibility depends on ability/type/status predicates
                    // that Battler does not model yet. Do not infer it from
                    // unrelated move state.
                    toxic_orb_eligible: false,
                    flame_orb_eligible: false,
                    mystical_rock_eligible: false,
                })
                .to_vec()
            }
            3 => rewards::rogue_pool_weights(&great_context(), 0).to_vec(),
            4 => rewards::master_pool_weights(&great_context(), 0).to_vec(),
            _ => return None,
        };
        let existing = [
            ExistingModifierStack {
                id: "EXP_CHARM",
                stack: u32::from(self.exp_charm_stacks),
                cap: 99,
                is_held_item: false,
                is_form_change_item: false,
            },
            ExistingModifierStack {
                id: "EXP_SHARE",
                stack: u32::from(self.exp_share_stacks),
                cap: 5,
                is_held_item: false,
                is_form_change_item: false,
            },
            ExistingModifierStack {
                id: "AMULET_COIN",
                stack: u32::from(self.amulet_coin_stacks),
                cap: 5,
                is_held_item: false,
                is_form_change_item: false,
            },
        ];
        rewards::filter_modifier_pool_stack_caps(tier, &raw, &existing)
    }

    /// Roll one natural PLAYER reward candidate from this wave's reward RNG
    /// stream and current modeled party. The candidate is for oracle/debug
    /// inspection; it is not yet the selectable three-option offer.
    pub fn natural_reward_candidate(&self) -> Option<rewards::NaturalRewardCandidate> {
        rewards::roll_natural_player_candidate(&self.seed, u32::from(self.wave), 0, true, |tier| {
            self.modeled_player_modifier_tier_weights(tier)
                .unwrap_or_default()
        })
    }

    /// Return a source-seeded three-choice offer when every rolled option is
    /// in the generated COMMON subset whose identity and group are modeled.
    /// Higher tiers and their generated identities are deliberately withheld
    /// until their generators are ported; this never filters a candidate and
    /// presents a different tier as if it were equivalent.
    pub fn natural_common_reward_offer(&self) -> Option<rewards::NaturalRewardOffer> {
        self.natural_reward_offer_supported_through(0)
    }

    /// Natural offer limited to COMMON plus GREAT items whose effects are
    /// and the single ULTRA PP Max effect represented in this MiniRun slice.
    pub fn natural_supported_reward_offer(&self) -> Option<rewards::NaturalRewardOffer> {
        self.natural_reward_offer_supported_through(2)
    }

    fn natural_reward_offer_supported_through(
        &self,
        supported_tier: u8,
    ) -> Option<rewards::NaturalRewardOffer> {
        let compatible_tms =
            rewards::compatible_tms_for_party_excluding_used(&self.battle.player, 0);
        let compatible_great_tms =
            rewards::compatible_tms_for_party_excluding_used(&self.battle.player, 1);
        let offer = rewards::roll_natural_player_offer(
            &self.seed,
            u32::from(self.wave),
            0,
            true,
            3,
            15,
            |tier| {
                self.modeled_player_modifier_tier_weights(tier)
                    .unwrap_or_default()
            },
            |id, rng| match pokerogue_data::IMPORTED_PLAYER_MODIFIER_POOL
                .iter()
                .find(|entry| entry.id == id && entry.tier <= supported_tier)
                .map(|entry| entry.tier)
            {
                Some(0) => {
                    let identity =
                        rewards::resolve_common_player_modifier(id, rng, 0, &compatible_tms);
                    identity
                }
                Some(1) if id == "TERA_SHARD" => None,
                Some(1) if id == "TM_GREAT" => {
                    rewards::resolve_generated_modifier(id, rng, 0, &compatible_great_tms)
                }
                Some(1) => {
                    if let Some(identity) = rewards::resolve_great_direct_modifier(id) {
                        Some(identity)
                    } else {
                        Some(rewards::NaturalRewardIdentity {
                            name: id.to_owned(),
                            group: None,
                            generated_variant: None,
                        })
                    }
                }
                Some(2) if id == "TERA_SHARD" => None,
                Some(2) => {
                    if let Some(identity) = rewards::resolve_ultra_direct_modifier(id) {
                        Some(identity)
                    } else {
                        Some(rewards::NaturalRewardIdentity {
                            name: id.to_owned(),
                            group: None,
                            generated_variant: None,
                        })
                    }
                }
                _ => Some(rewards::NaturalRewardIdentity {
                    name: id.to_owned(),
                    group: None,
                    generated_variant: None,
                }),
            },
        )?;
        let supported_option = |option: &rewards::NaturalRewardOption| match option.id {
            "POKEBALL" | "RARE_CANDY" | "POTION" | "SUPER_POTION" | "ETHER" | "MAX_ETHER"
            | "LURE" => option.tier == 0,
            "TM_COMMON" | "TM_GREAT" => {
                option.tier == if option.id == "TM_COMMON" { 0 } else { 1 }
                    && option
                        .generated_variant
                        .as_deref()
                        .and_then(pokerogue_data::move_by_id)
                        .is_some()
            }
            "BERRY" => option.tier == 0 && option.generated_variant.as_deref() == Some("SITRUS"),
            "TEMP_STAT_STAGE_BOOSTER" => {
                option.tier == 0
                    && matches!(
                        option.generated_variant.as_deref(),
                        Some("ATK" | "DEF" | "SPATK" | "SPDEF" | "SPD" | "ACC")
                    )
            }
            "GREAT_BALL" | "NUGGET" | "ELIXIR" | "MAX_ELIXIR" | "HYPER_POTION" | "MAX_POTION"
            | "FULL_RESTORE" | "SOOTHE_BELL" | "REVIVE" | "MAX_REVIVE" | "PP_UP" => {
                option.tier == 1
            }
            "PP_MAX" | "ULTRA_BALL" | "EXP_CHARM" | "BIG_NUGGET" | "EXP_SHARE" | "AMULET_COIN" => {
                option.tier == 2
            }
            _ => false,
        };
        offer.options.iter().all(supported_option).then_some(offer)
    }

    /// Claim a supported direct-effect item from a reproducible COMMON offer.
    /// Generated COMMON effects without a matching run representation are
    /// rejected atomically and remain unavailable for selection.
    pub fn claim_natural_common_reward(
        &mut self,
        index: usize,
        target: usize,
    ) -> Result<(), MiniRunError> {
        if self.stage != MiniRunStage::Reward {
            return Err(MiniRunError::NotAtReward);
        }
        if self.reward_claimed {
            return Err(MiniRunError::RewardAlreadyClaimed);
        }
        self.claim_natural_reward_from_offer(index, target, None, false)
    }

    pub fn claim_natural_supported_reward(
        &mut self,
        index: usize,
        target: usize,
    ) -> Result<(), MiniRunError> {
        self.claim_natural_supported_reward_for_move(index, target, None)
    }

    pub fn claim_natural_supported_reward_for_move(
        &mut self,
        index: usize,
        target: usize,
        move_index: Option<usize>,
    ) -> Result<(), MiniRunError> {
        self.claim_natural_reward_from_offer(index, target, move_index, true)
    }

    fn claim_natural_reward_from_offer(
        &mut self,
        index: usize,
        target: usize,
        move_index: Option<usize>,
        include_great_recovery: bool,
    ) -> Result<(), MiniRunError> {
        if self.stage != MiniRunStage::Reward {
            return Err(MiniRunError::NotAtReward);
        }
        if self.reward_claimed {
            return Err(MiniRunError::RewardAlreadyClaimed);
        }
        let offer = if include_great_recovery {
            self.natural_supported_reward_offer()
        } else {
            self.natural_common_reward_offer()
        }
        .ok_or(MiniRunError::InvalidReward)?;
        let option = offer
            .options
            .get(index)
            .ok_or(MiniRunError::InvalidReward)?;
        if option.id == "TM_COMMON" || option.id == "TM_GREAT" {
            let tm_tier = if option.id == "TM_COMMON" { 0 } else { 1 };
            if option.tier != tm_tier {
                return Err(MiniRunError::InvalidReward);
            }
            let movement = option
                .generated_variant
                .as_deref()
                .and_then(pokerogue_data::move_by_id)
                .ok_or(MiniRunError::InvalidReward)?;
            let member = self
                .battle
                .player
                .member_mut(target)
                .ok_or(MiniRunError::InvalidRewardTarget)?;
            let known = member.moves[..member.move_count()]
                .iter()
                .map(|movement| movement.numeric_id())
                .collect::<Vec<_>>();
            let compatible = pokerogue_data::compatible_tms_for_species(
                member.species.numeric_id(),
                &known,
                &member.used_tm_ids,
                tm_tier,
            )
            .iter()
            .any(|candidate| candidate.numeric_id == movement.numeric_id);
            if !compatible || member.used_tm_ids.contains(&movement.numeric_id) {
                return Err(MiniRunError::InvalidRewardTarget);
            }
            let learned = if member.move_count() < 4 {
                member.learn_move(Move::Imported(movement.numeric_id))
            } else if let Some(move_index) = move_index {
                member.replace_move(move_index, Move::Imported(movement.numeric_id))
            } else {
                false
            };
            if !learned {
                return Err(MiniRunError::InvalidRewardTarget);
            }
            member.used_tm_ids.push(movement.numeric_id);
            self.reward_claimed = true;
            return Ok(());
        }
        let reward = match option.id {
            "POKEBALL" => rewards::Reward::Pokeballs,
            "GREAT_BALL" => rewards::Reward::GreatBalls,
            "PP_UP" => rewards::Reward::PpUp,
            "PP_MAX" => rewards::Reward::PpMax,
            "ULTRA_BALL" => rewards::Reward::UltraBalls,
            "EXP_CHARM" => rewards::Reward::ExpCharm,
            "EXP_SHARE" => rewards::Reward::ExpShare,
            "AMULET_COIN" => rewards::Reward::AmuletCoin,
            "BIG_NUGGET" => rewards::Reward::BigNugget,
            "NUGGET" => rewards::Reward::Nugget,
            "ELIXIR" => rewards::Reward::Elixir,
            "MAX_ELIXIR" => rewards::Reward::MaxElixir,
            "RARE_CANDY" => rewards::Reward::RareCandy,
            "POTION" => rewards::Reward::Potion,
            "SUPER_POTION" => rewards::Reward::SuperPotion,
            "ETHER" => rewards::Reward::Ether,
            "MAX_ETHER" => rewards::Reward::MaxEther,
            "HYPER_POTION" => rewards::Reward::HyperPotion,
            "MAX_POTION" => rewards::Reward::MaxPotion,
            "FULL_RESTORE" => rewards::Reward::FullRestore,
            "SOOTHE_BELL" => rewards::Reward::SootheBell,
            "REVIVE" => rewards::Reward::Revive,
            "MAX_REVIVE" => rewards::Reward::MaxRevive,
            "LURE" => rewards::Reward::Lure,
            "TEMP_STAT_STAGE_BOOSTER" if option.generated_variant.as_deref() == Some("ACC") => {
                rewards::Reward::TempAccuracy
            }
            "TEMP_STAT_STAGE_BOOSTER" if option.generated_variant.as_deref() == Some("SPD") => {
                rewards::Reward::TempSpeed
            }
            "TEMP_STAT_STAGE_BOOSTER" if option.generated_variant.as_deref() == Some("ATK") => {
                rewards::Reward::TempAttack
            }
            "TEMP_STAT_STAGE_BOOSTER" if option.generated_variant.as_deref() == Some("DEF") => {
                rewards::Reward::TempDefense
            }
            "TEMP_STAT_STAGE_BOOSTER" if option.generated_variant.as_deref() == Some("SPATK") => {
                rewards::Reward::TempSpecialAttack
            }
            "TEMP_STAT_STAGE_BOOSTER" if option.generated_variant.as_deref() == Some("SPDEF") => {
                rewards::Reward::TempSpecialDefense
            }
            "BERRY" if option.generated_variant.as_deref() == Some("SITRUS") => {
                rewards::Reward::SitrusBerry
            }
            _ => return Err(MiniRunError::InvalidReward),
        };
        self.apply_modifier_reward(reward, target, move_index)
    }

    pub fn claim_reward(&mut self, index: usize, target: usize) -> Result<(), MiniRunError> {
        if self.stage != MiniRunStage::Reward {
            return Err(MiniRunError::NotAtReward);
        }
        if self.reward_claimed {
            return Err(MiniRunError::RewardAlreadyClaimed);
        }
        let reward = *self
            .reward_options()
            .get(index)
            .ok_or(MiniRunError::InvalidReward)?;
        self.apply_modifier_reward(reward, target, None)?;
        self.reward_claimed = true;
        Ok(())
    }

    /// Apply the modeled direct effects shared by guaranteed and natural
    /// modifier offers. `move_index` is required only for single-move PP items.
    pub fn apply_modifier_reward(
        &mut self,
        reward: rewards::Reward,
        target: usize,
        move_index: Option<usize>,
    ) -> Result<(), MiniRunError> {
        if self.stage != MiniRunStage::Reward {
            return Err(MiniRunError::NotAtReward);
        }
        if self.reward_claimed {
            return Err(MiniRunError::RewardAlreadyClaimed);
        }
        match reward {
            rewards::Reward::Pokeballs => {
                self.pokeballs = self.pokeballs.saturating_add(5).min(MAX_POKEBALL_COUNT);
            }
            rewards::Reward::PpUp | rewards::Reward::PpMax => {
                let member = self
                    .battle
                    .player
                    .member_mut(target)
                    .ok_or(MiniRunError::InvalidRewardTarget)?;
                let index = move_index.ok_or(MiniRunError::InvalidRewardTarget)?;
                if !member.can_apply_pp_up(index) {
                    return Err(MiniRunError::InvalidRewardTarget);
                }
                let points = if reward == rewards::Reward::PpUp {
                    1
                } else {
                    3
                };
                member.move_pp_ups[index] = (member.move_pp_ups[index] + points).min(3);
            }
            rewards::Reward::GreatBalls => {
                self.greatballs = self.greatballs.saturating_add(5).min(MAX_POKEBALL_COUNT);
            }
            rewards::Reward::UltraBalls => {
                self.ultraballs = self.ultraballs.saturating_add(5).min(MAX_POKEBALL_COUNT);
            }
            rewards::Reward::ExpCharm => {
                self.exp_charm_stacks = self.exp_charm_stacks.saturating_add(1).min(99);
            }
            rewards::Reward::ExpShare => {
                self.exp_share_stacks = self.exp_share_stacks.saturating_add(1).min(5);
            }
            rewards::Reward::AmuletCoin => {
                self.amulet_coin_stacks = self.amulet_coin_stacks.saturating_add(1).min(5);
            }
            rewards::Reward::Nugget | rewards::Reward::BigNugget => {
                let multiplier = if reward == rewards::Reward::Nugget {
                    1.0
                } else {
                    2.5
                };
                let reward_money = classic_wave_money_amount(u32::from(self.wave), multiplier);
                self.money = self
                    .money
                    .saturating_add(apply_amulet_coin(reward_money, self.amulet_coin_stacks))
                    .min(MAX_RUN_MONEY);
            }
            rewards::Reward::Elixir | rewards::Reward::MaxElixir => {
                let member = self
                    .battle
                    .player
                    .member_mut(target)
                    .ok_or(MiniRunError::InvalidRewardTarget)?;
                if !member.move_pp_used.iter().any(|used| *used > 0) {
                    return Err(MiniRunError::InvalidRewardTarget);
                }
                let points = (reward == rewards::Reward::Elixir).then_some(10);
                member.restore_all_move_pp(points);
            }
            rewards::Reward::Lure => {
                // Source LapsingPersistentModifier.add refreshes matching
                // lures to their original duration rather than stacking them.
                if self.lure_battles_remaining >= rewards::LURE_MAX_BATTLES {
                    return Err(MiniRunError::InvalidRewardTarget);
                }
                self.lure_battles_remaining = rewards::LURE_MAX_BATTLES;
            }
            rewards::Reward::SootheBell => {
                let member = self
                    .battle
                    .player
                    .member_mut(target)
                    .ok_or(MiniRunError::InvalidRewardTarget)?;
                if member.soothe_bell_stacks >= 3 {
                    return Err(MiniRunError::InvalidRewardTarget);
                }
                member.soothe_bell_stacks += 1;
            }
            rewards::Reward::TempAccuracy
            | rewards::Reward::TempSpeed
            | rewards::Reward::TempAttack
            | rewards::Reward::TempDefense
            | rewards::Reward::TempSpecialAttack
            | rewards::Reward::TempSpecialDefense => {
                let index = reward_temp_booster_index(reward).ok_or(MiniRunError::InvalidReward)?;
                if self.temp_stat_booster_battles_remaining[index]
                    >= rewards::TEMP_STAT_BOOSTER_MAX_BATTLES
                {
                    return Err(MiniRunError::InvalidRewardTarget);
                }
                self.temp_stat_booster_battles_remaining[index] =
                    rewards::TEMP_STAT_BOOSTER_MAX_BATTLES;
            }
            rewards::Reward::SitrusBerry => {
                let member = self
                    .battle
                    .player
                    .member_mut(target)
                    .ok_or(MiniRunError::InvalidRewardTarget)?;
                if member.hp <= 0 || member.has_sitrus_berry {
                    return Err(MiniRunError::InvalidRewardTarget);
                }
                member.has_sitrus_berry = true;
            }
            rewards::Reward::Potion
            | rewards::Reward::SuperPotion
            | rewards::Reward::HyperPotion
            | rewards::Reward::MaxPotion
            | rewards::Reward::FullRestore => {
                let member = self
                    .battle
                    .player
                    .member_mut(target)
                    .ok_or(MiniRunError::InvalidRewardTarget)?;
                member.hp = reward
                    .heal(member.hp, member.max_hp)
                    .ok_or(MiniRunError::InvalidRewardTarget)?;
            }
            rewards::Reward::Revive | rewards::Reward::MaxRevive => {
                let member = self
                    .battle
                    .player
                    .member_mut(target)
                    .ok_or(MiniRunError::InvalidRewardTarget)?;
                member.hp = reward
                    .revive(member.hp, member.max_hp)
                    .ok_or(MiniRunError::InvalidRewardTarget)?;
            }
            rewards::Reward::RareCandy => {
                let member = self
                    .battle
                    .player
                    .member_mut(target)
                    .ok_or(MiniRunError::InvalidRewardTarget)?;
                if !member.rare_candy_level_up() {
                    return Err(MiniRunError::InvalidRewardTarget);
                }
            }
            rewards::Reward::Ether | rewards::Reward::MaxEther => {
                let member = self
                    .battle
                    .player
                    .member_mut(target)
                    .ok_or(MiniRunError::InvalidRewardTarget)?;
                let move_index = move_index.ok_or(MiniRunError::InvalidRewardTarget)?;
                let points = (reward == rewards::Reward::Ether).then_some(10);
                if !member.restore_move_pp(move_index, points) {
                    return Err(MiniRunError::InvalidRewardTarget);
                }
            }
        }
        self.reward_claimed = true;
        Ok(())
    }

    pub fn continue_after_reward(&mut self) -> Result<(), MiniRunError> {
        if self.stage != MiniRunStage::Reward {
            return Err(MiniRunError::NotAtReward);
        }
        if self.wave == self.wave_limit {
            self.stage = MiniRunStage::Complete;
            return Ok(());
        }
        self.wave += 1;
        self.battle = Battle::town_encounter_for_wave_with_team(
            self.seed.clone(),
            self.time_of_day.as_str(),
            self.wave as u32,
            self.battle.player.clone(),
        )
        .map_err(MiniRunError::Encounter)?;
        self.reward_experience = 0;
        self.reward_claimed = false;
        self.stage = MiniRunStage::Battle;
        Ok(())
    }

    /// Checkpoints are written only between waves; resuming an in-progress battle replays it.
    pub fn checkpoint(&self) -> Result<String, MiniRunError> {
        if !matches!(
            self.stage,
            MiniRunStage::MoveLearning | MiniRunStage::Reward | MiniRunStage::Complete
        ) {
            return Err(MiniRunError::UnsafeCheckpoint);
        }
        if !self
            .seed
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
        {
            return Err(MiniRunError::InvalidCheckpoint);
        }
        let mut output = format!(
            "POKEROGUE_MINIRUN\t27\nseed\t{}\ntime\t{}\nwave\t{}\nwave_limit\t{}\nstage\t{}\nreward_exp\t{}\nexp_charm_stacks\t{}\nexp_share_stacks\t{}\namulet_coin_stacks\t{}\nsuper_exp_charm_stacks\t{}\npokeballs\t{}\ngreatballs\t{}\nultraballs\t{}\nmoney\t{}\nlure_battles_remaining\t{}\ntemp_stat_booster_battles_remaining\t{},{},{},{},{},{}\nactive\t{}\nparty_count\t{}\n",
            self.seed,
            self.time_of_day,
            self.wave,
            self.wave_limit,
            match self.stage {
                MiniRunStage::MoveLearning => "learn_move",
                MiniRunStage::Reward => "reward",
                MiniRunStage::Complete => "complete",
                _ => unreachable!(),
            },
            self.reward_experience,
            self.exp_charm_stacks,
            self.exp_share_stacks,
            self.amulet_coin_stacks,
            self.super_exp_charm_stacks,
            self.pokeballs,
            self.greatballs,
            self.ultraballs,
            self.money,
            self.lure_battles_remaining,
            self.temp_stat_booster_battles_remaining[0],
            self.temp_stat_booster_battles_remaining[1],
            self.temp_stat_booster_battles_remaining[2],
            self.temp_stat_booster_battles_remaining[3],
            self.temp_stat_booster_battles_remaining[4],
            self.temp_stat_booster_battles_remaining[5],
            self.battle.player.active_slot(),
            self.battle.player.len(),
        );
        output.push_str(&format!(
            "reward_claimed\t{}\n",
            u8::from(self.reward_claimed)
        ));
        for (slot, move_id) in &self.battle.pending_level_moves {
            output.push_str(&format!("pending_move\t{slot},{move_id}\n"));
        }
        for (slot, move_id) in &self.battle.pending_prevolution_moves {
            output.push_str(&format!("pending_prevo\t{slot},{move_id}\n"));
        }
        for prompt in &self.battle.pending_level_up_prompts {
            let (kind, slot, move_id) = match prompt {
                PendingLevelUpPrompt::LearnMove {
                    party_slot,
                    move_id,
                } => ("learn", party_slot, move_id),
                PendingLevelUpPrompt::Prevolution {
                    party_slot,
                    move_id,
                } => ("prevo", party_slot, move_id),
            };
            output.push_str(&format!("pending_prompt\t{kind},{slot},{move_id}\n"));
        }
        for slot in 0..self.battle.player.len() {
            let member = self
                .battle
                .player
                .member(slot)
                .ok_or(MiniRunError::InvalidCheckpoint)?;
            output.push_str(&format!(
                "pokemon\t{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                member.species.numeric_id(),
                member.level,
                member.hp,
                member.exp,
                member.moves[0].numeric_id(),
                member.moves[1].numeric_id(),
                member.move_pp_used[0],
                member.move_pp_used[1],
                member.stat_stage(BattleStat::Accuracy),
                member.stat_stage(BattleStat::Evasion),
                member.stat_stage(BattleStat::Speed),
                member.stat_stage(BattleStat::Defense),
                member.stat_stage(BattleStat::Attack),
                member.stat_stage(BattleStat::SpecialAttack),
                member.stat_stage(BattleStat::SpecialDefense),
                u8::from(member.has_sitrus_berry),
                member.move_pp_ups[0] * 4 + member.move_pp_ups[1],
            ));
            output.push_str(&format!(
                "moveset\t{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                slot,
                member.move_count,
                member.moves[0].numeric_id(),
                member.moves[1].numeric_id(),
                member.moves[2].numeric_id(),
                member.moves[3].numeric_id(),
                member.move_pp_used[0],
                member.move_pp_used[1],
                member.move_pp_used[2],
                member.move_pp_used[3],
                member.move_pp_ups[0],
                member.move_pp_ups[1],
                member.move_pp_ups[2],
                member.move_pp_ups[3],
            ));
            output.push_str(&format!(
                "used_tms\t{},{}\n",
                slot,
                if member.used_tm_ids.is_empty() {
                    "-".to_owned()
                } else {
                    member
                        .used_tm_ids
                        .iter()
                        .map(u16::to_string)
                        .collect::<Vec<_>>()
                        .join("|")
                },
            ));
            output.push_str(&format!("friendship\t{},{}\n", slot, member.friendship));
            output.push_str(&format!(
                "soothe_bell\t{},{}\n",
                slot, member.soothe_bell_stacks
            ));
        }
        Ok(output)
    }

    pub fn from_checkpoint(input: &str) -> Result<Self, MiniRunError> {
        let mut lines = input.lines();
        let source_version = lines.next();
        let is_v27 = source_version == Some("POKEROGUE_MINIRUN\t27");
        let is_v26 = source_version == Some("POKEROGUE_MINIRUN\t26") || is_v27;
        let is_v25 = source_version == Some("POKEROGUE_MINIRUN\t25") || is_v26;
        // v25-v27 add optional pending level-up phases; older field layouts match v24.
        let version = if is_v25 {
            Some("POKEROGUE_MINIRUN\t24")
        } else {
            source_version
        };
        if !matches!(
            version,
            Some(
                "POKEROGUE_MINIRUN\t4"
                    | "POKEROGUE_MINIRUN\t5"
                    | "POKEROGUE_MINIRUN\t6"
                    | "POKEROGUE_MINIRUN\t7"
                    | "POKEROGUE_MINIRUN\t8"
                    | "POKEROGUE_MINIRUN\t9"
                    | "POKEROGUE_MINIRUN\t10"
                    | "POKEROGUE_MINIRUN\t11"
                    | "POKEROGUE_MINIRUN\t12"
                    | "POKEROGUE_MINIRUN\t13"
                    | "POKEROGUE_MINIRUN\t14"
                    | "POKEROGUE_MINIRUN\t15"
                    | "POKEROGUE_MINIRUN\t16"
                    | "POKEROGUE_MINIRUN\t17"
                    | "POKEROGUE_MINIRUN\t18"
                    | "POKEROGUE_MINIRUN\t19"
                    | "POKEROGUE_MINIRUN\t20"
                    | "POKEROGUE_MINIRUN\t21"
                    | "POKEROGUE_MINIRUN\t22"
                    | "POKEROGUE_MINIRUN\t23"
                    | "POKEROGUE_MINIRUN\t24"
            )
        ) {
            return Err(MiniRunError::InvalidCheckpoint);
        }
        let mut seed = None;
        let mut time = None;
        let mut wave = None;
        let mut wave_limit = None;
        let mut stage = None;
        let mut reward_exp = None;
        let mut exp_charm_stacks = None;
        let mut exp_share_stacks = None;
        let mut amulet_coin_stacks = None;
        let mut super_exp_charm_stacks = None;
        let mut pokeballs = None;
        let mut greatballs = None;
        let mut ultraballs = None;
        let mut money = None;
        let mut lure_battles_remaining = None;
        let mut legacy_accuracy_booster_battles_remaining = None;
        let mut legacy_speed_booster_battles_remaining = None;
        let mut temp_stat_booster_battles_remaining = None;
        let mut reward_claimed = if version == Some("POKEROGUE_MINIRUN\t4") {
            Some(true)
        } else {
            None
        };
        let mut active = None;
        let mut party_count = None;
        let mut members = Vec::new();
        let mut extended_movesets = Vec::new();
        let mut used_tm_records = Vec::new();
        let mut friendship_records = Vec::new();
        let mut soothe_bell_records = Vec::new();
        let mut pending_move_records = Vec::new();
        let mut pending_prevo_records = Vec::new();
        let mut pending_prompt_records = Vec::new();
        for line in lines {
            let (key, value) = line
                .split_once('\t')
                .ok_or(MiniRunError::InvalidCheckpoint)?;
            match key {
                "seed" => seed = Some(value.to_owned()),
                "time" => time = Some(value.to_owned()),
                "wave" => wave = value.parse::<u8>().ok(),
                "wave_limit" => wave_limit = value.parse::<u8>().ok(),
                "stage" => {
                    stage = match value {
                        "learn_move" if is_v25 => Some(MiniRunStage::MoveLearning),
                        "reward" => Some(MiniRunStage::Reward),
                        "complete" => Some(MiniRunStage::Complete),
                        _ => None,
                    }
                }
                "reward_exp" => reward_exp = value.parse::<u32>().ok(),
                "exp_charm_stacks" => exp_charm_stacks = value.parse::<u8>().ok(),
                "exp_share_stacks" => exp_share_stacks = value.parse::<u8>().ok(),
                "amulet_coin_stacks" => amulet_coin_stacks = value.parse::<u8>().ok(),
                "super_exp_charm_stacks" => super_exp_charm_stacks = value.parse::<u8>().ok(),
                "reward_claimed" => {
                    reward_claimed = match value {
                        "0" => Some(false),
                        "1" => Some(true),
                        _ => return Err(MiniRunError::InvalidCheckpoint),
                    }
                }
                "pokeballs" => pokeballs = value.parse::<u8>().ok(),
                "greatballs" => greatballs = value.parse::<u8>().ok(),
                "ultraballs" => ultraballs = value.parse::<u8>().ok(),
                "money" => money = value.parse::<u64>().ok(),
                "lure_battles_remaining" => lure_battles_remaining = value.parse::<u8>().ok(),
                "accuracy_booster_battles_remaining" => {
                    legacy_accuracy_booster_battles_remaining = value.parse::<u8>().ok()
                }
                "speed_booster_battles_remaining" => {
                    legacy_speed_booster_battles_remaining = value.parse::<u8>().ok()
                }
                "temp_stat_booster_battles_remaining" => {
                    let values: Vec<_> = value.split(',').collect();
                    if values.len() != 6 {
                        return Err(MiniRunError::InvalidCheckpoint);
                    }
                    let parsed = values
                        .iter()
                        .map(|value| value.parse::<u8>().ok())
                        .collect::<Option<Vec<_>>>()
                        .ok_or(MiniRunError::InvalidCheckpoint)?;
                    temp_stat_booster_battles_remaining = Some([
                        parsed[0], parsed[1], parsed[2], parsed[3], parsed[4], parsed[5],
                    ]);
                }
                "active" => active = value.parse::<usize>().ok(),
                "party_count" => party_count = value.parse::<usize>().ok(),
                "pokemon" => {
                    let values: Vec<_> = value.split(',').collect();
                    let current_format = version == Some("POKEROGUE_MINIRUN\t13")
                        || version == Some("POKEROGUE_MINIRUN\t14")
                        || version == Some("POKEROGUE_MINIRUN\t15")
                        || version == Some("POKEROGUE_MINIRUN\t16")
                        || version == Some("POKEROGUE_MINIRUN\t17")
                        || version == Some("POKEROGUE_MINIRUN\t18")
                        || version == Some("POKEROGUE_MINIRUN\t19")
                        || version == Some("POKEROGUE_MINIRUN\t20")
                        || version == Some("POKEROGUE_MINIRUN\t21")
                        || version == Some("POKEROGUE_MINIRUN\t22")
                        || version == Some("POKEROGUE_MINIRUN\t23")
                        || version == Some("POKEROGUE_MINIRUN\t24");
                    let sitrus_format = version == Some("POKEROGUE_MINIRUN\t14")
                        || version == Some("POKEROGUE_MINIRUN\t15")
                        || version == Some("POKEROGUE_MINIRUN\t16")
                        || version == Some("POKEROGUE_MINIRUN\t17")
                        || version == Some("POKEROGUE_MINIRUN\t18")
                        || version == Some("POKEROGUE_MINIRUN\t19")
                        || version == Some("POKEROGUE_MINIRUN\t20")
                        || version == Some("POKEROGUE_MINIRUN\t21")
                        || version == Some("POKEROGUE_MINIRUN\t22")
                        || version == Some("POKEROGUE_MINIRUN\t23")
                        || version == Some("POKEROGUE_MINIRUN\t24");
                    let pp_up_format = version == Some("POKEROGUE_MINIRUN\t17")
                        || version == Some("POKEROGUE_MINIRUN\t18")
                        || version == Some("POKEROGUE_MINIRUN\t19")
                        || version == Some("POKEROGUE_MINIRUN\t20")
                        || version == Some("POKEROGUE_MINIRUN\t21")
                        || version == Some("POKEROGUE_MINIRUN\t22")
                        || version == Some("POKEROGUE_MINIRUN\t23")
                        || version == Some("POKEROGUE_MINIRUN\t24");
                    if pp_up_format && values.len() != 17 {
                        return Err(MiniRunError::InvalidCheckpoint);
                    }
                    if sitrus_format && values.len() != 16 && !pp_up_format {
                        return Err(MiniRunError::InvalidCheckpoint);
                    }
                    if (current_format && values.len() != 16 && !pp_up_format)
                        || (!current_format && (values.len() == 16 || values.len() == 17))
                    {
                        return Err(MiniRunError::InvalidCheckpoint);
                    }
                    if values.len() != 9
                        && values.len() != 11
                        && values.len() != 12
                        && values.len() != 15
                        && values.len() != 16
                        && values.len() != 17
                    {
                        return Err(MiniRunError::InvalidCheckpoint);
                    }
                    let number = |index: usize| values[index].parse::<i32>().ok();
                    let id = number(0)
                        .and_then(|n| u16::try_from(n).ok())
                        .ok_or(MiniRunError::InvalidCheckpoint)?;
                    let level = number(1)
                        .and_then(|n| u8::try_from(n).ok())
                        .filter(|n| (1..=100).contains(n))
                        .ok_or(MiniRunError::InvalidCheckpoint)?;
                    let hp = number(2).ok_or(MiniRunError::InvalidCheckpoint)?;
                    let exp = values[3]
                        .parse::<u32>()
                        .map_err(|_| MiniRunError::InvalidCheckpoint)?;
                    let moves = [number(4), number(5)].map(|n| {
                        n.and_then(|v| u16::try_from(v).ok())
                            .ok_or(MiniRunError::InvalidCheckpoint)
                    });
                    let moves = [moves[0]?, moves[1]?];
                    let (move_pp_used, stage_offset) = if values.len() >= 11 {
                        let used = [values[6], values[7]].map(|value| {
                            value
                                .parse::<u8>()
                                .map_err(|_| MiniRunError::InvalidCheckpoint)
                        });
                        ([used[0]?, used[1]?], 8)
                    } else {
                        ([0; 2], 6)
                    };
                    let mut stages = [0; 7];
                    let serialized_stage_count = if values.len() == 16 || values.len() == 17 {
                        7
                    } else {
                        values.len() - stage_offset
                    };
                    for (index, stage) in stages.iter_mut().take(serialized_stage_count).enumerate()
                    {
                        *stage = number(stage_offset + index)
                            .and_then(|value| i8::try_from(value).ok())
                            .ok_or(MiniRunError::InvalidCheckpoint)?;
                    }
                    let has_sitrus_berry = if values.len() == 16 {
                        match values[15] {
                            "0" => false,
                            "1" => true,
                            _ => return Err(MiniRunError::InvalidCheckpoint),
                        }
                    } else if values.len() == 17 {
                        match values[15] {
                            "0" => false,
                            "1" => true,
                            _ => return Err(MiniRunError::InvalidCheckpoint),
                        }
                    } else {
                        false
                    };
                    let move_pp_ups = if pp_up_format {
                        let packed = values[16]
                            .parse::<u8>()
                            .map_err(|_| MiniRunError::InvalidCheckpoint)?;
                        if packed >= 16 {
                            return Err(MiniRunError::InvalidCheckpoint);
                        }
                        [packed / 4, packed % 4]
                    } else {
                        [0; 2]
                    };
                    members.push(
                        Battler::restore_checkpoint(
                            id,
                            level,
                            hp,
                            exp,
                            moves,
                            move_pp_used,
                            move_pp_ups,
                            stages,
                            has_sitrus_berry,
                        )
                        .ok_or(MiniRunError::InvalidCheckpoint)?,
                    );
                }
                "moveset" => {
                    if version != Some("POKEROGUE_MINIRUN\t21")
                        && version != Some("POKEROGUE_MINIRUN\t22")
                        && version != Some("POKEROGUE_MINIRUN\t23")
                        && version != Some("POKEROGUE_MINIRUN\t24")
                    {
                        return Err(MiniRunError::InvalidCheckpoint);
                    }
                    let values: Vec<_> = value.split(',').collect();
                    if values.len() != 14 {
                        return Err(MiniRunError::InvalidCheckpoint);
                    }
                    let slot = values[0]
                        .parse::<usize>()
                        .map_err(|_| MiniRunError::InvalidCheckpoint)?;
                    let count = values[1]
                        .parse::<u8>()
                        .map_err(|_| MiniRunError::InvalidCheckpoint)?;
                    let ids = values[2..6]
                        .iter()
                        .map(|value| {
                            value
                                .parse::<u16>()
                                .map_err(|_| MiniRunError::InvalidCheckpoint)
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let pp_used = values[6..10]
                        .iter()
                        .map(|value| {
                            value
                                .parse::<u8>()
                                .map_err(|_| MiniRunError::InvalidCheckpoint)
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let pp_ups = values[10..14]
                        .iter()
                        .map(|value| {
                            value
                                .parse::<u8>()
                                .map_err(|_| MiniRunError::InvalidCheckpoint)
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    extended_movesets.push((
                        slot,
                        count,
                        [ids[0], ids[1], ids[2], ids[3]],
                        [pp_used[0], pp_used[1], pp_used[2], pp_used[3]],
                        [pp_ups[0], pp_ups[1], pp_ups[2], pp_ups[3]],
                    ));
                }
                "used_tms" => {
                    if !matches!(
                        version,
                        Some(
                            "POKEROGUE_MINIRUN\t22"
                                | "POKEROGUE_MINIRUN\t23"
                                | "POKEROGUE_MINIRUN\t24"
                        )
                    ) {
                        return Err(MiniRunError::InvalidCheckpoint);
                    }
                    let (slot, ids) = value
                        .split_once(',')
                        .ok_or(MiniRunError::InvalidCheckpoint)?;
                    let slot = slot
                        .parse::<usize>()
                        .map_err(|_| MiniRunError::InvalidCheckpoint)?;
                    let ids = if ids == "-" {
                        Vec::new()
                    } else {
                        ids.split('|')
                            .map(|value| {
                                value
                                    .parse::<u16>()
                                    .ok()
                                    .filter(|id| pokerogue_data::move_by_numeric_id(*id).is_some())
                                    .ok_or(MiniRunError::InvalidCheckpoint)
                            })
                            .collect::<Result<Vec<_>, _>>()?
                    };
                    if ids
                        .iter()
                        .copied()
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                        != ids.len()
                    {
                        return Err(MiniRunError::InvalidCheckpoint);
                    }
                    used_tm_records.push((slot, ids));
                }
                "friendship" => {
                    if !matches!(
                        version,
                        Some("POKEROGUE_MINIRUN\t23" | "POKEROGUE_MINIRUN\t24")
                    ) {
                        return Err(MiniRunError::InvalidCheckpoint);
                    }
                    let (slot, friendship) = value
                        .split_once(',')
                        .ok_or(MiniRunError::InvalidCheckpoint)?;
                    friendship_records.push((
                        slot.parse::<usize>()
                            .map_err(|_| MiniRunError::InvalidCheckpoint)?,
                        friendship
                            .parse::<u8>()
                            .map_err(|_| MiniRunError::InvalidCheckpoint)?,
                    ));
                }
                "soothe_bell" => {
                    if version != Some("POKEROGUE_MINIRUN\t24") {
                        return Err(MiniRunError::InvalidCheckpoint);
                    }
                    let (slot, stacks) = value
                        .split_once(',')
                        .ok_or(MiniRunError::InvalidCheckpoint)?;
                    let stacks = stacks
                        .parse::<u8>()
                        .ok()
                        .filter(|stacks| *stacks <= 3)
                        .ok_or(MiniRunError::InvalidCheckpoint)?;
                    soothe_bell_records.push((
                        slot.parse::<usize>()
                            .map_err(|_| MiniRunError::InvalidCheckpoint)?,
                        stacks,
                    ));
                }
                "pending_move" => {
                    if !is_v25 {
                        return Err(MiniRunError::InvalidCheckpoint);
                    }
                    let (slot, move_id) = value
                        .split_once(',')
                        .ok_or(MiniRunError::InvalidCheckpoint)?;
                    pending_move_records.push((
                        slot.parse::<usize>()
                            .map_err(|_| MiniRunError::InvalidCheckpoint)?,
                        move_id
                            .parse::<u16>()
                            .map_err(|_| MiniRunError::InvalidCheckpoint)?,
                    ));
                }
                "pending_prevo" => {
                    if !is_v26 {
                        return Err(MiniRunError::InvalidCheckpoint);
                    }
                    let (slot, move_id) = value
                        .split_once(',')
                        .ok_or(MiniRunError::InvalidCheckpoint)?;
                    pending_prevo_records.push((
                        slot.parse::<usize>()
                            .map_err(|_| MiniRunError::InvalidCheckpoint)?,
                        move_id
                            .parse::<u16>()
                            .map_err(|_| MiniRunError::InvalidCheckpoint)?,
                    ));
                }
                "pending_prompt" => {
                    if !is_v27 {
                        return Err(MiniRunError::InvalidCheckpoint);
                    }
                    let mut fields = value.split(',');
                    let kind = fields.next().ok_or(MiniRunError::InvalidCheckpoint)?;
                    let slot = fields
                        .next()
                        .ok_or(MiniRunError::InvalidCheckpoint)?
                        .parse::<usize>()
                        .map_err(|_| MiniRunError::InvalidCheckpoint)?;
                    let move_id = fields
                        .next()
                        .ok_or(MiniRunError::InvalidCheckpoint)?
                        .parse::<u16>()
                        .map_err(|_| MiniRunError::InvalidCheckpoint)?;
                    if fields.next().is_some() {
                        return Err(MiniRunError::InvalidCheckpoint);
                    }
                    pending_prompt_records.push(match kind {
                        "learn" => PendingLevelUpPrompt::LearnMove {
                            party_slot: slot,
                            move_id,
                        },
                        "prevo" => PendingLevelUpPrompt::Prevolution {
                            party_slot: slot,
                            move_id,
                        },
                        _ => return Err(MiniRunError::InvalidCheckpoint),
                    });
                }
                _ => return Err(MiniRunError::InvalidCheckpoint),
            }
        }
        let seed = seed.ok_or(MiniRunError::InvalidCheckpoint)?;
        let time_of_day = time.ok_or(MiniRunError::InvalidCheckpoint)?;
        let wave = wave
            .filter(|n| (1..=Self::MAX_WAVE_LIMIT).contains(n))
            .ok_or(MiniRunError::InvalidCheckpoint)?;
        let wave_limit = match (version, wave_limit) {
            (
                Some(
                    "POKEROGUE_MINIRUN\t14"
                    | "POKEROGUE_MINIRUN\t15"
                    | "POKEROGUE_MINIRUN\t16"
                    | "POKEROGUE_MINIRUN\t17"
                    | "POKEROGUE_MINIRUN\t18"
                    | "POKEROGUE_MINIRUN\t19"
                    | "POKEROGUE_MINIRUN\t20"
                    | "POKEROGUE_MINIRUN\t21"
                    | "POKEROGUE_MINIRUN\t22"
                    | "POKEROGUE_MINIRUN\t23"
                    | "POKEROGUE_MINIRUN\t24",
                ),
                Some(limit),
            ) if (1..=Self::MAX_WAVE_LIMIT).contains(&limit) => limit,
            (
                Some(
                    "POKEROGUE_MINIRUN\t14"
                    | "POKEROGUE_MINIRUN\t15"
                    | "POKEROGUE_MINIRUN\t16"
                    | "POKEROGUE_MINIRUN\t17"
                    | "POKEROGUE_MINIRUN\t18"
                    | "POKEROGUE_MINIRUN\t19"
                    | "POKEROGUE_MINIRUN\t20"
                    | "POKEROGUE_MINIRUN\t21"
                    | "POKEROGUE_MINIRUN\t22"
                    | "POKEROGUE_MINIRUN\t23"
                    | "POKEROGUE_MINIRUN\t24",
                ),
                _,
            ) => {
                return Err(MiniRunError::InvalidCheckpoint);
            }
            (_, None) => Self::MAX_WAVE_LIMIT,
            (_, Some(_)) => return Err(MiniRunError::InvalidCheckpoint),
        };
        if wave > wave_limit {
            return Err(MiniRunError::InvalidCheckpoint);
        }
        let stage = stage.ok_or(MiniRunError::InvalidCheckpoint)?;
        let reward_experience = reward_exp.ok_or(MiniRunError::InvalidCheckpoint)?;
        let exp_charm_stacks = exp_charm_stacks
            .filter(|stacks| *stacks <= 99)
            .ok_or(MiniRunError::InvalidCheckpoint)?;
        let super_exp_charm_stacks = match (version, super_exp_charm_stacks) {
            (
                Some(
                    "POKEROGUE_MINIRUN\t8"
                    | "POKEROGUE_MINIRUN\t9"
                    | "POKEROGUE_MINIRUN\t10"
                    | "POKEROGUE_MINIRUN\t11"
                    | "POKEROGUE_MINIRUN\t12"
                    | "POKEROGUE_MINIRUN\t13"
                    | "POKEROGUE_MINIRUN\t14"
                    | "POKEROGUE_MINIRUN\t15"
                    | "POKEROGUE_MINIRUN\t16"
                    | "POKEROGUE_MINIRUN\t17"
                    | "POKEROGUE_MINIRUN\t18"
                    | "POKEROGUE_MINIRUN\t19"
                    | "POKEROGUE_MINIRUN\t20"
                    | "POKEROGUE_MINIRUN\t21"
                    | "POKEROGUE_MINIRUN\t22"
                    | "POKEROGUE_MINIRUN\t23"
                    | "POKEROGUE_MINIRUN\t24",
                ),
                Some(stacks),
            ) if stacks <= 30 => stacks,
            (
                Some(
                    "POKEROGUE_MINIRUN\t8"
                    | "POKEROGUE_MINIRUN\t9"
                    | "POKEROGUE_MINIRUN\t10"
                    | "POKEROGUE_MINIRUN\t11"
                    | "POKEROGUE_MINIRUN\t12"
                    | "POKEROGUE_MINIRUN\t13"
                    | "POKEROGUE_MINIRUN\t14"
                    | "POKEROGUE_MINIRUN\t15"
                    | "POKEROGUE_MINIRUN\t16"
                    | "POKEROGUE_MINIRUN\t17"
                    | "POKEROGUE_MINIRUN\t18"
                    | "POKEROGUE_MINIRUN\t19"
                    | "POKEROGUE_MINIRUN\t20"
                    | "POKEROGUE_MINIRUN\t21"
                    | "POKEROGUE_MINIRUN\t22"
                    | "POKEROGUE_MINIRUN\t23"
                    | "POKEROGUE_MINIRUN\t24",
                ),
                _,
            ) => {
                return Err(MiniRunError::InvalidCheckpoint);
            }
            (_, None) => 0,
            (_, Some(_)) => return Err(MiniRunError::InvalidCheckpoint),
        };
        let pokeballs = pokeballs
            .filter(|count| *count <= MAX_POKEBALL_COUNT)
            .ok_or(MiniRunError::InvalidCheckpoint)?;
        let greatballs = match (version, greatballs) {
            (
                Some(
                    "POKEROGUE_MINIRUN\t15"
                    | "POKEROGUE_MINIRUN\t16"
                    | "POKEROGUE_MINIRUN\t17"
                    | "POKEROGUE_MINIRUN\t18"
                    | "POKEROGUE_MINIRUN\t19"
                    | "POKEROGUE_MINIRUN\t20"
                    | "POKEROGUE_MINIRUN\t21"
                    | "POKEROGUE_MINIRUN\t22"
                    | "POKEROGUE_MINIRUN\t23"
                    | "POKEROGUE_MINIRUN\t24",
                ),
                Some(count),
            ) if count <= MAX_POKEBALL_COUNT => count,
            (
                Some(
                    "POKEROGUE_MINIRUN\t15"
                    | "POKEROGUE_MINIRUN\t16"
                    | "POKEROGUE_MINIRUN\t17"
                    | "POKEROGUE_MINIRUN\t18"
                    | "POKEROGUE_MINIRUN\t19"
                    | "POKEROGUE_MINIRUN\t20"
                    | "POKEROGUE_MINIRUN\t21"
                    | "POKEROGUE_MINIRUN\t22"
                    | "POKEROGUE_MINIRUN\t23"
                    | "POKEROGUE_MINIRUN\t24",
                ),
                _,
            )
            | (_, Some(_)) => {
                return Err(MiniRunError::InvalidCheckpoint);
            }
            (_, None) => 0,
        };
        let exp_share_stacks = match (version, exp_share_stacks) {
            (
                Some(
                    "POKEROGUE_MINIRUN\t19"
                    | "POKEROGUE_MINIRUN\t20"
                    | "POKEROGUE_MINIRUN\t21"
                    | "POKEROGUE_MINIRUN\t22"
                    | "POKEROGUE_MINIRUN\t23"
                    | "POKEROGUE_MINIRUN\t24",
                ),
                Some(stacks),
            ) if stacks <= 5 => stacks,
            (
                Some(
                    "POKEROGUE_MINIRUN\t19"
                    | "POKEROGUE_MINIRUN\t20"
                    | "POKEROGUE_MINIRUN\t21"
                    | "POKEROGUE_MINIRUN\t22"
                    | "POKEROGUE_MINIRUN\t23"
                    | "POKEROGUE_MINIRUN\t24",
                ),
                _,
            )
            | (_, Some(_)) => {
                return Err(MiniRunError::InvalidCheckpoint);
            }
            (_, None) => 0,
        };
        let amulet_coin_stacks = match (version, amulet_coin_stacks) {
            (
                Some(
                    "POKEROGUE_MINIRUN\t20"
                    | "POKEROGUE_MINIRUN\t21"
                    | "POKEROGUE_MINIRUN\t22"
                    | "POKEROGUE_MINIRUN\t23"
                    | "POKEROGUE_MINIRUN\t24",
                ),
                Some(stacks),
            ) if stacks <= 5 => stacks,
            (
                Some(
                    "POKEROGUE_MINIRUN\t20"
                    | "POKEROGUE_MINIRUN\t21"
                    | "POKEROGUE_MINIRUN\t22"
                    | "POKEROGUE_MINIRUN\t23"
                    | "POKEROGUE_MINIRUN\t24",
                ),
                _,
            ) => {
                return Err(MiniRunError::InvalidCheckpoint);
            }
            (Some(_), None) => 0,
            _ => return Err(MiniRunError::InvalidCheckpoint),
        };
        let ultraballs = match (version, ultraballs) {
            (
                Some(
                    "POKEROGUE_MINIRUN\t18"
                    | "POKEROGUE_MINIRUN\t19"
                    | "POKEROGUE_MINIRUN\t20"
                    | "POKEROGUE_MINIRUN\t21"
                    | "POKEROGUE_MINIRUN\t22"
                    | "POKEROGUE_MINIRUN\t23"
                    | "POKEROGUE_MINIRUN\t24",
                ),
                Some(count),
            ) if count <= MAX_POKEBALL_COUNT => count,
            (
                Some(
                    "POKEROGUE_MINIRUN\t18"
                    | "POKEROGUE_MINIRUN\t19"
                    | "POKEROGUE_MINIRUN\t20"
                    | "POKEROGUE_MINIRUN\t21"
                    | "POKEROGUE_MINIRUN\t22"
                    | "POKEROGUE_MINIRUN\t23"
                    | "POKEROGUE_MINIRUN\t24",
                ),
                _,
            )
            | (_, Some(_)) => {
                return Err(MiniRunError::InvalidCheckpoint);
            }
            (_, None) => 0,
        };
        let money = match (version, money) {
            (
                Some(
                    "POKEROGUE_MINIRUN\t16"
                    | "POKEROGUE_MINIRUN\t17"
                    | "POKEROGUE_MINIRUN\t18"
                    | "POKEROGUE_MINIRUN\t19"
                    | "POKEROGUE_MINIRUN\t20"
                    | "POKEROGUE_MINIRUN\t21"
                    | "POKEROGUE_MINIRUN\t22"
                    | "POKEROGUE_MINIRUN\t23"
                    | "POKEROGUE_MINIRUN\t24",
                ),
                Some(amount),
            ) if amount <= MAX_RUN_MONEY => amount,
            (
                Some(
                    "POKEROGUE_MINIRUN\t16"
                    | "POKEROGUE_MINIRUN\t17"
                    | "POKEROGUE_MINIRUN\t18"
                    | "POKEROGUE_MINIRUN\t19"
                    | "POKEROGUE_MINIRUN\t20"
                    | "POKEROGUE_MINIRUN\t21"
                    | "POKEROGUE_MINIRUN\t22"
                    | "POKEROGUE_MINIRUN\t23"
                    | "POKEROGUE_MINIRUN\t24",
                ),
                _,
            )
            | (_, Some(_)) => {
                return Err(MiniRunError::InvalidCheckpoint);
            }
            (_, None) => 0,
        };
        let lure_battles_remaining = match (version, lure_battles_remaining) {
            (
                Some(
                    "POKEROGUE_MINIRUN\t9"
                    | "POKEROGUE_MINIRUN\t10"
                    | "POKEROGUE_MINIRUN\t11"
                    | "POKEROGUE_MINIRUN\t12"
                    | "POKEROGUE_MINIRUN\t13"
                    | "POKEROGUE_MINIRUN\t14"
                    | "POKEROGUE_MINIRUN\t15"
                    | "POKEROGUE_MINIRUN\t16"
                    | "POKEROGUE_MINIRUN\t17"
                    | "POKEROGUE_MINIRUN\t18"
                    | "POKEROGUE_MINIRUN\t19"
                    | "POKEROGUE_MINIRUN\t20"
                    | "POKEROGUE_MINIRUN\t21"
                    | "POKEROGUE_MINIRUN\t22"
                    | "POKEROGUE_MINIRUN\t23"
                    | "POKEROGUE_MINIRUN\t24",
                ),
                Some(count),
            ) if count <= rewards::LURE_MAX_BATTLES => count,
            (
                Some(
                    "POKEROGUE_MINIRUN\t9"
                    | "POKEROGUE_MINIRUN\t10"
                    | "POKEROGUE_MINIRUN\t11"
                    | "POKEROGUE_MINIRUN\t12",
                ),
                _,
            ) => {
                return Err(MiniRunError::InvalidCheckpoint);
            }
            (_, None) => 0,
            (_, Some(_)) => return Err(MiniRunError::InvalidCheckpoint),
        };
        let temp_stat_booster_battles_remaining = match (
            version,
            temp_stat_booster_battles_remaining,
            legacy_accuracy_booster_battles_remaining,
            legacy_speed_booster_battles_remaining,
        ) {
            (
                Some(
                    "POKEROGUE_MINIRUN\t12"
                    | "POKEROGUE_MINIRUN\t13"
                    | "POKEROGUE_MINIRUN\t14"
                    | "POKEROGUE_MINIRUN\t15"
                    | "POKEROGUE_MINIRUN\t16"
                    | "POKEROGUE_MINIRUN\t17"
                    | "POKEROGUE_MINIRUN\t18"
                    | "POKEROGUE_MINIRUN\t19"
                    | "POKEROGUE_MINIRUN\t20"
                    | "POKEROGUE_MINIRUN\t21"
                    | "POKEROGUE_MINIRUN\t22"
                    | "POKEROGUE_MINIRUN\t23"
                    | "POKEROGUE_MINIRUN\t24",
                ),
                Some(counts),
                None,
                None,
            ) if counts
                .iter()
                .all(|count| *count <= rewards::TEMP_STAT_BOOSTER_MAX_BATTLES) =>
            {
                counts
            }
            (
                Some(
                    "POKEROGUE_MINIRUN\t12"
                    | "POKEROGUE_MINIRUN\t13"
                    | "POKEROGUE_MINIRUN\t14"
                    | "POKEROGUE_MINIRUN\t15"
                    | "POKEROGUE_MINIRUN\t16"
                    | "POKEROGUE_MINIRUN\t17"
                    | "POKEROGUE_MINIRUN\t18"
                    | "POKEROGUE_MINIRUN\t19"
                    | "POKEROGUE_MINIRUN\t20"
                    | "POKEROGUE_MINIRUN\t21"
                    | "POKEROGUE_MINIRUN\t22"
                    | "POKEROGUE_MINIRUN\t23"
                    | "POKEROGUE_MINIRUN\t24",
                ),
                _,
                _,
                _,
            ) => {
                return Err(MiniRunError::InvalidCheckpoint);
            }
            (Some("POKEROGUE_MINIRUN\t11"), None, Some(acc), Some(speed))
                if acc <= rewards::TEMP_STAT_BOOSTER_MAX_BATTLES
                    && speed <= rewards::TEMP_STAT_BOOSTER_MAX_BATTLES =>
            {
                [0, 0, 0, 0, speed, acc]
            }
            (Some("POKEROGUE_MINIRUN\t11"), _, _, _) => {
                return Err(MiniRunError::InvalidCheckpoint);
            }
            (Some("POKEROGUE_MINIRUN\t10"), None, Some(acc), None)
                if acc <= rewards::TEMP_STAT_BOOSTER_MAX_BATTLES =>
            {
                [0, 0, 0, 0, 0, acc]
            }
            (Some("POKEROGUE_MINIRUN\t10"), _, _, _) => {
                return Err(MiniRunError::InvalidCheckpoint);
            }
            (_, None, None, None) => [0; 6],
            _ => return Err(MiniRunError::InvalidCheckpoint),
        };
        let active = active.ok_or(MiniRunError::InvalidCheckpoint)?;
        if members.len() != party_count.ok_or(MiniRunError::InvalidCheckpoint)?
            || members.is_empty()
            || members.len() > PLAYER_PARTY_MAX_SIZE
            || !seed
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
        {
            return Err(MiniRunError::InvalidCheckpoint);
        }
        if version == Some("POKEROGUE_MINIRUN\t21")
            || version == Some("POKEROGUE_MINIRUN\t22")
            || version == Some("POKEROGUE_MINIRUN\t23")
            || version == Some("POKEROGUE_MINIRUN\t24")
        {
            if extended_movesets.len() != members.len() {
                return Err(MiniRunError::InvalidCheckpoint);
            }
            extended_movesets.sort_by_key(|moveset| moveset.0);
            for (slot, (recorded_slot, count, ids, pp_used, pp_ups)) in
                extended_movesets.into_iter().enumerate()
            {
                if slot != recorded_slot
                    || !members[slot].restore_extended_moveset(count, ids, pp_used, pp_ups)
                {
                    return Err(MiniRunError::InvalidCheckpoint);
                }
            }
        } else if !extended_movesets.is_empty() {
            return Err(MiniRunError::InvalidCheckpoint);
        }
        if matches!(
            version,
            Some("POKEROGUE_MINIRUN\t22" | "POKEROGUE_MINIRUN\t23" | "POKEROGUE_MINIRUN\t24")
        ) {
            if used_tm_records.len() != members.len() {
                return Err(MiniRunError::InvalidCheckpoint);
            }
            used_tm_records.sort_by_key(|record| record.0);
            for (slot, (recorded_slot, ids)) in used_tm_records.into_iter().enumerate() {
                if slot != recorded_slot {
                    return Err(MiniRunError::InvalidCheckpoint);
                }
                members[slot].used_tm_ids = ids;
            }
        } else if !used_tm_records.is_empty() {
            return Err(MiniRunError::InvalidCheckpoint);
        }
        if matches!(
            version,
            Some("POKEROGUE_MINIRUN\t23" | "POKEROGUE_MINIRUN\t24")
        ) {
            if friendship_records.len() != members.len() {
                return Err(MiniRunError::InvalidCheckpoint);
            }
            friendship_records.sort_by_key(|record| record.0);
            for (slot, (recorded_slot, friendship)) in friendship_records.into_iter().enumerate() {
                if slot != recorded_slot {
                    return Err(MiniRunError::InvalidCheckpoint);
                }
                members[slot].friendship = friendship;
            }
        } else if !friendship_records.is_empty() {
            return Err(MiniRunError::InvalidCheckpoint);
        }
        if version == Some("POKEROGUE_MINIRUN\t24") {
            if soothe_bell_records.len() != members.len() {
                return Err(MiniRunError::InvalidCheckpoint);
            }
            soothe_bell_records.sort_by_key(|record| record.0);
            for (slot, (recorded_slot, stacks)) in soothe_bell_records.into_iter().enumerate() {
                if slot != recorded_slot {
                    return Err(MiniRunError::InvalidCheckpoint);
                }
                members[slot].soothe_bell_stacks = stacks;
            }
        } else if !soothe_bell_records.is_empty() {
            return Err(MiniRunError::InvalidCheckpoint);
        }
        let player =
            Team::from_checkpoint(members, active).ok_or(MiniRunError::InvalidCheckpoint)?;
        let mut battle =
            Battle::town_encounter_for_wave_with_team(&seed, &time_of_day, wave as u32, player)
                .map_err(MiniRunError::Encounter)?;
        if (stage == MiniRunStage::MoveLearning)
            != (!pending_move_records.is_empty() || !pending_prevo_records.is_empty())
            || pending_move_records.len() > 64
            || pending_prevo_records.len() > 64
        {
            return Err(MiniRunError::InvalidCheckpoint);
        }
        let mut seen_pending_moves = Vec::new();
        for (slot, move_id) in &pending_move_records {
            let member = battle
                .player
                .member(*slot)
                .ok_or(MiniRunError::InvalidCheckpoint)?;
            let movement = pokerogue_data::move_by_numeric_id(*move_id)
                .filter(|movement| !movement.is_unimplemented)
                .ok_or(MiniRunError::InvalidCheckpoint)?;
            if member.move_count() != 4
                || member.moves[..member.move_count()]
                    .iter()
                    .any(|known| known.numeric_id() == movement.numeric_id)
                || seen_pending_moves.contains(&(*slot, *move_id))
            {
                return Err(MiniRunError::InvalidCheckpoint);
            }
            seen_pending_moves.push((*slot, *move_id));
        }
        battle.pending_level_moves = pending_move_records;
        for (slot, move_id) in &pending_prevo_records {
            if battle.player.member(*slot).is_none()
                || pokerogue_data::move_by_numeric_id(*move_id).is_none()
            {
                return Err(MiniRunError::InvalidCheckpoint);
            }
        }
        battle.pending_prevolution_moves = pending_prevo_records;
        let expected_prompts = pending_level_up_sequence(
            &battle.pending_level_moves,
            &battle.pending_prevolution_moves,
            battle.player.len(),
        );
        if is_v27 && pending_prompt_records != expected_prompts {
            return Err(MiniRunError::InvalidCheckpoint);
        }
        battle.pending_level_up_prompts = if is_v27 {
            pending_prompt_records
        } else {
            expected_prompts
        };
        Ok(Self {
            seed,
            time_of_day,
            wave,
            wave_limit,
            stage,
            battle,
            reward_experience,
            exp_charm_stacks,
            exp_share_stacks,
            amulet_coin_stacks,
            super_exp_charm_stacks,
            pokeballs,
            greatballs,
            ultraballs,
            money,
            lure_battles_remaining,
            temp_stat_booster_battles_remaining,
            reward_claimed: reward_claimed.ok_or(MiniRunError::InvalidCheckpoint)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classic_run_starts_with_one_thousand_money() {
        let run = MiniRun::new("StartingMoney", "DAY").unwrap();
        assert_eq!(run.money, 1000);
    }
    #[test]
    fn battle_resolves_turn_and_changes_hp() {
        let mut b = Battle::demo();
        let hp = b.enemy.hp;
        b.choose_move(0);
        assert!(b.enemy.hp < hp);
        assert_eq!(b.turn, 2);
        assert!(b.player.hp < b.player.max_hp);
        assert!(b.last_events.iter().any(|event| matches!(
            event,
            BattleEvent::DamageApplied {
                target: BattleActor::Enemy,
                amount,
                ..
            } if *amount == hp - b.enemy.hp
        )));
    }
    #[test]
    fn move_usage_consumes_pp_and_empty_pp_rejects_without_turn_or_rng() {
        let mut battle = Battle::demo();
        let selected = battle.player.moves[0];
        battle.take_turn(PlayerAction::UseMove(0)).unwrap();
        assert_eq!(battle.player.move_pp_used[0], 1);
        assert_eq!(battle.player.move_pp_remaining(0), Some(selected.pp() - 1));

        let mut exhausted = Battle::demo();
        exhausted.player.move_pp_used[0] = exhausted.player.moves[0].pp();
        let enemy_hp = exhausted.enemy.hp;
        let rng_draws = exhausted.rng_draws().len();
        assert_eq!(
            exhausted.take_turn(PlayerAction::UseMove(0)),
            Err(TurnActionError::NoPp)
        );
        assert_eq!(exhausted.enemy.hp, enemy_hp);
        assert_eq!(exhausted.turn, 1);
        assert_eq!(exhausted.rng_draws().len(), rng_draws);
    }

    #[test]
    fn exp_share_splits_base_exp_among_participants_and_nonparticipants() {
        let mut battle = Battle::demo_with_seed("ExpShareStackCapCore");
        battle.enemy = Battler::demo(
            Species::from_numeric_id(399).unwrap(),
            100,
            [Move::Tackle; 2],
        );
        let extra = Battler::demo(Species::Charmander, 5, [Move::Tackle; 2]);
        battle.player.add_member(extra).unwrap();
        battle.participants = vec![true, true, false];
        battle.outcome = Some(Outcome::PlayerWin);
        let mut awards = battle.award_victory_experience_with_exp_share(0, 0, 5, 10);
        awards.sort_by_key(|(slot, _, _)| *slot);
        let actual: Vec<_> = awards.iter().map(|(slot, exp, _)| (*slot, *exp)).collect();
        assert_eq!(battle.enemy_exp_value(), 1001.0);
        assert_eq!(actual, [(0, 500), (1, 500), (2, 500)]);
        assert_eq!(
            battle
                .last_events
                .iter()
                .filter_map(|event| match event {
                    BattleEvent::ExperienceGained {
                        party_slot, amount, ..
                    } => Some((*party_slot, *amount)),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            [(0, 500), (1, 500), (2, 500)]
        );
    }

    #[test]
    fn generated_gender_matches_species_gender_support_and_symbol() {
        assert_eq!(Gender::Male.symbol(), Some("♂"));
        assert_eq!(Gender::Female.symbol(), Some("♀"));
        assert_eq!(Gender::Genderless.symbol(), None);

        let ditto = Battler::demo(Species::from_numeric_id(132).unwrap(), 5, [Move::Tackle]);
        assert_eq!(ditto.gender, Gender::Genderless);
    }

    #[test]
    fn awarded_experience_advances_level_and_preserves_remainder_for_hud() {
        let mut battler = Battler::demo(Species::Bulbasaur, 5, [Move::Tackle]);
        let start = battler.exp;
        let next = total_exp_for_level(6, battler.species.growth_rate());
        let amount = next - start + 7;
        let gained = battler.grant_exp(amount);
        assert_eq!(gained, [6]);
        assert_eq!(battler.level, 6);
        assert_eq!(
            battler.exp - total_exp_for_level(6, battler.species.growth_rate()),
            7
        );
    }

    #[test]
    fn invalid_move_does_not_mutate_battle() {
        let mut b = Battle::demo();
        let hp = b.enemy.hp;
        b.choose_move(99);
        assert_eq!(b.enemy.hp, hp);
        assert_eq!(b.turn, 1);
    }

    #[test]
    fn boss_damage_is_limited_to_current_segment_and_advances_shield_index() {
        let mut boss = Battler::demo(Species::Bulbasaur, 20, [Move::Tackle, Move::Tackle]);
        boss.max_hp = 200;
        boss.hp = 200;
        boss.configure_boss_segments(2);
        assert_eq!(boss.apply_damage(40, 0), 40);
        assert_eq!((boss.hp, boss.boss_segment_index), (160, 1));
        assert_eq!(boss.apply_damage(70, 0), 60);
        assert_eq!((boss.hp, boss.boss_segment_index), (100, 0));
        assert_eq!(boss.apply_damage(500, 0), 100);
        assert_eq!(boss.hp, 0);
    }

    #[test]
    fn classic_wave_ten_boss_level_and_segment_count_follow_source_formula() {
        for real in [-1.0, -0.5, 0.0, 0.5, 0.999] {
            let level = classic_boss_level(10, 10, real, false);
            assert!((6..=8).contains(&level));
            assert_eq!(classic_boss_segments(10, level, 600), 2);
        }
        assert_eq!(classic_boss_segments(10, 100, 670), 4);
    }
}
