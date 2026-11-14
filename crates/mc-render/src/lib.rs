pub mod atlas;
pub mod font;
pub mod mesher;
pub mod renderer;

pub use atlas::{BlockFaceUvs, TextureAtlas};
pub use mesher::{mesh_chunk, ChunkMeshResult, ChunkNeighborhood, MeshData, WorldVertex};
pub use renderer::{FrameRenderInput, Renderer, UiBatch, UiVertex};

#[cfg(test)]
mod tests {
    use super::*;
    use mc_core::{BlockPos, ChunkPos, DimensionId};
    use mc_data::{BlockState, GameData};
    use mc_world::World;

    #[test]
    fn atlas_and_chunk_mesher_produce_geometry() {
        let data = GameData::load();
        let atlas = TextureAtlas::build(&data);
        assert!(atlas.width >= 1024);
        assert!(atlas.height >= 18);
        assert_eq!(atlas.rgba.len(), (atlas.width * atlas.height * 4) as usize);

        let mut world = World::new(DimensionId::Overworld, 42);
        let chunk = mc_world::Chunk::new(ChunkPos::new(0, 0), DimensionId::Overworld);
        world.chunks.insert(ChunkPos::new(0, 0), chunk);
        world.set_block(
            BlockPos::new(4, 64, 4),
            BlockState::new(data.blocks.stone),
            &data.blocks,
        );
        world.set_block(
            BlockPos::new(4, 65, 4),
            BlockState::new(data.blocks.grass_block),
            &data.blocks,
        );
        world.set_block(
            BlockPos::new(5, 65, 4),
            BlockState::new(data.blocks.water),
            &data.blocks,
        );

        let hood = ChunkNeighborhood::capture(&world, ChunkPos::new(0, 0)).unwrap();
        let mesh = mesh_chunk(&hood, &data.blocks, &atlas);
        assert!(!mesh.opaque.indices.is_empty());
        assert!(!mesh.cutout.indices.is_empty());
        assert!(!mesh.translucent.indices.is_empty());
    }
}
