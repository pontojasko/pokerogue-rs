#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Element {
    Normal,
    Fighting,
    Flying,
    Poison,
    Ground,
    Rock,
    Bug,
    Ghost,
    Steel,
    Fire,
    Water,
    Grass,
    Electric,
    Psychic,
    Ice,
    Dragon,
    Dark,
    Fairy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum GrowthRate {
    Erratic,
    Fast,
    MediumFast,
    MediumSlow,
    Slow,
    Fluctuating,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportedSpecies {
    pub id: &'static str,
    pub numeric_id: u16,
    pub name: &'static str,
    pub type1: Element,
    pub type2: Option<Element>,
    pub base_hp: i32,
    pub base_attack: i32,
    pub base_defense: i32,
    pub base_special_attack: i32,
    pub base_special_defense: i32,
    pub base_speed: i32,
    pub base_exp: u16,
    pub growth_rate: GrowthRate,
    pub catch_rate: u8,
    pub base_friendship: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportedSpeciesTm {
    pub species_numeric_id: u16,
    pub move_numeric_id: u16,
    pub order: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportedSpeciesPrevolution {
    pub species_numeric_id: u16,
    pub prevolution_numeric_id: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportedTmTier {
    pub move_numeric_id: u16,
    pub tier_id: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportedMove {
    pub id: &'static str,
    pub numeric_id: u16,
    pub name: &'static str,
    pub element: Element,
    pub power: i16,
    pub accuracy: i16,
    pub pp: u8,
    pub priority: i8,
    pub is_special: bool,
    pub is_status: bool,
    pub is_unimplemented: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportedLevelMove {
    pub species_id: &'static str,
    pub species_numeric_id: u16,
    pub level: u8,
    pub move_id: &'static str,
    pub move_numeric_id: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelMoveSource {
    Direct,
    Prevolution,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedLevelMove {
    pub level: u8,
    pub move_numeric_id: u16,
    pub source: LevelMoveSource,
}

/// Port of the pinned `filterAndSortLevelMoves` for LEVEL and PREVO sources.
pub fn filter_resolved_level_moves(
    mut candidates: Vec<ResolvedLevelMove>,
    starting_level: u8,
    current_level: u8,
) -> Vec<ResolvedLevelMove> {
    candidates.sort_by_key(|entry| {
        (
            entry.level,
            match entry.source {
                LevelMoveSource::Direct => 0,
                LevelMoveSource::Prevolution => 6,
            },
        )
    });
    let own_moves: Vec<_> = candidates
        .iter()
        .filter(|entry| entry.source == LevelMoveSource::Direct && entry.level <= current_level)
        .map(|entry| entry.move_numeric_id)
        .collect();
    let future_moves: Vec<_> = candidates
        .iter()
        .filter(|entry| entry.source == LevelMoveSource::Direct && entry.level > current_level)
        .map(|entry| entry.move_numeric_id)
        .collect();

    let mut unique = Vec::new();
    let mut resolved = Vec::new();
    for entry in candidates {
        if entry.level < starting_level
            || entry.level > current_level
            || (entry.source == LevelMoveSource::Prevolution
                && (own_moves.contains(&entry.move_numeric_id)
                    || future_moves.contains(&entry.move_numeric_id)))
            || unique.contains(&entry.move_numeric_id)
        {
            continue;
        }
        unique.push(entry.move_numeric_id);
        resolved.push(entry);
    }
    resolved
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportedEncounterPool {
    pub tier_id: u8,
    pub tier: &'static str,
    pub time_id: i8,
    pub time_of_day: &'static str,
    pub species: &'static [(u16, &'static str)],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportedTierWeight {
    pub boss_table: bool,
    pub tier_id: u8,
    pub tier: &'static str,
    pub roll_max: u16,
    pub min_roll: u16,
    pub max_roll: u16,
    pub weight: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportedTierSelectionRule {
    pub normal_roll_max: u16,
    pub boss_roll_max: u16,
    pub boss_table_condition: &'static str,
    pub override_species_source: &'static str,
    pub override_species_bypasses_tier: bool,
    pub forced_tier_source: &'static str,
    pub forced_tier_skips_roll: bool,
    pub luck_modifier_formula: &'static str,
    pub effective_roll_max_formula: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportedTimePoolRule {
    pub always_include_bucket: &'static str,
    pub current_time_bucket: &'static str,
    pub always_bucket_first: bool,
}

mod generated_data;
mod generated_modifier_pools;
pub use generated_data::{
    EXP_THRESHOLDS, IMPORTED_LEVEL_MOVES, IMPORTED_MOVES, IMPORTED_SPECIES,
    IMPORTED_SPECIES_PREVOLUTION, IMPORTED_SPECIES_TMS, IMPORTED_TM_TIERS, TOWN_COMMON_ALL_POOL,
    TOWN_ENCOUNTER_POOLS, TOWN_TIER_SELECTION_RULE, TOWN_TIER_WEIGHTS, TOWN_TIME_POOL_RULE,
};
pub use generated_modifier_pools::{IMPORTED_PLAYER_MODIFIER_POOL, ImportedModifierPoolEntry};

pub fn species_by_id(id: &str) -> Option<&'static ImportedSpecies> {
    IMPORTED_SPECIES.iter().find(|record| record.id == id)
}

pub fn species_by_numeric_id(id: u16) -> Option<&'static ImportedSpecies> {
    IMPORTED_SPECIES
        .iter()
        .find(|record| record.numeric_id == id)
}

pub fn move_by_id(id: &str) -> Option<&'static ImportedMove> {
    IMPORTED_MOVES.iter().find(|record| record.id == id)
}

pub fn move_by_numeric_id(id: u16) -> Option<&'static ImportedMove> {
    IMPORTED_MOVES.iter().find(|record| record.numeric_id == id)
}

/// Source-ordered TM candidates for one imported base species. Filters known,
/// level-up, previously used, wrong-tier, and explicitly unimplemented moves.
pub fn compatible_tms_for_species(
    species_numeric_id: u16,
    known_move_ids: &[u16],
    used_tm_ids: &[u16],
    tier_id: u8,
) -> Vec<&'static ImportedMove> {
    let mut family = vec![species_numeric_id];
    let mut current = species_numeric_id;
    while let Some(parent) = IMPORTED_SPECIES_PREVOLUTION
        .iter()
        .find(|entry| entry.species_numeric_id == current)
        .map(|entry| entry.prevolution_numeric_id)
    {
        if family.contains(&parent) {
            break;
        }
        family.push(parent);
        current = parent;
    }

    let mut ordered_tms = Vec::new();
    for member_id in family {
        for entry in IMPORTED_SPECIES_TMS
            .iter()
            .filter(|entry| entry.species_numeric_id == member_id)
        {
            if !ordered_tms.contains(&entry.move_numeric_id) {
                ordered_tms.push(entry.move_numeric_id);
            }
        }
    }

    ordered_tms
        .into_iter()
        .filter(|move_id| !known_move_ids.contains(move_id))
        .filter(|move_id| !used_tm_ids.contains(move_id))
        .filter(|move_id| {
            !IMPORTED_LEVEL_MOVES.iter().any(|learnset| {
                learnset.species_numeric_id == species_numeric_id
                    && learnset.move_numeric_id == *move_id
            })
        })
        .filter(|move_id| {
            IMPORTED_TM_TIERS
                .iter()
                .any(|tm| tm.move_numeric_id == *move_id && tm.tier_id == tier_id)
        })
        .filter_map(move_by_numeric_id)
        .filter(|movement| !movement.is_unimplemented)
        .collect()
}

pub fn level_moves_for(
    species_id: &str,
    through_level: u8,
) -> impl Iterator<Item = &'static ImportedLevelMove> {
    IMPORTED_LEVEL_MOVES
        .iter()
        .filter(move |record| record.species_id == species_id && record.level <= through_level)
}

/// Source-shaped `getLevelMoves(includePrevolutionMoves=true)` projection for
/// imported base species. Forms, evolution moves and relearn entries are absent.
pub fn level_moves_with_prevolutions(
    species_numeric_id: u16,
    starting_level: u8,
    current_level: u8,
) -> Vec<ResolvedLevelMove> {
    let mut chain = Vec::new();
    let mut current = species_numeric_id;
    while let Some(parent) = IMPORTED_SPECIES_PREVOLUTION
        .iter()
        .find(|entry| entry.species_numeric_id == current)
        .map(|entry| entry.prevolution_numeric_id)
    {
        if chain.contains(&parent) {
            break;
        }
        chain.push(parent);
        current = parent;
    }
    let has_prevolution = !chain.is_empty();
    chain.reverse();

    let direct: Vec<_> = IMPORTED_LEVEL_MOVES
        .iter()
        .filter(|entry| {
            entry.species_numeric_id == species_numeric_id
                && entry.level > 0
                && (!has_prevolution || entry.level > 1)
        })
        .collect();
    let mut candidates: Vec<_> = direct
        .iter()
        .map(|entry| ResolvedLevelMove {
            level: entry.level,
            move_numeric_id: entry.move_numeric_id,
            source: LevelMoveSource::Direct,
        })
        .collect();
    for (chain_index, species_id) in chain.into_iter().enumerate() {
        candidates.extend(
            IMPORTED_LEVEL_MOVES
                .iter()
                .filter(|entry| {
                    entry.species_numeric_id == species_id
                        && entry.level > 0
                        && (chain_index == 0 || entry.level > 1)
                })
                .map(|entry| ResolvedLevelMove {
                    level: entry.level,
                    move_numeric_id: entry.move_numeric_id,
                    source: LevelMoveSource::Prevolution,
                }),
        );
    }
    // Stable sorting retains source evolution-chain order for same-level PREVO rows.
    filter_resolved_level_moves(candidates, starting_level, current_level)
}

pub fn total_exp_for_level(level: u8, growth_rate: GrowthRate) -> u32 {
    if level == 0 {
        return 0;
    }
    EXP_THRESHOLDS[growth_rate as usize][level.saturating_sub(1) as usize]
}

fn imported_species(id: &'static str) -> &'static ImportedSpecies {
    species_by_id(id).expect("species missing from generated source data")
}

fn imported_move(id: &'static str) -> &'static ImportedMove {
    move_by_id(id).expect("move missing from generated source data")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BattleStat {
    Attack,
    SpecialAttack,
    SpecialDefense,
    Accuracy,
    Evasion,
    Speed,
    Defense,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveEffect {
    StatStageChange {
        stat: BattleStat,
        stages: i8,
        chance: i16,
    },
    SelfStatStageChange {
        stat: BattleStat,
        stages: i8,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveTarget {
    Opponent,
    Attacker,
}

mod type_chart;
pub use type_chart::single_type_effectiveness;

pub fn type_effectiveness(attack: Element, defender_types: &[Element]) -> f32 {
    defender_types
        .iter()
        .map(|&defender| single_type_effectiveness(attack, defender))
        .product()
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Species {
    Bulbasaur,
    Charmander,
    Imported(u16),
}
impl Species {
    pub fn from_numeric_id(id: u16) -> Option<Self> {
        match id {
            1 => Some(Self::Bulbasaur),
            4 => Some(Self::Charmander),
            _ => species_by_numeric_id(id).map(|_| Self::Imported(id)),
        }
    }
    fn data(self) -> &'static ImportedSpecies {
        match self {
            Self::Bulbasaur => imported_species("BULBASAUR"),
            Self::Charmander => imported_species("CHARMANDER"),
            Self::Imported(id) => species_by_numeric_id(id).expect("imported species ID"),
        }
    }
    pub fn primary_type(self) -> Element {
        self.data().type1
    }
    pub fn types(self) -> (Element, Option<Element>) {
        let data = self.data();
        (data.type1, data.type2)
    }
    pub fn name(self) -> &'static str {
        self.data().name
    }
    pub fn numeric_id(self) -> u16 {
        self.data().numeric_id
    }
    pub fn growth_rate(self) -> GrowthRate {
        self.data().growth_rate
    }
    pub fn base_exp(self) -> u16 {
        self.data().base_exp
    }
    pub fn catch_rate(self) -> u8 {
        self.data().catch_rate
    }
    pub fn base_friendship(self) -> u8 {
        self.data().base_friendship
    }
    pub fn base_hp(self) -> i32 {
        self.data().base_hp
    }
    pub fn base_attack(self) -> i32 {
        self.data().base_attack
    }
    pub fn base_defense(self) -> i32 {
        self.data().base_defense
    }
    pub fn base_special_attack(self) -> i32 {
        self.data().base_special_attack
    }
    pub fn base_special_defense(self) -> i32 {
        self.data().base_special_defense
    }
    pub fn base_speed(self) -> i32 {
        self.data().base_speed
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    VineWhip,
    Tackle,
    Ember,
    SandAttack,
    Counter,
    QuickAttack,
    Imported(u16),
}
impl Move {
    pub fn struggle() -> Self {
        let movement = move_by_id("STRUGGLE").expect("imported Struggle move");
        Self::Imported(movement.numeric_id)
    }
    fn data(self) -> &'static ImportedMove {
        match self {
            Self::VineWhip => imported_move("VINE_WHIP"),
            Self::Tackle => imported_move("TACKLE"),
            Self::Ember => imported_move("EMBER"),
            Self::SandAttack => imported_move("SAND_ATTACK"),
            Self::Counter => imported_move("COUNTER"),
            Self::QuickAttack => imported_move("QUICK_ATTACK"),
            Self::Imported(id) => move_by_numeric_id(id).expect("imported move ID"),
        }
    }
    pub fn id(self) -> &'static str {
        self.data().id
    }
    pub fn numeric_id(self) -> u16 {
        self.data().numeric_id
    }
    pub fn name(self) -> &'static str {
        self.data().name
    }
    pub fn power(self) -> i16 {
        self.data().power
    }
    pub fn element(self) -> Element {
        self.data().element
    }
    pub fn is_special(self) -> bool {
        self.data().is_special
    }
    pub fn is_status(self) -> bool {
        self.data().is_status
    }
    pub fn accuracy(self) -> i16 {
        self.data().accuracy
    }
    pub fn pp(self) -> u8 {
        self.data().pp
    }
    pub fn effect(self) -> Option<MoveEffect> {
        match self {
            Self::SandAttack => Some(MoveEffect::StatStageChange {
                stat: BattleStat::Accuracy,
                stages: -1,
                chance: -1,
            }),
            Self::Imported(id) if move_by_numeric_id(id).is_some_and(|m| m.id == "HARDEN") => {
                Some(MoveEffect::SelfStatStageChange {
                    stat: BattleStat::Defense,
                    stages: 1,
                })
            }
            Self::Imported(id) if move_by_numeric_id(id).is_some_and(|m| m.id == "GROWL") => {
                Some(MoveEffect::StatStageChange {
                    stat: BattleStat::Attack,
                    stages: -1,
                    chance: -1,
                })
            }
            Self::Imported(id)
                if move_by_numeric_id(id).is_some_and(|m| matches!(m.id, "TAIL_WHIP" | "LEER")) =>
            {
                Some(MoveEffect::StatStageChange {
                    stat: BattleStat::Defense,
                    stages: -1,
                    chance: -1,
                })
            }
            Self::VineWhip
            | Self::Tackle
            | Self::Ember
            | Self::Counter
            | Self::QuickAttack
            | Self::Imported(_) => None,
        }
    }

    pub fn target(self) -> MoveTarget {
        match self {
            Self::Counter => MoveTarget::Attacker,
            Self::Imported(id) if move_by_numeric_id(id).is_some_and(|m| m.id == "HARDEN") => {
                MoveTarget::Attacker
            }
            Self::VineWhip
            | Self::Tackle
            | Self::Ember
            | Self::SandAttack
            | Self::QuickAttack
            | Self::Imported(_) => MoveTarget::Opponent,
        }
    }

    pub fn priority(self) -> i8 {
        self.data().priority
    }

    pub fn is_typeless(self) -> bool {
        self.id() == "STRUGGLE"
    }
}
