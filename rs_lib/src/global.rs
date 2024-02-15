// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use anyhow::Error as AnyError;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use url::Url;

use super::cache::HttpCache;
use super::cache::HttpCacheItemKey;
use super::common::DenoCacheEnv;
use crate::cache::url_to_filename;
use crate::cache::CacheReadFileError;
use crate::cache::Checksum;
use crate::cache::GlobalToLocalCopy;
use crate::cache::SerializedCachedUrlMetadata;
use crate::cache::UrlToFilenameConversionError;
use crate::common::checksum;
use crate::common::HeadersMap;
use crate::ChecksumIntegrityError;

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
  ) -> Result<PathBuf, AnyError> {
    Ok(self.path.join(url_to_filename(url)?))
  }

  fn get_cache_filepath(
    &self,
    url: &Url,
  ) -> Result<PathBuf, UrlToFilenameConversionError> {
    Ok(self.path.join(url_to_filename(url)?))
  }

  #[inline]
  fn key_file_path<'a>(&self, key: &'a HttpCacheItemKey) -> &'a PathBuf {
    // The key file path is always set for the global cache because
    // the file will always exist, unlike the local cache, which won't
    // have this for redirects.
    key.file_path.as_ref().unwrap()
  }

  fn read_serialized_cache_metadata<T: DeserializeOwned>(
    &self,
    key: &HttpCacheItemKey,
  ) -> Result<Option<T>, AnyError> {
    let path = self.key_file_path(key).with_extension("metadata.json");
    let bytes = self.env.read_file_bytes(&path)?;
    Ok(match bytes {
      Some(bytes) => Some(serde_json::from_slice::<T>(&bytes)?),
      None => None,
    })
  }
}

impl<Env: DenoCacheEnv> HttpCache for GlobalHttpCache<Env> {
  fn cache_item_key<'a>(
    &self,
    url: &'a Url,
  ) -> Result<HttpCacheItemKey<'a>, UrlToFilenameConversionError> {
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
  ) -> Result<Option<SystemTime>, AnyError> {
    #[cfg(debug_assertions)]
    debug_assert!(!key.is_local_key);

    Ok(self.env.modified(self.key_file_path(key))?)
  }

  fn set(
    &self,
    url: &Url,
    headers: HeadersMap,
    content: &[u8],
  ) -> Result<(), AnyError> {
    let cache_filepath = self.get_cache_filepath(url)?;
    // Cache content
    self.env.atomic_write_file(&cache_filepath, content)?;

    write_metadata(
      &self.env,
      &cache_filepath,
      &SerializedCachedUrlMetadata {
        time: Some(self.env.time_now()),
        url: url.to_string(),
        headers,
      },
    )?;

    Ok(())
  }

  fn read_file_bytes(
    &self,
    key: &HttpCacheItemKey,
    maybe_checksum: Option<Checksum>,
    _allow_global_to_local: GlobalToLocalCopy,
  ) -> Result<Option<Vec<u8>>, CacheReadFileError> {
    #[cfg(debug_assertions)]
    debug_assert!(!key.is_local_key);

    let maybe_file_bytes = self.env.read_file_bytes(self.key_file_path(key))?;

    if let Some(file_bytes) = &maybe_file_bytes {
      if let Some(expected_checksum) = maybe_checksum {
        let actual = checksum(file_bytes);
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

    Ok(maybe_file_bytes)
  }

  fn read_headers(
    &self,
    key: &HttpCacheItemKey,
  ) -> Result<Option<HeadersMap>, AnyError> {
    // targeted deserialize
    #[derive(Deserialize)]
    struct SerializedHeaders {
      pub headers: HeadersMap,
    }

    #[cfg(debug_assertions)]
    debug_assert!(!key.is_local_key);
    Ok(
      self
        .read_serialized_cache_metadata::<SerializedHeaders>(key)?
        .map(|item| item.headers),
    )
  }

  fn read_download_time(
    &self,
    key: &HttpCacheItemKey,
  ) -> Result<Option<SystemTime>, AnyError> {
    // targeted deserialize
    #[derive(Deserialize)]
    struct SerializedTime {
      pub now: Option<SystemTime>,
    }

    #[cfg(debug_assertions)]
    debug_assert!(!key.is_local_key);
    Ok(
      self
        .read_serialized_cache_metadata::<SerializedTime>(key)?
        .and_then(|item| item.now),
    )
  }
}

fn write_metadata<Env: DenoCacheEnv>(
  env: &Env,
  path: &Path,
  meta_data: &SerializedCachedUrlMetadata,
) -> Result<(), AnyError> {
  let path = path.with_extension("metadata.json");
  let json = serde_json::to_string_pretty(meta_data)?;
  env.atomic_write_file(&path, json.as_bytes())?;
  Ok(())
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
