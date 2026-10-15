use crate::compression;
use anyhow::{bail, Context, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use sha2::Digest;
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

const MAGIC_DBPF: u32 = 0x4650_4244;
const INDEX_VERSION: u32 = 3;
// A malformed DBPF can advertise an enormous decompressed size and otherwise
// make a read-only catalog/health scan allocate unbounded memory. Legitimate
// Sims 3 resources are well below this limit; exceeding it is reported as a
// parse error and never treated as an identity match.
const MAX_RESOURCE_MEMORY: u32 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct IndexRecord {
    type_id: u32,
    group: u32,
    instance_hi: u32,
    instance_lo: u32,
    chunk_offset: u32,
    file_size_raw: u32,
    mem_size: u32,
    compression_and_unknown: u32,
}

impl IndexRecord {
    fn field(&self, index: usize) -> u32 {
        match index {
            0 => self.type_id,
            1 => self.group,
            2 => self.instance_hi,
            3 => self.instance_lo,
            4 => self.chunk_offset,
            5 => self.file_size_raw,
            6 => self.mem_size,
            7 => self.compression_and_unknown,
            _ => unreachable!(),
        }
    }

    fn set_field(&mut self, index: usize, value: u32) {
        match index {
            0 => self.type_id = value,
            1 => self.group = value,
            2 => self.instance_hi = value,
            3 => self.instance_lo = value,
            4 => self.chunk_offset = value,
            5 => self.file_size_raw = value,
            6 => self.mem_size = value,
            7 => self.compression_and_unknown = value,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResourceEntry {
    pub type_id: u32,
    pub group: u32,
    pub instance: u64,
    pub chunk_offset: u32,
    pub file_size: u32,
    pub mem_size: u32,
    pub compressed: u16,
    pub unknown2: u16,
    pub file_size_high_bit: bool,
}

impl ResourceEntry {
    pub fn key_string(&self) -> String {
        format!(
            "0x{:08X}-0x{:08X}-0x{:016X}",
            self.type_id, self.group, self.instance
        )
    }
}

#[derive(Debug)]
pub struct Package {
    pub path: PathBuf,
    pub major: u32,
    pub minor: u32,
    pub entries: Vec<ResourceEntry>,
    payload_cache: Arc<Mutex<std::collections::HashMap<(u32, u32, u64), Vec<u8>>>>,
}

fn payload_is_reused_by_scanner(type_id: u32) -> bool {
    matches!(
        type_id,
        0x034A_EECB // CASP
            | 0x319E_4F1D // OBJD
            | 0xB52F_5055 // FBLN
            | 0x0166_038C // NMAP
            | 0x2205_57DA // STBL
            | 0x073F_AA07 // S3SA
            | 0x73E9_3EEB // manifest
            | 0x025C_95B6 // LAYO
            | 0x025E_D6F4 // SIME
            | 0x0341_ACC9 // SIMO
            // Do not retain large visual/morph payloads here. The scanner
            // fingerprints those resources but does not reuse their decoded
            // bytes within the same Package instance; retaining them made a
            // few merged packages consume hundreds of megabytes each.
    )
}

impl Package {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let meta = fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
        if meta.len() < 96 {
            bail!("file header is smaller than 96 bytes");
        }

        let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        let magic = file.read_u32::<LittleEndian>()?;
        let major = file.read_u32::<LittleEndian>()?;
        let minor = file.read_u32::<LittleEndian>()?;

        let mut skip24 = [0u8; 24];
        file.read_exact(&mut skip24)?;
        let index_count = file.read_u32::<LittleEndian>()?;
        file.read_u32::<LittleEndian>()?;
        let index_length = file.read_u32::<LittleEndian>()?;

        let mut skip12 = [0u8; 12];
        file.read_exact(&mut skip12)?;
        let index_version = file.read_u32::<LittleEndian>()?;
        let index_position = file.read_u32::<LittleEndian>()?;

        let mut skip28 = [0u8; 28];
        file.read_exact(&mut skip28)?;

        if magic != MAGIC_DBPF {
            bail!("file magic does not match DBPF");
        }
        if major != 2 {
            bail!("unsupported DBPF major version {major}");
        }
        if index_version != INDEX_VERSION {
            bail!("unsupported DBPF index version {index_version}");
        }

        if index_count == 0 {
            return Ok(Self {
                path: path.to_path_buf(),
                major,
                minor,
                entries: Vec::new(),
                payload_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
            });
        }
        if index_position == 0 {
            bail!("DBPF has entries but no index position");
        }

        let index_end = (index_position as u64)
            .checked_add(index_length as u64)
            .ok_or_else(|| anyhow::anyhow!("DBPF index range overflow"))?;
        if index_end > meta.len() {
            bail!("DBPF index points outside the file");
        }

        file.seek(SeekFrom::Start(index_position as u64))?;
        let index_type = file.read_u32::<LittleEndian>()?;

        // Sims 3 DBPF index v3 has eight DWORD fields. A SET bit means that
        // field is stored once in the index header and omitted from each entry.
        let common_field_count = (0..8)
            .filter(|bit| (index_type & (1u32 << bit)) != 0)
            .count();

        let expected = 4u64
            + (common_field_count as u64 * 4)
            + ((8usize - common_field_count) as u64 * 4 * index_count as u64);
        if index_length as u64 != expected {
            bail!(
                "corrupted DBPF index length {} (expected {})",
                index_length,
                expected
            );
        }

        let mut template = IndexRecord::default();
        for field_index in 0..8 {
            if (index_type & (1u32 << field_index)) != 0 {
                template.set_field(field_index, file.read_u32::<LittleEndian>()?);
            }
        }

        let mut entries = Vec::with_capacity(index_count as usize);
        for _ in 0..index_count {
            let mut record = template;
            for field_index in 0..8 {
                if (index_type & (1u32 << field_index)) == 0 {
                    record.set_field(field_index, file.read_u32::<LittleEndian>()?);
                } else {
                    // Explicitly read through the template accessor in debug builds,
                    // which also keeps the index layout mapping covered.
                    let _ = template.field(field_index);
                }
            }

            let chunk_end = (record.chunk_offset as u64)
                .checked_add((record.file_size_raw & 0x7FFF_FFFF) as u64)
                .ok_or_else(|| anyhow::anyhow!("resource range overflow"))?;
            if chunk_end > meta.len() {
                bail!(
                    "resource 0x{:08X}-0x{:08X}-0x{:08X}{:08X} points outside the file",
                    record.type_id,
                    record.group,
                    record.instance_hi,
                    record.instance_lo
                );
            }

            entries.push(ResourceEntry {
                type_id: record.type_id,
                group: record.group,
                instance: ((record.instance_hi as u64) << 32) | record.instance_lo as u64,
                chunk_offset: record.chunk_offset,
                file_size: record.file_size_raw & 0x7FFF_FFFF,
                mem_size: record.mem_size,
                compressed: (record.compression_and_unknown & 0xFFFF) as u16,
                unknown2: (record.compression_and_unknown >> 16) as u16,
                file_size_high_bit: (record.file_size_raw & 0x8000_0000) != 0,
            });
        }

        Ok(Self {
            path: path.to_path_buf(),
            major,
            minor,
            entries,
            payload_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
        })
    }

    pub fn raw_data(&self, entry: &ResourceEntry) -> Result<Vec<u8>> {
        let mut file = File::open(&self.path)?;
        self.raw_data_from_file(&mut file, entry)
    }

    pub fn raw_data_from_file(&self, file: &mut File, entry: &ResourceEntry) -> Result<Vec<u8>> {
        file.seek(SeekFrom::Start(entry.chunk_offset as u64))?;
        let mut data = vec![0u8; entry.file_size as usize];
        file.read_exact(&mut data)?;
        Ok(data)
    }

    pub fn data(&self, entry: &ResourceEntry) -> Result<Vec<u8>> {
        let mut file = File::open(&self.path)?;
        self.data_from_file(&mut file, entry)
    }

    pub fn data_from_file(&self, file: &mut File, entry: &ResourceEntry) -> Result<Vec<u8>> {
        self.data_from_file_with_digest(file, entry)
            .map(|(data, _)| data)
    }

    pub fn data_from_file_with_digest(
        &self,
        file: &mut File,
        entry: &ResourceEntry,
    ) -> Result<(Vec<u8>, [u8; 32])> {
        if entry.mem_size > MAX_RESOURCE_MEMORY {
            bail!(
                "resource {} declares unsupported decompressed size {} bytes",
                entry.key_string(),
                entry.mem_size
            );
        }
        let cache_key = (entry.type_id, entry.group, entry.instance);
        if payload_is_reused_by_scanner(entry.type_id) {
            if let Some(cached) = self
                .payload_cache
                .lock()
                .map_err(|_| anyhow::anyhow!("package payload cache poisoned"))?
                .get(&cache_key)
                .cloned()
            {
                return Ok((cached.clone(), sha2::Sha256::digest(&cached).into()));
            }
        }
        let raw = self.raw_data_from_file(file, entry)?;
        let (data, digest) = match entry.compressed {
            0x0000 => {
                if entry.file_size != entry.mem_size {
                    bail!(
                        "resource {} is marked uncompressed but disk size {} != memory size {}",
                        entry.key_string(),
                        entry.file_size,
                        entry.mem_size
                    );
                }
                Ok((raw.clone(), sha2::Sha256::digest(&raw).into()))
            }
            0xFFFF => compression::uncompress_stream_with_digest(
                &raw,
                entry.file_size as usize,
                entry.mem_size as usize,
            )
            .with_context(|| format!("decompress {}", entry.key_string())),
            other => bail!(
                "resource {} uses unsupported compression flag 0x{other:04X}",
                entry.key_string()
            ),
        }?;
        if payload_is_reused_by_scanner(entry.type_id) {
            self.payload_cache
                .lock()
                .map_err(|_| anyhow::anyhow!("package payload cache poisoned"))?
                .insert(cache_key, data.clone());
        }
        Ok((data, digest))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_record_field_mapping_covers_all_eight_fields() {
        let mut record = IndexRecord::default();
        for index in 0..8 {
            record.set_field(index, (index as u32) + 10);
        }
        for index in 0..8 {
            assert_eq!(record.field(index), (index as u32) + 10);
        }
    }
}
