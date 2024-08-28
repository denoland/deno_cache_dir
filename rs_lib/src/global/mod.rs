// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use serde::Deserialize;
use url::Url;

use super::cache::HttpCache;
use super::cache::HttpCacheItemKey;
use super::env::DenoCacheEnv;
use crate::cache::url_to_filename;
use crate::cache::CacheEntry;
use crate::cache::CacheReadFileError;
use crate::cache::Checksum;
use crate::cache::SerializedCachedUrlMetadata;
use crate::common::checksum;
use crate::common::HeadersMap;
use crate::ChecksumIntegrityError;

mod cache_file;

#[derive(Debug)]
pub struct GlobalHttpCache<Env: DenoCacheEnv> {
  path: PathBuf,
  pub(crate) env: Env,
}

impl<Env: DenoCacheEnv> GlobalHttpCache<Env> {
  pub fn new(path: PathBuf, env: Env) -> Self {
    #[cfg(not(feature = "wasm"))]
    assert!(path.is_absolute());
    Self { path, env }
  }

  pub fn get_global_cache_location(&self) -> &PathBuf {
    &self.path
  }

  pub fn get_global_cache_filepath(
    &self,
    url: &Url,
  ) -> std::io::Result<PathBuf> {
    Ok(self.path.join(url_to_filename(url)?))
  }

  fn get_cache_filepath(&self, url: &Url) -> std::io::Result<PathBuf> {
    Ok(self.path.join(url_to_filename(url)?))
  }

  #[inline]
  fn key_file_path<'a>(&self, key: &'a HttpCacheItemKey) -> &'a PathBuf {
    // The key file path is always set for the global cache because
    // the file will always exist, unlike the local cache, which won't
    // have this for redirects.
    key.file_path.as_ref().unwrap()
  }
}

impl<Env: DenoCacheEnv> HttpCache for GlobalHttpCache<Env> {
  fn cache_item_key<'a>(
    &self,
    url: &'a Url,
  ) -> std::io::Result<HttpCacheItemKey<'a>> {
    Ok(HttpCacheItemKey {
      #[cfg(debug_assertions)]
      is_local_key: false,
      url,
      file_path: Some(self.get_cache_filepath(url)?),
    })
  }

  fn contains(&self, url: &Url) -> bool {
    let Ok(cache_filepath) = self.get_cache_filepath(url) else {
      return false;
    };
    self.env.is_file(&cache_filepath)
  }

  fn read_modified_time(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<SystemTime>> {
    #[cfg(debug_assertions)]
    debug_assert!(!key.is_local_key);

    self.env.modified(self.key_file_path(key))
  }

  fn set(
    &self,
    url: &Url,
    headers: HeadersMap,
    content: &[u8],
  ) -> std::io::Result<()> {
    let cache_filepath = self.get_cache_filepath(url)?;
    cache_file::write(
      &self.env,
      &cache_filepath,
      content,
      &SerializedCachedUrlMetadata {
        time: Some(
          self
            .env
            .time_now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        ),
        url: url.to_string(),
        headers,
      },
    )
    .unwrap();

    Ok(())
  }

  fn get(
    &self,
    key: &HttpCacheItemKey,
    maybe_checksum: Option<Checksum>,
  ) -> Result<Option<CacheEntry>, CacheReadFileError> {
    #[cfg(debug_assertions)]
    debug_assert!(!key.is_local_key);

    let file_path = self.key_file_path(key);
    let maybe_file = match cache_file::read(&self.env, file_path) {
      Ok(maybe_file) => maybe_file,
      Err(cache_file::ReadError::Io(err)) => return Err(CacheReadFileError::Io(err)),
      Err(cache_file::ReadError::InvalidFormat) => {
        handle_maybe_deno_1_x_cache_entry(&self.env, file_path);
        None
      }
    };

    if let Some(file) = &maybe_file {
      if let Some(expected_checksum) = maybe_checksum {
        let actual = checksum(&file.content);
        if expected_checksum.as_str() != actual {
          return Err(CacheReadFileError::ChecksumIntegrity(Box::new(
            ChecksumIntegrityError {
              url: key.url.clone(),
              expected: expected_checksum.as_str().to_string(),
              actual,
            },
          )));
        }
      }
    }

    Ok(maybe_file)
  }

  fn read_headers(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<HeadersMap>> {
    // targeted deserialize
    #[derive(Deserialize)]
    struct SerializedHeaders {
      pub headers: HeadersMap,
    }

    #[cfg(debug_assertions)]
    debug_assert!(!key.is_local_key);

    let maybe_metadata = cache_file::read_metadata::<SerializedHeaders>(
      &self.env,
      self.key_file_path(key),
    )?;
    Ok(maybe_metadata.map(|m| m.headers))
  }

  fn read_download_time(
    &self,
    key: &HttpCacheItemKey,
  ) -> std::io::Result<Option<SystemTime>> {
    // targeted deserialize
    #[derive(Deserialize)]
    struct SerializedTime {
      pub time: Option<u64>,
    }

    #[cfg(debug_assertions)]
    debug_assert!(!key.is_local_key);
    let maybe_metadata = cache_file::read_metadata::<SerializedTime>(
      &self.env,
      self.key_file_path(key),
    )?;
    Ok(maybe_metadata.and_then(|m| {
      Some(SystemTime::UNIX_EPOCH + Duration::from_secs(m.time?))
    }))
  }
}

fn handle_maybe_deno_1_x_cache_entry(env: &impl DenoCacheEnv, file_path: &Path) {
  // Deno 1.x structures its cache in two separate files using
  // the same name for the content, but a separate
  // <filename>.metadata.json file.
  //
  // We don't want the following scenario to happen:
  //
  // 1. User generates the cache on Deno 1.x.
  //    - <filename> and <filename>.metadata.json are created.
  // 2. User updates to Deno 2 and is updated to the new cache.
  //    - <filename> is updated to new single file format
  // 3. User downgrades to Deno 1.x.
  //    - <filename> is now using the new Deno 2.0 format which
  //      is incorrect and has a different content than if they
  //      cached on Deno 1.x
  //
  // To prevent this scenario, check for the precence of the Deno 1.x
  // <filename>.metadata.json file. If it exists, delete it.
  let metadata_file = file_path.with_extension("metadata.json");
  if env.is_file(&metadata_file) {
    // delete the Deno 1.x cache files, deleting the metadata.json
    // file first in case the process exits between these two statements
    let _ = env.remove_file(&file_path.with_extension("metadata.json"));
    let _ = env.remove_file(file_path);
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_url_to_filename() {
    let test_cases = [
      ("https://deno.land/x/foo.ts", "https/deno.land/2c0a064891b9e3fbe386f5d4a833bce5076543f5404613656042107213a7bbc8"),
      (
        "https://deno.land:8080/x/foo.ts",
        "https/deno.land_PORT8080/2c0a064891b9e3fbe386f5d4a833bce5076543f5404613656042107213a7bbc8",
      ),
      ("https://deno.land/", "https/deno.land/8a5edab282632443219e051e4ade2d1d5bbc671c781051bf1437897cbdfea0f1"),
      (
        "https://deno.land/?asdf=qwer",
        "https/deno.land/e4edd1f433165141015db6a823094e6bd8f24dd16fe33f2abd99d34a0a21a3c0",
      ),
      // should be the same as case above, fragment (#qwer) is ignored
      // when hashing
      (
        "https://deno.land/?asdf=qwer#qwer",
        "https/deno.land/e4edd1f433165141015db6a823094e6bd8f24dd16fe33f2abd99d34a0a21a3c0",
      ),
      (
        "data:application/typescript;base64,ZXhwb3J0IGNvbnN0IGEgPSAiYSI7CgpleHBvcnQgZW51bSBBIHsKICBBLAogIEIsCiAgQywKfQo=",
        "data/c21c7fc382b2b0553dc0864aa81a3acacfb7b3d1285ab5ae76da6abec213fb37",
      ),
      (
        "data:text/plain,Hello%2C%20Deno!",
        "data/967374e3561d6741234131e342bf5c6848b70b13758adfe23ee1a813a8131818",
      )
    ];

    for (url, expected) in test_cases.iter() {
      let u = Url::parse(url).unwrap();
      let p = url_to_filename(&u).unwrap();
      assert_eq!(p, PathBuf::from(expected));
    }
  }
}
