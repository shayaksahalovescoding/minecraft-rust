use crate::nbt::NbtTag;
use flate2::read::{GzDecoder, ZlibDecoder};
use flate2::write::{GzEncoder, ZlibEncoder};
use flate2::Compression;
use mc_core::ChunkPos;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Constants directly matching `net.minecraft.world.level.chunk.storage.RegionFile`.
pub const SECTOR_BYTES: usize = 4096;
pub const SECTOR_INTS: usize = 1024;
pub const CHUNK_HEADER_SIZE: usize = 5;
pub const EXTERNAL_STREAM_FLAG: u8 = 128;
pub const EXTERNAL_CHUNK_THRESHOLD: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RegionCompression {
    Gzip = 1,
    Zlib = 2,
    None = 3,
}

impl RegionCompression {
    pub fn from_byte(b: u8) -> io::Result<Self> {
        match b & 0x7F {
            1 => Ok(Self::Gzip),
            2 => Ok(Self::Zlib),
            3 => Ok(Self::None),
            other => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported region compression {other}"),
            )),
        }
    }
}

pub struct RegionFile {
    path: PathBuf,
    external_dir: PathBuf,
    file: File,
    offsets: [u32; SECTOR_INTS],
    timestamps: [u32; SECTOR_INTS],
    used_sectors: Vec<bool>,
}

impl RegionFile {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let external_dir = path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;

        let len = file.metadata()?.len() as usize;
        let mut offsets = [0u32; SECTOR_INTS];
        let mut timestamps = [0u32; SECTOR_INTS];

        if len < SECTOR_BYTES * 2 {
            file.set_len((SECTOR_BYTES * 2) as u64)?;
            file.seek(SeekFrom::Start(0))?;
            let zeros = vec![0u8; SECTOR_BYTES * 2];
            file.write_all(&zeros)?;
        } else {
            file.seek(SeekFrom::Start(0))?;
            let mut hdr = vec![0u8; SECTOR_BYTES * 2];
            file.read_exact(&mut hdr)?;
            for i in 0..SECTOR_INTS {
                offsets[i] = u32::from_be_bytes(hdr[i * 4..i * 4 + 4].try_into().unwrap());
                let t_off = SECTOR_BYTES + i * 4;
                timestamps[i] = u32::from_be_bytes(hdr[t_off..t_off + 4].try_into().unwrap());
            }
        }

        let total_sectors = (file.metadata()?.len() as usize).div_ceil(SECTOR_BYTES).max(2);
        let mut used_sectors = vec![false; total_sectors];
        used_sectors[0] = true;
        used_sectors[1] = true;

        for &entry in &offsets {
            if entry != 0 {
                let sec = (entry >> 8) as usize;
                let cnt = (entry & 0xFF) as usize;
                for s in sec..(sec + cnt) {
                    if s < used_sectors.len() {
                        used_sectors[s] = true;
                    }
                }
            }
        }

        Ok(Self {
            path,
            external_dir,
            file,
            offsets,
            timestamps,
            used_sectors,
        })
    }

    #[inline]
    fn index(pos: ChunkPos) -> usize {
        pos.region_local_x() + pos.region_local_z() * 32
    }

    pub fn has_chunk(&self, pos: ChunkPos) -> bool {
        self.offsets[Self::index(pos)] != 0
    }

    pub fn read_chunk_nbt(&mut self, pos: ChunkPos) -> io::Result<Option<NbtTag>> {
        let entry = self.offsets[Self::index(pos)];
        if entry == 0 {
            return Ok(None);
        }
        let sector = (entry >> 8) as usize;
        let num_sectors = (entry & 0xFF) as usize;
        if sector == 0 || num_sectors == 0 {
            return Ok(None);
        }

        self.file.seek(SeekFrom::Start((sector * SECTOR_BYTES) as u64))?;
        let mut len_buf = [0u8; 4];
        self.file.read_exact(&mut len_buf)?;
        let declared_len = u32::from_be_bytes(len_buf) as usize;
        if declared_len == 0 {
            return Ok(None);
        }
        let mut comp_buf = [0u8; 1];
        self.file.read_exact(&mut comp_buf)?;
        let comp_byte = comp_buf[0];
        let compression = RegionCompression::from_byte(comp_byte)?;

        let raw_payload = if (comp_byte & EXTERNAL_STREAM_FLAG) != 0 {
            let ext_path = self.external_chunk_path(pos);
            std::fs::read(ext_path)?
        } else {
            let payload_len = declared_len.saturating_sub(1);
            let mut buf = vec![0u8; payload_len];
            self.file.read_exact(&mut buf)?;
            buf
        };

        let (_, tag) = match compression {
            RegionCompression::Gzip => {
                let mut dec = GzDecoder::new(&raw_payload[..]);
                NbtTag::read_named(&mut dec)?
            }
            RegionCompression::Zlib => {
                let mut dec = ZlibDecoder::new(&raw_payload[..]);
                NbtTag::read_named(&mut dec)?
            }
            RegionCompression::None => {
                let mut cursor = io::Cursor::new(&raw_payload[..]);
                NbtTag::read_named(&mut cursor)?
            }
        };
        Ok(Some(tag))
    }

    pub fn write_chunk_nbt(
        &mut self,
        pos: ChunkPos,
        tag: &NbtTag,
        compression: RegionCompression,
    ) -> io::Result<()> {
        let payload = match compression {
            RegionCompression::Gzip => {
                let mut enc = GzEncoder::new(Vec::new(), Compression::default());
                tag.write_named(&mut enc, "")?;
                enc.finish()?
            }
            RegionCompression::Zlib => {
                let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
                tag.write_named(&mut enc, "")?;
                enc.finish()?
            }
            RegionCompression::None => {
                let mut buf = Vec::new();
                tag.write_named(&mut buf, "")?;
                buf
            }
        };

        let idx = Self::index(pos);
        let old_entry = self.offsets[idx];
        if old_entry != 0 {
            let old_sec = (old_entry >> 8) as usize;
            let old_cnt = (old_entry & 0xFF) as usize;
            for s in old_sec..(old_sec + old_cnt) {
                if s < self.used_sectors.len() {
                    self.used_sectors[s] = false;
                }
            }
        }

        let total_len = payload.len() + CHUNK_HEADER_SIZE;
        let needed_sectors = total_len.div_ceil(SECTOR_BYTES);

        let (write_buf, sectors_to_alloc) = if needed_sectors >= EXTERNAL_CHUNK_THRESHOLD {
            let ext_path = self.external_chunk_path(pos);
            std::fs::write(ext_path, &payload)?;
            let mut hdr = Vec::with_capacity(5);
            hdr.extend_from_slice(&1u32.to_be_bytes());
            hdr.push((compression as u8) | EXTERNAL_STREAM_FLAG);
            (hdr, 1)
        } else {
            let mut buf = Vec::with_capacity(needed_sectors * SECTOR_BYTES);
            buf.extend_from_slice(&((payload.len() + 1) as u32).to_be_bytes());
            buf.push(compression as u8);
            buf.extend_from_slice(&payload);
            buf.resize(needed_sectors * SECTOR_BYTES, 0);
            (buf, needed_sectors)
        };

        let start_sector = self.alloc_sectors(sectors_to_alloc);
        self.file.seek(SeekFrom::Start((start_sector * SECTOR_BYTES) as u64))?;
        self.file.write_all(&write_buf)?;

        let packed = ((start_sector as u32) << 8) | (sectors_to_alloc as u32 & 0xFF);
        self.offsets[idx] = packed;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as u32)
            .unwrap_or(0);
        self.timestamps[idx] = now;

        self.file.seek(SeekFrom::Start((idx * 4) as u64))?;
        self.file.write_all(&packed.to_be_bytes())?;
        self.file.seek(SeekFrom::Start((SECTOR_BYTES + idx * 4) as u64))?;
        self.file.write_all(&now.to_be_bytes())?;
        self.file.flush()?;
        Ok(())
    }

    fn alloc_sectors(&mut self, count: usize) -> usize {
        let mut run = 0;
        let mut start = 2;
        for i in 2..self.used_sectors.len() {
            if !self.used_sectors[i] {
                if run == 0 {
                    start = i;
                }
                run += 1;
                if run == count {
                    for s in start..(start + count) {
                        self.used_sectors[s] = true;
                    }
                    return start;
                }
            } else {
                run = 0;
            }
        }
        let start = self.used_sectors.len();
        self.used_sectors.resize(start + count, true);
        start
    }

    fn external_chunk_path(&self, pos: ChunkPos) -> PathBuf {
        self.external_dir.join(format!("c.{}.{}.mcc", pos.x, pos.z))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn nbt_and_region_roundtrip() {
        let mut root = BTreeMap::new();
        root.insert("DataVersion".to_string(), NbtTag::Int(4790));
        root.insert("xPos".to_string(), NbtTag::Int(3));
        root.insert("zPos".to_string(), NbtTag::Int(-2));
        root.insert("Biomes".to_string(), NbtTag::IntArray(vec![1, 2, 3, 4]));
        let tag = NbtTag::Compound(root);

        let gz = tag.to_gzip_bytes("").unwrap();
        let (_, back) = NbtTag::from_gzip_bytes(&gz).unwrap();
        assert_eq!(back, tag);

        let tmp = std::env::temp_dir().join("mc_test_region_0_0.mca");
        let _ = std::fs::remove_file(&tmp);
        {
            let mut rf = RegionFile::open(&tmp).unwrap();
            let pos = ChunkPos::new(3, -2);
            rf.write_chunk_nbt(pos, &tag, RegionCompression::Zlib).unwrap();
            assert!(rf.has_chunk(pos));
            let read_back = rf.read_chunk_nbt(pos).unwrap().unwrap();
            assert_eq!(read_back, tag);
        }
        let _ = std::fs::remove_file(&tmp);
    }
}
