#[cfg(test)]
mod equivalence_smoke {
    use pokerogue_core::{
        Battle, BattleActor, BattleEvent, BattleRng, Battler, FieldExitKind, HitResult, Outcome,
        PlayerAction, SwitchError, SwitchEvent, Team, TurnActionError, calculate_damage,
        calculate_damage_for_types, calculate_damage_with_boosters, check_move_accuracy,
    };
    use pokerogue_data::{
        BattleStat, Element, GrowthRate, IMPORTED_LEVEL_MOVES, IMPORTED_MOVES, IMPORTED_SPECIES,
        IMPORTED_SPECIES_PREVOLUTION, IMPORTED_SPECIES_TMS, IMPORTED_TM_TIERS, ImportedTierWeight,
        Move, MoveEffect, Species, TOWN_COMMON_ALL_POOL, TOWN_ENCOUNTER_POOLS,
        TOWN_TIER_SELECTION_RULE, TOWN_TIER_WEIGHTS, TOWN_TIME_POOL_RULE, level_moves_for,
        move_by_id, move_by_numeric_id, single_type_effectiveness, species_by_id,
        species_by_numeric_id, type_effectiveness,
    };

    fn checkpoint_as_v16(checkpoint: &str) -> String {
        checkpoint
            .lines()
            .filter(|line| {
                !line.starts_with("ultraballs\t")
                    && !line.starts_with("exp_share_stacks\t")
                    && !line.starts_with("amulet_coin_stacks\t")
                    && !line.starts_with("moveset\t")
                    && !line.starts_with("used_tms\t")
                    && !line.starts_with("friendship\t")
                    && !line.starts_with("soothe_bell\t")
            })
            .map(|line| {
                if line == "POKEROGUE_MINIRUN\t17"
                    || line == "POKEROGUE_MINIRUN\t18"
                    || line == "POKEROGUE_MINIRUN\t19"
                    || line == "POKEROGUE_MINIRUN\t20"
                    || line == "POKEROGUE_MINIRUN\t22"
                    || line == "POKEROGUE_MINIRUN\t23"
                    || line == "POKEROGUE_MINIRUN\t27"
                {
                    "POKEROGUE_MINIRUN\t16".to_owned()
                } else if let Some(record) = line.strip_prefix("pokemon\t") {
                    let mut fields: Vec<_> = record.split(',').collect();
                    fields.pop();
                    format!("pokemon\t{}", fields.join(","))
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    }

    fn element(name: &str) -> Element {
        match name {
            "NORMAL" => Element::Normal,
            "FIGHTING" => Element::Fighting,
            "FLYING" => Element::Flying,
            "POISON" => Element::Poison,
            "GROUND" => Element::Ground,
            "ROCK" => Element::Rock,
            "BUG" => Element::Bug,
            "GHOST" => Element::Ghost,
            "STEEL" => Element::Steel,
            "FIRE" => Element::Fire,
            "WATER" => Element::Water,
            "GRASS" => Element::Grass,
            "ELECTRIC" => Element::Electric,
            "PSYCHIC" => Element::Psychic,
            "ICE" => Element::Ice,
            "DRAGON" => Element::Dragon,
            "DARK" => Element::Dark,
            "FAIRY" => Element::Fairy,
            _ => panic!("unknown fixture type {name}"),
        }
    }

    fn phaser_fixture_values(turn: u32, range: u32, min: i32) -> Vec<i32> {
        include_str!("../fixtures/phaser-rng.csv")
            .lines()
            .filter_map(|line| {
                let fields: Vec<_> = line.split(',').collect();
                (fields.len() == 4
                    && fields[0] == turn.to_string()
                    && fields[1] == range.to_string()
                    && fields[2] == min.to_string())
                .then(|| fields[3].parse().unwrap())
            })
            .collect()
    }

    #[test]
    fn rare_candy_level_thresholds_match_executed_original_apply() {
        for row in include_str!("../fixtures/rare-candy.csv")
            .lines()
            .filter(|line| line.starts_with("MEDIUM_SLOW,"))
        {
            let fields: Vec<_> = row.split(',').collect();
            let initial_level: u8 = fields[1].parse().unwrap();
            let initial_exp: u32 = fields[2].parse().unwrap();
            let expected_level: u8 = fields[3].parse().unwrap();
            let expected_exp: u32 = fields[4].parse().unwrap();
            let mut battler = Battler::demo(
                Species::Bulbasaur,
                initial_level,
                [pokerogue_data::Move::Tackle; 2],
            );
            battler.exp = initial_exp;
            assert!(battler.rare_candy_level_up());
            assert_eq!(battler.level, expected_level);
            assert_eq!(battler.exp, expected_exp);
        }
    }

    #[test]
    fn rare_candy_friendship_gain_matches_executed_source_and_saves() {
        for row in include_str!("../fixtures/rare-candy.csv")
            .lines()
            .filter(|line| line.starts_with("friendship,"))
        {
            let fields: Vec<_> = row.split(',').collect();
            let initial: u8 = fields[1].parse().unwrap();
            let expected: u8 = fields[4].parse().unwrap();
            let mut member =
                Battler::demo(Species::Bulbasaur, 5, [pokerogue_data::Move::Tackle; 2]);
            member.friendship = initial;
            member.rare_candy_level_up();
            assert_eq!(member.friendship, expected, "{row}");
        }
        let mut run = pokerogue_core::MiniRun::new("RareCandyFriendshipSave", "DAY").unwrap();
        run.stage = pokerogue_core::MiniRunStage::Reward;
        let source = include_str!("../fixtures/rare-candy.csv")
            .lines()
            .find(|line| line.starts_with("friendship,198,"))
            .unwrap();
        let expected: u8 = source.split(',').nth(4).unwrap().parse().unwrap();
        let member = run.battle.player.member_mut(0).unwrap();
        member.friendship = 198;
        run.apply_modifier_reward(pokerogue_core::rewards::Reward::RareCandy, 0, None)
            .unwrap();
        assert_eq!(run.battle.player.member(0).unwrap().friendship, expected);
        let checkpoint = run.checkpoint().unwrap();
        assert!(checkpoint.starts_with("POKEROGUE_MINIRUN\t27\n"));
        assert_eq!(
            pokerogue_core::MiniRun::from_checkpoint(&checkpoint)
                .unwrap()
                .battle
                .player
                .member(0)
                .unwrap()
                .friendship,
            expected
        );
        let v22 = checkpoint
            .replacen("POKEROGUE_MINIRUN\t27", "POKEROGUE_MINIRUN\t22", 1)
            .lines()
            .filter(|line| !line.starts_with("friendship\t") && !line.starts_with("soothe_bell\t"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        assert_eq!(
            pokerogue_core::MiniRun::from_checkpoint(&v22)
                .unwrap()
                .battle
                .player
                .member(0)
                .unwrap()
                .friendship,
            Species::Bulbasaur.base_friendship()
        );
    }

    #[test]
    fn soothe_bell_offer_boosts_friendship_and_migrates_save() {
        use pokerogue_core::{MiniRun, MiniRunStage};

        for row in include_str!("../fixtures/soothe-bell.csv")
            .lines()
            .filter(|line| !line.starts_with('#'))
        {
            let fields: Vec<_> = row.split(',').collect();
            let stacks: u8 = fields[1].parse().unwrap();
            let base_gain: u8 = fields[2].parse().unwrap();
            let expected_gain: u8 = fields[3].parse().unwrap();
            let expected_max: u8 = fields[4].parse().unwrap();
            assert_eq!(expected_max, 3, "{row}");
            if fields[0] == "rare_candy" {
                assert_eq!(base_gain, 6);
                let mut battler = Battler::demo(Species::Bulbasaur, 5, [Move::Tackle; 2]);
                battler.friendship = 50;
                battler.soothe_bell_stacks = stacks;
                assert!(battler.rare_candy_level_up());
                assert_eq!(battler.friendship, 50 + expected_gain, "{row}");
            } else {
                assert_eq!(fields[0], "battle");
                assert_eq!(base_gain, 3);
                assert_eq!(
                    expected_gain,
                    (f64::from(base_gain) * (1.0 + 0.5 * f64::from(stacks))).floor() as u8,
                    "{row}"
                );
            }
        }

        let source_row = include_str!("../fixtures/great-recovery-offer.csv")
            .lines()
            .find(|line| line.starts_with("SootheBellSeed"))
            .unwrap();
        let fields: Vec<_> = source_row.split(',').collect();
        let mut run = MiniRun::new_with_wave_limit(fields[0], "DAY", 20).unwrap();
        run.wave = fields[1].parse().unwrap();
        run.stage = MiniRunStage::Reward;
        let offer = run.natural_supported_reward_offer().unwrap();
        let options = offer
            .options
            .iter()
            .map(|option| {
                format!(
                    "{}:{}:{}:{}",
                    option.id, option.upgrades, option.tier, option.pool_index
                )
            })
            .collect::<Vec<_>>();
        let expected_options = fields[2]
            .split('|')
            .map(|option| {
                let pieces: Vec<_> = option.split(':').collect();
                format!("{}:{}:{}:{}", pieces[0], pieces[1], pieces[2], pieces[3])
            })
            .collect::<Vec<_>>();
        assert_eq!(options, expected_options);
        let expected_draws: Vec<_> = fields[3]
            .split('|')
            .map(|draw| {
                let parts: Vec<_> = draw.split(':').collect();
                (
                    parts[0].parse::<u32>().unwrap(),
                    parts[1].parse::<i32>().unwrap(),
                    parts[2].parse::<i32>().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            offer
                .rng_draws
                .iter()
                .map(|draw| (draw.range, draw.min, draw.value))
                .collect::<Vec<_>>(),
            expected_draws
        );
        let soothe_index = offer
            .options
            .iter()
            .position(|option| option.id == "SOOTHE_BELL")
            .unwrap();
        run.claim_natural_supported_reward(soothe_index, 0).unwrap();
        assert_eq!(run.battle.player.member(0).unwrap().soothe_bell_stacks, 1);
        assert!(
            run.battle
                .player
                .member_mut(0)
                .unwrap()
                .rare_candy_level_up()
        );
        assert_eq!(run.battle.player.member(0).unwrap().friendship, 59);
        let save = run.checkpoint().unwrap();
        assert!(save.starts_with("POKEROGUE_MINIRUN\t27\n"));
        let restored = MiniRun::from_checkpoint(&save).unwrap();
        assert_eq!(
            restored.battle.player.member(0).unwrap().soothe_bell_stacks,
            1
        );
        assert_eq!(restored.battle.player.member(0).unwrap().friendship, 59);

        let v24 = save
            .replacen("POKEROGUE_MINIRUN\t27", "POKEROGUE_MINIRUN\t24", 1)
            .lines()
            .filter(|line| !line.starts_with("pending_move\t"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        assert_eq!(
            MiniRun::from_checkpoint(&v24)
                .unwrap()
                .battle
                .player
                .member(0)
                .unwrap()
                .soothe_bell_stacks,
            1
        );

        let v23 = save
            .replacen("POKEROGUE_MINIRUN\t27", "POKEROGUE_MINIRUN\t23", 1)
            .lines()
            .filter(|line| !line.starts_with("soothe_bell\t"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        assert_eq!(
            MiniRun::from_checkpoint(&v23)
                .unwrap()
                .battle
                .player
                .member(0)
                .unwrap()
                .soothe_bell_stacks,
            0
        );
        assert_eq!(
            MiniRun::from_checkpoint(&v23)
                .unwrap()
                .battle
                .player
                .member(0)
                .unwrap()
                .friendship,
            59
        );
    }

    #[test]
    fn exp_caps_and_level_updates_match_executed_source_methods() {
        for row in include_str!("../fixtures/exp-cap.csv")
            .lines()
            .filter(|line| !line.starts_with('#'))
        {
            let fields: Vec<_> = row.split(',').collect();
            let wave: u32 = fields[1].parse().unwrap();
            let override_cap: i32 = fields[2].parse().unwrap();
            let ignore_cap = fields[3] == "1";
            let source_cap: u64 = fields[4].parse().unwrap();
            let growth = match fields[5] {
                "MEDIUM_FAST" => GrowthRate::MediumFast,
                "MEDIUM_SLOW" => GrowthRate::MediumSlow,
                name => panic!("unsupported source fixture growth rate {name}"),
            };
            assert_eq!(
                pokerogue_core::classic_exp_level_cap(wave, override_cap, ignore_cap),
                source_cap.min(100) as u8,
                "cap mismatch: {row}"
            );
            let species = IMPORTED_SPECIES
                .iter()
                .find(|species| species.growth_rate == growth)
                .unwrap();
            let mut battler = Battler::demo(
                pokerogue_data::Species::from_numeric_id(species.numeric_id).unwrap(),
                fields[6].parse().unwrap(),
                [pokerogue_data::Move::Tackle; 2],
            );
            battler.exp = fields[7].parse().unwrap();
            let levels =
                battler.grant_exp_capped(fields[8].parse().unwrap(), source_cap.min(100) as u8);
            assert_eq!(
                battler.level,
                fields[9].parse().unwrap(),
                "level mismatch: {row}"
            );
            assert_eq!(
                battler.exp,
                fields[10].parse().unwrap(),
                "EXP mismatch: {row}"
            );
            assert_eq!(
                levels.last().copied().unwrap_or(battler.level),
                battler.level
            );
        }
    }

    #[test]
    fn participant_exp_shares_match_executed_source_party_method() {
        let source_rows: Vec<_> = include_str!("../fixtures/party-exp.csv")
            .lines()
            .filter(|line| !line.starts_with('#'))
            .map(|line| line.split(',').collect::<Vec<_>>())
            .collect();
        let mut battle = Battle::demo_with_seed("PartyExpParitySeed");
        battle.enemy = Battler::demo(
            pokerogue_data::Species::from_numeric_id(399).unwrap(),
            100,
            [pokerogue_data::Move::Tackle; 2],
        );
        assert_eq!(battle.enemy_exp_value(), 1001.0);
        battle.player.member_mut(0).unwrap().level = 10;
        battle.switch_player(1).unwrap();
        battle.switch_player(0).unwrap();
        let initial_friendship = [
            battle.player.member(0).unwrap().friendship,
            battle.player.member(1).unwrap().friendship,
        ];
        battle.outcome = Some(Outcome::PlayerWin);
        let awards = battle.award_victory_experience_with_level_cap(0, 0, 10);
        let mut actual: Vec<_> = awards.iter().map(|(slot, exp, _)| (*slot, *exp)).collect();
        actual.sort_unstable();
        assert_eq!(actual, [(1, 500)]);
        assert_eq!(source_rows[0][0], "two-participants-one-at-cap");
        let mut source: Vec<_> = source_rows[0][4]
            .split('|')
            .map(|pair| {
                let (slot, exp) = pair.split_once(':').unwrap();
                (slot.parse::<usize>().unwrap(), exp.parse::<u32>().unwrap())
            })
            .collect();
        source.sort_unstable();
        assert_eq!(actual, source);

        let mut share = Battle::demo_with_seed("PartyExpQueueOrderSeed");
        share.enemy = Battler::demo(
            pokerogue_data::Species::from_numeric_id(399).unwrap(),
            100,
            [pokerogue_data::Move::Tackle; 2],
        );
        share.outcome = Some(Outcome::PlayerWin);
        let awards = share.award_victory_experience_with_exp_share(0, 0, 1, 10);
        let actual_queue_order: Vec<_> =
            awards.iter().map(|(slot, exp, _)| (*slot, *exp)).collect();
        let source_queue_order: Vec<_> = source_rows[2][4]
            .split('|')
            .map(|pair| {
                let (slot, exp) = pair.split_once(':').unwrap();
                (slot.parse::<usize>().unwrap(), exp.parse::<u32>().unwrap())
            })
            .collect();
        assert_eq!(actual_queue_order, source_queue_order);
        let expected_friendship: Vec<_> = source_rows[0][5]
            .split('|')
            .map(|pair| {
                let (slot, amount) = pair.split_once(':').unwrap();
                (
                    slot.parse::<usize>().unwrap(),
                    amount.parse::<u8>().unwrap(),
                )
            })
            .collect();
        let actual_friendship = (0..2)
            .filter_map(|slot| {
                let gain =
                    battle.player.member(slot).unwrap().friendship - initial_friendship[slot];
                (gain > 0).then_some((slot, gain))
            })
            .collect::<Vec<_>>();
        assert_eq!(actual_friendship, expected_friendship);
        let battle_boost = include_str!("../fixtures/soothe-bell.csv")
            .lines()
            .find(|line| line.starts_with("battle,1,"))
            .unwrap()
            .split(',')
            .nth(3)
            .unwrap()
            .parse::<u8>()
            .unwrap();
        let mut boosted = Battle::demo_with_seed("SootheBellBattleFriendship");
        boosted.outcome = Some(Outcome::PlayerWin);
        let member = boosted.player.member_mut(0).unwrap();
        member.soothe_bell_stacks = 1;
        let old_friendship = member.friendship;
        boosted.award_victory_experience_with_level_cap(0, 0, 100);
        assert_eq!(
            boosted.player.member(0).unwrap().friendship - old_friendship,
            battle_boost
        );
        let mut share = Battle::demo_with_seed("PartyExpShareSeed");
        share.enemy = Battler::demo(
            pokerogue_data::Species::from_numeric_id(399).unwrap(),
            100,
            [pokerogue_data::Move::SandAttack; 2],
        );
        share.enemy.hp = 1;
        share.player.member_mut(0).unwrap().moves[0] = pokerogue_data::Move::Ember;
        share.player.member_mut(0).unwrap().level = 5;
        share.player.member_mut(1).unwrap().level = 5;
        share.player.member_mut(0).unwrap().hp = share.player.member(0).unwrap().max_hp;
        share.player.member_mut(1).unwrap().hp = share.player.member(1).unwrap().max_hp;
        share.take_turn(PlayerAction::UseMove(0)).unwrap();
        assert_eq!(
            share.outcome,
            Some(Outcome::PlayerWin),
            "enemy hp {} log {} events {:?}",
            share.enemy.hp,
            share.log,
            share.last_events
        );
        let share_awards = share.award_victory_experience_with_exp_share(0, 0, 1, 10);
        let share_queue_order: Vec<_> = share_awards
            .iter()
            .map(|(slot, exp, _)| (*slot, *exp))
            .collect();
        let source_queue_order: Vec<_> = source_rows[2][4]
            .split('|')
            .map(|pair| {
                let (slot, exp) = pair.split_once(':').unwrap();
                (slot.parse::<usize>().unwrap(), exp.parse::<u32>().unwrap())
            })
            .collect();
        assert_eq!(share_queue_order, source_queue_order);
        let mut share_actual: Vec<_> = share_awards
            .iter()
            .map(|(slot, exp, _)| (*slot, *exp))
            .collect();
        share_actual.sort_unstable();
        let mut share_source: Vec<_> = source_rows[2][4]
            .split('|')
            .map(|pair| {
                let (slot, exp) = pair.split_once(':').unwrap();
                (slot.parse::<usize>().unwrap(), exp.parse::<u32>().unwrap())
            })
            .collect();
        share_source.sort_unstable();
        assert_eq!(share_actual, share_source);
        let mut one = Battle::demo_with_seed("PartyExpSingleSeed");
        one.enemy = Battler::demo(
            pokerogue_data::Species::from_numeric_id(399).unwrap(),
            100,
            [pokerogue_data::Move::Tackle; 2],
        );
        one.outcome = Some(Outcome::PlayerWin);
        let one_award = one.award_victory_experience_with_level_cap(0, 0, 10);
        let source_single = source_rows[1][4]
            .split_once(':')
            .map(|(slot, exp)| (slot.parse::<usize>().unwrap(), exp.parse::<u32>().unwrap()))
            .unwrap();
        assert_eq!(one_award[0].0, source_single.0);
        assert_eq!(one_award[0].1, source_single.1);
    }

    #[test]
    fn victory_exp_learns_level_moves_into_empty_slots_in_source_order() {
        use pokerogue_core::{Battle, BattleEvent, Outcome};

        let mut battle = Battle::demo_with_seed("LevelMoveAwardSeed");
        let member = battle.player.member_mut(0).unwrap();
        member.exp = pokerogue_data::total_exp_for_level(7, member.species.growth_rate()) - 1;
        battle.outcome = Some(Outcome::PlayerWin);
        let awards = battle.award_victory_experience();
        assert_eq!(awards[0].2, vec![6, 7]);
        let member = battle.player.member(0).unwrap();
        assert_eq!(member.level, 7);
        assert_eq!(member.moves[member.move_count() - 1].numeric_id(), 73);
        assert_eq!(member.move_pp_remaining(member.move_count() - 1), Some(10));
        assert!(battle.last_events.contains(&BattleEvent::MoveLearned {
            party_slot: 0,
            move_id: 73,
        }));

        let source_learnset_has_move = include_str!("../fixtures/imported-demo-data.csv")
            .lines()
            .any(|row| row == "level_move,BULBASAUR,1,7,LEECH_SEED,73");
        assert!(source_learnset_has_move);

        let queue_row = include_str!("../fixtures/level-up-move-queue.csv")
            .lines()
            .find(|line| !line.is_empty() && !line.starts_with('#'))
            .unwrap();
        let queue_fields: Vec<_> = queue_row.split(',').collect();
        assert_eq!(queue_fields[3], "battle:prevoMovePrompt:Bug Bite");
        let mut imported_candidates: Vec<_> = pokerogue_data::level_moves_for("BULBASAUR", 13)
            .filter(|entry| entry.level > 5)
            .collect();
        imported_candidates.sort_by_key(|entry| entry.level);
        let imported_order = imported_candidates
            .iter()
            .map(|entry| entry.move_id)
            .collect::<Vec<_>>()
            .join("|");
        assert_eq!(imported_order, queue_fields[1]);

        let mut queue_battle = Battle::demo_with_seed("LevelMoveQueueSeed");
        let member = queue_battle.player.member_mut(0).unwrap();
        member.exp = pokerogue_data::total_exp_for_level(13, member.species.growth_rate()) - 1;
        queue_battle.outcome = Some(Outcome::PlayerWin);
        queue_battle.award_victory_experience();
        let actual_queue_prefix = queue_battle
            .last_events
            .iter()
            .filter(|event| matches!(event, BattleEvent::MoveLearned { party_slot: 0, .. }))
            .count();
        assert_eq!(
            actual_queue_prefix, 2,
            "two empty slots accept the first two queued moves"
        );
        let actual_ids = queue_battle
            .last_events
            .iter()
            .filter_map(|event| match event {
                BattleEvent::MoveLearned {
                    party_slot: 0,
                    move_id,
                } => Some(*move_id),
                _ => None,
            })
            .collect::<Vec<_>>();
        let expected_prefix = queue_fields[2]
            .split('|')
            .take(2)
            .map(|id| move_by_id(id).unwrap().numeric_id)
            .collect::<Vec<_>>();
        assert_eq!(actual_ids, expected_prefix);

        let mut full = Battle::demo_with_seed("LevelMoveFullSetSeed");
        let member = full.player.member_mut(0).unwrap();
        member.learn_move(Move::Imported(move_by_id("RAZOR_LEAF").unwrap().numeric_id));
        member.learn_move(Move::Imported(move_by_id("GROWTH").unwrap().numeric_id));
        member.exp = pokerogue_data::total_exp_for_level(7, member.species.growth_rate()) - 1;
        full.outcome = Some(Outcome::PlayerWin);
        full.award_victory_experience();
        assert_eq!(full.player.member(0).unwrap().move_count(), 4);
        assert_eq!(
            full.pending_level_moves,
            vec![(0, move_by_id("LEECH_SEED").unwrap().numeric_id)]
        );
        assert!(
            !full
                .last_events
                .iter()
                .any(|event| matches!(event, BattleEvent::MoveLearned { party_slot: 0, .. }))
        );
    }

    #[test]
    fn imported_level_and_prevolution_candidates_match_original_filter_fixture() {
        for row in include_str!("../fixtures/level-move-filter.csv")
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
        {
            let fields: Vec<_> = row.split(',').collect();
            let species = pokerogue_data::species_by_id(fields[0]).unwrap();
            let resolved = pokerogue_data::level_moves_with_prevolutions(
                species.numeric_id,
                fields[1].parse().unwrap(),
                fields[2].parse().unwrap(),
            );
            let actual = resolved
                .iter()
                .map(|entry| {
                    let source = match entry.source {
                        pokerogue_data::LevelMoveSource::Direct => 0,
                        pokerogue_data::LevelMoveSource::Prevolution => 6,
                    };
                    format!(
                        "{}:{}:{}",
                        entry.level,
                        pokerogue_data::move_by_numeric_id(entry.move_numeric_id)
                            .unwrap()
                            .id,
                        source
                    )
                })
                .collect::<Vec<_>>()
                .join("|");
            assert_eq!(actual, fields[3], "{row}");
        }
    }

    #[test]
    fn level_move_filter_edges_match_typescript_future_own_and_duplicates() {
        for row in include_str!("../fixtures/level-move-filter-edges.csv")
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
        {
            let fields: Vec<_> = row.split(',').collect();
            let candidates = fields[2]
                .split('|')
                .map(|entry| {
                    let mut values = entry.split(':');
                    pokerogue_data::ResolvedLevelMove {
                        level: values.next().unwrap().parse().unwrap(),
                        move_numeric_id: values.next().unwrap().parse().unwrap(),
                        source: match values.next().unwrap() {
                            "0" => pokerogue_data::LevelMoveSource::Direct,
                            "6" => pokerogue_data::LevelMoveSource::Prevolution,
                            source => panic!("unexpected move source {source}"),
                        },
                    }
                })
                .collect();
            let result = pokerogue_data::filter_resolved_level_moves(
                candidates,
                fields[0].parse().unwrap(),
                fields[1].parse().unwrap(),
            );
            let actual = result
                .iter()
                .map(|entry| {
                    let source = match entry.source {
                        pokerogue_data::LevelMoveSource::Direct => 0,
                        pokerogue_data::LevelMoveSource::Prevolution => 6,
                    };
                    format!("{}:{}:{}", entry.level, entry.move_numeric_id, source)
                })
                .collect::<Vec<_>>()
                .join("|");
            assert_eq!(actual, fields[3], "{row}");
        }
    }

    #[test]
    fn full_moveset_level_up_choices_persist_replace_and_decline() {
        use pokerogue_core::{MiniRun, MiniRunError, MiniRunStage};

        let mut run = MiniRun::new_with_wave_limit("PendingMoveSaveSeed", "DAY", 20).unwrap();
        run.wave = 11;
        let member = run.battle.player.member_mut(0).unwrap();
        member.learn_move(Move::Imported(move_by_id("RAZOR_LEAF").unwrap().numeric_id));
        member.learn_move(Move::Imported(move_by_id("GROWTH").unwrap().numeric_id));
        member.exp = pokerogue_data::total_exp_for_level(13, member.species.growth_rate()) - 1;
        member.attack = 10_000;
        member.speed = 10_000;
        run.battle.enemy = Battler::demo(Species::Charmander, 5, [Move::Tackle; 2]);
        run.battle.enemy.hp = 1;
        run.battle.enemy.speed = 1;
        run.take_action(PlayerAction::UseMove(0)).unwrap();
        assert_eq!(run.stage, MiniRunStage::MoveLearning);
        assert_eq!(
            run.battle.pending_level_moves,
            vec![
                (0, move_by_id("LEECH_SEED").unwrap().numeric_id),
                (0, move_by_id("POISON_POWDER").unwrap().numeric_id),
                (0, move_by_id("SLEEP_POWDER").unwrap().numeric_id),
            ]
        );

        let saved = run.checkpoint().unwrap();
        assert!(saved.starts_with("POKEROGUE_MINIRUN\t27\n"));
        let mut resumed = MiniRun::from_checkpoint(&saved).unwrap();
        assert_eq!(resumed.stage, MiniRunStage::MoveLearning);
        assert_eq!(resumed.pending_level_move(), run.pending_level_move());
        let rng_before_choice = resumed.battle.rng_draws().to_vec();

        assert!(matches!(
            resumed.resolve_pending_level_move(Some(4)),
            Err(MiniRunError::InvalidRewardTarget)
        ));
        assert_eq!(resumed.pending_level_move(), run.pending_level_move());
        let moves_before_decline = resumed
            .battle
            .player
            .member(0)
            .unwrap()
            .moves
            .map(Move::numeric_id);
        resumed.resolve_pending_level_move(None).unwrap();
        assert_eq!(
            resumed
                .battle
                .player
                .member(0)
                .unwrap()
                .moves
                .map(Move::numeric_id),
            moves_before_decline
        );
        assert_eq!(
            resumed.pending_level_move().unwrap().1,
            move_by_id("POISON_POWDER").unwrap().numeric_id
        );
        resumed.resolve_pending_level_move(Some(0)).unwrap();
        resumed.resolve_pending_level_move(Some(1)).unwrap();
        assert_eq!(resumed.stage, MiniRunStage::Reward);
        let member = resumed.battle.player.member(0).unwrap();
        assert_eq!(
            member.moves[0].numeric_id(),
            move_by_id("POISON_POWDER").unwrap().numeric_id
        );
        assert_eq!(
            member.moves[1].numeric_id(),
            move_by_id("SLEEP_POWDER").unwrap().numeric_id
        );
        assert!(resumed.battle.pending_level_moves.is_empty());
        assert_eq!(resumed.battle.rng_draws(), rng_before_choice.as_slice());
        let after = MiniRun::from_checkpoint(&resumed.checkpoint().unwrap()).unwrap();
        assert_eq!(
            after.battle.player.member(0).unwrap().moves[0].numeric_id(),
            member.moves[0].numeric_id()
        );
    }

    #[test]
    fn prevolution_move_is_informational_and_persists_until_acknowledged() {
        use pokerogue_core::{MiniRun, MiniRunStage};

        let mut run = MiniRun::new_with_wave_limit("PrevoReminderSeed", "DAY", 20).unwrap();
        run.wave = 11;
        let earned = run.battle.enemy_exp_value().floor() as u32;
        let member = run.battle.player.member_mut(0).unwrap();
        member.species = Species::Imported(266); // Silcoon; Wurmple is its imported prevolution.
        member.level = 14;
        member.exp = pokerogue_data::total_exp_for_level(15, member.species.growth_rate()) - earned;
        let moves_before = member.moves.map(Move::numeric_id);
        let move_count_before = member.move_count();
        member.attack = 10_000;
        member.speed = 10_000;
        run.battle.enemy.hp = 1;
        run.battle.enemy.speed = 1;
        run.take_action(PlayerAction::UseMove(0)).unwrap();

        let bug_bite = move_by_id("BUG_BITE").unwrap().numeric_id;
        assert_eq!(run.stage, MiniRunStage::MoveLearning);
        assert!(run.battle.pending_level_moves.is_empty());
        assert_eq!(run.pending_prevolution_move(), Some((0, bug_bite)));
        let member = run.battle.player.member(0).unwrap();
        assert_eq!(member.level, 15);
        assert_eq!(member.move_count(), move_count_before);
        assert_eq!(member.moves.map(Move::numeric_id), moves_before);

        let saved = run.checkpoint().unwrap();
        assert!(saved.starts_with("POKEROGUE_MINIRUN\t27\n"));
        let mut resumed = MiniRun::from_checkpoint(&saved).unwrap();
        assert_eq!(resumed.pending_prevolution_move(), Some((0, bug_bite)));
        let draws = resumed.battle.rng_draws().to_vec();
        resumed.acknowledge_prevolution_move().unwrap();
        assert_eq!(resumed.stage, MiniRunStage::Reward);
        assert!(resumed.battle.pending_prevolution_moves.is_empty());
        assert_eq!(resumed.battle.rng_draws(), draws.as_slice());
        assert_eq!(
            resumed.battle.player.member(0).unwrap().move_count(),
            move_count_before
        );
    }

    #[test]
    fn multi_member_prevolution_reminders_follow_nested_exp_phase_order() {
        use pokerogue_core::{MiniRun, MiniRunStage};

        let mut run = MiniRun::new_with_wave_limit("NestedPrevoQueueSeed", "DAY", 20).unwrap();
        run.wave = 11;
        let exp_each = (run.battle.enemy_exp_value() / 2.0).floor() as u32;
        {
            let member = run.battle.player.member_mut(0).unwrap();
            member.level = 12;
            member.exp =
                pokerogue_data::total_exp_for_level(13, member.species.growth_rate()) - exp_each;
            member.learn_move(Move::Imported(move_by_id("RAZOR_LEAF").unwrap().numeric_id));
            member.learn_move(Move::Imported(move_by_id("GROWTH").unwrap().numeric_id));
            member.attack = 10_000;
            member.speed = 10_000;
        }
        {
            let member = run.battle.player.member_mut(1).unwrap();
            member.species = Species::Imported(266);
            member.level = 14;
            member.exp =
                pokerogue_data::total_exp_for_level(15, member.species.growth_rate()) - exp_each;
            member.attack = 10_000;
            member.speed = 10_000;
        }
        // The pinned applyPartyExp fixture queues recipients in party order.
        run.battle.switch_player(1).unwrap();
        run.battle.switch_player(0).unwrap();
        run.battle.enemy.hp = 1;
        run.battle.enemy.speed = 1;
        run.take_action(PlayerAction::UseMove(0)).unwrap();

        let bug_bite = move_by_id("BUG_BITE").unwrap().numeric_id;
        assert_eq!(run.stage, MiniRunStage::MoveLearning);
        assert_eq!(run.battle.pending_prevolution_moves, vec![(1, bug_bite)]);
        let rust_prompt_order = run
            .battle
            .pending_level_up_prompts
            .iter()
            .map(|prompt| match prompt {
                pokerogue_core::PendingLevelUpPrompt::LearnMove {
                    party_slot,
                    move_id,
                } => {
                    let movement = pokerogue_data::move_by_numeric_id(*move_id).unwrap();
                    format!("learn:{party_slot}:{}", movement.id)
                }
                pokerogue_core::PendingLevelUpPrompt::Prevolution {
                    party_slot,
                    move_id,
                } => {
                    let member = run.battle.player.member(*party_slot).unwrap();
                    let movement = pokerogue_data::move_by_numeric_id(*move_id).unwrap();
                    format!("message:{}:{}", member.species.name(), movement.name)
                }
            })
            .collect::<Vec<_>>()
            .join("|");
        let source_nested_order = include_str!("../fixtures/level-up-move-queue.csv")
            .lines()
            .find(|line| !line.is_empty() && !line.starts_with('#'))
            .unwrap()
            .split(',')
            .nth(4)
            .unwrap();
        assert_eq!(rust_prompt_order, source_nested_order);
        assert_eq!(
            run.pending_level_up_prompt(),
            Some(pokerogue_core::PendingLevelUpPrompt::LearnMove {
                party_slot: 0,
                move_id: move_by_id("POISON_POWDER").unwrap().numeric_id,
            })
        );
        let mut resumed = MiniRun::from_checkpoint(&run.checkpoint().unwrap()).unwrap();
        assert_eq!(
            resumed.pending_level_up_prompt(),
            run.pending_level_up_prompt()
        );
        let rng = resumed.battle.rng_draws().to_vec();
        resumed.resolve_pending_level_move(Some(0)).unwrap();
        resumed.resolve_pending_level_move(Some(1)).unwrap();
        assert_eq!(resumed.pending_prevolution_move(), Some((1, bug_bite)));
        let resumed = MiniRun::from_checkpoint(&resumed.checkpoint().unwrap()).unwrap();
        let mut resumed = resumed;
        resumed.acknowledge_prevolution_move().unwrap();
        assert_eq!(resumed.stage, MiniRunStage::Reward);
        assert_eq!(resumed.battle.rng_draws(), rng.as_slice());
    }

    #[test]
    fn level_up_stats_and_hp_match_executed_source_methods() {
        for row in include_str!("../fixtures/level-up.csv")
            .lines()
            .filter(|line| !line.starts_with('#'))
        {
            let fields: Vec<_> = row.split(',').collect();
            let species_id = match fields[1] {
                "BULBASAUR" => 1,
                "CHARMANDER" => 4,
                other => panic!("unexpected level-up fixture species {other}"),
            };
            let mut battler = Battler::demo(
                pokerogue_data::Species::from_numeric_id(species_id).unwrap(),
                fields[2].parse().unwrap(),
                [pokerogue_data::Move::Tackle; 2],
            );
            battler.exp = fields[3].parse().unwrap();
            battler.hp = fields[5].parse().unwrap();
            let levels = battler.grant_exp_capped(fields[4].parse().unwrap(), 10);
            assert_eq!(battler.level, fields[6].parse().unwrap(), "{row}");
            assert_eq!(battler.exp, fields[7].parse().unwrap(), "{row}");
            assert_eq!(
                levels.len(),
                if fields[0] == "single-level" { 1 } else { 3 }
            );
            let expected: Vec<i32> = fields[9].split('|').map(|v| v.parse().unwrap()).collect();
            assert_eq!(
                [
                    battler.max_hp,
                    battler.attack,
                    battler.defense,
                    battler.special_attack,
                    battler.special_defense,
                    battler.speed,
                ],
                expected.as_slice(),
                "stats mismatch: {row}"
            );
            assert_eq!(
                battler.hp,
                fields[10].parse().unwrap(),
                "HP mismatch: {row}"
            );
            assert_eq!(fields[11], "1", "LevelUpPhase parent start: {row}");
            assert_eq!(fields[12], "1", "LevelUpPhase end in SKIP mode: {row}");
            assert_eq!(fields[13], fields[6], "highest-level tracking: {row}");
            assert_eq!(fields[14], fields[6], "achievement validation: {row}");
        }
    }

    #[test]
    fn exp_phase_original_schedules_the_level_up_for_rust_transition() {
        let fields: Vec<_> = include_str!("../fixtures/exp-phase.csv")
            .lines()
            .find(|line| !line.starts_with('#'))
            .unwrap()
            .split(',')
            .collect();
        let mut battler = Battler::demo(
            pokerogue_data::Species::Bulbasaur,
            fields[1].parse().unwrap(),
            [pokerogue_data::Move::Tackle; 2],
        );
        battler.exp = fields[2].parse().unwrap();
        let levels = battler.grant_exp_capped(fields[3].parse().unwrap(), 10);
        assert_eq!(fields[6], "1", "original phase calls its parent start");
        assert_eq!(fields[7], "LevelUpPhase:1:5:8");
        assert_eq!(fields[8], "1", "original callback ends after EXP display");
        assert_eq!(battler.level, fields[4].parse().unwrap());
        assert_eq!(battler.exp, fields[5].parse().unwrap());
        assert_eq!(levels, [6, 7, 8]);
        assert_eq!(
            format!("LevelUpPhase:1:5:{}", levels.last().unwrap()),
            fields[7]
        );
    }

    #[test]
    fn boss_segment_damage_matches_original_utility() {
        for row in include_str!("../fixtures/boss-damage.csv")
            .lines()
            .filter(|l| !l.starts_with('#'))
        {
            let f: Vec<_> = row.split(',').collect();
            let index = if f[5] == "-" {
                None
            } else {
                Some(f[5].parse().unwrap())
            };
            let actual = pokerogue_core::calculate_boss_segment_damage(
                f[1].parse().unwrap(),
                f[2].parse().unwrap(),
                f[3].parse().unwrap(),
                f[4].parse().unwrap(),
                index,
            );
            assert_eq!(
                actual,
                (f[6].parse().unwrap(), f[7].parse().unwrap()),
                "{row}"
            );
        }
    }

    #[test]
    fn wild_boss_segment_boosts_and_rng_match_executed_source_method() {
        for row in include_str!("../fixtures/boss-segment-stages.csv")
            .lines()
            .filter(|line| line.starts_with("BossBreak"))
        {
            let fields: Vec<_> = row.split(',').collect();
            let seed = fields[0];
            let segments: u8 = fields[1].parse().unwrap();
            let old_index: u8 = fields[2].parse().unwrap();
            assert_eq!(old_index, segments - 1);
            let values: Vec<i32> = fields[4].split('|').map(|x| x.parse().unwrap()).collect();
            let stages: [i8; 5] = fields[5]
                .split('|')
                .map(|x| x.parse().unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let mut boss = Battler::demo(Species::Bulbasaur, 5, [Move::Tackle; 2]);
            boss.configure_boss_segments(segments);
            boss.max_hp = i32::from(segments) * 100;
            boss.hp = boss.max_hp;
            boss.attack = values[0];
            boss.defense = values[1];
            boss.special_attack = values[2];
            boss.special_defense = values[3];
            boss.speed = values[4];
            for (stat, stage) in [
                BattleStat::Attack,
                BattleStat::Defense,
                BattleStat::SpecialAttack,
                BattleStat::SpecialDefense,
                BattleStat::Speed,
            ]
            .into_iter()
            .zip(stages)
            {
                boss.set_stat_stage(stat, stage);
            }
            let mut rng = BattleRng::new(seed);
            // The fixture directly invokes handleBossSegmentCleared with a
            // controlled segment index; turn zero is its unshifted Alea seed.
            let boosts = boss.apply_boss_segment_cleared(fields[3].parse().unwrap(), &mut rng, 0);
            assert_eq!(boss.boss_segment_index.to_string(), fields[6], "{row}");
            let actual_changes = boosts
                .iter()
                .map(|(stat, stages)| {
                    let name = match *stat {
                        BattleStat::Attack => "ATK",
                        BattleStat::Defense => "DEF",
                        BattleStat::SpecialAttack => "SPATK",
                        BattleStat::SpecialDefense => "SPDEF",
                        BattleStat::Speed => "SPD",
                        _ => unreachable!(),
                    };
                    format!("{name}:{stages}")
                })
                .collect::<Vec<_>>()
                .join("|");
            assert_eq!(actual_changes, fields[7], "{row}");
            assert_eq!(
                boosts.len(),
                fields[7].split('|').filter(|s| !s.is_empty()).count(),
                "{row}"
            );
            let draws = rng
                .draws()
                .iter()
                .map(|d| format!("{}:{}:{}", d.range, d.min, d.value))
                .collect::<Vec<_>>()
                .join("|");
            assert_eq!(draws, fields[8], "{row}");
        }
    }

    #[test]
    fn classic_boss_level_matches_executed_source_fixture() {
        for row in include_str!("../fixtures/boss-level.csv")
            .lines()
            .filter(|line| !line.starts_with('#'))
        {
            let fields: Vec<_> = row.split(',').collect();
            let wave = fields[1].parse::<u32>().unwrap();
            let random_real = fields[2].parse::<f64>().unwrap();
            let wave_final = fields[3] == "1";
            let expected = fields[4].parse::<u8>().unwrap();
            let rng_calls = fields[5].parse::<usize>().unwrap();
            assert_eq!(
                pokerogue_core::classic_boss_level(wave, wave, random_real, wave_final),
                expected,
                "{row}"
            );
            assert_eq!(rng_calls, usize::from(!wave_final && wave % 250 != 0));
        }
    }

    #[test]
    fn boss_segment_count_matches_executed_source_fixture() {
        for row in include_str!("../fixtures/boss-segments.csv")
            .lines()
            .filter(|line| !line.starts_with('#'))
        {
            let fields: Vec<_> = row.split(',').collect();
            let wave = fields[1].parse::<u32>().unwrap();
            let level = fields[2].parse::<u8>().unwrap();
            let base_total = fields[3].parse::<u16>().unwrap();
            let expected = fields[5].parse::<u8>().unwrap();
            assert_eq!(
                pokerogue_core::classic_boss_segments(wave, level, base_total),
                expected,
                "{row}"
            );
        }
    }

    #[test]
    fn imported_modifier_pool_catalog_preserves_original_order_and_weights() {
        let fixture = include_str!("../fixtures/modifier-pools.csv");
        let rows: Vec<_> = fixture
            .lines()
            .filter(|line| line.starts_with("modifier,"))
            .collect();
        assert_eq!(
            rows.len(),
            pokerogue_data::IMPORTED_PLAYER_MODIFIER_POOL.len()
        );
        let tier_names = ["COMMON", "GREAT", "ULTRA", "ROGUE", "MASTER"];
        for (row, entry) in rows
            .iter()
            .zip(pokerogue_data::IMPORTED_PLAYER_MODIFIER_POOL)
        {
            let fields: Vec<_> = row.split(',').collect();
            assert_eq!(fields[1], tier_names[entry.tier as usize]);
            assert_eq!(fields[2].parse::<u8>().unwrap(), entry.tier);
            assert_eq!(fields[3].parse::<u16>().unwrap(), entry.pool_index);
            assert_eq!(fields[4], entry.id);
            let weight = (fields[5] != "dynamic").then(|| fields[5].parse::<u16>().unwrap());
            let max_weight = (!fields[6].is_empty()).then(|| fields[6].parse::<u16>().unwrap());
            assert_eq!(weight, entry.static_weight);
            assert_eq!(max_weight, entry.max_weight);
        }
    }

    #[test]
    fn natural_reward_candidate_matches_executed_source_method_and_rng() {
        use pokerogue_core::rewards::roll_natural_player_candidate;
        let fields: Vec<_> = include_str!("../fixtures/natural-reward-candidate.csv")
            .lines()
            .find(|line| line.starts_with("candidate,"))
            .unwrap()
            .split(',')
            .collect();
        let expected_draws: Vec<_> = fields[3]
            .split('|')
            .map(|draw| {
                let parts: Vec<_> = draw.split(':').collect();
                (
                    parts[0].parse::<u32>().unwrap(),
                    parts[1].parse::<i32>().unwrap(),
                    parts[2].parse::<i32>().unwrap(),
                )
            })
            .collect();
        let candidate =
            roll_natural_player_candidate(fields[2], fields[1].parse().unwrap(), 0, true, |tier| {
                if tier == 0 {
                    vec![6, 2, 0, 0, 0, 0, 2, 4, 2, 2]
                } else {
                    vec![1]
                }
            })
            .unwrap();
        assert_eq!(candidate.id, fields[4]);
        assert_eq!(candidate.tier, fields[6].parse::<u8>().unwrap());
        assert_eq!(candidate.upgrades, fields[5].parse::<u32>().unwrap());
        assert_eq!(candidate.pool_index, fields[7].parse::<usize>().unwrap());
        assert_eq!(
            candidate
                .rng_draws
                .iter()
                .map(|draw| (draw.range, draw.min, draw.value))
                .collect::<Vec<_>>(),
            expected_draws
        );
    }

    #[test]
    fn mini_run_live_party_rolls_source_natural_candidate_and_rng() {
        let fields: Vec<_> = include_str!("../fixtures/natural-reward-candidate.csv")
            .lines()
            .find(|line| line.starts_with("candidate,"))
            .unwrap()
            .split(',')
            .collect();
        let mut run = pokerogue_core::MiniRun::new(fields[2], "DAY").unwrap();
        run.wave = fields[1].parse().unwrap();
        let source_weights: Vec<u32> = include_str!("../fixtures/common-rewards.csv")
            .lines()
            .find(|line| line.starts_with("common,1,1,5,-,100:100:0:25:0|100:100:0:35:0,"))
            .unwrap()
            .split(',')
            .nth(6)
            .unwrap()
            .split('|')
            .map(|weight| weight.parse().unwrap())
            .collect();
        assert_eq!(run.common_reward_pool_weights().as_slice(), source_weights);

        let candidate = run.natural_reward_candidate().unwrap();
        assert_eq!(candidate.id, fields[4]);
        assert_eq!(candidate.tier, fields[6].parse::<u8>().unwrap());
        assert_eq!(candidate.upgrades, fields[5].parse::<u32>().unwrap());
        assert_eq!(candidate.pool_index, fields[7].parse::<usize>().unwrap());
        let expected_draws: Vec<_> = fields[3]
            .split('|')
            .map(|draw| {
                let parts: Vec<_> = draw.split(':').collect();
                (
                    parts[0].parse::<u32>().unwrap(),
                    parts[1].parse::<i32>().unwrap(),
                    parts[2].parse::<i32>().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            candidate
                .rng_draws
                .iter()
                .map(|draw| (draw.range, draw.min, draw.value))
                .collect::<Vec<_>>(),
            expected_draws
        );
    }

    #[test]
    fn mini_run_natural_common_offer_matches_source_options_and_rng() {
        let fields: Vec<_> = include_str!("../fixtures/natural-reward-candidate.csv")
            .lines()
            .find(|line| line.starts_with("offer,"))
            .unwrap()
            .split(',')
            .collect();
        let mut run = pokerogue_core::MiniRun::new(fields[2], "DAY").unwrap();
        run.wave = fields[1].parse().unwrap();
        let offer = run.natural_common_reward_offer().unwrap();
        let actual: Vec<_> = offer
            .options
            .iter()
            .map(|option| {
                format!(
                    "{}:{}:{}:{}",
                    option.name, option.upgrades, option.tier, option.pool_index
                )
            })
            .collect();
        let expected: Vec<_> = fields[3].split('|').collect();
        assert_eq!(actual, expected);
        let expected_draws: Vec<_> = fields[4]
            .split('|')
            .map(|draw| {
                let parts: Vec<_> = draw.split(':').collect();
                (
                    parts[0].parse::<u32>().unwrap(),
                    parts[1].parse::<i32>().unwrap(),
                    parts[2].parse::<i32>().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            offer
                .rng_draws
                .iter()
                .map(|draw| (draw.range, draw.min, draw.value))
                .collect::<Vec<_>>(),
            expected_draws
        );
        run.stage = pokerogue_core::MiniRunStage::Reward;
        let resumed = pokerogue_core::MiniRun::from_checkpoint(&run.checkpoint().unwrap()).unwrap();
        assert_eq!(resumed.natural_common_reward_offer().unwrap(), offer);
    }

    #[test]
    fn mini_run_natural_great_recovery_offer_applies_and_persists() {
        use pokerogue_core::{MiniRun, MiniRunStage};

        let source_row = include_str!("../fixtures/great-recovery-offer.csv")
            .lines()
            .find(|line| line.starts_with("GreatRecoverySeed"))
            .unwrap();
        let fields: Vec<_> = source_row.split(',').collect();
        let mut run = MiniRun::new_with_wave_limit(fields[0], "DAY", 20).unwrap();
        run.wave = fields[1].parse().unwrap();
        run.stage = MiniRunStage::Reward;
        let healthy = run.battle.player.member_mut(0).unwrap();
        healthy.max_hp = 160;
        healthy.hp = 18;
        run.battle.player.member_mut(1).unwrap().hp = 0;

        let source_common: Vec<u32> = include_str!("../fixtures/common-rewards.csv")
            .lines()
            .find(|line| line.ends_with("6|2|3|1|0|0|2|4|2|2"))
            .unwrap()
            .split(',')
            .nth(6)
            .unwrap()
            .split('|')
            .map(|weight| weight.parse().unwrap())
            .collect();
        assert_eq!(
            run.common_reward_pool_weights().as_slice(),
            source_common.as_slice()
        );

        let source_great: Vec<u32> = include_str!("../fixtures/great-rewards.csv")
            .lines()
            .find(|line| line.starts_with("great,mini-run-great-recovery-wave-19,"))
            .unwrap()
            .split(',')
            .nth(3)
            .unwrap()
            .split('|')
            .map(|weight| weight.parse().unwrap())
            .collect();
        assert_eq!(
            run.modeled_player_modifier_tier_weights(1).unwrap(),
            source_great
        );
        let offer = run.natural_supported_reward_offer().unwrap();
        let actual_options: Vec<_> = offer
            .options
            .iter()
            .map(|option| {
                format!(
                    "{}:{}:{}:{}",
                    option.id, option.upgrades, option.tier, option.pool_index
                )
            })
            .collect();
        assert_eq!(actual_options.join("|"), fields[2]);
        let source_draws: Vec<_> = fields[3]
            .split('|')
            .map(|draw| {
                let parts: Vec<_> = draw.split(':').collect();
                (
                    parts[0].parse::<u32>().unwrap(),
                    parts[1].parse::<i32>().unwrap(),
                    parts[2].parse::<i32>().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            offer
                .rng_draws
                .iter()
                .map(|draw| (draw.range, draw.min, draw.value))
                .collect::<Vec<_>>(),
            source_draws
        );

        let index = offer
            .options
            .iter()
            .position(|option| option.id == "REVIVE")
            .unwrap();
        let target = 1;
        let expected = run.battle.player.member(target).unwrap().max_hp * 50 / 100;
        let battle_draw_count = run.battle.rng_draws().len();
        run.claim_natural_supported_reward(index, target).unwrap();
        assert_eq!(run.battle.player.member(target).unwrap().hp, expected);
        assert_eq!(run.battle.rng_draws().len(), battle_draw_count);
        let resumed = MiniRun::from_checkpoint(&run.checkpoint().unwrap()).unwrap();
        assert_eq!(resumed.wave_limit, 20);
        assert_eq!(resumed.battle.player.member(target).unwrap().hp, expected);
        assert!(resumed.reward_claimed);
    }

    #[test]
    fn natural_full_restore_offer_and_hp_effect_match_source_slices_and_save() {
        use pokerogue_core::{MiniRun, MiniRunStage, rewards::Reward};

        let source_row = include_str!("../fixtures/great-recovery-offer.csv")
            .lines()
            .find(|line| line.starts_with("GreatFullRestoreSeed"))
            .unwrap();
        let fields: Vec<_> = source_row.split(',').collect();
        let mut run = MiniRun::new_with_wave_limit(fields[0], "DAY", 20).unwrap();
        run.wave = fields[1].parse().unwrap();
        run.stage = MiniRunStage::Reward;
        for slot in 0..2 {
            let member = run.battle.player.member_mut(slot).unwrap();
            let needed = pokerogue_data::total_exp_for_level(100, member.species.growth_rate())
                .saturating_sub(member.exp);
            member.grant_exp(needed);
            member.hp = member.max_hp.saturating_sub(100);
            assert!(member.hp > 0);
        }

        let source_great: Vec<u32> = include_str!("../fixtures/great-rewards.csv")
            .lines()
            .find(|line| line.starts_with("great,mini-run-great-full-restore-wave-19,"))
            .unwrap()
            .split(',')
            .nth(3)
            .unwrap()
            .split('|')
            .map(|weight| weight.parse().unwrap())
            .collect();
        assert_eq!(
            run.modeled_player_modifier_tier_weights(1).unwrap(),
            source_great
        );
        let source_common: Vec<u32> = include_str!("../fixtures/common-rewards.csv")
            .lines()
            .find(|line| line.starts_with("common,1,19,5,-,100:300:0:35:0|100:300:0:35:0,"))
            .unwrap()
            .split(',')
            .nth(6)
            .unwrap()
            .split('|')
            .map(|weight| weight.parse().unwrap())
            .collect();
        assert_eq!(
            run.common_reward_pool_weights().as_slice(),
            source_common.as_slice()
        );
        let offer = run.natural_supported_reward_offer().unwrap();
        let expected_options: Vec<_> = fields[2]
            .split('|')
            .map(|option| {
                let parts: Vec<_> = option.split(':').collect();
                format!("{}:{}:{}:{}", parts[0], parts[1], parts[2], parts[3])
            })
            .collect();
        let actual_options: Vec<_> = offer
            .options
            .iter()
            .map(|option| {
                format!(
                    "{}:{}:{}:{}",
                    option.id, option.upgrades, option.tier, option.pool_index
                )
            })
            .collect();
        assert_eq!(actual_options, expected_options);
        let expected_draws: Vec<_> = fields[3]
            .split('|')
            .map(|draw| {
                let parts: Vec<_> = draw.split(':').collect();
                (
                    parts[0].parse::<u32>().unwrap(),
                    parts[1].parse::<i32>().unwrap(),
                    parts[2].parse::<i32>().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            offer
                .rng_draws
                .iter()
                .map(|draw| (draw.range, draw.min, draw.value))
                .collect::<Vec<_>>(),
            expected_draws
        );

        let target_index = offer
            .options
            .iter()
            .position(|option| option.id == "FULL_RESTORE")
            .unwrap();
        assert_eq!(Reward::FullRestore.heal(100, 300), Some(300));
        run.claim_natural_supported_reward(target_index, 0).unwrap();
        let healed_hp = run.battle.player.member(0).unwrap().hp;
        assert_eq!(healed_hp, run.battle.player.member(0).unwrap().max_hp);
        let resumed = MiniRun::from_checkpoint(&run.checkpoint().unwrap()).unwrap();
        assert_eq!(resumed.battle.player.member(0).unwrap().hp, healed_hp);
    }

    #[test]
    fn pp_up_matches_executed_source_and_natural_offer_persists() {
        use pokerogue_core::{MiniRun, MiniRunStage, rewards::Reward};
        use pokerogue_data::Move;

        for row in include_str!("../fixtures/pp-up.csv")
            .lines()
            .filter(|line| !line.starts_with('#'))
        {
            let f: Vec<_> = row.split(',').collect();
            let base: u8 = f[1].parse().unwrap();
            let initial: u8 = f[2].parse().unwrap();
            let override_pp: u8 = f[3].parse().unwrap();
            let up_points: u8 = f[4].parse().unwrap();
            let eligible = f[5] == "1";
            let mut battler = pokerogue_core::Battler::demo(
                pokerogue_data::Species::Bulbasaur,
                5,
                [Move::Tackle, Move::Ember],
            );
            let data_move = if base == 25 { "EMBER" } else { "TACKLE" };
            battler.moves[0] =
                Move::Imported(pokerogue_data::move_by_id(data_move).unwrap().numeric_id);
            if initial > 0 {
                battler.move_pp_ups[0] = initial;
            }
            if override_pp == 0 && base != 4 {
                let source_max = base + initial * (base / 5).max(1);
                assert_eq!(battler.move_pp_max(0), Some(source_max), "{}", f[0]);
            }
            if override_pp == 0 && base != 4 {
                assert_eq!(battler.can_apply_pp_up(0), eligible, "{}", f[0]);
            }
            if eligible {
                battler.move_pp_ups[0] = (battler.move_pp_ups[0] + up_points).min(3);
                assert_eq!(
                    battler.move_pp_ups[0],
                    f[6].parse::<u8>().unwrap(),
                    "{}",
                    f[0]
                );
                assert_eq!(
                    battler.move_pp_max(0),
                    Some(f[7].parse().unwrap()),
                    "{}",
                    f[0]
                );
            }
        }

        let row = include_str!("../fixtures/great-recovery-offer.csv")
            .lines()
            .find(|line| line.starts_with("GreatPpUpSeed"))
            .unwrap();
        let fields: Vec<_> = row.split(',').collect();
        let mut run = MiniRun::new_with_wave_limit(fields[0], "DAY", 20).unwrap();
        run.money = 0;
        run.wave = fields[1].parse().unwrap();
        run.stage = MiniRunStage::Reward;
        run.battle.player.member_mut(0).unwrap().max_hp = 160;
        run.battle.player.member_mut(0).unwrap().hp = 18;
        run.battle.player.member_mut(1).unwrap().hp = 0;
        run.battle.player.member_mut(0).unwrap().moves[0] = Move::Tackle;
        let offer = run.natural_supported_reward_offer().unwrap();
        assert_eq!(
            offer
                .options
                .iter()
                .map(|o| format!("{}:{}:{}:{}", o.id, o.upgrades, o.tier, o.pool_index))
                .collect::<Vec<_>>()
                .join("|"),
            fields[2]
        );
        let source_draws: Vec<_> = fields[3]
            .split('|')
            .map(|draw| {
                let p: Vec<_> = draw.split(':').collect();
                (
                    p[0].parse::<u32>().unwrap(),
                    p[1].parse::<i32>().unwrap(),
                    p[2].parse::<i32>().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            offer
                .rng_draws
                .iter()
                .map(|draw| (draw.range, draw.min, draw.value))
                .collect::<Vec<_>>(),
            source_draws
        );
        let index = offer
            .options
            .iter()
            .position(|option| option.id == "PP_UP")
            .unwrap();
        assert!(run.apply_modifier_reward(Reward::PpUp, 0, None).is_err());
        run.claim_natural_supported_reward_for_move(index, 0, Some(0))
            .unwrap();
        let checkpoint = run.checkpoint().unwrap();
        let restored = MiniRun::from_checkpoint(&checkpoint).unwrap();
        assert_eq!(restored.battle.player.member(0).unwrap().move_pp_ups[0], 1);
        assert_eq!(
            restored.battle.player.member(0).unwrap().move_pp_max(0),
            Some(42)
        );
        let migrated_v16 = MiniRun::from_checkpoint(&checkpoint_as_v16(&checkpoint)).unwrap();
        assert_eq!(
            migrated_v16.battle.player.member(0).unwrap().move_pp_ups,
            [0, 0, 0, 0]
        );
    }

    #[test]
    fn ultra_exp_charm_offer_application_and_save_match_source_methods() {
        use pokerogue_core::{
            MiniRun, MiniRunStage, PlayerAction, apply_exp_boosters, rewards::Reward,
        };
        let row = include_str!("../fixtures/great-recovery-offer.csv")
            .lines()
            .find(|line| line.starts_with("UltraExpCharmSeed"))
            .unwrap();
        let fields: Vec<_> = row.split(',').collect();
        let mut run = MiniRun::new_with_wave_limit(fields[0], "DAY", 20).unwrap();
        run.money = 0;
        run.wave = fields[1].parse().unwrap();
        run.stage = MiniRunStage::Reward;
        run.battle.player.member_mut(0).unwrap().max_hp = 160;
        run.battle.player.member_mut(0).unwrap().hp = 18;
        run.battle.player.member_mut(1).unwrap().hp = 0;
        let offer = run.natural_supported_reward_offer().unwrap();
        assert_eq!(
            offer
                .options
                .iter()
                .map(|o| format!("{}:{}:{}:{}", o.id, o.upgrades, o.tier, o.pool_index))
                .collect::<Vec<_>>()
                .join("|"),
            fields[2]
        );
        let expected_draws: Vec<_> = fields[3]
            .split('|')
            .map(|draw| {
                let values: Vec<_> = draw.split(':').collect();
                (
                    values[0].parse::<u32>().unwrap(),
                    values[1].parse::<i32>().unwrap(),
                    values[2].parse::<i32>().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            offer
                .rng_draws
                .iter()
                .map(|draw| (draw.range, draw.min, draw.value))
                .collect::<Vec<_>>(),
            expected_draws
        );
        let index = offer
            .options
            .iter()
            .position(|option| option.id == "EXP_CHARM")
            .unwrap();
        run.claim_natural_supported_reward(index, 0).unwrap();
        assert_eq!(run.exp_charm_stacks, 1);
        assert_eq!(apply_exp_boosters(100, run.exp_charm_stacks, 0), 125);
        let save = run.checkpoint().unwrap();
        assert_eq!(MiniRun::from_checkpoint(&save).unwrap().exp_charm_stacks, 1);
        let mut resumed = MiniRun::from_checkpoint(&save).unwrap();
        let mut baseline = MiniRun::from_checkpoint(&save).unwrap();
        baseline.exp_charm_stacks = 0;
        resumed.continue_after_reward().unwrap();
        baseline.continue_after_reward().unwrap();
        assert_eq!(resumed.wave, 20);
        assert_eq!(resumed.stage, MiniRunStage::Battle);
        resumed.battle.enemy.hp = 1;
        baseline.battle.enemy.hp = 1;
        for next in [&mut resumed, &mut baseline] {
            let active = next.battle.player.active_slot();
            let member = next.battle.player.member_mut(active).unwrap();
            member.hp = member.max_hp;
            next.take_action(PlayerAction::UseMove(0)).unwrap();
            assert_eq!(next.stage, MiniRunStage::Reward);
        }
        assert_eq!(
            resumed.reward_experience,
            apply_exp_boosters(baseline.reward_experience, 1, 0),
            "the persisted Ultra EXP Charm affects the next wave's award using the original multiplier"
        );
        let mut capped = MiniRun::new("ExpCharmCap", "DAY").unwrap();
        capped.stage = MiniRunStage::Reward;
        capped.exp_charm_stacks = 98;
        capped
            .apply_modifier_reward(Reward::ExpCharm, 0, None)
            .unwrap();
        assert_eq!(capped.exp_charm_stacks, 99);
        capped.stage = MiniRunStage::Battle;
        assert_eq!(
            capped.modeled_player_modifier_tier_weights(2).unwrap()[21],
            0
        );
        capped.exp_charm_stacks = 98;
        assert_eq!(
            capped.modeled_player_modifier_tier_weights(2).unwrap()[21],
            8
        );
    }

    #[test]
    fn mini_run_exp_share_awards_reserve_xp_after_a_real_victory() {
        use pokerogue_core::{MiniRun, MiniRunStage};
        let mut run = MiniRun::new("MiniRunExpSharePath", "DAY").unwrap();
        run.exp_share_stacks = 1;
        let active = run.battle.player.active_slot();
        let reserve = (0..run.battle.player.len())
            .find(|slot| *slot != active)
            .unwrap();
        let active_member = run.battle.player.member_mut(active).unwrap();
        active_member.hp = active_member.max_hp;
        active_member.moves[0] = pokerogue_data::Move::Ember;
        let reserve_member = run.battle.player.member_mut(reserve).unwrap();
        reserve_member.hp = reserve_member.max_hp;
        let reserve_exp_before = reserve_member.exp;
        let growl = pokerogue_data::move_by_id("GROWL").unwrap();
        run.battle.enemy.hp = 1;
        run.battle.enemy.moves = [pokerogue_data::Move::Imported(growl.numeric_id); 4];
        let enemy_exp = run.battle.enemy_exp_value();
        run.take_action(PlayerAction::UseMove(0)).unwrap();
        assert_eq!(run.stage, MiniRunStage::Reward);
        let expected_total = enemy_exp.floor() as u32 + (enemy_exp * 0.2).floor() as u32;
        assert_eq!(run.reward_experience, expected_total);
        let reserve_after = run.battle.player.member(reserve).unwrap();
        assert!(reserve_after.exp > reserve_exp_before || reserve_after.level > 1);
    }

    #[test]
    fn ultra_exp_share_offer_claim_and_checkpoint_match_source_fixture() {
        use pokerogue_core::{MiniRun, MiniRunStage, rewards::Reward};
        let row = include_str!("../fixtures/great-recovery-offer.csv")
            .lines()
            .find(|line| line.starts_with("UltraExpShareSeed"))
            .unwrap();
        let fields: Vec<_> = row.split(',').collect();
        let mut run = MiniRun::new_with_wave_limit(fields[0], "DAY", 20).unwrap();
        run.wave = fields[1].parse().unwrap();
        run.stage = MiniRunStage::Reward;
        run.battle.player.member_mut(0).unwrap().max_hp = 160;
        run.battle.player.member_mut(0).unwrap().hp = 18;
        run.battle.player.member_mut(1).unwrap().hp = 0;
        let offer = run.natural_supported_reward_offer().unwrap();
        assert_eq!(
            offer
                .options
                .iter()
                .map(|o| format!("{}:{}:{}:{}", o.id, o.upgrades, o.tier, o.pool_index))
                .collect::<Vec<_>>()
                .join("|"),
            fields[2]
        );
        let source_draws: Vec<_> = fields[3]
            .split('|')
            .map(|draw| {
                let values: Vec<_> = draw.split(':').collect();
                (
                    values[0].parse::<u32>().unwrap(),
                    values[1].parse::<i32>().unwrap(),
                    values[2].parse::<i32>().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            offer
                .rng_draws
                .iter()
                .map(|draw| (draw.range, draw.min, draw.value))
                .collect::<Vec<_>>(),
            source_draws
        );
        let selected = offer
            .options
            .iter()
            .position(|option| option.id == "EXP_SHARE")
            .unwrap();
        let draws_before = run.battle.rng_draws().len();
        run.claim_natural_supported_reward(selected, 0).unwrap();
        assert_eq!(run.exp_share_stacks, 1);
        assert_eq!(run.battle.rng_draws().len(), draws_before);
        let save = run.checkpoint().unwrap();
        assert!(save.starts_with("POKEROGUE_MINIRUN\t27\n"));
        assert_eq!(MiniRun::from_checkpoint(&save).unwrap().exp_share_stacks, 1);
        let old_v19 = save
            .replacen("POKEROGUE_MINIRUN\t27", "POKEROGUE_MINIRUN\t19", 1)
            .lines()
            .filter(|line| !line.starts_with("amulet_coin_stacks\t"))
            .filter(|line| {
                !line.starts_with("moveset\t")
                    && !line.starts_with("used_tms\t")
                    && !line.starts_with("friendship\t")
                    && !line.starts_with("soothe_bell\t")
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        assert_eq!(
            MiniRun::from_checkpoint(&old_v19).unwrap().exp_share_stacks,
            1
        );
        let old_v18 = old_v19
            .replacen("POKEROGUE_MINIRUN\t19", "POKEROGUE_MINIRUN\t18", 1)
            .lines()
            .filter(|line| !line.starts_with("exp_share_stacks\t"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        assert_eq!(
            MiniRun::from_checkpoint(&old_v18).unwrap().exp_share_stacks,
            0
        );

        let mut capped = MiniRun::new("ExpShareCap", "DAY").unwrap();
        capped.stage = MiniRunStage::Reward;
        capped.exp_share_stacks = 4;
        capped
            .apply_modifier_reward(Reward::ExpShare, 0, None)
            .unwrap();
        assert_eq!(capped.exp_share_stacks, 5);
        capped.stage = MiniRunStage::Battle;
        assert_eq!(
            capped.modeled_player_modifier_tier_weights(2).unwrap()[22],
            0
        );
        capped.exp_share_stacks = 4;
        assert_eq!(
            capped.modeled_player_modifier_tier_weights(2).unwrap()[22],
            10
        );
    }

    #[test]
    fn ultra_amulet_coin_offer_money_effect_and_save_match_source() {
        use pokerogue_core::{
            MiniRun, MiniRunStage,
            rewards::{self, Reward},
        };
        let row = include_str!("../fixtures/great-recovery-offer.csv")
            .lines()
            .find(|line| line.starts_with("UltraAmuletCoinSeed"))
            .unwrap();
        let fields: Vec<_> = row.split(',').collect();
        let parse_weights = |prefix: &str, key: &str| -> Vec<u32> {
            include_str!("../fixtures/great-rewards.csv")
                .lines()
                .find(|line| line.starts_with(&format!("{prefix},{key},")))
                .unwrap()
                .split(',')
                .nth(3)
                .unwrap()
                .split('|')
                .map(|value| value.parse().unwrap())
                .collect()
        };
        let common: Vec<u32> = include_str!("../fixtures/common-rewards.csv")
            .lines()
            .find(|line| line.starts_with("common,") && line.ends_with("6|2|0|0|0|0|2|4|2|2"))
            .unwrap()
            .split(',')
            .nth(6)
            .unwrap()
            .split('|')
            .map(|value| value.parse().unwrap())
            .collect();
        let ultra: Vec<u32> = include_str!("../fixtures/ultra-rewards.csv")
            .lines()
            .find(|line| line.starts_with("ultra,mini-run-wave-19,"))
            .unwrap()
            .split(',')
            .nth(3)
            .unwrap()
            .split('|')
            .map(|value| value.parse().unwrap())
            .collect();
        let offer = rewards::roll_natural_player_offer(
            fields[0],
            fields[1].parse().unwrap(),
            0,
            true,
            3,
            15,
            |tier| match tier {
                0 => common.clone(),
                1 => parse_weights("great", "mini-run-healthy-wave-19"),
                2 => ultra.clone(),
                _ => Vec::new(),
            },
            |id, rng| match pokerogue_data::IMPORTED_PLAYER_MODIFIER_POOL
                .iter()
                .find(|entry| entry.id == id)
                .map(|entry| entry.tier)
            {
                Some(0) => rewards::resolve_common_player_modifier(id, rng, 0, &[]),
                Some(2) => {
                    let identity = rewards::resolve_ultra_direct_modifier(id)?;
                    Some(rewards::NaturalRewardIdentity {
                        name: id.to_owned(),
                        group: identity.group,
                        generated_variant: None,
                    })
                }
                _ => None,
            },
        )
        .unwrap();
        assert_eq!(
            offer
                .options
                .iter()
                .map(|o| format!("{}:{}:{}:{}", o.id, o.upgrades, o.tier, o.pool_index))
                .collect::<Vec<_>>()
                .join("|"),
            fields[2]
        );
        let expected_draws: Vec<_> = fields[3]
            .split('|')
            .map(|draw| {
                let parts: Vec<_> = draw.split(':').collect();
                (
                    parts[0].parse::<u32>().unwrap(),
                    parts[1].parse::<i32>().unwrap(),
                    parts[2].parse::<i32>().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            offer
                .rng_draws
                .iter()
                .map(|draw| (draw.range, draw.min, draw.value))
                .collect::<Vec<_>>(),
            expected_draws
        );
        let mut run = MiniRun::new_with_wave_limit(fields[0], "DAY", 20).unwrap();
        run.wave = fields[1].parse().unwrap();
        run.stage = MiniRunStage::Reward;
        run.apply_modifier_reward(Reward::AmuletCoin, 0, None)
            .unwrap();
        assert_eq!(run.amulet_coin_stacks, 1);
        run.stage = MiniRunStage::Reward;
        run.reward_claimed = false;
        run.money = 0;
        run.apply_modifier_reward(Reward::BigNugget, 0, None)
            .unwrap();
        let source_base = include_str!("../fixtures/money-reward.csv")
            .lines()
            .find(|line| line.starts_with("big-nugget-wave-19,"))
            .unwrap()
            .split(',')
            .nth(4)
            .unwrap()
            .parse::<u64>()
            .unwrap();
        assert_eq!(run.money, pokerogue_core::apply_amulet_coin(source_base, 1));
        let save = run.checkpoint().unwrap();
        assert!(save.starts_with("POKEROGUE_MINIRUN\t27\n"));
        assert_eq!(
            MiniRun::from_checkpoint(&save).unwrap().amulet_coin_stacks,
            1
        );
        let legacy_v19 = save
            .replacen("POKEROGUE_MINIRUN\t27", "POKEROGUE_MINIRUN\t19", 1)
            .lines()
            .filter(|line| !line.starts_with("amulet_coin_stacks\t"))
            .filter(|line| {
                !line.starts_with("moveset\t")
                    && !line.starts_with("used_tms\t")
                    && !line.starts_with("friendship\t")
                    && !line.starts_with("soothe_bell\t")
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        assert_eq!(
            MiniRun::from_checkpoint(&legacy_v19)
                .unwrap()
                .amulet_coin_stacks,
            0
        );

        let mut capped = MiniRun::new("AmuletCoinStackCap", "DAY").unwrap();
        capped.stage = MiniRunStage::Reward;
        capped.amulet_coin_stacks = 4;
        capped
            .apply_modifier_reward(Reward::AmuletCoin, 0, None)
            .unwrap();
        assert_eq!(capped.amulet_coin_stacks, 5);
        assert_eq!(
            capped.modeled_player_modifier_tier_weights(2).unwrap()[7],
            0
        );
    }

    #[test]
    fn live_seeded_amulet_offer_can_be_claimed_and_saved() {
        use pokerogue_core::{MiniRun, MiniRunStage};

        let mut run = MiniRun::new_with_wave_limit("LiveAmulet000766", "DAY", 20).unwrap();
        run.wave = 19;
        run.stage = MiniRunStage::Reward;
        let offer = run.natural_supported_reward_offer().unwrap();
        let source = include_str!("../fixtures/great-recovery-offer.csv")
            .lines()
            .find(|line| line.starts_with("LiveAmulet000766,"))
            .unwrap();
        let fields: Vec<_> = source.split(',').collect();
        assert_eq!(
            offer
                .options
                .iter()
                .map(|option| format!(
                    "{}:{}:{}:{}",
                    option.id, option.upgrades, option.tier, option.pool_index
                ))
                .collect::<Vec<_>>(),
            fields[2].split('|').collect::<Vec<_>>()
        );
        let expected_draws: Vec<_> = fields[3]
            .split('|')
            .map(|draw| {
                let parts: Vec<_> = draw.split(':').collect();
                (
                    parts[0].parse::<u32>().unwrap(),
                    parts[1].parse::<i32>().unwrap(),
                    parts[2].parse::<i32>().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            offer
                .rng_draws
                .iter()
                .map(|draw| (draw.range, draw.min, draw.value))
                .collect::<Vec<_>>(),
            expected_draws
        );
        let draws = offer.rng_draws.clone();
        let amulet_index = offer
            .options
            .iter()
            .position(|option| option.id == "AMULET_COIN")
            .unwrap();
        run.claim_natural_supported_reward(amulet_index, 0).unwrap();
        assert_eq!(run.amulet_coin_stacks, 1);
        assert!(run.reward_claimed);
        let resumed = MiniRun::from_checkpoint(&run.checkpoint().unwrap()).unwrap();
        assert_eq!(resumed.amulet_coin_stacks, 1);
        assert_eq!(
            resumed.natural_supported_reward_offer().unwrap().rng_draws,
            draws
        );
    }

    #[test]
    fn ultra_ball_natural_offer_capture_and_inventory_match_source_slices() {
        use pokerogue_core::{MiniRun, MiniRunStage, PlayerAction, Pokeball, rewards::Reward};

        let row = include_str!("../fixtures/great-recovery-offer.csv")
            .lines()
            .find(|line| line.starts_with("UltraBallSeed"))
            .unwrap();
        let fields: Vec<_> = row.split(',').collect();
        let mut run = MiniRun::new_with_wave_limit(fields[0], "DAY", 20).unwrap();
        run.wave = fields[1].parse().unwrap();
        run.stage = MiniRunStage::Reward;
        run.battle.player.member_mut(0).unwrap().max_hp = 160;
        run.battle.player.member_mut(0).unwrap().hp = 18;
        run.battle.player.member_mut(1).unwrap().hp = 0;
        let offer = run.natural_supported_reward_offer().unwrap();
        assert_eq!(
            offer
                .options
                .iter()
                .map(|o| format!("{}:{}:{}:{}", o.id, o.upgrades, o.tier, o.pool_index))
                .collect::<Vec<_>>()
                .join("|"),
            fields[2]
        );
        let expected_draws: Vec<_> = fields[3]
            .split('|')
            .map(|draw| {
                let values: Vec<_> = draw.split(':').collect();
                (
                    values[0].parse::<u32>().unwrap(),
                    values[1].parse::<i32>().unwrap(),
                    values[2].parse::<i32>().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            offer
                .rng_draws
                .iter()
                .map(|draw| (draw.range, draw.min, draw.value))
                .collect::<Vec<_>>(),
            expected_draws
        );
        let index = offer
            .options
            .iter()
            .position(|option| option.id == "ULTRA_BALL")
            .unwrap();
        assert_eq!(Reward::UltraBalls.tier(), 2);
        run.claim_natural_supported_reward(index, 0).unwrap();
        assert_eq!(run.ultraballs, 5);
        let saved = run.checkpoint().unwrap();
        assert!(saved.starts_with("POKEROGUE_MINIRUN\t27\n"));
        let resumed = MiniRun::from_checkpoint(&saved).unwrap();
        assert_eq!(resumed.ultraballs, 5);
        let legacy = saved
            .replacen("POKEROGUE_MINIRUN\t27", "POKEROGUE_MINIRUN\t17", 1)
            .lines()
            .filter(|line| {
                !line.starts_with("ultraballs\t")
                    && !line.starts_with("exp_share_stacks\t")
                    && !line.starts_with("amulet_coin_stacks\t")
                    && !line.starts_with("moveset\t")
                    && !line.starts_with("used_tms\t")
                    && !line.starts_with("friendship\t")
                    && !line.starts_with("soothe_bell\t")
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        assert_eq!(MiniRun::from_checkpoint(&legacy).unwrap().ultraballs, 0);

        let mut capture = MiniRun::new("UltraBallCaptureSeed", "DAY").unwrap();
        capture.ultraballs = 1;
        let enemy = &capture.battle.enemy;
        let mut oracle_rng = BattleRng::new(
            capture
                .battle
                .initial_encounter
                .as_ref()
                .unwrap()
                .battle_seed
                .clone(),
        );
        let expected = pokerogue_core::attempt_capture(
            &mut oracle_rng,
            1,
            enemy.species.catch_rate(),
            enemy.hp,
            enemy.max_hp,
            Pokeball::Ultra,
            1.0,
            1.0,
            0,
        )
        .unwrap();
        capture.take_action(PlayerAction::ThrowUltraBall).unwrap();
        assert_eq!(capture.ultraballs, 0);
        assert_eq!(
            capture
                .battle
                .last_capture_attempt
                .as_ref()
                .unwrap()
                .modified_catch_rate,
            expected.modified_catch_rate
        );
        assert_eq!(
            capture
                .battle
                .last_capture_attempt
                .as_ref()
                .unwrap()
                .shake_rolls,
            expected.shake_rolls
        );

        let mut empty = MiniRun::new("NoUltraBallSeed", "DAY").unwrap();
        let draws = empty.battle.rng_draws().to_vec();
        assert_eq!(
            empty.take_action(PlayerAction::ThrowUltraBall),
            Err(pokerogue_core::MiniRunError::NoUltraBalls)
        );
        assert_eq!(empty.battle.rng_draws(), draws);

        let mut capped = MiniRun::new("UltraBallCapSeed", "DAY").unwrap();
        capped.stage = MiniRunStage::Reward;
        capped.ultraballs = 97;
        capped
            .apply_modifier_reward(Reward::UltraBalls, 0, None)
            .unwrap();
        assert_eq!(capped.ultraballs, 99);
    }

    #[test]
    fn ultra_pp_max_offer_matches_executed_source_and_persists() {
        use pokerogue_core::{MiniRun, MiniRunStage, rewards::Reward};

        let row = include_str!("../fixtures/great-recovery-offer.csv")
            .lines()
            .find(|line| line.starts_with("UltraPpMaxSeed"))
            .unwrap();
        let fields: Vec<_> = row.split(',').collect();
        let mut run = MiniRun::new_with_wave_limit(fields[0], "DAY", 20).unwrap();
        run.wave = fields[1].parse().unwrap();
        run.stage = MiniRunStage::Reward;
        run.battle.player.member_mut(0).unwrap().max_hp = 160;
        run.battle.player.member_mut(0).unwrap().hp = 18;
        run.battle.player.member_mut(1).unwrap().hp = 0;
        run.battle.player.member_mut(0).unwrap().moves[0] = pokerogue_data::Move::Tackle;
        let source_ultra: Vec<u32> = include_str!("../fixtures/ultra-rewards.csv")
            .lines()
            .find(|line| line.starts_with("ultra,mini-run-wave-19,"))
            .unwrap()
            .split(',')
            .nth(3)
            .unwrap()
            .split('|')
            .map(|weight| weight.parse().unwrap())
            .collect();
        assert_eq!(
            run.modeled_player_modifier_tier_weights(2).unwrap(),
            source_ultra
        );
        let offer = run.natural_supported_reward_offer().unwrap();
        assert_eq!(
            offer
                .options
                .iter()
                .map(|o| format!("{}:{}:{}:{}", o.id, o.upgrades, o.tier, o.pool_index))
                .collect::<Vec<_>>()
                .join("|"),
            fields[2]
        );
        let expected_draws: Vec<_> = fields[3]
            .split('|')
            .map(|draw| {
                let values: Vec<_> = draw.split(':').collect();
                (
                    values[0].parse::<u32>().unwrap(),
                    values[1].parse::<i32>().unwrap(),
                    values[2].parse::<i32>().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            offer
                .rng_draws
                .iter()
                .map(|draw| (draw.range, draw.min, draw.value))
                .collect::<Vec<_>>(),
            expected_draws
        );
        let index = offer
            .options
            .iter()
            .position(|option| option.id == "PP_MAX")
            .unwrap();
        assert_eq!(Reward::PpMax.tier(), 2);
        let battle_draw_count = run.battle.rng_draws().len();
        run.claim_natural_supported_reward_for_move(index, 0, Some(0))
            .unwrap();
        assert_eq!(run.battle.rng_draws().len(), battle_draw_count);
        let restored = MiniRun::from_checkpoint(&run.checkpoint().unwrap()).unwrap();
        assert_eq!(restored.battle.player.member(0).unwrap().move_pp_ups[0], 3);
        assert_eq!(
            restored.battle.player.member(0).unwrap().move_pp_max(0),
            Some(56)
        );
    }

    #[test]
    fn great_ball_offer_capture_and_save_match_executed_source_slices() {
        use pokerogue_core::{
            BattleRng, MiniRun, MiniRunStage, PlayerAction, Pokeball, attempt_capture,
        };

        let fields: Vec<_> = include_str!("../fixtures/great-recovery-offer.csv")
            .lines()
            .find(|line| line.starts_with("GreatBallSearch"))
            .unwrap()
            .split(',')
            .collect();
        let mut run = MiniRun::new_with_wave_limit(fields[0], "DAY", 20).unwrap();
        run.wave = fields[1].parse().unwrap();
        run.stage = MiniRunStage::Reward;
        let member = run.battle.player.member_mut(0).unwrap();
        member.max_hp = 160;
        member.hp = 18;
        run.battle.player.member_mut(1).unwrap().hp = 0;
        let offer = run.natural_supported_reward_offer().unwrap();
        let actual: Vec<_> = offer
            .options
            .iter()
            .map(|option| {
                format!(
                    "{}:{}:{}:{}",
                    option.id, option.upgrades, option.tier, option.pool_index
                )
            })
            .collect();
        assert_eq!(actual.join("|"), fields[2]);
        let expected_draws: Vec<_> = fields[3]
            .split('|')
            .map(|draw| {
                let values: Vec<_> = draw.split(':').collect();
                (
                    values[0].parse::<u32>().unwrap(),
                    values[1].parse::<i32>().unwrap(),
                    values[2].parse::<i32>().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            offer
                .rng_draws
                .iter()
                .map(|draw| (draw.range, draw.min, draw.value))
                .collect::<Vec<_>>(),
            expected_draws
        );
        let index = offer
            .options
            .iter()
            .position(|option| option.id == "GREAT_BALL")
            .unwrap_or_else(|| panic!("Great Ball missing from actual offer: {actual:?}"));
        run.claim_natural_supported_reward(index, 0).unwrap();
        assert_eq!(run.greatballs, 5);
        let resumed = MiniRun::from_checkpoint(&run.checkpoint().unwrap()).unwrap();
        assert_eq!(resumed.greatballs, 5);
        assert!(
            run.checkpoint()
                .unwrap()
                .starts_with("POKEROGUE_MINIRUN\t27\n")
        );

        let mut capture_run = MiniRun::new("GreatBallCaptureSeed", "DAY").unwrap();
        capture_run.greatballs = 1;
        let species = capture_run.battle.enemy.species;
        let hp = capture_run.battle.enemy.hp;
        let max_hp = capture_run.battle.enemy.max_hp;
        let mut oracle_rng = BattleRng::new("GreatBallCaptureSeed");
        let expected = attempt_capture(
            &mut oracle_rng,
            1,
            species.catch_rate(),
            hp,
            max_hp,
            Pokeball::Great,
            1.0,
            1.0,
            0,
        )
        .unwrap();
        capture_run
            .take_action(PlayerAction::ThrowGreatBall)
            .unwrap();
        assert_eq!(capture_run.greatballs, 0);
        assert_eq!(
            capture_run
                .battle
                .last_capture_attempt
                .as_ref()
                .unwrap()
                .modified_catch_rate,
            expected.modified_catch_rate
        );

        let mut empty_inventory = MiniRun::new("NoGreatBallSeed", "DAY").unwrap();
        let turn = empty_inventory.battle.turn;
        assert_eq!(
            empty_inventory.take_action(PlayerAction::ThrowGreatBall),
            Err(pokerogue_core::MiniRunError::NoGreatBalls)
        );
        assert_eq!(empty_inventory.battle.turn, turn);
        assert!(empty_inventory.battle.rng_draws().is_empty());

        let mut capped = MiniRun::new("GreatBallCapSeed", "DAY").unwrap();
        capped.stage = MiniRunStage::Reward;
        capped.greatballs = 97;
        capped
            .apply_modifier_reward(pokerogue_core::rewards::Reward::GreatBalls, 0, None)
            .unwrap();
        assert_eq!(capped.greatballs, 99);

        let legacy_v14 = checkpoint_as_v16(&run.checkpoint().unwrap())
            .replacen("POKEROGUE_MINIRUN\t16", "POKEROGUE_MINIRUN\t14", 1)
            .lines()
            .filter(|line| {
                !line.starts_with("greatballs\t")
                    && !line.starts_with("ultraballs\t")
                    && !line.starts_with("money\t")
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(MiniRun::from_checkpoint(&legacy_v14).unwrap().greatballs, 0);

        let inventory_rows: Vec<_> = include_str!("../fixtures/ball-inventory.csv")
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect();
        assert_eq!(
            inventory_rows,
            [
                "GREAT_BALL,0,5,5",
                "GREAT_BALL,97,5,99",
                "ULTRA_BALL,0,5,5",
                "ULTRA_BALL,97,5,99"
            ]
        );
    }

    #[test]
    fn ultra_big_nugget_offer_effect_and_persistence_match_source() {
        use pokerogue_core::{MiniRun, MiniRunStage, rewards::Reward};
        let row = include_str!("../fixtures/great-recovery-offer.csv")
            .lines()
            .find(|line| line.starts_with("UltraBigNuggetSeed"))
            .unwrap();
        let fields: Vec<_> = row.split(',').collect();
        let mut run = MiniRun::new_with_wave_limit(fields[0], "DAY", 20).unwrap();
        run.money = 0;
        run.wave = fields[1].parse().unwrap();
        run.stage = MiniRunStage::Reward;
        run.battle.player.member_mut(0).unwrap().max_hp = 160;
        run.battle.player.member_mut(0).unwrap().hp = 18;
        run.battle.player.member_mut(1).unwrap().hp = 0;
        let offer = run.natural_supported_reward_offer().unwrap();
        assert_eq!(
            offer
                .options
                .iter()
                .map(|o| format!("{}:{}:{}:{}", o.id, o.upgrades, o.tier, o.pool_index))
                .collect::<Vec<_>>()
                .join("|"),
            fields[2]
        );
        let expected: Vec<_> = fields[3]
            .split('|')
            .map(|draw| {
                let values: Vec<_> = draw.split(':').collect();
                (
                    values[0].parse::<u32>().unwrap(),
                    values[1].parse::<i32>().unwrap(),
                    values[2].parse::<i32>().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            offer
                .rng_draws
                .iter()
                .map(|draw| (draw.range, draw.min, draw.value))
                .collect::<Vec<_>>(),
            expected
        );
        let index = offer
            .options
            .iter()
            .position(|option| option.id == "BIG_NUGGET")
            .unwrap();
        assert_eq!(Reward::BigNugget.tier(), 2);
        run.claim_natural_supported_reward(index, 0).unwrap();
        let source_amount: u64 = include_str!("../fixtures/money-reward.csv")
            .lines()
            .find(|line| line.starts_with("big-nugget-wave-19,"))
            .unwrap()
            .split(',')
            .nth(4)
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(run.money, source_amount);
        let saved = run.checkpoint().unwrap();
        assert_eq!(
            MiniRun::from_checkpoint(&saved).unwrap().money,
            source_amount
        );
        let mut capped = MiniRun::new("BigNuggetCap", "DAY").unwrap();
        capped.stage = MiniRunStage::Reward;
        capped.wave = 1;
        capped.money = pokerogue_core::MAX_RUN_MONEY - 5;
        capped
            .apply_modifier_reward(Reward::BigNugget, 0, None)
            .unwrap();
        assert_eq!(capped.money, pokerogue_core::MAX_RUN_MONEY);
    }

    #[test]
    fn natural_nugget_offer_and_wallet_match_executed_source() {
        use pokerogue_core::{MAX_RUN_MONEY, MiniRun, MiniRunStage, classic_wave_money_amount};

        for row in include_str!("../fixtures/money-reward.csv")
            .lines()
            .filter(|line| !line.starts_with('#') && !line.starts_with("money_modifier,"))
        {
            let fields: Vec<_> = row.split(',').collect();
            let wave: u32 = fields[1].parse().unwrap();
            assert_eq!(
                classic_wave_money_amount(wave, fields[2].parse().unwrap()),
                fields[4].parse::<u64>().unwrap(),
                "money formula: {row}"
            );
        }

        for row in include_str!("../fixtures/money-reward.csv")
            .lines()
            .filter(|line| line.starts_with("money_modifier,"))
        {
            let fields: Vec<_> = row.split(',').collect();
            assert_eq!(
                pokerogue_core::apply_amulet_coin(
                    fields[2].parse().unwrap(),
                    fields[1].parse().unwrap()
                ),
                fields[3].parse::<u64>().unwrap(),
                "Amulet Coin apply: {row}"
            );
        }

        let fields: Vec<_> = include_str!("../fixtures/great-recovery-offer.csv")
            .lines()
            .find(|line| line.starts_with("GreatNuggetSeed"))
            .unwrap()
            .split(',')
            .collect();
        let mut run = MiniRun::new_with_wave_limit(fields[0], "DAY", 20).unwrap();
        run.money = 0;
        run.wave = fields[1].parse().unwrap();
        run.stage = MiniRunStage::Reward;
        run.battle.player.member_mut(0).unwrap().max_hp = 160;
        run.battle.player.member_mut(0).unwrap().hp = 18;
        run.battle.player.member_mut(1).unwrap().max_hp = 22;
        run.battle.player.member_mut(1).unwrap().hp = 0;
        let offer = run.natural_supported_reward_offer().unwrap();
        let options: Vec<_> = offer
            .options
            .iter()
            .map(|option| {
                format!(
                    "{}:{}:{}:{}",
                    option.id, option.upgrades, option.tier, option.pool_index
                )
            })
            .collect();
        assert_eq!(options.join("|"), fields[2]);
        assert_eq!(
            offer
                .rng_draws
                .iter()
                .map(|draw| (draw.range, draw.min, draw.value))
                .collect::<Vec<_>>(),
            fields[3]
                .split('|')
                .map(|draw| {
                    let values: Vec<_> = draw.split(':').collect();
                    (
                        values[0].parse().unwrap(),
                        values[1].parse().unwrap(),
                        values[2].parse().unwrap(),
                    )
                })
                .collect::<Vec<_>>()
        );
        let nugget = offer
            .options
            .iter()
            .position(|option| option.id == "NUGGET")
            .unwrap();
        run.claim_natural_supported_reward(nugget, 0).unwrap();
        assert_eq!(run.money, 370);
        let save = run.checkpoint().unwrap();
        assert!(save.starts_with("POKEROGUE_MINIRUN\t27\n"));
        assert_eq!(MiniRun::from_checkpoint(&save).unwrap().money, 370);

        let v15 = checkpoint_as_v16(&save)
            .replacen("POKEROGUE_MINIRUN\t16", "POKEROGUE_MINIRUN\t15", 1)
            .replace("money\t370\n", "");
        assert_eq!(MiniRun::from_checkpoint(&v15).unwrap().money, 0);
        let mut capped = MiniRun::new("NuggetWalletCap", "DAY").unwrap();
        capped.stage = MiniRunStage::Reward;
        capped.wave = 1;
        capped.money = MAX_RUN_MONEY - 5;
        capped
            .apply_modifier_reward(pokerogue_core::rewards::Reward::Nugget, 0, None)
            .unwrap();
        assert_eq!(capped.money, MAX_RUN_MONEY);
    }

    #[test]
    fn great_elixir_offer_restores_and_persists_move_pp_against_source_fixture() {
        use pokerogue_core::{MiniRun, MiniRunStage, rewards::Reward};

        let fields: Vec<_> = include_str!("../fixtures/great-recovery-offer.csv")
            .lines()
            .find(|line| line.starts_with("GreatElixirSeed"))
            .unwrap()
            .split(',')
            .collect();
        let mut run = MiniRun::new_with_wave_limit(fields[0], "DAY", 20).unwrap();
        run.wave = fields[1].parse().unwrap();
        run.stage = MiniRunStage::Reward;
        let member = run.battle.player.member_mut(0).unwrap();
        member.max_hp = 160;
        member.hp = 18;
        member.move_pp_used = [20, 30, 0, 0];
        run.battle.player.member_mut(1).unwrap().hp = 0;

        let expected_weights: Vec<u32> = include_str!("../fixtures/great-rewards.csv")
            .lines()
            .find(|line| line.starts_with("great,mini-run-great-elixir-wave-19,"))
            .unwrap()
            .split(',')
            .nth(3)
            .unwrap()
            .split('|')
            .map(|value| value.parse().unwrap())
            .collect();
        assert_eq!(
            run.modeled_player_modifier_tier_weights(1).unwrap(),
            expected_weights
        );

        let offer = run.natural_supported_reward_offer().unwrap();
        let actual: Vec<_> = offer
            .options
            .iter()
            .map(|option| {
                format!(
                    "{}:{}:{}:{}",
                    option.id, option.upgrades, option.tier, option.pool_index
                )
            })
            .collect();
        assert_eq!(actual.join("|"), fields[2]);
        let expected_draws: Vec<_> = fields[3]
            .split('|')
            .map(|draw| {
                let parts: Vec<_> = draw.split(':').collect();
                (
                    parts[0].parse::<u32>().unwrap(),
                    parts[1].parse::<i32>().unwrap(),
                    parts[2].parse::<i32>().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            offer
                .rng_draws
                .iter()
                .map(|draw| (draw.range, draw.min, draw.value))
                .collect::<Vec<_>>(),
            expected_draws
        );
        let index = offer
            .options
            .iter()
            .position(|option| option.id == "ELIXIR")
            .unwrap();
        run.claim_natural_supported_reward(index, 0).unwrap();
        assert_eq!(
            run.battle.player.member(0).unwrap().move_pp_used,
            [10, 20, 0, 0]
        );
        let resumed = MiniRun::from_checkpoint(&run.checkpoint().unwrap()).unwrap();
        assert_eq!(
            resumed.battle.player.member(0).unwrap().move_pp_used,
            [10, 20, 0, 0]
        );

        let source_restore: Vec<_> = include_str!("../fixtures/pp-restore.csv")
            .lines()
            .find(|line| line.starts_with("all,30|25,-1,"))
            .unwrap()
            .split(',')
            .nth(3)
            .unwrap()
            .split('|')
            .map(|value| value.parse::<u8>().unwrap())
            .collect();
        let mut max_elixir_run = MiniRun::new("MaxElixirSaveSeed", "DAY").unwrap();
        max_elixir_run.stage = MiniRunStage::Reward;
        max_elixir_run
            .battle
            .player
            .member_mut(0)
            .unwrap()
            .move_pp_used = [30, 25, 0, 0];
        max_elixir_run
            .apply_modifier_reward(Reward::MaxElixir, 0, None)
            .unwrap();
        assert_eq!(
            &max_elixir_run.battle.player.member(0).unwrap().move_pp_used[..source_restore.len()],
            source_restore.as_slice()
        );
    }

    #[test]
    fn natural_common_lure_choice_applies_and_persists_without_battle_rng() {
        let fields: Vec<_> = include_str!("../fixtures/natural-reward-candidate.csv")
            .lines()
            .find(|line| line.starts_with("offer,"))
            .unwrap()
            .split(',')
            .collect();
        let mut run = pokerogue_core::MiniRun::new(fields[2], "DAY").unwrap();
        run.wave = fields[1].parse().unwrap();
        run.stage = pokerogue_core::MiniRunStage::Reward;
        let battle_draw_count = run.battle.rng_draws().len();

        run.claim_natural_common_reward(0, 0).unwrap();
        assert!(run.reward_claimed);
        assert_eq!(run.lure_battles_remaining, 10);
        assert_eq!(run.battle.rng_draws().len(), battle_draw_count);
        let resumed = pokerogue_core::MiniRun::from_checkpoint(&run.checkpoint().unwrap()).unwrap();
        assert_eq!(resumed.lure_battles_remaining, 10);
        assert!(resumed.reward_claimed);
    }

    #[test]
    fn three_natural_reward_options_match_executed_source_retry_flow() {
        let row = include_str!("../fixtures/natural-reward-candidate.csv")
            .lines()
            .find(|line| line.starts_with("offer,"))
            .unwrap();
        let fields: Vec<_> = row.split(',').collect();
        let expected_options: Vec<_> = fields[3].split('|').collect();
        let offer = pokerogue_core::rewards::roll_natural_player_offer(
            fields[2],
            fields[1].parse().unwrap(),
            0,
            true,
            3,
            15,
            |tier| {
                if tier == 0 {
                    vec![6, 2, 0, 0, 0, 0, 2, 4, 2, 2]
                } else {
                    vec![1]
                }
            },
            |id, rng| {
                pokerogue_core::rewards::resolve_common_player_modifier(
                    id,
                    rng,
                    0,
                    &["TACKLE", "GROWL"],
                )
            },
        )
        .unwrap();
        assert_eq!(offer.options.len(), 3);
        for (option, expected) in offer.options.iter().zip(expected_options) {
            let fields: Vec<_> = expected.split(':').collect();
            assert_eq!(option.id, fields[0]);
            assert_eq!(option.upgrades, fields[1].parse::<u32>().unwrap());
            assert_eq!(option.tier, fields[2].parse::<u8>().unwrap());
            assert_eq!(option.pool_index, fields[3].parse::<usize>().unwrap());
        }
        let expected_draws: Vec<_> = fields[4]
            .split('|')
            .map(|draw| {
                let parts: Vec<_> = draw.split(':').collect();
                (
                    parts[0].parse::<u32>().unwrap(),
                    parts[1].parse::<i32>().unwrap(),
                    parts[2].parse::<i32>().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            offer
                .rng_draws
                .iter()
                .map(|draw| (draw.range, draw.min, draw.value))
                .collect::<Vec<_>>(),
            expected_draws
        );
    }

    #[test]
    fn common_modifier_identity_preserves_source_groups_without_static_rng() {
        use pokerogue_core::{BattleRng, rewards::resolve_common_player_modifier};
        let mut rng = BattleRng::new("common-groups");
        let expected = [
            ("POKEBALL", Some("pb")),
            ("RARE_CANDY", None),
            ("POTION", Some("potion")),
            ("SUPER_POTION", Some("potion")),
            ("ETHER", Some("ether")),
            ("MAX_ETHER", Some("ether")),
            ("LURE", Some("lure")),
        ];
        for (id, group) in expected {
            let identity = resolve_common_player_modifier(id, &mut rng, 0, &[]).unwrap();
            assert_eq!(identity.name, id);
            assert_eq!(identity.group.as_deref(), group);
            assert_eq!(identity.generated_variant, None);
        }
        assert!(rng.draws().is_empty());
        assert!(resolve_common_player_modifier("GREAT_BALL", &mut rng, 0, &[]).is_none());
    }

    #[test]
    fn generated_common_reward_callbacks_match_executed_typescript() {
        let stats = ["ATK", "DEF", "SPATK", "SPDEF", "SPD", "ACC"];
        let berries = [
            "SITRUS", "LUM", "ENIGMA", "LIECHI", "GANLON", "PETAYA", "APICOT", "SALAC", "LANSAT",
            "STARF", "LEPPA",
        ];
        for row in include_str!("../fixtures/generated-common-rewards.csv")
            .lines()
            .filter(|line| !line.starts_with('#'))
        {
            let fields: Vec<_> = row.split(',').collect();
            let mut rng = pokerogue_core::BattleRng::new(fields[1]);
            let compatible_tms = ["TACKLE", "GROWL"];
            if fields[0] == "TM_COMMON_EMPTY" {
                assert!(
                    pokerogue_core::rewards::resolve_generated_modifier(
                        "TM_COMMON",
                        &mut rng,
                        0,
                        &[],
                    )
                    .is_none()
                );
                assert!(rng.draws().is_empty());
                continue;
            }
            let identity = pokerogue_core::rewards::resolve_generated_modifier(
                fields[0],
                &mut rng,
                0,
                &compatible_tms,
            )
            .unwrap();
            let source_variant = match fields[0] {
                "TEMP_STAT_STAGE_BOOSTER" => stats[fields[2].parse::<usize>().unwrap() - 1],
                "BERRY" => berries[fields[2].parse::<usize>().unwrap()],
                "TM_COMMON" => fields[2],
                _ => unreachable!(),
            };
            assert_eq!(
                identity.generated_variant.as_deref(),
                Some(source_variant),
                "{row}"
            );
            let expected_draws: Vec<_> = fields[3]
                .split('|')
                .map(|draw| {
                    let parts: Vec<_> = draw.split(':').collect();
                    (
                        parts[0].parse::<u32>().unwrap(),
                        parts[1].parse::<i32>().unwrap(),
                        parts[2].parse::<i32>().unwrap(),
                    )
                })
                .collect();
            assert_eq!(
                rng.draws()
                    .iter()
                    .map(|draw| (draw.range, draw.min, draw.value))
                    .collect::<Vec<_>>(),
                expected_draws,
                "{row}"
            );
        }
    }

    #[test]
    fn null_generated_reward_retries_same_tier_like_executed_typescript() {
        let row = include_str!("../fixtures/null-reward-retry.csv")
            .lines()
            .find(|line| line.starts_with("null-retry,"))
            .unwrap();
        let fields: Vec<_> = row.split(',').collect();
        let expected_draws: Vec<_> = fields[2]
            .split('|')
            .map(|draw| {
                let parts: Vec<_> = draw.split(':').collect();
                (
                    parts[0].parse::<u32>().unwrap(),
                    parts[1].parse::<i32>().unwrap(),
                    parts[2].parse::<i32>().unwrap(),
                )
            })
            .collect();
        let offer = pokerogue_core::rewards::roll_natural_player_offer(
            fields[1],
            0,
            0,
            false,
            1,
            15,
            |_| {
                let mut weights = vec![0; 10];
                weights[0] = 1;
                weights[9] = 1;
                weights
            },
            |id, rng| {
                if id == "TM_COMMON" {
                    return pokerogue_core::rewards::resolve_generated_modifier(id, rng, 0, &[]);
                }
                Some(pokerogue_core::rewards::NaturalRewardIdentity {
                    name: "Poké Ball".into(),
                    group: Some("pb".into()),
                    generated_variant: None,
                })
            },
        )
        .unwrap();
        assert_eq!(offer.options[0].id, fields[3]);
        assert_eq!(offer.options[0].tier, 0);
        assert_eq!(offer.options[0].pool_index, 0);
        assert_eq!(
            offer
                .rng_draws
                .iter()
                .map(|draw| (draw.range, draw.min, draw.value))
                .collect::<Vec<_>>(),
            expected_draws,
        );
    }

    #[test]
    fn one_hundred_null_generated_rewards_step_down_tier_like_typescript() {
        let row = include_str!("../fixtures/null-reward-retry.csv")
            .lines()
            .find(|line| line.starts_with("stepdown,"))
            .unwrap();
        let fields: Vec<_> = row.split(',').collect();
        let expected_draws: Vec<_> = fields[2]
            .split('|')
            .map(|draw| {
                let parts: Vec<_> = draw.split(':').collect();
                (
                    parts[0].parse::<u32>().unwrap(),
                    parts[1].parse::<i32>().unwrap(),
                    parts[2].parse::<i32>().unwrap(),
                )
            })
            .collect();
        let mut null_candidates = 0;
        let offer = pokerogue_core::rewards::roll_natural_player_offer(
            fields[1],
            0,
            0,
            false,
            1,
            15,
            |tier| {
                let mut weights = if tier == 1 { vec![0; 19] } else { vec![0; 10] };
                if tier == 1 {
                    weights[18] = 1;
                } else if tier == 0 {
                    weights[0] = 1;
                }
                weights
            },
            |id, rng| {
                if id == "TM_GREAT" {
                    null_candidates += 1;
                    return pokerogue_core::rewards::resolve_generated_modifier(id, rng, 0, &[]);
                }
                Some(pokerogue_core::rewards::NaturalRewardIdentity {
                    name: "Poké Ball".into(),
                    group: Some("pb".into()),
                    generated_variant: None,
                })
            },
        )
        .unwrap();
        assert_eq!(null_candidates, fields[4].parse::<usize>().unwrap());
        assert_eq!(offer.options[0].id, fields[3]);
        assert_eq!(offer.options[0].tier, 0);
        assert_eq!(offer.options[0].pool_index, 0);
        assert_eq!(
            offer
                .rng_draws
                .iter()
                .map(|draw| (draw.range, draw.min, draw.value))
                .collect::<Vec<_>>(),
            expected_draws,
        );
    }

    #[test]
    fn reward_wave_seed_matches_original_utf16_shift() {
        for row in include_str!("../fixtures/reward-seed.csv")
            .lines()
            .filter(|l| !l.starts_with('#'))
        {
            let f: Vec<_> = row.split(',').collect();
            let expected: Vec<u16> = f[3]
                .split('|')
                .map(|v| u16::from_str_radix(v, 16).unwrap())
                .collect();
            assert_eq!(
                pokerogue_core::rewards::shifted_reward_seed_units(f[1], f[2].parse().unwrap()),
                expected
            );
        }
    }

    #[test]
    fn reward_retries_and_stack_caps_match_executed_original_methods() {
        use pokerogue_core::rewards::{
            RewardCandidate, reward_candidate_with_retry, stack_filtered_weight,
        };
        fn decode(v: &str) -> RewardCandidate {
            let f: Vec<_> = v.split(':').collect();
            RewardCandidate {
                name: f[0].into(),
                group: if f[1] == "-" { None } else { Some(f[1].into()) },
                tier: f[2].parse().unwrap(),
                upgrades: f[3].parse().unwrap(),
            }
        }
        for row in include_str!("../fixtures/reward-retries.csv")
            .lines()
            .filter(|l| !l.starts_with('#'))
        {
            let f: Vec<_> = row.split(',').collect();
            if f[0] == "cap" {
                let stacks: Vec<(u32, u32)> = if f[1] == "-" {
                    vec![]
                } else {
                    f[1].split('|')
                        .map(|v| {
                            let (a, b) = v.split_once(':').unwrap();
                            (a.parse().unwrap(), b.parse().unwrap())
                        })
                        .collect()
                };
                assert_eq!(
                    stack_filtered_weight(6, &stacks, f[2] == "1", f[3] == "1"),
                    f[4].parse::<u32>().unwrap(),
                    "{row}"
                );
                continue;
            }
            let existing: Vec<_> = if f[3] == "-" {
                vec![]
            } else {
                f[3].split('|').map(decode).collect()
            };
            let candidates: Vec<_> = f[4].split('|').map(decode).collect();
            let mut trace = Vec::new();
            let mut cursor = 0;
            let actual =
                reward_candidate_with_retry(&existing, f[2].parse().unwrap(), |tier, upgrades| {
                    trace.push(format!(
                        "{}:{}:1",
                        tier.map_or("-".into(), |v| v.to_string()),
                        upgrades.map_or("-".into(), |v| v.to_string())
                    ));
                    let result = candidates[cursor.min(candidates.len() - 1)].clone();
                    cursor += 1;
                    Ok::<_, ()>(result)
                })
                .unwrap();
            assert_eq!(actual, decode(f[5]), "{row}");
            assert_eq!(trace.join("|"), f[6], "{row}");
        }
        let unsupported =
            reward_candidate_with_retry::<&str>(&[], 15, |_, _| Err("unsupported original item"));
        assert_eq!(unsupported, Err("unsupported original item"));
    }

    #[test]
    fn common_reward_weights_match_complete_executed_source_callbacks() {
        use pokerogue_core::rewards::{COMMON_POOL_IDS, RewardPartyMember, common_pool_weights};
        for row in include_str!("../fixtures/common-rewards.csv")
            .lines()
            .filter(|l| !l.starts_with('#'))
        {
            let f: Vec<_> = row.split(',').collect();
            if f[0] == "ids" {
                assert_eq!(COMMON_POOL_IDS.join("|"), f[1]);
                continue;
            }
            let lures: Vec<(u32, u32)> = if f[4] == "-" {
                Vec::new()
            } else {
                f[4].split('|')
                    .map(|v| {
                        let (a, b) = v.split_once(':').unwrap();
                        (a.parse().unwrap(), b.parse().unwrap())
                    })
                    .collect()
            };
            let party: Vec<_> = f[5]
                .split('|')
                .map(|v| {
                    let n: Vec<i32> = v.split(':').map(|n| n.parse().unwrap()).collect();
                    RewardPartyMember {
                        hp: n[0],
                        max_hp: n[1],
                        holds_leppa: n[2] != 0,
                        move_pp: vec![(n[3] as u32, n[4] as u32)],
                    }
                })
                .collect();
            let actual = common_pool_weights(
                &party,
                f[1] == "1",
                f[2].parse().unwrap(),
                f[3].parse().unwrap(),
                &lures,
            );
            let expected: Vec<u32> = f[6].split('|').map(|v| v.parse().unwrap()).collect();
            assert_eq!(actual.as_slice(), expected.as_slice(), "{row}");
        }
    }

    #[test]
    fn mini_run_common_weights_use_live_party_pp_and_ball_state() {
        let source_row = include_str!("../fixtures/common-rewards.csv")
            .lines()
            .find(|line| {
                line.starts_with("common,1,1,5,-,")
                    && line.contains("100:100:0:35:30|100:100:0:35:0,")
            })
            .expect("source-executed two-member PP fixture");
        let expected: Vec<u32> = source_row
            .split(',')
            .nth(6)
            .unwrap()
            .split('|')
            .map(|weight| weight.parse().unwrap())
            .collect();
        let mut run = pokerogue_core::MiniRun::new("CommonContextFixture", "DAY").unwrap();
        let member = run
            .battle
            .player
            .member_mut(0)
            .expect("initial player exists");
        let pp_slot = member
            .moves
            .iter()
            .position(|movement| movement.pp() == 35)
            .expect("imported starter has a 35 PP move");
        member.move_pp_used[pp_slot] = 30;
        assert_eq!(run.common_reward_pool_weights().as_slice(), expected);
    }

    #[test]
    fn mini_run_reward_weight_shapes_preserve_source_pool_indexes() {
        let run = pokerogue_core::MiniRun::new("RewardPoolShape", "DAY").unwrap();
        for (tier, expected_len) in [(0, 10), (1, 24), (2, 26), (3, 19), (4, 7)] {
            let weights = run
                .modeled_player_modifier_tier_weights(tier)
                .expect("supported PLAYER pool tier");
            assert_eq!(weights.len(), expected_len, "tier {tier}");
        }

        // Orb eligibility depends on species abilities, status immunity, held
        // items and moves not modeled in this starter-party state. Unknown
        // eligibility must not be inferred from PP/moveset shape.
        let ultra = run.modeled_player_modifier_tier_weights(2).unwrap();
        for id in ["TOXIC_ORB", "FLAME_ORB"] {
            let entry = pokerogue_data::IMPORTED_PLAYER_MODIFIER_POOL
                .iter()
                .find(|entry| entry.id == id)
                .unwrap();
            assert_eq!(ultra[entry.pool_index as usize], 0, "{id}");
        }
    }

    #[test]
    fn natural_reward_tiers_match_executed_source_and_exact_roll_order() {
        use pokerogue_core::rewards::roll_player_reward_tier;
        for row in include_str!("../fixtures/reward-tiers.csv")
            .lines()
            .filter(|l| !l.starts_with('#'))
        {
            let f: Vec<_> = row.split(',').collect();
            if f[0] == "weight" {
                let weights: Vec<u32> = f[1].split('|').map(|v| v.parse().unwrap()).collect();
                let mut calls = 0;
                let index =
                    pokerogue_core::rewards::roll_weighted_reward_index(&weights, |range| {
                        calls += 1;
                        assert_eq!(range, f[2].parse::<u32>().unwrap());
                        f[3].parse().unwrap()
                    });
                assert_eq!(calls, 1);
                assert_eq!(index, Some(f[4].parse().unwrap()));
                continue;
            }
            let draws: Vec<(u32, u32)> = f[4]
                .split('|')
                .map(|d| {
                    let (range, value) = d.split_once(':').unwrap();
                    (range.parse().unwrap(), value.parse().unwrap())
                })
                .collect();
            let mut cursor = 0;
            let mask: Vec<_> = f[3].chars().map(|v| v == '1').collect();
            let result = roll_player_reward_tier(
                f[1].parse().unwrap(),
                f[2] == "1",
                mask.try_into().unwrap(),
                |range| {
                    let expected = draws[cursor];
                    cursor += 1;
                    assert_eq!(range, expected.0, "{row}");
                    expected.1
                },
            );
            assert_eq!(cursor, draws.len(), "{row}");
            assert_eq!(result.tier, f[5].parse::<u8>().unwrap(), "{row}");
            assert_eq!(result.upgrades, f[6].parse::<u32>().unwrap(), "{row}");
        }
    }

    #[test]
    fn guaranteed_rewards_and_consumables_match_executed_source() {
        use pokerogue_core::{
            MiniRun, MiniRunStage,
            rewards::{GUARANTEED_REWARDS, Reward},
        };
        let mut options = Vec::new();
        for row in include_str!("../fixtures/rewards.csv")
            .lines()
            .filter(|l| !l.starts_with('#'))
        {
            let f: Vec<_> = row.split(',').collect();
            match f[0] {
                "option" => options.push((f[1], f[2].parse::<u8>().unwrap())),
                "heal" => {
                    let item = match f[1] {
                        "POTION" => Reward::Potion,
                        "SUPER_POTION" => Reward::SuperPotion,
                        "HYPER_POTION" => Reward::HyperPotion,
                        "MAX_POTION" => Reward::MaxPotion,
                        "FULL_RESTORE" => Reward::FullRestore,
                        _ => panic!("unknown healing item {}", f[1]),
                    };
                    let hp = f[2].parse().unwrap();
                    let actual = item.heal(hp, f[3].parse().unwrap());
                    assert_eq!(actual.is_some(), f[5] == "1");
                    assert_eq!(actual.unwrap_or(hp), f[4].parse::<i32>().unwrap());
                }
                "revive" => {
                    let item = if f[1] == "REVIVE" {
                        Reward::Revive
                    } else {
                        Reward::MaxRevive
                    };
                    let hp = f[2].parse().unwrap();
                    let actual = item.revive(hp, f[3].parse().unwrap());
                    assert_eq!(actual.is_some(), f[5] == "1");
                    assert_eq!(actual.unwrap_or(hp), f[4].parse::<i32>().unwrap());
                }
                "balls" => {
                    let mut run = MiniRun::new("RewardFixture", "DAY").unwrap();
                    run.stage = MiniRunStage::Reward;
                    run.pokeballs = f[1].parse().unwrap();
                    run.apply_modifier_reward(pokerogue_core::rewards::Reward::Pokeballs, 0, None)
                        .unwrap();
                    assert_eq!(run.pokeballs, f[2].parse::<u8>().unwrap());
                }
                _ => panic!("unknown fixture row"),
            }
        }
        assert_eq!(
            options,
            GUARANTEED_REWARDS
                .iter()
                .map(|r| (r.id(), r.tier()))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn reward_claim_is_atomic_and_persists_before_and_after_application() {
        use pokerogue_core::{MiniRun, MiniRunError, MiniRunStage};
        let mut run = MiniRun::new("RewardSave", "DAY").unwrap();
        assert_eq!(run.claim_reward(0, 0), Err(MiniRunError::NotAtReward));
        run.stage = MiniRunStage::Reward;
        run.battle.player.hp = 1;
        run.battle.player.move_pp_used = [3, 2, 0, 0];
        run.battle.player.set_stat_stage(BattleStat::Defense, 2);
        run.exp_charm_stacks = 2;
        run.super_exp_charm_stacks = 1;
        let save = run.checkpoint().unwrap();
        let mut restored = MiniRun::from_checkpoint(&save).unwrap();
        assert_eq!(restored.battle.player.move_pp_used, [3, 2, 0, 0]);
        assert_eq!(restored.battle.player.stat_stage(BattleStat::Defense), 2);
        assert_eq!(restored.exp_charm_stacks, 2);
        assert_eq!(restored.super_exp_charm_stacks, 1);
        assert_eq!(restored.reward_options().len(), 3);
        assert_eq!(
            restored.claim_reward(1, 9),
            Err(MiniRunError::InvalidRewardTarget)
        );
        assert!(!restored.reward_claimed);
        let draws = restored.battle.rng_draws().len();
        restored.claim_reward(1, 0).unwrap();
        assert_eq!(
            restored.battle.player.hp,
            21.min(restored.battle.player.max_hp)
        );
        assert_eq!(restored.battle.rng_draws().len(), draws);
        let mut claimed = MiniRun::from_checkpoint(&restored.checkpoint().unwrap()).unwrap();
        assert_eq!(claimed.battle.player.hp, restored.battle.player.hp);
        assert!(claimed.reward_options().is_empty());
        assert_eq!(
            claimed.claim_reward(0, 0),
            Err(MiniRunError::RewardAlreadyClaimed)
        );
        claimed.continue_after_reward().unwrap();
        assert!(!claimed.reward_claimed);
        assert_eq!(claimed.battle.player.move_pp_used, [3, 2, 0, 0]);
        let legacy_v5 = checkpoint_as_v16(&save)
            .lines()
            .map(|line| {
                if let Some(record) = line.strip_prefix("pokemon\t") {
                    let fields: Vec<_> = record.split(',').collect();
                    format!(
                        "pokemon\t{},{},{},{},{},{},{},{},{}",
                        fields[0],
                        fields[1],
                        fields[2],
                        fields[3],
                        fields[4],
                        fields[5],
                        fields[8],
                        fields[9],
                        fields[10]
                    )
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            .replace("wave_limit\t10\n", "")
            .replacen("MINIRUN\t16", "MINIRUN\t5", 1)
            .replace("greatballs\t0\n", "")
            .replace("ultraballs\t0\n", "")
            .replace("exp_share_stacks\t0\n", "")
            .replace("money\t1000\n", "")
            .replace("lure_battles_remaining\t0\n", "")
            .replace("accuracy_booster_battles_remaining\t0\n", "")
            .replace("speed_booster_battles_remaining\t0\n", "")
            .replace("temp_stat_booster_battles_remaining\t0,0,0,0,0,0\n", "")
            .replace("super_exp_charm_stacks\t0\n", "")
            .replace("super_exp_charm_stacks\t1\n", "");
        let migrated_v5 = MiniRun::from_checkpoint(&legacy_v5).unwrap();
        assert_eq!(migrated_v5.battle.player.move_pp_used, [0, 0, 0, 0]);
        let legacy_v4 = legacy_v5
            .replace("MINIRUN\t5", "MINIRUN\t4")
            .replace("reward_claimed\t0\n", "");
        let migrated_v4 = MiniRun::from_checkpoint(&legacy_v4).unwrap();
        assert!(migrated_v4.reward_claimed);
        assert_eq!(migrated_v4.battle.player.move_pp_used, [0, 0, 0, 0]);

        let legacy_v6 = checkpoint_as_v16(&save)
            .lines()
            .map(|line| {
                if let Some(record) = line.strip_prefix("pokemon\t") {
                    let fields: Vec<_> = record.split(',').collect();
                    format!("pokemon\t{}", fields[..11].join(","))
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            .replace("wave_limit\t10\n", "")
            .replacen("MINIRUN\t16", "MINIRUN\t6", 1)
            .replace("greatballs\t0\n", "")
            .replace("ultraballs\t0\n", "")
            .replace("exp_share_stacks\t0\n", "")
            .replace("money\t1000\n", "")
            .replace("lure_battles_remaining\t0\n", "")
            .replace("accuracy_booster_battles_remaining\t0\n", "")
            .replace("speed_booster_battles_remaining\t0\n", "")
            .replace("temp_stat_booster_battles_remaining\t0,0,0,0,0,0\n", "")
            .replace("super_exp_charm_stacks\t0\n", "")
            .replace("super_exp_charm_stacks\t1\n", "");
        let migrated_v6 = MiniRun::from_checkpoint(&legacy_v6).unwrap();
        assert_eq!(migrated_v6.battle.player.move_pp_used, [3, 2, 0, 0]);
        assert_eq!(migrated_v6.battle.player.stat_stage(BattleStat::Defense), 0);
        let legacy_v7 = checkpoint_as_v16(&save)
            .lines()
            .map(|line| {
                if let Some(record) = line.strip_prefix("pokemon\t") {
                    format!(
                        "pokemon\t{}",
                        record.split(',').take(12).collect::<Vec<_>>().join(",")
                    )
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            .replace("wave_limit\t10\n", "")
            .replacen("MINIRUN\t16", "MINIRUN\t7", 1)
            .replace("greatballs\t0\n", "")
            .replace("ultraballs\t0\n", "")
            .replace("exp_share_stacks\t0\n", "")
            .replace("money\t1000\n", "")
            .replace("lure_battles_remaining\t0\n", "")
            .replace("accuracy_booster_battles_remaining\t0\n", "")
            .replace("speed_booster_battles_remaining\t0\n", "")
            .replace("temp_stat_booster_battles_remaining\t0,0,0,0,0,0\n", "")
            .replace("super_exp_charm_stacks\t0\n", "")
            .replace("super_exp_charm_stacks\t1\n", "");
        let migrated_v7 = MiniRun::from_checkpoint(&legacy_v7).unwrap();
        assert_eq!(migrated_v7.super_exp_charm_stacks, 0);
        let legacy_v8 = checkpoint_as_v16(&save)
            .lines()
            .map(|line| {
                if let Some(record) = line.strip_prefix("pokemon\t") {
                    format!(
                        "pokemon\t{}",
                        record.split(',').take(12).collect::<Vec<_>>().join(",")
                    )
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            .replace("wave_limit\t10\n", "")
            .replacen("MINIRUN\t16", "MINIRUN\t8", 1)
            .replace("greatballs\t0\n", "")
            .replace("ultraballs\t0\n", "")
            .replace("exp_share_stacks\t0\n", "")
            .replace("money\t1000\n", "")
            .replace("lure_battles_remaining\t0\n", "")
            .replace("accuracy_booster_battles_remaining\t0\n", "")
            .replace("speed_booster_battles_remaining\t0\n", "")
            .replace("temp_stat_booster_battles_remaining\t0,0,0,0,0,0\n", "");
        let migrated_v8 = MiniRun::from_checkpoint(&legacy_v8).unwrap();
        assert_eq!(migrated_v8.lure_battles_remaining, 0);
    }

    #[test]
    fn lure_claim_refreshes_duration_and_survives_checkpoint() {
        use pokerogue_core::{MiniRun, MiniRunStage, rewards::Reward};
        let mut run = MiniRun::new("LureRewardSeed", "DAY").unwrap();
        run.stage = MiniRunStage::Reward;
        run.lure_battles_remaining = 4;
        run.apply_modifier_reward(Reward::Lure, 0, None).unwrap();
        assert_eq!(run.lure_battles_remaining, 10);
        assert_eq!(run.common_reward_pool_weights()[6], 0);

        let restored = MiniRun::from_checkpoint(&run.checkpoint().unwrap()).unwrap();
        assert_eq!(restored.lure_battles_remaining, 10);
        assert_eq!(restored.common_reward_pool_weights()[6], 0);
        let invalid_lure = run
            .checkpoint()
            .unwrap()
            .replace("lure_battles_remaining\t10", "lure_battles_remaining\t11");
        assert!(matches!(
            MiniRun::from_checkpoint(&invalid_lure),
            Err(pokerogue_core::MiniRunError::InvalidCheckpoint)
        ));
    }

    #[test]
    fn x_accuracy_reward_applies_caps_lapses_and_survives_save_v11() {
        use pokerogue_core::{
            MiniRun, MiniRunStage, apply_accuracy_stage_booster, rewards::Reward,
        };

        assert_eq!(apply_accuracy_stage_booster(0, true), 1);
        assert_eq!(apply_accuracy_stage_booster(5, true), 6);
        assert_eq!(apply_accuracy_stage_booster(6, true), 6);
        assert_eq!(apply_accuracy_stage_booster(-6, false), -6);
        assert_eq!(pokerogue_core::rewards::lapse_temp_stat_booster(5), Some(4));
        assert_eq!(pokerogue_core::rewards::lapse_temp_stat_booster(1), None);
        assert!((pokerogue_core::boosted_stage_multiplier(0, 0.2) - 1.2).abs() < 1e-9);
        assert_eq!(pokerogue_core::boosted_stage_multiplier(6, 0.2), 4.0);

        let mut run = MiniRun::new("AccuracyBoosterSaveSeed", "DAY").unwrap();
        run.stage = MiniRunStage::Reward;
        run.apply_modifier_reward(Reward::TempAccuracy, 0, None)
            .unwrap();
        assert_eq!(run.temp_stat_booster_battles_remaining[5], 5);
        let save = run.checkpoint().unwrap();
        assert!(save.starts_with("POKEROGUE_MINIRUN\t27\n"));
        let resumed = MiniRun::from_checkpoint(&save).unwrap();
        assert_eq!(resumed.temp_stat_booster_battles_remaining[5], 5);
        let legacy_v11 = checkpoint_as_v16(&save)
            .replace("wave_limit\t10\n", "")
            .replacen("MINIRUN\t16", "MINIRUN\t11", 1)
            .replace("greatballs\t0\n", "")
            .replace("ultraballs\t0\n", "")
            .replace("exp_share_stacks\t0\n", "")
            .replace("money\t1000\n", "")
            .replace(
                "temp_stat_booster_battles_remaining\t0,0,0,0,0,5\n",
                "accuracy_booster_battles_remaining\t5\nspeed_booster_battles_remaining\t0\n",
            )
            .lines()
            .map(|line| {
                if let Some(record) = line.strip_prefix("pokemon\t") {
                    format!(
                        "pokemon\t{}",
                        record.split(',').take(12).collect::<Vec<_>>().join(",")
                    )
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let migrated_v11 = MiniRun::from_checkpoint(&legacy_v11).unwrap();
        assert_eq!(
            migrated_v11.temp_stat_booster_battles_remaining,
            [0, 0, 0, 0, 0, 5]
        );
        let legacy_v10 = legacy_v11
            .replacen("MINIRUN\t11", "MINIRUN\t10", 1)
            .replace("speed_booster_battles_remaining\t0\n", "");
        let migrated_v10 = MiniRun::from_checkpoint(&legacy_v10).unwrap();
        assert_eq!(migrated_v10.temp_stat_booster_battles_remaining[5], 5);
        assert_eq!(migrated_v10.temp_stat_booster_battles_remaining[4], 0);

        let mut speed_run = MiniRun::new("SpeedBoosterSaveSeed", "DAY").unwrap();
        speed_run.stage = MiniRunStage::Reward;
        speed_run
            .apply_modifier_reward(Reward::TempSpeed, 0, None)
            .unwrap();
        let speed_resumed = MiniRun::from_checkpoint(&speed_run.checkpoint().unwrap()).unwrap();
        assert_eq!(speed_resumed.temp_stat_booster_battles_remaining[4], 5);

        let invalid = save.replace(
            "temp_stat_booster_battles_remaining\t0,0,0,0,0,5",
            "temp_stat_booster_battles_remaining\t0,0,0,0,0,6",
        );
        assert!(matches!(
            MiniRun::from_checkpoint(&invalid),
            Err(pokerogue_core::MiniRunError::InvalidCheckpoint)
        ));
    }

    #[test]
    fn sitrus_berry_source_threshold_effect_and_save_roundtrip() {
        use pokerogue_core::{MiniRun, MiniRunStage, rewards::Reward};

        let fixture = include_str!("../fixtures/sitrus.csv");
        for line in fixture
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
        {
            let fields: Vec<_> = line.split(',').collect();
            match fields[0] {
                "predicate" => {
                    let max: i32 = fields[1].parse().unwrap();
                    let hp: i32 = fields[2].parse().unwrap();
                    let expected = fields[3] == "true";
                    assert_eq!(hp * 2 < max, expected);
                }
                "effect" => {
                    let max: i32 = fields[1].parse().unwrap();
                    assert_eq!(fields[2], "PokemonHealPhase");
                    assert_eq!((max / 4).max(1), fields[3].parse::<i32>().unwrap());
                }
                _ => panic!("unexpected Sitrus fixture row"),
            }
        }

        let mut run = MiniRun::new("SitrusSaveSeed", "DAY").unwrap();
        run.stage = MiniRunStage::Reward;
        run.apply_modifier_reward(Reward::SitrusBerry, 0, None)
            .unwrap();
        assert!(run.battle.player.has_sitrus_berry);
        let save = run.checkpoint().unwrap();
        assert!(save.starts_with("POKEROGUE_MINIRUN\t27\n"));
        let restored = MiniRun::from_checkpoint(&save).unwrap();
        assert!(restored.battle.player.has_sitrus_berry);
        assert_eq!(restored.wave_limit, 10);
        let old_v13 = checkpoint_as_v16(&save)
            .replace("wave_limit\t10\n", "")
            .replacen("MINIRUN\t16", "MINIRUN\t13", 1)
            .replace("greatballs\t0\n", "")
            .replace("ultraballs\t0\n", "")
            .replace("exp_share_stacks\t0\n", "")
            .replace("money\t1000\n", "");
        let migrated_v13 = MiniRun::from_checkpoint(&old_v13).unwrap();
        assert_eq!(migrated_v13.wave_limit, 20);
        assert!(migrated_v13.battle.player.has_sitrus_berry);

        let legacy = checkpoint_as_v16(&save)
            .replace("wave_limit\t10\n", "")
            .replacen("MINIRUN\t16", "MINIRUN\t12", 1)
            .replace("greatballs\t0\n", "")
            .replace("ultraballs\t0\n", "")
            .replace("exp_share_stacks\t0\n", "")
            .replace("money\t1000\n", "")
            .replace("pokemon\t", "pokemon\t");
        // v12 predates held Sitrus state and must not infer the berry.
        let legacy = legacy
            .lines()
            .map(|line| {
                if let Some(row) = line.strip_prefix("pokemon\t") {
                    format!(
                        "pokemon\t{}",
                        row.split(',').take(15).collect::<Vec<_>>().join(",")
                    )
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !MiniRun::from_checkpoint(&legacy)
                .unwrap()
                .battle
                .player
                .has_sitrus_berry
        );

        let mut long = MiniRun::new_with_wave_limit("LegacyTwentySeed", "DAY", 20).unwrap();
        long.wave = 20;
        long.stage = MiniRunStage::Reward;
        long.reward_claimed = true;
        let mut long = MiniRun::from_checkpoint(&long.checkpoint().unwrap()).unwrap();
        assert_eq!(long.wave_limit, 20);
        long.continue_after_reward().unwrap();
        assert_eq!(long.stage, MiniRunStage::Complete);

        let mut natural = None;
        for index in 0..2500 {
            let mut candidate =
                MiniRun::new(format!("NaturalSitrusSeed{index:04}"), "DAY").unwrap();
            candidate.stage = MiniRunStage::Reward;
            if let Some(offer) = candidate.natural_common_reward_offer() {
                if let Some(slot) = offer.options.iter().position(|option| {
                    option.id == "BERRY" && option.generated_variant.as_deref() == Some("SITRUS")
                }) {
                    natural = Some((candidate, slot));
                    break;
                }
            }
        }
        let (mut natural, option) = natural.expect("natural Sitrus offer seed");
        natural.claim_natural_common_reward(option, 0).unwrap();
        assert!(natural.battle.player.has_sitrus_berry);
        assert!(
            MiniRun::from_checkpoint(&natural.checkpoint().unwrap())
                .unwrap()
                .battle
                .player
                .has_sitrus_berry
        );

        let mut battle = MiniRun::new("SitrusBattleSeed", "DAY").unwrap();
        battle.battle.player.max_hp = 100;
        battle.battle.player.hp = 49;
        battle.battle.player.has_sitrus_berry = true;
        battle.battle.player.attack = 1;
        battle.battle.enemy.hp = 500;
        battle.battle.enemy.max_hp = 500;
        battle.battle.enemy.attack = 50;
        battle.take_action(PlayerAction::UseMove(0)).unwrap();
        assert!(!battle.battle.player.has_sitrus_berry);
        assert!(battle.battle.player.hp > 0);
        assert!(battle.battle.player.hp <= battle.battle.player.max_hp);
        assert!(battle.battle.last_events.iter().any(|event| matches!(
            event,
            pokerogue_core::BattleEvent::SitrusBerryConsumed { .. }
        )));
    }

    #[test]
    fn temp_stat_booster_matches_executed_typescript_methods() {
        let fixture = include_str!("../fixtures/temp-stat-booster.csv");
        for line in fixture
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
        {
            let fields: Vec<_> = line.split(',').collect();
            let stat = fields[0];
            let input: f64 = fields[2].parse().unwrap();
            let expected: f64 = fields[3].parse().unwrap();
            assert_eq!(fields[4], "true");
            assert_eq!(fields[5], "true");
            assert_eq!(fields[6], "false");
            assert_eq!(fields[7], "5");
            assert_eq!(pokerogue_core::rewards::lapse_temp_stat_booster(2), Some(1));
            assert_eq!(pokerogue_core::rewards::lapse_temp_stat_booster(1), None);
            if stat == "ACC" {
                assert_eq!(
                    f64::from(pokerogue_core::apply_accuracy_stage_booster(
                        input as i8,
                        true
                    )),
                    expected
                );
            } else {
                assert_eq!(input + fields[1].parse::<f64>().unwrap(), expected);
                if stat == "SPD" {
                    assert_eq!(pokerogue_core::boosted_stage_multiplier(0, 0.2), 1.2);
                }
            }
        }
    }

    #[test]
    fn all_booster_stats_apply_to_physical_and_special_damage() {
        let player = Battler::demo(Species::Bulbasaur, 30, [Move::Tackle, Move::Ember]);
        let foe = Battler::demo(Species::Charmander, 30, [Move::Tackle, Move::Ember]);
        let physical = calculate_damage(&player, &foe, Move::Tackle, 100).damage;
        let attack_boosted = calculate_damage_with_boosters(
            &player,
            &foe,
            Move::Tackle,
            100,
            [true, false, false, false, false, false],
            [false; 6],
        )
        .damage;
        let defense_boosted = calculate_damage_with_boosters(
            &player,
            &foe,
            Move::Tackle,
            100,
            [false; 6],
            [false, true, false, false, false, false],
        )
        .damage;
        assert!(attack_boosted > physical);
        assert!(defense_boosted < physical);

        let special = calculate_damage(&player, &foe, Move::Ember, 100).damage;
        let special_attack_boosted = calculate_damage_with_boosters(
            &player,
            &foe,
            Move::Ember,
            100,
            [false, false, true, false, false, false],
            [false; 6],
        )
        .damage;
        let special_defense_boosted = calculate_damage_with_boosters(
            &player,
            &foe,
            Move::Ember,
            100,
            [false; 6],
            [false, false, false, true, false, false],
        )
        .damage;
        assert!(special_attack_boosted > special);
        assert!(special_defense_boosted < special);

        let mut staged_player = player.clone();
        staged_player.set_stat_stage(BattleStat::Attack, 1);
        staged_player.set_stat_stage(BattleStat::SpecialAttack, 1);
        assert!(calculate_damage(&staged_player, &foe, Move::Tackle, 100).damage > physical);
        assert!(calculate_damage(&staged_player, &foe, Move::Ember, 100).damage > special);
    }

    #[test]
    fn naturally_generated_boosters_for_all_six_stats_can_be_claimed_and_saved() {
        use pokerogue_core::{MiniRun, MiniRunStage};
        let variants = [
            ("ATK", 0),
            ("DEF", 1),
            ("SPATK", 2),
            ("SPDEF", 3),
            ("SPD", 4),
            ("ACC", 5),
        ];
        for (variant, expected_index) in variants {
            let mut found = None;
            for seed_index in 0..2500 {
                let mut candidate =
                    MiniRun::new(format!("AllBooster{variant}{seed_index:04}"), "DAY").unwrap();
                candidate.stage = MiniRunStage::Reward;
                if let Some(offer) = candidate.natural_common_reward_offer() {
                    if let Some(option_index) = offer.options.iter().position(|option| {
                        option.id == "TEMP_STAT_STAGE_BOOSTER"
                            && option.generated_variant.as_deref() == Some(variant)
                    }) {
                        found = Some((candidate, option_index));
                        break;
                    }
                }
            }
            let (mut run, option_index) =
                found.unwrap_or_else(|| panic!("no natural COMMON {variant} fixture seed"));
            run.claim_natural_common_reward(option_index, 0).unwrap();
            assert_eq!(run.temp_stat_booster_battles_remaining[expected_index], 5);
            let saved = MiniRun::from_checkpoint(&run.checkpoint().unwrap()).unwrap();
            assert_eq!(
                saved.temp_stat_booster_battles_remaining,
                run.temp_stat_booster_battles_remaining
            );
        }
    }

    #[test]
    fn claimed_attack_booster_changes_actual_turn_damage() {
        use pokerogue_core::{MiniRun, MiniRunStage, rewards::Reward};
        let mut plain = MiniRun::new("ActualBoosterBattleSeed", "DAY").unwrap();
        let mut boosted = MiniRun::new("ActualBoosterBattleSeed", "DAY").unwrap();
        plain.battle.player.attack = 200;
        boosted.battle.player.attack = 200;
        plain.battle.enemy.hp = 500;
        plain.battle.enemy.max_hp = 500;
        boosted.battle.enemy.hp = 500;
        boosted.battle.enemy.max_hp = 500;
        boosted.stage = MiniRunStage::Reward;
        boosted
            .apply_modifier_reward(Reward::TempAttack, 0, None)
            .unwrap();
        boosted.stage = MiniRunStage::Battle;
        plain.take_action(PlayerAction::UseMove(0)).unwrap();
        boosted.take_action(PlayerAction::UseMove(0)).unwrap();
        assert!(boosted.battle.enemy.hp < plain.battle.enemy.hp);
    }

    #[test]
    fn natural_x_accuracy_common_option_is_claimable_only_for_acc_variant() {
        use pokerogue_core::{MiniRun, MiniRunStage};

        let mut found = None;
        for index in 0..2500 {
            let mut candidate = MiniRun::new(format!("TempAccSeed{index:04}"), "DAY").unwrap();
            candidate.stage = MiniRunStage::Reward;
            if let Some(offer) = candidate.natural_common_reward_offer() {
                if let Some(option) = offer.options.iter().position(|option| {
                    option.id == "TEMP_STAT_STAGE_BOOSTER"
                        && option.generated_variant.as_deref() == Some("ACC")
                }) {
                    found = Some((candidate, option));
                    break;
                }
            }
        }
        let (mut run, index) = found.expect("deterministic seed with a natural ACC booster");
        run.claim_natural_common_reward(index, 0).unwrap();
        assert_eq!(run.temp_stat_booster_battles_remaining[5], 5);
        assert!(run.reward_claimed);
    }

    #[test]
    fn natural_x_speed_common_option_changes_only_player_turn_speed() {
        use pokerogue_core::{Battler, MiniRun, MiniRunStage};
        use pokerogue_data::{Move, Species};

        let mut found = None;
        for index in 0..2500 {
            let mut candidate = MiniRun::new(format!("TempSpeedSeed{index:04}"), "DAY").unwrap();
            candidate.stage = MiniRunStage::Reward;
            if let Some(offer) = candidate.natural_common_reward_offer() {
                if let Some(option) = offer.options.iter().position(|option| {
                    option.id == "TEMP_STAT_STAGE_BOOSTER"
                        && option.generated_variant.as_deref() == Some("SPD")
                }) {
                    found = Some((candidate, option));
                    break;
                }
            }
        }
        let (mut run, index) = found.expect("deterministic seed with a natural SPD booster");
        run.claim_natural_common_reward(index, 0).unwrap();
        assert_eq!(run.temp_stat_booster_battles_remaining[4], 5);
        let player = Battler::demo(Species::Bulbasaur, 5, [Move::Tackle, Move::Tackle]);
        let normal = player.effective_speed_with_bonus(0.0);
        let boosted = player.effective_speed_with_bonus(0.2);
        assert!(boosted > normal);
    }

    #[test]
    fn temp_booster_durations_lapse_at_the_modeled_battle_outcome() {
        use pokerogue_core::{MiniRun, rewards::TEMP_STAT_BOOSTER_MAX_BATTLES};

        let mut run = MiniRun::new("TempBoosterLapseSeed", "DAY").unwrap();
        run.temp_stat_booster_battles_remaining[5] = TEMP_STAT_BOOSTER_MAX_BATTLES;
        run.temp_stat_booster_battles_remaining[4] = TEMP_STAT_BOOSTER_MAX_BATTLES;
        run.battle.enemy.hp = 1;
        run.battle.player.hp = 500;
        run.battle.player.max_hp = 500;
        run.take_action(PlayerAction::UseMove(0)).unwrap();
        assert_eq!(run.battle.outcome, Some(Outcome::PlayerWin));
        assert_eq!(run.temp_stat_booster_battles_remaining[5], 4);
        assert_eq!(run.temp_stat_booster_battles_remaining[4], 4);
    }

    #[test]
    fn lure_lapse_and_refresh_match_executed_source_modifier_methods() {
        use pokerogue_core::rewards::lapse_lure_battles;
        for row in include_str!("../fixtures/lure.csv")
            .lines()
            .filter(|line| !line.starts_with('#'))
        {
            let fields: Vec<_> = row.split(',').collect();
            match fields[0] {
                "lapse" => {
                    let actual = lapse_lure_battles(fields[1].parse().unwrap());
                    assert_eq!(actual.is_some(), fields[3] == "1", "{row}");
                    if let Some(remaining) = actual {
                        assert_eq!(remaining, fields[2].parse::<u8>().unwrap(), "{row}");
                    }
                }
                "refresh" => {
                    assert_eq!(
                        pokerogue_core::rewards::LURE_MAX_BATTLES,
                        fields[2].parse::<u8>().unwrap()
                    );
                }
                _ => panic!("unknown lure fixture row {row}"),
            }
        }

        let mut run = pokerogue_core::MiniRun::new("LureLapseOnVictory", "DAY").unwrap();
        run.lure_battles_remaining = 10;
        run.battle.enemy.hp = 1;
        run.take_action(PlayerAction::UseMove(1)).unwrap();
        assert_eq!(run.stage, pokerogue_core::MiniRunStage::Reward);
        assert_eq!(run.lure_battles_remaining, 9);
    }

    #[test]
    fn source_supported_progression_and_pp_rewards_apply_and_survive_saves() {
        use pokerogue_core::{MiniRun, MiniRunError, MiniRunStage, rewards::Reward};

        let mut candy_run = MiniRun::new("RewardEffects", "DAY").unwrap();
        candy_run.stage = MiniRunStage::Reward;
        let member = candy_run.battle.player.member_mut(0).unwrap();
        member.exp =
            pokerogue_data::total_exp_for_level(member.level, member.species.growth_rate());
        let old_level = member.level;
        let draws = candy_run.battle.rng_draws().len();
        candy_run
            .apply_modifier_reward(Reward::RareCandy, 0, None)
            .unwrap();
        assert_eq!(
            candy_run.battle.player.member(0).unwrap().level,
            old_level + 1
        );
        assert_eq!(candy_run.battle.rng_draws().len(), draws);
        let restored = MiniRun::from_checkpoint(&candy_run.checkpoint().unwrap()).unwrap();
        assert_eq!(
            restored.battle.player.member(0).unwrap().level,
            old_level + 1
        );
        assert_eq!(
            restored.battle.player.member(0).unwrap().exp,
            pokerogue_data::total_exp_for_level(
                old_level + 1,
                restored
                    .battle
                    .player
                    .member(0)
                    .unwrap()
                    .species
                    .growth_rate()
            )
        );

        let mut ether_run = MiniRun::new("RewardEffects", "DAY").unwrap();
        ether_run.stage = MiniRunStage::Reward;
        ether_run.battle.player.member_mut(0).unwrap().move_pp_used = [20, 15, 0, 0];
        assert_eq!(
            ether_run.apply_modifier_reward(Reward::Ether, 0, None),
            Err(MiniRunError::InvalidRewardTarget)
        );
        assert_eq!(
            ether_run.battle.player.member(0).unwrap().move_pp_used,
            [20, 15, 0, 0]
        );
        ether_run
            .apply_modifier_reward(Reward::Ether, 0, Some(0))
            .unwrap();
        assert_eq!(
            ether_run.battle.player.member(0).unwrap().move_pp_used,
            [10, 15, 0, 0]
        );
        let restored = MiniRun::from_checkpoint(&ether_run.checkpoint().unwrap()).unwrap();
        assert_eq!(
            restored.battle.player.member(0).unwrap().move_pp_used,
            [10, 15, 0, 0]
        );

        for reward in [Reward::HyperPotion, Reward::MaxPotion] {
            let mut run = MiniRun::new("RewardEffects", "DAY").unwrap();
            run.stage = MiniRunStage::Reward;
            let member = run.battle.player.member_mut(0).unwrap();
            member.hp = 1;
            let expected_hp = member.max_hp;
            run.apply_modifier_reward(reward, 0, None).unwrap();
            assert_eq!(run.battle.player.member(0).unwrap().hp, expected_hp);
            assert_eq!(
                MiniRun::from_checkpoint(&run.checkpoint().unwrap())
                    .unwrap()
                    .battle
                    .player
                    .member(0)
                    .unwrap()
                    .hp,
                expected_hp
            );
        }

        for reward in [Reward::Revive, Reward::MaxRevive] {
            let mut run = MiniRun::new("RewardEffects", "DAY").unwrap();
            run.stage = MiniRunStage::Reward;
            run.battle.player.member_mut(0).unwrap().hp = 0;
            let expected_hp = reward
                .revive(0, run.battle.player.member(0).unwrap().max_hp)
                .unwrap();
            run.apply_modifier_reward(reward, 0, None).unwrap();
            assert_eq!(run.battle.player.member(0).unwrap().hp, expected_hp);
            assert_eq!(
                MiniRun::from_checkpoint(&run.checkpoint().unwrap())
                    .unwrap()
                    .battle
                    .player
                    .member(0)
                    .unwrap()
                    .hp,
                expected_hp
            );
        }

        let mut max_ether_run = MiniRun::new("RewardEffects", "DAY").unwrap();
        max_ether_run.stage = MiniRunStage::Reward;
        max_ether_run
            .battle
            .player
            .member_mut(0)
            .unwrap()
            .move_pp_used = [20, 15, 0, 0];
        max_ether_run
            .apply_modifier_reward(Reward::MaxEther, 0, Some(1))
            .unwrap();
        assert_eq!(
            max_ether_run.battle.player.member(0).unwrap().move_pp_used,
            [20, 0, 0, 0]
        );
    }

    #[test]
    fn original_move_definitions_are_reflected_in_port_data() {
        // move.ts defines Tackle and Ember at 40 power; these values are a
        // focused data parity check against those declarations.
        assert_eq!(Move::Tackle.power(), 40);
        assert_eq!(Move::Tackle.pp(), 35);
        assert_eq!(Move::Ember.power(), 40);
        assert_eq!(Move::Ember.pp(), 25);
        assert_eq!(Move::VineWhip.power(), 45);
        assert_eq!(Move::QuickAttack.power(), 40);
        assert_eq!(Move::QuickAttack.priority(), 1);
        assert_eq!(Move::Tackle.priority(), 0);
        assert_eq!(Move::Counter.priority(), -5);
        assert_eq!(Move::SandAttack.accuracy(), 100);
        assert!(Move::SandAttack.is_status());
        assert_eq!(
            Move::SandAttack.effect(),
            Some(MoveEffect::StatStageChange {
                stat: BattleStat::Accuracy,
                stages: -1,
                chance: -1,
            })
        );
        let harden = Move::Imported(106);
        assert_eq!(harden.id(), "HARDEN");
        assert_eq!(harden.target(), pokerogue_data::MoveTarget::Attacker);
        assert_eq!(
            harden.effect(),
            Some(MoveEffect::SelfStatStageChange {
                stat: BattleStat::Defense,
                stages: 1,
            })
        );
        let withdraw = Move::Imported(110);
        assert_eq!(withdraw.id(), "WITHDRAW");
        assert_eq!(withdraw.target(), pokerogue_data::MoveTarget::Attacker);
        assert_eq!(withdraw.effect(), harden.effect());
    }

    #[test]
    fn harden_single_move_branch_and_defense_stage_match_executed_typescript() {
        let rows: Vec<_> = include_str!("../fixtures/harden.csv").lines().collect();
        assert!(rows.contains(&"move,HARDEN,2,0"));
        assert!(rows.contains(&"effect,HARDEN,StatStageChangePhase,1,DEF,1,0"));
        for line in rows.into_iter().filter(|line| line.starts_with("stage,")) {
            let fields: Vec<_> = line.split(',').collect();
            let stage: i8 = fields[1].parse().unwrap();
            let source_multiplier: f64 = fields[2].parse().unwrap();
            let attacker = Battler::demo(Species::Bulbasaur, 5, [Move::Tackle, Move::VineWhip]);
            let mut defender = Battler::demo(Species::Imported(266), 9, [Move::Imported(106); 2]);
            defender.set_stat_stage(BattleStat::Defense, stage);
            let expected_defense = f64::from(defender.defense) * source_multiplier;
            let result = calculate_damage(&attacker, &defender, Move::Tackle, 100);
            let level_multiplier = 2.0 * f64::from(attacker.level) / 5.0 + 2.0;
            let expected_damage =
                ((level_multiplier * f64::from(Move::Tackle.power()) * f64::from(attacker.attack)
                    / expected_defense
                    / 50.0
                    + 2.0)
                    * 1.0
                    * 1.0
                    * f64::from(result.stab_multiplier)
                    * f64::from(result.type_multiplier))
                .floor()
                .max(1.0) as i32;
            assert_eq!(result.damage, expected_damage, "defense stage {stage}");
        }

        let mut battle = Battle::demo_with_seed("HardenSingleMoveSeed");
        battle.enemy = Battler::demo(Species::Imported(266), 9, [Move::Imported(106); 2]);
        battle.take_turn(PlayerAction::UseMove(0)).unwrap();
        assert_eq!(battle.enemy.stat_stage(BattleStat::Defense), 1);
        let decision = battle.last_enemy_decision.as_ref().unwrap();
        assert_eq!(decision.selected_move.id(), "HARDEN");
        assert_eq!(decision.candidates, [Move::Imported(106)]);
        assert!(decision.random_rolls.is_empty());
        assert!(battle.log.contains("Defense rose to 1"));
    }

    #[test]
    fn imported_enemy_stage_moves_match_source_scores_and_random_choice() {
        let fixture = include_str!("../fixtures/status-stage-score.csv");
        for row in fixture.lines().filter(|line| {
            line.starts_with("GROWL,")
                || line.starts_with("TAIL_WHIP,")
                || line.starts_with("LEER,")
        }) {
            let fields: Vec<_> = row.split(',').collect();
            let id = fields[0];
            let stat = match fields[1] {
                "ATK" => BattleStat::Attack,
                "DEF" => BattleStat::Defense,
                _ => unreachable!(),
            };
            let stage: i8 = fields[2].parse().unwrap();
            let expected: f64 = fields[4].parse().unwrap();
            let move_ = Move::Imported(move_by_id(id).unwrap().numeric_id);
            let other = if id == "GROWL" {
                Move::Imported(move_by_id("TAIL_WHIP").unwrap().numeric_id)
            } else {
                Move::Imported(move_by_id("GROWL").unwrap().numeric_id)
            };
            let mut battle = Battle::demo_with_seed("ParitySeed000000");
            battle.enemy = Battler::demo(Species::Charmander, 5, [move_, other]);
            battle.player.set_stat_stage(stat, stage);
            battle.choose_move(0);
            let decision = battle.last_enemy_decision.as_ref().unwrap();
            assert_eq!(decision.candidates, [move_, other]);
            let actual = decision
                .scores
                .iter()
                .find(|(candidate, _)| candidate.id() == id)
                .unwrap()
                .1;
            assert_eq!(actual, expected, "{id} target stage {stage}");
        }

        let mut battle = Battle::demo_with_seed("ParitySeed000000");
        let growl = Move::Imported(move_by_id("GROWL").unwrap().numeric_id);
        let tail_whip = Move::Imported(move_by_id("TAIL_WHIP").unwrap().numeric_id);
        battle.enemy = Battler::demo(Species::Charmander, 5, [growl, tail_whip]);
        battle.choose_move(0);
        let decision = battle.last_enemy_decision.as_ref().unwrap();
        assert_eq!(decision.selected_move.id(), "GROWL");
        assert_eq!(decision.random_rolls, [4]);
        assert_eq!(
            growl.effect(),
            Some(MoveEffect::StatStageChange {
                stat: BattleStat::Attack,
                stages: -1,
                chance: -1,
            })
        );
        assert_eq!(
            tail_whip.effect(),
            Some(MoveEffect::StatStageChange {
                stat: BattleStat::Defense,
                stages: -1,
                chance: -1,
            })
        );
    }

    #[test]
    fn pp_consumption_matches_executed_pokemon_move_method() {
        let mut battler = Battler::demo(Species::Charmander, 5, [Move::Tackle, Move::Ember]);
        for row in include_str!("../fixtures/move-pp.csv")
            .lines()
            .filter(|line| line.starts_with("use,"))
        {
            let fields: Vec<_> = row.split(',').collect();
            let used = fields[1].parse::<u8>().unwrap();
            let count = fields[2].parse::<u8>().unwrap();
            let max = fields[3].parse::<u8>().unwrap();
            let expected = fields[4].parse::<u8>().unwrap();
            let index = match max {
                35 => 0,
                25 => 1,
                _ => panic!("unsupported oracle PP {max}"),
            };
            battler.move_pp_used[index] = used;
            assert!(battler.consume_move_pp(index, count));
            assert_eq!(battler.move_pp_used[index], expected, "{row}");
        }
    }

    #[test]
    fn pp_restore_matches_executed_single_and_party_restore_methods() {
        for row in include_str!("../fixtures/pp-restore.csv")
            .lines()
            .filter(|line| !line.starts_with('#'))
        {
            let fields: Vec<_> = row.split(',').collect();
            let restore = fields[2]
                .parse::<i8>()
                .ok()
                .and_then(|value| u8::try_from(value).ok());
            let expected: Vec<u8> = fields[3]
                .split('|')
                .map(|value| value.parse().unwrap())
                .collect();
            let mut battler = Battler::demo(Species::Charmander, 5, [Move::Tackle, Move::Ember]);
            match fields[0] {
                "single" => {
                    battler.move_pp_used[0] = fields[1].parse().unwrap();
                    assert!(battler.restore_move_pp(0, restore));
                }
                "all" => {
                    let used: Vec<u8> = fields[1]
                        .split('|')
                        .map(|value| value.parse().unwrap())
                        .collect();
                    battler.move_pp_used[..used.len()].copy_from_slice(&used);
                    battler.restore_all_move_pp(restore);
                }
                _ => panic!("unknown PP restore fixture {row}"),
            }
            assert_eq!(&battler.move_pp_used[..expected.len()], expected, "{row}");
        }
    }

    #[test]
    fn four_move_slots_roundtrip_in_checkpoint_v23() {
        use pokerogue_core::{MiniRun, MiniRunStage};

        let mut run = MiniRun::new("FourMoveCheckpointSeed", "DAY").unwrap();
        run.stage = MiniRunStage::Reward;
        let member = run.battle.player.member_mut(0).unwrap();
        member.moves = ["TACKLE", "GROWL", "VINE_WHIP", "LEECH_SEED"]
            .map(|id| Move::Imported(move_by_id(id).unwrap().numeric_id));
        member.move_count = 4;
        member.move_pp_used = [1, 2, 3, 4];
        member.move_pp_ups = [1, 0, 0, 0];

        let checkpoint = run.checkpoint().unwrap();
        assert!(checkpoint.starts_with("POKEROGUE_MINIRUN\t27\n"));
        let restored = MiniRun::from_checkpoint(&checkpoint).unwrap();
        let restored_member = restored.battle.player.member(0).unwrap();
        assert_eq!(restored_member.move_count(), 4);
        assert_eq!(
            restored_member.moves.map(Move::id),
            ["TACKLE", "GROWL", "VINE_WHIP", "LEECH_SEED"]
        );
        assert_eq!(restored_member.move_pp_used, [1, 2, 3, 4]);
        assert_eq!(restored_member.move_pp_ups, [1, 0, 0, 0]);
    }

    #[test]
    fn natural_common_tm_teaches_an_eligible_member_and_persists() {
        use pokerogue_core::{MiniRun, MiniRunStage};

        let mut run = MiniRun::new("TmSearch1", "DAY").unwrap();
        run.stage = MiniRunStage::Reward;
        let offer = run.natural_supported_reward_offer().unwrap();
        let source_row = include_str!("../fixtures/great-recovery-offer.csv")
            .lines()
            .find(|line| line.starts_with("TmSearch1,1,"))
            .unwrap();
        let source: Vec<_> = source_row.splitn(4, ',').collect();
        assert_eq!(
            offer
                .options
                .iter()
                .map(|option| format!(
                    "{}:{}:{}:{}",
                    option.name, option.upgrades, option.tier, option.pool_index
                ))
                .collect::<Vec<_>>()
                .join("|"),
            source[2]
        );
        assert_eq!(
            offer
                .rng_draws
                .iter()
                .map(|draw| format!("{}:{}:{}", draw.range, draw.min, draw.value))
                .collect::<Vec<_>>()
                .join("|"),
            source[3]
        );
        let option_index = offer
            .options
            .iter()
            .position(|option| option.id == "TM_COMMON")
            .expect("seeded offer with a compatible TM");
        let move_id = offer.options[option_index]
            .generated_variant
            .as_deref()
            .unwrap();
        assert_eq!(
            offer.options[option_index].generated_variant.as_deref(),
            Some("BEAT_UP")
        );
        let target = (0..run.battle.player.len())
            .find(|slot| {
                let member = run.battle.player.member(*slot).unwrap();
                let known = member.moves[..member.move_count()]
                    .iter()
                    .map(|movement| movement.numeric_id())
                    .collect::<Vec<_>>();
                pokerogue_data::compatible_tms_for_species(
                    member.species.numeric_id(),
                    &known,
                    &[],
                    0,
                )
                .iter()
                .any(|movement| movement.id == move_id)
            })
            .unwrap();
        let draws_before = run.battle.rng_draws().len();
        let initial_count = run.battle.player.member(target).unwrap().move_count();
        run.claim_natural_supported_reward(option_index, target)
            .unwrap();
        let member = run.battle.player.member(target).unwrap();
        assert_eq!(member.move_count(), initial_count + 1);
        assert_eq!(member.moves[initial_count].id(), move_id);
        assert!(run.reward_claimed);
        assert_eq!(run.battle.rng_draws().len(), draws_before);
        let restored = MiniRun::from_checkpoint(&run.checkpoint().unwrap()).unwrap();
        assert_eq!(
            restored.battle.player.member(target).unwrap().used_tm_ids,
            vec![move_by_id(move_id).unwrap().numeric_id]
        );
        assert_eq!(
            restored.battle.player.member(target).unwrap().moves[initial_count].id(),
            move_id
        );
        assert!(
            !pokerogue_data::compatible_tms_for_species(
                restored
                    .battle
                    .player
                    .member(target)
                    .unwrap()
                    .species
                    .numeric_id(),
                &restored.battle.player.member(target).unwrap().moves
                    [..restored.battle.player.member(target).unwrap().move_count()]
                    .iter()
                    .map(|movement| movement.numeric_id())
                    .collect::<Vec<_>>(),
                &restored.battle.player.member(target).unwrap().used_tm_ids,
                0,
            )
            .iter()
            .any(|movement| movement.id == move_id)
        );

        let mut full_run = MiniRun::new("TmSearch1", "DAY").unwrap();
        full_run.stage = MiniRunStage::Reward;
        let full_offer = full_run.natural_supported_reward_offer().unwrap();
        let full_index = full_offer
            .options
            .iter()
            .position(|option| option.id == "TM_COMMON")
            .unwrap();
        let full_move_id = full_offer.options[full_index]
            .generated_variant
            .as_deref()
            .unwrap();
        let full_target = (0..full_run.battle.player.len())
            .find(|slot| {
                let member = full_run.battle.player.member(*slot).unwrap();
                let known = member.moves[..member.move_count()]
                    .iter()
                    .map(|movement| movement.numeric_id())
                    .collect::<Vec<_>>();
                pokerogue_data::compatible_tms_for_species(
                    member.species.numeric_id(),
                    &known,
                    &[],
                    0,
                )
                .iter()
                .any(|movement| movement.id == full_move_id)
            })
            .unwrap();
        let full_member = full_run.battle.player.member_mut(full_target).unwrap();
        full_member.moves = ["TACKLE", "GROWL", "VINE_WHIP", "LEECH_SEED"]
            .map(|id| Move::Imported(move_by_id(id).unwrap().numeric_id));
        full_member.move_count = 4;
        full_member.move_pp_used[1] = 2;
        full_member.move_pp_ups[1] = 1;
        let before = full_member.moves;
        let replacement_slot = 1;
        assert!(
            full_run
                .claim_natural_supported_reward(full_index, full_target)
                .is_err()
        );
        assert_eq!(
            full_run.battle.player.member(full_target).unwrap().moves,
            before
        );
        assert!(!full_run.reward_claimed);
        full_run
            .claim_natural_supported_reward_for_move(
                full_index,
                full_target,
                Some(replacement_slot),
            )
            .unwrap();
        let replaced = full_run.battle.player.member(full_target).unwrap();
        assert_eq!(replaced.moves[replacement_slot].id(), full_move_id);
        assert_eq!(replaced.move_pp_used[replacement_slot], 0);
        assert_eq!(replaced.move_pp_ups[replacement_slot], 0);
        assert!(
            replaced
                .used_tm_ids
                .contains(&move_by_id(full_move_id).unwrap().numeric_id)
        );
        let save = full_run.checkpoint().unwrap();
        let migrated_v21 = save
            .replacen("POKEROGUE_MINIRUN\t27", "POKEROGUE_MINIRUN\t21", 1)
            .lines()
            .filter(|line| {
                !line.starts_with("used_tms\t")
                    && !line.starts_with("friendship\t")
                    && !line.starts_with("soothe_bell\t")
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        assert!(
            MiniRun::from_checkpoint(&migrated_v21)
                .unwrap()
                .battle
                .player
                .member(full_target)
                .unwrap()
                .used_tm_ids
                .is_empty()
        );
    }

    #[test]
    fn great_tm_can_be_claimed_by_an_eligible_party_member() {
        use pokerogue_core::{MiniRun, MiniRunStage};

        let (mut run, tm_index) = (0..20_000)
            .find_map(|index| {
                let mut run = MiniRun::new(format!("GreatTmClaim{index}"), "DAY").ok()?;
                run.wave = 1;
                run.stage = MiniRunStage::Reward;
                let offer = run.natural_supported_reward_offer()?;
                let index = offer
                    .options
                    .iter()
                    .position(|option| option.id == "TM_GREAT")?;
                Some((run, index))
            })
            .expect("bounded seed search finds a Great TM option");
        let option = run.natural_supported_reward_offer().unwrap().options[tm_index].clone();
        let movement = option.generated_variant.unwrap();
        let target = (0..run.battle.player.len())
            .find(|slot| {
                let member = run.battle.player.member(*slot).unwrap();
                let known = member.moves[..member.move_count()]
                    .iter()
                    .map(|movement| movement.numeric_id())
                    .collect::<Vec<_>>();
                pokerogue_data::compatible_tms_for_species(
                    member.species.numeric_id(),
                    &known,
                    &member.used_tm_ids,
                    1,
                )
                .iter()
                .any(|candidate| candidate.id == movement)
            })
            .unwrap();
        run.claim_natural_supported_reward(tm_index, target)
            .unwrap();
        let member = run.battle.player.member(target).unwrap();
        assert!(
            member.moves[..member.move_count()]
                .iter()
                .any(|known| known.id() == movement)
        );
        assert!(!member.used_tm_ids.is_empty());
        assert!(
            member
                .used_tm_ids
                .contains(&pokerogue_data::move_by_id(&movement).unwrap().numeric_id)
        );
    }

    #[test]
    fn natural_great_tm_offer_and_rng_match_executed_typescript_wrapper() {
        use pokerogue_core::{MiniRun, MiniRunStage};
        let row = include_str!("../fixtures/great-recovery-offer.csv")
            .lines()
            .find(|line| line.starts_with("TmGreatSearch0000,19,"))
            .unwrap();
        let source: Vec<_> = row.splitn(4, ',').collect();
        let mut run = MiniRun::new(source[0], "DAY").unwrap();
        run.wave = 19;
        run.stage = MiniRunStage::Reward;
        let offer = run.natural_supported_reward_offer().unwrap();
        assert_eq!(
            offer
                .options
                .iter()
                .map(|option| format!(
                    "{}:{}:{}:{}",
                    option.name, option.upgrades, option.tier, option.pool_index
                ))
                .collect::<Vec<_>>()
                .join("|"),
            source[2]
        );
        assert_eq!(
            offer
                .rng_draws
                .iter()
                .map(|draw| format!("{}:{}:{}", draw.range, draw.min, draw.value))
                .collect::<Vec<_>>()
                .join("|"),
            source[3]
        );
    }

    #[test]
    fn learn_move_empty_slot_and_duplicate_guards_match_executed_source_phase() {
        let rows: Vec<_> = include_str!("../fixtures/learn-move-phase.csv")
            .lines()
            .filter(|line| {
                !line.is_empty()
                    && !line.starts_with('#')
                    && !line.starts_with("case,")
                    && !line.starts_with("set-move")
            })
            .collect();
        assert_eq!(rows.len(), 4);
        for row in rows {
            let fields: Vec<_> = row.split(',').collect();
            match fields[0] {
                "empty-slot" => {
                    assert_eq!(fields[4], "super|mode|learn:2");
                    let moves = fields[1]
                        .split('|')
                        .map(|id| Move::Imported(move_by_id(id).unwrap().numeric_id))
                        .collect::<Vec<_>>();
                    let mut battler = Battler::demo(Species::Bulbasaur, 5, [moves[0], moves[1]]);
                    assert!(
                        battler
                            .learn_move(Move::Imported(move_by_id(fields[2]).unwrap().numeric_id,))
                    );
                    assert_eq!(battler.move_count(), 3);
                }
                "duplicate-move" => {
                    assert_eq!(fields[4], "super|end");
                    let mut battler = Battler::demo(
                        Species::Bulbasaur,
                        5,
                        [
                            Move::Tackle,
                            Move::Imported(move_by_id("GROWL").unwrap().numeric_id),
                        ],
                    );
                    assert!(
                        !battler
                            .learn_move(Move::Imported(move_by_id(fields[2]).unwrap().numeric_id,))
                    );
                    assert_eq!(battler.move_count(), 2);
                }
                "full-moveset" => assert_eq!(fields[4], "super|mode|replace"),
                "unimplemented" => assert_eq!(fields[4], "super|end"),
                _ => panic!("unexpected LearnMovePhase fixture case: {row}"),
            }
        }
    }

    #[test]
    fn move_replacement_slot_matches_executed_original_set_move_method() {
        for row in include_str!("../fixtures/learn-move-phase.csv")
            .lines()
            .filter(|line| line.starts_with("set-move,"))
        {
            let fields: Vec<_> = row.split(',').collect();
            let move_ids: [&str; 4] = fields[3].split('|').collect::<Vec<_>>().try_into().unwrap();
            let mut battler = Battler::demo(
                Species::Bulbasaur,
                5,
                [
                    Move::Imported(move_by_id("TACKLE").unwrap().numeric_id),
                    Move::Imported(move_by_id("GROWL").unwrap().numeric_id),
                    Move::Imported(move_by_id("VINE_WHIP").unwrap().numeric_id),
                    Move::Imported(move_by_id("LEECH_SEED").unwrap().numeric_id),
                ],
            );
            let index: usize = fields[1].parse().unwrap();
            if fields[2] != "NONE" {
                assert!(battler.replace_move(
                    index,
                    Move::Imported(move_by_id(fields[2]).unwrap().numeric_id)
                ));
            }
            assert_eq!(battler.moves.map(Move::id), move_ids, "{row}");
        }
    }

    #[test]
    fn generated_species_and_move_records_match_pinned_typescript_extraction() {
        let mut species_count = 0;
        let mut move_count = 0;
        let mut level_move_count = 0;
        let mut species_tm_count = 0;
        let mut prevolution_count = 0;
        let mut tm_tier_count = 0;
        let mut pool_bucket_count = 0;
        let mut tier_weight_count = 0;
        for row in include_str!("../fixtures/imported-demo-data.csv").lines() {
            if row.is_empty() || row.starts_with('#') {
                continue;
            }
            let fields: Vec<_> = row.split(',').collect();
            match fields[0] {
                "species" => {
                    species_count += 1;
                    let data = IMPORTED_SPECIES
                        .iter()
                        .find(|record| record.id == fields[1])
                        .unwrap();
                    assert_eq!(data.numeric_id.to_string(), fields[2], "{row}");
                    assert_eq!(data.name, fields[3], "{row}");
                    assert_eq!(data.type1, element(fields[4]), "{row}");
                    assert_eq!(
                        data.type2
                            .map(|t| format!("{t:?}").to_uppercase())
                            .unwrap_or_default(),
                        fields[5]
                    );
                    assert_eq!(data.base_hp.to_string(), fields[6], "{row}");
                    assert_eq!(data.base_attack.to_string(), fields[7], "{row}");
                    assert_eq!(data.base_defense.to_string(), fields[8], "{row}");
                    assert_eq!(data.base_special_attack.to_string(), fields[9], "{row}");
                    assert_eq!(data.base_special_defense.to_string(), fields[10], "{row}");
                    assert_eq!(data.base_speed.to_string(), fields[11], "{row}");
                    assert_eq!(data.base_exp.to_string(), fields[12], "{row}");
                    let growth_name = match data.growth_rate {
                        pokerogue_data::GrowthRate::Erratic => "ERRATIC",
                        pokerogue_data::GrowthRate::Fast => "FAST",
                        pokerogue_data::GrowthRate::MediumFast => "MEDIUM_FAST",
                        pokerogue_data::GrowthRate::MediumSlow => "MEDIUM_SLOW",
                        pokerogue_data::GrowthRate::Slow => "SLOW",
                        pokerogue_data::GrowthRate::Fluctuating => "FLUCTUATING",
                    };
                    assert_eq!(growth_name, fields[13], "{row}");
                    assert_eq!(data.catch_rate.to_string(), fields[14], "{row}");
                    assert_eq!(data.base_friendship.to_string(), fields[15], "{row}");
                    assert_eq!(data.ability1, fields[16], "{row}");
                    assert_eq!(data.ability2, fields[17], "{row}");
                    assert_eq!(data.ability_hidden, fields[18], "{row}");
                    assert_eq!(
                        data.male_percent
                            .map(|value| value.to_string())
                            .unwrap_or_default(),
                        fields[19],
                        "{row}"
                    );
                    assert_eq!(data.ability1_id.to_string(), fields[20], "{row}");
                    assert_eq!(data.ability2_id.to_string(), fields[21], "{row}");
                    assert_eq!(data.ability_hidden_id.to_string(), fields[22], "{row}");
                }
                "move" => {
                    move_count += 1;
                    let data = IMPORTED_MOVES
                        .iter()
                        .find(|record| record.id == fields[1])
                        .unwrap();
                    assert_eq!(data.numeric_id.to_string(), fields[2], "{row}");
                    assert_eq!(data.name, fields[3], "{row}");
                    assert_eq!(
                        format!("{:?}", data.element).to_uppercase(),
                        fields[4],
                        "{row}"
                    );
                    assert_eq!(
                        if data.is_status {
                            "STATUS"
                        } else if data.is_special {
                            "SPECIAL"
                        } else {
                            "PHYSICAL"
                        },
                        fields[5],
                        "{row}"
                    );
                    assert_eq!(data.power.to_string(), fields[6], "{row}");
                    assert_eq!(data.accuracy.to_string(), fields[7], "{row}");
                    assert_eq!(data.pp.to_string(), fields[8], "{row}");
                    assert_eq!(data.priority.to_string(), fields[9], "{row}");
                    assert_eq!(data.is_unimplemented, fields[10] == "1", "{row}");
                }
                "level_move" => {
                    level_move_count += 1;
                    let species_numeric_id: u16 = fields[2].parse().unwrap();
                    let level: u8 = fields[3].parse().unwrap();
                    let move_numeric_id: u16 = fields[5].parse().unwrap();
                    assert!(species_by_id(fields[1]).is_some(), "{row}");
                    assert!(move_by_id(fields[4]).is_some(), "{row}");
                    assert!(
                        level_moves_for(fields[1], level).any(|record| {
                            record.species_numeric_id == species_numeric_id
                                && record.level == level
                                && record.move_id == fields[4]
                                && record.move_numeric_id == move_numeric_id
                        }),
                        "{row}"
                    );
                }
                "species_tm" => {
                    species_tm_count += 1;
                    let expected = pokerogue_data::ImportedSpeciesTm {
                        species_numeric_id: fields[2].parse().unwrap(),
                        order: fields[3].parse().unwrap(),
                        move_numeric_id: fields[5].parse().unwrap(),
                    };
                    assert_eq!(
                        fields[1],
                        species_by_numeric_id(expected.species_numeric_id)
                            .unwrap()
                            .id
                    );
                    assert!(IMPORTED_SPECIES_TMS.contains(&expected), "{row}");
                    assert!(
                        move_by_numeric_id(expected.move_numeric_id).is_some(),
                        "{row}"
                    );
                }
                "prevolution" => {
                    prevolution_count += 1;
                    let expected = pokerogue_data::ImportedSpeciesPrevolution {
                        species_numeric_id: fields[1].parse().unwrap(),
                        prevolution_numeric_id: fields[2].parse().unwrap(),
                    };
                    assert!(IMPORTED_SPECIES_PREVOLUTION.contains(&expected), "{row}");
                }
                "tm_tier" => {
                    tm_tier_count += 1;
                    let expected = pokerogue_data::ImportedTmTier {
                        move_numeric_id: fields[2].parse().unwrap(),
                        tier_id: fields[4].parse().unwrap(),
                    };
                    assert_eq!(
                        move_by_id(fields[1]).unwrap().numeric_id,
                        expected.move_numeric_id
                    );
                    assert!(IMPORTED_TM_TIERS.contains(&expected), "{row}");
                }
                "pool_bucket" => {
                    pool_bucket_count += 1;
                    assert_eq!(fields[1], "TOWN", "{row}");
                    let tier_id: u8 = fields[3].parse().unwrap();
                    let time_id: i8 = fields[5].parse().unwrap();
                    let expected_count: usize = fields[6].parse().unwrap();
                    let bucket = TOWN_ENCOUNTER_POOLS
                        .iter()
                        .find(|pool| pool.tier == fields[2] && pool.time_of_day == fields[4])
                        .unwrap();
                    assert_eq!(bucket.tier_id, tier_id, "{row}");
                    assert_eq!(bucket.time_id, time_id, "{row}");
                    assert_eq!(bucket.species.len(), expected_count, "{row}");
                    let expected = if fields[7].is_empty() {
                        Vec::new()
                    } else {
                        fields[7]
                            .split('|')
                            .map(|entry| {
                                let (id, numeric_id) = entry.split_once(':').unwrap();
                                (numeric_id.parse::<u16>().unwrap(), id)
                            })
                            .collect()
                    };
                    assert_eq!(bucket.species, expected, "{row}");
                    assert!(
                        bucket.species.iter().all(|(numeric_id, id)| {
                            species_by_id(id).is_some_and(|record| record.numeric_id == *numeric_id)
                        }),
                        "{row}"
                    );
                }
                "tier_weight" => {
                    tier_weight_count += 1;
                    let boss_table = fields[1] == "BOSS";
                    let tier_id: u8 = fields[3].parse().unwrap();
                    let expected = ImportedTierWeight {
                        boss_table,
                        tier_id,
                        tier: fields[2],
                        roll_max: fields[4].parse().unwrap(),
                        min_roll: fields[5].parse().unwrap(),
                        max_roll: fields[6].parse().unwrap(),
                        weight: fields[7].parse().unwrap(),
                    };
                    assert!(TOWN_TIER_WEIGHTS.contains(&expected), "{row}");
                }
                "condition" => match fields[1] {
                    "boss_table" => {
                        assert_eq!(fields[2], TOWN_TIER_SELECTION_RULE.boss_table_condition)
                    }
                    "override_species" => {
                        assert_eq!(fields[2], TOWN_TIER_SELECTION_RULE.override_species_source);
                        assert_eq!(fields[3], "bypasses_tier=true");
                        assert!(TOWN_TIER_SELECTION_RULE.override_species_bypasses_tier);
                    }
                    "forced_tier" => {
                        assert_eq!(fields[2], TOWN_TIER_SELECTION_RULE.forced_tier_source);
                        assert_eq!(fields[3], "skips_roll=true");
                        assert!(TOWN_TIER_SELECTION_RULE.forced_tier_skips_roll);
                    }
                    "luck_modifier" => {
                        assert_eq!(fields[2], TOWN_TIER_SELECTION_RULE.luck_modifier_formula);
                        assert_eq!(fields[3], "roll=rollMax - luckModifier");
                        assert_eq!(
                            TOWN_TIER_SELECTION_RULE.effective_roll_max_formula,
                            "rollMax - luckModifier"
                        );
                    }
                    "time_pool" => {
                        assert_eq!(fields[2], "ALL+timeOfDay");
                        assert_eq!(fields[3], "all_first=true");
                        assert_eq!(TOWN_TIME_POOL_RULE.always_include_bucket, "ALL");
                        assert_eq!(TOWN_TIME_POOL_RULE.current_time_bucket, "timeOfDay");
                        assert!(TOWN_TIME_POOL_RULE.always_bucket_first);
                    }
                    condition => panic!("unknown imported condition {condition}"),
                },
                kind => panic!("unknown imported fixture kind {kind}"),
            }
        }
        assert_eq!(species_count, IMPORTED_SPECIES.len());
        assert_eq!(move_count, IMPORTED_MOVES.len());
        assert_eq!(level_move_count, IMPORTED_LEVEL_MOVES.len());
        assert_eq!(species_tm_count, IMPORTED_SPECIES_TMS.len());
        assert_eq!(prevolution_count, IMPORTED_SPECIES_PREVOLUTION.len());
        assert_eq!(tm_tier_count, IMPORTED_TM_TIERS.len());
        assert_eq!(pool_bucket_count, 9 * 5);
        assert_eq!(pool_bucket_count, TOWN_ENCOUNTER_POOLS.len());
        assert_eq!(tier_weight_count, 9);
        assert_eq!(tier_weight_count, TOWN_TIER_WEIGHTS.len());
        assert_eq!(TOWN_TIER_SELECTION_RULE.normal_roll_max, 512);
        assert_eq!(TOWN_TIER_SELECTION_RULE.boss_roll_max, 64);
        for boss_table in [false, true] {
            let weights: Vec<_> = TOWN_TIER_WEIGHTS
                .iter()
                .filter(|weight| weight.boss_table == boss_table)
                .collect();
            let roll_max = if boss_table { 64 } else { 512 };
            assert!(
                weights
                    .iter()
                    .all(|weight| weight.weight == weight.max_roll - weight.min_roll + 1)
            );
            assert_eq!(
                weights.iter().map(|weight| weight.weight).sum::<u16>(),
                roll_max
            );
        }
        let common_all = TOWN_ENCOUNTER_POOLS
            .iter()
            .find(|pool| pool.tier == "COMMON" && pool.time_of_day == "ALL")
            .unwrap();
        assert_eq!(TOWN_COMMON_ALL_POOL, common_all.species);
        assert_eq!(Species::Bulbasaur.base_speed(), 45);
        assert_eq!(Species::Charmander.types(), (Element::Fire, None));
        assert_eq!(Move::QuickAttack.name(), "Quick Attack");
    }

    #[test]
    fn starter_tm_compatibility_matches_executed_typescript_generator() {
        for row in include_str!("../fixtures/tm-compatibility.csv")
            .lines()
            .filter(|line| line.starts_with("compatible,"))
        {
            let fields: Vec<_> = row.split(',').collect();
            let known: Vec<u16> = fields[4].split('|').map(|id| id.parse().unwrap()).collect();
            let expected: Vec<u16> = if fields[5].is_empty() {
                Vec::new()
            } else {
                fields[5].split('|').map(|id| id.parse().unwrap()).collect()
            };
            let actual: Vec<_> = pokerogue_data::compatible_tms_for_species(
                fields[2].parse().unwrap(),
                &known,
                &[],
                fields[3].parse().unwrap(),
            )
            .into_iter()
            .map(|movement| movement.numeric_id)
            .collect();
            assert_eq!(actual, expected, "{row}");
        }
        let run = pokerogue_core::MiniRun::new("TmPartyFixture", "DAY").unwrap();
        for row in include_str!("../fixtures/tm-compatibility.csv")
            .lines()
            .filter(|line| line.starts_with("party,"))
        {
            let fields: Vec<_> = row.split(',').collect();
            let expected: Vec<u16> = fields[2].split('|').map(|id| id.parse().unwrap()).collect();
            let actual: Vec<u16> = pokerogue_core::rewards::compatible_tms_for_party(
                &run.battle.player,
                fields[1].parse().unwrap(),
            )
            .iter()
            .map(|id| move_by_id(id).unwrap().numeric_id)
            .collect();
            assert_eq!(actual, expected, "{row}");
        }
    }

    #[test]
    fn great_reward_pool_weights_match_executed_typescript_callbacks() {
        use pokerogue_core::rewards::{GreatRewardContext, GreatRewardPartyMember};
        let make_context = |key: &str| match key {
            "fresh-wave-1" => GreatRewardContext {
                classic: true,
                daily: false,
                spliced_only: false,
                endless: false,
                fresh_start_challenge: false,
                mini_black_hole_unlocked: false,
                wave: 1,
                ball_counts: [0; 5],
                lures: vec![],
                soothe_bell_boosted: false,
                fusions_boosted: false,
                party: vec![
                    GreatRewardPartyMember {
                        hp: 100,
                        max_hp: 100,
                        has_status: false,
                        status_protected: false,
                        holds_leppa: false,
                        move_pp: vec![],
                        has_learnable_level_moves: true,
                        tera_eligible: true,
                        is_fused: false,
                    };
                    2
                ],
            },
            "damaged-status-faint-wave-180" => GreatRewardContext {
                classic: true,
                daily: false,
                spliced_only: false,
                endless: false,
                fresh_start_challenge: false,
                mini_black_hole_unlocked: false,
                wave: 180,
                ball_counts: [0, 99, 0, 0, 0],
                lures: vec![(15, 9)],
                soothe_bell_boosted: true,
                fusions_boosted: false,
                party: vec![
                    GreatRewardPartyMember {
                        hp: 100,
                        max_hp: 300,
                        has_status: true,
                        status_protected: false,
                        holds_leppa: false,
                        move_pp: vec![(35, 30)],
                        has_learnable_level_moves: true,
                        tera_eligible: true,
                        is_fused: false,
                    },
                    GreatRewardPartyMember {
                        hp: 0,
                        max_hp: 200,
                        has_status: false,
                        status_protected: false,
                        holds_leppa: false,
                        move_pp: vec![(40, 0)],
                        has_learnable_level_moves: false,
                        tera_eligible: false,
                        is_fused: false,
                    },
                ],
            },
            "classic-last-wave" => GreatRewardContext {
                classic: true,
                daily: false,
                spliced_only: false,
                endless: false,
                fresh_start_challenge: false,
                mini_black_hole_unlocked: false,
                wave: 199,
                ball_counts: [0; 5],
                lures: vec![],
                soothe_bell_boosted: false,
                fusions_boosted: false,
                party: vec![
                    GreatRewardPartyMember {
                        hp: 50,
                        max_hp: 300,
                        has_status: false,
                        status_protected: false,
                        holds_leppa: false,
                        move_pp: vec![],
                        has_learnable_level_moves: true,
                        tera_eligible: true,
                        is_fused: false,
                    },
                    GreatRewardPartyMember {
                        hp: 200,
                        max_hp: 300,
                        has_status: false,
                        status_protected: false,
                        holds_leppa: false,
                        move_pp: vec![],
                        has_learnable_level_moves: true,
                        tera_eligible: true,
                        is_fused: false,
                    },
                ],
            },
            "mini-run-great-recovery-wave-19" => GreatRewardContext {
                classic: true,
                daily: false,
                spliced_only: false,
                endless: false,
                fresh_start_challenge: false,
                mini_black_hole_unlocked: false,
                wave: 19,
                ball_counts: [5, 0, 0, 0, 0],
                lures: vec![],
                soothe_bell_boosted: false,
                fusions_boosted: false,
                party: vec![
                    GreatRewardPartyMember {
                        hp: 18,
                        max_hp: 160,
                        has_status: false,
                        status_protected: false,
                        holds_leppa: false,
                        move_pp: vec![(25, 0), (35, 0)],
                        has_learnable_level_moves: true,
                        tera_eligible: true,
                        is_fused: false,
                    },
                    GreatRewardPartyMember {
                        hp: 0,
                        max_hp: 22,
                        has_status: false,
                        status_protected: false,
                        holds_leppa: false,
                        move_pp: vec![(25, 0), (35, 0)],
                        has_learnable_level_moves: true,
                        tera_eligible: true,
                        is_fused: false,
                    },
                ],
            },
            "mini-run-great-elixir-wave-19" => GreatRewardContext {
                classic: true,
                daily: false,
                spliced_only: false,
                endless: false,
                fresh_start_challenge: false,
                mini_black_hole_unlocked: false,
                wave: 19,
                ball_counts: [5, 0, 0, 0, 0],
                lures: vec![],
                soothe_bell_boosted: false,
                fusions_boosted: false,
                party: vec![
                    GreatRewardPartyMember {
                        hp: 18,
                        max_hp: 160,
                        has_status: false,
                        status_protected: false,
                        holds_leppa: false,
                        move_pp: vec![(25, 20), (35, 30)],
                        has_learnable_level_moves: true,
                        tera_eligible: true,
                        is_fused: false,
                    },
                    GreatRewardPartyMember {
                        hp: 0,
                        max_hp: 22,
                        has_status: false,
                        status_protected: false,
                        holds_leppa: false,
                        move_pp: vec![(25, 0), (35, 0)],
                        has_learnable_level_moves: true,
                        tera_eligible: true,
                        is_fused: false,
                    },
                ],
            },
            "mini-run-great-full-restore-wave-19" => GreatRewardContext {
                classic: true,
                daily: false,
                spliced_only: false,
                endless: false,
                fresh_start_challenge: false,
                mini_black_hole_unlocked: false,
                wave: 19,
                ball_counts: [5, 0, 0, 0, 0],
                lures: vec![],
                soothe_bell_boosted: false,
                fusions_boosted: false,
                party: (0..2)
                    .map(|_| GreatRewardPartyMember {
                        hp: 100,
                        max_hp: 300,
                        has_status: false,
                        status_protected: false,
                        holds_leppa: false,
                        move_pp: vec![(25, 0), (35, 0)],
                        has_learnable_level_moves: true,
                        tera_eligible: true,
                        is_fused: false,
                    })
                    .collect(),
            },
            "mini-run-healthy-wave-19" => GreatRewardContext {
                classic: true,
                daily: false,
                spliced_only: false,
                endless: false,
                fresh_start_challenge: false,
                mini_black_hole_unlocked: false,
                wave: 19,
                ball_counts: [5, 0, 0, 0, 0],
                lures: vec![],
                soothe_bell_boosted: false,
                fusions_boosted: false,
                party: (0..2)
                    .map(|_| GreatRewardPartyMember {
                        hp: 100,
                        max_hp: 100,
                        has_status: false,
                        status_protected: false,
                        holds_leppa: false,
                        move_pp: vec![(25, 0), (35, 0)],
                        has_learnable_level_moves: false,
                        tera_eligible: false,
                        is_fused: false,
                    })
                    .collect(),
            },
            "mini-run-live-amulet-wave-19" => GreatRewardContext {
                classic: true,
                daily: false,
                spliced_only: false,
                endless: false,
                fresh_start_challenge: false,
                mini_black_hole_unlocked: false,
                wave: 19,
                ball_counts: [5, 0, 0, 0, 0],
                lures: vec![],
                soothe_bell_boosted: false,
                fusions_boosted: false,
                party: (0..2)
                    .map(|_| GreatRewardPartyMember {
                        hp: 100,
                        max_hp: 100,
                        has_status: false,
                        status_protected: false,
                        holds_leppa: false,
                        move_pp: vec![(25, 0), (35, 0)],
                        has_learnable_level_moves: true,
                        tera_eligible: true,
                        is_fused: false,
                    })
                    .collect(),
            },
            _ => panic!("unknown source context {key}"),
        };

        let source_ids: Vec<_> = pokerogue_data::IMPORTED_PLAYER_MODIFIER_POOL
            .iter()
            .filter(|entry| entry.tier == 1)
            .map(|entry| entry.id)
            .collect();
        assert_eq!(source_ids.len(), 24);
        let expected_ids: Vec<_> = include_str!("../fixtures/great-rewards.csv")
            .lines()
            .find(|line| line.starts_with("great_ids,"))
            .unwrap()
            .split_once(',')
            .unwrap()
            .1
            .split('|')
            .collect();
        assert_eq!(source_ids, expected_ids);
        for row in include_str!("../fixtures/great-rewards.csv")
            .lines()
            .filter(|line| line.starts_with("great,"))
        {
            let fields: Vec<_> = row.split(',').collect();
            let context = make_context(fields[1]);
            let expected: Vec<u32> = fields[3]
                .split('|')
                .map(|weight| weight.parse().unwrap())
                .collect();
            assert_eq!(
                pokerogue_core::rewards::great_pool_weights(&context, 0).to_vec(),
                expected,
                "{row}"
            );
        }
    }

    #[test]
    fn rogue_and_master_reward_weights_match_executed_typescript_callbacks() {
        use pokerogue_core::rewards::{GreatRewardContext, GreatRewardPartyMember};

        let member = GreatRewardPartyMember {
            hp: 100,
            max_hp: 100,
            has_status: false,
            status_protected: false,
            holds_leppa: false,
            move_pp: vec![],
            has_learnable_level_moves: false,
            tera_eligible: false,
            is_fused: false,
        };
        let context_for = |key: &str| {
            let (classic, endless, wave, balls, fusions, unlocked) = match key {
                "classic-wave-1" => (true, false, 1, [0; 5], false, false),
                "classic-wave-199" => (true, false, 199, [0, 0, 0, 99, 99], true, false),
                "endless-wave-100" => (false, true, 100, [0; 5], false, true),
                _ => panic!("unknown source context {key}"),
            };
            GreatRewardContext {
                classic,
                daily: false,
                spliced_only: false,
                endless,
                fresh_start_challenge: false,
                mini_black_hole_unlocked: unlocked,
                wave,
                ball_counts: balls,
                lures: vec![],
                soothe_bell_boosted: false,
                fusions_boosted: fusions,
                party: vec![member.clone(), member.clone()],
            }
        };

        for (tier, prefix, weights) in [
            (3_u8, "rogue", "rogue_ids,"),
            (4_u8, "master", "master_ids,"),
        ] {
            let actual_ids: Vec<_> = pokerogue_data::IMPORTED_PLAYER_MODIFIER_POOL
                .iter()
                .filter(|entry| entry.tier == tier)
                .map(|entry| entry.id)
                .collect();
            let expected_ids: Vec<_> = include_str!("../fixtures/rogue-master-rewards.csv")
                .lines()
                .find(|line| line.starts_with(prefix) && line.starts_with(weights))
                .unwrap()
                .split_once(',')
                .unwrap()
                .1
                .split('|')
                .collect();
            assert_eq!(actual_ids, expected_ids);
        }

        for row in include_str!("../fixtures/rogue-master-rewards.csv")
            .lines()
            .filter(|line| line.starts_with("rogue,") || line.starts_with("master,"))
        {
            let fields: Vec<_> = row.split(',').collect();
            let context = context_for(fields[1]);
            let expected: Vec<u32> = fields[3]
                .split('|')
                .map(|weight| weight.parse().unwrap())
                .collect();
            let actual = match fields[0] {
                "rogue" => pokerogue_core::rewards::rogue_pool_weights(&context, 0).to_vec(),
                "master" => pokerogue_core::rewards::master_pool_weights(&context, 0).to_vec(),
                _ => unreachable!(),
            };
            assert_eq!(actual, expected, "{row}");
        }
    }

    #[test]
    fn ultra_reward_weights_match_executed_typescript_callbacks() {
        use pokerogue_core::rewards::UltraRewardContext;

        let context_for = |key: &str| match key {
            "classic-wave-1-eligible" => UltraRewardContext {
                classic: true,
                daily: false,
                fresh_start_challenge: false,
                eviolite_unlocked: true,
                wave: 1,
                ultra_ball_count: 0,
                max_lure_battles_remaining: None,
                eviolite_eligible: true,
                leek_eligible: true,
                toxic_orb_eligible: false,
                flame_orb_eligible: false,
                mystical_rock_eligible: false,
            },
            "classic-wave-1-ultra-balls-capped" => UltraRewardContext {
                classic: true,
                daily: false,
                fresh_start_challenge: false,
                eviolite_unlocked: true,
                wave: 1,
                ultra_ball_count: 99,
                max_lure_battles_remaining: None,
                eviolite_eligible: true,
                leek_eligible: true,
                toxic_orb_eligible: false,
                flame_orb_eligible: false,
                mystical_rock_eligible: false,
            },
            "classic-wave-199-capped" => UltraRewardContext {
                classic: true,
                daily: false,
                fresh_start_challenge: false,
                eviolite_unlocked: true,
                wave: 199,
                ultra_ball_count: 99,
                max_lure_battles_remaining: Some(18),
                eviolite_eligible: true,
                leek_eligible: true,
                toxic_orb_eligible: false,
                flame_orb_eligible: false,
                mystical_rock_eligible: false,
            },
            "endless-wave-100-locked-challenge" => UltraRewardContext {
                classic: false,
                daily: false,
                fresh_start_challenge: true,
                eviolite_unlocked: false,
                wave: 100,
                ultra_ball_count: 0,
                max_lure_battles_remaining: Some(0),
                eviolite_eligible: true,
                leek_eligible: true,
                toxic_orb_eligible: false,
                flame_orb_eligible: false,
                mystical_rock_eligible: false,
            },
            "mini-run-wave-19" => UltraRewardContext {
                classic: true,
                daily: false,
                fresh_start_challenge: false,
                eviolite_unlocked: false,
                wave: 19,
                ultra_ball_count: 0,
                max_lure_battles_remaining: None,
                eviolite_eligible: false,
                leek_eligible: false,
                toxic_orb_eligible: false,
                flame_orb_eligible: false,
                mystical_rock_eligible: false,
            },
            _ => panic!("unknown source context {key}"),
        };

        let actual_ids: Vec<_> = pokerogue_data::IMPORTED_PLAYER_MODIFIER_POOL
            .iter()
            .filter(|entry| entry.tier == 2)
            .map(|entry| entry.id)
            .collect();
        let expected_ids: Vec<_> = include_str!("../fixtures/ultra-rewards.csv")
            .lines()
            .find(|line| line.starts_with("ultra_ids,"))
            .unwrap()
            .split_once(',')
            .unwrap()
            .1
            .split('|')
            .collect();
        assert_eq!(actual_ids, expected_ids);

        for row in include_str!("../fixtures/ultra-rewards.csv")
            .lines()
            .filter(|line| line.starts_with("ultra,"))
        {
            let fields: Vec<_> = row.split(',').collect();
            let expected: Vec<u32> = fields[3]
                .split('|')
                .map(|weight| weight.parse().unwrap())
                .collect();
            assert_eq!(
                pokerogue_core::rewards::ultra_pool_weights(&context_for(fields[1])).to_vec(),
                expected,
                "{row}"
            );
        }
        let mini_row = include_str!("../fixtures/ultra-rewards.csv")
            .lines()
            .find(|row| row.starts_with("ultra,mini-run-wave-19,"))
            .unwrap();
        let mut mini_run = pokerogue_core::MiniRun::new("UltraLiveWeights", "DAY").unwrap();
        mini_run.wave = 19;
        let source_weights: Vec<u32> = mini_row
            .split(',')
            .nth(3)
            .unwrap()
            .split('|')
            .map(|weight| weight.parse().unwrap())
            .collect();
        assert_eq!(
            mini_run.modeled_player_modifier_tier_weights(2).unwrap(),
            source_weights
        );
    }

    #[test]
    fn reward_stack_gate_filters_imported_pools_without_shifting_indexes() {
        use pokerogue_core::rewards::{ExistingModifierStack, filter_modifier_pool_stack_caps};

        let cases = [
            ExistingModifierStack {
                id: "POKEBALL",
                stack: 99,
                cap: 99,
                is_held_item: false,
                is_form_change_item: false,
            },
            ExistingModifierStack {
                id: "RARE_CANDY",
                stack: 2,
                cap: 99,
                is_held_item: false,
                is_form_change_item: false,
            },
            ExistingModifierStack {
                id: "SOOTHE_BELL",
                stack: 99,
                cap: 99,
                is_held_item: true,
                is_form_change_item: false,
            },
            ExistingModifierStack {
                id: "FORM_CHANGE_ITEM",
                stack: 99,
                cap: 99,
                is_held_item: false,
                is_form_change_item: true,
            },
        ];

        for tier in 0..=4 {
            let raw = vec![
                1;
                pokerogue_data::IMPORTED_PLAYER_MODIFIER_POOL
                    .iter()
                    .filter(|entry| entry.tier == tier)
                    .count()
            ];
            let filtered = filter_modifier_pool_stack_caps(tier, &raw, &cases).unwrap();
            assert_eq!(filtered.len(), raw.len());
            let ids: Vec<_> = pokerogue_data::IMPORTED_PLAYER_MODIFIER_POOL
                .iter()
                .filter(|entry| entry.tier == tier)
                .map(|entry| entry.id)
                .collect();
            for (index, id) in ids.iter().enumerate() {
                let expected = match *id {
                    "POKEBALL" => 0,
                    "RARE_CANDY" | "SOOTHE_BELL" | "FORM_CHANGE_ITEM" => 1,
                    _ => 1,
                };
                assert_eq!(filtered[index], expected, "tier {tier}, {id}");
            }
        }
        assert!(filter_modifier_pool_stack_caps(0, &[1], &cases).is_none());
    }

    #[test]
    fn rust_chart_matches_all_original_typescript_chart_entries() {
        let mut count = 0;
        for row in include_str!("../fixtures/type-chart.csv").lines() {
            let mut fields = row.split(',');
            let attack = element(fields.next().unwrap());
            let defense = element(fields.next().unwrap());
            let expected: f32 = fields.next().unwrap().parse().unwrap();
            assert_eq!(
                single_type_effectiveness(attack, defense),
                expected,
                "{row}"
            );
            count += 1;
        }
        assert_eq!(count, 18 * 18);
    }

    #[test]
    fn type_immunity_and_dual_type_multipliers_match_original_chart() {
        assert_eq!(type_effectiveness(Element::Normal, &[Element::Ghost]), 0.0);
        assert_eq!(
            type_effectiveness(Element::Fire, &[Element::Grass, Element::Poison]),
            2.0
        );
        assert_eq!(
            type_effectiveness(Element::Grass, &[Element::Grass, Element::Poison]),
            0.25
        );
        assert_eq!(type_effectiveness(Element::Grass, &[Element::Fire]), 0.5);
    }

    #[test]
    fn deterministic_damage_matches_source_derived_typescript_fixtures() {
        let rows: Vec<_> = include_str!("../fixtures/damage.csv").lines().collect();
        for row in rows {
            let fields: Vec<_> = row.split(',').collect();
            let expected: i32 = fields[1].parse().unwrap();
            let expected_stab: f64 = fields[3].parse().unwrap();
            let expected_effectiveness: f64 = fields[4].parse().unwrap();
            let (attacker, defender, move_) = match fields[0] {
                "vine-whip-vs-fire" => (
                    Battler::demo(Species::Bulbasaur, 5, [Move::VineWhip, Move::Tackle]),
                    Battler::demo(Species::Charmander, 5, [Move::Ember, Move::Tackle]),
                    Move::VineWhip,
                ),
                "ember-vs-grass-poison" => (
                    Battler::demo(Species::Charmander, 5, [Move::Ember, Move::Tackle]),
                    Battler::demo(Species::Bulbasaur, 5, [Move::VineWhip, Move::Tackle]),
                    Move::Ember,
                ),
                "original-tackle-damage-test" => {
                    let mut attacker =
                        Battler::demo(Species::Charmander, 100, [Move::Tackle, Move::Ember]);
                    attacker.attack = 80;
                    let mut defender =
                        Battler::demo(Species::Bulbasaur, 100, [Move::VineWhip, Move::Tackle]);
                    defender.defense = 90;
                    (attacker, defender, Move::Tackle)
                }
                name => panic!("unknown damage fixture {name}"),
            };
            let result = calculate_damage(&attacker, &defender, move_, 100);
            assert_eq!(result.damage, expected, "{row}");
            assert_eq!(result.stab_multiplier, expected_stab, "{row}");
            assert_eq!(result.type_multiplier, expected_effectiveness, "{row}");
        }
    }

    #[test]
    fn type_immunity_deals_zero_damage() {
        let normal = Battler::demo(Species::Charmander, 5, [Move::Tackle, Move::Ember]);
        let ghost_target = Battler::demo(Species::Bulbasaur, 5, [Move::VineWhip, Move::Tackle]);
        assert_eq!(
            type_effectiveness(Move::Tackle.element(), &[Element::Ghost]),
            0.0
        );
        let result = calculate_damage_for_types(
            &normal,
            &ghost_target,
            Move::Tackle,
            100,
            &[Element::Ghost],
        );
        assert_eq!(result.damage, 0);
        // An immunity returns before the source's one-damage minimum is applied.
        assert_eq!(
            single_type_effectiveness(Element::Normal, Element::Ghost),
            0.0
        );
    }

    #[test]
    fn player_can_reach_a_win_state() {
        let mut battle = Battle::demo();
        battle.player.speed = 100;
        battle.enemy.hp = 1;
        battle.choose_move(0);
        assert_eq!(battle.outcome, Some(Outcome::PlayerWin));
    }

    #[test]
    fn demo_battle_shows_dual_type_super_effectiveness() {
        let mut battle = Battle::demo();
        battle.player.speed = 100;
        battle.choose_move(0);
        assert!(battle.log.contains("Super effective ×2"));
    }

    #[test]
    fn rng_stream_matches_phaser_421_battle_seed_fixture() {
        let mut rng = BattleRng::new("ParitySeed000000");
        for line in include_str!("../fixtures/phaser-rng.csv").lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let values: Vec<u32> = line
                .split(',')
                .map(|value| value.parse().unwrap())
                .collect();
            let actual = rng.rand_int(values[0], values[1], values[2] as i32);
            assert_eq!(actual, values[3] as i32, "{line}");
        }
    }

    #[test]
    fn capture_rate_inputs_match_imported_pinned_species_data() {
        let rows: Vec<_> = include_str!("../fixtures/imported-demo-data.csv")
            .lines()
            .filter(|row| row.starts_with("species,"))
            .collect();
        assert_eq!(rows.len(), IMPORTED_SPECIES.len());
        for row in rows {
            let fields: Vec<_> = row.split(',').collect();
            let data = IMPORTED_SPECIES
                .iter()
                .find(|record| record.id == fields[1])
                .unwrap();
            assert_eq!(data.catch_rate.to_string(), fields[14], "{row}");
            assert_eq!(data.base_friendship.to_string(), fields[15], "{row}");
        }
        assert_eq!(Species::Bulbasaur.catch_rate().to_string(), "45");
    }

    #[test]
    fn ordinary_capture_attempt_matches_source_formula_and_rng_trace() {
        use pokerogue_core::{
            BattleRng, INITIAL_POKEBALL_COUNT, MiniRun, Pokeball, attempt_capture,
        };

        for row in include_str!("../fixtures/capture-attempt.csv").lines() {
            if row.is_empty() || row.starts_with('#') {
                continue;
            }
            let fields: Vec<_> = row.split(',').collect();
            assert_eq!(fields[1], "7c0881255af5cd51992e513ff93c2f71adbe40e0");
            if fields[0] == "ball_multiplier" {
                let ball = match fields[2].parse::<u8>().unwrap() {
                    0 => Pokeball::Poke,
                    1 => Pokeball::Great,
                    2 => Pokeball::Ultra,
                    3 => Pokeball::Rogue,
                    4 => Pokeball::Master,
                    5 => Pokeball::Luxury,
                    id => panic!("unsupported fixture ball {id}"),
                };
                assert_eq!(ball.catch_multiplier().to_string(), fields[3]);
                continue;
            }
            if fields[0] == "initial_balls" {
                assert_eq!(INITIAL_POKEBALL_COUNT.to_string(), fields[2]);
                assert_eq!(
                    MiniRun::new("InitialBallsSeed", "DAY")
                        .unwrap()
                        .pokeballs
                        .to_string(),
                    fields[2]
                );
                continue;
            }
            let species_id: u16 = fields[3].parse().unwrap();
            let species = Species::from_numeric_id(species_id).unwrap();
            let ball = match fields[7].parse::<u8>().unwrap() {
                0 => Pokeball::Poke,
                1 => Pokeball::Great,
                2 => Pokeball::Ultra,
                3 => Pokeball::Rogue,
                4 => Pokeball::Master,
                5 => Pokeball::Luxury,
                id => panic!("unsupported fixture ball {id}"),
            };
            let mut rng = BattleRng::new(fields[2]);
            let attempt = attempt_capture(
                &mut rng,
                1,
                species.catch_rate(),
                fields[5].parse().unwrap(),
                fields[6].parse().unwrap(),
                ball,
                fields[9].parse().unwrap(),
                fields[10].parse().unwrap(),
                fields[11].parse().unwrap(),
            )
            .unwrap();
            assert_eq!(species.catch_rate().to_string(), fields[4]);
            assert_eq!(ball.catch_multiplier().to_string(), fields[8]);
            assert_eq!(attempt.modified_catch_rate.to_string(), fields[12]);
            assert_eq!(attempt.shake_probability.to_string(), fields[13]);
            assert_eq!(
                attempt.critical.to_string(),
                if fields[14] == "1" { "true" } else { "false" }
            );
            assert_eq!(
                attempt
                    .shake_rolls
                    .iter()
                    .map(i32::to_string)
                    .collect::<Vec<_>>()
                    .join("|"),
                fields[15]
            );
            assert_eq!(
                attempt.caught.to_string(),
                if fields[16] == "1" { "true" } else { "false" }
            );
            let expected_draws: Vec<Vec<i32>> = fields[17]
                .split('|')
                .map(|draw| {
                    draw.split(':')
                        .skip(1)
                        .map(|value| value.parse().unwrap())
                        .collect()
                })
                .collect();
            assert_eq!(rng.draws().len(), expected_draws.len());
            for (actual, expected) in rng.draws().iter().zip(expected_draws) {
                assert_eq!(
                    (
                        actual.turn as i32,
                        actual.range as i32,
                        actual.min,
                        actual.value
                    ),
                    (1, expected[0], expected[1], expected[2])
                );
            }
        }
    }

    #[test]
    fn successful_wild_capture_uses_ball_adds_party_member_and_survives_save() {
        use pokerogue_core::{MiniRun, MiniRunStage};

        let mut run = MiniRun::new("CaptureOracleSeed00004", "DAY").unwrap();
        let target = Battler::demo(Species::Bulbasaur, 18, [Move::Tackle, Move::QuickAttack]);
        assert_eq!(target.max_hp, 44);
        run.battle.enemy = target;
        run.battle.enemy.hp = 22;
        assert_eq!(run.pokeballs, 5);

        run.take_action(PlayerAction::ThrowPokeball).unwrap();
        assert_eq!(run.stage, MiniRunStage::Reward);
        assert_eq!(run.pokeballs, 4);
        assert_eq!(run.battle.player.len(), 3);
        let caught = run.battle.player.member(2).unwrap();
        assert_eq!(caught.species, Species::Bulbasaur);
        assert_eq!((caught.hp, caught.max_hp, caught.level), (22, 44, 18));
        assert_eq!(run.battle.outcome, Some(Outcome::PlayerWin));
        assert!(run.battle.last_capture_attempt.as_ref().unwrap().caught);

        let save = run.checkpoint().unwrap();
        assert!(save.starts_with("POKEROGUE_MINIRUN\t27\n"));
        let mut resumed = MiniRun::from_checkpoint(&save).unwrap();
        assert_eq!(resumed.pokeballs, 4);
        assert_eq!(resumed.battle.player.len(), 3);
        assert_eq!(resumed.battle.player.member(2).unwrap().hp, 22);
        resumed.continue_after_reward().unwrap();
        assert_eq!(resumed.wave, 2);
        assert_eq!(resumed.battle.player.len(), 3);
    }

    #[test]
    fn failed_capture_consumes_turn_then_foe_acts_and_empty_inventory_is_noop() {
        let mut battle = Battle::demo_with_seed("CaptureOracleSeed00000");
        battle.enemy = Battler::demo(Species::Bulbasaur, 18, [Move::Tackle, Move::QuickAttack]);
        battle.enemy.hp = 22;
        battle.player.hp = 500;
        battle.player.max_hp = 500;
        let player_hp = battle.player.hp;
        battle.take_turn(PlayerAction::ThrowPokeball).unwrap();
        assert!(!battle.last_capture_attempt.as_ref().unwrap().caught);
        assert_eq!(battle.player.len(), 2);
        assert_eq!(battle.turn, 2);
        assert_eq!(
            battle.last_action_order,
            [BattleActor::Player, BattleActor::Enemy]
        );
        assert!(battle.last_events.iter().any(|event| matches!(
            event,
            BattleEvent::MoveExecuted {
                actor: BattleActor::Enemy,
                ..
            }
        )));
        assert!(battle.player.hp <= player_hp);

        let mut run = pokerogue_core::MiniRun::new("NoBallsSeed0001", "DAY").unwrap();
        run.pokeballs = 0;
        let turn = run.battle.turn;
        let enemy_hp = run.battle.enemy.hp;
        assert_eq!(
            run.take_action(PlayerAction::ThrowPokeball),
            Err(pokerogue_core::MiniRunError::NoPokeballs)
        );
        assert_eq!(run.battle.turn, turn);
        assert_eq!(run.battle.enemy.hp, enemy_hp);

        let mut full = pokerogue_core::MiniRun::new("FullPartyCaptureSeed", "DAY").unwrap();
        for species_id in [10, 19, 506, 819] {
            let species = Species::from_numeric_id(species_id).unwrap();
            full.battle
                .add_player_member(Battler::demo(species, 5, [Move::Tackle, Move::QuickAttack]))
                .unwrap();
        }
        let draws = full.battle.rng_draws().len();
        let enemy_hp = full.battle.enemy.hp;
        assert_eq!(full.battle.player.len(), 6);
        assert_eq!(
            full.take_action(PlayerAction::ThrowPokeball),
            Err(pokerogue_core::MiniRunError::Turn(
                pokerogue_core::TurnActionError::PartyFull
            ))
        );
        assert_eq!(full.pokeballs, pokerogue_core::INITIAL_POKEBALL_COUNT);
        assert_eq!(full.battle.enemy.hp, enemy_hp);
        assert_eq!(full.battle.rng_draws().len(), draws);
    }

    #[test]
    fn accuracy_rolls_hit_miss_and_guaranteed_moves_skip_rng() {
        let mut hit_rng = BattleRng::new("ParitySeed000000");
        assert_eq!(
            check_move_accuracy(100, 0, 2, 1, &mut hit_rng),
            HitResult::Hit
        );

        let mut miss_rng = BattleRng::new("ParitySeed000000");
        assert_eq!(
            check_move_accuracy(100, 0, 3, 1, &mut miss_rng),
            HitResult::Miss
        );

        let mut guaranteed_rng = BattleRng::new("ParitySeed000000");
        assert_eq!(
            check_move_accuracy(-1, 0, 6, 1, &mut guaranteed_rng),
            HitResult::Hit
        );
        assert_eq!(
            guaranteed_rng.rand_int(1, 100, 0),
            51,
            "guaranteed accuracy must consume no RNG"
        );
    }

    #[test]
    fn battle_accuracy_miss_skips_damage_rng_and_damage() {
        let mut battle = Battle::demo_with_seed("ParitySeed000000");
        battle.enemy.set_stat_stage(BattleStat::Evasion, 6);
        let enemy_hp = battle.enemy.hp;
        battle.choose_move(0);
        assert!(battle.log.contains("Bulbasaur used Vine Whip! It missed!"));
        assert_eq!(battle.enemy.hp, enemy_hp);
    }

    #[test]
    fn successful_demo_turn_consumes_accuracy_then_damage_draws_in_speed_order() {
        let mut battle = Battle::demo_with_seed("ParitySeed000000");
        let player_hp = battle.player.hp;
        let enemy_hp = battle.enemy.hp;
        let initial_player = battle.player.clone();
        let initial_enemy = battle.enemy.clone();
        battle.choose_move(0);
        assert_eq!(
            battle.last_enemy_decision.as_ref().unwrap().selected_move,
            Move::Ember
        );
        assert_eq!(
            battle.last_enemy_decision.as_ref().unwrap().random_rolls,
            [4]
        );
        let damage_rolls = phaser_fixture_values(1, 16, 85);
        assert_eq!(damage_rolls, [99, 94]);
        let enemy_damage = calculate_damage(
            &initial_enemy,
            &initial_player,
            Move::Ember,
            damage_rolls[0] as u8,
        )
        .damage;
        let player_damage = calculate_damage(
            &initial_player,
            &initial_enemy,
            Move::VineWhip,
            damage_rolls[1] as u8,
        )
        .damage;
        assert_eq!(player_hp - battle.player.hp, enemy_damage);
        assert_eq!(enemy_hp - battle.enemy.hp, player_damage);
    }

    #[test]
    fn accuracy_stage_clamp_matches_executed_typescript_setter() {
        let mut battler = Battler::demo(Species::Charmander, 5, [Move::Ember, Move::Tackle]);
        for row in include_str!("../fixtures/stat-stages.csv").lines() {
            if row.is_empty() || row.starts_with('#') {
                continue;
            }
            let (input, expected) = row.split_once(',').unwrap();
            battler.set_stat_stage(BattleStat::Accuracy, input.parse().unwrap());
            assert_eq!(
                battler.stat_stage(BattleStat::Accuracy),
                expected.parse::<i8>().unwrap(),
                "{row}"
            );
        }
    }

    #[test]
    fn stat_stages_clamp_at_both_ends_and_persist_between_turns() {
        let mut battler = Battler::demo(Species::Charmander, 5, [Move::Ember, Move::Tackle]);
        assert_eq!(battler.change_stat_stage(BattleStat::Accuracy, -8), -6);
        assert_eq!(battler.change_stat_stage(BattleStat::Accuracy, -1), 0);
        assert_eq!(battler.change_stat_stage(BattleStat::Accuracy, 20), 12);
        assert_eq!(battler.stat_stage(BattleStat::Accuracy), 6);
        battler.reset_stat_stages();
        assert_eq!(battler.stat_stage(BattleStat::Accuracy), 0);
    }

    #[test]
    fn standard_switch_faint_and_return_clear_stages_at_leave_field() {
        for exit in [
            FieldExitKind::Switch,
            FieldExitKind::Faint,
            FieldExitKind::Return,
        ] {
            let mut battler = Battler::demo(Species::Bulbasaur, 5, [Move::VineWhip, Move::Tackle]);
            battler.set_stat_stage(BattleStat::Accuracy, -4);
            battler.set_stat_stage(BattleStat::Evasion, 2);
            assert_eq!(battler.leave_field(exit), None, "{exit:?}");
            assert_eq!(battler.stat_stage(BattleStat::Accuracy), 0, "{exit:?}");
            assert_eq!(battler.stat_stage(BattleStat::Evasion), 0, "{exit:?}");
        }
    }

    #[test]
    fn deferred_switch_kinds_clear_stages_only_when_exit_finishes() {
        for exit in [
            FieldExitKind::InitialSwitch,
            FieldExitKind::ForceSwitch,
            FieldExitKind::ShedTail,
        ] {
            let mut battler = Battler::demo(Species::Bulbasaur, 5, [Move::VineWhip, Move::Tackle]);
            battler.set_stat_stage(BattleStat::Accuracy, -4);
            assert_eq!(battler.leave_field(exit), None);
            assert_eq!(battler.stat_stage(BattleStat::Accuracy), -4);
            battler.finish_field_exit();
            assert_eq!(battler.stat_stage(BattleStat::Accuracy), 0);
        }
    }

    #[test]
    fn baton_pass_transfers_modeled_stages_and_clears_outgoing_battler() {
        let mut outgoing = Battler::demo(Species::Bulbasaur, 5, [Move::VineWhip, Move::Tackle]);
        let mut incoming = Battler::demo(Species::Charmander, 5, [Move::Ember, Move::Tackle]);
        outgoing.set_stat_stage(BattleStat::Accuracy, -3);
        outgoing.set_stat_stage(BattleStat::Evasion, 5);
        incoming.set_stat_stage(BattleStat::Evasion, -2);

        let passed = outgoing.leave_field(FieldExitKind::BatonPass).unwrap();
        incoming.receive_passed_stages(passed);

        assert_eq!(outgoing.stat_stage(BattleStat::Accuracy), -3);
        assert_eq!(outgoing.stat_stage(BattleStat::Evasion), 5);
        assert_eq!(incoming.stat_stage(BattleStat::Accuracy), -3);
        assert_eq!(incoming.stat_stage(BattleStat::Evasion), 5);
        outgoing.finish_field_exit();
        assert_eq!(outgoing.stat_stage(BattleStat::Accuracy), 0);
        assert_eq!(outgoing.stat_stage(BattleStat::Evasion), 0);
    }

    #[test]
    fn voluntary_switch_orders_events_and_preserves_pokemon_data() {
        let mut battle = Battle::demo();
        let outgoing_hp = battle.player.hp - 4;
        battle.player.hp -= 4;
        battle.player.set_stat_stage(BattleStat::Accuracy, -3);
        let reserve = battle.player.member_mut(1).unwrap();
        reserve.hp -= 5;
        reserve.attack += 7;
        reserve.set_stat_stage(BattleStat::Evasion, 3);
        let expected_reserve_hp = reserve.hp;
        let expected_reserve_attack = reserve.attack;
        let events = battle.switch_player(1);
        assert_eq!(
            events,
            Ok([
                SwitchEvent::OutgoingLeftField { slot: 0 },
                SwitchEvent::OutgoingStagesReset { slot: 0 },
                SwitchEvent::IncomingStagesReset { slot: 1 },
                SwitchEvent::IncomingEnteredField { slot: 1 },
                SwitchEvent::SwitchCompleted {
                    outgoing: 0,
                    active: 1
                },
            ])
        );
        assert_eq!(battle.player.active_slot(), 1);
        assert_eq!(battle.player.species, Species::Charmander);
        assert_eq!(battle.player.stat_stage(BattleStat::Accuracy), 0);
        assert_eq!(battle.player.stat_stage(BattleStat::Evasion), 0);
        assert_eq!(battle.player.member(0).unwrap().hp, outgoing_hp);
        assert_eq!(battle.player.reserve().stat_stage(BattleStat::Accuracy), 0);
        assert_eq!(battle.player.reserve().stat_stage(BattleStat::Evasion), 0);
        assert_eq!(battle.player.hp, expected_reserve_hp);
        assert_eq!(battle.player.attack, expected_reserve_attack);
        assert_eq!(
            battle.player.moves,
            [Move::Ember, Move::Tackle, Move::Tackle, Move::Tackle]
        );
    }

    #[test]
    fn invalid_switch_slots_and_fainted_reserve_are_rejected_without_mutation() {
        let mut battle = Battle::demo();
        let active_hp = battle.player.hp;
        assert_eq!(battle.switch_player(2), Err(SwitchError::SlotOutOfRange));
        assert_eq!(battle.switch_player(0), Err(SwitchError::AlreadyActive));
        assert_eq!(battle.player.active_slot(), 0);
        assert_eq!(battle.player.hp, active_hp);
        assert_eq!(
            battle.take_turn(PlayerAction::Switch(2)),
            Err(TurnActionError::Switch(SwitchError::SlotOutOfRange))
        );
        assert_eq!(battle.turn, 1);
        assert_eq!(battle.enemy.hp, battle.enemy.max_hp);

        battle.player.member_mut(1).unwrap().hp = 0;
        assert_eq!(battle.switch_player(1), Err(SwitchError::Fainted));
        assert_eq!(battle.player.active_slot(), 0);
        assert_eq!(battle.player.hp, active_hp);
    }

    #[test]
    fn switching_out_and_back_does_not_advance_turn_or_consume_rng() {
        let mut control = Battle::demo();
        let mut switched = control.clone();
        let turn = switched.turn;
        switched.switch_player(1).unwrap();
        switched.switch_player(0).unwrap();
        assert_eq!(switched.turn, turn);
        control.choose_move(0);
        switched.choose_move(0);
        assert_eq!(switched.turn, control.turn);
        assert_eq!(switched.player.hp, control.player.hp);
        assert_eq!(switched.enemy.hp, control.enemy.hp);
        assert_eq!(switched.log, control.log);
    }

    #[test]
    fn switch_turn_matches_original_command_priority_and_enemy_rng_fixture() {
        let oracle = include_str!("../fixtures/command-order.csv")
            .lines()
            .find(|line| line.starts_with("player-switch-vs-enemy-fight,"))
            .unwrap();
        assert_eq!(
            oracle,
            "player-switch-vs-enemy-fight,POKEMON,FIGHT,player>enemy"
        );
        let mut battle = Battle::demo_with_seed("ParitySeed000000");
        let outgoing_hp = battle.player.hp;
        let incoming_before = battle.player.member(1).unwrap().clone();
        let expected_accuracy = phaser_fixture_values(1, 100, 0)[0];
        let expected_damage_roll = phaser_fixture_values(1, 16, 85)[0] as u8;

        let result = battle.take_turn(PlayerAction::Switch(1));
        assert!(matches!(result, Ok(Some(_))));
        assert_eq!(battle.turn, 2);
        assert_eq!(battle.player.member(0).unwrap().hp, outgoing_hp);
        let expected_damage = calculate_damage(
            &battle.enemy,
            &incoming_before,
            Move::Ember,
            expected_damage_roll,
        )
        .damage;
        assert!(expected_accuracy < 100);
        assert_eq!(incoming_before.hp - battle.player.hp, expected_damage);
        assert!(battle.log.find("come back!").unwrap() < battle.log.find("used Ember!").unwrap());
        assert_eq!(
            battle.last_action_order,
            [BattleActor::Player, BattleActor::Enemy]
        );
        assert_eq!(battle.last_speed_shuffle_roll, None);
    }

    #[test]
    fn fight_order_matches_typescript_priority_speed_stages_and_tie_shuffle() {
        for row in include_str!("../fixtures/action-order.csv").lines() {
            if row.is_empty() || row.starts_with('#') {
                continue;
            }
            let fields: Vec<_> = row.split(',').collect();
            let seed = if fields[0] == "equal-speed-priority-reversed-tie" {
                "OrderTieSeed001"
            } else {
                "OrderParitySeed"
            };
            let mut battle = Battle::demo_with_seed(seed);
            let (player_speed, enemy_speed, player_move, player_stage, enemy_stage) =
                match fields[0] {
                    "faster-player" => (12, 9, Move::VineWhip, 0, 0),
                    "slower-player" => (7, 11, Move::VineWhip, 0, 0),
                    "priority-over-speed" => (7, 11, Move::QuickAttack, 0, 0),
                    "speed-stages" => (9, 11, Move::VineWhip, 1, -1),
                    "equal-speed-priority" | "equal-speed-priority-reversed-tie" => {
                        (10, 10, Move::VineWhip, 0, 0)
                    }
                    other => panic!("unknown order fixture case: {other}"),
                };
            battle.player.speed = player_speed;
            battle.enemy.speed = enemy_speed;
            battle
                .player
                .set_stat_stage(BattleStat::Speed, player_stage);
            battle.enemy.set_stat_stage(BattleStat::Speed, enemy_stage);
            battle.player.moves[0] = player_move;
            battle.take_turn(PlayerAction::UseMove(0)).unwrap();

            let actual_order: Vec<_> = battle
                .last_action_order
                .iter()
                .map(|actor| match actor {
                    BattleActor::Player => "player",
                    BattleActor::Enemy => "enemy",
                })
                .collect();
            assert_eq!(actual_order.join("|"), fields[5], "{row}");
            assert_eq!(
                battle.player.effective_speed().to_string(),
                fields[1],
                "{row}"
            );
            assert_eq!(
                battle.enemy.effective_speed().to_string(),
                fields[2],
                "{row}"
            );
            assert_eq!(player_move.priority().to_string(), fields[3], "{row}");
            assert_eq!(Move::Ember.priority().to_string(), fields[4], "{row}");
            assert_eq!(
                battle.last_speed_shuffle_roll.unwrap().to_string(),
                fields[7],
                "{row}"
            );
        }
    }

    #[test]
    fn no_usable_pp_uses_source_struggle_without_stab_or_type_effect() {
        use pokerogue_data::Move;

        let fixture = include_str!("../fixtures/struggle.csv");
        let movement = Move::struggle();
        let mut source_move = None;
        let mut source_fallback = false;
        for line in fixture
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
        {
            let fields: Vec<_> = line.split(',').collect();
            match fields[0] {
                "fallback" => {
                    source_fallback =
                        fields[1] == movement.numeric_id().to_string() && fields[2] == "IGNORE_PP";
                }
                "move" => {
                    assert_eq!(movement.id(), fields[1]);
                    assert_eq!(movement.power().to_string(), fields[2]);
                    assert_eq!(movement.accuracy().to_string(), fields[3]);
                    assert_eq!(movement.pp().to_string(), fields[4]);
                    assert_eq!(movement.priority().to_string(), fields[6]);
                    assert!(movement.is_typeless());
                    source_move = Some(());
                }
                _ => panic!("unexpected Struggle fixture row"),
            }
        }
        assert!(source_fallback && source_move.is_some());

        let mut battle = Battle::demo_with_seed("StruggleNoPpSeed");
        battle.player.max_hp = 100;
        battle.player.hp = 100;
        battle.player.speed = 999;
        battle.enemy.speed = 1;
        battle.enemy.max_hp = 1000;
        battle.enemy.hp = 1000;
        battle.player.move_pp_used = battle.player.moves.map(Move::pp);
        battle.enemy.moves = [Move::Imported(334); 4]; // source Harden
        battle.enemy.move_pp_used = [0; 4];
        let spent_pp = battle.player.move_pp_used;
        battle.take_turn(PlayerAction::UseMove(0)).unwrap();
        assert_eq!(battle.player.move_pp_used, spent_pp);
        assert_eq!(battle.player.hp, 75);
        assert!(
            battle.enemy.hp < 1000,
            "Struggle damage plus enemy Harden sequence"
        );
        assert!(battle.rng_draws().iter().all(|draw| draw.range != 100));
        assert!(battle.last_events.iter().any(|event| matches!(
            event,
            pokerogue_core::BattleEvent::MoveExecuted { actor: pokerogue_core::BattleActor::Player, move_ }
                if move_.id() == "STRUGGLE"
        )));
        assert_eq!(
            pokerogue_core::calculate_damage(&battle.player, &battle.enemy, movement, 100)
                .type_multiplier,
            1.0
        );
        assert_eq!(
            pokerogue_core::calculate_damage(&battle.player, &battle.enemy, movement, 100)
                .stab_multiplier,
            1.0
        );

        let mut enemy_exhausted = Battle::demo_with_seed("EnemyStruggleNoPpSeed");
        enemy_exhausted.enemy.move_pp_used = enemy_exhausted.enemy.moves.map(Move::pp);
        enemy_exhausted.enemy.max_hp = 1000;
        enemy_exhausted.enemy.hp = 1000;
        enemy_exhausted.player.speed = 999;
        enemy_exhausted.take_turn(PlayerAction::UseMove(0)).unwrap();
        let decision = enemy_exhausted.last_enemy_decision.as_ref().unwrap();
        assert_eq!(decision.selected_move.id(), "STRUGGLE");
        assert!(decision.random_rolls.is_empty());
        assert!(
            enemy_exhausted.enemy.hp < 750,
            "enemy Struggle recoil is 25% max HP"
        );
        assert!(enemy_exhausted.last_events.iter().any(|event| matches!(
            event,
            pokerogue_core::BattleEvent::MoveExecuted { actor: pokerogue_core::BattleActor::Enemy, move_ }
                if move_.id() == "STRUGGLE"
        )));
    }

    #[test]
    fn wild_enemy_smart_random_matches_executed_original_ai_fixture() {
        let row = include_str!("../fixtures/enemy-ai.csv")
            .lines()
            .find(|line| !line.is_empty() && !line.starts_with('#'))
            .unwrap();
        let fields: Vec<_> = row.split(',').collect();
        assert_eq!(fields[1], "Charmander");
        assert_eq!(fields[2], "ATK10|SPATK11");
        assert_eq!(fields[3], "Bulbasaur");
        assert_eq!(fields[4], "19");
        assert_eq!(fields[5], "GRASS|POISON");
        assert_eq!(fields[6], "Ember|Tackle");
        assert_eq!(fields[7], "Ember:30|Tackle:8");
        let mut battle = Battle::demo_with_seed(fields[0]);
        assert_eq!(battle.enemy.species.name(), fields[1]);
        assert_eq!(battle.enemy.attack, 10);
        assert_eq!(battle.enemy.special_attack, 11);
        assert_eq!(battle.player.species.name(), fields[3]);
        assert_eq!(battle.player.hp.to_string(), fields[4]);
        assert_eq!(
            battle.enemy.moves[..battle.enemy.move_count()]
                .iter()
                .map(|m| m.name())
                .collect::<Vec<_>>()
                .join("|"),
            fields[6]
        );
        assert!(
            calculate_damage(&battle.enemy, &battle.player, Move::Ember, 100).damage
                < battle.player.hp
        );
        assert!(
            calculate_damage(&battle.enemy, &battle.player, Move::Tackle, 100).damage
                < battle.player.hp
        );

        battle.take_turn(PlayerAction::UseMove(1)).unwrap();
        let decision = battle.last_enemy_decision.as_ref().unwrap();
        assert_eq!(decision.selected_move.name().to_lowercase(), fields[9]);
        assert_eq!(decision.candidates, [Move::Ember, Move::Tackle]);
        assert_eq!(decision.ko_damage, [(Move::Ember, 15), (Move::Tackle, 5)]);
        assert_eq!(decision.scores, [(Move::Ember, 30.0), (Move::Tackle, 8.0)]);
        let expected_calls: Vec<_> = fields[8]
            .split('|')
            .map(|call| call.split(':').nth(1).unwrap().parse::<i32>().unwrap())
            .collect();
        assert_eq!(decision.random_rolls, expected_calls);
        assert!(battle.log.contains("Charmander used Ember!"));
    }

    #[test]
    fn enemy_ko_prefilter_matches_original_at_hp_boundaries_and_preserves_rng() {
        for row in include_str!("../fixtures/enemy-ko.csv").lines() {
            if row.is_empty() || row.starts_with('#') {
                continue;
            }
            let fields: Vec<_> = row.split(',').collect();
            let target_hp: i32 = fields[2].parse().unwrap();
            let mut battle = Battle::demo_with_seed(fields[1]);
            battle.player.hp = target_hp;
            battle.take_turn(PlayerAction::UseMove(1)).unwrap();
            let decision = battle.last_enemy_decision.as_ref().unwrap();

            let parse_moves = |value: &str| -> Vec<Move> {
                if value == "none" || value.is_empty() {
                    return Vec::new();
                }
                value
                    .split('|')
                    .map(|name| match name {
                        "Ember" | "ember" => Move::Ember,
                        "Tackle" | "tackle" => Move::Tackle,
                        other => panic!("unknown fixture move {other}"),
                    })
                    .collect()
            };
            let parse_damage = |value: &str| -> Vec<(Move, i32)> {
                value
                    .split('|')
                    .map(|pair| {
                        let (name, damage) = pair.split_once(':').unwrap();
                        (parse_moves(name).remove(0), damage.parse().unwrap())
                    })
                    .collect()
            };
            assert_eq!(decision.ko_damage, parse_damage(fields[3]), "{row}");
            let actual_ko_moves: Vec<_> = decision
                .ko_damage
                .iter()
                .filter(|(_, damage)| *damage >= target_hp)
                .map(|(move_, _)| *move_)
                .collect();
            assert_eq!(actual_ko_moves, parse_moves(fields[4]), "{row}");
            assert_eq!(decision.candidates, parse_moves(fields[5]), "{row}");
            let expected_scores: Vec<_> = fields[6]
                .split('|')
                .map(|pair| {
                    let (name, score) = pair.split_once(':').unwrap();
                    (parse_moves(name).remove(0), score.parse::<f64>().unwrap())
                })
                .collect();
            assert_eq!(decision.scores, expected_scores, "{row}");
            assert_eq!(
                decision.selected_move.name().to_lowercase(),
                fields[8],
                "{row}"
            );
            let expected_rolls: Vec<i32> = if fields[7] == "none" {
                Vec::new()
            } else {
                fields[7]
                    .split('|')
                    .map(|call| call.split(':').nth(1).unwrap().parse().unwrap())
                    .collect()
            };
            assert_eq!(decision.random_rolls, expected_rolls, "{row}");
            if fields[0] == "ember-exact-ko-boundary" {
                assert_eq!(decision.candidates, [Move::Ember]);
                assert!(decision.random_rolls.is_empty());
            } else if fields[0] == "both-exact-ko-boundary" {
                assert_eq!(decision.candidates, [Move::Ember, Move::Tackle]);
                assert_eq!(decision.random_rolls, [4]);
            } else {
                assert_eq!(decision.candidates, [Move::Ember, Move::Tackle]);
                assert_eq!(decision.random_rolls, [4]);
            }
        }
    }

    #[test]
    fn hp_remains_with_each_member_across_multiple_switch_turns() {
        let mut battle = Battle::demo_with_seed("ParitySeed000000");
        let bulbasaur_hp = battle.player.hp;
        let charmander_hp = battle.player.member(1).unwrap().hp;
        battle.take_turn(PlayerAction::Switch(1)).unwrap();
        let charmander_after_turn_one = battle.player.hp;
        assert!(charmander_after_turn_one < charmander_hp);
        assert_eq!(battle.player.member(0).unwrap().hp, bulbasaur_hp);

        battle.take_turn(PlayerAction::Switch(0)).unwrap();
        let bulbasaur_after_turn_two = battle.player.hp;
        assert!(bulbasaur_after_turn_two < bulbasaur_hp);
        assert_eq!(
            battle.player.member(1).unwrap().hp,
            charmander_after_turn_one
        );

        battle.take_turn(PlayerAction::Switch(1)).unwrap();
        assert!(battle.player.hp < charmander_after_turn_one);
        assert_eq!(
            battle.player.member(0).unwrap().hp,
            bulbasaur_after_turn_two
        );
    }

    #[test]
    fn team_supports_original_six_member_cap_and_switch_targets() {
        use pokerogue_core::{PLAYER_PARTY_MAX_SIZE, TeamError};

        let party_source = include_str!("../fixtures/party-capacity.csv")
            .lines()
            .find(|line| line.starts_with("party_capacity,"))
            .unwrap();
        let source_fields: Vec<_> = party_source.split(',').collect();
        assert_eq!(source_fields[1], "7c0881255af5cd51992e513ff93c2f71adbe40e0");
        assert_eq!(PLAYER_PARTY_MAX_SIZE.to_string(), source_fields[2]);

        let mut battle = Battle::demo();
        for species_id in [10, 19, 506, 819] {
            let species = Species::from_numeric_id(species_id).unwrap();
            battle
                .add_player_member(Battler::demo(species, 5, [Move::Tackle, Move::QuickAttack]))
                .unwrap();
        }
        assert_eq!(battle.player.len(), PLAYER_PARTY_MAX_SIZE);
        assert_eq!(
            battle.add_player_member(Battler::demo(
                Species::Bulbasaur,
                5,
                [Move::Tackle, Move::QuickAttack]
            )),
            Err(TeamError::PartyFull)
        );
        assert_eq!(battle.player.len(), 6);

        battle.player.member_mut(1).unwrap().hp = 0;
        assert_eq!(battle.player.next_switch_target(), Some(2));
        let events = battle.player.switch_to(5).unwrap();
        assert_eq!(battle.player.active_slot(), 5);
        assert!(matches!(
            events[4],
            SwitchEvent::SwitchCompleted { active: 5, .. }
        ));
    }

    #[test]
    fn debug_sand_attack_reduces_accuracy_and_causes_the_seeded_enemy_miss() {
        let mut battle = Battle::demo_with_seed("SandSeed40");
        let starting_enemy_hp = battle.enemy.hp;
        battle.debug_sand_attack();
        assert_eq!(battle.enemy.stat_stage(BattleStat::Accuracy), -1);
        assert_eq!(battle.enemy.hp, starting_enemy_hp);
        assert!(battle.log.contains("Sand Attack"));

        let starting_player_hp = battle.player.hp;
        battle.choose_move(0);
        assert!(
            battle.log.contains("Charmander used Ember! It missed!"),
            "unexpected turn log: {}",
            battle.log
        );
        assert!(battle.log.contains("Bulbasaur used Vine Whip!"));
        assert_eq!(battle.player.hp, starting_player_hp);
        assert_eq!(battle.enemy.stat_stage(BattleStat::Accuracy), -1);
    }

    #[test]
    fn faint_with_reserve_waits_for_forced_replacement_and_resumes_next_turn() {
        let mut battle = Battle::demo_with_seed("ForcedReplacementSeed");
        battle.player.hp = 1;
        battle.player.set_stat_stage(BattleStat::Accuracy, -3);
        battle.player.set_stat_stage(BattleStat::Evasion, 2);
        battle.player.set_stat_stage(BattleStat::Speed, 1);
        battle.enemy.speed = 30;
        battle.enemy.set_stat_stage(BattleStat::Accuracy, 6);
        let reserve_before = battle.player.member(1).unwrap().clone();
        battle.take_turn(PlayerAction::UseMove(0)).unwrap();

        assert_eq!(battle.last_action_order[0], BattleActor::Enemy);
        assert_eq!(battle.player.active_slot(), 0);
        assert_eq!(battle.player.hp, 0);
        assert_eq!(battle.turn, 1);
        assert_eq!(battle.outcome, None);
        assert!(battle.needs_replacement());
        assert_eq!(battle.pending_replacement_slot(), Some(0));
        assert_eq!(battle.player.member(1).unwrap().hp, reserve_before.hp);
        assert_eq!(battle.player.stat_stage(BattleStat::Accuracy), 0);
        assert_eq!(battle.player.stat_stage(BattleStat::Evasion), 0);
        assert_eq!(battle.player.stat_stage(BattleStat::Speed), 0);

        assert_eq!(
            battle.take_turn(PlayerAction::UseMove(0)),
            Err(TurnActionError::MandatoryReplacement)
        );
        assert_eq!(
            battle.switch_player(1),
            Err(SwitchError::MandatoryReplacement)
        );
        assert_eq!(battle.replace_fainted_player(0), Err(SwitchError::Fainted));
        assert_eq!(
            battle.replace_fainted_player(8),
            Err(SwitchError::SlotOutOfRange)
        );
        assert_eq!(
            battle
                .last_events
                .iter()
                .filter(|event| matches!(
                    event,
                    BattleEvent::MoveExecuted {
                        actor: BattleActor::Enemy,
                        ..
                    }
                ))
                .count(),
            1
        );
        assert!(!battle.last_events.iter().any(|event| matches!(
            event,
            BattleEvent::MoveExecuted {
                actor: BattleActor::Player,
                ..
            }
        )));

        let faint_event = battle
            .last_events
            .iter()
            .position(|event| matches!(event, BattleEvent::PokemonFainted { slot: 0 }))
            .unwrap();
        let request_event = battle
            .last_events
            .iter()
            .position(|event| matches!(event, BattleEvent::ReplacementRequired { fainted_slot: 0 }))
            .unwrap();
        assert!(faint_event < request_event);
        assert!(
            battle
                .last_events
                .contains(&BattleEvent::StatStagesReset { slot: 0 })
        );

        let draws_at_prompt = battle.rng_draws().len();
        let events = battle.replace_fainted_player(1).unwrap();
        assert_eq!(events[0], SwitchEvent::OutgoingLeftField { slot: 0 });
        assert_eq!(events[3], SwitchEvent::IncomingEnteredField { slot: 1 });
        assert_eq!(battle.player.active_slot(), 1);
        assert_eq!(battle.player.hp, reserve_before.hp);
        assert_eq!(battle.player.moves, reserve_before.moves);
        assert_eq!(battle.turn, 2);
        assert_eq!(battle.outcome, None);
        assert!(!battle.needs_replacement());
        assert_eq!(battle.rng_draws().len(), draws_at_prompt);

        let switch_event = battle
            .last_events
            .iter()
            .position(|event| {
                matches!(
                    event,
                    BattleEvent::Switch(SwitchEvent::SwitchCompleted { active: 1, .. })
                )
            })
            .unwrap();
        let turn_end = battle
            .last_events
            .iter()
            .position(|event| matches!(event, BattleEvent::TurnEnded { turn: 2 }))
            .unwrap();
        assert!(switch_event < turn_end);
        assert_eq!(
            battle
                .last_events
                .iter()
                .filter(|event| matches!(
                    event,
                    BattleEvent::MoveExecuted {
                        actor: BattleActor::Enemy,
                        ..
                    }
                ))
                .count(),
            1,
            "forced replacement must not cause a second enemy attack"
        );

        battle.take_turn(PlayerAction::UseMove(0)).unwrap();
        assert_eq!(battle.turn, 3);
        assert!(battle.rng_draws().len() > draws_at_prompt);
        assert_eq!(battle.outcome, None);
    }

    #[test]
    fn town_wave1_enemy_first_faint_cancels_player_move_and_replaces_from_party() {
        let mut turtwig = Battler::demo(
            Species::from_numeric_id(387).unwrap(),
            5,
            [Move::Imported(33), Move::Imported(71), Move::Imported(110)],
        );
        turtwig.hp = 21;
        turtwig.max_hp = 21;
        turtwig.attack = 12;
        turtwig.defense = 12;
        turtwig.special_attack = 10;
        turtwig.special_defense = 11;
        turtwig.speed = 8;
        let mut bulbasaur = Battler::demo(
            Species::Bulbasaur,
            5,
            [Move::Imported(33), Move::Imported(45), Move::Imported(22)],
        );
        bulbasaur.hp = 20;
        bulbasaur.max_hp = 20;
        bulbasaur.attack = 10;
        bulbasaur.defense = 10;
        bulbasaur.special_attack = 12;
        bulbasaur.special_defense = 12;
        bulbasaur.speed = 10;

        let team = Team::new(turtwig, bulbasaur);
        let (mut battle, encounter) =
            Battle::town_classic_entry_for_wave_with_team("M12Speed0016", "DAY", 1, team).unwrap();
        assert_eq!(encounter.battler.species.numeric_id(), 276);
        assert_eq!(encounter.battler.hp, 14);
        assert_eq!(encounter.battler.speed, 9);
        assert_eq!(
            battle.player.member(0).unwrap().moves[2],
            Move::Imported(110)
        );

        battle.take_turn(PlayerAction::UseMove(0)).unwrap();
        assert_eq!(
            battle.last_action_order,
            [BattleActor::Enemy, BattleActor::Player]
        );
        assert_eq!(
            battle.player.member(0).unwrap().hp,
            13,
            "first-turn events: {:?}; draws: {:?}",
            battle.last_events,
            battle.rng_draws()
        );
        assert_eq!(battle.enemy.hp, 7);
        assert_eq!(battle.player.member(0).unwrap().move_pp_used[0], 1);

        battle.take_turn(PlayerAction::UseMove(0)).unwrap();
        assert_eq!(
            battle.last_action_order,
            [BattleActor::Enemy, BattleActor::Player]
        );
        assert_eq!(battle.player.member(0).unwrap().hp, 6);
        assert_eq!(battle.enemy.hp, 1);
        assert_eq!(battle.player.member(0).unwrap().move_pp_used[0], 2);

        battle.take_turn(PlayerAction::UseMove(0)).unwrap();
        assert!(battle.needs_replacement());
        assert_eq!(battle.pending_replacement_slot(), Some(0));
        assert_eq!(battle.player.active_slot(), 0);
        assert_eq!(battle.player.member(0).unwrap().hp, 0);
        assert_eq!(battle.enemy.hp, 1);
        assert_eq!(battle.player.member(0).unwrap().move_pp_used[0], 2);
        assert_eq!(battle.turn, 3);
        assert!(battle.last_events.iter().any(|event| matches!(
            event,
            BattleEvent::MoveExecuted {
                actor: BattleActor::Enemy,
                move_: Move::Imported(64)
            }
        )));
        assert!(
            !battle.last_events.iter().any(|event| matches!(
                event,
                BattleEvent::MoveExecuted {
                    actor: BattleActor::Player,
                    ..
                }
            )),
            "the source-queued player MovePhase is canceled while its actor is fainted"
        );
        assert!(
            battle
                .last_events
                .iter()
                .any(|event| matches!(event, BattleEvent::ReplacementRequired { fainted_slot: 0 }))
        );

        let rng_at_replacement = battle.rng_draws().len();
        battle.replace_fainted_player(1).unwrap();
        assert_eq!(battle.player.active_slot(), 1);
        assert_eq!(battle.player.hp, 20);
        assert_eq!(battle.turn, 4);
        assert_eq!(battle.rng_draws().len(), rng_at_replacement);
        assert!(!battle.needs_replacement());
    }

    #[test]
    fn faint_phase_guards_and_turn_end_match_executed_source_methods() {
        let rows: Vec<Vec<_>> = include_str!("../fixtures/faint-phase-guards.csv")
            .lines()
            .filter(|line| !line.starts_with('#'))
            .map(|line| line.split(',').collect())
            .collect();
        assert_eq!(
            rows[0],
            ["inactive-move", "1", "1", "-", "-", "-", "-", "-"]
        );
        assert_eq!(rows[1], ["turn-end", "-", "-", "2", "2", "1", "1", "1"]);

        // The Rust continuation is covered separately from the method fixture:
        // enemy action -> faint prompt -> replacement -> TurnEnd, without a
        // second enemy action or replacement-local RNG draw.
        let mut battle = Battle::demo_with_seed("ForcedReplacementSeed");
        battle.player.hp = 1;
        battle.enemy.speed = 30;
        battle.take_turn(PlayerAction::UseMove(0)).unwrap();
        assert!(battle.needs_replacement());
        assert_eq!(battle.turn, 1);
        battle.replace_fainted_player(1).unwrap();
        assert_eq!(battle.turn, 2);
        assert!(
            battle
                .last_events
                .iter()
                .any(|event| matches!(event, BattleEvent::TurnEnded { turn: 2 }))
        );
        assert_eq!(
            battle
                .last_events
                .iter()
                .filter(|event| matches!(
                    event,
                    BattleEvent::MoveExecuted {
                        actor: BattleActor::Enemy,
                        ..
                    }
                ))
                .count(),
            1
        );
    }

    #[test]
    fn faint_without_a_healthy_reserve_ends_the_battle() {
        let mut battle = Battle::demo_with_seed("NoReserveFaintSeed");
        battle.player.hp = 1;
        battle.player.member_mut(1).unwrap().hp = 0;
        battle.take_turn(PlayerAction::UseMove(0)).unwrap();
        assert_eq!(battle.outcome, Some(Outcome::PlayerLoss));
        assert!(!battle.needs_replacement());
        assert_eq!(battle.turn, 1);
        assert!(
            battle
                .last_events
                .contains(&BattleEvent::BattleEnded(Outcome::PlayerLoss))
        );
        assert!(
            !battle
                .last_events
                .iter()
                .any(|event| matches!(event, BattleEvent::ReplacementRequired { .. }))
        );
        assert_eq!(
            battle.replace_fainted_player(1),
            Err(SwitchError::BattleOver)
        );
    }

    #[test]
    fn faint_and_replacement_branch_match_executed_typescript_fixture() {
        let rows: Vec<_> = include_str!("../fixtures/faint-switch.csv")
            .lines()
            .filter(|row| !row.is_empty() && !row.starts_with('#'))
            .map(|row| row.split(',').collect::<Vec<_>>())
            .collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            &rows[0][..3],
            ["no-legal-reserve", "unshift", "GameOverPhase"]
        );
        assert_eq!(
            &rows[1][..7],
            [
                "healthy-reserve",
                "push",
                "SwitchPhase",
                "SWITCH",
                "0",
                "true",
                "false"
            ]
        );
    }

    #[test]
    fn attacker_target_counter_move_is_excluded_before_ko_damage_and_selection() {
        use pokerogue_data::MoveTarget;

        assert_eq!(Move::Counter.target(), MoveTarget::Attacker);
        assert_eq!(Move::Counter.power(), -1);
        let mut battle = Battle::demo_with_seed("CounterSeed00000");
        battle.enemy.moves = [Move::Counter, Move::Tackle, Move::Tackle, Move::Tackle];
        battle.player.hp = 1;
        battle.take_turn(PlayerAction::UseMove(1)).unwrap();
        let decision = battle.last_enemy_decision.as_ref().unwrap();
        assert_eq!(decision.ko_damage, [(Move::Tackle, 5)]);
        assert_eq!(decision.candidates, [Move::Tackle]);
        assert_eq!(decision.selected_move, Move::Tackle);
        assert!(decision.random_rolls.is_empty());

        let fixture: Vec<_> = include_str!("../fixtures/enemy-ko-target.csv")
            .lines()
            .filter(|row| !row.is_empty() && !row.starts_with('#'))
            .map(|row| row.split(',').collect::<Vec<_>>())
            .collect();
        assert_eq!(fixture.len(), 2);
        assert_eq!(fixture[0][0], "attacker-target-counter-excluded");
        assert_eq!(fixture[0][3], "tackle:5");
        assert_eq!(fixture[0][5], "Tackle");
        assert_eq!(fixture[0][7], "none");
        assert_eq!(fixture[0][8], "tackle");
        // Source-executed counterfactual: if Counter is misclassified as an ordinary target,
        // it enters the KO pool, the seeded 8:5 roll advances to it, and it is chosen.
        assert_eq!(fixture[1][0], "counter-mistarget-control");
        assert_eq!(fixture[1][5], "Counter|Tackle");
        assert_eq!(fixture[1][7], "8:5");
        assert_eq!(fixture[1][8], "counter");
    }

    #[test]
    fn town_first_wave_encounter_matches_executed_arena_fixture() {
        use pokerogue_core::{normal_tier_for_roll, select_town_encounter};

        let rows: Vec<Vec<&str>> = include_str!("../fixtures/town-encounter.csv")
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| line.split(',').collect())
            .collect();
        let scenario = rows.iter().find(|row| row[0] == "scenario").unwrap();
        assert_eq!(scenario[1], "7c0881255af5cd51992e513ff93c2f71adbe40e0");
        let result = select_town_encounter(scenario[2], scenario[6]).unwrap();
        assert_eq!(result.tier_roll.to_string(), scenario[10]);
        assert_eq!(result.tier, scenario[11]);
        let candidates: Vec<_> = scenario[14]
            .split('|')
            .map(|candidate| {
                let (id, numeric) = candidate.split_once(':').unwrap();
                (numeric.parse::<u16>().unwrap(), id)
            })
            .collect();
        assert_eq!(
            result.candidates,
            candidates
                .iter()
                .map(|(id, name)| (*id, *name))
                .collect::<Vec<_>>()
        );
        assert_eq!(result.selected_index.to_string(), scenario[15]);
        assert_eq!(result.species.0.to_string(), scenario[17]);
        assert_eq!(result.species.1, scenario[16]);
        let draws: Vec<_> = scenario[18]
            .split('|')
            .map(|draw| {
                let fields: Vec<_> = draw.split(':').collect();
                (
                    fields[0].parse::<u32>().unwrap(),
                    fields[1].parse::<i32>().unwrap(),
                    fields[2].parse::<i32>().unwrap(),
                )
            })
            .collect();
        assert_eq!(result.rng_draws.len(), draws.len());
        for (actual, (range, min, value)) in result.rng_draws.iter().zip(draws) {
            assert_eq!(
                (actual.turn, actual.range, actual.min, actual.value),
                (0, range, min, value)
            );
        }

        // Every threshold and inclusive edge is checked against the source-executed tier methods.
        for weight in rows
            .iter()
            .filter(|row| row[0] == "weight" && row[1] == "NON_BOSS")
        {
            let tier_id = weight[3].parse::<u8>().unwrap();
            let start = weight[5].parse::<u16>().unwrap();
            let end = weight[6].parse::<u16>().unwrap();
            for roll in start..=end {
                assert_eq!(normal_tier_for_roll(roll), Some(tier_id), "roll {roll}");
            }
        }
        assert_eq!(normal_tier_for_roll(512), None);

        // The original updatePoolsForTimeOfDay method is source-executed for each time bucket.
        for row in rows.iter().filter(|row| row[0] == "time_pool") {
            let tier_id = row[2].parse::<u8>().unwrap();
            let combined: Vec<u16> = if row[7].is_empty() {
                vec![]
            } else {
                row[7]
                    .split('|')
                    .map(|item| item.split(':').nth(1).unwrap().parse().unwrap())
                    .collect()
            };
            let rust_pool: Vec<u16> = ["ALL", row[1]]
                .into_iter()
                .flat_map(|time| {
                    TOWN_ENCOUNTER_POOLS
                        .iter()
                        .filter(move |pool| pool.tier_id == tier_id && pool.time_of_day == time)
                        .flat_map(|pool| pool.species.iter().map(|species| species.0))
                })
                .collect();
            assert_eq!(rust_pool, combined, "time {} tier {tier_id}", row[1]);
        }
    }

    #[test]
    fn first_wave_battle_entry_matches_source_level_species_stats_and_learnset() {
        use pokerogue_core::{Battle, generate_town_wild_encounter_for_wave_isolated};

        let row = include_str!("../fixtures/town-encounter.csv")
            .lines()
            .find(|line| line.starts_with("entry_scenario,"))
            .unwrap();
        let fields: Vec<_> = row.split(',').collect();
        assert_eq!(fields[1], "7c0881255af5cd51992e513ff93c2f71adbe40e0");
        let encounter =
            generate_town_wild_encounter_for_wave_isolated(fields[2], fields[6], 1).unwrap();
        assert_eq!(encounter.level.to_string(), fields[4]);
        assert_eq!(encounter.selection.tier_roll.to_string(), fields[7]);
        assert_eq!(encounter.selection.tier, fields[8]);
        assert_eq!(encounter.battler.species.name().to_uppercase(), fields[9]);
        assert_eq!(
            encounter.battler.species.numeric_id().to_string(),
            fields[10]
        );
        assert_eq!(encounter.selection.selected_index.to_string(), fields[11]);
        assert_eq!(encounter.battle_seed, fields[17]);
        assert_eq!(encounter.available_level_moves.join("|"), fields[12]);
        assert_eq!(
            encounter.battler.moves[..encounter.battler.move_count()]
                .iter()
                .map(|m| m.id())
                .collect::<Vec<_>>()
                .join("|"),
            fields[13]
        );
        let stats: Vec<i32> = fields[14].split('|').map(|n| n.parse().unwrap()).collect();
        assert_eq!(
            [
                encounter.battler.hp,
                encounter.battler.attack,
                encounter.battler.defense,
                encounter.battler.special_attack,
                encounter.battler.special_defense,
                encounter.battler.speed,
            ],
            stats.as_slice()
        );
        let calls: Vec<_> = fields[15].split('|').collect();
        let source_floats: Vec<f64> = calls
            .iter()
            .filter_map(|call| call.strip_prefix("F:").map(|n| n.parse().unwrap()))
            .collect();
        assert_eq!(encounter.level_random_values.len(), source_floats.len());
        for (rust, source) in encounter.level_random_values.iter().zip(source_floats) {
            assert!((rust - source).abs() < 1e-15);
        }
        let source_ints: Vec<_> = calls
            .iter()
            .filter_map(|call| call.strip_prefix("I:"))
            .collect();
        assert_eq!(source_ints.len(), encounter.selection.rng_draws.len());
        for (call, draw) in source_ints.iter().zip(&encounter.selection.rng_draws) {
            let expected: Vec<i32> = call
                .split(':')
                .map(|value| value.parse().unwrap())
                .collect();
            assert_eq!(
                (draw.range as i32, draw.min, draw.value),
                (expected[0], expected[1], expected[2])
            );
        }

        let mut battle = Battle::town_encounter_with_seed(fields[2], fields[6]).unwrap();
        assert_eq!(battle.enemy.species.numeric_id().to_string(), fields[10]);
        assert_eq!(battle.enemy_exp_value(), fields[16].parse::<f64>().unwrap());
        assert_eq!(
            battle.initial_encounter.as_ref().unwrap().level,
            encounter.level
        );
        // The generated two-slot moveset includes one damaging move and remains playable
        // through the existing battle resolver.
        assert!(battle.take_turn(PlayerAction::UseMove(0)).is_ok());
    }

    #[test]
    fn imported_exp_thresholds_match_executed_typescript_function() {
        for row in include_str!("../fixtures/exp-thresholds.csv").lines() {
            if row.is_empty() || row.starts_with('#') {
                continue;
            }
            let fields: Vec<_> = row.split(',').collect();
            let rate: usize = fields[2].parse().unwrap();
            let level: usize = fields[3].parse().unwrap();
            let expected: u32 = fields[4].parse().unwrap();
            assert_eq!(
                pokerogue_data::EXP_THRESHOLDS[rate][level - 1],
                expected,
                "{row}"
            );
        }
    }

    #[test]
    fn new_battle_phase_handoff_and_next_wave_match_executed_source_methods() {
        use pokerogue_core::{MiniRun, MiniRunStage};
        for row in include_str!("../fixtures/new-battle-handoff.csv")
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
        {
            let fields: Vec<_> = row.split(',').collect();
            let current_wave: u8 = fields[0].parse().unwrap();
            let source_next_wave: u8 = fields[1].parse().unwrap();
            assert_eq!(fields[2], "WILD");
            assert_eq!(fields[3], "remove:NewBattlePhase|newBattle|end");
            if current_wave < 20 {
                let mut run = MiniRun::new_with_wave_limit("NewBattleHandoff", "DAY", 20).unwrap();
                run.wave = current_wave;
                run.stage = MiniRunStage::Reward;
                run.reward_claimed = true;
                run.continue_after_reward().unwrap();
                assert_eq!(run.wave, source_next_wave, "wave {current_wave}");
                assert_eq!(run.stage, MiniRunStage::Battle);
            } else {
                assert_eq!(source_next_wave, 21);
                let mut bounded =
                    MiniRun::new_with_wave_limit("NewBattleHandoff", "DAY", 20).unwrap();
                bounded.wave = current_wave;
                bounded.stage = MiniRunStage::Reward;
                bounded.reward_claimed = true;
                bounded.continue_after_reward().unwrap();
                assert_eq!(bounded.stage, MiniRunStage::Complete);
            }
        }
    }

    #[test]
    fn new_battle_source_execution_matches_local_wave_entry_through_twenty() {
        use pokerogue_core::{MiniRun, MiniRunStage};

        let mut run = MiniRun::new_with_wave_limit("MiniRunTownSeed0001", "DAY", 20).unwrap();
        for row in include_str!("../fixtures/new-battle-execution.csv")
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
        {
            let fields = row.split(',').collect::<Vec<_>>();
            let previous_wave = fields[0].parse::<u8>().unwrap();
            let source_wave = fields[1].parse::<u8>().unwrap();
            assert_eq!(fields[2], "WILD");
            assert_eq!(fields[3], "0", "bounded source harness fixes double=false");
            assert_eq!(
                fields[4], "1",
                "source Battle increments turn after construction"
            );
            assert_eq!(fields[7], fields[5], "offset-scoped RNG restores wave seed");
            let trace = fields[8];
            assert!(trace.contains("|turn:1"), "source turn increment: {trace}");
            assert_eq!(
                trace.matches("sow:").count(),
                2,
                "seed reset and scoped battle seed"
            );

            if source_wave > 20 {
                assert_eq!(previous_wave, 20);
                assert_eq!(run.stage, MiniRunStage::Battle);
                run.stage = MiniRunStage::Reward;
                run.reward_claimed = true;
                run.continue_after_reward().unwrap();
                assert_eq!(run.stage, MiniRunStage::Complete);
                continue;
            }
            while run.wave < source_wave {
                run.stage = MiniRunStage::Reward;
                run.reward_claimed = true;
                run.continue_after_reward().unwrap();
            }
            assert_eq!(
                run.wave, source_wave,
                "source transition from {previous_wave}"
            );
            assert_eq!(run.stage, MiniRunStage::Battle);
        }
    }

    #[test]
    fn victory_phase_queue_matches_executed_source_reward_schedule() {
        use pokerogue_core::{MiniRun, MiniRunStage, classic_victory_reward_schedule};
        for row in include_str!("../fixtures/victory-phase.csv")
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
        {
            let (wave, queue) = row.split_once(',').unwrap();
            let wave: u32 = wave.parse().unwrap();
            let phases: Vec<_> = queue.split('|').collect();
            let schedule = classic_victory_reward_schedule(wave);
            assert_eq!(
                phases
                    .iter()
                    .filter(|phase| **phase == "SelectModifierPhase")
                    .count(),
                usize::from(schedule.natural_modifier_offer),
                "wave {wave}"
            );
            assert_eq!(
                phases
                    .iter()
                    .filter(|phase| **phase == "ModifierRewardPhase:EXP_CHARM")
                    .count(),
                usize::from(schedule.exp_charm_count),
                "wave {wave}"
            );
            assert_eq!(
                phases
                    .iter()
                    .filter(|phase| **phase == "ModifierRewardPhase:SUPER_EXP_CHARM")
                    .count(),
                usize::from(schedule.super_exp_charm_count),
                "wave {wave}"
            );
            assert!(
                phases.first() == Some(&"BattleEndPhase:true"),
                "wave {wave}"
            );
            assert!(phases.contains(&"EggLapsePhase"), "wave {wave}");
            assert!(
                phases.contains(&"NewBattlePhase"),
                "source continues after wave {wave}"
            );
        }
        let mut run = MiniRun::new_with_wave_limit("SourceVictoryQueue", "DAY", 20).unwrap();
        run.wave = 20;
        run.stage = MiniRunStage::Reward;
        run.reward_claimed = true;
        run.continue_after_reward().unwrap();
        assert_eq!(run.stage, MiniRunStage::Complete);
        assert!(
            include_str!("../fixtures/victory-phase.csv")
                .lines()
                .any(|line| line.starts_with("20,") && line.contains("NewBattlePhase")),
            "the 20-wave local endpoint is a custom truncation of Classic's source queue"
        );
    }

    #[test]
    fn exp_charm_schedule_and_formula_match_executed_typescript_methods() {
        use pokerogue_core::{apply_exp_boosters, classic_victory_reward_schedule};

        for row in include_str!("../fixtures/exp-charm.csv").lines() {
            if row.is_empty() || row.starts_with('#') {
                continue;
            }
            let fields: Vec<_> = row.split(',').collect();
            assert_eq!(fields[0], "schedule");
            let wave: u32 = fields[1].parse().unwrap();
            let expected = classic_victory_reward_schedule(wave);
            assert_eq!(
                expected.natural_modifier_offer,
                fields[2] == "1",
                "wave {wave}"
            );
            assert_eq!(
                expected.exp_charm_count,
                fields[3].parse::<u8>().unwrap(),
                "wave {wave}"
            );
            assert_eq!(
                expected.super_exp_charm_count,
                fields[4].parse::<u8>().unwrap(),
                "wave {wave}"
            );
            let normal_stacks: u8 = fields[5].parse().unwrap();
            let super_stacks: u8 = fields[6].parse().unwrap();
            assert_eq!(fields[7], "25", "EXP Charm percentage");
            assert_eq!(fields[8], "60", "Super EXP Charm percentage");
            let input: u32 = fields[9].parse().unwrap();
            let expected_exp: u32 = fields[10].parse().unwrap();
            assert_eq!(
                apply_exp_boosters(input, normal_stacks, super_stacks),
                expected_exp,
                "wave {wave}"
            );
            assert_eq!(fields[11], "99", "EXP Charm stack cap");
            assert_eq!(fields[12], "30", "Super EXP Charm stack cap");
        }
    }

    #[test]
    fn exp_charm_only_applies_to_later_victory_experience() {
        let mut battle =
            Battle::town_encounter_for_wave("TownWaveEntrySeed0001", "DAY", 2).unwrap();
        battle.outcome = Some(Outcome::PlayerWin);
        let base = battle.enemy_exp_value().floor() as u32;
        let first = battle.award_victory_experience_with_charm_stacks(0);
        assert_eq!(first[0].1, base);

        let mut following_battle =
            Battle::town_encounter_for_wave("TownWaveEntrySeed0001", "DAY", 3).unwrap();
        following_battle.outcome = Some(Outcome::PlayerWin);
        let raw = following_battle.enemy_exp_value();
        let participant_exp = raw.floor() as u32;
        let with_charm = following_battle.award_victory_experience_with_charm_stacks(1);
        assert_eq!(
            with_charm[0].1,
            (participant_exp as f64 * 1.25).floor() as u32
        );
    }

    #[test]
    fn mini_run_applies_source_fixed_rewards_after_experience_on_boss_waves() {
        use pokerogue_core::{MiniRun, MiniRunStage};
        for (wave, regular, super_charm) in [(10, 2, 0), (20, 0, 1)] {
            let mut run = MiniRun::new_with_wave_limit("BossRewardSchedule", "DAY", 20).unwrap();
            run.wave = wave;
            run.battle.player.speed = 999;
            run.battle.enemy.hp = 1;
            run.take_action(PlayerAction::UseMove(0)).unwrap();
            assert_eq!(run.stage, MiniRunStage::Reward, "wave {wave}");
            assert!(
                run.battle
                    .player
                    .member(run.battle.player.active_slot())
                    .unwrap()
                    .level
                    <= pokerogue_core::classic_exp_level_cap(u32::from(wave), 0, false),
                "XP exceeded the original Classic level cap at wave {wave}"
            );
            assert_eq!(run.exp_charm_stacks, regular, "wave {wave}");
            assert_eq!(run.super_exp_charm_stacks, super_charm, "wave {wave}");
            assert!(run.reward_claimed, "wave {wave} has fixed reward only");
            assert!(run.reward_options().is_empty(), "wave {wave}");
            let resumed = MiniRun::from_checkpoint(&run.checkpoint().unwrap()).unwrap();
            assert_eq!(resumed.exp_charm_stacks, regular, "wave {wave}");
            assert_eq!(resumed.super_exp_charm_stacks, super_charm, "wave {wave}");
        }
    }

    #[test]
    fn generated_first_wave_can_be_played_to_victory_and_award_source_exp() {
        let mut battle = Battle::town_encounter_with_seed("TownWaveEntrySeed0001", "DAY").unwrap();
        for _ in 0..20 {
            if battle.outcome.is_some() {
                break;
            }
            if battle.needs_replacement() {
                battle.replace_fainted_player(1).unwrap();
            } else {
                battle.take_turn(PlayerAction::UseMove(0)).unwrap();
            }
        }
        assert_eq!(battle.outcome, Some(Outcome::PlayerWin), "{}", battle.log);
        let rewards = battle.award_victory_experience();
        assert_eq!(rewards.len(), 1);
        assert_eq!(rewards[0].0, 0);
        assert_eq!(rewards[0].1, battle.enemy_exp_value().floor() as u32);
        assert!(battle.player.member(0).unwrap().exp > total_exp_for_test_level());
    }

    #[test]
    fn town_encounters_through_wave_twenty_match_source_executed_typescript_fixtures() {
        use pokerogue_core::generate_town_wild_encounter_for_wave_isolated;

        for row in include_str!("../fixtures/town-encounter.csv")
            .lines()
            .filter(|line| line.starts_with("run_wave,"))
        {
            let fields: Vec<_> = row.split(',').collect();
            assert_eq!(fields[1], "7c0881255af5cd51992e513ff93c2f71adbe40e0");
            let wave: u32 = fields[3].parse().unwrap();
            let encounter =
                generate_town_wild_encounter_for_wave_isolated(fields[2], fields[5], wave).unwrap();
            assert_eq!(encounter.battle_seed, fields[16], "wave {wave}");
            if wave % 10 == 0 {
                assert!(encounter.boss_segments >= 2);
            }
            assert_eq!(encounter.level.to_string(), fields[4], "wave {wave}");
            assert_eq!(
                encounter.selection.tier_roll.to_string(),
                fields[6],
                "wave {wave}"
            );
            assert_eq!(encounter.selection.tier, fields[7], "wave {wave}");
            assert_eq!(
                pokerogue_data::species_by_numeric_id(encounter.battler.species.numeric_id())
                    .unwrap()
                    .id,
                fields[8],
                "wave {wave}"
            );
            assert_eq!(
                encounter.battler.species.numeric_id().to_string(),
                fields[9],
                "wave {wave}"
            );
            assert_eq!(
                encounter.selection.selected_index.to_string(),
                fields[10],
                "wave {wave}"
            );
            assert_eq!(
                encounter.available_level_moves.join("|"),
                fields[11],
                "wave {wave}"
            );
            let projected_moves: Vec<_> = fields[12].split('|').collect();
            let modeled_moves = encounter.battler.moves.map(Move::id);
            assert_eq!(modeled_moves[0], projected_moves[0], "wave {wave}");
            // Battler currently has a fixed two-move array; a source projection
            // with one move duplicates slot zero in the model (e.g. Silcoon).
            assert_eq!(
                modeled_moves[1],
                *projected_moves.get(1).unwrap_or(&projected_moves[0]),
                "wave {wave}"
            );
            let stats: Vec<i32> = fields[13]
                .split('|')
                .map(|value| value.parse().unwrap())
                .collect();
            assert_eq!(
                [
                    encounter.battler.hp,
                    encounter.battler.attack,
                    encounter.battler.defense,
                    encounter.battler.special_attack,
                    encounter.battler.special_defense,
                    encounter.battler.speed
                ],
                stats.as_slice(),
                "wave {wave}"
            );
            let calls: Vec<_> = fields[14].split('|').collect();
            let source_floats: Vec<f64> = calls
                .iter()
                .filter_map(|call| {
                    call.strip_prefix("F:")
                        .or_else(|| call.strip_prefix("R:"))
                        .map(|value| value.parse().unwrap())
                })
                .collect();
            assert_eq!(
                encounter.level_random_values.len(),
                source_floats.len(),
                "wave {wave}"
            );
            for (actual, expected) in encounter.level_random_values.iter().zip(source_floats) {
                assert!(
                    (actual - expected).abs() < 1e-15,
                    "wave {wave}: {actual} != {expected}"
                );
            }
            let source_ints: Vec<Vec<i32>> = calls
                .iter()
                .filter_map(|call| call.strip_prefix("I:"))
                .map(|call| {
                    call.split(':')
                        .map(|value| value.parse().unwrap())
                        .collect()
                })
                .collect();
            assert_eq!(
                source_ints.len(),
                encounter.selection.rng_draws.len(),
                "wave {wave}"
            );
            for (source, rust) in source_ints.iter().zip(&encounter.selection.rng_draws) {
                assert_eq!(
                    (rust.range as i32, rust.min, rust.value),
                    (source[0], source[1], source[2]),
                    "wave {wave}"
                );
            }
            let battle = Battle::town_encounter_for_wave(fields[2], fields[5], wave).unwrap();
            assert_eq!(
                battle.enemy_exp_value(),
                fields[15].parse::<f64>().unwrap(),
                "wave {wave}"
            );
        }
    }

    #[test]
    fn normal_town_entry_consumes_double_check_before_encounter_rng() {
        let (battle, encounter) =
            pokerogue_core::Battle::town_classic_entry_for_wave("M12TownWave1Replay6", "DAY", 1)
                .unwrap();
        // The executed Phaser trace consumes integerInRange(0, 7) in
        // BattleScene.checkIsDouble before EncounterPhase calls randomSpecies.
        assert_eq!(encounter.selection.tier_roll, 288);
        assert_eq!(encounter.selection.species.0, 819);
        assert_eq!(
            encounter
                .construction_rng_draws
                .iter()
                .map(|draw| (draw.range, draw.min, draw.value))
                .collect::<Vec<_>>(),
            vec![
                (256, 0, 218),
                (4_294_967_296, 0, 693_933_964),
                (25, 0, 1),
                (13_049, 0, 4_721),
                (13_049, 0, 6_106),
            ]
        );
        assert_eq!(encounter.generated_ivs, [20, 21, 25, 5, 28, 12]);
        assert_eq!(encounter.generated_id, 693_933_964);
        assert_eq!(encounter.generated_nature, 1);
        assert_eq!(encounter.generated_ability_index, 0);
        assert_eq!(encounter.generated_ability_id, 167);
        assert_eq!(encounter.generated_moves, [33, 39]);
        assert_eq!(
            [
                encounter.battler.max_hp,
                encounter.battler.attack,
                encounter.battler.defense,
                encounter.battler.special_attack,
                encounter.battler.special_defense,
                encounter.battler.speed,
            ],
            [15, 8, 6, 6, 6, 6]
        );
        assert_eq!(encounter.additional_opponents.len(), 1);
        let second = &encounter.additional_opponents[0];
        assert_eq!(second.species_id, 396);
        assert_eq!(second.generated_ivs, [5, 29, 28, 13, 19, 17]);
        assert_eq!(second.generated_nature, 9);
        assert_eq!(second.generated_ability_id, 120);
        assert_eq!(second.generated_moves, [33, 45]);
        assert_eq!(
            [
                second.battler.max_hp,
                second.battler.attack,
                second.battler.defense,
                second.battler.special_attack,
                second.battler.special_defense,
                second.battler.speed,
            ],
            [13, 7, 7, 6, 5, 7]
        );
        assert_eq!(
            encounter
                .encounter_rng_trace
                .iter()
                .map(|draw| (draw.range, draw.min, draw.value))
                .collect::<Vec<_>>(),
            vec![
                (512, 0, 288),
                (24, 0, 23),
                (256, 0, 218),
                (4_294_967_296, 0, 693_933_964),
                (25, 0, 1),
                (13_049, 0, 4_721),
                (13_049, 0, 6_106),
                (512, 0, 203),
                (24, 0, 18),
                (256, 0, 0),
                (4_294_967_296, 0, 1_272_854_129),
                (25, 0, 9),
                (2, 0, 0),
                (13_049, 0, 10_078),
                (13_049, 0, 11_880),
                (12, 0, 5),
                (6, 0, 4),
                (2, 0, 1),
                (18, 0, 12),
                (18, 0, 17),
                (3, 0, 0),
                (2, 0, 0),
                (3, 0, 0),
                (2, 0, 0),
                (2, 0, 1),
                (2, 0, 1),
                (2, 0, 1),
                (2, 0, 1),
            ]
        );
        assert_eq!(battle.player.len(), 1);
        let player = battle.player.member(0).unwrap();
        assert_eq!(player.species.numeric_id(), 1);
        assert_eq!(player.level, 5);
        assert_eq!(player.hp, 20);
        assert_eq!(player.max_hp, 20);
        assert_eq!(
            [
                player.max_hp,
                player.attack,
                player.defense,
                player.special_attack,
                player.special_defense,
                player.speed
            ],
            [20, 10, 10, 12, 12, 10]
        );
        assert_eq!(
            player.moves[..usize::from(player.move_count)]
                .iter()
                .map(|move_| move_.numeric_id())
                .collect::<Vec<_>>(),
            vec![33, 45, 22]
        );
    }

    #[test]
    fn town_wave1_first_move_matches_source_rng_and_next_command_state() {
        let (mut battle, _) =
            pokerogue_core::Battle::town_classic_entry_for_wave("M12TownWave1Action0", "DAY", 1)
                .unwrap();
        battle
            .take_turn(PlayerAction::UseMove(0))
            .expect("the source-supported first move should resolve");

        assert_eq!(
            battle
                .rng_draws()
                .iter()
                .map(|draw| (draw.range, draw.min, draw.value))
                .collect::<Vec<_>>(),
            vec![
                (8, 0, 5),   // source EnemyPokemon.getNextTargets for Tackle
                (100, 0, 9), // player accuracy
                (24, 0, 12), // player critical check
                (16, 85, 90),
                (100, 0, 24), // enemy accuracy
                (24, 0, 9),   // enemy critical check
                (16, 85, 88),
            ]
        );
        assert_eq!(battle.turn, 2);
        assert_eq!((battle.player.hp, battle.enemy.hp), (15, 8));
        assert_eq!(battle.player.move_pp_used[0], 1);
        assert_eq!(battle.enemy.move_pp_used[0], 1);
        assert_eq!(
            battle
                .last_enemy_decision
                .as_ref()
                .unwrap()
                .selected_move
                .numeric_id(),
            33
        );
        assert!(
            !battle
                .last_events
                .iter()
                .any(|event| matches!(event, BattleEvent::CriticalHit { .. }))
        );
    }

    #[test]
    fn mini_run_checkpoint_restores_reward_and_team_before_next_wave() {
        use pokerogue_core::{MiniRun, MiniRunError, MiniRunStage};

        let mut run = MiniRun::new("TownWaveEntrySeed0001", "DAY").unwrap();
        assert!(matches!(
            run.checkpoint(),
            Err(MiniRunError::UnsafeCheckpoint)
        ));
        for _ in 0..20 {
            if run.stage != MiniRunStage::Battle {
                break;
            }
            if run.battle.needs_replacement() {
                run.replace_fainted_player(1).unwrap();
            } else {
                run.take_action(PlayerAction::UseMove(0)).unwrap();
            }
        }
        assert_eq!(run.stage, MiniRunStage::Reward);
        assert_eq!(run.exp_charm_stacks, 0);
        assert_eq!(
            run.reward_experience,
            run.battle.enemy_exp_value().floor() as u32
        );
        for species_id in [10, 19, 506, 819] {
            let species = Species::from_numeric_id(species_id).unwrap();
            run.battle
                .add_player_member(Battler::demo(species, 5, [Move::Tackle, Move::QuickAttack]))
                .unwrap();
        }
        assert_eq!(run.battle.player.len(), 6);
        let save = run.checkpoint().unwrap();
        assert!(matches!(
            MiniRun::from_checkpoint("POKEROGUE_MINIRUN\t99"),
            Err(MiniRunError::InvalidCheckpoint)
        ));
        assert!(matches!(
            MiniRun::from_checkpoint("POKEROGUE_MINIRUN\t2"),
            Err(MiniRunError::InvalidCheckpoint)
        ));
        assert!(matches!(
            MiniRun::from_checkpoint("POKEROGUE_MINIRUN\t3"),
            Err(MiniRunError::InvalidCheckpoint)
        ));
        assert!(matches!(
            MiniRun::from_checkpoint("corrupt"),
            Err(MiniRunError::InvalidCheckpoint)
        ));
        let mut resumed = MiniRun::from_checkpoint(&save).unwrap();
        assert_eq!(resumed.wave, 1);
        assert_eq!(resumed.stage, MiniRunStage::Reward);
        assert_eq!(resumed.exp_charm_stacks, run.exp_charm_stacks);
        assert_eq!(resumed.reward_experience, run.reward_experience);
        assert_eq!(resumed.battle.player.len(), 6);
        for slot in 0..run.battle.player.len() {
            let expected = run.battle.player.member(slot).unwrap();
            let actual = resumed.battle.player.member(slot).unwrap();
            assert_eq!(
                (actual.hp, actual.exp, actual.level),
                (expected.hp, expected.exp, expected.level)
            );
        }
        let next = MiniRun::new("TownWaveEntrySeed0001", "DAY").unwrap();
        let expected_next =
            pokerogue_core::generate_town_wild_encounter_for_wave(&next.seed, &next.time_of_day, 2)
                .unwrap();
        resumed.continue_after_reward().unwrap();
        assert_eq!(resumed.wave, 2);
        assert_eq!(resumed.battle.player.len(), 6);
        assert_eq!(resumed.exp_charm_stacks, 0);
        assert_eq!(
            resumed.battle.enemy.species.numeric_id(),
            expected_next.selection.species.0
        );
    }

    #[test]
    fn deterministic_ten_wave_run_clears_boss_and_saves() {
        use pokerogue_core::{MiniRun, MiniRunStage};
        let mut run = MiniRun::new("MiniRunSeed0001", "DAY").unwrap();
        let mut resumed = false;
        for _ in 0..500 {
            if run.stage == MiniRunStage::Complete {
                break;
            }
            if run.stage == MiniRunStage::Lost {
                break;
            }
            if run.stage == MiniRunStage::Reward {
                let expected_normal = if run.wave >= 10 { 2 } else { 0 };
                let expected_super = if run.wave >= 20 { 1 } else { 0 };
                assert_eq!(run.exp_charm_stacks, expected_normal);
                assert_eq!(run.super_exp_charm_stacks, expected_super);
                if run.wave % 10 == 0 {
                    let species_id = run.battle.enemy.species.numeric_id();
                    let boss_segments = run.battle.enemy.boss_segments;
                    let save = run.checkpoint().unwrap();
                    run = MiniRun::from_checkpoint(&save).unwrap();
                    assert_eq!(run.wave % 10, 0);
                    assert_eq!(run.stage, MiniRunStage::Reward);
                    assert_eq!(run.battle.enemy.species.numeric_id(), species_id);
                    assert_eq!(run.battle.enemy.boss_segments, boss_segments);
                    assert!(boss_segments >= 2);
                }
                if !resumed {
                    let save = run.checkpoint().unwrap();
                    assert!(save.starts_with("POKEROGUE_MINIRUN\t27\n"));
                    run = MiniRun::from_checkpoint(&save).unwrap();
                    assert_eq!(run.stage, MiniRunStage::Reward);
                    assert_eq!(run.wave, 1);
                    resumed = true;
                }
                if run.reward_claimed {
                    run.continue_after_reward().unwrap();
                    continue;
                }
                if let Some(offer) = run.natural_common_reward_offer() {
                    let mut ranked: Vec<_> = offer.options.iter().enumerate().collect();
                    ranked.sort_by_key(|(_, option)| match option.id {
                        "RARE_CANDY" => 0,
                        "SUPER_POTION" | "HYPER_POTION" | "MAX_POTION" | "POTION" => 1,
                        "BERRY" if option.generated_variant.as_deref() == Some("SITRUS") => 2,
                        "TEMP_STAT_STAGE_BOOSTER" => 3,
                        "ETHER" | "MAX_ETHER" => 4,
                        "LURE" => 5,
                        "POKEBALL" => 6,
                        "NUGGET" => 7,
                        _ => 6,
                    });
                    for (index, option) in ranked {
                        let target = if let Some(reward) = [
                            pokerogue_core::rewards::Reward::Potion,
                            pokerogue_core::rewards::Reward::SuperPotion,
                            pokerogue_core::rewards::Reward::HyperPotion,
                            pokerogue_core::rewards::Reward::MaxPotion,
                        ]
                        .into_iter()
                        .find(|reward| reward.id() == option.id)
                        {
                            (0..run.battle.player.len())
                                .filter_map(|slot| {
                                    let member = run.battle.player.member(slot)?;
                                    Some((slot, reward.heal(member.hp, member.max_hp)? - member.hp))
                                })
                                .max_by_key(|(_, amount)| *amount)
                                .map(|(slot, _)| slot)
                                .unwrap_or(0)
                        } else {
                            0
                        };
                        if run.claim_natural_common_reward(index, target).is_ok() {
                            break;
                        }
                    }
                }
                if run.reward_claimed {
                    run.continue_after_reward().unwrap();
                    continue;
                }
                let best_heal = (0..run.battle.player.len())
                    .filter_map(|slot| {
                        let member = run.battle.player.member(slot)?;
                        Some((
                            slot,
                            pokerogue_core::rewards::Reward::SuperPotion
                                .heal(member.hp, member.max_hp)?
                                - member.hp,
                        ))
                    })
                    .max_by_key(|(_, amount)| *amount);
                if let Some((target, _)) = best_heal {
                    run.claim_reward(2, target).unwrap();
                } else {
                    run.apply_modifier_reward(pokerogue_core::rewards::Reward::RareCandy, 0, None)
                        .unwrap();
                }
                run.continue_after_reward().unwrap();
                continue;
            }
            let active = run.battle.player.active_slot();
            let reserve = 1 - active;
            if run.battle.needs_replacement() {
                assert!(
                    run.battle.player.member(reserve).unwrap().hp > 0,
                    "no forced replacement"
                );
                run.replace_fainted_player(reserve).unwrap();
            } else {
                let member = run.battle.player.member(active).unwrap();
                if member.hp * 4 < member.max_hp
                    && run.battle.player.member(reserve).unwrap().hp > 0
                {
                    run.take_action(PlayerAction::Switch(reserve)).unwrap();
                } else {
                    let target = &run.battle.enemy;
                    let best = (0..member.move_count())
                        .filter(|index| member.move_pp_remaining(*index).unwrap_or(0) > 0)
                        .max_by_key(|index| {
                            calculate_damage(member, target, member.moves[*index], 100).damage
                        })
                        .unwrap_or(0);
                    if let Err(error) = run.take_action(PlayerAction::UseMove(best)) {
                        panic!(
                            "wave {} enemy {} moves {:?}: {error:?}",
                            run.wave,
                            run.battle.enemy.species.name(),
                            run.battle.enemy.moves.map(|movement| movement.id())
                        );
                    }
                }
            }
        }
        assert!(matches!(
            run.stage,
            MiniRunStage::Complete | MiniRunStage::Lost
        ));
        assert_eq!(
            run.stage,
            MiniRunStage::Complete,
            "run ended at wave {}: {} (player HP {}/{})",
            run.wave,
            run.battle.log,
            run.battle
                .player
                .member(run.battle.player.active_slot())
                .unwrap()
                .hp,
            run.battle
                .player
                .member(run.battle.player.active_slot())
                .unwrap()
                .max_hp
        );
        assert_eq!(run.wave, 10);
        assert!(resumed, "run never exercised save/resume");
    }

    #[test]
    fn deterministic_twenty_wave_run_clears_bosses_and_saves() {
        use pokerogue_core::{MiniRun, MiniRunStage};
        let mut run = MiniRun::new_with_wave_limit("MiniRunSeed0001", "DAY", 20).unwrap();
        let mut resumed = false;
        let mut great_recovery_claimed = false;
        for _ in 0..500 {
            if run.stage == MiniRunStage::Complete {
                break;
            }
            if run.stage == MiniRunStage::Lost {
                break;
            }
            if run.stage == MiniRunStage::Reward {
                let expected_normal = if run.wave >= 10 { 2 } else { 0 };
                let expected_super = if run.wave >= 20 { 1 } else { 0 };
                assert_eq!(run.exp_charm_stacks, expected_normal);
                assert_eq!(run.super_exp_charm_stacks, expected_super);
                if run.wave % 10 == 0 {
                    let species_id = run.battle.enemy.species.numeric_id();
                    let boss_segments = run.battle.enemy.boss_segments;
                    let save = run.checkpoint().unwrap();
                    run = MiniRun::from_checkpoint(&save).unwrap();
                    assert_eq!(run.wave % 10, 0);
                    assert_eq!(run.stage, MiniRunStage::Reward);
                    assert_eq!(run.battle.enemy.species.numeric_id(), species_id);
                    assert_eq!(run.battle.enemy.boss_segments, boss_segments);
                    assert!(boss_segments >= 2);
                }
                if !resumed {
                    let save = run.checkpoint().unwrap();
                    assert!(save.starts_with("POKEROGUE_MINIRUN\t27\n"));
                    run = MiniRun::from_checkpoint(&save).unwrap();
                    assert_eq!(run.stage, MiniRunStage::Reward);
                    assert_eq!(run.wave, 1);
                    resumed = true;
                }
                if run.reward_claimed {
                    run.continue_after_reward().unwrap();
                    continue;
                }
                if let Some(offer) = run.natural_supported_reward_offer() {
                    let mut ranked: Vec<_> = offer.options.iter().enumerate().collect();
                    ranked.sort_by_key(|(_, option)| match option.id {
                        "RARE_CANDY" | "REVIVE" | "MAX_REVIVE" => 0,
                        "SUPER_POTION" | "HYPER_POTION" | "MAX_POTION" | "POTION" => 1,
                        "BERRY" if option.generated_variant.as_deref() == Some("SITRUS") => 2,
                        "TEMP_STAT_STAGE_BOOSTER" => 3,
                        "ETHER" | "MAX_ETHER" => 4,
                        "LURE" => 5,
                        "POKEBALL" => 6,
                        _ => 6,
                    });
                    for (index, option) in ranked {
                        if !matches!(
                            option.id,
                            "REVIVE"
                                | "MAX_REVIVE"
                                | "POTION"
                                | "SUPER_POTION"
                                | "HYPER_POTION"
                                | "MAX_POTION"
                        ) {
                            continue;
                        }
                        let target = if matches!(option.id, "REVIVE" | "MAX_REVIVE") {
                            (0..run.battle.player.len())
                                .find(|slot| {
                                    run.battle
                                        .player
                                        .member(*slot)
                                        .is_some_and(|member| member.hp == 0)
                                })
                                .unwrap_or(0)
                        } else if let Some(reward) = [
                            pokerogue_core::rewards::Reward::Potion,
                            pokerogue_core::rewards::Reward::SuperPotion,
                            pokerogue_core::rewards::Reward::HyperPotion,
                            pokerogue_core::rewards::Reward::MaxPotion,
                        ]
                        .into_iter()
                        .find(|reward| reward.id() == option.id)
                        {
                            (0..run.battle.player.len())
                                .filter_map(|slot| {
                                    let member = run.battle.player.member(slot)?;
                                    Some((slot, reward.heal(member.hp, member.max_hp)? - member.hp))
                                })
                                .max_by_key(|(_, amount)| *amount)
                                .map(|(slot, _)| slot)
                                .unwrap_or(0)
                        } else {
                            0
                        };
                        if run.claim_natural_supported_reward(index, target).is_ok() {
                            great_recovery_claimed |= option.tier == 1
                                && matches!(
                                    option.id,
                                    "REVIVE" | "MAX_REVIVE" | "HYPER_POTION" | "MAX_POTION"
                                );
                            break;
                        }
                    }
                }
                if run.reward_claimed {
                    run.continue_after_reward().unwrap();
                    continue;
                }
                let best_heal = (0..run.battle.player.len())
                    .filter_map(|slot| {
                        let member = run.battle.player.member(slot)?;
                        Some((
                            slot,
                            pokerogue_core::rewards::Reward::SuperPotion
                                .heal(member.hp, member.max_hp)?
                                - member.hp,
                        ))
                    })
                    .max_by_key(|(_, amount)| *amount);
                if let Some((target, _)) = best_heal {
                    run.claim_reward(2, target).unwrap();
                } else {
                    run.apply_modifier_reward(pokerogue_core::rewards::Reward::RareCandy, 0, None)
                        .unwrap();
                }
                run.continue_after_reward().unwrap();
                continue;
            }
            let active = run.battle.player.active_slot();
            let reserve = 1 - active;
            if run.battle.needs_replacement() {
                assert!(
                    run.battle.player.member(reserve).unwrap().hp > 0,
                    "no forced replacement"
                );
                run.replace_fainted_player(reserve).unwrap();
            } else {
                let member = run.battle.player.member(active).unwrap();
                if member.hp * 4 < member.max_hp
                    && run.battle.player.member(reserve).unwrap().hp > 0
                {
                    run.take_action(PlayerAction::Switch(reserve)).unwrap();
                } else {
                    let target = &run.battle.enemy;
                    let best = (0..member.move_count())
                        .filter(|index| member.move_pp_remaining(*index).unwrap_or(0) > 0)
                        .max_by_key(|index| {
                            calculate_damage(member, target, member.moves[*index], 100).damage
                        })
                        .unwrap_or(0);
                    if let Err(error) = run.take_action(PlayerAction::UseMove(best)) {
                        panic!(
                            "wave {} enemy {} moves {:?}: {error:?}",
                            run.wave,
                            run.battle.enemy.species.name(),
                            run.battle.enemy.moves.map(|movement| movement.id())
                        );
                    }
                }
            }
        }
        assert!(matches!(
            run.stage,
            MiniRunStage::Complete | MiniRunStage::Lost
        ));
        if run.stage != MiniRunStage::Complete {
            eprintln!(
                "route failure: wave={} enemy={} enemy_hp={}/{} turn={}",
                run.wave,
                run.battle.enemy.species.name(),
                run.battle.enemy.hp,
                run.battle.enemy.max_hp,
                run.battle.turn
            );
            for slot in 0..run.battle.player.len() {
                let member = run.battle.player.member(slot).unwrap();
                eprintln!(
                    "party[{slot}] {} level={} hp={}/{} pp={:?}",
                    member.species.name(),
                    member.level,
                    member.hp,
                    member.max_hp,
                    (0..member.move_count())
                        .map(|index| member.move_pp_remaining(index))
                        .collect::<Vec<_>>()
                );
            }
        }
        assert_eq!(
            run.stage,
            MiniRunStage::Complete,
            "20-wave run ended at wave {}: {} (active HP {}/{})",
            run.wave,
            run.battle.log,
            run.battle
                .player
                .member(run.battle.player.active_slot())
                .unwrap()
                .hp,
            run.battle
                .player
                .member(run.battle.player.active_slot())
                .unwrap()
                .max_hp
        );
        assert_eq!(run.wave, 20);
        assert!(resumed, "run never exercised save/resume");
        assert!(
            great_recovery_claimed,
            "run did not claim a GREAT recovery item"
        );
    }

    #[test]
    fn ordinary_ball_is_blocked_until_source_boss_shield_is_broken() {
        let source_blocked = include_str!("../fixtures/boss-ball-gate.csv")
            .lines()
            .find(|row| row.starts_with("ordinary-boss-shield,"))
            .unwrap();
        let fields: Vec<_> = source_blocked.split(',').collect();
        assert_eq!(fields[7], "0");
        assert_eq!(fields[8], "battle:noPokeballStrong");
        assert!(fields[9].is_empty(), "source gate must not draw RNG");
        let (_battle, encounter) = pokerogue_core::Battle::town_classic_entry_for_wave(
            "BossCatchRestrictionSeed",
            "DAY",
            10,
        )
        .unwrap();
        assert!(encounter.boss_segments >= 2);
        let mut battle = Battle::demo_with_seed("BossCatchRestrictionSeed");
        battle.enemy = encounter.battler;
        let initial_draws = battle.rng_draws().len();
        assert_eq!(
            battle.take_turn(PlayerAction::ThrowPokeball),
            Err(TurnActionError::BossNotCatchable)
        );
        assert_eq!(battle.rng_draws().len(), initial_draws);
        assert_eq!(battle.turn, 1);

        battle.enemy.boss_segment_index = 0;
        assert!(battle.take_turn(PlayerAction::ThrowPokeball).is_ok());
        let source_open = include_str!("../fixtures/boss-ball-gate.csv")
            .lines()
            .find(|row| row.starts_with("ordinary-boss-last-segment,"))
            .unwrap();
        assert_eq!(source_open.split(',').nth(7), Some("1"));
    }

    fn total_exp_for_test_level() -> u32 {
        pokerogue_data::total_exp_for_level(5, Species::Bulbasaur.growth_rate())
    }
}
