// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::io::ErrorKind;
use std::path::Path;

use serde::de::DeserializeOwned;

use crate::cache::CacheEntry;
use crate::DenoCacheEnv;
use crate::SerializedCachedUrlMetadata;

// File format:
// <content>\n// denoCacheMetadata=<metadata><EOF>

const LAST_LINE_PREFIX: &[u8] = b"\n// denoCacheMetadata=";

pub fn write(
  env: &impl DenoCacheEnv,
  path: &Path,
  content: &[u8],
  metadata: &SerializedCachedUrlMetadata,
) -> std::io::Result<()> {
  let serialized_metadata = serde_json::to_vec(&metadata).unwrap();
  let capacity =
    content.len() + LAST_LINE_PREFIX.len() + serialized_metadata.len();
  let mut result = Vec::with_capacity(capacity);
  result.extend(content);
  result.extend(LAST_LINE_PREFIX);
  result.extend(serialized_metadata);
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
    read_content_and_metadata(&original_file_bytes)
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
    read_content_and_metadata::<TMetadata>(&file_bytes)
  else {
    return Ok(None);
  };

  Ok(Some(metadata))
}

fn read_content_and_metadata<TMetadata: DeserializeOwned>(
  file_bytes: &[u8],
) -> Option<(&[u8], TMetadata)> {
  let (file_bytes, metadata_bytes) = split_content_metadata(file_bytes)?;
  let serialized_metadata =
    serde_json::from_slice::<TMetadata>(metadata_bytes).ok()?;

  Some((file_bytes, serialized_metadata))
}

fn split_content_metadata(file_bytes: &[u8]) -> Option<(&[u8], &[u8])> {
  let last_newline_index = file_bytes.iter().rposition(|&b| b == b'\n')?;

  let (content, trailing_bytes) = file_bytes.split_at(last_newline_index);
  if !trailing_bytes.starts_with(LAST_LINE_PREFIX) {
    return None;
  }
  Some((content, &trailing_bytes[LAST_LINE_PREFIX.len()..]))
}
