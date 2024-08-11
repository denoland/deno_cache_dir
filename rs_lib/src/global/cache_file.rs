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
  let serialized_metadata_size_bytes =
    (serialized_metadata.len() as u32).to_le_bytes();
  let content_size_bytes = (content.len() as u32).to_le_bytes();
  let capacity = MAGIC_BYTES.len()
    + serialized_metadata_size_bytes.len()
    + content_size_bytes.len()
    + serialized_metadata.len()
    + content.len();
  let mut result = Vec::with_capacity(capacity);
  result.extend(MAGIC_BYTES.as_bytes());
  result.extend(serialized_metadata_size_bytes);
  result.extend(content_size_bytes);
  result.extend(serialized_metadata);
  result.extend(content);
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

  let Some((file_bytes, (prelude, metadata))) =
    read_prelude_and_metadata(&original_file_bytes)
  else {
    return Ok(None);
  };

  let Some((file_bytes, content)) =
    read_exact_bytes(file_bytes, prelude.content_len)
  else {
    return Ok(None);
  };

  if !file_bytes.is_empty() {
    return Ok(None); // corrupt
  }

  // reuse the original_file_bytes vector to store the content
  let content_len = content.len();
  let content_index = original_file_bytes.len() - content_len;
  original_file_bytes
    .copy_within(content_index..content_index + content_len, 0);
  original_file_bytes.truncate(content_len);

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

  let Some((file_bytes, (prelude, metadata))) =
    read_prelude_and_metadata::<TMetadata>(&file_bytes)
  else {
    return Ok(None);
  };

  // skip over the content and just ensure the file isn't corrupted and
  // has the trailer in the correct position
  let Some((file_bytes, _)) = read_exact_bytes(file_bytes, prelude.content_len)
  else {
    return Ok(None);
  };
  if !file_bytes.is_empty() {
    return Ok(None); // corrupt
  }

  Ok(Some(metadata))
}

type PreludeAndMetadata<TMetadata> = (Prelude, TMetadata);

fn read_prelude_and_metadata<TMetadata: DeserializeOwned>(
  file_bytes: &[u8],
) -> Option<(&[u8], PreludeAndMetadata<TMetadata>)> {
  let file_bytes = read_magic_bytes(file_bytes)?;
  let (file_bytes, prelude) = read_prelude(file_bytes)?;

  let (file_bytes, header_bytes) =
    read_exact_bytes(file_bytes, prelude.metadata_len)?;

  let serialized_metadata =
    serde_json::from_slice::<TMetadata>(header_bytes).ok()?;

  Some((file_bytes, (prelude, serialized_metadata)))
}

struct Prelude {
  metadata_len: usize,
  content_len: usize,
}

fn read_prelude(file_bytes: &[u8]) -> Option<(&[u8], Prelude)> {
  let mut u32_buf: [u8; 4] = [0; 4];
  u32_buf.copy_from_slice(&file_bytes[..4]);
  let metadata_len = u32::from_le_bytes(u32_buf) as usize;
  u32_buf.copy_from_slice(&file_bytes[4..8]);
  let content_len = u32::from_le_bytes(u32_buf) as usize;

  Some((
    &file_bytes[8..],
    Prelude {
      metadata_len,
      content_len,
    },
  ))
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
  Some((&file_bytes[size..], &file_bytes[..size]))
}
