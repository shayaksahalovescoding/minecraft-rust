use crate::entity::{EntityKind, EntityManager};
use crate::player::Player;
use glam::DVec3;
use mc_core::{BlockPos, Difficulty, GameMode};
use mc_data::{BlockState, GameData, ItemStack};
use mc_world::World;

/// Execute a Minecraft slash command (e.g. `/give`, `/tp`, `/gamemode`, `/setblock`, `/fill`, `/summon`, `/time`, `/weather`, `/seed`, `/effect`, `/enchant`, `/xp`, `/kill`, `/clear`, `/gamerule`, `/difficulty`, `/help`).
pub fn execute_command(
    raw: &str,
    player: &mut Player,
    world: &mut World,
    entities: &mut EntityManager,
    data: &GameData,
) -> String {
    let trimmed = raw.trim().trim_start_matches('/');
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.is_empty() {
        return String::new();
    }

    match parts[0].to_ascii_lowercase().as_str() {
        "help" => "Commands: /gamemode, /give, /tp, /setblock, /fill, /summon, /time, /weather, /seed, /effect, /enchant, /xp, /kill, /clear, /gamerule, /difficulty, /spawnpoint, /say".to_string(),
        "seed" => format!("Seed: [{}]", world.seed),
        "say" => format!("[{}] {}", player.name, parts[1..].join(" ")),
        "gamemode" => {
            let Some(gm) = parts.get(1).and_then(|s| GameMode::from_str(s)) else {
                return "Usage: /gamemode <survival|creative|adventure|spectator>".into();
            };
            player.game_mode = gm;
            if !gm.can_fly() {
                player.is_flying = false;
            }
            format!("Set own game mode to {}", gm.name())
        }
        "difficulty" => {
            let Some(diff) = parts.get(1).and_then(|s| Difficulty::from_str(s)) else {
                return format!("Current difficulty is {}", world.difficulty.name());
            };
            world.difficulty = diff;
            format!("The difficulty has been set to {}", diff.name())
        }
        "time" => {
            match parts.get(1).copied() {
                Some("set") => {
                    let t = match parts.get(2).copied() {
                        Some("day") => 1000,
                        Some("noon") => 6000,
                        Some("night") => 13000,
                        Some("midnight") => 18000,
                        Some(n) => n.parse::<i64>().unwrap_or(world.day_time),
                        None => return "Usage: /time set <day|noon|night|midnight|ticks>".into(),
                    };
                    world.day_time = t.rem_euclid(24000);
                    format!("Set the time to {}", world.day_time)
                }
                Some("add") => {
                    let dt = parts.get(2).and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
                    world.day_time = (world.day_time + dt).rem_euclid(24000);
                    format!("Added {} to the time", dt)
                }
                _ => format!("The time is {}", world.day_time),
            }
        }
        "weather" => {
            match parts.get(1).copied() {
                Some("clear") => {
                    world.is_raining = false;
                    world.is_thundering = false;
                    "Changing to clear weather".into()
                }
                Some("rain") => {
                    world.is_raining = true;
                    world.is_thundering = false;
                    "Changing to rainy weather".into()
                }
                Some("thunder") => {
                    world.is_raining = true;
                    world.is_thundering = true;
                    "Changing to rain and thunder".into()
                }
                _ => "Usage: /weather <clear|rain|thunder>".into(),
            }
        }
        "tp" | "teleport" => {
            if parts.len() < 4 {
                return "Usage: /tp <x> <y> <z>".into();
            }
            let x = parse_rel(parts[1], player.pos.x);
            let y = parse_rel(parts[2], player.pos.y);
            let z = parse_rel(parts[3], player.pos.z);
            player.pos = DVec3::new(x, y, z);
            player.vel = DVec3::ZERO;
            player.fall_distance = 0.0;
            format!("Teleported {} to {:.1}, {:.1}, {:.1}", player.name, x, y, z)
        }
        "give" => {
            // Supports `/give <item> [count]` or `/give @s <item> [count]`
            let (item_arg, count_arg) = if parts.get(1).is_some_and(|s| s.starts_with('@') || *s == player.name) {
                (parts.get(2).copied(), parts.get(3).copied())
            } else {
                (parts.get(1).copied(), parts.get(2).copied())
            };
            let Some(name) = item_arg else {
                return "Usage: /give <item> [count]".into();
            };
            let Some(id) = data.items.by_name(name) else {
                return format!("Unknown item '{name}'");
            };
            let count = count_arg.and_then(|s| s.parse::<u16>().ok()).unwrap_or(1).max(1);
            let max_s = data.items.get(id).max_stack as u16;
            let mut rem = count;
            while rem > 0 {
                let take = rem.min(max_s) as u8;
                player.give_item(ItemStack::new(id, take), data);
                rem -= take as u16;
            }
            format!("Gave {} [{}] to {}", count, data.items.get(id).name, player.name)
        }
        "setblock" => {
            if parts.len() < 5 {
                return "Usage: /setblock <x> <y> <z> <block>".into();
            }
            let bp = BlockPos::new(
                parse_rel(parts[1], player.pos.x).floor() as i32,
                parse_rel(parts[2], player.pos.y).floor() as i32,
                parse_rel(parts[3], player.pos.z).floor() as i32,
            );
            let Some(bid) = data.blocks.by_name(parts[4]) else {
                return format!("Unknown block '{}'", parts[4]);
            };
            world.set_block(bp, BlockState::new(bid), &data.blocks);
            format!("Changed the block at {}, {}, {}", bp.x, bp.y, bp.z)
        }
        "fill" => {
            if parts.len() < 8 {
                return "Usage: /fill <x1> <y1> <z1> <x2> <y2> <z2> <block>".into();
            }
            let x1 = parse_rel(parts[1], player.pos.x).floor() as i32;
            let y1 = parse_rel(parts[2], player.pos.y).floor() as i32;
            let z1 = parse_rel(parts[3], player.pos.z).floor() as i32;
            let x2 = parse_rel(parts[4], player.pos.x).floor() as i32;
            let y2 = parse_rel(parts[5], player.pos.y).floor() as i32;
            let z2 = parse_rel(parts[6], player.pos.z).floor() as i32;
            let Some(bid) = data.blocks.by_name(parts[7]) else {
                return format!("Unknown block '{}'", parts[7]);
            };
            let mut count = 0u32;
            for y in y1.min(y2)..=y1.max(y2) {
                for z in z1.min(z2)..=z1.max(z2) {
                    for x in x1.min(x2)..=x1.max(x2) {
                        if count >= 32768 {
                            break;
                        }
                        world.set_block(BlockPos::new(x, y, z), BlockState::new(bid), &data.blocks);
                        count += 1;
                    }
                }
            }
            format!("Successfully filled {count} blocks")
        }
        "summon" => {
            let Some(name) = parts.get(1) else {
                return "Usage: /summon <entity> [x y z]".into();
            };
            let Some(kind) = EntityKind::from_name(name) else {
                return format!("Unknown entity '{name}'");
            };
            let pos = if parts.len() >= 5 {
                DVec3::new(
                    parse_rel(parts[2], player.pos.x),
                    parse_rel(parts[3], player.pos.y),
                    parse_rel(parts[4], player.pos.z),
                )
            } else {
                player.pos + player.look_dir() * 3.0
            };
            entities.spawn(kind, pos, data);
            format!("Summoned new {}", kind.registry_name())
        }
        "effect" => {
            if parts.get(1) == Some(&"clear") {
                player.effects.clear();
                return "Cleared every effect from player".into();
            }
            let Some(eff) = parts.get(1) else {
                return "Usage: /effect <name|clear> [seconds] [amplifier]".into();
            };
            let secs = parts.get(2).and_then(|s| s.parse::<u32>().ok()).unwrap_or(30);
            let amp = parts.get(3).and_then(|s| s.parse::<u8>().ok()).unwrap_or(0);
            let short = eff.strip_prefix("minecraft:").unwrap_or(eff);
            player.add_effect(short, amp, secs * 20);
            format!("Applied {short} for {secs}s")
        }
        "enchant" => {
            let Some(ench) = parts.get(1) else {
                return "Usage: /enchant <enchantment> [level]".into();
            };
            let lvl = parts.get(2).and_then(|s| s.parse::<u8>().ok()).unwrap_or(1).max(1);
            let short = ench.strip_prefix("minecraft:").unwrap_or(ench);
            if player.main_hand().is_empty() {
                return "Hold an item first".into();
            }
            player.main_hand_mut().enchantments.push((short.to_string(), lvl));
            format!("Applied enchantment {short} {lvl}")
        }
        "xp" | "experience" => {
            let amt = parts.get(1).and_then(|s| s.parse::<u32>().ok()).unwrap_or(100);
            player.add_xp(amt);
            format!("Gave {amt} experience points")
        }
        "clear" => {
            for s in &mut player.inventory {
                *s = ItemStack::EMPTY;
            }
            "Cleared player inventory".into()
        }
        "kill" => {
            if parts.get(1) == Some(&"@e") {
                let n = entities.entities.len();
                entities.entities.clear();
                return format!("Killed {n} entities");
            }
            player.health = 0.0;
            player.dead = true;
            format!("Killed {}", player.name)
        }
        "spawnpoint" => {
            player.spawn_pos = BlockPos::from_vec3(player.pos);
            player.spawn_dim = player.dimension;
            format!("Set spawn point to {}, {}, {}", player.spawn_pos.x, player.spawn_pos.y, player.spawn_pos.z)
        }
        "gamerule" => {
            let Some(rule) = parts.get(1) else {
                return "Usage: /gamerule <keepInventory|doDaylightCycle|doMobSpawning|naturalRegeneration> <true|false>".into();
            };
            let val = parts.get(2).and_then(|s| s.parse::<bool>().ok());
            match (*rule, val) {
                ("keepInventory", Some(v)) => world.game_rules.keep_inventory = v,
                ("doDaylightCycle", Some(v)) => world.game_rules.do_daylight_cycle = v,
                ("doMobSpawning", Some(v)) => world.game_rules.do_mob_spawning = v,
                ("naturalRegeneration", Some(v)) => world.game_rules.natural_regeneration = v,
                ("doWeatherCycle", Some(v)) => world.game_rules.do_weather_cycle = v,
                ("mobGriefing", Some(v)) => world.game_rules.mob_griefing = v,
                _ => return format!("Unknown or invalid gamerule '{rule}'"),
            }
            format!("Gamerule {rule} updated")
        }
        other => format!("Unknown command '{other}'. Type /help for help."),
    }
}

fn parse_rel(s: &str, base: f64) -> f64 {
    if let Some(rest) = s.strip_prefix('~') {
        if rest.is_empty() {
            base
        } else {
            base + rest.parse::<f64>().unwrap_or(0.0)
        }
    } else {
        s.parse::<f64>().unwrap_or(base)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_core::{ChunkPos, DimensionId};
    use mc_world::Chunk;

    #[test]
    fn combat_rules_and_commands_and_player_physics() {
        let data = GameData::load();
        let mut world = World::new(DimensionId::Overworld, 99);
        let mut ch = Chunk::new(ChunkPos::new(0, 0), DimensionId::Overworld);
        for z in 0..16 {
            for x in 0..16 {
                ch.set_block(x, 63, z, BlockState::new(data.blocks.stone));
            }
        }
        world.insert_chunk(ch, &data.blocks);

        let mut player = Player::new("Steve", BlockPos::new(8, 64, 8), GameMode::Survival);
        let mut mgr = EntityManager::new();

        let msg = execute_command("/give diamond_sword 1", &mut player, &mut world, &mut mgr, &data);
        assert!(msg.contains("diamond_sword"));
        assert!(!player.inventory[0].is_empty());

        let msg2 = execute_command("/summon creeper 10 64 10", &mut player, &mut world, &mut mgr, &data);
        assert!(msg2.contains("creeper"));
        assert_eq!(mgr.entities.len(), 1);

        // Diamond armor mitigation check (20 armor, 8 toughness -> 10 * (1 - 17.5/25) = 3.0)
        let dmg = crate::player::CombatRules::get_damage_after_absorb(10.0, 20.0, 8.0);
        assert!((dmg - 3.0).abs() < 1e-5);
    }
}
