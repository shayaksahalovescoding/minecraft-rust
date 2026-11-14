use crate::blocks::{BlockId, BlockRegistry, ToolKind, ToolTier};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ItemId(pub u16);

impl ItemId {
    pub const AIR: Self = Self(0);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArmorSlot {
    Helmet,
    Chestplate,
    Leggings,
    Boots,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ItemStack {
    pub item: ItemId,
    pub count: u8,
    pub damage: u16,
    pub enchantments: Vec<(String, u8)>,
    pub custom_name: Option<String>,
}

impl ItemStack {
    pub const EMPTY: Self = Self {
        item: ItemId::AIR,
        count: 0,
        damage: 0,
        enchantments: Vec::new(),
        custom_name: None,
    };

    pub fn new(item: ItemId, count: u8) -> Self {
        if item == ItemId::AIR || count == 0 {
            Self::EMPTY
        } else {
            Self {
                item,
                count,
                damage: 0,
                enchantments: Vec::new(),
                custom_name: None,
            }
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.item == ItemId::AIR || self.count == 0
    }

    pub fn enchantment_level(&self, id: &str) -> u8 {
        let short = id.strip_prefix("minecraft:").unwrap_or(id);
        self.enchantments
            .iter()
            .find(|(k, _)| k.strip_prefix("minecraft:").unwrap_or(k) == short)
            .map(|(_, lvl)| *lvl)
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone)]
pub struct ItemDef {
    pub id: ItemId,
    pub name: String,
    pub max_stack: u8,
    pub max_durability: u16,
    pub tool_kind: ToolKind,
    pub tool_tier: ToolTier,
    pub mining_speed: f32,
    pub attack_damage: f32,
    pub attack_speed: f32,
    pub armor_slot: Option<ArmorSlot>,
    pub armor_defense: f32,
    pub armor_toughness: f32,
    pub knockback_resistance: f32,
    pub food_nutrition: u8,
    pub food_saturation: f32,
    pub places_block: Option<BlockId>,
    pub fuel_ticks: u16,
    pub texture: Option<String>,
    pub enchantability: u8,
}

#[derive(Debug)]
pub struct ItemRegistry {
    defs: Vec<ItemDef>,
    by_name: HashMap<String, ItemId>,
    pub air: ItemId,
}

impl ItemRegistry {
    pub fn from_embedded(blocks: &BlockRegistry) -> Self {
        let reg_val: Value =
            serde_json::from_str(include_str!("../../../data/extracted_26_1_2/registries.json"))
                .expect("valid registries.json");
        let tex_map: HashMap<String, String> = serde_json::from_str(include_str!(
            "../../../data/extracted_26_1_2/item_textures.json"
        ))
        .expect("valid item_textures.json");

        let mut names: Vec<String> = reg_val["items"]
            .as_array()
            .expect("items array")
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();

        if let Some(pos) = names.iter().position(|n| n == "air") {
            names.swap(0, pos);
        } else {
            names.insert(0, "air".to_string());
        }

        let mut defs = Vec::with_capacity(names.len());
        let mut by_name = HashMap::with_capacity(names.len());

        for (i, name) in names.iter().enumerate() {
            let id = ItemId(i as u16);
            by_name.insert(name.clone(), id);
            let tex = tex_map.get(name).cloned();
            defs.push(classify_item(id, name, tex, blocks));
        }

        Self {
            defs,
            by_name,
            air: ItemId::AIR,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.defs.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    #[inline]
    pub fn get(&self, id: ItemId) -> &ItemDef {
        &self.defs[(id.0 as usize).min(self.defs.len() - 1)]
    }

    #[inline]
    pub fn by_name(&self, name: &str) -> Option<ItemId> {
        let short = name.strip_prefix("minecraft:").unwrap_or(name);
        self.by_name.get(short).copied()
    }

    pub fn all(&self) -> &[ItemDef] {
        &self.defs
    }
}

fn classify_item(id: ItemId, name: &str, texture: Option<String>, blocks: &BlockRegistry) -> ItemDef {
    let places_block = blocks.by_name(name).filter(|b| *b != BlockId::AIR);

    // Exact ToolMaterial values reverse-engineered from Fabric 26.1.2.jar
    let (tier, mat_durability, mat_speed, mat_dmg, mat_ench) = if name.starts_with("wooden_") {
        (ToolTier::Wood, 59u16, 2.0f32, 0.0f32, 15u8)
    } else if name.starts_with("stone_") {
        (ToolTier::Stone, 131, 4.0, 1.0, 5)
    } else if name.starts_with("copper_") {
        (ToolTier::Copper, 190, 5.0, 1.0, 13)
    } else if name.starts_with("iron_") {
        (ToolTier::Iron, 250, 6.0, 2.0, 14)
    } else if name.starts_with("golden_") {
        (ToolTier::Wood, 32, 12.0, 0.0, 22)
    } else if name.starts_with("diamond_") {
        (ToolTier::Diamond, 1561, 8.0, 3.0, 10)
    } else if name.starts_with("netherite_") {
        (ToolTier::Netherite, 2031, 9.0, 4.0, 15)
    } else {
        (ToolTier::Hand, 0, 1.0, 0.0, 0)
    };

    let (tool_kind, mut max_durability, mining_speed, attack_damage, attack_speed, mut enchantability) =
        if name.ends_with("_sword") && mat_durability > 0 {
            (ToolKind::Sword, mat_durability, 1.5, 4.0 + mat_dmg, 1.6, mat_ench)
        } else if name.ends_with("_pickaxe") && mat_durability > 0 {
            (ToolKind::Pickaxe, mat_durability, mat_speed, 2.0 + mat_dmg, 1.2, mat_ench)
        } else if name.ends_with("_axe") && mat_durability > 0 {
            (ToolKind::Axe, mat_durability, mat_speed, 6.0 + mat_dmg, 0.9, mat_ench)
        } else if name.ends_with("_shovel") && mat_durability > 0 {
            (ToolKind::Shovel, mat_durability, mat_speed, 2.5 + mat_dmg, 1.0, mat_ench)
        } else if name.ends_with("_hoe") && mat_durability > 0 {
            (ToolKind::Hoe, mat_durability, mat_speed, 1.0, 2.0, mat_ench)
        } else if name == "shears" {
            (ToolKind::Shears, 238, 5.0, 1.0, 4.0, 0)
        } else if name == "trident" {
            (ToolKind::Sword, 250, 1.0, 9.0, 1.1, 1)
        } else if name == "mace" {
            (ToolKind::Sword, 500, 1.0, 6.0, 0.6, 15)
        } else {
            (ToolKind::None, 0, 1.0, 1.0, 4.0, 0)
        };

    // Exact ArmorMaterials values reverse-engineered from Fabric 26.1.2.jar
    let armor_slot = if name.ends_with("_helmet") || name == "turtle_helmet" {
        Some(ArmorSlot::Helmet)
    } else if name.ends_with("_chestplate") {
        Some(ArmorSlot::Chestplate)
    } else if name.ends_with("_leggings") {
        Some(ArmorSlot::Leggings)
    } else if name.ends_with("_boots") {
        Some(ArmorSlot::Boots)
    } else {
        None
    };

    let (armor_defense, armor_toughness, knockback_resistance) = if let Some(slot) = armor_slot {
        let (mult, boots, legs, chest, helm, ench, tough, kb) = if name.starts_with("leather_") {
            (5u16, 1.0f32, 2.0, 3.0, 1.0, 15u8, 0.0f32, 0.0f32)
        } else if name.starts_with("copper_") {
            (11, 1.0, 3.0, 4.0, 2.0, 8, 0.0, 0.0)
        } else if name.starts_with("chainmail_") {
            (15, 1.0, 4.0, 5.0, 2.0, 12, 0.0, 0.0)
        } else if name.starts_with("iron_") {
            (15, 2.0, 5.0, 6.0, 2.0, 9, 0.0, 0.0)
        } else if name.starts_with("golden_") {
            (7, 1.0, 3.0, 5.0, 2.0, 25, 0.0, 0.0)
        } else if name.starts_with("diamond_") {
            (33, 3.0, 6.0, 8.0, 3.0, 10, 2.0, 0.0)
        } else if name.starts_with("netherite_") {
            (37, 3.0, 6.0, 8.0, 3.0, 15, 3.0, 0.1)
        } else if name == "turtle_helmet" {
            (25, 2.0, 5.0, 6.0, 2.0, 9, 0.0, 0.0)
        } else {
            (5, 1.0, 2.0, 3.0, 1.0, 10, 0.0, 0.0)
        };
        let base_slot_dur = match slot {
            ArmorSlot::Helmet => 11,
            ArmorSlot::Chestplate => 16,
            ArmorSlot::Leggings => 15,
            ArmorSlot::Boots => 13,
        };
        max_durability = base_slot_dur * mult;
        enchantability = ench;
        let def = match slot {
            ArmorSlot::Boots => boots,
            ArmorSlot::Leggings => legs,
            ArmorSlot::Chestplate => chest,
            ArmorSlot::Helmet => helm,
        };
        (def, tough, kb)
    } else {
        (0.0, 0.0, 0.0)
    };

    if name == "bow" {
        max_durability = 384;
        enchantability = 1;
    } else if name == "crossbow" {
        max_durability = 465;
        enchantability = 1;
    } else if name == "shield" {
        max_durability = 336;
    } else if name == "elytra" {
        max_durability = 432;
    } else if name == "flint_and_steel" {
        max_durability = 64;
    } else if name == "fishing_rod" {
        max_durability = 64;
        enchantability = 1;
    }

    let (food_nutrition, food_saturation) = match name {
        "apple" => (4, 2.4),
        "golden_apple" => (4, 9.6),
        "enchanted_golden_apple" => (4, 9.6),
        "bread" => (5, 6.0),
        "cooked_beef" | "cooked_porkchop" => (8, 12.8),
        "beef" | "porkchop" => (3, 1.8),
        "cooked_chicken" => (6, 7.2),
        "chicken" => (2, 1.2),
        "cooked_mutton" => (6, 9.6),
        "mutton" => (2, 1.2),
        "cooked_cod" => (5, 6.0),
        "cooked_salmon" => (6, 9.6),
        "carrot" => (3, 3.6),
        "golden_carrot" => (6, 14.4),
        "baked_potato" => (5, 6.0),
        "potato" => (1, 0.6),
        "melon_slice" => (2, 1.2),
        "sweet_berries" | "glow_berries" => (2, 0.4),
        "cookie" => (2, 0.4),
        "pumpkin_pie" => (8, 4.8),
        "mushroom_stew" | "rabbit_stew" => (6, 7.2),
        "rotten_flesh" => (4, 0.8),
        _ => (0, 0.0),
    };

    let fuel_ticks = match name {
        "lava_bucket" => 20000,
        "coal_block" => 16000,
        "dried_kelp_block" => 4000,
        "blaze_rod" => 2400,
        "coal" | "charcoal" => 1600,
        s if s.ends_with("_boat") => 1200,
        s if s.ends_with("_log") || s.ends_with("_wood") || s.ends_with("_planks") => 300,
        "crafting_table" | "chest" | "bookshelf" | "barrel" => 300,
        "stick" | "bamboo" => 100,
        s if s.ends_with("_sapling") || s.ends_with("_wool") => 100,
        _ => 0,
    };

    let max_stack = if max_durability > 0
        || name.ends_with("_bucket")
        || matches!(
            name,
            "bucket"
                | "water_bucket"
                | "lava_bucket"
                | "milk_bucket"
                | "saddle"
                | "mushroom_stew"
                | "rabbit_stew"
                | "potion"
                | "splash_potion"
                | "lingering_potion"
                | "enchanted_book"
                | "totem_of_undying"
        )
        || name.ends_with("_bed")
        || name.ends_with("_boat")
        || name.ends_with("minecart")
    {
        1
    } else if matches!(
        name,
        "snowball" | "ender_pearl" | "egg" | "honey_bottle" | "armor_stand"
    ) || name.ends_with("_sign")
        || name.ends_with("_banner")
    {
        16
    } else {
        64
    };

    ItemDef {
        id,
        name: name.to_string(),
        max_stack,
        max_durability,
        tool_kind,
        tool_tier: tier,
        mining_speed,
        attack_damage,
        attack_speed,
        armor_slot,
        armor_defense,
        armor_toughness,
        knockback_resistance,
        food_nutrition,
        food_saturation,
        places_block,
        fuel_ticks,
        texture,
        enchantability,
    }
}
