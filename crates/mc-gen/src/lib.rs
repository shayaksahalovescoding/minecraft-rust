pub mod end;
pub mod nether;
pub mod noise;
pub mod overworld;

use mc_core::{ChunkPos, DimensionId};
use mc_data::GameData;
use mc_world::Chunk;

pub struct WorldGenerator {
    pub seed: i64,
    pub overworld: overworld::OverworldGenerator,
    pub nether: nether::NetherGenerator,
    pub end: end::EndGenerator,
}

impl WorldGenerator {
    pub fn new(seed: i64) -> Self {
        Self {
            seed,
            overworld: overworld::OverworldGenerator::new(seed),
            nether: nether::NetherGenerator::new(seed),
            end: end::EndGenerator::new(seed),
        }
    }

    pub fn generate_chunk(
        &self,
        dimension: DimensionId,
        pos: ChunkPos,
        data: &GameData,
    ) -> Chunk {
        match dimension {
            DimensionId::Overworld => self.overworld.generate_chunk(pos, data),
            DimensionId::TheNether => self.nether.generate_chunk(pos, data),
            DimensionId::TheEnd => self.end.generate_chunk(pos, data),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_overworld_nether_end_deterministically() {
        let data = GameData::load();
        let gen_a = WorldGenerator::new(42);
        let gen_b = WorldGenerator::new(42);

        let ow_a = gen_a.generate_chunk(DimensionId::Overworld, ChunkPos::new(0, 0), &data);
        let ow_b = gen_b.generate_chunk(DimensionId::Overworld, ChunkPos::new(0, 0), &data);
        assert_eq!(ow_a.get_block(8, -64, 8).id(), data.blocks.bedrock);
        assert_eq!(ow_a.heightmap, ow_b.heightmap);

        let nether = gen_a.generate_chunk(DimensionId::TheNether, ChunkPos::new(0, 0), &data);
        assert_eq!(nether.get_block(4, 0, 4).id(), data.blocks.bedrock);
        assert_eq!(nether.get_block(4, 127, 4).id(), data.blocks.bedrock);

        let end = gen_a.generate_chunk(DimensionId::TheEnd, ChunkPos::new(0, 0), &data);
        assert_eq!(end.get_block(8, 63, 8).id(), data.blocks.bedrock);
    }
}
