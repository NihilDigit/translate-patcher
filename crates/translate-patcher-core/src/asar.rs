use std::{fs, path::Path};

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct AsarArchive {
    bytes: Vec<u8>,
    header: Value,
    header_size: usize,
}

#[derive(Debug, Clone)]
pub struct AsarFile {
    pub path: String,
    pub size: usize,
}

#[derive(Debug, Clone)]
pub struct Replacement {
    pub path: String,
    pub content: Vec<u8>,
}

impl AsarArchive {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
        Self::from_bytes(bytes).with_context(|| format!("failed to parse {}", path.display()))
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() < 16 {
            bail!("ASAR archive is too small");
        }

        let header_size = read_u32(&bytes, 4)? as usize;
        let json_size = read_u32(&bytes, 12)? as usize;
        let json_start = 16;
        let json_end = json_start + json_size;
        if bytes.len() < json_end {
            bail!("ASAR header is truncated");
        }

        let header: Value = serde_json::from_slice(&bytes[json_start..json_end])?;
        if header.get("files").and_then(Value::as_object).is_none() {
            bail!("ASAR header does not contain a files object");
        }

        Ok(Self {
            bytes,
            header,
            header_size,
        })
    }

    pub fn files(&self) -> Vec<AsarFile> {
        let mut out = Vec::new();
        collect_files(&self.header, "", &mut out);
        out
    }

    pub fn read_file(&self, path: &str) -> Result<&[u8]> {
        let entry = self
            .entry(path)
            .with_context(|| format!("file not found in ASAR: {path}"))?;
        let offset = entry_offset(entry)?;
        let size = entry_size(entry)?;
        let start = self.data_base() + offset;
        let end = start + size;
        self.bytes
            .get(start..end)
            .ok_or_else(|| anyhow!("file content range is out of bounds: {path}"))
    }

    pub fn repack_with_replacements(&self, replacements: &[Replacement]) -> Result<Vec<u8>> {
        let mut header = self.header.clone();
        let mut files = Vec::new();
        let mut offset = 0usize;
        repack_node(&mut header, "", self, replacements, &mut offset, &mut files)?;

        let header_json = serde_json::to_vec(&header)?;
        let padding = (4 - (header_json.len() % 4)) % 4;
        let inner_size = 4 + header_json.len() + padding;
        let packed_header_size = inner_size + 4;

        let mut out = Vec::with_capacity(16 + header_json.len() + padding + offset);
        out.extend_from_slice(&4u32.to_le_bytes());
        out.extend_from_slice(&(packed_header_size as u32).to_le_bytes());
        out.extend_from_slice(&(inner_size as u32).to_le_bytes());
        out.extend_from_slice(&(header_json.len() as u32).to_le_bytes());
        out.extend_from_slice(&header_json);
        out.extend(std::iter::repeat(0).take(padding));
        for file in files {
            out.extend(file);
        }
        Ok(out)
    }

    fn data_base(&self) -> usize {
        8 + self.header_size
    }

    fn entry(&self, path: &str) -> Result<&Value> {
        let mut node = &self.header;
        for part in path.split('/') {
            node = node
                .get("files")
                .and_then(Value::as_object)
                .and_then(|files| files.get(part))
                .ok_or_else(|| anyhow!("missing ASAR path component: {part}"))?;
        }
        Ok(node)
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| anyhow!("missing u32 at offset {offset}"))?;
    Ok(u32::from_le_bytes(slice.try_into().unwrap()))
}

fn collect_files(node: &Value, prefix: &str, out: &mut Vec<AsarFile>) {
    let Some(files) = node.get("files").and_then(Value::as_object) else {
        return;
    };

    for (name, entry) in files {
        let path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        };
        if entry.get("files").is_some() {
            collect_files(entry, &path, out);
        } else if let Some(size) = entry.get("size").and_then(Value::as_u64) {
            out.push(AsarFile {
                path,
                size: size as usize,
            });
        }
    }
}

fn repack_node(
    node: &mut Value,
    prefix: &str,
    archive: &AsarArchive,
    replacements: &[Replacement],
    offset: &mut usize,
    file_contents: &mut Vec<Vec<u8>>,
) -> Result<()> {
    let files = node
        .get_mut("files")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| anyhow!("ASAR node missing files object"))?;

    for (name, entry) in files {
        let path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        };

        if entry.get("files").is_some() {
            repack_node(entry, &path, archive, replacements, offset, file_contents)?;
            continue;
        }

        let content = if let Some(replacement) = replacements.iter().find(|item| item.path == path)
        {
            replacement.content.clone()
        } else {
            archive.read_file(&path)?.to_vec()
        };

        update_file_entry(entry, *offset, &content)?;
        *offset += content.len();
        file_contents.push(content);
    }

    Ok(())
}

fn update_file_entry(entry: &mut Value, offset: usize, content: &[u8]) -> Result<()> {
    let object = entry
        .as_object_mut()
        .ok_or_else(|| anyhow!("ASAR file entry is not an object"))?;
    object.insert("offset".to_string(), Value::String(offset.to_string()));
    object.insert("size".to_string(), json!(content.len()));
    if object.contains_key("integrity") {
        object.insert("integrity".to_string(), integrity(content));
    }
    Ok(())
}

fn entry_offset(entry: &Value) -> Result<usize> {
    if let Some(offset) = entry.get("offset").and_then(Value::as_str) {
        return offset
            .parse()
            .with_context(|| format!("invalid ASAR offset: {offset}"));
    }
    if let Some(offset) = entry.get("offset").and_then(Value::as_u64) {
        return Ok(offset as usize);
    }
    bail!("ASAR file entry is missing offset")
}

fn entry_size(entry: &Value) -> Result<usize> {
    entry
        .get("size")
        .and_then(Value::as_u64)
        .map(|size| size as usize)
        .ok_or_else(|| anyhow!("ASAR file entry is missing size"))
}

fn integrity(content: &[u8]) -> Value {
    const BLOCK_SIZE: usize = 4 * 1024 * 1024;
    let blocks: Vec<_> = content.chunks(BLOCK_SIZE).map(sha256_hex).collect();
    json!({
        "algorithm": "SHA256",
        "hash": sha256_hex(content),
        "blockSize": BLOCK_SIZE,
        "blocks": blocks,
    })
}

fn sha256_hex(content: &[u8]) -> String {
    let digest = Sha256::digest(content);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn write_asar(path: impl AsRef<Path>, bytes: &[u8]) -> Result<()> {
    let path = path.as_ref();
    let tmp = path.with_extension("asar.tmp");
    fs::write(&tmp, bytes).with_context(|| format!("failed to write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| {
        format!(
            "failed to replace {} with {}",
            path.display(),
            tmp.display()
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AsarArchive, Replacement};

    #[test]
    fn repacks_replaced_file() {
        let original = test_asar(&[("data/scenario/scene1.ks", "hello"), ("main.js", "main")]);
        let archive = AsarArchive::from_bytes(original).unwrap();
        let out = archive
            .repack_with_replacements(&[Replacement {
                path: "data/scenario/scene1.ks".to_string(),
                content: b"world".to_vec(),
            }])
            .unwrap();
        let repacked = AsarArchive::from_bytes(out).unwrap();
        assert_eq!(
            repacked.read_file("data/scenario/scene1.ks").unwrap(),
            b"world"
        );
        assert_eq!(repacked.read_file("main.js").unwrap(), b"main");
    }

    fn test_asar(files: &[(&str, &str)]) -> Vec<u8> {
        let mut header = serde_json::json!({ "files": {} });
        let mut offset = 0usize;
        for (path, content) in files {
            insert_entry(&mut header, path, offset, content.len());
            offset += content.len();
        }
        let header_json = serde_json::to_vec(&header).unwrap();
        let padding = (4 - (header_json.len() % 4)) % 4;
        let inner_size = 4 + header_json.len() + padding;
        let packed_header_size = inner_size + 4;
        let mut out = Vec::new();
        out.extend_from_slice(&4u32.to_le_bytes());
        out.extend_from_slice(&(packed_header_size as u32).to_le_bytes());
        out.extend_from_slice(&(inner_size as u32).to_le_bytes());
        out.extend_from_slice(&(header_json.len() as u32).to_le_bytes());
        out.extend_from_slice(&header_json);
        out.extend(std::iter::repeat(0).take(padding));
        for (_, content) in files {
            out.extend_from_slice(content.as_bytes());
        }
        out
    }

    fn insert_entry(header: &mut serde_json::Value, path: &str, offset: usize, size: usize) {
        let mut node = header;
        let parts: Vec<_> = path.split('/').collect();
        for part in &parts[..parts.len() - 1] {
            let files = node.get_mut("files").unwrap().as_object_mut().unwrap();
            node = files
                .entry((*part).to_string())
                .or_insert_with(|| serde_json::json!({ "files": {} }));
        }
        node.get_mut("files")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(
                parts.last().unwrap().to_string(),
                serde_json::json!({ "offset": offset.to_string(), "size": size }),
            );
    }
}
