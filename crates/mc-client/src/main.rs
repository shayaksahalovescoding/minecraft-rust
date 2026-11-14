mod app;
mod save;
mod ui;

use app::McApp;
use winit::event_loop::{ControlFlow, EventLoop};

fn main() {
    let event_loop = EventLoop::new().expect("create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = McApp::new();
    event_loop.run_app(&mut app).expect("run minecraft client");
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use glam::DVec3;
    use mc_core::{BlockPos, ChunkPos, DimensionId, GameMode};
    use mc_data::{BlockState, GameData, ItemStack, LootContext};
    use mc_entities::{execute_command, EntityKind, EntityManager, Player, PlayerInput};
    use mc_gen::WorldGenerator;
    use mc_render::{mesh_chunk, ChunkNeighborhood, TextureAtlas};
    use mc_world::World;
    use std::path::PathBuf;

    #[test]
    fn full_end_to_end_gameplay_and_persistence_loop() {
        let data = GameData::load();
        let atlas = TextureAtlas::build(&data);
        let world_gen = WorldGenerator::new(2612026);

        // 1. Generate Overworld chunks + mesh
        let mut world = World::new(DimensionId::Overworld, 2612026);
        for dz in -1..=1 {
            for dx in -1..=1 {
                let c = world_gen.generate_chunk(DimensionId::Overworld, ChunkPos::new(dx, dz), &data);
                world.insert_chunk(c, &data.blocks);
            }
        }
        let hood = ChunkNeighborhood::capture(&world, ChunkPos::new(0, 0)).unwrap();
        let mesh = mesh_chunk(&hood, &data.blocks, &atlas);
        assert!(!mesh.opaque.indices.is_empty());

        // 2. Player physics on terrain
        let mut spawn_y = 64;
        for y in (0..200).rev() {
            if !world.get_block(BlockPos::new(8, y, 8)).is_air() {
                spawn_y = y + 2;
                break;
            }
        }
        let mut player = Player::new("Steve", BlockPos::new(8, spawn_y, 8), GameMode::Survival);
        let mut entities = EntityManager::new();
        let input = PlayerInput {
            forward: 1.0,
            yaw: 0.0,
            ..Default::default()
        };
        for _ in 0..20 {
            player.tick(&input, &world, &data);
            world.tick(&data, ChunkPos::new(0, 0));
            entities.tick(&mut world, &mut player, &data);
        }

        // 3. Block place, break, loot, crafting, and smelting
        let target = BlockPos::new(8, spawn_y + 1, 9);
        let oak_log = data.items.by_name("oak_log").unwrap();
        let mut grid2x2 = [
            ItemStack::new(oak_log, 1),
            ItemStack::EMPTY,
            ItemStack::EMPTY,
            ItemStack::EMPTY,
        ];
        let planks_match = data.recipes.match_crafting(2, 2, &grid2x2).expect("oak_log -> oak_planks");
        assert_eq!(planks_match.result.count, 4);
        grid2x2[0] = ItemStack::EMPTY;

        world.set_block(target, BlockState::new(data.blocks.stone), &data.blocks);
        assert_eq!(world.get_block(target).id(), data.blocks.stone);
        let mut rng = rand::thread_rng();
        let drops = data.loot.roll_block("stone", &LootContext::default(), &mut rng);
        assert!(!drops.is_empty());

        let furnace_pos = BlockPos::new(7, spawn_y + 1, 9);
        world.set_block(furnace_pos, BlockState::new(data.blocks.furnace), &data.blocks);
        if let Some(mc_world::BlockEntityData::Furnace { input, fuel, .. }) = world.get_block_entity_mut(furnace_pos) {
            *input = ItemStack::new(data.items.by_name("raw_iron").unwrap(), 1);
            *fuel = ItemStack::new(data.items.by_name("coal").unwrap(), 1);
        }
        for _ in 0..205 {
            world.tick(&data, ChunkPos::new(0, 0));
        }
        if let Some(mc_world::BlockEntityData::Furnace { output, .. }) = world.get_block_entity(furnace_pos) {
            assert_eq!(output.item, data.items.by_name("iron_ingot").unwrap());
        } else {
            panic!("expected furnace block entity");
        }

        // 4. Portal & Dimension Transitions (Overworld -> TheNether & TheEnd)
        let nether_chunk = world_gen.generate_chunk(DimensionId::TheNether, ChunkPos::new(0, 0), &data);
        let mut nether = World::new(DimensionId::TheNether, 2612026);
        nether.insert_chunk(nether_chunk, &data.blocks);
        assert_eq!(nether.get_block(BlockPos::new(0, 0, 0)).id(), data.blocks.bedrock);
        let end_chunk = world_gen.generate_chunk(DimensionId::TheEnd, ChunkPos::new(0, 0), &data);
        let mut end = World::new(DimensionId::TheEnd, 2612026);
        end.insert_chunk(end_chunk, &data.blocks);
        assert_eq!(end.get_block(BlockPos::new(0, 62, 0)).id(), data.blocks.end_stone);

        // 5. Combat & command system
        let z_id = entities.spawn(EntityKind::Zombie, player.pos + DVec3::new(1.5, 0.0, 0.0), &data);
        let killed = entities.damage_entity(z_id, 50.0, DVec3::X, &mut world, &data);
        let _ = killed;
        let reply = execute_command("/time set noon", &mut player, &mut world, &mut entities, &data);
        assert!(reply.contains("6000"));

        // 5. Anvil .mca + level.dat save & reload round-trip
        let tmp_dir = PathBuf::from("target/test_world_save");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        save::save_dirty_chunks(&tmp_dir, &mut world, &data).unwrap();
        save::save_level_dat(&tmp_dir, &world, "Test World", &player).unwrap();

        let (meta, loaded_p) = save::load_level_dat(&tmp_dir.join("level.dat")).unwrap();
        assert_eq!(meta.seed, 2612026);
        assert_eq!(meta.day_time, 6000);
        assert_eq!(loaded_p.unwrap().name, "Steve");

        let reloaded_chunk = save::load_chunk_from_disk(&tmp_dir, DimensionId::Overworld, ChunkPos::new(0, 0), &data)
            .expect("chunk must reload from .mca");
        assert_eq!(
            reloaded_chunk.get_block(target.local_x(), target.y, target.local_z()).id(),
            data.blocks.stone
        );
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
