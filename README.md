# Minecraft 26.1.2 — Clean-Room Rust Edition

A complete, from-scratch clean-room implementation of **Minecraft Java Edition 26.1.2** (`Protocol 775`, `DataVersion 4790`) written in Rust. Built around data, formulas, and constants reverse-engineered from a local Minecraft Java 26.1.2 installation, paired with a native `wgpu` 3D renderer, `winit` desktop client, `rodio` audio engine, multi-threaded world generation/meshing, and Anvil `.mca` persistence.

---

## Quick Start

### Prerequisites
- **Rust toolchain** supporting **Edition 2024** (verified with `rustc 1.99.1` / `cargo 1.99.1` on `x86_64-pc-windows-msvc`)
- **Optional (for official vanilla textures & sounds):** An existing Minecraft Java `26.1.2` / `Fabric 26.1.2` installation in `%APPDATA%\.minecraft` (or `$HOME/.minecraft`). If absent, the renderer and audio engine automatically use built-in procedural 16x16 pixel-art tiles and synthesized sound fallbacks.

### Build & Play
```powershell
# Run the game in release mode
cargo run --release -p mc-client

# Or build the optimized standalone executable and launch it directly
cargo build --release -p mc-client
.\target\release\mc-client.exe
```

### Run Test Suite
```powershell
cargo test --workspace
```

---

## Workspace Architecture

```
.
├── Cargo.toml                  # 10-crate Rust 2024 workspace
├── tools/
│   └── extract_26_1_2.py       # Extracts registries, recipes, loot, tags & asset maps from Fabric 26.1.2.jar
├── data/
│   └── extracted_26_1_2/       # Embedded JSON data tables (blocks, items, recipes, biomes, advancements, etc.)
├── docs/
│   └── superpowers/specs/      # Architecture & reverse-engineering specification
├── saves/                      # Runtime Anvil world save directory (created when saving worlds)
└── crates/
    ├── mc-core/                # BlockPos, ChunkPos, DimensionId, Direction, GameMode, Difficulty, AABB, RNG, SharedConstants
    ├── mc-nbt/                 # Binary NBT codec + Anvil .mca RegionFile reader/writer (4 KiB sectors + .mcc overflow)
    ├── mc-data/                # Embedded 26.1.2 registries, tags, recipes, loot tables, biomes, enchantments & advancements
    ├── mc-world/               # World state, 16x16x384 chunks, nibble sky/block light BFS, fluids, redstone, block entities, explosions
    ├── mc-gen/                 # Deterministic Overworld, Nether & End terrain, cave, ore, tree & structure generators
    ├── mc-entities/            # Player physics, FoodData, CombatRules, mob/boss AI, projectiles, drops, particles & /commands
    ├── mc-proto/               # Protocol 775 VarInt + zlib packet framing, Server List Ping & LAN multiplayer server
    ├── mc-audio/               # Asset index 30.json OGG loader + rodio playback with procedural fallback synthesizer
    ├── mc-render/              # Texture atlas (JAR + fallback), parallel chunk mesher, 8x8 bitmap font & wgpu 3D/2D pipelines
    └── mc-client/              # Desktop game binary, winit event loop, 20 TPS fixed loop, Anvil save/load & full UI screens
```

---

## Features

### World, Dimensions & Simulation (20 TPS)
- **Three Dimensions:** Overworld (`y = -64..319`, skylight, day/night cycle, clouds), The Nether (`y = 0..255`, ceiling, fast lava, water evaporation, bed explosions, `8:1` portal coordinate scaling), and The End (`y = 0..255`, End Stone islands, Ender Dragon fight).
- **Lighting & Physics:** Nibble-packed sky & block light BFS propagation, swept `AABB` player & entity collision, gravity blocks (`sand`, `red_sand`, `gravel`, `anvil`), water/lava flow ticks, crop growth, fire spread, and explosion raycasting.
- **Redstone & Mechanisms:** Redstone dust signal strength (`0..15`), levers, buttons, repeaters, comparators, redstone torches, redstone lamps, observers, doors, trapdoors, TNT ignition, dispensers, and pistons.
- **Block Entities:** Chests, barrels, shulker boxes, furnaces, blast furnaces, smokers, brewing stands, hoppers, dispensers, enchanting tables, and anvils.

### Rendering & Audio
- **Texture Atlas (`mc-render`):** Automatically loads PNG textures from `assets/minecraft/textures/` inside `Fabric 26.1.2.jar` into an extruded-border 16x16 GPU atlas (plus `destroy_stage_0..9` crack overlays) with procedural fallbacks if no JAR is found.
- **Multi-Pass Chunk Mesher:** `rayon`-parallel meshing across `opaque`, `cutout`, and `translucent` passes with cross-chunk face culling, 4-corner smooth voxel ambient occlusion, biome grass/foliage/water tinting, and custom shapes (`Slab`, `Stair`, `Fence`, `Wall`, `Pane`, `Torch`, `Cross`, `Door`, `Trapdoor`, `Carpet`, `SnowLayer`, `Bed`, `Chest`, `Fluid`, `Wire`, `Repeater`, `Cactus`, `Farmland`, `Portal`).
- **Atmosphere & Models:** Dynamic day/night sky color, sun, moon, night stars, drifting clouds, dimension/fluid distance fog, first-person hand/held-item rendering, third-person player model (`F5`), entity silhouettes, and particle effects.
- **Audio (`mc-audio`):** Reads `.minecraft/assets/indexes/30.json` to play real OGG game sounds via `rodio`, falling back seamlessly to procedural waveform synthesis.

### Progression, UI & Persistence
- **Full Menu Suite:** Title Screen (Singleplayer world list, Create New World with seed & mode, Multiplayer Server Ping/Connect, Options), Pause Menu (with Open to LAN), Death Screen, Survival Inventory (`2x2` crafting + armor + offhand), Searchable Creative Catalog (`1,485` items), Crafting Table (`3x3`), Furnace/Blast Furnace/Smoker, Chest/Barrel, Enchanting Table, Anvil, Brewing Stand, Villager Trading, and Advancements (`1,617` tracked advancements + unlock toasts).
- **Anvil Save Format:** Saves GZip-compressed `level.dat` (`DataVersion = 4790`), per-dimension region files (`region/r.*.mca`, `DIM-1/region/r.*.mca`, `DIM1/region/r.*.mca`), and per-dimension entity JSON under `saves/<world_name>/`.

---

## Controls & Commands

### Controls
| Input | Action |
| :--- | :--- |
| `W` `A` `S` `D` | Move / Strafe |
| `Mouse` | Look around |
| `Space` | Jump / Swim up (`Double-tap` in Creative to toggle Fly) |
| `Shift` | Sneak / Descend while flying |
| `Ctrl` | Sprint |
| `Left Click` | Attack entity / Hold to break targeted block |
| `Right Click` | Place block / Eat food / Equip armor / Open container / Trade / Ignite / Shoot |
| `1–9` / `Mouse Wheel` | Select hotbar slot |
| `Q` | Drop held item |
| `E` | Open Inventory (Survival) or Searchable Item Catalog (Creative) |
| `T` / `/` | Open Chat / Command bar |
| `L` | View Advancements |
| `F3` | Toggle Debug HUD (FPS, XYZ, Block, Chunk, Biome, Dimension, Light) |
| `F5` | Toggle First-Person / Third-Person Camera |
| `Esc` | Close menu / Pause / Save & Quit |

### Supported Slash Commands
Use `T` or `/` in-game to run commands implemented in `mc-entities`:
- `/help`
- `/gamemode <survival|creative|adventure|spectator>`
- `/difficulty <peaceful|easy|normal|hard>`
- `/give <target> <item> [count]`
- `/tp <x> <y> <z>`
- `/setblock <x> <y> <z> <block>`
- `/fill <x1> <y1> <z1> <x2> <y2> <z2> <block>`
- `/summon <entity> [x y z]`
- `/time <set <day|noon|night|midnight|ticks>|add <ticks>>`
- `/weather <clear|rain|thunder>`
- `/effect <target> <effect> [seconds] [amplifier]`
- `/enchant <target> <enchantment> [level]`
- `/xp <amount>`
- `/gamerule <rule> [value]`
- `/spawnpoint [x y z]`
- `/clear`, `/kill`, `/seed`, `/say`

---

## Extracted 26.1.2 Dataset & Protocol Constants

All data in `data/extracted_26_1_2/` is extracted from Minecraft Java `26.1.2` (`WORLD_VERSION = 4790`, `PROTOCOL_VERSION = 775`):

| Category | Count | File |
| :--- | :---: | :--- |
| Blocks | 1,147 | `registries.json` |
| Items | 1,485 | `registries.json` |
| Entities / Effects / Particles | 159 / 40 / 119 | `registries.json` |
| Biomes / Enchantments / Damage Types | 65 / 43 / 50 | `biomes.json`, `enchantments.json`, `damage_types.json` |
| Dimension Types | 4 | `dimension_types.json` |
| Recipes / Loot Tables / Tags | 1,515 / 1,326 / 758 | `recipes.json`, `loot_tables.json`, `tags.json` |
| Villager Trades / Advancements | 387 / 1,617 | `villager_trades.json`, `advancements.json` |
| Resolved Block / Item Textures | 1,170 / 1,267 | `block_textures.json`, `item_textures.json` |

To re-run extraction against your local `.minecraft` installation:
```powershell
python tools/extract_26_1_2.py
```

---

## Verification

`cargo test --workspace` covers all 10 workspace crates (`13` unit & integration tests):
- **`mc-core`**: Coordinate packing, `AABB` ray/swept collision, directional math, RNG determinism
- **`mc-nbt`**: Binary NBT round-trip and Anvil `.mca` sector storage
- **`mc-data`**: Verification of all embedded 26.1.2 registries, tags, recipes, and loot tables
- **`mc-world`**: Voxel DDA raycasting, redstone circuits, and scheduled water flow
- **`mc-gen`**: Deterministic Overworld, Nether, and End chunk generation
- **`mc-entities`**: `CombatRules` absorption math, player physics, and `/command` execution
- **`mc-proto`**: VarInt codec and Server List Ping round-trip
- **`mc-audio`**: Audio engine initialization
- **`mc-render`**: Texture atlas packing and multi-pass chunk geometry generation
- **`mc-client`**: End-to-end integration test covering terrain generation, meshing, physics, 2x2 crafting, block loot, furnace smelting, Nether & End chunks, combat, commands, and Anvil `.mca` / `level.dat` save & reload
