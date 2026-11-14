use mc_data::{CookingKind, ItemId, ItemStack};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BlockEntityData {
    Chest {
        items: Vec<ItemStack>,
    },
    Barrel {
        items: Vec<ItemStack>,
    },
    ShulkerBox {
        items: Vec<ItemStack>,
    },
    Furnace {
        kind: CookingKindDef,
        input: ItemStack,
        fuel: ItemStack,
        output: ItemStack,
        burn_time: u16,
        burn_duration: u16,
        cook_time: u16,
        cook_total: u16,
    },
    BrewingStand {
        bottles: [ItemStack; 3],
        ingredient: ItemStack,
        fuel: ItemStack,
        brew_time: u16,
        fuel_charge: u8,
    },
    Hopper {
        items: Vec<ItemStack>,
        cooldown: u8,
    },
    Dispenser {
        items: Vec<ItemStack>,
    },
    Sign {
        lines: [String; 4],
    },
    Spawner {
        entity_type: String,
        delay: u16,
    },
    Beacon {
        level: u8,
        primary_effect: Option<String>,
    },
    Jukebox {
        record: Option<ItemId>,
    },
    EnchantingTable,
    Anvil,
    Bed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CookingKindDef {
    Smelting,
    Blasting,
    Smoking,
}

impl From<CookingKindDef> for CookingKind {
    fn from(value: CookingKindDef) -> Self {
        match value {
            CookingKindDef::Smelting => CookingKind::Smelting,
            CookingKindDef::Blasting => CookingKind::Blasting,
            CookingKindDef::Smoking => CookingKind::Smoking,
        }
    }
}

impl BlockEntityData {
    pub fn new_chest() -> Self {
        Self::Chest {
            items: vec![ItemStack::EMPTY; 27],
        }
    }

    pub fn new_barrel() -> Self {
        Self::Barrel {
            items: vec![ItemStack::EMPTY; 27],
        }
    }

    pub fn new_furnace(kind: CookingKindDef) -> Self {
        Self::Furnace {
            kind,
            input: ItemStack::EMPTY,
            fuel: ItemStack::EMPTY,
            output: ItemStack::EMPTY,
            burn_time: 0,
            burn_duration: 0,
            cook_time: 0,
            cook_total: if kind == CookingKindDef::Smelting { 200 } else { 100 },
        }
    }

    pub fn new_brewing_stand() -> Self {
        Self::BrewingStand {
            bottles: [ItemStack::EMPTY, ItemStack::EMPTY, ItemStack::EMPTY],
            ingredient: ItemStack::EMPTY,
            fuel: ItemStack::EMPTY,
            brew_time: 0,
            fuel_charge: 0,
        }
    }

    pub fn new_hopper() -> Self {
        Self::Hopper {
            items: vec![ItemStack::EMPTY; 5],
            cooldown: 0,
        }
    }

    pub fn new_dispenser() -> Self {
        Self::Dispenser {
            items: vec![ItemStack::EMPTY; 9],
        }
    }

    pub fn container_fullness_signal(&self) -> u8 {
        let items: &[ItemStack] = match self {
            Self::Chest { items }
            | Self::Barrel { items }
            | Self::ShulkerBox { items }
            | Self::Hopper { items, .. }
            | Self::Dispenser { items } => items,
            Self::Furnace {
                input,
                fuel,
                output,
                ..
            } => {
                let mut sum = 0.0f32;
                for s in [input, fuel, output] {
                    if !s.is_empty() {
                        sum += s.count as f32 / 64.0;
                    }
                }
                if sum == 0.0 {
                    return 0;
                }
                return (1.0 + (sum / 3.0) * 14.0).floor().min(15.0) as u8;
            }
            _ => return 0,
        };
        if items.is_empty() {
            return 0;
        }
        let mut sum = 0.0f32;
        let mut any = false;
        for s in items {
            if !s.is_empty() {
                any = true;
                sum += s.count as f32 / 64.0;
            }
        }
        if !any {
            0
        } else {
            (1.0 + (sum / items.len() as f32) * 14.0).floor().min(15.0) as u8
        }
    }
}
