# Minecraft Java Edition 26.1.2 — Reverse-Engineered Specification & Clean-Room Rust Architecture

## 1. Reverse-Engineering Provenance & Evidence

- **Target Artifact:** `C:\Users\shaya\AppData\Roaming\.minecraft\versions\Fabric 26.1.2\Fabric 26.1.2.jar`
- **SHA-256:** `b1b3158572666445eff01e82fad8c7de2e4953db6d354f311730d77a8359d0b0`
- **REA Evidence ID:** `ev_73a1fbfdb243c9a57235d44b393fdec537187fe217f3572976ffd295bf520efb` (`inspect-artifact` / `inventory_artifact` substep `ev_722235f24e0d17e94d4191a27a5516a86e6bd0264217478c0f5b96a26306cd83`, manifest `agm_9e98304faf7ca76f581024d427f28edd6c96b38f57d42fd5e3b68b27e55fe9c2`)
- **Archive Totals (observed):** 30,675 JAR entries (`node_count: 29917`, `occurrence_count: 30676`, `edge_count: 30675`), 6,882 top-level unobfuscated Java classes, 10,462 `assets/minecraft/*` files, 8,282 `data/minecraft/*` files.
- **Version Metadata (`version.json` & `net.minecraft.SharedConstants`):**
  - `id`: `"26.1.2"`, `world_version`: `4790`, `protocol_version`: `775`, `resource_pack_format`: `84.0`, `data_pack_format`: `101.1`, `build_time`: `2026-04-09T10:11:03+00:00`.
- **Indexed Asset Manifest:** `%APPDATA%\.minecraft\assets\indexes\30.json` (4,750 objects, including 2,094 mob sounds, 1,502 block sounds, 65 music tracks, 21 records, etc.).

## 2. Observed Game Constants, Registries, and Rules

### 2.1 Registry Scale (observed via `javap` and JAR tree)
- `Blocks`: 1,147 static registry entries (`net.minecraft.world.level.block.Blocks`)
- `Items`: 1,485 static registry entries (`net.minecraft.world.item.Items`)
- `EntityType`: 159 static registry entries (`net.minecraft.world.entity.EntityType`)
- `MobEffects`: 40 status effects (`net.minecraft.world.effect.MobEffects`)
- `Fluids`: 5 entries (`EMPTY`, `FLOWING_WATER`, `WATER`, `FLOWING_LAVA`, `LAVA`)
- `ParticleTypes`: 119 particle types
- `SoundEvents`: 1,815 sound events
- Data-driven JSON registries: 1,617 advancements, 1,515 recipes, 1,326 loot tables, 1,202 NBT structures, 951 worldgen files, 758 tags, 387 villager trades, 68 trade sets, 51 painting variants, 50 damage types, 43 enchantments, 43 banner patterns, 28 trial spawners, 21 jukebox songs, 18 trim patterns, 11 trim materials, 4 dimension types.

### 2.2 Dimensions (`data/minecraft/dimension_type/*.json`)
- **Overworld (`overworld.json`):** `min_y = -64`, `height = 384`, `logical_height = 384`, `has_skylight = true`, `has_ceiling = false`, `coordinate_scale = 1.0`, `cloud_height = 192.33`, `sky_color = #78a7ff`, `fog_color = #c0d8ff`, `bed_rule = when_dark / always`.
- **The Nether (`the_nether.json`):** `min_y = 0`, `height = 256`, `logical_height = 128`, `has_skylight = false`, `has_ceiling = true`, `coordinate_scale = 8.0`, `ambient_light = 0.1`, `fast_lava = true`, `water_evaporates = true`, `bed_rule.explodes = true`, `respawn_anchor_works = true`.
- **The End (`the_end.json`):** `min_y = 0`, `height = 256`, `logical_height = 256`, `has_skylight = true`, `has_ceiling = false`, `has_ender_dragon_fight = true`, `ambient_light = 0.25`, `bed_rule.explodes = true`.

### 2.3 Physics, Combat, Hunger, Fluids, Redstone, Persistence (observed via `javap`)
- **Tick Loop:** Fixed 20 TPS (50 ms/tick), `DeltaTracker` interpolation for smooth frame rendering.
- **Movement & Physics (`LivingEntity`, `Player`):**
  - `DEFAULT_BASE_GRAVITY = 0.08`, `BASE_JUMP_POWER = 0.42`, `INPUT_FRICTION = 0.98`, `WATER_FLOAT_IMPULSE = 0.04`, `DEFAULT_KNOCKBACK = 0.4`, `INVULNERABLE_DURATION = 20`, `DEATH_DURATION = 20`.
  - `Player`: `MAX_HEALTH = 20`, `DEFAULT_BLOCK_INTERACTION_RANGE = 4.5`, `DEFAULT_ENTITY_INTERACTION_RANGE = 3.0`, `defaultFlySpeed = 0.02`.
- **Food & Saturation (`FoodData`):** Initial `foodLevel = 20`, `saturationLevel = 5.0`. When `exhaustionLevel > 4.0`, subtract `4.0` and drain `saturationLevel` by `1.0` (or `foodLevel` by `1` if saturation is 0 and difficulty != Peaceful).
- **Tool Tiers (`ToolMaterial`):**
  - `WOOD(59, 2.0, +0, 15)`, `STONE(131, 4.0, +1, 5)`, `COPPER(190, 5.0, +1, 13)`, `IRON(250, 6.0, +2, 14)`, `DIAMOND(1561, 8.0, +3, 10)`, `GOLD(32, 12.0, +0, 22)`, `NETHERITE(2031, 9.0, +4, 15)`.
- **Armor Tiers (`ArmorMaterials`):**
  - `LEATHER(5, [1,2,3,1], 15, 0, 0)`, `COPPER(11, [1,3,4,2], 8, 0, 0)`, `CHAINMAIL(15, [1,4,5,2], 12, 0, 0)`, `IRON(15, [2,5,6,2], 9, 0, 0)`, `GOLD(7, [1,3,5,2], 25, 0, 0)`, `DIAMOND(33, [3,6,8,3], 10, 2.0, 0)`, `TURTLE_SCUTE(25, [2,5,6,2], 9, 0, 0)`, `NETHERITE(37, [3,6,8,3], 15, 3.0, 0.1)`.
- **Combat Damage (`CombatRules`):**
  - `armorReduction = clamp(armor - dmg / (2 + toughness/4), armor * 0.2, 20) / 25`, `magicReduction = clamp(epf, 0, 20) / 25`.
- **Fluids (`WaterFluid`, `LavaFluid`):**
  - Water: `slopeFindDistance = 4`, `dropOff = 1`, `tickDelay = 5`.
  - Lava: Overworld `slopeFindDistance = 2`, `dropOff = 2`, `tickDelay = 30`; Nether (`fast_lava`) `4`, `1`, `10`.
- **Redstone (`RedStoneWireBlock`, `PistonStructureResolver`):**
  - Signal strength `0..15`, piston `MAX_PUSH_DEPTH = 12`, slime/honey adherence, repeater 1–4 tick delay + side-locking, comparator compare/subtract + container fullness, observers, redstone torches, dispensers/droppers, hoppers.
- **Persistence (`RegionFile`):**
  - Anvil `.mca`: 32×32 chunks per file, `SECTOR_BYTES = 4096`, `SECTOR_INTS = 1024`, 5-byte chunk header (`length` + compression type), `.mcc` external chunk threshold `256` sectors.

## 3. Clean-Room Rust Workspace Architecture (`D:\VoiceClone`)

We implement the complete game in a modular Rust workspace (`edition = "2024"` / `2021`) so each subsystem is isolated, unit-tested, and compiled natively for Windows via `wgpu` + `winit`:

- `crates/mc-core`: Core math, `BlockPos`, `ChunkPos`, `AABB`, `Direction`, `Identifier`, `RandomSource` (Xoroshiro128++ / Java LCG), `BitStorage`, `GameMode`, `Difficulty`, `GameRules`.
- `crates/mc-nbt`: Complete binary NBT & SNBT parser/serializer, GZip/Zlib compression, Anvil `.mca` `RegionFile` + `level.dat` world persistence.
- `crates/mc-data`: Reverse-engineered registries + runtime/compiled data loader reading `Fabric 26.1.2.jar` JSONs (`blocks`, `items`, `recipes`, `loot_tables`, `tags`, `enchantments`, `damage_types`, `dimension_types`, `advancements`, `villager_trades`, `biomes`) with complete built-in tables so everything works both with and without the JAR.
- `crates/mc-world`: Chunk sections (`16x16x16`), Overworld/Nether/End `Level` storage, Sky+Block BFS light engine, deterministic `ScheduledTick` scheduler, Water/Lava fluid simulation, Redstone evaluator + Pistons/Repeaters/Comparators/Observers/Hoppers/TNT, Block Entities (Chest, Furnace/BlastFurnace/Smoker, BrewingStand, EnchantingTable, Anvil, Hopper, Beacon, Bed, Sign, Spawner, Jukebox), AABB swept collision & block raycast (DDA).
- `crates/mc-gen`: Seeded multi-octave Perlin/Simplex/BlendedNoise & Spline terrain generator for **Overworld** (25+ biomes, 3D cheese/spaghetti/noodle caves, aquifers, deepslate, ore veins, trees, villages, temples, dungeons, mineshafts, ruined portals, strongholds with End Portal), **The Nether** (5 biomes, lava ocean, fortresses, bastions, ancient debris), and **The End** (main island, obsidian pillars + crystals, outer islands + chorus + End Cities).
- `crates/mc-entities`: Player (`FoodData`, XP, Health, Armor, Offhand, 2x2/3x3 Crafting, Ender Chest, Survival/Creative/Adventure/Spectator), 30+ Mobs (Passive, Hostile, Nether, End, Bosses: **Ender Dragon** & **Wither**) with Goal/Behavior AI & pathfinding, Villager trading, Projectiles, Item entities, XP orbs, Falling blocks, Boats, Minecarts, Particles.
- `crates/mc-proto`: Minecraft `protocol_version = 775` VarInt/packet framing, Handshake/Status/Login/Configuration/Play codec, integrated & dedicated TCP multiplayer server + client.
- `crates/mc-audio`: Asset index (`30.json`) `.ogg` loader from `.minecraft/assets/objects` + `rodio` 3D spatial audio playback + procedural PCM synthesis fallback.
- `crates/mc-render`: `wgpu` renderer with real JAR PNG texture atlas + animated textures + biome tinting + procedural fallback atlas, multi-threaded chunk mesher (face culling + 4-corner voxel AO + dual sky/block lightmap + opaque/cutout/translucent passes), sky/sun/moon/stars/3D clouds, entity box renderer, particle renderer, block crack & selection wireframe, first-person hand & F5 3rd-person camera.
- `crates/mc-client`: `winit` window & input loop, fixed 20 TPS tick + interpolated rendering, full GUI screens (Title, World Create/Select, Multiplayer, Pause, HUD, Inventory/Creative Search, Crafting, Furnace, Chest, Enchanting, Anvil, Brewing, Villager Trade, Chat + Command Parser with `/give`, `/tp`, `/gamemode`, `/setblock`, `/fill`, `/summon`, `/time`, `/weather`, `/seed`, `/locate`, `/effect`, `/enchant`, `/xp`, `/kill`, `/gamerule`, `/difficulty`, `/help`, F3 Debug HUD, Advancements).
