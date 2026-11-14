use flate2::read::{GzDecoder, ZlibDecoder};
use flate2::write::{GzEncoder, ZlibEncoder};
use flate2::Compression;
use std::collections::BTreeMap;
use std::io::{self, Read, Write};

pub type NbtCompound = BTreeMap<String, NbtTag>;

#[derive(Debug, Clone, PartialEq)]
pub enum NbtTag {
    End,
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    List(Vec<NbtTag>),
    Compound(NbtCompound),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl NbtTag {
    pub fn type_id(&self) -> u8 {
        match self {
            Self::End => 0,
            Self::Byte(_) => 1,
            Self::Short(_) => 2,
            Self::Int(_) => 3,
            Self::Long(_) => 4,
            Self::Float(_) => 5,
            Self::Double(_) => 6,
            Self::ByteArray(_) => 7,
            Self::String(_) => 8,
            Self::List(_) => 9,
            Self::Compound(_) => 10,
            Self::IntArray(_) => 11,
            Self::LongArray(_) => 12,
        }
    }

    pub fn as_compound(&self) -> Option<&NbtCompound> {
        match self {
            Self::Compound(c) => Some(c),
            _ => None,
        }
    }

    pub fn as_compound_mut(&mut self) -> Option<&mut NbtCompound> {
        match self {
            Self::Compound(c) => Some(c),
            _ => None,
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Self::Byte(v) => Some(*v as i32),
            Self::Short(v) => Some(*v as i32),
            Self::Int(v) => Some(*v),
            Self::Long(v) => Some(*v as i32),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Byte(v) => Some(*v as i64),
            Self::Short(v) => Some(*v as i64),
            Self::Int(v) => Some(*v as i64),
            Self::Long(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float(v) => Some(*v as f64),
            Self::Double(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn write_named<W: Write>(&self, w: &mut W, name: &str) -> io::Result<()> {
        w.write_all(&[self.type_id()])?;
        if self.type_id() != 0 {
            write_string(w, name)?;
            self.write_payload(w)?;
        }
        Ok(())
    }

    pub fn write_payload<W: Write>(&self, w: &mut W) -> io::Result<()> {
        match self {
            Self::End => {}
            Self::Byte(v) => w.write_all(&[*v as u8])?,
            Self::Short(v) => w.write_all(&v.to_be_bytes())?,
            Self::Int(v) => w.write_all(&v.to_be_bytes())?,
            Self::Long(v) => w.write_all(&v.to_be_bytes())?,
            Self::Float(v) => w.write_all(&v.to_be_bytes())?,
            Self::Double(v) => w.write_all(&v.to_be_bytes())?,
            Self::ByteArray(arr) => {
                w.write_all(&(arr.len() as i32).to_be_bytes())?;
                for &b in arr {
                    w.write_all(&[b as u8])?;
                }
            }
            Self::String(s) => write_string(w, s)?,
            Self::List(items) => {
                let elem_id = items.first().map(|t| t.type_id()).unwrap_or(0);
                w.write_all(&[elem_id])?;
                w.write_all(&(items.len() as i32).to_be_bytes())?;
                for item in items {
                    item.write_payload(w)?;
                }
            }
            Self::Compound(map) => {
                for (k, v) in map {
                    v.write_named(w, k)?;
                }
                w.write_all(&[0])?;
            }
            Self::IntArray(arr) => {
                w.write_all(&(arr.len() as i32).to_be_bytes())?;
                for &v in arr {
                    w.write_all(&v.to_be_bytes())?;
                }
            }
            Self::LongArray(arr) => {
                w.write_all(&(arr.len() as i32).to_be_bytes())?;
                for &v in arr {
                    w.write_all(&v.to_be_bytes())?;
                }
            }
        }
        Ok(())
    }

    pub fn read_named<R: Read>(r: &mut R) -> io::Result<(String, Self)> {
        let mut id = [0u8; 1];
        r.read_exact(&mut id)?;
        if id[0] == 0 {
            return Ok((String::new(), Self::End));
        }
        let name = read_string(r)?;
        let tag = Self::read_payload(r, id[0])?;
        Ok((name, tag))
    }

    pub fn read_payload<R: Read>(r: &mut R, type_id: u8) -> io::Result<Self> {
        match type_id {
            0 => Ok(Self::End),
            1 => {
                let mut b = [0u8; 1];
                r.read_exact(&mut b)?;
                Ok(Self::Byte(b[0] as i8))
            }
            2 => {
                let mut b = [0u8; 2];
                r.read_exact(&mut b)?;
                Ok(Self::Short(i16::from_be_bytes(b)))
            }
            3 => {
                let mut b = [0u8; 4];
                r.read_exact(&mut b)?;
                Ok(Self::Int(i32::from_be_bytes(b)))
            }
            4 => {
                let mut b = [0u8; 8];
                r.read_exact(&mut b)?;
                Ok(Self::Long(i64::from_be_bytes(b)))
            }
            5 => {
                let mut b = [0u8; 4];
                r.read_exact(&mut b)?;
                Ok(Self::Float(f32::from_be_bytes(b)))
            }
            6 => {
                let mut b = [0u8; 8];
                r.read_exact(&mut b)?;
                Ok(Self::Double(f64::from_be_bytes(b)))
            }
            7 => {
                let len = read_i32(r)?.max(0) as usize;
                let mut buf = vec![0u8; len];
                r.read_exact(&mut buf)?;
                Ok(Self::ByteArray(buf.into_iter().map(|b| b as i8).collect()))
            }
            8 => Ok(Self::String(read_string(r)?)),
            9 => {
                let mut eid = [0u8; 1];
                r.read_exact(&mut eid)?;
                let len = read_i32(r)?.max(0) as usize;
                let mut items = Vec::with_capacity(len.min(65536));
                for _ in 0..len {
                    items.push(Self::read_payload(r, eid[0])?);
                }
                Ok(Self::List(items))
            }
            10 => {
                let mut map = BTreeMap::new();
                loop {
                    let (name, tag) = Self::read_named(r)?;
                    if matches!(tag, Self::End) {
                        break;
                    }
                    map.insert(name, tag);
                }
                Ok(Self::Compound(map))
            }
            11 => {
                let len = read_i32(r)?.max(0) as usize;
                let mut items = Vec::with_capacity(len.min(65536));
                for _ in 0..len {
                    items.push(read_i32(r)?);
                }
                Ok(Self::IntArray(items))
            }
            12 => {
                let len = read_i32(r)?.max(0) as usize;
                let mut items = Vec::with_capacity(len.min(65536));
                for _ in 0..len {
                    let mut b = [0u8; 8];
                    r.read_exact(&mut b)?;
                    items.push(i64::from_be_bytes(b));
                }
                Ok(Self::LongArray(items))
            }
            other => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unknown NBT tag id {other}"),
            )),
        }
    }

    pub fn to_gzip_bytes(&self, root_name: &str) -> io::Result<Vec<u8>> {
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        self.write_named(&mut enc, root_name)?;
        enc.finish()
    }

    pub fn from_gzip_bytes(bytes: &[u8]) -> io::Result<(String, Self)> {
        let mut dec = GzDecoder::new(bytes);
        Self::read_named(&mut dec)
    }

    pub fn to_zlib_bytes(&self, root_name: &str) -> io::Result<Vec<u8>> {
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
        self.write_named(&mut enc, root_name)?;
        enc.finish()
    }

    pub fn from_zlib_bytes(bytes: &[u8]) -> io::Result<(String, Self)> {
        let mut dec = ZlibDecoder::new(bytes);
        Self::read_named(&mut dec)
    }
}

fn read_i32<R: Read>(r: &mut R) -> io::Result<i32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(i32::from_be_bytes(b))
}

fn write_string<W: Write>(w: &mut W, s: &str) -> io::Result<()> {
    let bytes = s.as_bytes();
    let len = bytes.len().min(u16::MAX as usize) as u16;
    w.write_all(&len.to_be_bytes())?;
    w.write_all(&bytes[..len as usize])?;
    Ok(())
}

fn read_string<R: Read>(r: &mut R) -> io::Result<String> {
    let mut lb = [0u8; 2];
    r.read_exact(&mut lb)?;
    let len = u16::from_be_bytes(lb) as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    String::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
