use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct BiomeDef {
    pub name: String,
    pub temperature: f32,
    pub downfall: f32,
    pub has_precipitation: bool,
    pub sky_color: [u8; 3],
    pub fog_color: [u8; 3],
    pub water_color: [u8; 3],
    pub grass_color: Option<[u8; 3]>,
    pub foliage_color: Option<[u8; 3]>,
}

#[derive(Debug, Clone)]
pub struct EnchantmentDef {
    pub name: String,
    pub max_level: u8,
    pub weight: u8,
    pub anvil_cost: u8,
}

#[derive(Debug, Clone)]
pub struct DamageTypeDef {
    pub name: String,
    pub exhaustion: f32,
    pub scaling: String,
}

#[derive(Debug, Clone)]
pub struct VillagerTradeDef {
    pub id: String,
    pub wants_item: String,
    pub wants_count: u8,
    pub gives_item: String,
    pub gives_count: u8,
    pub max_uses: u32,
    pub villager_xp: u32,
}

#[derive(Debug, Clone)]
pub struct AdvancementDef {
    pub id: String,
    pub title: String,
    pub description: String,
    pub parent: Option<String>,
}

#[derive(Debug)]
pub struct WorldDataRegistry {
    pub biomes: HashMap<String, BiomeDef>,
    pub enchantments: HashMap<String, EnchantmentDef>,
    pub damage_types: HashMap<String, DamageTypeDef>,
    pub villager_trades: Vec<VillagerTradeDef>,
    pub advancements: HashMap<String, AdvancementDef>,
    pub entity_names: Vec<String>,
    pub effect_names: Vec<String>,
    pub particle_names: Vec<String>,
}

impl WorldDataRegistry {
    pub fn from_embedded() -> Self {
        let reg_val: Value =
            serde_json::from_str(include_str!("../../../data/extracted_26_1_2/registries.json"))
                .expect("valid registries.json");
        let biomes_raw: HashMap<String, Value> =
            serde_json::from_str(include_str!("../../../data/extracted_26_1_2/biomes.json"))
                .expect("valid biomes.json");
        let ench_raw: HashMap<String, Value> = serde_json::from_str(include_str!(
            "../../../data/extracted_26_1_2/enchantments.json"
        ))
        .expect("valid enchantments.json");
        let dmg_raw: HashMap<String, Value> = serde_json::from_str(include_str!(
            "../../../data/extracted_26_1_2/damage_types.json"
        ))
        .expect("valid damage_types.json");
        let trades_raw: HashMap<String, Value> = serde_json::from_str(include_str!(
            "../../../data/extracted_26_1_2/villager_trades.json"
        ))
        .expect("valid villager_trades.json");
        let adv_raw: HashMap<String, Value> = serde_json::from_str(include_str!(
            "../../../data/extracted_26_1_2/advancements.json"
        ))
        .expect("valid advancements.json");

        let mut biomes = HashMap::with_capacity(biomes_raw.len());
        for (k, v) in biomes_raw {
            let eff = v.get("effects").unwrap_or(&Value::Null);
            biomes.insert(
                k.clone(),
                BiomeDef {
                    name: k,
                    temperature: v.get("temperature").and_then(|x| x.as_f64()).unwrap_or(0.8) as f32,
                    downfall: v.get("downfall").and_then(|x| x.as_f64()).unwrap_or(0.4) as f32,
                    has_precipitation: v
                        .get("has_precipitation")
                        .and_then(|x| x.as_bool())
                        .unwrap_or(true),
                    sky_color: parse_rgb(eff.get("sky_color"), [120, 167, 255]),
                    fog_color: parse_rgb(eff.get("fog_color"), [192, 216, 255]),
                    water_color: parse_rgb(eff.get("water_color"), [63, 118, 228]),
                    grass_color: eff.get("grass_color").map(|c| parse_rgb(Some(c), [121, 192, 90])),
                    foliage_color: eff
                        .get("foliage_color")
                        .map(|c| parse_rgb(Some(c), [89, 174, 48])),
                },
            );
        }

        let mut enchantments = HashMap::with_capacity(ench_raw.len());
        for (k, v) in ench_raw {
            enchantments.insert(
                k.clone(),
                EnchantmentDef {
                    name: k,
                    max_level: v.get("max_level").and_then(|x| x.as_u64()).unwrap_or(1) as u8,
                    weight: v.get("weight").and_then(|x| x.as_u64()).unwrap_or(1) as u8,
                    anvil_cost: v.get("anvil_cost").and_then(|x| x.as_u64()).unwrap_or(1) as u8,
                },
            );
        }

        let mut damage_types = HashMap::with_capacity(dmg_raw.len());
        for (k, v) in dmg_raw {
            damage_types.insert(
                k.clone(),
                DamageTypeDef {
                    name: k,
                    exhaustion: v.get("exhaustion").and_then(|x| x.as_f64()).unwrap_or(0.1) as f32,
                    scaling: v
                        .get("scaling")
                        .and_then(|x| x.as_str())
                        .unwrap_or("when_caused_by_living_non_player")
                        .to_string(),
                },
            );
        }

        let mut villager_trades = Vec::new();
        for (k, v) in trades_raw {
            let wants_item = v
                .pointer("/wants/id")
                .and_then(|x| x.as_str())
                .unwrap_or("minecraft:emerald")
                .strip_prefix("minecraft:")
                .unwrap_or("emerald")
                .to_string();
            let wants_count = v
                .pointer("/wants/count")
                .and_then(|x| x.as_u64())
                .unwrap_or(1) as u8;
            let gives_item = v
                .pointer("/gives/id")
                .and_then(|x| x.as_str())
                .unwrap_or("minecraft:emerald")
                .strip_prefix("minecraft:")
                .unwrap_or("emerald")
                .to_string();
            let gives_count = v
                .pointer("/gives/count")
                .and_then(|x| x.as_u64())
                .unwrap_or(1) as u8;
            villager_trades.push(VillagerTradeDef {
                id: k,
                wants_item,
                wants_count: wants_count.max(1),
                gives_item,
                gives_count: gives_count.max(1),
                max_uses: v.get("max_uses").and_then(|x| x.as_u64()).unwrap_or(12) as u32,
                villager_xp: v.get("xp").and_then(|x| x.as_u64()).unwrap_or(2) as u32,
            });
        }

        let mut advancements = HashMap::new();
        for (k, v) in adv_raw {
            if let Some(disp) = v.get("display") {
                let title = disp
                    .pointer("/title/translate")
                    .and_then(|x| x.as_str())
                    .unwrap_or(&k)
                    .to_string();
                let description = disp
                    .pointer("/description/translate")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let parent = v.get("parent").and_then(|x| x.as_str()).map(|s| s.to_string());
                advancements.insert(
                    k.clone(),
                    AdvancementDef {
                        id: k,
                        title,
                        description,
                        parent,
                    },
                );
            }
        }

        let str_list = |field: &str| -> Vec<String> {
            reg_val[field]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default()
        };

        Self {
            biomes,
            enchantments,
            damage_types,
            villager_trades,
            advancements,
            entity_names: str_list("entities"),
            effect_names: str_list("effects"),
            particle_names: str_list("particles"),
        }
    }
}

fn parse_rgb(val: Option<&Value>, default: [u8; 3]) -> [u8; 3] {
    let Some(v) = val else {
        return default;
    };
    if let Some(n) = v.as_u64() {
        return [((n >> 16) & 0xFF) as u8, ((n >> 8) & 0xFF) as u8, (n & 0xFF) as u8];
    }
    if let Some(s) = v.as_str() {
        let hex = s.trim_start_matches('#');
        if hex.len() >= 6 {
            if let Ok(n) = u32::from_str_radix(&hex[..6], 16) {
                return [((n >> 16) & 0xFF) as u8, ((n >> 8) & 0xFF) as u8, (n & 0xFF) as u8];
            }
        }
    }
    default
}

#[cfg(test)]
mod tests {
    use crate::GameData;
    use crate::items::ItemStack;
    use crate::loot::LootContext;
    use crate::recipes::CookingKind;

    #[test]
    fn embedded_26_1_2_data_loads_completely() {
        let data = GameData::load();
        assert!(data.blocks.len() >= 1100);
        assert!(data.items.len() >= 1400);
        assert!(data.world.biomes.len() >= 60);
        assert!(data.world.enchantments.len() >= 40);
        assert!(data.world.damage_types.len() >= 45);
        assert!(data.loot.table_count() >= 1300);

        // Verify crafting recipe: oak_log -> oak_planks
        let log_id = data.items.by_name("oak_log").unwrap();
        let mut grid = vec![ItemStack::EMPTY; 4];
        grid[0] = ItemStack::new(log_id, 1);
        let m = data.recipes.match_crafting(2, 2, &grid).expect("oak_log -> planks");
        assert_eq!(data.items.get(m.result.item).name, "oak_planks");
        assert_eq!(m.result.count, 4);

        // Verify smelting recipe: raw_iron -> iron_ingot
        let raw_iron = data.items.by_name("raw_iron").unwrap();
        let (smelted, _, _) = data.recipes.match_cooking(CookingKind::Smelting, raw_iron).unwrap();
        assert_eq!(data.items.get(smelted).name, "iron_ingot");

        // Verify loot table: stone drops cobblestone without silk touch
        let mut rng = rand::thread_rng();
        let drops = data.loot.roll_block("stone", &LootContext::default(), &mut rng);
        assert_eq!(data.items.get(drops[0].item).name, "cobblestone");
    }
}
