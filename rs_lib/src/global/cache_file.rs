use std::io::ErrorKind;
use std::path::Path;

use serde::de::DeserializeOwned;

use crate::cache::CacheEntry;
use crate::DenoCacheEnv;
use crate::DenoCacheEnvFsFile;
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
  let capacity = MAGIC_BYTES.len() * 3
    + serialized_metadata_size_bytes.len()
    + content_size_bytes.len()
    + serialized_metadata.len()
    + content.len();
  let mut result = Vec::with_capacity(capacity);
  result.extend(MAGIC_BYTES.as_bytes());
  result.extend(serialized_metadata_size_bytes);
  result.extend(content_size_bytes);
  result.extend(serialized_metadata);
  result.extend(MAGIC_BYTES.as_bytes());
  result.extend(content);
  result.extend(MAGIC_BYTES.as_bytes());
  debug_assert_eq!(result.len(), capacity);
  env.atomic_write_file(path, &result)?;
  Ok(())
}

pub fn read(
  env: &impl DenoCacheEnv,
  path: &Path,
) -> std::io::Result<Option<CacheEntry>> {
  let Some((mut file, prelude, metadata)) =
    open_read_prelude_and_metadata(env, path)?
  else {
    return Ok(None);
  };

  let Some(content) =
    read_exact_bytes_with_trailer(&mut *file, prelude.content_len)?
  else {
    return Ok(None);
  };

  Ok(Some(CacheEntry { metadata, content }))
}

pub fn read_metadata<TMetadata: DeserializeOwned>(
  env: &impl DenoCacheEnv,
  path: &Path,
) -> std::io::Result<Option<TMetadata>> {
  let Some((mut file, prelude, metadata)) =
    open_read_prelude_and_metadata::<TMetadata>(env, path)?
  else {
    return Ok(None);
  };

  // skip over the content and just ensure the file isn't corrupted and
  // has the trailer in the correct position
  file.seek_relative(prelude.content_len as i64)?;
  let Some(read_magic_bytes) = read_exact_bytes(&mut *file, MAGIC_BYTES.len())?
  else {
    return Ok(None);
  };
  if read_magic_bytes != MAGIC_BYTES.as_bytes() {
    return Ok(None);
  }

  Ok(Some(metadata))
}

type FilePreludeAndMetadata<TMetadata> =
  (Box<dyn DenoCacheEnvFsFile>, Prelude, TMetadata);

fn open_read_prelude_and_metadata<TMetadata: DeserializeOwned>(
  env: &impl DenoCacheEnv,
  path: &Path,
) -> std::io::Result<Option<FilePreludeAndMetadata<TMetadata>>> {
  let mut original_file = match env.open_read(path) {
    Ok(file) => file,
    Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
    Err(err) => return Err(err),
  };
  let file = &mut *original_file;

  let Some(prelude) = read_prelude(file)? else {
    return Ok(None);
  };

  let Some(header_bytes) =
    read_exact_bytes_with_trailer(file, prelude.metadata_len)?
  else {
    return Ok(None);
  };

  let Ok(serialized_metadata) =
    serde_json::from_slice::<TMetadata>(&header_bytes)
  else {
    return Ok(None);
  };

  Ok(Some((original_file, prelude, serialized_metadata)))
}

struct Prelude {
  metadata_len: usize,
  content_len: usize,
}

fn read_prelude(
  file: &mut dyn DenoCacheEnvFsFile,
) -> std::io::Result<Option<Prelude>> {
  let Some(prelude) = read_exact_bytes(file, MAGIC_BYTES.len() + 8)? else {
    return Ok(None);
  };
  if &prelude[0..MAGIC_BYTES.len()] != MAGIC_BYTES.as_bytes() {
    return Ok(None);
  }

  let mut pos = MAGIC_BYTES.len();
  let mut u32_buf: [u8; 4] = [0; 4];
  u32_buf.copy_from_slice(&prelude[pos..pos + 4]);
  let metadata_len = u32::from_le_bytes(u32_buf) as usize;
  pos += 4;
  u32_buf.copy_from_slice(&prelude[pos..pos + 4]);
  let content_len = u32::from_le_bytes(u32_buf) as usize;

  Ok(Some(Prelude {
    metadata_len,
    content_len,
  }))
}

fn read_exact_bytes_with_trailer(
  file: &mut dyn DenoCacheEnvFsFile,
  size: usize,
) -> std::io::Result<Option<Vec<u8>>> {
  let Some(mut bytes) = read_exact_bytes(file, size + MAGIC_BYTES.len())?
  else {
    return Ok(None);
  };
  if !bytes.ends_with(MAGIC_BYTES.as_bytes()) {
    return Ok(None);
  }
  bytes.truncate(size);
  Ok(Some(bytes))
}

fn read_exact_bytes(
  file: &mut dyn DenoCacheEnvFsFile,
  size: usize,
) -> std::io::Result<Option<Vec<u8>>> {
  let mut bytes = vec![0u8; size];
  let read_size = file.read(&mut bytes)?;
  if read_size != bytes.len() {
    return Ok(None);
  }
  Ok(Some(bytes))
}
