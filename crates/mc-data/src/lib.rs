pub mod blocks;
pub mod items;
pub mod loot;
pub mod recipes;
pub mod tags;
pub mod world_data;

pub use blocks::{
    BlockDef, BlockId, BlockRegistry, BlockShape, BlockState, FaceTextures, TintKind, ToolKind,
    ToolTier,
};
pub use items::{ArmorSlot, ItemDef, ItemId, ItemRegistry, ItemStack};
pub use loot::{LootEngine, LootContext};
pub use recipes::{CookingKind, RecipeEngine, RecipeMatch};
pub use tags::TagRegistry;
pub use world_data::{AdvancementDef, BiomeDef, DamageTypeDef, EnchantmentDef, VillagerTradeDef, WorldDataRegistry};

use std::sync::Arc;

/// Complete, immutable, reverse-engineered Minecraft 26.1.2 game database.
#[derive(Debug)]
pub struct GameData {
    pub tags: TagRegistry,
    pub blocks: BlockRegistry,
    pub items: ItemRegistry,
    pub recipes: RecipeEngine,
    pub loot: LootEngine,
    pub world: WorldDataRegistry,
}

impl GameData {
    pub fn load() -> Arc<Self> {
        let tags = TagRegistry::from_embedded();
        let blocks = BlockRegistry::from_embedded(&tags);
        let items = ItemRegistry::from_embedded(&blocks);
        let recipes = RecipeEngine::from_embedded(&items, &tags);
        let loot = LootEngine::from_embedded(&items, &tags);
        let world = WorldDataRegistry::from_embedded();
        Arc::new(Self {
            tags,
            blocks,
            items,
            recipes,
            loot,
            world,
        })
    }
}
