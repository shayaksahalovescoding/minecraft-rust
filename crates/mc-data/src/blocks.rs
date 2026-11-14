use crate::tags::TagRegistry;
use mc_core::Direction;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct BlockId(pub u16);

impl BlockId {
    pub const AIR: Self = Self(0);
}

/// Compact 32-bit BlockState: lower 16 bits = BlockId, upper 16 bits = state properties.
/// State bits layout:
/// - bits 0..3: facing (0=Down,1=Up,2=North,3=South,4=West,5=East) or axis (0=Y,1=X,2=Z)
/// - bit 3: lit / open / powered
/// - bits 4..8: level / age / power (0..15)
/// - bit 8: waterlogged
/// - bit 9: half (0=bottom/lower, 1=top/upper)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct BlockState(pub u32);

impl BlockState {
    pub const AIR: Self = Self(0);

    #[inline]
    pub const fn new(id: BlockId) -> Self {
        Self(id.0 as u32)
    }

    #[inline]
    pub const fn with_meta(id: BlockId, meta: u16) -> Self {
        Self((id.0 as u32) | ((meta as u32) << 16))
    }

    #[inline]
    pub const fn id(self) -> BlockId {
        BlockId((self.0 & 0xFFFF) as u16)
    }

    #[inline]
    pub const fn meta(self) -> u16 {
        (self.0 >> 16) as u16
    }

    #[inline]
    pub const fn is_air(self) -> bool {
        (self.0 & 0xFFFF) == 0
    }

    #[inline]
    pub const fn facing(self) -> Direction {
        Direction::from_index((self.meta() & 0x7) as u8)
    }

    #[inline]
    pub const fn with_facing(self, dir: Direction) -> Self {
        let m = (self.meta() & !0x7) | (dir.index() as u16 & 0x7);
        Self::with_meta(self.id(), m)
    }

    #[inline]
    pub const fn is_active(self) -> bool {
        (self.meta() & (1 << 3)) != 0
    }

    #[inline]
    pub const fn with_active(self, active: bool) -> Self {
        let m = if active {
            self.meta() | (1 << 3)
        } else {
            self.meta() & !(1 << 3)
        };
        Self::with_meta(self.id(), m)
    }

    #[inline]
    pub const fn level(self) -> u8 {
        ((self.meta() >> 4) & 0xF) as u8
    }

    #[inline]
    pub const fn with_level(self, lvl: u8) -> Self {
        let m = (self.meta() & !(0xF << 4)) | (((lvl as u16) & 0xF) << 4);
        Self::with_meta(self.id(), m)
    }

    #[inline]
    pub const fn is_waterlogged(self) -> bool {
        (self.meta() & (1 << 8)) != 0
    }

    #[inline]
    pub const fn with_waterlogged(self, wl: bool) -> Self {
        let m = if wl {
            self.meta() | (1 << 8)
        } else {
            self.meta() & !(1 << 8)
        };
        Self::with_meta(self.id(), m)
    }

    #[inline]
    pub const fn is_top_half(self) -> bool {
        (self.meta() & (1 << 9)) != 0
    }

    #[inline]
    pub const fn with_top_half(self, top: bool) -> Self {
        let m = if top {
            self.meta() | (1 << 9)
        } else {
            self.meta() & !(1 << 9)
        };
        Self::with_meta(self.id(), m)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockShape {
    Empty,
    Full,
    Slab,
    Stair,
    Fence,
    Wall,
    Torch,
    Cross,
    Door,
    Trapdoor,
    Carpet,
    Bed,
    Chest,
    Fluid,
    Wire,
    Repeater,
    SnowLayer,
    Pane,
    Cactus,
    Farmland,
    Portal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    None,
    Pickaxe,
    Axe,
    Shovel,
    Hoe,
    Sword,
    Shears,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ToolTier {
    Hand = 0,
    Wood = 1,
    Stone = 2,
    Copper = 3,
    Iron = 4,
    Diamond = 5,
    Netherite = 6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TintKind {
    None,
    Grass,
    Foliage,
    Water,
    Redstone,
}

#[derive(Debug, Clone, Default)]
pub struct FaceTextures {
    pub top: Option<String>,
    pub bottom: Option<String>,
    pub side: Option<String>,
    pub front: Option<String>,
    pub overlay: Option<String>,
    pub particle: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BlockDef {
    pub id: BlockId,
    pub name: String,
    pub hardness: f32,
    pub blast_resistance: f32,
    pub light_emission: u8,
    pub light_opacity: u8,
    pub is_solid: bool,
    pub is_opaque: bool,
    pub is_translucent: bool,
    pub is_cutout: bool,
    pub is_replaceable: bool,
    pub is_fluid: bool,
    pub flammable: bool,
    pub slipperiness: f32,
    pub shape: BlockShape,
    pub preferred_tool: ToolKind,
    pub min_tool_tier: ToolTier,
    pub requires_tool: bool,
    pub textures: FaceTextures,
    pub tint: TintKind,
    pub fallback_rgba: [u8; 4],
}

#[derive(Debug)]
pub struct BlockRegistry {
    defs: Vec<BlockDef>,
    by_name: HashMap<String, BlockId>,
    // Cached common block IDs for hot-path access
    pub air: BlockId,
    pub stone: BlockId,
    pub granite: BlockId,
    pub diorite: BlockId,
    pub andesite: BlockId,
    pub deepslate: BlockId,
    pub cobblestone: BlockId,
    pub bedrock: BlockId,
    pub dirt: BlockId,
    pub coarse_dirt: BlockId,
    pub grass_block: BlockId,
    pub podzol: BlockId,
    pub mycelium: BlockId,
    pub mud: BlockId,
    pub sand: BlockId,
    pub red_sand: BlockId,
    pub sandstone: BlockId,
    pub red_sandstone: BlockId,
    pub terracotta: BlockId,
    pub gravel: BlockId,
    pub clay: BlockId,
    pub water: BlockId,
    pub lava: BlockId,
    pub ice: BlockId,
    pub packed_ice: BlockId,
    pub blue_ice: BlockId,
    pub snow_block: BlockId,
    pub snow: BlockId,
    pub oak_log: BlockId,
    pub birch_log: BlockId,
    pub spruce_log: BlockId,
    pub jungle_log: BlockId,
    pub acacia_log: BlockId,
    pub dark_oak_log: BlockId,
    pub cherry_log: BlockId,
    pub mangrove_log: BlockId,
    pub oak_leaves: BlockId,
    pub birch_leaves: BlockId,
    pub spruce_leaves: BlockId,
    pub jungle_leaves: BlockId,
    pub acacia_leaves: BlockId,
    pub dark_oak_leaves: BlockId,
    pub cherry_leaves: BlockId,
    pub mangrove_leaves: BlockId,
    pub oak_planks: BlockId,
    pub glass: BlockId,
    pub coal_ore: BlockId,
    pub iron_ore: BlockId,
    pub copper_ore: BlockId,
    pub gold_ore: BlockId,
    pub redstone_ore: BlockId,
    pub lapis_ore: BlockId,
    pub diamond_ore: BlockId,
    pub emerald_ore: BlockId,
    pub deepslate_coal_ore: BlockId,
    pub deepslate_iron_ore: BlockId,
    pub deepslate_copper_ore: BlockId,
    pub deepslate_gold_ore: BlockId,
    pub deepslate_redstone_ore: BlockId,
    pub deepslate_lapis_ore: BlockId,
    pub deepslate_diamond_ore: BlockId,
    pub deepslate_emerald_ore: BlockId,
    pub obsidian: BlockId,
    pub crying_obsidian: BlockId,
    pub netherrack: BlockId,
    pub soul_sand: BlockId,
    pub soul_soil: BlockId,
    pub basalt: BlockId,
    pub blackstone: BlockId,
    pub glowstone: BlockId,
    pub nether_quartz_ore: BlockId,
    pub nether_gold_ore: BlockId,
    pub ancient_debris: BlockId,
    pub crimson_nylium: BlockId,
    pub warped_nylium: BlockId,
    pub crimson_stem: BlockId,
    pub warped_stem: BlockId,
    pub nether_wart_block: BlockId,
    pub warped_wart_block: BlockId,
    pub end_stone: BlockId,
    pub end_stone_bricks: BlockId,
    pub purpur_block: BlockId,
    pub chorus_plant: BlockId,
    pub chorus_flower: BlockId,
    pub end_portal_frame: BlockId,
    pub end_portal: BlockId,
    pub nether_portal: BlockId,
    pub torch: BlockId,
    pub wall_torch: BlockId,
    pub lantern: BlockId,
    pub sea_lantern: BlockId,
    pub crafting_table: BlockId,
    pub furnace: BlockId,
    pub blast_furnace: BlockId,
    pub smoker: BlockId,
    pub chest: BlockId,
    pub barrel: BlockId,
    pub enchanting_table: BlockId,
    pub anvil: BlockId,
    pub brewing_stand: BlockId,
    pub bookshelf: BlockId,
    pub redstone_wire: BlockId,
    pub redstone_torch: BlockId,
    pub redstone_block: BlockId,
    pub redstone_lamp: BlockId,
    pub repeater: BlockId,
    pub comparator: BlockId,
    pub lever: BlockId,
    pub oak_button: BlockId,
    pub stone_pressure_plate: BlockId,
    pub piston: BlockId,
    pub sticky_piston: BlockId,
    pub piston_head: BlockId,
    pub observer: BlockId,
    pub hopper: BlockId,
    pub dispenser: BlockId,
    pub dropper: BlockId,
    pub slime_block: BlockId,
    pub honey_block: BlockId,
    pub tnt: BlockId,
    pub fire: BlockId,
    pub short_grass: BlockId,
    pub tall_grass: BlockId,
    pub dandelion: BlockId,
    pub poppy: BlockId,
    pub sugar_cane: BlockId,
    pub cactus: BlockId,
    pub farmland: BlockId,
    pub wheat: BlockId,
    pub carrots: BlockId,
    pub potatoes: BlockId,
    pub pumpkin: BlockId,
    pub melon: BlockId,
    pub white_bed: BlockId,
    pub spawner: BlockId,
    pub mossy_cobblestone: BlockId,
    pub stone_bricks: BlockId,
    pub moss_block: BlockId,
    pub dripstone_block: BlockId,
    pub sculk: BlockId,
    pub ladder: BlockId,
    pub cobweb: BlockId,
    pub magma_block: BlockId,
    pub beacon: BlockId,
    pub jukebox: BlockId,
    pub white_wool: BlockId,
}

impl BlockRegistry {
    pub fn from_embedded(tags: &TagRegistry) -> Self {
        let reg_val: Value =
            serde_json::from_str(include_str!("../../../data/extracted_26_1_2/registries.json"))
                .expect("valid registries.json");
        let tex_map: HashMap<String, Value> = serde_json::from_str(include_str!(
            "../../../data/extracted_26_1_2/block_textures.json"
        ))
        .expect("valid block_textures.json");

        let mut names: Vec<String> = reg_val["blocks"]
            .as_array()
            .expect("blocks array")
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();

        // Ensure "air" is always index 0
        if let Some(pos) = names.iter().position(|n| n == "air") {
            names.swap(0, pos);
        } else {
            names.insert(0, "air".to_string());
        }

        let mut defs = Vec::with_capacity(names.len());
        let mut by_name = HashMap::with_capacity(names.len());

        for (i, name) in names.iter().enumerate() {
            let id = BlockId(i as u16);
            by_name.insert(name.clone(), id);
            let textures = parse_face_textures(tex_map.get(name));
            let def = classify_block(id, name, textures, tags);
            defs.push(def);
        }

        let get = |n: &str| *by_name.get(n).unwrap_or(&BlockId::AIR);

        Self {
            air: get("air"),
            stone: get("stone"),
            granite: get("granite"),
            diorite: get("diorite"),
            andesite: get("andesite"),
            deepslate: get("deepslate"),
            cobblestone: get("cobblestone"),
            bedrock: get("bedrock"),
            dirt: get("dirt"),
            coarse_dirt: get("coarse_dirt"),
            grass_block: get("grass_block"),
            podzol: get("podzol"),
            mycelium: get("mycelium"),
            mud: get("mud"),
            sand: get("sand"),
            red_sand: get("red_sand"),
            sandstone: get("sandstone"),
            red_sandstone: get("red_sandstone"),
            terracotta: get("terracotta"),
            gravel: get("gravel"),
            clay: get("clay"),
            water: get("water"),
            lava: get("lava"),
            ice: get("ice"),
            packed_ice: get("packed_ice"),
            blue_ice: get("blue_ice"),
            snow_block: get("snow_block"),
            snow: get("snow"),
            oak_log: get("oak_log"),
            birch_log: get("birch_log"),
            spruce_log: get("spruce_log"),
            jungle_log: get("jungle_log"),
            acacia_log: get("acacia_log"),
            dark_oak_log: get("dark_oak_log"),
            cherry_log: get("cherry_log"),
            mangrove_log: get("mangrove_log"),
            oak_leaves: get("oak_leaves"),
            birch_leaves: get("birch_leaves"),
            spruce_leaves: get("spruce_leaves"),
            jungle_leaves: get("jungle_leaves"),
            acacia_leaves: get("acacia_leaves"),
            dark_oak_leaves: get("dark_oak_leaves"),
            cherry_leaves: get("cherry_leaves"),
            mangrove_leaves: get("mangrove_leaves"),
            oak_planks: get("oak_planks"),
            glass: get("glass"),
            coal_ore: get("coal_ore"),
            iron_ore: get("iron_ore"),
            copper_ore: get("copper_ore"),
            gold_ore: get("gold_ore"),
            redstone_ore: get("redstone_ore"),
            lapis_ore: get("lapis_ore"),
            diamond_ore: get("diamond_ore"),
            emerald_ore: get("emerald_ore"),
            deepslate_coal_ore: get("deepslate_coal_ore"),
            deepslate_iron_ore: get("deepslate_iron_ore"),
            deepslate_copper_ore: get("deepslate_copper_ore"),
            deepslate_gold_ore: get("deepslate_gold_ore"),
            deepslate_redstone_ore: get("deepslate_redstone_ore"),
            deepslate_lapis_ore: get("deepslate_lapis_ore"),
            deepslate_diamond_ore: get("deepslate_diamond_ore"),
            deepslate_emerald_ore: get("deepslate_emerald_ore"),
            obsidian: get("obsidian"),
            crying_obsidian: get("crying_obsidian"),
            netherrack: get("netherrack"),
            soul_sand: get("soul_sand"),
            soul_soil: get("soul_soil"),
            basalt: get("basalt"),
            blackstone: get("blackstone"),
            glowstone: get("glowstone"),
            nether_quartz_ore: get("nether_quartz_ore"),
            nether_gold_ore: get("nether_gold_ore"),
            ancient_debris: get("ancient_debris"),
            crimson_nylium: get("crimson_nylium"),
            warped_nylium: get("warped_nylium"),
            crimson_stem: get("crimson_stem"),
            warped_stem: get("warped_stem"),
            nether_wart_block: get("nether_wart_block"),
            warped_wart_block: get("warped_wart_block"),
            end_stone: get("end_stone"),
            end_stone_bricks: get("end_stone_bricks"),
            purpur_block: get("purpur_block"),
            chorus_plant: get("chorus_plant"),
            chorus_flower: get("chorus_flower"),
            end_portal_frame: get("end_portal_frame"),
            end_portal: get("end_portal"),
            nether_portal: get("nether_portal"),
            torch: get("torch"),
            wall_torch: get("wall_torch"),
            lantern: get("lantern"),
            sea_lantern: get("sea_lantern"),
            crafting_table: get("crafting_table"),
            furnace: get("furnace"),
            blast_furnace: get("blast_furnace"),
            smoker: get("smoker"),
            chest: get("chest"),
            barrel: get("barrel"),
            enchanting_table: get("enchanting_table"),
            anvil: get("anvil"),
            brewing_stand: get("brewing_stand"),
            bookshelf: get("bookshelf"),
            redstone_wire: get("redstone_wire"),
            redstone_torch: get("redstone_torch"),
            redstone_block: get("redstone_block"),
            redstone_lamp: get("redstone_lamp"),
            repeater: get("repeater"),
            comparator: get("comparator"),
            lever: get("lever"),
            oak_button: get("oak_button"),
            stone_pressure_plate: get("stone_pressure_plate"),
            piston: get("piston"),
            sticky_piston: get("sticky_piston"),
            piston_head: get("piston_head"),
            observer: get("observer"),
            hopper: get("hopper"),
            dispenser: get("dispenser"),
            dropper: get("dropper"),
            slime_block: get("slime_block"),
            honey_block: get("honey_block"),
            tnt: get("tnt"),
            fire: get("fire"),
            short_grass: get("short_grass"),
            tall_grass: get("tall_grass"),
            dandelion: get("dandelion"),
            poppy: get("poppy"),
            sugar_cane: get("sugar_cane"),
            cactus: get("cactus"),
            farmland: get("farmland"),
            wheat: get("wheat"),
            carrots: get("carrots"),
            potatoes: get("potatoes"),
            pumpkin: get("pumpkin"),
            melon: get("melon"),
            white_bed: get("white_bed"),
            spawner: get("spawner"),
            mossy_cobblestone: get("mossy_cobblestone"),
            stone_bricks: get("stone_bricks"),
            moss_block: get("moss_block"),
            dripstone_block: get("dripstone_block"),
            sculk: get("sculk"),
            ladder: get("ladder"),
            cobweb: get("cobweb"),
            magma_block: get("magma_block"),
            beacon: get("beacon"),
            jukebox: get("jukebox"),
            white_wool: get("white_wool"),
            defs,
            by_name,
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
    pub fn get(&self, id: BlockId) -> &BlockDef {
        &self.defs[(id.0 as usize).min(self.defs.len() - 1)]
    }

    #[inline]
    pub fn by_name(&self, name: &str) -> Option<BlockId> {
        let short = name.strip_prefix("minecraft:").unwrap_or(name);
        self.by_name.get(short).copied()
    }

    pub fn all(&self) -> &[BlockDef] {
        &self.defs
    }
}

fn parse_face_textures(val: Option<&Value>) -> FaceTextures {
    let Some(v) = val else {
        return FaceTextures::default();
    };
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(|s| s.to_string());
    FaceTextures {
        top: s("top"),
        bottom: s("bottom"),
        side: s("side"),
        front: s("front"),
        overlay: s("overlay"),
        particle: s("particle"),
    }
}

fn classify_block(id: BlockId, name: &str, textures: FaceTextures, tags: &TagRegistry) -> BlockDef {
    let is_air_like = matches!(name, "air" | "cave_air" | "void_air");
    let is_water = name == "water" || name == "bubble_column";
    let is_lava = name == "lava";
    let is_fluid = is_water || is_lava;

    let shape = if is_air_like {
        BlockShape::Empty
    } else if is_fluid {
        BlockShape::Fluid
    } else if name.ends_with("_slab") {
        BlockShape::Slab
    } else if name.ends_with("_stairs") {
        BlockShape::Stair
    } else if name.ends_with("_fence") || name.ends_with("_fence_gate") {
        BlockShape::Fence
    } else if name.ends_with("_wall") {
        BlockShape::Wall
    } else if name.contains("torch") {
        BlockShape::Torch
    } else if name.ends_with("_door") {
        BlockShape::Door
    } else if name.ends_with("_trapdoor") {
        BlockShape::Trapdoor
    } else if name.ends_with("_carpet") || name.ends_with("_pressure_plate") {
        BlockShape::Carpet
    } else if name.ends_with("_bed") {
        BlockShape::Bed
    } else if name.contains("chest") {
        BlockShape::Chest
    } else if name == "redstone_wire" {
        BlockShape::Wire
    } else if name == "repeater" || name == "comparator" {
        BlockShape::Repeater
    } else if name == "snow" {
        BlockShape::SnowLayer
    } else if name.ends_with("_pane") || name == "iron_bars" {
        BlockShape::Pane
    } else if name == "cactus" {
        BlockShape::Cactus
    } else if name == "farmland" || name == "dirt_path" {
        BlockShape::Farmland
    } else if name == "nether_portal" || name == "end_portal" {
        BlockShape::Portal
    } else if tags.block_has_tag(name, "flowers")
        || tags.block_has_tag(name, "saplings")
        || tags.block_has_tag(name, "crops")
        || matches!(
            name,
            "short_grass"
                | "tall_grass"
                | "fern"
                | "large_fern"
                | "dead_bush"
                | "sugar_cane"
                | "bamboo"
                | "cobweb"
                | "sweet_berry_bush"
                | "brown_mushroom"
                | "red_mushroom"
                | "crimson_fungus"
                | "warped_fungus"
                | "crimson_roots"
                | "warped_roots"
                | "nether_sprouts"
                | "fire"
                | "soul_fire"
                | "vine"
                | "ladder"
                | "lever"
        )
        || name.ends_with("_button")
        || name.ends_with("_rail")
        || name == "rail"
    {
        BlockShape::Cross
    } else {
        BlockShape::Full
    };

    let is_leaves = tags.block_has_tag(name, "leaves");
    let is_glass = name == "glass" || name.ends_with("_glass") || name.ends_with("_pane");
    let is_cutout = is_leaves
        || is_glass
        || matches!(
            shape,
            BlockShape::Cross | BlockShape::Torch | BlockShape::Door | BlockShape::Trapdoor | BlockShape::Wire | BlockShape::Repeater | BlockShape::Pane
        )
        || matches!(name, "spawner" | "beacon" | "brewing_stand" | "hopper" | "lantern" | "soul_lantern");

    let is_translucent = is_water || name.contains("stained_glass") || name == "ice" || name == "slime_block" || name == "honey_block" || name == "nether_portal";

    let is_solid = !is_air_like
        && !is_fluid
        && !matches!(
            shape,
            BlockShape::Cross | BlockShape::Torch | BlockShape::Wire | BlockShape::Portal
        )
        && !matches!(name, "cobweb" | "fire" | "soul_fire");

    let is_opaque = shape == BlockShape::Full && !is_cutout && !is_translucent && !is_air_like && !is_fluid;

    let is_replaceable = is_air_like
        || is_fluid
        || tags.block_has_tag(name, "replaceable")
        || matches!(name, "short_grass" | "tall_grass" | "fern" | "snow" | "fire" | "soul_fire" | "dead_bush");

    let preferred_tool = if tags.block_has_tag(name, "mineable/pickaxe") {
        ToolKind::Pickaxe
    } else if tags.block_has_tag(name, "mineable/axe") {
        ToolKind::Axe
    } else if tags.block_has_tag(name, "mineable/shovel") {
        ToolKind::Shovel
    } else if tags.block_has_tag(name, "mineable/hoe") {
        ToolKind::Hoe
    } else if is_leaves || tags.block_has_tag(name, "wool") || name == "cobweb" {
        ToolKind::Shears
    } else {
        ToolKind::None
    };

    let min_tool_tier = if tags.block_has_tag(name, "needs_diamond_tool") {
        ToolTier::Diamond
    } else if tags.block_has_tag(name, "needs_iron_tool") {
        ToolTier::Iron
    } else if tags.block_has_tag(name, "needs_stone_tool") {
        ToolTier::Stone
    } else {
        ToolTier::Hand
    };

    let requires_tool = min_tool_tier > ToolTier::Hand
        || (preferred_tool == ToolKind::Pickaxe
            && !matches!(name, "ice" | "packed_ice" | "blue_ice" | "glass"));

    let (hardness, blast_resistance) = match name {
        "air" | "cave_air" | "void_air" => (0.0, 0.0),
        "bedrock" | "end_portal" | "end_portal_frame" | "command_block" | "barrier" => (-1.0, 3_600_000.0),
        "obsidian" | "crying_obsidian" | "respawn_anchor" => (50.0, 1200.0),
        "ancient_debris" | "netherite_block" => (30.0, 1200.0),
        "ender_chest" => (22.5, 600.0),
        "water" | "lava" => (100.0, 100.0),
        s if s.starts_with("deepslate") && s.contains("ore") => (4.5, 3.0),
        s if s.contains("ore") => (3.0, 3.0),
        "deepslate" | "cobbled_deepslate" => (3.0, 6.0),
        "iron_block" | "gold_block" | "diamond_block" | "emerald_block" | "copper_block" | "anvil" => (5.0, 6.0),
        "spawner" => (5.0, 5.0),
        "furnace" | "blast_furnace" | "smoker" | "dispenser" | "dropper" | "observer" => (3.5, 3.5),
        "stone" | "cobblestone" | "stone_bricks" | "andesite" | "diorite" | "granite" => (1.5, 6.0),
        "netherrack" => (0.4, 0.4),
        "end_stone" | "end_stone_bricks" => (3.0, 9.0),
        s if tags.block_has_tag(s, "logs") => (2.0, 2.0),
        s if tags.block_has_tag(s, "planks") => (2.0, 3.0),
        "crafting_table" | "chest" | "barrel" | "jukebox" => (2.5, 2.5),
        "bookshelf" => (1.5, 1.5),
        "grass_block" | "dirt" | "gravel" | "clay" | "farmland" => (0.6, 0.6),
        "sand" | "red_sand" | "soul_sand" | "soul_soil" => (0.5, 0.5),
        _s if is_leaves || is_glass => (0.2, 0.2),
        "tnt" | "torch" | "wall_torch" | "redstone_torch" | "short_grass" | "tall_grass" | "fire" => (0.0, 0.0),
        _ if preferred_tool == ToolKind::Pickaxe => (2.0, 6.0),
        _ if preferred_tool == ToolKind::Axe => (2.0, 2.5),
        _ if preferred_tool == ToolKind::Shovel => (0.6, 0.6),
        _ => (0.5, 0.5),
    };

    let light_emission = match name {
        "lava" | "glowstone" | "sea_lantern" | "shroomlight" | "lantern" | "jack_o_lantern" | "beacon" | "end_portal" | "fire" => 15,
        "torch" | "wall_torch" | "end_rod" => 14,
        "blast_furnace" | "smoker" | "furnace" => 0, // lit variant handled dynamically
        "nether_portal" => 11,
        "soul_lantern" | "soul_torch" | "soul_fire" | "crying_obsidian" => 10,
        "redstone_torch" => 7,
        "magma_block" => 3,
        "brown_mushroom" | "brewing_stand" => 1,
        _ => 0,
    };

    let light_opacity = if is_air_like || matches!(shape, BlockShape::Cross | BlockShape::Torch | BlockShape::Wire | BlockShape::Portal) {
        0
    } else if is_water || is_leaves || is_ice_like(name) {
        1
    } else if is_opaque {
        15
    } else {
        0
    };

    let slipperiness = match name {
        "ice" | "packed_ice" => 0.98,
        "blue_ice" => 0.989,
        "slime_block" => 0.8,
        _ => 0.6,
    };

    let tint = if matches!(name, "grass_block" | "short_grass" | "tall_grass" | "fern" | "large_fern" | "sugar_cane") {
        TintKind::Grass
    } else if is_leaves && !matches!(name, "cherry_leaves" | "azalea_leaves" | "flowering_azalea_leaves") {
        TintKind::Foliage
    } else if is_water {
        TintKind::Water
    } else if name == "redstone_wire" {
        TintKind::Redstone
    } else {
        TintKind::None
    };

    let flammable = tags.block_has_tag(name, "logs")
        || tags.block_has_tag(name, "planks")
        || is_leaves
        || tags.block_has_tag(name, "wool")
        || matches!(name, "tnt" | "bookshelf" | "short_grass" | "tall_grass");

    let fallback_rgba = default_block_color(name);

    BlockDef {
        id,
        name: name.to_string(),
        hardness,
        blast_resistance,
        light_emission,
        light_opacity,
        is_solid,
        is_opaque,
        is_translucent,
        is_cutout,
        is_replaceable,
        is_fluid,
        flammable,
        slipperiness,
        shape,
        preferred_tool,
        min_tool_tier,
        requires_tool,
        textures,
        tint,
        fallback_rgba,
    }
}

fn is_ice_like(name: &str) -> bool {
    matches!(name, "ice" | "frosted_ice")
}

fn default_block_color(name: &str) -> [u8; 4] {
    match name {
        "grass_block" | "short_grass" | "tall_grass" => [95, 159, 53, 255],
        "dirt" | "farmland" | "coarse_dirt" => [134, 96, 67, 255],
        "stone" | "cobblestone" | "andesite" | "gravel" => [125, 125, 125, 255],
        "deepslate" => [72, 72, 78, 255],
        "bedrock" => [48, 48, 48, 255],
        "sand" | "sandstone" => [219, 207, 163, 255],
        "red_sand" | "red_sandstone" => [190, 102, 33, 255],
        "water" => [48, 96, 214, 175],
        "lava" => [235, 95, 20, 255],
        "oak_log" | "oak_planks" => [156, 121, 74, 255],
        "birch_log" => [216, 212, 203, 255],
        "spruce_log" => [82, 56, 34, 255],
        "oak_leaves" | "birch_leaves" | "jungle_leaves" => [66, 132, 44, 255],
        "cherry_leaves" => [238, 173, 203, 255],
        "snow" | "snow_block" => [245, 250, 255, 255],
        "ice" | "packed_ice" | "blue_ice" => [145, 185, 250, 220],
        "obsidian" => [25, 18, 36, 255],
        "netherrack" => [110, 42, 42, 255],
        "glowstone" => [245, 210, 110, 255],
        "end_stone" => [222, 223, 155, 255],
        "diamond_ore" | "deepslate_diamond_ore" => [90, 220, 225, 255],
        "gold_ore" | "deepslate_gold_ore" => [235, 200, 60, 255],
        "iron_ore" | "deepslate_iron_ore" => [195, 165, 145, 255],
        "coal_ore" | "deepslate_coal_ore" => [55, 55, 55, 255],
        "copper_ore" | "deepslate_copper_ore" => [190, 115, 75, 255],
        "redstone_ore" | "redstone_wire" | "redstone_block" => [200, 30, 30, 255],
        "tnt" => [210, 55, 45, 255],
        _ => {
            // Deterministic hash color so every one of the 1,147 blocks looks visually distinct
            let mut h: u32 = 2166136261;
            for b in name.bytes() {
                h ^= b as u32;
                h = h.wrapping_mul(16777619);
            }
            [
                64 + ((h & 0x7F) as u8),
                64 + (((h >> 8) & 0x7F) as u8),
                64 + (((h >> 16) & 0x7F) as u8),
                255,
            ]
        }
    }
}
