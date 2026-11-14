use crate::atlas::TextureAtlas;
use mc_core::{ChunkPos, Direction};
use mc_data::{BlockDef, BlockRegistry, BlockShape, BlockState, TintKind};
use mc_world::World;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct WorldVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    pub light: [f32; 2],
}

impl WorldVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<WorldVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 20,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: 36,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

#[derive(Default, Clone)]
pub struct MeshData {
    pub vertices: Vec<WorldVertex>,
    pub indices: Vec<u32>,
}

impl MeshData {
    pub fn push_quad(
        &mut self,
        p0: [f32; 3],
        p1: [f32; 3],
        p2: [f32; 3],
        p3: [f32; 3],
        uv: [f32; 4],
        colors: [[f32; 4]; 4],
        lights: [[f32; 2]; 4],
    ) {
        let base = self.vertices.len() as u32;
        self.vertices.push(WorldVertex {
            pos: p0,
            uv: [uv[0], uv[1]],
            color: colors[0],
            light: lights[0],
        });
        self.vertices.push(WorldVertex {
            pos: p1,
            uv: [uv[0], uv[3]],
            color: colors[1],
            light: lights[1],
        });
        self.vertices.push(WorldVertex {
            pos: p2,
            uv: [uv[2], uv[3]],
            color: colors[2],
            light: lights[2],
        });
        self.vertices.push(WorldVertex {
            pos: p3,
            uv: [uv[2], uv[1]],
            color: colors[3],
            light: lights[3],
        });
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

#[derive(Default)]
pub struct ChunkMeshResult {
    pub chunk_pos: ChunkPos,
    pub opaque: MeshData,
    pub cutout: MeshData,
    pub translucent: MeshData,
}

/// Thread-safe, self-contained 18 x H x 18 neighborhood captured from `World`
/// so chunk meshing can execute fully lock-free in `rayon`.
pub struct ChunkNeighborhood {
    pub chunk_pos: ChunkPos,
    pub min_y: i32,
    pub height: i32,
    pub states: Vec<BlockState>,
    pub sky_light: Vec<u8>,
    pub block_light: Vec<u8>,
    pub column_tints: Vec<([f32; 3], [f32; 3], [f32; 3])>, // 16x16 (grass, foliage, water)
}

impl ChunkNeighborhood {
    pub fn capture(world: &World, chunk_pos: ChunkPos) -> Option<Self> {
        let center = world.chunks.get(&chunk_pos)?;
        let min_y = center.min_y;
        let height = (center.sections.len() as i32) * 16;
        let len = (18 * 18 * height) as usize;
        let mut states = vec![BlockState::AIR; len];
        let mut sky_light = vec![15u8; len];
        let mut block_light = vec![0u8; len];

        let _base_wx = chunk_pos.x * 16;
        let _base_wz = chunk_pos.z * 16;

        // Fill center and 1-block border from 3x3 chunk neighborhood
        for dz in -1i32..=1 {
            for dx in -1i32..=1 {
                let ncpos = ChunkPos::new(chunk_pos.x + dx, chunk_pos.z + dz);
                let Some(chunk) = world.chunks.get(&ncpos) else {
                    continue;
                };
                let x_start = if dx == -1 { 15 } else { 0 };
                let x_end = if dx == 1 { 1 } else { 16 };
                let z_start = if dz == -1 { 15 } else { 0 };
                let z_end = if dz == 1 { 1 } else { 16 };

                for (si, sec) in chunk.sections.iter().enumerate() {
                    let sec_y0 = chunk.min_y + (si as i32) * 16;
                    let empty_sec = sec.non_air_count == 0;
                    for ly in 0..16 {
                        let wy = sec_y0 + ly as i32;
                        let yi = (wy - min_y) as usize;
                        if yi >= height as usize {
                            continue;
                        }
                        for lz in z_start..z_end {
                            let wz_local = dz * 16 + lz as i32 + 1;
                            for lx in x_start..x_end {
                                let wx_local = dx * 16 + lx as i32 + 1;
                                let idx = (yi * 18 + wz_local as usize) * 18 + wx_local as usize;
                                if !empty_sec {
                                    states[idx] = sec.get(lx, ly, lz);
                                }
                                sky_light[idx] = sec.get_sky_light(lx, ly, lz);
                                block_light[idx] = sec.get_block_light(lx, ly, lz);
                            }
                        }
                    }
                }
            }
        }

        let mut column_tints = Vec::with_capacity(256);
        for z in 0..16 {
            for x in 0..16 {
                let biome = center.biome_at(x, z);
                column_tints.push(biome_tints(biome));
            }
        }

        Some(Self {
            chunk_pos,
            min_y,
            height,
            states,
            sky_light,
            block_light,
            column_tints,
        })
    }

    #[inline]
    fn idx(&self, lx: i32, y: i32, lz: i32) -> Option<usize> {
        let yi = y - self.min_y;
        if yi < 0 || yi >= self.height || lx < -1 || lx > 16 || lz < -1 || lz > 16 {
            return None;
        }
        Some(((yi as usize) * 18 + ((lz + 1) as usize)) * 18 + ((lx + 1) as usize))
    }

    #[inline]
    pub fn get(&self, lx: i32, y: i32, lz: i32) -> BlockState {
        self.idx(lx, y, lz)
            .map(|i| self.states[i])
            .unwrap_or(BlockState::AIR)
    }

    #[inline]
    pub fn light(&self, lx: i32, y: i32, lz: i32) -> [f32; 2] {
        if let Some(i) = self.idx(lx, y, lz) {
            [
                self.sky_light[i] as f32 / 15.0,
                self.block_light[i] as f32 / 15.0,
            ]
        } else if y >= self.min_y + self.height {
            [1.0, 0.0]
        } else {
            [0.0, 0.0]
        }
    }

    #[inline]
    fn is_occluder(&self, lx: i32, y: i32, lz: i32, blocks: &BlockRegistry) -> bool {
        let s = self.get(lx, y, lz);
        if s.is_air() {
            return false;
        }
        blocks.get(s.id()).is_opaque
    }
}

pub fn mesh_chunk(
    hood: &ChunkNeighborhood,
    blocks: &BlockRegistry,
    atlas: &TextureAtlas,
) -> ChunkMeshResult {
    let mut res = ChunkMeshResult {
        chunk_pos: hood.chunk_pos,
        opaque: MeshData::default(),
        cutout: MeshData::default(),
        translucent: MeshData::default(),
    };

    let wx0 = (hood.chunk_pos.x * 16) as f32;
    let wz0 = (hood.chunk_pos.z * 16) as f32;
    let max_y = hood.min_y + hood.height;

    for y in hood.min_y..max_y {
        for lz in 0..16i32 {
            for lx in 0..16i32 {
                let state = hood.get(lx, y, lz);
                if state.is_air() {
                    continue;
                }
                let def = blocks.get(state.id());
                if def.shape == BlockShape::Empty {
                    continue;
                }

                let bx = wx0 + lx as f32;
                let by = y as f32;
                let bz = wz0 + lz as f32;
                let tint_tuple = hood.column_tints[(lz * 16 + lx) as usize];
                let tint = resolve_tint(def, state, tint_tuple);
                let face_uvs = atlas.block_uv(state.id());

                match def.shape {
                    BlockShape::Full => {
                        emit_full_block(
                            hood,
                            blocks,
                            def,
                            state,
                            lx,
                            y,
                            lz,
                            [bx, by, bz],
                            tint,
                            face_uvs,
                            &mut res,
                        );
                    }
                    BlockShape::Fluid => {
                        emit_fluid_block(
                            hood,
                            blocks,
                            def,
                            state,
                            lx,
                            y,
                            lz,
                            [bx, by, bz],
                            tint,
                            face_uvs,
                            &mut res,
                        );
                    }
                    BlockShape::Cross => {
                        let l = hood.light(lx, y, lz);
                        let color = [tint[0], tint[1], tint[2], 1.0];
                        emit_cross(
                            [bx, by, bz],
                            face_uvs.side,
                            color,
                            l,
                            &mut res.cutout,
                        );
                    }
                    BlockShape::Torch => {
                        let l = [1.0, 1.0];
                        let color = [1.0, 1.0, 1.0, 1.0];
                        emit_box(
                            [bx + 0.4375, by, bz + 0.4375],
                            [bx + 0.5625, by + 0.625, bz + 0.5625],
                            face_uvs,
                            Direction::North,
                            color,
                            l,
                            &mut res.cutout,
                        );
                    }
                    BlockShape::Slab => {
                        let (y0, y1) = if state.is_top_half() {
                            (0.5, 1.0)
                        } else {
                            (0.0, 0.5)
                        };
                        let l = neighbor_max_light(hood, lx, y, lz);
                        emit_box(
                            [bx, by + y0, bz],
                            [bx + 1.0, by + y1, bz + 1.0],
                            face_uvs,
                            state.facing(),
                            [tint[0], tint[1], tint[2], 1.0],
                            l,
                            &mut res.opaque,
                        );
                    }
                    BlockShape::Stair => {
                        let l = neighbor_max_light(hood, lx, y, lz);
                        let col = [tint[0], tint[1], tint[2], 1.0];
                        let facing = state.facing();
                        let top = state.is_top_half();
                        let (base_y0, base_y1, step_y0, step_y1) = if top {
                            (0.5, 1.0, 0.0, 0.5)
                        } else {
                            (0.0, 0.5, 0.5, 1.0)
                        };
                        emit_box(
                            [bx, by + base_y0, bz],
                            [bx + 1.0, by + base_y1, bz + 1.0],
                            face_uvs,
                            facing,
                            col,
                            l,
                            &mut res.opaque,
                        );
                        let (sx0, sz0, sx1, sz1) = match facing {
                            Direction::North => (0.0, 0.0, 1.0, 0.5),
                            Direction::South => (0.0, 0.5, 1.0, 1.0),
                            Direction::West => (0.0, 0.0, 0.5, 1.0),
                            _ => (0.5, 0.0, 1.0, 1.0),
                        };
                        emit_box(
                            [bx + sx0, by + step_y0, bz + sz0],
                            [bx + sx1, by + step_y1, bz + sz1],
                            face_uvs,
                            facing,
                            col,
                            l,
                            &mut res.opaque,
                        );
                    }
                    BlockShape::Carpet | BlockShape::SnowLayer => {
                        let h = if def.shape == BlockShape::Carpet {
                            0.0625
                        } else {
                            ((state.level().min(7) + 1) as f32) * 0.125
                        };
                        let l = neighbor_max_light(hood, lx, y, lz);
                        emit_box(
                            [bx, by, bz],
                            [bx + 1.0, by + h, bz + 1.0],
                            face_uvs,
                            Direction::North,
                            [1.0, 1.0, 1.0, 1.0],
                            l,
                            &mut res.opaque,
                        );
                    }
                    BlockShape::Wire | BlockShape::Repeater => {
                        let l = hood.light(lx, y, lz);
                        let h = if def.shape == BlockShape::Wire { 0.03 } else { 0.125 };
                        emit_box(
                            [bx + 0.05, by, bz + 0.05],
                            [bx + 0.95, by + h, bz + 0.95],
                            face_uvs,
                            state.facing(),
                            [tint[0], tint[1], tint[2], 1.0],
                            l,
                            &mut res.cutout,
                        );
                    }
                    BlockShape::Door => {
                        let l = neighbor_max_light(hood, lx, y, lz);
                        let dir = if state.is_active() {
                            match state.facing() {
                                Direction::North => Direction::East,
                                Direction::East => Direction::South,
                                Direction::South => Direction::West,
                                _ => Direction::North,
                            }
                        } else {
                            state.facing()
                        };
                        let (x0, z0, x1, z1) = match dir {
                            Direction::North => (0.0, 0.0, 1.0, 0.1875),
                            Direction::South => (0.0, 0.8125, 1.0, 1.0),
                            Direction::West => (0.0, 0.0, 0.1875, 1.0),
                            _ => (0.8125, 0.0, 1.0, 1.0),
                        };
                        emit_box(
                            [bx + x0, by, bz + z0],
                            [bx + x1, by + 1.0, bz + z1],
                            face_uvs,
                            dir,
                            [1.0; 4],
                            l,
                            &mut res.cutout,
                        );
                    }
                    BlockShape::Trapdoor => {
                        let l = neighbor_max_light(hood, lx, y, lz);
                        if state.is_active() {
                            let (x0, z0, x1, z1) = match state.facing() {
                                Direction::North => (0.0, 0.8125, 1.0, 1.0),
                                Direction::South => (0.0, 0.0, 1.0, 0.1875),
                                Direction::West => (0.8125, 0.0, 1.0, 1.0),
                                _ => (0.0, 0.0, 0.1875, 1.0),
                            };
                            emit_box(
                                [bx + x0, by, bz + z0],
                                [bx + x1, by + 1.0, bz + z1],
                                face_uvs,
                                state.facing(),
                                [1.0; 4],
                                l,
                                &mut res.cutout,
                            );
                        } else {
                            let (y0, y1) = if state.is_top_half() {
                                (0.8125, 1.0)
                            } else {
                                (0.0, 0.1875)
                            };
                            emit_box(
                                [bx, by + y0, bz],
                                [bx + 1.0, by + y1, bz + 1.0],
                                face_uvs,
                                Direction::North,
                                [1.0; 4],
                                l,
                                &mut res.cutout,
                            );
                        }
                    }
                    BlockShape::Fence | BlockShape::Wall | BlockShape::Pane => {
                        let l = neighbor_max_light(hood, lx, y, lz);
                        let hw = if def.shape == BlockShape::Fence { 0.125 } else { 0.25 };
                        emit_box(
                            [bx + 0.5 - hw, by, bz + 0.5 - hw],
                            [bx + 0.5 + hw, by + 1.0, bz + 0.5 + hw],
                            face_uvs,
                            Direction::North,
                            [1.0; 4],
                            l,
                            &mut res.opaque,
                        );
                        let arm_w = 0.08;
                        for d in [Direction::North, Direction::South, Direction::West, Direction::East] {
                            let n = d.normal();
                            let nb = hood.get(lx + n.x, y + n.y, lz + n.z);
                            if !nb.is_air() && (blocks.get(nb.id()).is_solid || blocks.get(nb.id()).shape == def.shape) {
                                let (ax0, az0, ax1, az1) = match d {
                                    Direction::North => (0.5 - arm_w, 0.0, 0.5 + arm_w, 0.5),
                                    Direction::South => (0.5 - arm_w, 0.5, 0.5 + arm_w, 1.0),
                                    Direction::West => (0.0, 0.5 - arm_w, 0.5, 0.5 + arm_w),
                                    _ => (0.5, 0.5 - arm_w, 1.0, 0.5 + arm_w),
                                };
                                emit_box(
                                    [bx + ax0, by + 0.35, bz + az0],
                                    [bx + ax1, by + 0.85, bz + az1],
                                    face_uvs,
                                    d,
                                    [1.0; 4],
                                    l,
                                    &mut res.opaque,
                                );
                            }
                        }
                    }
                    BlockShape::Chest => {
                        let l = neighbor_max_light(hood, lx, y, lz);
                        emit_box(
                            [bx + 0.0625, by, bz + 0.0625],
                            [bx + 0.9375, by + 0.875, bz + 0.9375],
                            face_uvs,
                            state.facing(),
                            [1.0; 4],
                            l,
                            &mut res.opaque,
                        );
                    }
                    BlockShape::Bed => {
                        let l = neighbor_max_light(hood, lx, y, lz);
                        emit_box(
                            [bx, by + 0.1875, bz],
                            [bx + 1.0, by + 0.5625, bz + 1.0],
                            face_uvs,
                            state.facing(),
                            [1.0; 4],
                            l,
                            &mut res.opaque,
                        );
                    }
                    BlockShape::Cactus => {
                        let l = neighbor_max_light(hood, lx, y, lz);
                        emit_box(
                            [bx + 0.0625, by, bz + 0.0625],
                            [bx + 0.9375, by + 1.0, bz + 0.9375],
                            face_uvs,
                            Direction::North,
                            [1.0; 4],
                            l,
                            &mut res.cutout,
                        );
                    }
                    BlockShape::Farmland => {
                        let l = neighbor_max_light(hood, lx, y, lz);
                        emit_box(
                            [bx, by, bz],
                            [bx + 1.0, by + 0.9375, bz + 1.0],
                            face_uvs,
                            Direction::North,
                            [1.0; 4],
                            l,
                            &mut res.opaque,
                        );
                    }
                    BlockShape::Portal => {
                        let l = [1.0, 1.0];
                        let facing = state.facing();
                        let (x0, z0, x1, z1) = match facing {
                            Direction::East | Direction::West => (0.375, 0.0, 0.625, 1.0),
                            _ => (0.0, 0.375, 1.0, 0.625),
                        };
                        emit_box(
                            [bx + x0, by, bz + z0],
                            [bx + x1, by + 1.0, bz + z1],
                            face_uvs,
                            facing,
                            [0.85, 0.5, 1.0, 0.78],
                            l,
                            &mut res.translucent,
                        );
                    }
                    BlockShape::Empty => {}
                }
            }
        }
    }

    res
}

fn biome_tints(biome: &str) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let short = biome.strip_prefix("minecraft:").unwrap_or(biome);
    if short.contains("desert") || short.contains("savanna") || short.contains("badlands") {
        ([0.75, 0.72, 0.33], [0.68, 0.62, 0.25], [0.25, 0.46, 0.89])
    } else if short.contains("snow") || short.contains("frozen") || short.contains("ice") || short.contains("grove") {
        ([0.50, 0.70, 0.59], [0.38, 0.60, 0.48], [0.22, 0.35, 0.84])
    } else if short.contains("taiga") || short.contains("spruce") {
        ([0.52, 0.72, 0.51], [0.38, 0.60, 0.42], [0.25, 0.46, 0.89])
    } else if short.contains("jungle") || short.contains("bamboo") {
        ([0.35, 0.79, 0.24], [0.30, 0.72, 0.18], [0.26, 0.70, 0.82])
    } else if short.contains("swamp") || short.contains("mangrove") {
        ([0.42, 0.44, 0.24], [0.38, 0.40, 0.22], [0.38, 0.48, 0.39])
    } else if short.contains("cherry") {
        ([0.71, 0.86, 0.38], [0.60, 0.80, 0.30], [0.25, 0.46, 0.89])
    } else {
        // Plains / Forest
        ([0.47, 0.75, 0.35], [0.38, 0.66, 0.26], [0.25, 0.46, 0.89])
    }
}

fn resolve_tint(def: &BlockDef, state: BlockState, tints: ([f32; 3], [f32; 3], [f32; 3])) -> [f32; 3] {
    match def.tint {
        TintKind::Grass => tints.0,
        TintKind::Foliage => tints.1,
        TintKind::Water => tints.2,
        TintKind::Redstone => {
            let p = (state.level() as f32) / 15.0;
            [0.30 + 0.70 * p, 0.04 + 0.10 * p, 0.04]
        }
        TintKind::None => [1.0, 1.0, 1.0],
    }
}

fn dir_shade(dir: Direction) -> f32 {
    match dir {
        Direction::Up => 1.0,
        Direction::Down => 0.52,
        Direction::North | Direction::South => 0.80,
        Direction::East | Direction::West => 0.68,
    }
}

fn emit_full_block(
    hood: &ChunkNeighborhood,
    blocks: &BlockRegistry,
    def: &BlockDef,
    state: BlockState,
    lx: i32,
    y: i32,
    lz: i32,
    base: [f32; 3],
    tint: [f32; 3],
    uvs: crate::atlas::BlockFaceUvs,
    res: &mut ChunkMeshResult,
) {
    let target_is_translucent = def.is_translucent;
    let target_is_cutout = def.is_cutout;

    let facing = state.facing();
    for dir in Direction::ALL {
        let n = dir.normal();
        let (nx, ny, nz) = (n.x, n.y, n.z);
        let nb = hood.get(lx + nx, y + ny, lz + nz);
        if !nb.is_air() {
            let ndef = blocks.get(nb.id());
            if def.is_opaque && ndef.is_opaque {
                continue;
            }
            if def.is_translucent && nb.id() == def.id {
                continue;
            }
        }

        let is_grass_block = def.id == blocks.grass_block;
        let face_tint = if is_grass_block {
            if dir == Direction::Up {
                tint
            } else {
                [1.0, 1.0, 1.0]
            }
        } else {
            tint
        };

        let uv = uvs.for_face(dir, facing);
        let [p0, p1, p2, p3] = face_verts(base, [base[0] + 1.0, base[1] + 1.0, base[2] + 1.0], dir);
        let shade = dir_shade(dir);
        let l = if def.light_emission >= 12 {
            [[1.0, 1.0]; 4]
        } else {
            let nl = hood.light(lx + nx, y + ny, lz + nz);
            [nl; 4]
        };
        let ao = face_ao(hood, blocks, lx, y, lz, dir);
        let alpha = if def.is_translucent { 0.82 } else { 1.0 };
        let colors = [
            [face_tint[0] * shade * ao[0], face_tint[1] * shade * ao[0], face_tint[2] * shade * ao[0], alpha],
            [face_tint[0] * shade * ao[1], face_tint[1] * shade * ao[1], face_tint[2] * shade * ao[1], alpha],
            [face_tint[0] * shade * ao[2], face_tint[1] * shade * ao[2], face_tint[2] * shade * ao[2], alpha],
            [face_tint[0] * shade * ao[3], face_tint[1] * shade * ao[3], face_tint[2] * shade * ao[3], alpha],
        ];
        if target_is_translucent {
            res.translucent.push_quad(p0, p1, p2, p3, uv, colors, l);
        } else if target_is_cutout {
            res.cutout.push_quad(p0, p1, p2, p3, uv, colors, l);
        } else {
            res.opaque.push_quad(p0, p1, p2, p3, uv, colors, l);
        }

        // Emit grass side overlay onto cutout pass slightly nudged outward along normal
        if is_grass_block && dir != Direction::Up && dir != Direction::Down {
            if let Some(ov_uv) = uvs.overlay {
                let eps = 0.0015;
                let off = [nx as f32 * eps, 0.0, nz as f32 * eps];
                let ocolors = [
                    [tint[0] * shade * ao[0], tint[1] * shade * ao[0], tint[2] * shade * ao[0], 1.0],
                    [tint[0] * shade * ao[1], tint[1] * shade * ao[1], tint[2] * shade * ao[1], 1.0],
                    [tint[0] * shade * ao[2], tint[1] * shade * ao[2], tint[2] * shade * ao[2], 1.0],
                    [tint[0] * shade * ao[3], tint[1] * shade * ao[3], tint[2] * shade * ao[3], 1.0],
                ];
                res.cutout.push_quad(
                    [p0[0] + off[0], p0[1], p0[2] + off[2]],
                    [p1[0] + off[0], p1[1], p1[2] + off[2]],
                    [p2[0] + off[0], p2[1], p2[2] + off[2]],
                    [p3[0] + off[0], p3[1], p3[2] + off[2]],
                    ov_uv,
                    ocolors,
                    l,
                );
            }
        }
    }
}

fn emit_fluid_block(
    hood: &ChunkNeighborhood,
    blocks: &BlockRegistry,
    def: &BlockDef,
    state: BlockState,
    lx: i32,
    y: i32,
    lz: i32,
    base: [f32; 3],
    tint: [f32; 3],
    uvs: crate::atlas::BlockFaceUvs,
    res: &mut ChunkMeshResult,
) {
    let above_same = hood.get(lx, y + 1, lz).id() == def.id;
    let top_h = if above_same {
        1.0
    } else {
        let lvl = state.level().min(7) as f32;
        ((8.0 - lvl) / 8.0) * 0.88
    };
    let is_water = def.id == blocks.water;
    let (target, alpha, light_override) = if is_water {
        (&mut res.translucent, 0.72, None)
    } else {
        (&mut res.opaque, 1.0, Some([1.0, 1.0]))
    };

    for dir in Direction::ALL {
        let n = dir.normal();
        let (nx, ny, nz) = (n.x, n.y, n.z);
        let nb = hood.get(lx + nx, y + ny, lz + nz);
        if nb.id() == def.id {
            continue;
        }
        if !nb.is_air() && blocks.get(nb.id()).is_opaque && !(dir == Direction::Up && top_h < 0.99) {
            continue;
        }
        let uv = uvs.for_face(dir, Direction::North);
        let [p0, p1, p2, p3] = face_verts(base, [base[0] + 1.0, base[1] + top_h, base[2] + 1.0], dir);
        let shade = dir_shade(dir);
        let nl = light_override.unwrap_or_else(|| hood.light(lx + nx, y + ny, lz + nz));
        let c = [tint[0] * shade, tint[1] * shade, tint[2] * shade, alpha];
        target.push_quad(p0, p1, p2, p3, uv, [c; 4], [nl; 4]);
    }
}

fn emit_box(
    min: [f32; 3],
    max: [f32; 3],
    uvs: crate::atlas::BlockFaceUvs,
    facing: Direction,
    color: [f32; 4],
    light: [f32; 2],
    mesh: &mut MeshData,
) {
    for dir in Direction::ALL {
        let [p0, p1, p2, p3] = face_verts(min, max, dir);
        let shade = dir_shade(dir);
        let c = [color[0] * shade, color[1] * shade, color[2] * shade, color[3]];
        mesh.push_quad(p0, p1, p2, p3, uvs.for_face(dir, facing), [c; 4], [light; 4]);
    }
}

fn emit_cross(
    base: [f32; 3],
    uv: [f32; 4],
    color: [f32; 4],
    light: [f32; 2],
    mesh: &mut MeshData,
) {
    let [x0, y0, z0] = [base[0] + 0.15, base[1], base[2] + 0.15];
    let [x1, y1, z1] = [base[0] + 0.85, base[1] + 0.88, base[2] + 0.85];
    // Diagonal 1 (both front and back)
    mesh.push_quad([x0, y1, z0], [x0, y0, z0], [x1, y0, z1], [x1, y1, z1], uv, [color; 4], [light; 4]);
    mesh.push_quad([x1, y1, z1], [x1, y0, z1], [x0, y0, z0], [x0, y1, z0], uv, [color; 4], [light; 4]);
    // Diagonal 2 (both front and back)
    mesh.push_quad([x0, y1, z1], [x0, y0, z1], [x1, y0, z0], [x1, y1, z0], uv, [color; 4], [light; 4]);
    mesh.push_quad([x1, y1, z0], [x1, y0, z0], [x0, y0, z1], [x0, y1, z1], uv, [color; 4], [light; 4]);
}

/// CCW outward-facing quad vertices for each cardinal direction (`0=top-left, 1=bot-left, 2=bot-right, 3=top-right`).
pub fn face_verts(min: [f32; 3], max: [f32; 3], dir: Direction) -> [[f32; 3]; 4] {
    let [x0, y0, z0] = min;
    let [x1, y1, z1] = max;
    match dir {
        Direction::Up => [[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]],
        Direction::Down => [[x0, y0, z1], [x0, y0, z0], [x1, y0, z0], [x1, y0, z1]],
        Direction::North => [[x1, y1, z0], [x1, y0, z0], [x0, y0, z0], [x0, y1, z0]],
        Direction::South => [[x0, y1, z1], [x0, y0, z1], [x1, y0, z1], [x1, y1, z1]],
        Direction::West => [[x0, y1, z0], [x0, y0, z0], [x0, y0, z1], [x0, y1, z1]],
        Direction::East => [[x1, y1, z1], [x1, y0, z1], [x1, y0, z0], [x1, y1, z0]],
    }
}

fn neighbor_max_light(hood: &ChunkNeighborhood, lx: i32, y: i32, lz: i32) -> [f32; 2] {
    let mut best = hood.light(lx, y, lz);
    for d in Direction::ALL {
        let n = d.normal();
        let l = hood.light(lx + n.x, y + n.y, lz + n.z);
        best[0] = best[0].max(l[0]);
        best[1] = best[1].max(l[1]);
    }
    best
}

fn face_ao(
    hood: &ChunkNeighborhood,
    blocks: &BlockRegistry,
    lx: i32,
    y: i32,
    lz: i32,
    dir: Direction,
) -> [f32; 4] {
    let n = dir.normal();
    let fx = lx + n.x;
    let fy = y + n.y;
    let fz = lz + n.z;
    let occ = |dx: i32, dy: i32, dz: i32| -> u8 {
        if hood.is_occluder(fx + dx, fy + dy, fz + dz, blocks) {
            1
        } else {
            0
        }
    };
    let ao_val = |s1: u8, s2: u8, c: u8| -> f32 {
        let level = if s1 == 1 && s2 == 1 { 0 } else { 3 - (s1 + s2 + c) };
        0.55 + (level as f32) * 0.15
    };
    match dir {
        Direction::Up => [
            ao_val(occ(-1, 0, 0), occ(0, 0, -1), occ(-1, 0, -1)),
            ao_val(occ(-1, 0, 0), occ(0, 0, 1), occ(-1, 0, 1)),
            ao_val(occ(1, 0, 0), occ(0, 0, 1), occ(1, 0, 1)),
            ao_val(occ(1, 0, 0), occ(0, 0, -1), occ(1, 0, -1)),
        ],
        Direction::Down => [
            ao_val(occ(-1, 0, 0), occ(0, 0, 1), occ(-1, 0, 1)),
            ao_val(occ(-1, 0, 0), occ(0, 0, -1), occ(-1, 0, -1)),
            ao_val(occ(1, 0, 0), occ(0, 0, -1), occ(1, 0, -1)),
            ao_val(occ(1, 0, 0), occ(0, 0, 1), occ(1, 0, 1)),
        ],
        Direction::North => [
            ao_val(occ(1, 0, 0), occ(0, 1, 0), occ(1, 1, 0)),
            ao_val(occ(1, 0, 0), occ(0, -1, 0), occ(1, -1, 0)),
            ao_val(occ(-1, 0, 0), occ(0, -1, 0), occ(-1, -1, 0)),
            ao_val(occ(-1, 0, 0), occ(0, 1, 0), occ(-1, 1, 0)),
        ],
        Direction::South => [
            ao_val(occ(-1, 0, 0), occ(0, 1, 0), occ(-1, 1, 0)),
            ao_val(occ(-1, 0, 0), occ(0, -1, 0), occ(-1, -1, 0)),
            ao_val(occ(1, 0, 0), occ(0, -1, 0), occ(1, -1, 0)),
            ao_val(occ(1, 0, 0), occ(0, 1, 0), occ(1, 1, 0)),
        ],
        Direction::West => [
            ao_val(occ(0, 0, -1), occ(0, 1, 0), occ(0, 1, -1)),
            ao_val(occ(0, 0, -1), occ(0, -1, 0), occ(0, -1, -1)),
            ao_val(occ(0, 0, 1), occ(0, -1, 0), occ(0, -1, 1)),
            ao_val(occ(0, 0, 1), occ(0, 1, 0), occ(0, 1, 1)),
        ],
        Direction::East => [
            ao_val(occ(0, 0, 1), occ(0, 1, 0), occ(0, 1, 1)),
            ao_val(occ(0, 0, 1), occ(0, -1, 0), occ(0, -1, 1)),
            ao_val(occ(0, 0, -1), occ(0, -1, 0), occ(0, -1, -1)),
            ao_val(occ(0, 0, -1), occ(0, 1, 0), occ(0, 1, -1)),
        ],
    }
}
