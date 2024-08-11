// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::io::ErrorKind;
use std::path::Path;

use serde::de::DeserializeOwned;

use crate::cache::CacheEntry;
use crate::DenoCacheEnv;
use crate::SerializedCachedUrlMetadata;

const MAGIC_BYTES: &str = "d3n0l4nd";

pub fn write(
  env: &impl DenoCacheEnv,
  path: &Path,
  content: &[u8],
  metadata: &SerializedCachedUrlMetadata,
) -> std::io::Result<()> {
  let serialized_metadata = serde_json::to_vec(&metadata).unwrap();
  let content_size_bytes = (content.len() as u32).to_le_bytes();
  let capacity = content.len()
    + serialized_metadata.len()
    + content_size_bytes.len()
    + MAGIC_BYTES.len();
  let mut result = Vec::with_capacity(capacity);
  result.extend(content);
  result.extend(serialized_metadata);
  result.extend(content_size_bytes);
  result.extend(MAGIC_BYTES.as_bytes());
  debug_assert_eq!(result.len(), capacity);
  env.atomic_write_file(path, &result)?;
  Ok(())
}

pub fn read(
  env: &impl DenoCacheEnv,
  path: &Path,
) -> std::io::Result<Option<CacheEntry>> {
  let mut original_file_bytes = match env.read_file_bytes(path) {
    Ok(file) => file,
    Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
    Err(err) => return Err(err),
  };

  let Some((content, metadata)) =
    read_prelude_and_metadata(&original_file_bytes)
  else {
    return Ok(None);
  };

  // truncate the original bytes to just the content
  original_file_bytes.truncate(content.len());

  Ok(Some(CacheEntry {
    metadata,
    content: original_file_bytes,
  }))
}

pub fn read_metadata<TMetadata: DeserializeOwned>(
  env: &impl DenoCacheEnv,
  path: &Path,
) -> std::io::Result<Option<TMetadata>> {
  let file_bytes = match env.read_file_bytes(path) {
    Ok(file) => file,
    Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
    Err(err) => return Err(err),
  };

  let Some((_content_bytes, metadata)) =
    read_prelude_and_metadata::<TMetadata>(&file_bytes)
  else {
    return Ok(None);
  };

  Ok(Some(metadata))
}

fn read_prelude_and_metadata<TMetadata: DeserializeOwned>(
  file_bytes: &[u8],
) -> Option<(&[u8], TMetadata)> {
  let file_bytes = read_magic_bytes(file_bytes)?;
  let (file_bytes, content_len) = read_content_length(file_bytes)?;

  let metadata_len = file_bytes.len() - content_len;
  let (file_bytes, header_bytes) = read_exact_bytes(file_bytes, metadata_len)?;

  let serialized_metadata =
    serde_json::from_slice::<TMetadata>(header_bytes).ok()?;

  if file_bytes.len() != content_len {
    // corrupt
    return None;
  }

  Some((file_bytes, serialized_metadata))
}

fn read_content_length(file_bytes: &[u8]) -> Option<(&[u8], usize)> {
  if file_bytes.len() < 4 {
    return None;
  }

  let mut u32_buf: [u8; 4] = [0; 4];
  let read_index = file_bytes.len() - 4;
  u32_buf.copy_from_slice(&file_bytes[read_index..]);
  let content_len = u32::from_le_bytes(u32_buf) as usize;

  Some((&file_bytes[..read_index], content_len))
}

fn read_magic_bytes(file_bytes: &[u8]) -> Option<&[u8]> {
  let (file_bytes, magic_bytes) =
    read_exact_bytes(file_bytes, MAGIC_BYTES.len())?;
  if magic_bytes != MAGIC_BYTES.as_bytes() {
    return None;
  }
  Some(file_bytes)
}

#[inline]
fn read_exact_bytes(file_bytes: &[u8], size: usize) -> Option<(&[u8], &[u8])> {
  if file_bytes.len() < size {
    return None;
  }
  let pos = file_bytes.len() - size;
  Some((&file_bytes[..pos], &file_bytes[pos..]))
}
