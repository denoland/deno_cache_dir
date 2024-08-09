use std::path::Path;

use crate::cache::CacheEntry;
use crate::DenoCacheEnv;
use crate::SerializedCachedUrlMetadata;

const MAGIC_BYTES: &str = "d3n0l4nd";

pub fn write<Env: DenoCacheEnv>(
  env: &Env,
  path: &Path,
  body: &[u8],
  metadata: &SerializedCachedUrlMetadata,
) -> std::io::Result<()> {
  let path = path.with_extension("bin");
  let serialized_metadata = serde_json::to_vec(&metadata).unwrap();
  let serialized_metadata_size_bytes =
    (serialized_metadata.len() as u32).to_le_bytes();
  let body_size_bytes = (body.len() as u32).to_le_bytes();
  let capacity = MAGIC_BYTES.len()
    + serialized_metadata_size_bytes.len()
    + body_size_bytes.len()
    + serialized_metadata.len()
    + body.len();
  let mut result = Vec::with_capacity(capacity);
  result.extend(MAGIC_BYTES.as_bytes());
  result.extend(serialized_metadata_size_bytes);
  result.extend(body_size_bytes);
  result.extend(serialized_metadata);
  result.extend(body);
  debug_assert_eq!(result.len(), capacity);
  env.atomic_write_file(&path, &result)?;
  Ok(())
}

pub fn read<Env: DenoCacheEnv>(
  env: &Env,
  path: &Path,
) -> std::io::Result<Option<CacheEntry>> {
  let path = path.with_extension("bin");
  let Some(data) = env.read_file_bytes(&path)? else {
    return Ok(None);
  };

  if data.len() < MAGIC_BYTES.len() + 8
    || &data[0..MAGIC_BYTES.len()] != MAGIC_BYTES.as_bytes()
  {
    return Ok(None);
  }

  let mut pos = MAGIC_BYTES.len();
  let mut u32_buf: [u8; 4] = [0; 4];
  u32_buf.copy_from_slice(&data[pos..pos + 4]);
  let serialized_len = u32::from_le_bytes(u32_buf) as usize;
  pos += 4;
  u32_buf.copy_from_slice(&data[pos..pos + 4]);
  let body_len = u32::from_le_bytes(u32_buf) as usize;
  pos += 4;
  if data.len() != pos + serialized_len + body_len {
    return Ok(None);
  }
  let serialized_data = &data[pos..pos + serialized_len];
  pos += serialized_len;
  let body = &data[pos..pos + body_len];

  // todo(THIS PR): no unwrap
  let serialized_metadata: SerializedCachedUrlMetadata =
    serde_json::from_slice(serialized_data).unwrap();
  // todo(THIS PR): don't clone here
  let body = body.to_vec();
  Ok(Some(CacheEntry {
    body,
    metadata: serialized_metadata,
  }))
}
