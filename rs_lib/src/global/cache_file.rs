use std::io::ErrorKind;
use std::path::Path;

use crate::cache::CacheEntry;
use crate::DenoCacheEnv;
use crate::DenoCacheEnvFsFile;
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
  let capacity = MAGIC_BYTES.len() * 3
    + serialized_metadata_size_bytes.len()
    + body_size_bytes.len()
    + serialized_metadata.len()
    + body.len();
  let mut result = Vec::with_capacity(capacity);
  result.extend(MAGIC_BYTES.as_bytes());
  result.extend(serialized_metadata_size_bytes);
  result.extend(body_size_bytes);
  result.extend(serialized_metadata);
  result.extend(MAGIC_BYTES.as_bytes());
  result.extend(body);
  result.extend(MAGIC_BYTES.as_bytes());
  debug_assert_eq!(result.len(), capacity);
  env.atomic_write_file(&path, &result)?;
  Ok(())
}

pub fn read<Env: DenoCacheEnv>(
  env: &Env,
  path: &Path,
) -> std::io::Result<Option<CacheEntry>> {
  let path = path.with_extension("bin");
  let mut file = match env.open_read(&path) {
    Ok(file) => file,
    Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
    Err(err) => return Err(err),
  };
  let file = &mut *file;

  let Some(prelude) = read_prelude(file)? else {
    return Ok(None);
  };

  let Some(header_bytes) = read_exact_bytes_with_trailer(file, prelude.header_len)? else {
    return Ok(None);
  };

  // todo(THIS PR): no unwrap
  let serialized_metadata: SerializedCachedUrlMetadata =
    serde_json::from_slice(&header_bytes).unwrap();

  let Some(body) = read_exact_bytes_with_trailer(file, prelude.body_len)? else {
    return Ok(None);
  };

  Ok(Some(CacheEntry {
    body,
    metadata: serialized_metadata,
  }))
}

struct Prelude {
  header_len: usize,
  body_len: usize,
}

fn read_prelude(file: &mut dyn DenoCacheEnvFsFile) -> std::io::Result<Option<Prelude>> {
  let Some(prelude) = read_exact_bytes(file, MAGIC_BYTES.len() + 8)? else {
    return Ok(None);
  };
  if &prelude[0..MAGIC_BYTES.len()] != MAGIC_BYTES.as_bytes()
  {
    return Ok(None);
  }

  let mut pos = MAGIC_BYTES.len();
  let mut u32_buf: [u8; 4] = [0; 4];
  u32_buf.copy_from_slice(&prelude[pos..pos + 4]);
  let header_len = u32::from_le_bytes(u32_buf) as usize;
  pos += 4;
  u32_buf.copy_from_slice(&prelude[pos..pos + 4]);
  let body_len = u32::from_le_bytes(u32_buf) as usize;

  Ok(Some(Prelude {
    header_len,
    body_len,
  }))
}

fn read_exact_bytes(file: &mut dyn DenoCacheEnvFsFile, size: usize) -> std::io::Result<Option<Vec<u8>>> {
  let mut bytes = vec![0u8; size];
  let read_size = file.read(&mut bytes)?;
  if read_size != bytes.len() {
    return Ok(None);
  }
  Ok(Some(bytes))
}

fn read_exact_bytes_with_trailer(file: &mut dyn DenoCacheEnvFsFile, size: usize) -> std::io::Result<Option<Vec<u8>>> {
  let Some(mut bytes) = read_exact_bytes(file, size + MAGIC_BYTES.len())? else {
    return Ok(None);
  };
  if !bytes.ends_with(MAGIC_BYTES.as_bytes()) {
    return Ok(None);
  }
  bytes.truncate(size);
  Ok(Some(bytes))
}