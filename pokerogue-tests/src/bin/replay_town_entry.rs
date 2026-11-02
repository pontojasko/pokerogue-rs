use pokerogue_core::{Battle, BattleActor, BattleEvent, PlayerAction};
use std::env;

fn main() {
    let mut args = env::args().skip(1);
    let seed = args.next().unwrap_or_else(|| "M12TownWave1Replay6".into());
    let time = args.next().unwrap_or_else(|| "DAY".into());
    let wave = args.next().and_then(|v| v.parse::<u32>().ok()).unwrap_or(1);
    let action = args.next();
    let (mut battle, encounter) = Battle::town_classic_entry_for_wave(&seed, &time, wave)
        .unwrap_or_else(|error| panic!("Rust entry generation failed: {error:?}"));
    let battler = &encounter.battler;
    let moves = battle.enemy.moves[..usize::from(battle.enemy.move_count)]
        .iter()
        .enumerate()
        .map(|(index, m)| {
            format!(
                "{{\"id\":{},\"name\":\"{}\",\"pp\":{},\"pp_used\":{},\"max_pp\":{}}}",
                m.numeric_id(),
                m.name(),
                battle.enemy.move_pp_remaining(index).unwrap_or(0),
                battle.enemy.move_pp_used[index],
                m.pp()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let draws = encounter
        .encounter_rng_trace
        .iter()
        .map(|d| {
            format!(
                "{{\"range\":{},\"min\":{},\"value\":{}}}",
                d.range, d.min, d.value
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let entry_draws = encounter
        .entry_rng_draws
        .iter()
        .map(|d| {
            format!(
                "{{\"turn\":{},\"range\":{},\"min\":{},\"value\":{}}}",
                d.turn, d.range, d.min, d.value
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let battle_seed_draws = encounter
        .battle_seed_rng_draws
        .iter()
        .map(|d| {
            format!(
                "{{\"range\":{},\"min\":{},\"value\":{}}}",
                d.range, d.min, d.value
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let player_party = (0..battle.player.len())
        .filter_map(|slot| battle.player.member(slot))
        .map(|member| {
            let moves = member.moves[..usize::from(member.move_count)]
                .iter()
                .enumerate()
                .map(|(index, movement)| {
                    format!(
                        "{{\"id\":{},\"pp\":{},\"pp_used\":{},\"max_pp\":{}}}",
                        movement.numeric_id(), member.move_pp_remaining(index).unwrap_or(0),
                        member.move_pp_used[index], movement.pp()
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"species_id\":{},\"level\":{},\"hp\":{},\"max_hp\":{},\"stats\":[{},{},{},{},{},{}],\"moves\":[{}]}}",
                member.species.numeric_id(), member.level, member.hp, member.max_hp,
                member.max_hp, member.attack, member.defense, member.special_attack,
                member.special_defense, member.speed, moves
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let candidates = encounter
        .selection
        .candidates
        .iter()
        .map(|(id, name)| format!("{{\"id\":{},\"name\":\"{}\"}}", id, name))
        .collect::<Vec<_>>()
        .join(",");
    let available = encounter
        .available_level_moves
        .iter()
        .map(|name| format!("\"{}\"", name))
        .collect::<Vec<_>>()
        .join(",");
    let mut opponents = vec![format!(
        "{{\"species_id\":{},\"level\":{},\"ivs\":[{},{},{},{},{},{}],\"nature\":{},\"ability_id\":{},\"hp\":{},\"max_hp\":{},\"stats\":[{},{},{},{},{},{}],\"moves\":[{}]}}",
        battler.species.numeric_id(),
        encounter.level,
        encounter.generated_ivs[0],
        encounter.generated_ivs[1],
        encounter.generated_ivs[2],
        encounter.generated_ivs[3],
        encounter.generated_ivs[4],
        encounter.generated_ivs[5],
        encounter.generated_nature,
        encounter.generated_ability_id,
        battler.hp,
        battler.max_hp,
        battler.max_hp,
        battler.attack,
        battler.defense,
        battler.special_attack,
        battler.special_defense,
        battler.speed,
        moves
    )];
    opponents.extend(encounter.additional_opponents.iter().map(|opponent| {
        let battler = &opponent.battler;
        let moves = battler.moves[..usize::from(battler.move_count)]
            .iter()
            .enumerate()
            .map(|(index, m)| format!(
                "{{\"id\":{},\"name\":\"{}\",\"pp\":{},\"pp_used\":{},\"max_pp\":{}}}",
                m.numeric_id(), m.name(), battler.move_pp_remaining(index).unwrap_or(0),
                battler.move_pp_used[index], m.pp()
            ))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"species_id\":{},\"level\":{},\"ivs\":[{},{},{},{},{},{}],\"nature\":{},\"ability_id\":{},\"hp\":{},\"max_hp\":{},\"stats\":[{},{},{},{},{},{}],\"moves\":[{}]}}",
            opponent.species_id, opponent.level,
            opponent.generated_ivs[0], opponent.generated_ivs[1], opponent.generated_ivs[2], opponent.generated_ivs[3], opponent.generated_ivs[4], opponent.generated_ivs[5],
            opponent.generated_nature, opponent.generated_ability_id, battler.hp, battler.max_hp,
            battler.max_hp, battler.attack, battler.defense, battler.special_attack, battler.special_defense, battler.speed, moves
        )
    }));
    let opponents = opponents.join(",");
    let action_result = match action.as_deref() {
        None => "null".to_string(),
        Some("first-move") => {
            let selected_move_id = battle.player.member(0).unwrap().moves[0].numeric_id();
            battle
                .take_turn(PlayerAction::UseMove(0))
                .unwrap_or_else(|error| panic!("Rust replay action failed: {error:?}"));
            let action_events = battle
                .last_events
                .iter()
                .map(|event| match event {
                    BattleEvent::MoveExecuted {
                        actor: BattleActor::Player,
                        move_,
                    } => {
                        format!("\"player_move:{}\"", move_.numeric_id())
                    }
                    BattleEvent::MoveExecuted {
                        actor: BattleActor::Enemy,
                        move_,
                    } => {
                        format!("\"enemy_move:{}\"", move_.numeric_id())
                    }
                    BattleEvent::TurnEnded { turn } => format!("\"turn_end:{turn}\""),
                    BattleEvent::PokemonFainted { slot } => format!("\"faint:{slot}\""),
                    other => format!("\"{other:?}\""),
                })
                .collect::<Vec<_>>()
                .join(",");
            let rng_draws = battle
                .rng_draws()
                .iter()
                .map(|draw| {
                    format!(
                        "{{\"turn\":{},\"range\":{},\"min\":{},\"value\":{}}}",
                        draw.turn, draw.range, draw.min, draw.value
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            let move_actions = battle
                .last_events
                .iter()
                .filter_map(|event| match event {
                    BattleEvent::MoveExecuted {
                        actor: BattleActor::Player,
                        move_,
                    } => Some(format!(
                        "{{\"actor\":\"player\",\"move_id\":{},\"target\":\"enemy\"}}",
                        move_.numeric_id()
                    )),
                    BattleEvent::MoveExecuted {
                        actor: BattleActor::Enemy,
                        move_,
                    } => Some(format!(
                        "{{\"actor\":\"enemy\",\"move_id\":{},\"target\":\"player\"}}",
                        move_.numeric_id()
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(",");
            let player = battle.player.member(0).unwrap();
            let enemy_move = battle
                .last_enemy_decision
                .as_ref()
                .map(|decision| decision.selected_move.numeric_id().to_string())
                .unwrap_or_else(|| "null".to_string());
            let enemy_move_pp_used = battle.enemy.move_pp_used[0];
            let enemy_move_pp_remaining = battle.enemy.move_pp_remaining(0).unwrap_or(0);
            format!(
                "{{\"request\":\"first-move\",\"selected_move_id\":{selected_move_id},\"turn\":{},\"player_hp\":{},\"player_max_hp\":{},\"player_move_pp_used\":{},\"player_move_pp_remaining\":{},\"enemy_hp\":{},\"enemy_max_hp\":{},\"enemy_move_id\":{enemy_move},\"enemy_move_pp_used\":{enemy_move_pp_used},\"enemy_move_pp_remaining\":{enemy_move_pp_remaining},\"events\":[{action_events}],\"move_actions\":[{move_actions}],\"rng_draws\":[{rng_draws}]}}",
                battle.turn,
                player.hp,
                player.max_hp,
                player.move_pp_used[0],
                battle.player.move_pp_remaining(0).unwrap_or(0),
                battle.enemy.hp,
                battle.enemy.max_hp
            )
        }
        Some(other) => panic!("Unsupported Rust replay action {other}"),
    };
    println!(
        "{{\"schema\":\"pokerust-rust-entry/v1\",\"scenario\":{{\"seed\":\"{}\",\"time_of_day\":\"{}\",\"wave\":{}}},\"observed\":{{\"battle_seed\":\"{}\",\"battle_seed_rng_draws\":[{}],\"wave_index\":{},\"double\":{},\"double_check_rng_draws\":[{}],\"species_id\":{},\"species\":\"{}\",\"tier\":\"{}\",\"tier_roll\":{},\"selected_index\":{},\"candidates\":[{}],\"level\":{},\"boss_segments\":{},\"hp\":{},\"max_hp\":{},\"stats\":[{},{},{},{},{},{}],\"moves\":[{}],\"available_level_moves\":[{}],\"encounter_rng_draws\":[{}],\"generated_id\":{},\"ivs\":[{},{},{},{},{},{}],\"nature\":{},\"ability_index\":{},\"ability_id\":{},\"opponent_party_cardinality\":{},\"opponents\":[{}],\"player_party\":[{}],\"level_random_values\":[{}],\"is_mystery_encounter\":{},\"action_result\":{}}},\"missing\":[\"phase-manager execution\"]}}",
        seed,
        time,
        wave,
        encounter.battle_seed,
        battle_seed_draws,
        wave,
        encounter
            .double
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".into()),
        entry_draws,
        battler.species.numeric_id(),
        battler.species.name(),
        encounter.selection.tier,
        encounter.selection.tier_roll,
        encounter.selection.selected_index,
        candidates,
        encounter.level,
        encounter.boss_segments,
        battler.hp,
        battler.max_hp,
        battler.max_hp,
        battler.attack,
        battler.defense,
        battler.special_attack,
        battler.special_defense,
        battler.speed,
        moves,
        available,
        draws,
        encounter.generated_id,
        encounter.generated_ivs[0],
        encounter.generated_ivs[1],
        encounter.generated_ivs[2],
        encounter.generated_ivs[3],
        encounter.generated_ivs[4],
        encounter.generated_ivs[5],
        encounter.generated_nature,
        encounter.generated_ability_index,
        encounter.generated_ability_id,
        1 + encounter.additional_opponents.len(),
        opponents,
        player_party,
        encounter
            .level_random_values
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(","),
        encounter.is_mystery_encounter,
        action_result,
    );
}
